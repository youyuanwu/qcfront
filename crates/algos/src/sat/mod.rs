//! SAT types and classical evaluation.
//!
//! Provides [`Literal`], [`Clause`], and [`evaluate_cnf`] for defining
//! and classically verifying CNF formulas. The circuit-based SAT oracle
//! lives in [`crate::grover::CnfOracle`].

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

/// Evaluate a CNF formula classically for a given bit assignment.
///
/// Returns `true` if `assignment` satisfies every clause. Each clause
/// is an OR of [`Literal`]s; the formula is the AND of all clauses.
///
/// Bit ordering is LSB-first: bit 0 → variable 1, bit 1 → variable 2, etc.
/// This matches the qubit ordering used by [`CnfOracle`] and Grover's
/// measurement output.
///
/// # Example
/// ```
/// use algos::sat::{evaluate_cnf, Literal};
///
/// // (x₁) ∧ (¬x₁ ∨ x₂)  — only x₁=1,x₂=1 satisfies
/// let clauses = vec![
///     vec![Literal::pos(1)],
///     vec![Literal::neg(1), Literal::pos(2)],
/// ];
/// assert!(!evaluate_cnf(&clauses, 0b00));  // x₁=0,x₂=0
/// assert!(!evaluate_cnf(&clauses, 0b01));  // x₁=1,x₂=0
/// assert!(evaluate_cnf(&clauses, 0b11));   // x₁=1,x₂=1
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
