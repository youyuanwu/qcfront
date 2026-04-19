//! Grover's search algorithm.
//!
//! Implements Grover's quantum search for finding a marked item in an
//! unstructured search space of N = 2^n items in O(√N) queries.
//!
//! The library is backend-agnostic: [`search`] and [`search_with_oracle`]
//! accept a closure that runs a circuit and returns measurement results.

use std::collections::{HashMap, HashSet};

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::multi_cz::build_multi_cz;

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

/// Oracle that marks solution states with a phase flip.
///
/// **Phase oracle contract:** The oracle flips the phase of solution states
/// and leaves all other states unchanged. All ancilla qubits are restored
/// to |0⟩ after each application.
/// Formally: O|x⟩|0⟩ = (-1)^f(x)|x⟩|0⟩ for all x.
pub struct GroverOracle {
    targets: Vec<usize>,
    num_qubits: usize,
}

impl GroverOracle {
    /// Mark a single target state.
    ///
    /// # Panics
    /// If `target >= 2^num_qubits`.
    pub fn single(num_qubits: usize, target: usize) -> Self {
        assert!(
            target < (1 << num_qubits),
            "target {} out of range for {} qubits (max {})",
            target,
            num_qubits,
            (1 << num_qubits) - 1
        );
        Self {
            targets: vec![target],
            num_qubits,
        }
    }

    /// Mark multiple target states.
    ///
    /// Deduplicates via `HashSet` (prevents phase-flip cancellation from duplicates).
    ///
    /// # Panics
    /// - If `targets` is empty
    /// - If any target >= 2^num_qubits
    pub fn multi(num_qubits: usize, targets: &[usize]) -> Self {
        assert!(!targets.is_empty(), "targets must not be empty");
        let max_val = 1 << num_qubits;
        for &t in targets {
            assert!(
                t < max_val,
                "target {} out of range for {} qubits (max {})",
                t,
                num_qubits,
                max_val - 1
            );
        }
        let unique: HashSet<usize> = targets.iter().copied().collect();
        Self {
            targets: unique.into_iter().collect(),
            num_qubits,
        }
    }

    /// Number of marked solutions. Used for optimal iteration count.
    pub fn num_solutions(&self) -> usize {
        self.targets.len()
    }

    /// Apply the oracle to a circuit.
    pub(crate) fn apply(&self, circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]) {
        for &target in &self.targets {
            apply_target_oracle(circuit, data_qubits, ancillas, target);
        }
    }
}

/// Result of a Grover search.
#[derive(Debug, Clone)]
pub struct GroverResult {
    /// Most-frequently measured basis state.
    pub measured_state: usize,
    /// Fraction of shots yielding `measured_state`.
    pub probability: f64,
    /// `Some(true/false)` when target is known via [`search`]; `None` for
    /// oracle-based search via [`search_with_oracle`].
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

/// Run Grover's search for a known target.
///
/// Convenience wrapper around [`search_with_oracle`] that creates a single-target
/// oracle and sets `success` in the result.
///
/// # Panics
/// - If `config.num_qubits < 2`
/// - If `target >= 2^config.num_qubits`
/// - If `config.num_shots < 1`
///
/// # Example
/// ```no_run
/// use algos::grover::{search, GroverConfig};
/// use roqoqo::backends::EvaluatingBackend;
/// use roqoqo_quest::Backend;
///
/// let config = GroverConfig { num_qubits: 3, num_shots: 100, ..Default::default() };
/// let result = search(&config, 5, |circuit, total_qubits| {
///     let backend = Backend::new(total_qubits, None);
///     let (bits, _, _) = backend.run_circuit(circuit).unwrap();
///     bits
/// });
/// assert_eq!(result.success, Some(true));
/// ```
pub fn search<F>(config: &GroverConfig, target: usize, run_circuit: F) -> GroverResult
where
    F: Fn(&Circuit, usize) -> HashMap<String, Vec<Vec<bool>>>,
{
    let oracle = GroverOracle::single(config.num_qubits, target);
    let mut result = search_with_oracle(config, &oracle, run_circuit);
    result.success = Some(result.measured_state == target);
    result
}

/// Run Grover's search with a custom oracle.
///
/// # Panics
/// - If `config.num_qubits < 2`
/// - If `config.num_shots < 1`
/// - If `config.num_qubits` doesn't match the oracle's `num_qubits`
pub fn search_with_oracle<F>(
    config: &GroverConfig,
    oracle: &GroverOracle,
    run_circuit: F,
) -> GroverResult
where
    F: Fn(&Circuit, usize) -> HashMap<String, Vec<Vec<bool>>>,
{
    let n = config.num_qubits;
    assert!(n >= 2, "num_qubits must be >= 2, got {}", n);
    assert_eq!(
        n, oracle.num_qubits,
        "config.num_qubits ({}) must match oracle num_qubits ({})",
        n, oracle.num_qubits
    );
    assert!(
        config.num_shots >= 1,
        "num_shots must be >= 1, got {}",
        config.num_shots
    );

    let iterations = config
        .num_iterations
        .unwrap_or_else(|| optimal_iterations(n, oracle.num_solutions()));

    let (circuit, total_qubits) = build_grover_circuit(n, oracle, iterations);

    // Collect measurement statistics across shots
    let mut counts: HashMap<usize, usize> = HashMap::new();

    for _ in 0..config.num_shots {
        let bit_registers = run_circuit(&circuit, total_qubits);
        let bits = match bit_registers.get("result") {
            Some(shots) if !shots.is_empty() => &shots[0],
            _ => continue,
        };

        // Convert bit vector to integer (LSB-first)
        let mut state: usize = 0;
        for (i, &bit) in bits.iter().enumerate().take(n) {
            if bit {
                state |= 1 << i;
            }
        }
        *counts.entry(state).or_insert(0) += 1;
    }

    let (&measured_state, &max_count) = counts
        .iter()
        .max_by_key(|(_, &count)| count)
        .unwrap_or((&0, &0));

    let probability = max_count as f64 / config.num_shots as f64;

    GroverResult {
        measured_state,
        probability,
        success: None,
        num_iterations: iterations,
        counts,
    }
}

/// Build a complete Grover search circuit.
///
/// Returns `(circuit, total_qubits)`.
pub(crate) fn build_grover_circuit(
    num_qubits: usize,
    oracle: &GroverOracle,
    num_iterations: usize,
) -> (Circuit, usize) {
    let n = num_qubits;
    let n_ancillas = if n >= 4 { n - 2 } else { 0 };
    let total_qubits = n + n_ancillas;

    let data_qubits: Vec<usize> = (0..n).collect();
    let ancillas: Vec<usize> = (n..n + n_ancillas).collect();

    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("result".to_string(), n, true);

    // 1. Initialize: H on all data qubits → equal superposition
    for &q in &data_qubits {
        circuit += Hadamard::new(q);
    }

    // 2. Repeat Grover iterations
    for _ in 0..num_iterations {
        oracle.apply(&mut circuit, &data_qubits, &ancillas);
        build_diffuser(&mut circuit, &data_qubits, &ancillas);
    }

    // 3. Measure data qubits
    for (i, &q) in data_qubits.iter().enumerate() {
        circuit += MeasureQubit::new(q, "result".to_string(), i);
    }

    (circuit, total_qubits)
}

/// Build the oracle sub-circuit that flips the phase of a single |target⟩.
fn apply_target_oracle(
    circuit: &mut Circuit,
    data_qubits: &[usize],
    ancillas: &[usize],
    target: usize,
) {
    // Apply X to qubits where target bit is 0
    for (i, &q) in data_qubits.iter().enumerate() {
        if (target >> i) & 1 == 0 {
            *circuit += PauliX::new(q);
        }
    }

    // Multi-controlled-Z: flips phase of |11…1⟩ (which is |target⟩ after X gates)
    *circuit += build_multi_cz(data_qubits, ancillas);

    // Undo X gates
    for (i, &q) in data_qubits.iter().enumerate() {
        if (target >> i) & 1 == 0 {
            *circuit += PauliX::new(q);
        }
    }
}

/// Build the diffuser sub-circuit (Grover diffusion operator).
///
/// Implements −(2|s⟩⟨s| − I) = I − 2|s⟩⟨s| via H-X-MCZ-X-H.
/// Equivalent to 2|s⟩⟨s| − I up to global phase (unobservable).
fn build_diffuser(circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]) {
    // H on all data qubits
    for &q in data_qubits {
        *circuit += Hadamard::new(q);
    }
    // X on all data qubits
    for &q in data_qubits {
        *circuit += PauliX::new(q);
    }

    // Multi-controlled-Z
    *circuit += build_multi_cz(data_qubits, ancillas);

    // X on all data qubits
    for &q in data_qubits {
        *circuit += PauliX::new(q);
    }
    // H on all data qubits
    for &q in data_qubits {
        *circuit += Hadamard::new(q);
    }
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
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;

    fn run_backend(circuit: &Circuit, total_qubits: usize) -> HashMap<String, Vec<Vec<bool>>> {
        let backend = Backend::new(total_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
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
            let result = search(&config, target, run_backend);
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
            let result = search(&config, target, run_backend);
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
        let result = search(&config, 2, run_backend);

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
        let result = search(&config, 6, run_backend);
        assert_eq!(result.num_iterations, 2);
        assert_eq!(result.success, Some(true));
    }

    /// search_with_oracle returns success=None.
    #[test]
    fn test_search_with_oracle_no_success() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 50,
            ..Default::default()
        };
        let oracle = GroverOracle::single(2, 3);
        let result = search_with_oracle(&config, &oracle, run_backend);
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
        let oracle = GroverOracle::multi(3, &[2, 5]);
        assert_eq!(oracle.num_solutions(), 2);

        let result = search_with_oracle(&config, &oracle, run_backend);
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
        let oracle = GroverOracle::multi(3, &[5, 5, 3, 5, 3]);
        assert_eq!(oracle.num_solutions(), 2); // only {3, 5}
    }

    /// is_match helper works.
    #[test]
    fn test_is_match() {
        let config = GroverConfig {
            num_qubits: 2,
            num_shots: 20,
            ..Default::default()
        };
        let result = search(&config, 1, run_backend);
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
        search(&config, 0, run_backend);
    }

    #[test]
    #[should_panic(expected = "target 8 out of range")]
    fn test_grover_panics_on_invalid_target() {
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 10,
            ..Default::default()
        };
        search(&config, 8, run_backend);
    }

    #[test]
    #[should_panic(expected = "num_shots must be >= 1")]
    fn test_grover_panics_on_zero_shots() {
        let config = GroverConfig {
            num_qubits: 3,
            num_shots: 0,
            ..Default::default()
        };
        search(&config, 3, run_backend);
    }

    #[test]
    #[should_panic(expected = "targets must not be empty")]
    fn test_multi_target_panics_on_empty() {
        GroverOracle::multi(3, &[]);
    }

    #[test]
    #[should_panic(expected = "target 8 out of range")]
    fn test_multi_target_panics_on_invalid() {
        GroverOracle::multi(3, &[1, 8]);
    }

    #[test]
    fn test_grover_default_config() {
        let config = GroverConfig::default();
        assert_eq!(config.num_qubits, 3);
        assert_eq!(config.num_iterations, None);
        assert_eq!(config.num_shots, 100);
    }
}
