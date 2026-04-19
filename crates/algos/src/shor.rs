//! Shor's factoring algorithm.
//!
//! Orchestrates quantum period finding with classical pre/post-processing
//! to factor semiprimes.
//!
//! The library is backend-agnostic: the top-level [`factor`] function accepts
//! a closure that runs a circuit and returns measurement results. The caller
//! supplies the backend.
//!
//! Currently supports N=15 only (hardcoded modular multiplication).

use std::collections::HashMap;

use num_integer::Integer;
use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::modmul_15::controlled_modmul_15;
use crate::math::{convergents, mod_pow, random_coprime};

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
/// The `run_circuit` closure executes a quantum circuit and returns bit-register
/// measurements. This keeps the library backend-agnostic — the caller provides
/// the backend.
///
/// # Arguments
/// * `n` — the number to factor (currently only 15 supported)
/// * `config` — algorithm configuration
/// * `run_circuit` — closure `(circuit, total_qubits) -> HashMap<String, Vec<Vec<bool>>>`
///
/// # Returns
/// `Some((p, q))` with `p ≤ q` and `p * q == n`, or `None` if all attempts failed.
///
/// # Example
/// ```no_run
/// use algos::shor::{factor, ShorConfig};
/// use roqoqo::backends::EvaluatingBackend;
/// use roqoqo_quest::Backend;
///
/// let result = factor(15, &ShorConfig::default(), |circuit, total_qubits| {
///     let backend = Backend::new(total_qubits, None);
///     let (bits, _, _) = backend.run_circuit(circuit).unwrap();
///     bits
/// });
/// assert_eq!(result, Some((3, 5)));
/// ```
pub fn factor<F>(n: u64, config: &ShorConfig, run_circuit: F) -> Option<(u64, u64)>
where
    F: Fn(&Circuit, usize) -> HashMap<String, Vec<Vec<bool>>>,
{
    factor_verbose(n, config, run_circuit, |_| {})
}

/// Factor `n` with detailed per-attempt reporting via callback.
///
/// Same as [`factor`] but calls `on_attempt` after each coprime attempt,
/// useful for logging/UI.
pub fn factor_verbose<F, G>(
    n: u64,
    config: &ShorConfig,
    run_circuit: F,
    mut on_attempt: G,
) -> Option<(u64, u64)>
where
    F: Fn(&Circuit, usize) -> HashMap<String, Vec<Vec<bool>>>,
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

        let (circuit, _n_counting, total_qubits) = build_order_finding_circuit(a, n);
        let mut order = None;
        let mut factors = None;
        let mut shots_used = 0;

        for shot in 0..config.shots_per_attempt {
            shots_used = shot + 1;
            let bit_registers = run_circuit(&circuit, total_qubits);
            let bits = match bit_registers.get("counting") {
                Some(shots) if !shots.is_empty() => &shots[0],
                _ => continue,
            };

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
/// Returns `(circuit, n_counting, total_qubits)`.
///
/// # Panics
/// Panics if `n != 15` (only N=15 is currently supported).
pub(crate) fn build_order_finding_circuit(a: u64, n: u64) -> (Circuit, usize, usize) {
    assert_eq!(n, 15, "Only N=15 is currently supported");
    assert!(a > 1 && a < n && a.gcd(&n) == 1, "a must be coprime to n");

    let n_bits: usize = 4;
    let n_counting = 2 * n_bits; // 8 counting qubits
    let work: [usize; 4] = [n_counting, n_counting + 1, n_counting + 2, n_counting + 3];
    let total_qubits = n_counting + n_bits; // 12

    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("counting".to_string(), n_counting, true);

    // 1. Initialize work register to |1⟩ (LSB of work register)
    circuit += PauliX::new(work[0]);

    // 2. Hadamard on all counting qubits → equal superposition
    for i in 0..n_counting {
        circuit += Hadamard::new(i);
    }

    // 3. Controlled modular exponentiations:
    //    counting qubit k controls multiplication by a^(2^k) mod n
    for k in 0..n_counting {
        let power = mod_pow(a, 1u64 << k, n);
        if power != 1 {
            controlled_modmul_15(&mut circuit, power, k, work);
        }
    }

    // 4. Inverse QFT on counting register
    let counting_qubits: Vec<usize> = (0..n_counting).collect();
    circuit += QFT::new(counting_qubits, true, true);

    // 5. Measure counting register
    for i in 0..n_counting {
        circuit += MeasureQubit::new(i, "counting".to_string(), i);
    }

    (circuit, n_counting, total_qubits)
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

/// Convert measurement bits to integer (bit 0 = LSB).
fn bits_to_int_lsb(bits: &[bool]) -> u64 {
    bits.iter()
        .enumerate()
        .fold(0u64, |acc, (i, &b)| if b { acc | (1 << i) } else { acc })
}

/// Convert measurement bits to integer (bit 0 = MSB).
fn bits_to_int_msb(bits: &[bool]) -> u64 {
    let n = bits.len();
    bits.iter().enumerate().fold(
        0u64,
        |acc, (i, &b)| if b { acc | (1 << (n - 1 - i)) } else { acc },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;

    #[test]
    fn test_build_circuit_a7_n15() {
        let (circuit, n_counting, total_qubits) = build_order_finding_circuit(7, 15);
        assert_eq!(n_counting, 8);
        assert_eq!(total_qubits, 12);
        let _ = circuit;
    }

    #[test]
    fn test_order_finding_a7_n15() {
        // a=7, N=15: order r=4
        // Ideal QPE measurements: 0, 64, 128, 192 (multiples of 256/4)
        let (circuit, _, total) = build_order_finding_circuit(7, 15);
        let backend = Backend::new(total, None);

        let mut found_order = false;
        for _ in 0..20 {
            let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("sim failed");
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
        let (circuit, _, total) = build_order_finding_circuit(2, 15);
        let backend = Backend::new(total, None);

        let mut found_order = false;
        for _ in 0..20 {
            let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("sim failed");
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
        let (circuit, _, total) = build_order_finding_circuit(11, 15);
        let backend = Backend::new(total, None);

        let mut found_order = false;
        for _ in 0..20 {
            let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("sim failed");
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

        let (circuit, _, total) = build_order_finding_circuit(a, n);
        let backend = Backend::new(total, None);

        let mut factors = None;
        for _ in 0..30 {
            let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("sim failed");
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
