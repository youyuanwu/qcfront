//! SAT oracle for Grover's algorithm.
//!
//! [`CnfOracle`] evaluates a CNF formula reversibly on quantum inputs using
//! De Morgan decomposition. It implements [`Oracle`] directly — no classical
//! pre-solving, providing genuine quantum advantage for Grover's search.
//!
//! [`evaluate_cnf`] is a free function for classical verification of results.

use std::cmp::max;
use std::collections::HashSet;
use std::num::NonZeroUsize;

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::multi_cx;
use crate::circuits::multi_cz;
use crate::grover::Oracle;

/// A literal in a CNF clause.
///
/// Represents a boolean variable or its negation. Use [`Literal::pos`]
/// and [`Literal::neg`] to construct. Variables are 1-indexed.
///
/// # Examples
/// ```
/// use algos::sat::Literal;
/// let x1 = Literal::pos(1);       // x₁
/// let not_x2 = Literal::neg(2);   // ¬x₂
/// assert!(!x1.is_negated());
/// assert!(not_x2.is_negated());
/// assert_eq!(not_x2.var(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Literal(i32);

impl Literal {
    /// Positive literal (variable appears un-negated).
    ///
    /// # Panics
    /// If `var` is 0.
    pub fn pos(var: usize) -> Self {
        assert!(var > 0, "variable must be >= 1, got 0");
        Self(var as i32)
    }

    /// Negative literal (variable is negated).
    ///
    /// # Panics
    /// If `var` is 0.
    pub fn neg(var: usize) -> Self {
        assert!(var > 0, "variable must be >= 1, got 0");
        Self(-(var as i32))
    }

    /// Construct from DIMACS-style signed integer.
    /// Positive = un-negated, negative = negated.
    ///
    /// # Panics
    /// If `val` is 0.
    pub fn from_dimacs(val: i32) -> Self {
        assert!(val != 0, "literal 0 is invalid (no polarity)");
        Self(val)
    }

    /// Variable index (1-based).
    pub fn var(&self) -> usize {
        self.0.unsigned_abs() as usize
    }

    /// Whether the literal is negated.
    pub fn is_negated(&self) -> bool {
        self.0 < 0
    }

    /// Qubit index for this variable (0-based).
    pub fn qubit(&self) -> usize {
        self.var() - 1
    }
}

/// A CNF clause: disjunction (OR) of literals.
pub type Clause = Vec<Literal>;

/// Build a Grover oracle for a CNF SAT formula.
///
/// The oracle flips the phase of computational basis states that satisfy
/// all clauses. It uses workspace qubits for clause evaluation and
/// uncomputes them after the phase flip.
///
/// # Arguments
/// * `num_vars` — number of boolean variables (≥ 2, mapped to data qubits)
/// * `clauses` — CNF formula: conjunction (AND) of clauses
/// * `num_iterations` — number of Grover iterations (required since M is unknown)
///
/// Evaluate a CNF formula classically for a given bit assignment.
///
/// Each clause is a disjunction (OR) of literals; the formula is the
/// conjunction (AND) of all clauses. Assignment bits are LSB-first:
/// bit 0 = variable 1, bit 1 = variable 2, etc.
///
/// Useful for verifying quantum search results against the ground truth.
pub fn evaluate_cnf(clauses: &[Clause], assignment: usize) -> bool {
    clauses.iter().all(|clause| {
        clause.iter().any(|lit| {
            let bit = (assignment >> lit.qubit()) & 1 == 1;
            if lit.is_negated() {
                !bit
            } else {
                bit
            }
        })
    })
}

// ---------------------------------------------------------------------------
// CnfOracle — circuit-based SAT oracle (implements Oracle trait)
// ---------------------------------------------------------------------------

/// Circuit-based SAT oracle using reversible De Morgan decomposition.
///
/// Unlike [`SatOracle::to_grover_oracle`], this does NOT classically
/// enumerate solutions. It builds a quantum circuit that evaluates the
/// CNF formula on a superposition of all assignments, providing genuine
/// quantum advantage for Grover's search.
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
    clauses: Vec<Vec<Literal>>,
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

    /// Evaluate the CNF formula classically for a given assignment.
    pub fn evaluate(&self, assignment: usize) -> bool {
        evaluate_cnf(&self.clauses, assignment)
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
        let c = self.clauses.len();
        // Ancilla layout: [clause_0, clause_1, ..., clause_{c-1}, scratch...]
        let clause_ancillas = &ancillas[..c];
        let scratch = &ancillas[c..];

        // --- Compute: evaluate each clause into its ancilla ---
        for (i, clause) in self.clauses.iter().enumerate() {
            let controls: Vec<usize> = clause.iter().map(|lit| data_qubits[lit.qubit()]).collect();

            // Step 1: X on UN-NEGATED variables (De Morgan: detect all-false)
            for lit in clause {
                if !lit.is_negated() {
                    *circuit += PauliX::new(data_qubits[lit.qubit()]);
                }
            }

            // Step 2: MCX → clause ancilla (computes NOR of literals)
            let mcx_scratch_needed = multi_cx::required_ancillas(controls.len());
            let mcx_ancillas = &scratch[..mcx_scratch_needed];
            *circuit += multi_cx::build_multi_cx(clause_ancillas[i], &controls, mcx_ancillas);

            // Step 3: Undo X gates
            for lit in clause {
                if !lit.is_negated() {
                    *circuit += PauliX::new(data_qubits[lit.qubit()]);
                }
            }

            // Step 4: X on clause ancilla → invert: ancilla=1 when clause TRUE
            *circuit += PauliX::new(clause_ancillas[i]);
        }

        // --- Phase flip: MCZ on all clause ancillas ---
        let mcz_scratch_needed = multi_cz::required_ancillas(c);
        let mcz_ancillas = &scratch[..mcz_scratch_needed];
        *circuit += multi_cz::build_multi_cz(clause_ancillas, mcz_ancillas);

        // --- Uncompute: reverse clause evaluation (in reverse order) ---
        for (i, clause) in self.clauses.iter().enumerate().rev() {
            let controls: Vec<usize> = clause.iter().map(|lit| data_qubits[lit.qubit()]).collect();

            // Undo step 4
            *circuit += PauliX::new(clause_ancillas[i]);

            // Undo step 1 (apply X before MCX)
            for lit in clause {
                if !lit.is_negated() {
                    *circuit += PauliX::new(data_qubits[lit.qubit()]);
                }
            }

            // Undo step 2 (MCX again to uncompute)
            let mcx_scratch_needed = multi_cx::required_ancillas(controls.len());
            let mcx_ancillas = &scratch[..mcx_scratch_needed];
            *circuit += multi_cx::build_multi_cx(clause_ancillas[i], &controls, mcx_ancillas);

            // Undo step 3 (undo X gates)
            for lit in clause {
                if !lit.is_negated() {
                    *circuit += PauliX::new(data_qubits[lit.qubit()]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grover::{try_search_with_oracle, GroverConfig, Oracle};
    use crate::runner::BitRegisters;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo::Circuit;
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

    // -----------------------------------------------------------------------
    // evaluate_cnf tests
    // -----------------------------------------------------------------------

    /// Test classical evaluation of (x₁ OR x₂) AND (¬x₁ OR x₃).
    #[test]
    fn test_evaluate_cnf_2sat() {
        let clauses = vec![
            vec![Literal::pos(1), Literal::pos(2)],
            vec![Literal::neg(1), Literal::pos(3)],
        ];
        assert!(!evaluate_cnf(&clauses, 0b000));
        assert!(!evaluate_cnf(&clauses, 0b001));
        assert!(evaluate_cnf(&clauses, 0b010));
        assert!(!evaluate_cnf(&clauses, 0b011));
        assert!(!evaluate_cnf(&clauses, 0b100));
        assert!(evaluate_cnf(&clauses, 0b101));
        assert!(evaluate_cnf(&clauses, 0b110));
        assert!(evaluate_cnf(&clauses, 0b111));
    }

    #[test]
    #[should_panic(expected = "literal 0 is invalid")]
    fn test_literal_panics_zero() {
        Literal::from_dimacs(0);
    }

    // -----------------------------------------------------------------------
    // CnfOracle construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cnf_oracle_evaluate_matches_free_fn() {
        let clauses = vec![
            vec![Literal::pos(1), Literal::pos(2)],
            vec![Literal::neg(1), Literal::pos(3)],
        ];
        let cnf = CnfOracle::new(3, &clauses);
        for i in 0..8 {
            assert_eq!(cnf.evaluate(i), evaluate_cnf(&clauses, i));
        }
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

    // -----------------------------------------------------------------------
    // CnfOracle ancilla budget
    // -----------------------------------------------------------------------

    #[test]
    fn test_cnf_oracle_ancilla_budget() {
        // 2 clauses of 2 literals: clause_anc=2, mcx=0, mcz=0
        let cnf = CnfOracle::new(
            3,
            &[
                vec![Literal::pos(1), Literal::pos(2)],
                vec![Literal::neg(1), Literal::pos(3)],
            ],
        );
        assert_eq!(cnf.num_ancillas(), 2);

        // 4 clauses: clause_anc=4, mcx=0, mcz=2
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

    // -----------------------------------------------------------------------
    // CnfOracle + Grover integration
    // -----------------------------------------------------------------------

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
            cnf.evaluate(result.measured_state),
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
