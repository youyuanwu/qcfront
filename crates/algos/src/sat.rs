//! SAT oracle for Grover's algorithm.
//!
//! Builds a quantum oracle that marks satisfying assignments of a CNF formula
//! by flipping their phase. Currently uses classical brute-force to enumerate
//! solutions (for demonstration). A circuit-based oracle using De Morgan
//! decomposition with Toffoli gates is planned.

use crate::grover::GroverOracle;

/// A literal in a CNF clause. 1-indexed variables, sign indicates polarity.
///
/// - `Literal(1)` = x₁ (positive)
/// - `Literal(-2)` = ¬x₂ (negated)
/// - Variable 0 is invalid (0 == -0 has no polarity)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Literal(pub i32);

impl Literal {
    /// Variable index (1-based). Panics if literal is 0.
    pub fn var(&self) -> usize {
        assert!(self.0 != 0, "Literal 0 is invalid (no polarity)");
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
/// # Panics
/// - If `num_vars < 2`
/// - If `clauses` is empty
/// - If any clause is empty
/// - If any literal references variable 0 or variable > num_vars
///
/// # Returns
/// A [`GroverOracle`] configured for use with [`crate::grover::search_with_oracle`].
/// The caller must set `num_iterations` explicitly in `GroverConfig` since
/// the number of solutions is unknown.
pub fn sat_oracle(num_vars: usize, clauses: &[Clause]) -> SatOracle {
    assert!(num_vars >= 2, "num_vars must be >= 2, got {}", num_vars);
    assert!(!clauses.is_empty(), "clauses must not be empty");

    for (ci, clause) in clauses.iter().enumerate() {
        assert!(
            !clause.is_empty(),
            "clause {} is empty (unsatisfiable by convention)",
            ci
        );
        for lit in clause {
            assert!(
                lit.0 != 0,
                "clause {} contains literal 0 (invalid: no polarity)",
                ci
            );
            assert!(
                lit.var() <= num_vars,
                "clause {} references variable {} but num_vars={}",
                ci,
                lit.var(),
                num_vars
            );
        }
    }

    SatOracle {
        num_vars,
        clauses: clauses.to_vec(),
    }
}

/// A SAT oracle that can build the Grover oracle circuit.
pub struct SatOracle {
    num_vars: usize,
    clauses: Vec<Clause>,
}

impl SatOracle {
    /// Number of workspace qubits needed (clause ancillas + sat ancilla).
    pub fn workspace_qubits(&self) -> usize {
        self.clauses.len() + 1
    }

    /// Total qubits needed (data + MCZ ancillas + workspace).
    pub fn total_qubits(&self) -> usize {
        let n = self.num_vars;
        let mcz_ancillas = if n >= 4 { n - 2 } else { 0 };
        n + mcz_ancillas + self.workspace_qubits()
    }

    /// Convert to a [`GroverOracle`] for use with `search_with_oracle`.
    ///
    /// This pre-computes satisfying assignments by classical brute-force
    /// (feasible for small num_vars) to create a `GroverOracle::multi`.
    /// For a true quantum advantage, a circuit-based oracle would be needed,
    /// but for demonstration purposes this validates the SAT→Grover pipeline.
    pub fn to_grover_oracle(&self) -> GroverOracle {
        let n = self.num_vars;
        let mut solutions = Vec::new();

        // Classically enumerate all 2^n assignments and find satisfying ones
        for assignment in 0..(1usize << n) {
            if self.evaluate(assignment) {
                solutions.push(assignment);
            }
        }

        if solutions.is_empty() {
            panic!("CNF formula is unsatisfiable — no solutions exist for Grover to find");
        }

        GroverOracle::multi(n, &solutions)
    }

    /// Evaluate the CNF formula classically for a given assignment.
    pub fn evaluate(&self, assignment: usize) -> bool {
        self.clauses.iter().all(|clause| {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grover::{search_with_oracle, GroverConfig};
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo::Circuit;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    fn run_backend(circuit: &Circuit, total_qubits: usize) -> HashMap<String, Vec<Vec<bool>>> {
        let backend = Backend::new(total_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
    }

    /// Test classical evaluation of (x₁ OR x₂) AND (¬x₁ OR x₃).
    /// Bit ordering: x₁=bit0, x₂=bit1, x₃=bit2 (LSB-first).
    #[test]
    fn test_evaluate_2sat() {
        let clauses = vec![
            vec![Literal(1), Literal(2)],  // x₁ OR x₂
            vec![Literal(-1), Literal(3)], // ¬x₁ OR x₃
        ];
        let oracle = sat_oracle(3, &clauses);

        // x₁x₂x₃ (MSB) → assignment (LSB-first)
        // 000 → 0: (0∨0)=0 → ✗
        assert!(!oracle.evaluate(0b000));
        // 001 → 1: x₁=1, (1∨0)=1, (¬1∨0)=0 → ✗
        assert!(!oracle.evaluate(0b001));
        // 010 → 2: x₂=1, (0∨1)=1, (¬0∨0)=1 → ✓
        assert!(oracle.evaluate(0b010));
        // 011 → 3: x₁=1,x₂=1, (1∨1)=1, (¬1∨0)=0 → ✗
        assert!(!oracle.evaluate(0b011));
        // 100 → 4: x₃=1, (0∨0)=0 → ✗
        assert!(!oracle.evaluate(0b100));
        // 101 → 5: x₁=1,x₃=1, (1∨0)=1, (¬1∨1)=1 → ✓
        assert!(oracle.evaluate(0b101));
        // 110 → 6: x₂=1,x₃=1, (0∨1)=1, (¬0∨1)=1 → ✓
        assert!(oracle.evaluate(0b110));
        // 111 → 7: all 1, (1∨1)=1, (¬1∨1)=1 → ✓
        assert!(oracle.evaluate(0b111));
    }

    /// Test to_grover_oracle finds correct solutions.
    #[test]
    fn test_to_grover_oracle() {
        let clauses = vec![vec![Literal(1), Literal(2)], vec![Literal(-1), Literal(3)]];
        let sat = sat_oracle(3, &clauses);
        let grover_oracle = sat.to_grover_oracle();
        assert_eq!(grover_oracle.num_solutions(), 4);
    }

    /// Full pipeline: SAT → Grover oracle → search finds satisfying assignment.
    /// Uses (x₁) AND (x₂ OR x₃) — solutions: x₁=1 AND (x₂=1 OR x₃=1) → {3,5,7}, M=3.
    #[test]
    fn test_sat_grover_pipeline() {
        let clauses = vec![
            vec![Literal(1)],             // x₁
            vec![Literal(2), Literal(3)], // x₂ OR x₃
        ];
        let sat = sat_oracle(3, &clauses);
        let grover_oracle = sat.to_grover_oracle();

        // M=3, N=8: k = floor(pi/4 * sqrt(8/3)) = floor(1.28) = 1
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 100,
            ..Default::default()
        };
        let result = search_with_oracle(&config, &grover_oracle, run_backend);

        // Should find a satisfying assignment
        assert!(
            sat.evaluate(result.measured_state),
            "Grover found {} which does not satisfy the formula",
            result.measured_state
        );
    }

    /// Test with a different SAT instance: (x₁) AND (¬x₁ OR x₂).
    /// Solutions: 11 (decimal 3) only for x₁=1, x₂=1.
    /// Wait: x₁=1 satisfies clause 1. ¬x₁ OR x₂ = 0 OR x₂ = x₂. So x₂ must be 1.
    /// Solution: 11 only → M=1.
    #[test]
    fn test_sat_single_solution() {
        let clauses = vec![
            vec![Literal(1)],              // x₁
            vec![Literal(-1), Literal(2)], // ¬x₁ OR x₂
        ];
        let sat = sat_oracle(2, &clauses);
        let grover_oracle = sat.to_grover_oracle();
        assert_eq!(grover_oracle.num_solutions(), 1);

        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 50,
            ..Default::default()
        };
        let result = search_with_oracle(&config, &grover_oracle, run_backend);
        assert_eq!(result.measured_state, 3); // binary 11
    }

    #[test]
    #[should_panic(expected = "num_vars must be >= 2")]
    fn test_sat_panics_small_vars() {
        sat_oracle(1, &[vec![Literal(1)]]);
    }

    #[test]
    #[should_panic(expected = "clauses must not be empty")]
    fn test_sat_panics_no_clauses() {
        sat_oracle(2, &[]);
    }

    #[test]
    #[should_panic(expected = "clause 0 is empty")]
    fn test_sat_panics_empty_clause() {
        sat_oracle(2, &[vec![]]);
    }

    #[test]
    #[should_panic(expected = "contains literal 0")]
    fn test_sat_panics_literal_zero() {
        sat_oracle(2, &[vec![Literal(0)]]);
    }

    #[test]
    #[should_panic(expected = "references variable 5 but num_vars=3")]
    fn test_sat_panics_out_of_range() {
        sat_oracle(3, &[vec![Literal(5)]]);
    }
}
