//! Grover's search algorithm.
//!
//! Implements Grover's quantum search for finding a marked item in an
//! unstructured search space of N = 2^n items in O(√N) queries.
//!
//! The library is backend-agnostic: [`search`] and [`try_search_with_oracle`]
//! accept any [`QuantumRunner`](crate::runner::QuantumRunner) implementation.
//!
//! ## Oracle Trait
//!
//! The [`Oracle`] trait abstracts over different marking strategies.
//! [`IndexOracle`] marks states by index (simple, solution set known).
//! [`CnfOracle`] evaluates CNF formulas reversibly (circuit-based, no
//! classical pre-solving). [`SubsetSumOracle`] evaluates subset-sum
//! instances using a controlled-adder circuit.

mod index;
mod sat;
mod subset_sum;

#[allow(deprecated)]
pub use index::{GroverOracle, IndexOracle};
pub use sat::CnfOracle;
pub use subset_sum::{verify_subset_sum, SubsetSumOracle};

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroUsize;

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::multi_cz::{build_multi_cz, required_ancillas};
use crate::circuits::transform;
use crate::qubit::{QubitAllocator, QubitRange};
use crate::runner::QuantumRunner;

// ---------------------------------------------------------------------------
// Oracle trait
// ---------------------------------------------------------------------------

/// A phase oracle for Grover's algorithm.
///
/// **Phase oracle invariant**: Implementors must satisfy
///   `O|x⟩|0⟩ = (-1)^f(x)|x⟩|0⟩` for all x.
///
/// Concretely:
/// - Flip the phase of solution states, leave others unchanged
/// - Restore all ancilla qubits to |0⟩ before returning
/// - Do NOT apply non-diagonal operations to data qubits
///   (amplitudes must not change, only phases)
///
/// A bit-flip oracle (writes f(x) into an ancilla instead of
/// flipping phase) will silently produce wrong results. Use the
/// compute → MCZ → uncompute pattern, not compute → leave.
pub trait Oracle {
    /// Number of data qubits this oracle operates on (search space = 2^n).
    fn num_data_qubits(&self) -> usize;

    /// Total ancilla qubits this oracle needs (MCZ/MCX decomposition
    /// scratch, clause ancillas, etc.). The driver allocates these
    /// **disjoint** from the diffuser's own MCZ ancillas.
    fn num_ancillas(&self) -> usize;

    /// Number of solutions, if known. Enables auto-computing the
    /// optimal iteration count. Return `None` when unknown (caller
    /// must set `GroverConfig::num_iterations` explicitly).
    fn num_solutions(&self) -> Option<NonZeroUsize>;

    /// Emit gates that flip the phase of solution states.
    ///
    /// Called once per Grover iteration on a superposition of all possible
    /// inputs. The circuit must evaluate `f(x)` for every basis state
    /// simultaneously and flip the phase of those where `f(x) = 1`.
    ///
    /// # Arguments
    ///
    /// * `data_qubits` — the search register. Each basis state `|x⟩` of
    ///   these n qubits encodes one candidate input to the search problem.
    ///   Bit mapping is LSB-first: `data_qubits[0]` = variable 1,
    ///   `data_qubits[1]` = variable 2, etc. For example, with 3 data
    ///   qubits, the state `|101⟩` represents x₁=1, x₂=0, x₃=1.
    ///   The oracle reads these qubits (as controls) but must not change
    ///   their computational-basis values — only phases may change.
    ///
    /// * `ancillas` — scratch qubits owned exclusively by this oracle.
    ///   Guaranteed to be `|0⟩` at the start of each Grover iteration.
    ///   The oracle may write intermediate results here (e.g., clause
    ///   evaluation results) but **must restore them to `|0⟩`** before
    ///   returning. Leaving ancillas entangled with data qubits corrupts
    ///   the diffuser step that follows.
    fn apply(&self, circuit: &mut Circuit, data_qubits: &QubitRange, ancillas: &QubitRange);
}

// ---------------------------------------------------------------------------
// GroverError
// ---------------------------------------------------------------------------

/// Errors from [`try_search_with_oracle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroverError {
    /// `num_iterations` is required when the oracle's `num_solutions()`
    /// returns `None`.
    IterationsRequired,
}

impl fmt::Display for GroverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IterationsRequired => write!(
                f,
                "num_iterations must be set in GroverConfig when the oracle's \
                 num_solutions() returns None"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for Grover's search.
pub struct GroverConfig {
    /// Number of data qubits (search space = 2^n). Must be ≥ 2.
    pub num_qubits: usize,
    /// Number of Grover iterations. `None` auto-computes optimal count
    /// using the oracle's `num_solutions()`.
    pub num_iterations: Option<usize>,
    /// Number of measurement shots. Must be ≥ 1.
    pub num_shots: usize,
}

impl Default for GroverConfig {
    fn default() -> Self {
        Self {
            num_qubits: 3,
            num_iterations: None,
            num_shots: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Result of a Grover search.
#[derive(Debug, Clone)]
pub struct GroverResult {
    /// Most-frequently measured basis state.
    pub measured_state: usize,
    /// Fraction of shots yielding `measured_state`.
    pub probability: f64,
    /// `Some(true/false)` when target is known via [`search`]; `None` for
    /// oracle-based search via [`try_search_with_oracle`].
    pub success: Option<bool>,
    /// Number of Grover iterations used.
    pub num_iterations: usize,
    /// All measurement outcomes: state → count.
    pub counts: HashMap<usize, usize>,
}

impl GroverResult {
    /// Check if the measured state matches a specific target.
    pub fn is_match(&self, target: usize) -> bool {
        self.measured_state == target
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run Grover's search for a known target.
///
/// Convenience wrapper around [`try_search_with_oracle`] that creates a
/// single-target oracle and sets `success` in the result.
///
/// # Panics
/// - If `config.num_qubits < 2`
/// - If `target >= 2^config.num_qubits`
/// - If `config.num_shots < 1`
pub fn search<R: QuantumRunner + ?Sized>(
    config: &GroverConfig,
    target: usize,
    runner: &R,
) -> GroverResult {
    let oracle = IndexOracle::single(config.num_qubits, target);
    let mut result = try_search_with_oracle(config, &oracle, runner)
        .expect("IndexOracle always provides num_solutions");
    result.success = Some(result.measured_state == target);
    result
}

/// Run Grover's search with a custom oracle (returns `Result`).
///
/// # Errors
/// Returns [`GroverError::IterationsRequired`] when `config.num_iterations`
/// is `None` and the oracle's `num_solutions()` returns `None`.
///
/// # Panics
/// - If `config.num_qubits < 2`
/// - If `config.num_shots < 1`
/// - If `config.num_qubits != oracle.num_data_qubits()`
pub fn try_search_with_oracle<O: Oracle + ?Sized, R: QuantumRunner + ?Sized>(
    config: &GroverConfig,
    oracle: &O,
    runner: &R,
) -> Result<GroverResult, GroverError> {
    let n = config.num_qubits;
    assert!(n >= 2, "num_qubits must be >= 2, got {}", n);
    assert_eq!(
        n,
        oracle.num_data_qubits(),
        "config.num_qubits ({}) must match oracle.num_data_qubits() ({})",
        n,
        oracle.num_data_qubits()
    );
    assert!(
        config.num_shots >= 1,
        "num_shots must be >= 1, got {}",
        config.num_shots
    );

    let iterations = match (config.num_iterations, oracle.num_solutions()) {
        (Some(k), _) => k,
        (None, Some(m)) => optimal_iterations(n, m.get()),
        (None, None) => return Err(GroverError::IterationsRequired),
    };

    let circuit = build_grover_circuit(n, oracle, iterations);

    let bit_registers = runner.run(&circuit, config.num_shots);
    let counts = crate::runner::Counts::from_register(&bit_registers, "result", n);

    let (measured_state, _) = counts.most_frequent();
    let probability = counts.probability(measured_state);

    Ok(GroverResult {
        measured_state,
        probability,
        success: None,
        num_iterations: iterations,
        counts: counts.as_map().clone(),
    })
}

/// Run Grover's search with a custom oracle.
///
/// # Panics
/// - If `config.num_qubits < 2`
/// - If `config.num_shots < 1`
/// - If `config.num_qubits` doesn't match the oracle's `num_data_qubits()`
/// - If `config.num_iterations` is `None` and the oracle can't provide
///   `num_solutions()`
#[deprecated(note = "use try_search_with_oracle which returns Result")]
pub fn search_with_oracle<R: QuantumRunner + ?Sized>(
    config: &GroverConfig,
    oracle: &IndexOracle,
    runner: &R,
) -> GroverResult {
    try_search_with_oracle(config, oracle, runner)
        .expect("search_with_oracle requires oracle with known num_solutions")
}

// ---------------------------------------------------------------------------
// Circuit construction (internal)
// ---------------------------------------------------------------------------

/// Build a complete Grover search circuit with disjoint qubit regions.
///
/// Layout: `[data (n)] [diffuser MCZ scratch (d)] [oracle scratch (a)]`
fn build_grover_circuit<O: Oracle + ?Sized>(
    num_qubits: usize,
    oracle: &O,
    num_iterations: usize,
) -> Circuit {
    let n = num_qubits;
    let d = required_ancillas(n);
    let a = oracle.num_ancillas();

    let mut alloc = QubitAllocator::new();
    let data = alloc.allocate("data", n);
    let diffuser_ancillas = alloc.allocate("diffuser", d);
    let oracle_ancillas = alloc.allocate("oracle", a);

    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("result".to_string(), n, true);

    // 1. Initialize: H on all data qubits → equal superposition
    for q in data.iter() {
        circuit += Hadamard::new(q.index());
    }

    // 2. Repeat Grover iterations
    for _ in 0..num_iterations {
        oracle.apply(&mut circuit, &data, &oracle_ancillas);
        build_diffuser(&mut circuit, &data, &diffuser_ancillas);
    }

    // 3. Measure data qubits
    for (i, q) in data.iter().enumerate() {
        circuit += MeasureQubit::new(q.index(), "result".to_string(), i);
    }

    circuit
}

/// Build the diffuser sub-circuit (Grover diffusion operator).
///
/// Implements −(2|s⟩⟨s| − I) = I − 2|s⟩⟨s| via H-X-MCZ-X-H.
/// Equivalent to 2|s⟩⟨s| − I up to global phase (unobservable).
fn build_diffuser(circuit: &mut Circuit, data_qubits: &QubitRange, ancillas: &QubitRange) {
    // Compute: H then X on all data qubits
    let mut compute = Circuit::new();
    for q in data_qubits.iter() {
        compute += Hadamard::new(q.index());
    }
    for q in data_qubits.iter() {
        compute += PauliX::new(q.index());
    }

    // Action: MCZ
    let mut action = Circuit::new();
    action += build_multi_cz(data_qubits, ancillas);

    // H·X → MCZ → X·H (automatic via within_apply)
    *circuit += transform::within_apply(&compute, &action)
        .expect("diffuser compute uses only H and X gates");
}

/// Compute optimal number of Grover iterations.
fn optimal_iterations(num_qubits: usize, num_solutions: usize) -> usize {
    assert!(num_solutions >= 1, "num_solutions must be >= 1");
    let n = 1usize << num_qubits; // N = 2^n
    assert!(
        num_solutions <= n,
        "num_solutions ({}) must be <= N ({})",
        num_solutions,
        n
    );
    let ratio = n as f64 / num_solutions as f64;
    let k = (std::f64::consts::FRAC_PI_4 * ratio.sqrt()).floor() as usize;
    if num_solutions == 1 {
        k.max(1)
    } else {
        k // k=0 is valid when M > N/2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::BitRegisters;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;

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
    fn test_optimal_iterations() {
        assert_eq!(optimal_iterations(2, 1), 1);
        assert_eq!(optimal_iterations(3, 1), 2);
        assert_eq!(optimal_iterations(4, 1), 3);
    }

    #[test]
    fn test_optimal_iterations_multi() {
        // n=3, M=4: k = floor(pi/4 * sqrt(8/4)) = floor(1.11) = 1
        assert_eq!(optimal_iterations(3, 4), 1);
        // n=3, M=7: k = floor(pi/4 * sqrt(8/7)) = floor(0.84) = 0
        assert_eq!(optimal_iterations(3, 7), 0);
    }

    /// 2-qubit Grover: search space N=4, test all targets.
    #[test]
    fn test_grover_2_qubits() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 50,
            ..Default::default()
        };

        for target in 0..4 {
            let result = search(&config, target, &test_runner);
            assert_eq!(
                result.success,
                Some(true),
                "2-qubit Grover failed to find target {}. Got {} (p={:.2})",
                target,
                result.measured_state,
                result.probability
            );
            assert!(
                result.probability > 0.8,
                "2-qubit Grover target {} probability too low: {:.2}",
                target,
                result.probability
            );
        }
    }

    /// 3-qubit Grover: search space N=8, test several targets.
    #[test]
    fn test_grover_3_qubits() {
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 100,
            ..Default::default()
        };

        for target in [0, 3, 5, 7] {
            let result = search(&config, target, &test_runner);
            assert_eq!(
                result.success,
                Some(true),
                "3-qubit Grover failed to find target {}. Got {} (p={:.2})",
                target,
                result.measured_state,
                result.probability
            );
        }
    }

    /// Verify the result struct fields are populated correctly.
    #[test]
    fn test_grover_result_fields() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 20,
            ..Default::default()
        };
        let result = search(&config, 2, &test_runner);

        assert_eq!(result.num_iterations, 1);
        assert!(!result.counts.is_empty());
        let total_counts: usize = result.counts.values().sum();
        assert_eq!(total_counts, 20);
        assert!(result.success.is_some());
    }

    /// Test with explicit iteration count.
    #[test]
    fn test_grover_explicit_iterations() {
        let config = GroverConfig {
            num_qubits: 3,
            num_iterations: Some(2),
            num_shots: 50,
        };
        let result = search(&config, 6, &test_runner);
        assert_eq!(result.num_iterations, 2);
        assert_eq!(result.success, Some(true));
    }

    /// try_search_with_oracle returns success=None.
    #[test]
    fn test_search_with_oracle_no_success() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 50,
            ..Default::default()
        };
        let oracle = IndexOracle::single(2, 3);
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert!(result.success.is_none());
        assert!(result.is_match(3));
    }

    /// Multi-target oracle: search for 2 out of 8 states (n=3).
    #[test]
    fn test_multi_target_oracle() {
        // M=2, N=8: θ=π/6, k=1 → P=sin²(π/2)=100%
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 100,
            ..Default::default()
        };
        let oracle = IndexOracle::multi(3, &[2, 5]);
        assert_eq!(oracle.num_solutions().unwrap().get(), 2);

        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert!(
            result.measured_state == 2 || result.measured_state == 5,
            "Expected target 2 or 5, got {}",
            result.measured_state
        );
        // Total probability across both targets should be near 100%
        let target_count =
            result.counts.get(&2).unwrap_or(&0) + result.counts.get(&5).unwrap_or(&0);
        let target_prob = target_count as f64 / 100.0;
        assert!(
            target_prob > 0.9,
            "Combined target probability too low: {:.2}",
            target_prob
        );
    }

    /// Multi-target oracle deduplicates.
    #[test]
    fn test_multi_target_dedup() {
        let oracle = IndexOracle::multi(3, &[5, 5, 3, 5, 3]);
        assert_eq!(oracle.num_solutions().unwrap().get(), 2); // only {3, 5}
    }

    /// is_match helper works.
    #[test]
    fn test_is_match() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 20,
            ..Default::default()
        };
        let result = search(&config, 1, &test_runner);
        assert!(result.is_match(1));
        assert!(!result.is_match(2));
    }

    #[test]
    #[should_panic(expected = "num_qubits must be >= 2")]
    fn test_grover_panics_on_1_qubit() {
        let config = GroverConfig {
            num_qubits: 1,
            num_shots: 10,
            ..Default::default()
        };
        search(&config, 0, &test_runner);
    }

    #[test]
    #[should_panic(expected = "target 8 out of range")]
    fn test_grover_panics_on_invalid_target() {
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 10,
            ..Default::default()
        };
        search(&config, 8, &test_runner);
    }

    #[test]
    #[should_panic(expected = "num_shots must be >= 1")]
    fn test_grover_panics_on_zero_shots() {
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 0,
            ..Default::default()
        };
        search(&config, 3, &test_runner);
    }

    #[test]
    #[should_panic(expected = "targets must not be empty")]
    fn test_multi_target_panics_on_empty() {
        IndexOracle::multi(3, &[]);
    }

    #[test]
    #[should_panic(expected = "target 8 out of range")]
    fn test_multi_target_panics_on_invalid() {
        IndexOracle::multi(3, &[1, 8]);
    }

    #[test]
    fn test_grover_default_config() {
        let config = GroverConfig::default();
        assert_eq!(config.num_qubits, 3);
        assert_eq!(config.num_iterations, None);
        assert_eq!(config.num_shots, 100);
    }

    /// IterationsRequired error when oracle has no num_solutions.
    #[test]
    fn test_iterations_required_error() {
        struct UnknownOracle;
        impl Oracle for UnknownOracle {
            fn num_data_qubits(&self) -> usize {
                2
            }
            fn num_ancillas(&self) -> usize {
                0
            }
            fn num_solutions(&self) -> Option<NonZeroUsize> {
                None
            }
            fn apply(&self, _: &mut Circuit, _: &QubitRange, _: &QubitRange) {}
        }
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 10,
            num_iterations: None,
        };
        let result = try_search_with_oracle(&config, &UnknownOracle, &test_runner);
        assert!(matches!(result, Err(GroverError::IterationsRequired)));
    }

    /// Oracle trait works with explicit iterations even when num_solutions is None.
    #[test]
    fn test_oracle_trait_with_explicit_iterations() {
        struct UnknownOracle;
        impl Oracle for UnknownOracle {
            fn num_data_qubits(&self) -> usize {
                2
            }
            fn num_ancillas(&self) -> usize {
                0
            }
            fn num_solutions(&self) -> Option<NonZeroUsize> {
                None
            }
            fn apply(&self, circuit: &mut Circuit, data_qubits: &QubitRange, _: &QubitRange) {
                // Mark state |3⟩ = |11⟩ — just MCZ on both qubits
                *circuit += ControlledPauliZ::new(
                    data_qubits.qubit(0).index(),
                    data_qubits.qubit(1).index(),
                );
            }
        }
        let config = GroverConfig {
            num_qubits: 2,
            num_iterations: Some(1),
            num_shots: 50,
        };
        let result = try_search_with_oracle(&config, &UnknownOracle, &test_runner).unwrap();
        assert_eq!(result.measured_state, 3);
    }
}
