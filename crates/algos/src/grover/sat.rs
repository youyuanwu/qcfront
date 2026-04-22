//! Circuit-based SAT oracle for Grover's algorithm.
//!
//! [`CnfOracle`] evaluates a CNF formula reversibly on quantum inputs using
//! per-clause De Morgan evaluation. Implements [`Oracle`] directly — no
//! classical pre-solving.

use std::cmp::max;
use std::collections::HashSet;
use std::num::NonZeroUsize;

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::multi_cx;
use crate::circuits::multi_cz;
use crate::circuits::transform;
use crate::sat::Clause;

use super::Oracle;

/// Circuit-based SAT oracle using per-clause De Morgan evaluation.
///
/// Builds a quantum circuit that evaluates the CNF formula on a
/// superposition of all assignments, providing genuine quantum
/// advantage for Grover's search. No classical pre-solving.
///
/// The constructor canonicalizes each clause:
/// - Deduplicates identical literals within a clause
/// - Drops tautological clauses (containing both `x` and `¬x`)
///
/// # Panics
/// - If `num_vars < 2`
/// - If no clauses remain after canonicalization (trivially satisfiable)
/// - If any literal references a variable outside `1..=num_vars`
pub struct CnfOracle {
    num_vars: usize,
    /// Canonicalized clauses: each inner Vec contains unique Literals,
    /// no clause contains both x and ¬x.
    pub(crate) clauses: Vec<Clause>,
}

impl CnfOracle {
    /// Build a CnfOracle from a CNF formula.
    ///
    /// # Arguments
    /// * `num_vars` — number of boolean variables (≥ 2, dense 1-indexed)
    /// * `clauses` — conjunction (AND) of disjunctions (OR) of literals
    pub fn new(num_vars: usize, clauses: &[Clause]) -> Self {
        assert!(num_vars >= 2, "num_vars must be >= 2, got {}", num_vars);
        assert!(!clauses.is_empty(), "clauses must not be empty");

        let mut canonical = Vec::new();
        for (ci, clause) in clauses.iter().enumerate() {
            assert!(
                !clause.is_empty(),
                "clause {} is empty (unsatisfiable by convention)",
                ci
            );
            for lit in clause {
                assert!(
                    lit.var() <= num_vars,
                    "clause {} references variable {} but num_vars={}",
                    ci,
                    lit.var(),
                    num_vars
                );
            }

            // Canonicalize: dedup literals, detect tautology
            let mut pos_vars = HashSet::new();
            let mut neg_vars = HashSet::new();
            let mut deduped = Vec::new();
            let mut seen = HashSet::new();
            let mut is_tautology = false;

            for lit in clause {
                if !seen.insert((lit.var(), lit.is_negated())) {
                    continue; // duplicate literal
                }
                if lit.is_negated() {
                    neg_vars.insert(lit.var());
                    if pos_vars.contains(&lit.var()) {
                        is_tautology = true;
                        break;
                    }
                } else {
                    pos_vars.insert(lit.var());
                    if neg_vars.contains(&lit.var()) {
                        is_tautology = true;
                        break;
                    }
                }
                deduped.push(*lit);
            }

            if !is_tautology {
                canonical.push(deduped);
            }
        }

        assert!(
            !canonical.is_empty(),
            "all clauses are tautological — formula is trivially satisfiable"
        );

        Self {
            num_vars,
            clauses: canonical,
        }
    }

    /// Number of canonicalized clauses.
    pub fn num_clauses(&self) -> usize {
        self.clauses.len()
    }
}

impl Oracle for CnfOracle {
    fn num_data_qubits(&self) -> usize {
        self.num_vars
    }

    fn num_ancillas(&self) -> usize {
        let c = self.clauses.len();
        let clause_ancillas = c;
        // MCX scratch per clause — reusable across sequential clauses
        let mcx_scratch = self
            .clauses
            .iter()
            .map(|cl| multi_cx::required_ancillas(cl.len()))
            .max()
            .unwrap_or(0);
        // MCZ scratch for final phase flip across all clause ancillas
        let final_mcz_scratch = multi_cz::required_ancillas(c);
        clause_ancillas + max(mcx_scratch, final_mcz_scratch)
    }

    fn num_solutions(&self) -> Option<NonZeroUsize> {
        None // unknown — that's the whole point
    }

    fn apply(&self, circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]) {
        // ===================================================================
        // What this circuit does
        // ===================================================================
        //
        // Grover's algorithm calls this oracle on a SUPERPOSITION of all
        // possible variable assignments simultaneously:
        //
        //   |ψ⟩ = Σ α_x |x⟩    where x ∈ {0, 1}^n
        //
        // Each basis state |x⟩ encodes one assignment: qubit 0 = variable 1,
        // qubit 1 = variable 2, etc. (LSB-first). For example, with 3 vars:
        //
        //   |101⟩ means x₁=1, x₂=0, x₃=1
        //
        // The oracle must evaluate the CNF formula f(x) on EVERY branch of
        // the superposition and flip the phase of satisfying assignments:
        //
        //   |x⟩ → (-1)^f(x) |x⟩
        //
        // ===================================================================
        // How the circuit evaluates the formula
        // ===================================================================
        //
        // Classical logic → reversible gates:
        //
        //   Logic op  │ Quantum gate(s)        │ Notes
        //   ──────────┼────────────────────────┼──────────────────────────
        //   NOT a     │ X(a)                   │ flips qubit in-place
        //   AND(a,b)  │ Toffoli(target, a, b)  │ target must start at |0⟩
        //   AND(n)    │ MCX(target, controls)  │ V-chain for n≥3 controls
        //   OR(a,b)   │ X+Toffoli+X+X          │ De Morgan: ¬(¬a ∧ ¬b)
        //   NOR(a,b)  │ X+Toffoli+X            │ AND of inverted inputs
        //   phase AND │ MCZ(qubits)            │ flips phase, not value
        //
        // The circuit mirrors the CNF structure directly:
        //   - One ancilla per clause → evaluates the OR (via De Morgan)
        //   - One final MCZ across all clause ancillas → evaluates the AND
        //
        //   CNF: (clause₁) ∧ (clause₂) ∧ ... ∧ (clauseₖ)
        //            ↓           ↓                 ↓
        //         ancilla₁   ancilla₂   ...    ancillaₖ   ← 1 if clause satisfied
        //            └───────────┼─────────────────┘
        //                     MCZ  ← phase flip if ALL ancillas = 1
        //
        // Each clause OR is computed using De Morgan's law:
        //   l₁ ∨ l₂ ∨ ... ∨ lₖ  =  ¬(¬l₁ ∧ ¬l₂ ∧ ... ∧ ¬lₖ)
        //
        // ===================================================================
        // Per-clause steps (example: clause (x₁ ∨ ¬x₂))
        // ===================================================================
        //
        //   1. X on un-negated vars → qubit=1 iff literal is false
        //   2. MCX → clause_ancilla = NOR(literals)
        //   3. Undo X gates (restore data qubits)
        //   4. X on clause_ancilla → invert NOR to OR
        //
        // ===================================================================
        // Structure: within_apply(compute, action)
        // ===================================================================
        //
        // Compute: evaluate all clauses into their ancillas (steps 1–4)
        // Action:  MCZ across clause ancillas (phase flip if satisfied)
        // Uncompute: automatic via inverse(compute) — within_apply handles this
        //
        // Ancilla layout:
        //   [clause₀, clause₁, ..., clause_{c-1}, scratch...]
        //   ├── clause results ──────────────────┤├─ reusable ─┤
        //
        // Scratch is shared: MCX scratch reuses across sequential clauses
        // (each MCX uncomputes its V-chain). The final MCZ reuses the
        // same pool (temporally disjoint from clause evaluation).

        let c = self.clauses.len();
        let clause_ancillas = &ancillas[..c];
        let scratch = &ancillas[c..];

        // --- Compute: evaluate each clause into its ancilla ---
        let mut compute = Circuit::new();
        for (i, clause) in self.clauses.iter().enumerate() {
            let controls: Vec<usize> = clause.iter().map(|lit| data_qubits[lit.qubit()]).collect();

            // X on un-negated variables
            for lit in clause {
                if !lit.is_negated() {
                    compute += PauliX::new(data_qubits[lit.qubit()]);
                }
            }

            // MCX → clause ancilla = NOR(literals)
            let mcx_scratch_needed = multi_cx::required_ancillas(controls.len());
            let mcx_ancillas = &scratch[..mcx_scratch_needed];
            compute += multi_cx::build_multi_cx(clause_ancillas[i], &controls, mcx_ancillas);

            // Undo X gates
            for lit in clause {
                if !lit.is_negated() {
                    compute += PauliX::new(data_qubits[lit.qubit()]);
                }
            }

            // X on clause ancilla — invert NOR to OR
            compute += PauliX::new(clause_ancillas[i]);
        }

        // --- Action: MCZ on all clause ancillas ---
        let mut action = Circuit::new();
        let mcz_scratch_needed = multi_cz::required_ancillas(c);
        let mcz_ancillas = &scratch[..mcz_scratch_needed];
        action += multi_cz::build_multi_cz(clause_ancillas, mcz_ancillas);

        // compute → action → inverse(compute)
        *circuit += transform::within_apply(&compute, &action)
            .expect("CNF compute circuit uses only supported gates");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grover::{try_search_with_oracle, GroverConfig, Oracle};
    use crate::runner::BitRegisters;
    use crate::sat::{evaluate_cnf, Literal};
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    fn test_runner(circuit: &Circuit, shots: usize) -> BitRegisters {
        let num_qubits = circuit.number_of_qubits();
        let backend = Backend::new(num_qubits, None);
        let mut combined: BitRegisters = HashMap::new();
        for _ in 0..shots {
            let (bits, _, _) = backend.run_circuit(circuit).unwrap();
            for (name, results) in bits {
                combined.entry(name).or_default().extend(results);
            }
        }
        combined
    }

    #[test]
    fn test_cnf_oracle_dedup() {
        let clauses = vec![vec![Literal::pos(1), Literal::pos(1), Literal::pos(2)]];
        let cnf = CnfOracle::new(2, &clauses);
        assert_eq!(cnf.clauses[0].len(), 2);
    }

    #[test]
    fn test_cnf_oracle_tautology_dropped() {
        let clauses = vec![
            vec![Literal::pos(1), Literal::neg(1)],
            vec![Literal::pos(2)],
        ];
        let cnf = CnfOracle::new(2, &clauses);
        assert_eq!(cnf.num_clauses(), 1);
    }

    #[test]
    #[should_panic(expected = "all clauses are tautological")]
    fn test_cnf_oracle_all_tautological() {
        CnfOracle::new(
            2,
            &[
                vec![Literal::pos(1), Literal::neg(1)],
                vec![Literal::pos(2), Literal::neg(2)],
            ],
        );
    }

    #[test]
    #[should_panic(expected = "num_vars must be >= 2")]
    fn test_cnf_oracle_panics_small_vars() {
        CnfOracle::new(1, &[vec![Literal::pos(1)]]);
    }

    #[test]
    #[should_panic(expected = "clauses must not be empty")]
    fn test_cnf_oracle_panics_no_clauses() {
        CnfOracle::new(2, &[]);
    }

    #[test]
    #[should_panic(expected = "clause 0 is empty")]
    fn test_cnf_oracle_panics_empty_clause() {
        CnfOracle::new(2, &[vec![]]);
    }

    #[test]
    #[should_panic(expected = "references variable 5 but num_vars=3")]
    fn test_cnf_oracle_panics_out_of_range() {
        CnfOracle::new(3, &[vec![Literal::pos(5)]]);
    }

    #[test]
    fn test_cnf_oracle_ancilla_budget() {
        let cnf = CnfOracle::new(
            3,
            &[
                vec![Literal::pos(1), Literal::pos(2)],
                vec![Literal::neg(1), Literal::pos(3)],
            ],
        );
        assert_eq!(cnf.num_ancillas(), 2);

        let cnf4 = CnfOracle::new(
            3,
            &[
                vec![Literal::pos(1)],
                vec![Literal::pos(2)],
                vec![Literal::pos(3)],
                vec![Literal::neg(1), Literal::pos(2)],
            ],
        );
        assert_eq!(cnf4.num_ancillas(), 6);
    }

    /// Single solution: (x₁) ∧ (¬x₁ ∨ x₂) → only x₁=1,x₂=1 (state 3).
    #[test]
    fn test_cnf_oracle_grover_single() {
        let clauses = vec![
            vec![Literal::pos(1)],
            vec![Literal::neg(1), Literal::pos(2)],
        ];
        let cnf = CnfOracle::new(2, &clauses);
        let config = GroverConfig {
            num_qubits: 2,
            num_iterations: Some(1),
            num_shots: 50,
        };
        let result = try_search_with_oracle(&config, &cnf, &test_runner).unwrap();
        assert_eq!(result.measured_state, 3);
        assert!(result.probability > 0.8);
    }

    /// Multi-solution: (x₁) ∧ (x₂ ∨ x₃) ∧ (¬x₂ ∨ x₃) → {5, 7}.
    #[test]
    fn test_cnf_oracle_grover_multi_solution() {
        let clauses = vec![
            vec![Literal::pos(1)],
            vec![Literal::pos(2), Literal::pos(3)],
            vec![Literal::neg(2), Literal::pos(3)],
        ];
        let cnf = CnfOracle::new(3, &clauses);
        let config = GroverConfig {
            num_qubits: 3,
            num_iterations: Some(1),
            num_shots: 100,
        };
        let result = try_search_with_oracle(&config, &cnf, &test_runner).unwrap();
        assert!(
            evaluate_cnf(&clauses, result.measured_state),
            "Found {} which is not a solution",
            result.measured_state
        );
        let sol_count = result.counts.get(&5).unwrap_or(&0) + result.counts.get(&7).unwrap_or(&0);
        assert!(sol_count as f64 / 100.0 > 0.8);
    }

    /// IterationsRequired when num_iterations is None and oracle has no num_solutions.
    #[test]
    fn test_cnf_oracle_needs_iterations() {
        let cnf = CnfOracle::new(2, &[vec![Literal::pos(1)]]);
        let config = GroverConfig {
            num_qubits: 2,
            num_iterations: None,
            num_shots: 10,
        };
        assert!(try_search_with_oracle(&config, &cnf, &test_runner).is_err());
    }
}
