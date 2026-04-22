//! Shor's factoring algorithm.
//!
//! Orchestrates quantum period finding with classical pre/post-processing
//! to factor semiprimes.
//!
//! The library is backend-agnostic: the top-level [`factor`] function accepts
//! any [`QuantumRunner`](crate::runner::QuantumRunner) implementation.
//!
//! Currently supports N=15 only (hardcoded modular multiplication).

use num_integer::Integer;
use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::modmul_15::controlled_modmul_15;
use crate::math::{convergents, mod_pow, random_coprime};
use crate::qpe::{bits_to_int_lsb, bits_to_int_msb, build_qpe_circuit};
use crate::qubit::QubitAllocator;
use crate::runner::QuantumRunner;

/// Configuration for Shor's factoring algorithm.
pub struct ShorConfig {
    /// Maximum number of coprime attempts before giving up.
    pub max_attempts: usize,
    /// Maximum number of circuit shots per coprime attempt.
    pub shots_per_attempt: usize,
}

impl Default for ShorConfig {
    fn default() -> Self {
        Self {
            max_attempts: 20,
            shots_per_attempt: 10,
        }
    }
}

/// Result of a single Shor attempt (one coprime choice).
#[derive(Debug, Clone)]
pub struct ShorAttempt {
    pub a: u64,
    pub order: Option<u64>,
    pub factors: Option<(u64, u64)>,
    pub shots_used: usize,
}

/// Factor `n` using Shor's algorithm.
///
/// # Arguments
/// * `n` — the number to factor (currently only 15 supported)
/// * `config` — algorithm configuration
/// * `runner` — quantum circuit runner (simulator or hardware)
///
/// # Returns
/// `Some((p, q))` with `p ≤ q` and `p * q == n`, or `None` if all attempts failed.
///
/// # Example
/// ```no_run
/// use algos::shor::{factor, ShorConfig};
/// use algos::runner::QuantumRunner;
///
/// # fn example(runner: &impl QuantumRunner) {
/// let result = factor(15, &ShorConfig::default(), runner);
/// assert_eq!(result, Some((3, 5)));
/// # }
/// ```
pub fn factor<R: QuantumRunner + ?Sized>(
    n: u64,
    config: &ShorConfig,
    runner: &R,
) -> Option<(u64, u64)> {
    factor_verbose(n, config, runner, |_| {})
}

/// Factor `n` with detailed per-attempt reporting via callback.
///
/// Same as [`factor`] but calls `on_attempt` after each coprime attempt,
/// useful for logging/UI.
pub fn factor_verbose<R: QuantumRunner + ?Sized, G>(
    n: u64,
    config: &ShorConfig,
    runner: &R,
    mut on_attempt: G,
) -> Option<(u64, u64)>
where
    G: FnMut(&ShorAttempt),
{
    for _ in 0..config.max_attempts {
        let a = random_coprime(n);

        // Check for trivial factor
        let g = a.gcd(&n);
        if g > 1 && g < n {
            let small = g.min(n / g);
            let big = g.max(n / g);
            let attempt = ShorAttempt {
                a,
                order: None,
                factors: Some((small, big)),
                shots_used: 0,
            };
            on_attempt(&attempt);
            return Some((small, big));
        }

        let (circuit, _n_counting) = build_order_finding_circuit(a, n);

        // Execute all shots at once, then process results with early exit
        let bit_registers = runner.run(&circuit, config.shots_per_attempt);
        let counting_shots = bit_registers
            .get("counting")
            .map(|s| s.as_slice())
            .unwrap_or(&[]);

        let mut order = None;
        let mut factors = None;
        let mut shots_used = 0;

        for (shot_idx, bits) in counting_shots.iter().enumerate() {
            shots_used = shot_idx + 1;

            if let Some(r) = extract_order(bits, a, n) {
                order = Some(r);
                if let Some(f) = find_factors(n, a, r) {
                    factors = Some(f);
                    break;
                }
            }
        }

        let attempt = ShorAttempt {
            a,
            order,
            factors,
            shots_used,
        };
        on_attempt(&attempt);

        if factors.is_some() {
            return factors;
        }
    }

    None
}

/// Build a quantum order-finding circuit for Shor's algorithm.
///
/// Qubit layout:
/// - `0..n_counting-1`: counting register (measured after QFT⁻¹)
/// - `n_counting..n_counting+n_bits-1`: work register
///
/// The circuit:
/// 1. Initializes work register to |1⟩
/// 2. Applies Hadamard to all counting qubits
/// 3. For each counting qubit k, applies controlled multiplication by a^(2^k) mod n
/// 4. Applies inverse QFT to counting register
/// 5. Measures counting register into readout "counting"
///
/// Returns `(circuit, n_counting)`.
///
/// # Panics
/// Panics if `n != 15` (only N=15 is currently supported).
pub(crate) fn build_order_finding_circuit(a: u64, n: u64) -> (Circuit, usize) {
    assert_eq!(n, 15, "Only N=15 is currently supported");
    assert!(a > 1 && a < n && a.gcd(&n) == 1, "a must be coprime to n");

    let n_bits: usize = 4;
    let n_counting = 2 * n_bits; // 8 counting qubits

    let mut alloc = QubitAllocator::new();
    // QPE internally allocates counting qubits 0..n_counting,
    // so we pad past that range for work qubits.
    let _counting_pad = alloc.allocate("counting", n_counting);
    let work_reg = alloc.allocate("work", n_bits);
    let work_qubits = work_reg.to_qubits();
    let work_indices: Vec<usize> = work_reg.iter().map(|q| q.index()).collect();

    // Initialize work register to |1⟩, then delegate to QPE
    let mut circuit = Circuit::new();
    circuit += PauliX::new(work_reg.qubit(0).index());

    let qpe = build_qpe_circuit(n_counting, &work_indices, |circ, ctrl, k| {
        let power = mod_pow(a, 1u64 << k, n);
        if power != 1 {
            controlled_modmul_15(circ, power, ctrl, &work_qubits);
        }
    });
    circuit += qpe;

    (circuit, n_counting)
}

/// Extract the order `r` from counting register measurement bits.
///
/// Uses continued fractions to find the denominator of the measured
/// phase, then verifies that `a^r ≡ 1 (mod n)`.
///
/// Returns `Some(r)` if a valid order is found, `None` otherwise
/// (the measurement may have been uninformative — retry with another shot).
pub(crate) fn extract_order(counting_bits: &[bool], a: u64, n: u64) -> Option<u64> {
    let n_counting = counting_bits.len();
    let dimension = 1u64 << n_counting;

    // Try both bit orderings since QFT convention may vary
    for &measured in &[
        bits_to_int_lsb(counting_bits),
        bits_to_int_msb(counting_bits),
    ] {
        if measured == 0 {
            continue;
        }

        let convs = convergents(measured, dimension, n_counting + 2);
        for &(_, q) in &convs {
            if q == 0 {
                continue;
            }
            // Check q and small multiples as candidate orders
            let mut candidate = q;
            while candidate < n {
                if candidate > 0 && mod_pow(a, candidate, n) == 1 {
                    return Some(candidate);
                }
                candidate += q;
            }
        }
    }

    None
}

/// Try to find non-trivial factors of `n` given that `a^r ≡ 1 (mod n)`.
///
/// Returns `Some((p, q))` with `p ≤ q` and `p * q == n` if successful.
pub(crate) fn find_factors(n: u64, a: u64, r: u64) -> Option<(u64, u64)> {
    if !r.is_multiple_of(2) {
        return None; // Need even order
    }
    let x = mod_pow(a, r / 2, n);
    if x == n - 1 || x == 1 {
        return None; // Trivial square root
    }

    let p = (x + 1).gcd(&n);
    let q = (x - 1).gcd(&n);

    if p > 1 && p < n && q > 1 && q < n {
        Some((p.min(q), p.max(q)))
    } else if p > 1 && p < n {
        Some((p.min(n / p), p.max(n / p)))
    } else if q > 1 && q < n {
        Some((q.min(n / q), q.max(n / q)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    fn run_one_shot(circuit: &Circuit) -> HashMap<String, Vec<Vec<bool>>> {
        let num_qubits = circuit.number_of_qubits();
        let backend = Backend::new(num_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
    }

    #[test]
    fn test_build_circuit_a7_n15() {
        let (circuit, n_counting) = build_order_finding_circuit(7, 15);
        assert_eq!(n_counting, 8);
        assert_eq!(circuit.number_of_qubits(), 12);
    }

    #[test]
    fn test_order_finding_a7_n15() {
        // a=7, N=15: order r=4
        // Ideal QPE measurements: 0, 64, 128, 192 (multiples of 256/4)
        let (circuit, _) = build_order_finding_circuit(7, 15);

        let mut found_order = false;
        for _ in 0..20 {
            let bit_registers = run_one_shot(&circuit);
            let bits = &bit_registers["counting"][0];

            if let Some(r) = extract_order(bits, 7, 15) {
                assert_eq!(r, 4, "Expected order 4 for a=7 mod 15, got {}", r);
                found_order = true;
                break;
            }
        }
        assert!(found_order, "Failed to find order in 20 attempts");
    }

    #[test]
    fn test_order_finding_a2_n15() {
        // a=2, N=15: order r=4
        let (circuit, _) = build_order_finding_circuit(2, 15);

        let mut found_order = false;
        for _ in 0..20 {
            let bit_registers = run_one_shot(&circuit);
            let bits = &bit_registers["counting"][0];

            if let Some(r) = extract_order(bits, 2, 15) {
                assert_eq!(r, 4, "Expected order 4 for a=2 mod 15, got {}", r);
                found_order = true;
                break;
            }
        }
        assert!(found_order, "Failed to find order in 20 attempts");
    }

    #[test]
    fn test_order_finding_a11_n15() {
        // a=11, N=15: order r=2
        let (circuit, _) = build_order_finding_circuit(11, 15);

        let mut found_order = false;
        for _ in 0..20 {
            let bit_registers = run_one_shot(&circuit);
            let bits = &bit_registers["counting"][0];

            if let Some(r) = extract_order(bits, 11, 15) {
                assert_eq!(r, 2, "Expected order 2 for a=11 mod 15, got {}", r);
                found_order = true;
                break;
            }
        }
        assert!(found_order, "Failed to find order in 20 attempts");
    }

    #[test]
    fn test_find_factors_15() {
        // a=7, r=4: 7^2 mod 15 = 4, gcd(5,15)=5, gcd(3,15)=3
        let result = find_factors(15, 7, 4);
        assert_eq!(result, Some((3, 5)));
    }

    #[test]
    fn test_find_factors_odd_order() {
        assert_eq!(find_factors(15, 7, 3), None);
    }

    #[test]
    fn test_find_factors_trivial_root() {
        // a=14, r=2: 14^1 mod 15 = 14 = n-1 → trivial
        assert_eq!(find_factors(15, 14, 2), None);
    }

    #[test]
    fn test_full_shor_pipeline() {
        // End-to-end: build circuit → simulate → extract order → find factors
        let n = 15u64;
        let a = 7u64;

        let (circuit, _) = build_order_finding_circuit(a, n);

        let mut factors = None;
        for _ in 0..30 {
            let bit_registers = run_one_shot(&circuit);
            let bits = &bit_registers["counting"][0];

            if let Some(r) = extract_order(bits, a, n) {
                if let Some(f) = find_factors(n, a, r) {
                    factors = Some(f);
                    break;
                }
            }
        }

        assert_eq!(factors, Some((3, 5)), "Should factor 15 into 3 × 5");
    }

    #[test]
    fn test_extract_order_known_measurement() {
        // Simulate known QPE output for a=7, N=15, r=4
        // Measurement should be 64 = 0b01000000 (MSB-first) or 0b00000010 (LSB-first)
        // 64/256 → convergent with denominator 4

        // MSB-first: bit 0 is MSB → 64 = bit[1]=1, rest=0
        let mut bits_msb = vec![false; 8];
        bits_msb[1] = true; // 64 in MSB-first = 0,1,0,0,0,0,0,0
        assert_eq!(extract_order(&bits_msb, 7, 15), Some(4));

        // LSB-first: bit 0 is LSB → 64 = bit[6]=1, rest=0
        let mut bits_lsb = vec![false; 8];
        bits_lsb[6] = true; // 64 in LSB-first = 0,0,0,0,0,0,1,0
        assert_eq!(extract_order(&bits_lsb, 7, 15), Some(4));
    }

    #[test]
    fn test_extract_order_measurement_192() {
        // 192/256 = 3/4 → denominator 4
        let mut bits_msb = vec![false; 8];
        bits_msb[0] = true; // 128
        bits_msb[1] = true; // 64 → total 192 in MSB-first
        assert_eq!(extract_order(&bits_msb, 7, 15), Some(4));
    }
}
