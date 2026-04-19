//! Multi-controlled-Z gate decomposition.
//!
//! Provides [`build_multi_cz`] which constructs a circuit that flips the phase
//! of |11…1⟩ on the given qubits.  For 1–3 qubits the native roqoqo gates
//! (PauliZ, ControlledPauliZ, ControlledControlledPauliZ) are used directly.
//! For n ≥ 4, a Barenco V-chain decomposition with ancillas is used.

use roqoqo::operations::*;
use roqoqo::Circuit;

/// Build a multi-controlled-Z gate that flips the phase of |11…1⟩.
///
/// # Arguments
/// * `qubits` — data qubits participating in the gate (len ≥ 1)
/// * `ancillas` — scratch qubits initialized to |0⟩; must have
///   `ancillas.len() == max(0, qubits.len() - 2)` for n ≥ 3, or be empty for n ≤ 2.
///
/// # Panics
/// Panics if `qubits` is empty or if the ancilla count doesn't match.
pub fn build_multi_cz(qubits: &[usize], ancillas: &[usize]) -> Circuit {
    let n = qubits.len();
    assert!(n >= 1, "build_multi_cz requires at least 1 qubit");

    let expected_ancillas = if n >= 4 { n - 2 } else { 0 };
    debug_assert_eq!(
        ancillas.len(),
        expected_ancillas,
        "Expected {} ancillas for {} qubits, got {}",
        expected_ancillas,
        n,
        ancillas.len()
    );

    let mut circuit = Circuit::new();

    match n {
        1 => {
            circuit += PauliZ::new(qubits[0]);
        }
        2 => {
            circuit += ControlledPauliZ::new(qubits[0], qubits[1]);
        }
        3 => {
            circuit += ControlledControlledPauliZ::new(qubits[0], qubits[1], qubits[2]);
        }
        _ => {
            // Barenco V-chain decomposition:
            // Forward pass: cascade Toffoli to propagate AND of controls into ancillas
            // Note: Toffoli::new(target, ctrl1, ctrl2) — first arg is TARGET
            circuit += Toffoli::new(ancillas[0], qubits[0], qubits[1]);
            for i in 1..ancillas.len() {
                circuit += Toffoli::new(ancillas[i], ancillas[i - 1], qubits[i + 1]);
            }

            // Apply CZ between last ancilla and last data qubit
            circuit += ControlledPauliZ::new(*ancillas.last().unwrap(), qubits[n - 1]);

            // Reverse pass: uncompute ancillas
            for i in (1..ancillas.len()).rev() {
                circuit += Toffoli::new(ancillas[i], ancillas[i - 1], qubits[i + 1]);
            }
            circuit += Toffoli::new(ancillas[0], qubits[0], qubits[1]);
        }
    }

    circuit
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    /// Run a circuit and return the bit register values.
    fn run(circuit: &Circuit, n_qubits: usize) -> HashMap<String, Vec<Vec<bool>>> {
        let backend = Backend::new(n_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
    }

    /// Verify CCZ is symmetric: all argument orderings produce the same result.
    /// This follows the project convention of testing gate semantics before use,
    /// established after the Toffoli argument-ordering bug.
    #[test]
    fn test_ccz_convention() {
        let orderings = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        // For each ordering, prepare |111⟩ and verify phase flip via interference
        for order in &orderings {
            let mut circuit = Circuit::new();
            circuit += DefinitionBit::new("m".to_string(), 1, true);

            // Prepare qubit 0 in |+⟩, qubits 1,2 in |1⟩
            circuit += Hadamard::new(0);
            circuit += PauliX::new(1);
            circuit += PauliX::new(2);

            // Apply CCZ with this ordering
            circuit += ControlledControlledPauliZ::new(order[0], order[1], order[2]);

            // Undo |1⟩ on qubits 1,2
            circuit += PauliX::new(1);
            circuit += PauliX::new(2);

            // Qubit 0 should now be in |−⟩ = H|1⟩
            circuit += Hadamard::new(0);
            circuit += MeasureQubit::new(0, "m".to_string(), 0);

            let results = run(&circuit, 3);
            let bit = results["m"][0][0];
            assert!(
                bit,
                "CCZ ordering {:?} should flip phase, measuring |1⟩",
                order
            );
        }
    }

    /// Multi-CZ on 1 qubit: PauliZ flips phase of |1⟩.
    #[test]
    fn test_multi_cz_1() {
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);

        // |+⟩ → Z → |−⟩ → H → |1⟩
        circuit += Hadamard::new(0);
        circuit += build_multi_cz(&[0], &[]);
        circuit += Hadamard::new(0);
        circuit += MeasureQubit::new(0, "m".to_string(), 0);

        let results = run(&circuit, 1);
        assert!(results["m"][0][0], "Z|+⟩ should give |1⟩ after H");
    }

    /// Multi-CZ on 2 qubits: CZ flips phase only when both are |1⟩.
    #[test]
    fn test_multi_cz_2() {
        // Prepare |+1⟩, apply CZ, check qubit 0 flipped
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);

        circuit += Hadamard::new(0);
        circuit += PauliX::new(1);
        circuit += build_multi_cz(&[0, 1], &[]);
        circuit += PauliX::new(1);
        circuit += Hadamard::new(0);
        circuit += MeasureQubit::new(0, "m".to_string(), 0);

        let results = run(&circuit, 2);
        assert!(results["m"][0][0], "CZ|+1⟩ should flip phase");

        // Prepare |+0⟩, apply CZ, check no flip
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);

        circuit2 += Hadamard::new(0);
        circuit2 += build_multi_cz(&[0, 1], &[]);
        circuit2 += Hadamard::new(0);
        circuit2 += MeasureQubit::new(0, "m".to_string(), 0);

        let results2 = run(&circuit2, 2);
        assert!(!results2["m"][0][0], "CZ|+0⟩ should not flip phase");
    }

    /// Multi-CZ on 3 qubits: CCZ flips phase only when all three are |1⟩.
    #[test]
    fn test_multi_cz_3() {
        // All |1⟩ → should flip
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);

        circuit += Hadamard::new(0);
        circuit += PauliX::new(1);
        circuit += PauliX::new(2);
        circuit += build_multi_cz(&[0, 1, 2], &[]);
        circuit += PauliX::new(1);
        circuit += PauliX::new(2);
        circuit += Hadamard::new(0);
        circuit += MeasureQubit::new(0, "m".to_string(), 0);

        let results = run(&circuit, 3);
        assert!(results["m"][0][0], "CCZ|+11⟩ should flip phase");

        // Only one other |1⟩ → should NOT flip
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);

        circuit2 += Hadamard::new(0);
        circuit2 += PauliX::new(1);
        // qubit 2 stays |0⟩
        circuit2 += build_multi_cz(&[0, 1, 2], &[]);
        circuit2 += PauliX::new(1);
        circuit2 += Hadamard::new(0);
        circuit2 += MeasureQubit::new(0, "m".to_string(), 0);

        let results2 = run(&circuit2, 3);
        assert!(!results2["m"][0][0], "CCZ|+10⟩ should not flip phase");
    }

    /// Multi-CZ on 4 qubits: V-chain decomposition with 2 ancillas.
    #[test]
    fn test_multi_cz_4() {
        // All |1⟩ → should flip
        let data_qubits = [0, 1, 2, 3];
        let ancillas = [4, 5]; // 4-2 = 2 ancillas

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);

        circuit += Hadamard::new(0);
        circuit += PauliX::new(1);
        circuit += PauliX::new(2);
        circuit += PauliX::new(3);
        circuit += build_multi_cz(&data_qubits, &ancillas);
        circuit += PauliX::new(1);
        circuit += PauliX::new(2);
        circuit += PauliX::new(3);
        circuit += Hadamard::new(0);
        circuit += MeasureQubit::new(0, "m".to_string(), 0);

        let results = run(&circuit, 6);
        assert!(results["m"][0][0], "MCZ(4)|+111⟩ should flip phase");

        // Only two others |1⟩ → should NOT flip
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);

        circuit2 += Hadamard::new(0);
        circuit2 += PauliX::new(1);
        circuit2 += PauliX::new(2);
        // qubit 3 stays |0⟩
        circuit2 += build_multi_cz(&data_qubits, &ancillas);
        circuit2 += PauliX::new(1);
        circuit2 += PauliX::new(2);
        circuit2 += Hadamard::new(0);
        circuit2 += MeasureQubit::new(0, "m".to_string(), 0);

        let results2 = run(&circuit2, 6);
        assert!(!results2["m"][0][0], "MCZ(4)|+110⟩ should not flip phase");
    }

    /// Multi-CZ on 5 qubits: V-chain with 3 ancillas.
    #[test]
    fn test_multi_cz_5() {
        let data_qubits = [0, 1, 2, 3, 4];
        let ancillas = [5, 6, 7]; // 5-2 = 3 ancillas

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);

        circuit += Hadamard::new(0);
        for &q in &data_qubits[1..] {
            circuit += PauliX::new(q);
        }
        circuit += build_multi_cz(&data_qubits, &ancillas);
        for &q in &data_qubits[1..] {
            circuit += PauliX::new(q);
        }
        circuit += Hadamard::new(0);
        circuit += MeasureQubit::new(0, "m".to_string(), 0);

        let results = run(&circuit, 8);
        assert!(results["m"][0][0], "MCZ(5)|+1111⟩ should flip phase");
    }

    /// Verify ancillas return to |0⟩ after V-chain uncomputation.
    #[test]
    fn test_ancilla_reset() {
        let data_qubits = [0, 1, 2, 3];
        let ancillas = [4, 5];

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("anc".to_string(), 2, true);

        // Prepare all data qubits as |1⟩
        for &q in &data_qubits {
            circuit += PauliX::new(q);
        }
        circuit += build_multi_cz(&data_qubits, &ancillas);

        // Measure ancillas — should be |0⟩
        circuit += MeasureQubit::new(4, "anc".to_string(), 0);
        circuit += MeasureQubit::new(5, "anc".to_string(), 1);

        let results = run(&circuit, 6);
        assert!(
            !results["anc"][0][0],
            "Ancilla 0 should be |0⟩ after uncomputation"
        );
        assert!(
            !results["anc"][0][1],
            "Ancilla 1 should be |0⟩ after uncomputation"
        );
    }
}
