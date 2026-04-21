//! Quantum Phase Estimation (QPE).
//!
//! Estimates the eigenvalue phase of a unitary operator U acting on an
//! eigenstate |ψ⟩: U|ψ⟩ = e^{2πiφ}|ψ⟩. The phase φ is encoded in a
//! counting register as a binary fraction.
//!
//! This module extracts the QPE pattern from `shor.rs` into a reusable
//! building block.

use roqoqo::operations::*;
use roqoqo::Circuit;

/// Build a QPE circuit.
///
/// # Arguments
/// * `n_counting` — number of counting qubits (precision bits)
/// * `work_qubits` — indices of the work register qubits (eigenstate)
/// * `controlled_powers` — function that appends controlled-U^(2^k) to
///   the circuit, given `(circuit, control_qubit, power_of_two_exponent)`.
///   Called for k = 0, 1, ..., n_counting-1.
///
/// # Returns
/// A circuit that:
/// 1. Applies Hadamard to all counting qubits
/// 2. Applies controlled-U^(2^k) for each counting qubit k
/// 3. Applies inverse QFT to the counting register
/// 4. Measures the counting register into readout "counting"
///
/// The work register is NOT initialized — the caller must prepare |ψ⟩
/// before this circuit (or prepend initialization gates).
///
/// Counting qubits use indices `0..n_counting`. Work qubits must not
/// overlap with this range.
///
/// # Example
/// ```no_run
/// use algos::qpe::build_qpe_circuit;
/// use roqoqo::operations::*;
///
/// let circuit = build_qpe_circuit(4, &[4], |circuit, ctrl, k| {
///     if k == 0 {
///         *circuit += ControlledPauliZ::new(ctrl, 4);
///     }
/// });
/// ```
pub fn build_qpe_circuit<F>(
    n_counting: usize,
    work_qubits: &[usize],
    mut controlled_powers: F,
) -> Circuit
where
    F: FnMut(&mut Circuit, usize, usize),
{
    assert!(n_counting >= 1, "need at least 1 counting qubit");

    // Verify no overlap between counting and work qubits
    for &wq in work_qubits {
        assert!(
            wq >= n_counting,
            "work qubit {} overlaps with counting range 0..{}",
            wq,
            n_counting
        );
    }

    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("counting".to_string(), n_counting, true);

    // 1. Hadamard on all counting qubits
    for i in 0..n_counting {
        circuit += Hadamard::new(i);
    }

    // 2. Controlled-U^(2^k) for each counting qubit
    for k in 0..n_counting {
        controlled_powers(&mut circuit, k, k);
    }

    // 3. Inverse QFT on counting register
    let counting_qubits: Vec<usize> = (0..n_counting).collect();
    circuit += QFT::new(counting_qubits, true, true);

    // 4. Measure counting register
    for i in 0..n_counting {
        circuit += MeasureQubit::new(i, "counting".to_string(), i);
    }

    circuit
}

/// Extract the phase from QPE counting register measurement.
///
/// Interprets the measured bits as a binary fraction φ = m / 2^n
/// where m is the integer value of the measurement.
///
/// Returns the phase in [0, 1).
pub fn extract_phase(counting_bits: &[bool]) -> f64 {
    let n = counting_bits.len();
    let m = bits_to_int_lsb(counting_bits);
    m as f64 / (1u64 << n) as f64
}

/// Convert measurement bits to integer (bit 0 = LSB).
pub fn bits_to_int_lsb(bits: &[bool]) -> u64 {
    bits.iter()
        .enumerate()
        .fold(0u64, |acc, (i, &b)| if b { acc | (1 << i) } else { acc })
}

/// Convert measurement bits to integer (bit 0 = MSB).
pub fn bits_to_int_msb(bits: &[bool]) -> u64 {
    let n = bits.len();
    bits.iter().enumerate().fold(
        0u64,
        |acc, (i, &b)| {
            if b {
                acc | (1 << (n - 1 - i))
            } else {
                acc
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;

    #[test]
    fn test_bits_to_int_lsb() {
        // 5 = 101 in binary, LSB-first: [true, false, true]
        assert_eq!(bits_to_int_lsb(&[true, false, true]), 5);
        assert_eq!(bits_to_int_lsb(&[]), 0);
        assert_eq!(bits_to_int_lsb(&[true]), 1);
    }

    #[test]
    fn test_bits_to_int_msb() {
        // 5 = 101 in binary, MSB-first: [true, false, true]
        assert_eq!(bits_to_int_msb(&[true, false, true]), 5);
        assert_eq!(bits_to_int_msb(&[]), 0);
    }

    #[test]
    fn test_extract_phase() {
        // 4 counting qubits, measurement = 4 → phase = 4/16 = 0.25
        let mut bits = vec![false; 4];
        bits[2] = true; // LSB-first: bit 2 = value 4
        assert!((extract_phase(&bits) - 0.25).abs() < 1e-14);
    }

    #[test]
    fn test_extract_phase_zero() {
        let bits = vec![false; 4];
        assert!((extract_phase(&bits)).abs() < 1e-14);
    }

    #[test]
    fn test_build_qpe_identity() {
        // QPE with identity unitary → phase = 0 → measurement = 0
        let circuit = build_qpe_circuit(3, &[3], |_circuit, _ctrl, _k| {
            // No gates — identity
        });

        let backend = Backend::new(circuit.number_of_qubits(), None);
        let (bits, _, _) = backend.run_circuit(&circuit).unwrap();
        let counting = &bits["counting"][0];

        // All counting bits should be 0 (phase = 0)
        assert!(
            counting.iter().all(|&b| !b),
            "QPE of identity should give phase 0, got {:?}",
            counting
        );
    }

    #[test]
    fn test_build_qpe_z_gate() {
        // QPE of Z gate on |1⟩: Z|1⟩ = -|1⟩ = e^{iπ}|1⟩ → phase = 0.5
        // Prepare |1⟩ on work qubit, then QPE with controlled-Z
        let n_counting = 4;
        let work = n_counting; // qubit 4

        let mut full = Circuit::new();
        full += PauliX::new(work); // prepare |1⟩

        let qpe = build_qpe_circuit(n_counting, &[work], |circuit, ctrl, k| {
            // controlled-Z^(2^k): for Z gate, Z^(2^k) = I when k≥1, Z when k=0
            // Actually Z^2 = I, so Z^(2^k) = Z if k=0, I if k≥1
            if k == 0 {
                *circuit += ControlledPauliZ::new(ctrl, work);
            }
        });
        full += qpe;

        let backend = Backend::new(full.number_of_qubits(), None);
        let (bits, _, _) = backend.run_circuit(&full).unwrap();
        let counting = &bits["counting"][0];

        let phase = extract_phase(counting);
        // Phase should be 0.5 (Z eigenvalue = -1 = e^{iπ}, φ = π/(2π) = 0.5)
        assert!(
            (phase - 0.5).abs() < 0.01,
            "QPE of Z on |1⟩ should give phase ≈ 0.5, got {}",
            phase
        );
    }

    #[test]
    #[should_panic(expected = "at least 1")]
    fn test_zero_counting_qubits() {
        build_qpe_circuit(0, &[0], |_, _, _| {});
    }

    #[test]
    #[should_panic(expected = "overlaps")]
    fn test_work_qubit_overlap() {
        build_qpe_circuit(4, &[2], |_, _, _| {});
    }
}
