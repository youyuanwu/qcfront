//! Multi-controlled-X gate decomposition (generalized Toffoli).
//!
//! Provides [`build_multi_cx`] which flips the **value** of a target qubit
//! conditioned on all control qubits being |1⟩. For 1–2 controls the native
//! roqoqo gates (CNOT, Toffoli) are used directly. For 3+ controls, a Barenco
//! V-chain decomposition with ancillas is used.
//!
//! Compare with [`super::multi_cz::build_multi_cz`] which flips the **phase**
//! (diagonal operation). MCX flips a bit (non-diagonal), needed for computing
//! boolean functions into ancilla qubits (e.g., SAT clause evaluation).

use roqoqo::operations::*;
use roqoqo::Circuit;

/// Number of ancilla qubits required by [`build_multi_cx`] for `nc` controls.
pub fn required_ancillas(nc: usize) -> usize {
    if nc >= 3 {
        nc - 2
    } else {
        0
    }
}

/// Build a multi-controlled-X gate: flip `target` if all `controls` are |1⟩.
///
/// # Arguments
/// * `target` — qubit whose value is flipped
/// * `controls` — qubits that must all be |1⟩ for the flip (len ≥ 1)
/// * `ancillas` — scratch qubits initialized to |0⟩; must have
///   `ancillas.len() == required_ancillas(controls.len())`.
///
/// # Panics
/// Panics if `controls` is empty or if the ancilla count doesn't match.
pub fn build_multi_cx(target: usize, controls: &[usize], ancillas: &[usize]) -> Circuit {
    let nc = controls.len();
    assert!(nc >= 1, "build_multi_cx requires at least 1 control");

    let expected_ancillas = required_ancillas(nc);
    debug_assert_eq!(
        ancillas.len(),
        expected_ancillas,
        "Expected {} ancillas for {} controls, got {}",
        expected_ancillas,
        nc,
        ancillas.len()
    );

    let mut circuit = Circuit::new();

    match nc {
        1 => {
            // CNOT: control → target
            circuit += CNOT::new(controls[0], target);
        }
        2 => {
            // Toffoli: target, ctrl1, ctrl2
            circuit += Toffoli::new(target, controls[0], controls[1]);
        }
        _ => {
            // V-chain decomposition:
            // Forward pass: AND controls[0..nc-2] into ancillas
            // Note: Toffoli::new(target, ctrl1, ctrl2) — first arg is TARGET
            circuit += Toffoli::new(ancillas[0], controls[0], controls[1]);
            for i in 1..ancillas.len() {
                circuit += Toffoli::new(ancillas[i], ancillas[i - 1], controls[i + 1]);
            }

            // Bottom: Toffoli with last ancilla, last control → target
            // last ancilla carries AND(controls[0..nc-2]), last control = controls[nc-1]
            circuit += Toffoli::new(target, *ancillas.last().unwrap(), controls[nc - 1]);

            // Reverse pass: uncompute ancillas
            for i in (1..ancillas.len()).rev() {
                circuit += Toffoli::new(ancillas[i], ancillas[i - 1], controls[i + 1]);
            }
            circuit += Toffoli::new(ancillas[0], controls[0], controls[1]);
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

    fn run(circuit: &Circuit, n_qubits: usize) -> HashMap<String, Vec<Vec<bool>>> {
        let backend = Backend::new(n_qubits, None);
        let (bits, _, _) = backend.run_circuit(circuit).unwrap();
        bits
    }

    /// MCX with 1 control = CNOT.
    #[test]
    fn test_multi_cx_1_control() {
        // |1⟩|0⟩ → CNOT → |1⟩|1⟩
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);
        circuit += PauliX::new(0); // control = |1⟩
        circuit += build_multi_cx(1, &[0], &[]);
        circuit += MeasureQubit::new(1, "m".to_string(), 0);

        let results = run(&circuit, 2);
        assert!(results["m"][0][0], "CNOT should flip target when control=1");

        // |0⟩|0⟩ → CNOT → |0⟩|0⟩
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);
        circuit2 += build_multi_cx(1, &[0], &[]);
        circuit2 += MeasureQubit::new(1, "m".to_string(), 0);

        let results2 = run(&circuit2, 2);
        assert!(!results2["m"][0][0], "CNOT should not flip when control=0");
    }

    /// MCX with 2 controls = Toffoli.
    #[test]
    fn test_multi_cx_2_controls() {
        // |11⟩|0⟩ → Toffoli → |11⟩|1⟩
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);
        circuit += PauliX::new(0);
        circuit += PauliX::new(1);
        circuit += build_multi_cx(2, &[0, 1], &[]);
        circuit += MeasureQubit::new(2, "m".to_string(), 0);

        let results = run(&circuit, 3);
        assert!(
            results["m"][0][0],
            "Toffoli should flip when both controls=1"
        );

        // |10⟩|0⟩ → no flip
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);
        circuit2 += PauliX::new(0);
        circuit2 += build_multi_cx(2, &[0, 1], &[]);
        circuit2 += MeasureQubit::new(2, "m".to_string(), 0);

        let results2 = run(&circuit2, 3);
        assert!(
            !results2["m"][0][0],
            "Toffoli should not flip when one control=0"
        );
    }

    /// MCX with 3 controls = V-chain with 1 ancilla.
    #[test]
    fn test_multi_cx_3_controls() {
        let controls = [0, 1, 2];
        let target = 3;
        let ancillas = [4]; // 3-2 = 1 ancilla

        // All controls |1⟩ → flip target
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("m".to_string(), 1, true);
        for &c in &controls {
            circuit += PauliX::new(c);
        }
        circuit += build_multi_cx(target, &controls, &ancillas);
        circuit += MeasureQubit::new(target, "m".to_string(), 0);

        let results = run(&circuit, 5);
        assert!(results["m"][0][0], "MCX(3) should flip when all controls=1");

        // One control |0⟩ → no flip
        let mut circuit2 = Circuit::new();
        circuit2 += DefinitionBit::new("m".to_string(), 1, true);
        circuit2 += PauliX::new(0);
        circuit2 += PauliX::new(1);
        // control 2 stays |0⟩
        circuit2 += build_multi_cx(target, &controls, &ancillas);
        circuit2 += MeasureQubit::new(target, "m".to_string(), 0);

        let results2 = run(&circuit2, 5);
        assert!(
            !results2["m"][0][0],
            "MCX(3) should not flip when one control=0"
        );
    }

    /// Verify ancillas return to |0⟩ after V-chain.
    #[test]
    fn test_multi_cx_ancilla_reset() {
        let controls = [0, 1, 2];
        let target = 3;
        let ancillas = [4];

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("anc".to_string(), 1, true);
        for &c in &controls {
            circuit += PauliX::new(c);
        }
        circuit += build_multi_cx(target, &controls, &ancillas);
        circuit += MeasureQubit::new(4, "anc".to_string(), 0);

        let results = run(&circuit, 5);
        assert!(
            !results["anc"][0][0],
            "Ancilla should be |0⟩ after uncomputation"
        );
    }
}
