//! Controlled classical adder for quantum registers.
//!
//! Provides [`controlled_add`] which adds a classical constant `k` to a
//! quantum register `sum[0..m-1]`, controlled by a single qubit. Uses an
//! MCX-cascade incrementer — no carry register needed.
//!
//! The inverse can be obtained via [`super::transform::inverse`].
//!
//! This is a domain-independent arithmetic primitive used by
//! [`crate::grover::SubsetSumOracle`] and potentially other algorithms
//! that need reversible addition.

use roqoqo::operations::*;
use roqoqo::Circuit;

use super::multi_cx::{build_multi_cx, required_ancillas};
use crate::qubit::Qubit;

/// Build a circuit that adds classical constant `k` to quantum register
/// `sum_qubits`, controlled by `control`.
///
/// When `control` is |1⟩, the sum register is incremented by `k`.
/// When `control` is |0⟩, the sum register is unchanged.
///
/// # Algorithm
///
/// For each set bit j of k (processed MSB to LSB):
/// ```text
/// for p in (m-1)..=(j+1):
///     MCX(controls=[control, sum_j, ..., sum_{p-1}], target=sum_p)
/// CNOT(control, sum_j)
/// ```
///
/// MSB-first ordering invariant: at the point MCX fires for carry into
/// `sum_p`, all control qubits `sum_j … sum_{p-1}` still hold their
/// pre-addition values because only bits at index > p have been touched
/// and the CNOT on `sum_j` happens last.
///
/// # Arguments
/// * `control` — qubit that enables/disables the addition
/// * `sum_qubits` — quantum register holding the running sum (LSB-first)
/// * `scratch` — MCX decomposition scratch; must have
///   `scratch.len() >= required_scratch(sum_qubits.len())`
/// * `k` — classical constant to add
///
/// # Panics
/// - If `sum_qubits` is empty
/// - If `k >= 2^m` where `m = sum_qubits.len()` (would overflow)
/// - If `scratch` is too small for the largest MCX
pub fn controlled_add(
    circuit: &mut Circuit,
    control: Qubit,
    sum_qubits: &[Qubit],
    scratch: &[Qubit],
    k: u64,
) {
    let m = sum_qubits.len();
    assert!(m >= 1, "sum_qubits must not be empty");
    assert!(
        k < (1u64 << m),
        "k={} overflows {}-bit sum register (max {})",
        k,
        m,
        (1u64 << m) - 1
    );

    if k == 0 {
        return; // nothing to add
    }

    // Process set bits from MSB to LSB
    for j in (0..m).rev() {
        if (k >> j) & 1 == 0 {
            continue;
        }
        // Add 2^j: carry cascade from MSB down to j+1
        for p in (j + 1..m).rev() {
            // MCX: controls = [control, sum_j, sum_{j+1}, ..., sum_{p-1}]
            //       target  = sum_p
            let num_sum_controls = p - j; // sum_j through sum_{p-1}
            let nc = 1 + num_sum_controls; // +1 for control qubit
            let mut controls = Vec::with_capacity(nc);
            controls.push(control);
            controls.extend(sum_qubits[j..p].iter().copied());
            let mcx_anc_needed = required_ancillas(nc);
            let mcx_scratch = &scratch[..mcx_anc_needed];
            *circuit += build_multi_cx(sum_qubits[p], &controls, mcx_scratch);
        }
        // Flip bit j itself
        *circuit += CNOT::new(control.index(), sum_qubits[j].index());
    }
}

/// Number of scratch qubits required for [`controlled_add`] on an
/// `m`-qubit sum register.
///
/// The largest MCX has `m` controls (1 data qubit + m-1 sum qubits),
/// arising when adding 2⁰ and propagating carry through all higher bits.
pub fn required_scratch(m: usize) -> usize {
    if m == 0 {
        return 0;
    }
    required_ancillas(m) // MCX with m controls
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    fn q(i: usize) -> Qubit {
        Qubit::from_raw(i)
    }
    fn qs(indices: &[usize]) -> Vec<Qubit> {
        indices.iter().map(|&i| q(i)).collect()
    }

    fn run(circuit: &Circuit, n_qubits: usize) -> HashMap<String, Vec<Vec<bool>>> {
        let backend = Backend::new(n_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
    }

    fn read_register(results: &HashMap<String, Vec<Vec<bool>>>, name: &str, width: usize) -> u64 {
        let bits = &results[name][0];
        let mut val = 0u64;
        for (i, &b) in bits.iter().enumerate().take(width) {
            if b {
                val |= 1 << i;
            }
        }
        val
    }

    /// Prepare sum register to a given value and run controlled_add.
    fn test_add(initial_sum: u64, k: u64, m: usize, control_on: bool) -> u64 {
        let control = q(0);
        let sum_qubits = qs(&(1..=m).collect::<Vec<_>>());
        let scratch_count = required_scratch(m);
        let scratch = qs(&(m + 1..m + 1 + scratch_count).collect::<Vec<_>>());
        let total_qubits = 1 + m + scratch_count;

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("sum".to_string(), m, true);

        // Set control
        if control_on {
            circuit += PauliX::new(control.index());
        }
        // Set initial sum
        for (i, sq) in sum_qubits.iter().enumerate() {
            if (initial_sum >> i) & 1 == 1 {
                circuit += PauliX::new(sq.index());
            }
        }

        controlled_add(&mut circuit, control, &sum_qubits, &scratch, k);

        // Measure sum register
        for (i, sq) in sum_qubits.iter().enumerate() {
            circuit += MeasureQubit::new(sq.index(), "sum".to_string(), i);
        }

        let results = run(&circuit, total_qubits);
        read_register(&results, "sum", m)
    }

    #[test]
    fn test_add_3_to_0() {
        // 0 + 3 = 3 (m=4, control on)
        assert_eq!(test_add(0, 3, 4, true), 3);
    }

    #[test]
    fn test_add_5_to_3() {
        // 3 + 5 = 8 (m=4, control on)
        assert_eq!(test_add(3, 5, 4, true), 8);
    }

    #[test]
    fn test_add_3_to_5() {
        // 5 + 3 = 8 (m=4, control on)
        assert_eq!(test_add(5, 3, 4, true), 8);
    }

    #[test]
    fn test_add_with_carry_propagation() {
        // 7 + 1 = 8 (carry through all 3 lower bits, m=4)
        assert_eq!(test_add(7, 1, 4, true), 8);
    }

    #[test]
    fn test_add_full_carry() {
        // 5 + 3 = 8 in worked example from design doc
        assert_eq!(test_add(5, 3, 4, true), 8);
    }

    #[test]
    fn test_add_control_off() {
        // Control = 0: sum should be unchanged
        assert_eq!(test_add(3, 5, 4, false), 3);
    }

    #[test]
    fn test_add_zero() {
        // Adding 0 is a no-op
        assert_eq!(test_add(7, 0, 4, true), 7);
    }

    #[test]
    fn test_add_m1() {
        // m=1: single bit, add 1 to 0 → 1
        assert_eq!(test_add(0, 1, 1, true), 1);
        // m=1: add 1 to 1 → 0 (overflow wraps in mod 2)
        assert_eq!(test_add(1, 1, 1, false), 1); // control off
    }

    #[test]
    fn test_add_m2() {
        // m=2: 1 + 2 = 3
        assert_eq!(test_add(1, 2, 2, true), 3);
        // m=2: 3 + 1 = 0 (mod 4)
        assert_eq!(test_add(3, 1, 2, true), 0);
    }

    #[test]
    fn test_add_m3() {
        // m=3: 5 + 3 = 0 (mod 8)
        assert_eq!(test_add(5, 3, 3, true), 0);
        // m=3: 2 + 3 = 5
        assert_eq!(test_add(2, 3, 3, true), 5);
    }

    /// Verify that add followed by inverse restores original sum.
    #[test]
    fn test_add_inverse_roundtrip() {
        use super::super::transform::inverse;

        let m = 4;
        let control = q(0);
        let sum_qubits = qs(&(1..=m).collect::<Vec<_>>());
        let scratch_count = required_scratch(m);
        let scratch = qs(&(m + 1..m + 1 + scratch_count).collect::<Vec<_>>());
        let total_qubits = 1 + m + scratch_count;

        let initial = 3u64;
        let k = 5u64;

        // Build add circuit, then use inverse() to undo it
        let mut add_circuit = Circuit::new();
        controlled_add(&mut add_circuit, control, &sum_qubits, &scratch, k);
        let inv = inverse(&add_circuit).unwrap();

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("sum".to_string(), m, true);
        circuit += PauliX::new(control.index());

        for (i, sq) in sum_qubits.iter().enumerate() {
            if (initial >> i) & 1 == 1 {
                circuit += PauliX::new(sq.index());
            }
        }

        circuit += add_circuit;
        circuit += inv;

        for (i, sq) in sum_qubits.iter().enumerate() {
            circuit += MeasureQubit::new(sq.index(), "sum".to_string(), i);
        }

        let results = run(&circuit, total_qubits);
        assert_eq!(read_register(&results, "sum", m), initial);
    }

    /// Exhaustive test: verify add + inverse() = identity for all initial
    /// sums and several k values with m=3.
    #[test]
    fn test_add_inverse_exhaustive_m3() {
        use super::super::transform::inverse;

        let m = 3;
        let control = q(0);
        let sum_qubits = qs(&(1..=m).collect::<Vec<_>>());
        let scratch_count = required_scratch(m);
        let scratch = qs(&(m + 1..m + 1 + scratch_count).collect::<Vec<_>>());
        let total_qubits = 1 + m + scratch_count;

        for k in 1..8u64 {
            for initial in 0..8u64 {
                let mut add_circuit = Circuit::new();
                controlled_add(&mut add_circuit, control, &sum_qubits, &scratch, k);
                let inv = inverse(&add_circuit).unwrap();

                let mut circuit = Circuit::new();
                circuit += DefinitionBit::new("sum".to_string(), m, true);
                circuit += PauliX::new(control.index());

                for (i, sq) in sum_qubits.iter().enumerate() {
                    if (initial >> i) & 1 == 1 {
                        circuit += PauliX::new(sq.index());
                    }
                }

                circuit += add_circuit;
                circuit += inv;

                for (i, sq) in sum_qubits.iter().enumerate() {
                    circuit += MeasureQubit::new(sq.index(), "sum".to_string(), i);
                }

                let results = run(&circuit, total_qubits);
                let result = read_register(&results, "sum", m);
                assert_eq!(
                    result, initial,
                    "roundtrip failed: initial={}, k={}, got={}",
                    initial, k, result
                );
            }
        }
    }

    /// Verify scratch qubits return to |0⟩ after controlled_add.
    #[test]
    fn test_scratch_reset() {
        let m = 4;
        let control = q(0);
        let sum_qubits = qs(&(1..=m).collect::<Vec<_>>());
        let scratch_count = required_scratch(m);
        let scratch = qs(&(m + 1..m + 1 + scratch_count).collect::<Vec<_>>());
        let total_qubits = 1 + m + scratch_count;

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("scratch".to_string(), scratch_count, true);
        circuit += PauliX::new(control.index());
        // sum = 7 (0111), k = 5 (101) → triggers full carry propagation
        for sq in &sum_qubits[..3] {
            circuit += PauliX::new(sq.index());
        }

        controlled_add(&mut circuit, control, &sum_qubits, &scratch, 5);

        for (i, sq) in scratch.iter().enumerate() {
            circuit += MeasureQubit::new(sq.index(), "scratch".to_string(), i);
        }

        let results = run(&circuit, total_qubits);
        for (i, &b) in results["scratch"][0].iter().enumerate().take(scratch_count) {
            assert!(!b, "scratch qubit {} should be |0⟩ after add", i);
        }
    }

    /// Verify correctness of addition for the design doc worked example.
    #[test]
    fn test_design_doc_example() {
        // S = {3, 5, 7}, T = 8
        // Add 3, then 5 → should get 8
        let m = 4;
        let sum_qubits = qs(&(2..2 + m).collect::<Vec<_>>()); // qubits 2-5
        let scratch_count = required_scratch(m);
        let scratch = qs(&(2 + m..2 + m + scratch_count).collect::<Vec<_>>());
        let total_qubits = 2 + m + scratch_count;

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("sum".to_string(), m, true);

        // data qubit 0 = include s₀=3 (control on)
        circuit += PauliX::new(0);
        controlled_add(&mut circuit, q(0), &sum_qubits, &scratch, 3);

        // data qubit 1 = include s₁=5 (control on)
        circuit += PauliX::new(1);
        controlled_add(&mut circuit, q(1), &sum_qubits, &scratch, 5);

        for (i, sq) in sum_qubits.iter().enumerate() {
            circuit += MeasureQubit::new(sq.index(), "sum".to_string(), i);
        }

        let results = run(&circuit, total_qubits);
        assert_eq!(read_register(&results, "sum", m), 8);
    }
}
