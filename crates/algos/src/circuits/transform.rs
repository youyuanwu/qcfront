//! Circuit transformation utilities.
//!
//! Higher-level operations on roqoqo [`Circuit`]s: [`inverse`] reverses a
//! circuit, [`within_apply`] automates compute → action → uncompute, and
//! [`is_unitary`] checks whether a circuit contains only reversible gates.
//!
//! These are **circuit-to-circuit meta-utilities**, distinct from the gate
//! primitives in sibling modules (multi_cx, multi_cz, adder) which build
//! circuits from qubit indices.

use std::fmt;

use roqoqo::operations::*;
use roqoqo::Circuit;

/// Error returned when [`inverse`] encounters an unsupported gate type.
#[derive(Debug, Clone)]
pub struct UnsupportedGate {
    /// Name of the unsupported operation.
    pub gate_name: String,
}

impl fmt::Display for UnsupportedGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inverse: unsupported gate type '{}'", self.gate_name)
    }
}

/// Check whether a circuit contains only unitary (reversible) gates.
///
/// Returns `false` if any non-unitary operation is found (measurement,
/// classical register definition, reset, etc.). This is a structural
/// check — it scans operation types, not matrices.
pub fn is_unitary(circuit: &Circuit) -> bool {
    for op in circuit.iter() {
        if is_non_unitary(op) {
            return false;
        }
    }
    true
}

/// Reverse a circuit: reverse gate order, replace each gate with its inverse.
///
/// Returns `Err(UnsupportedGate)` if a non-unitary operation (DefinitionBit,
/// MeasureQubit, pragmas) or an unknown gate type is encountered. Callers
/// should pass only unitary sub-circuits — use [`is_unitary`] to check.
pub fn inverse(circuit: &Circuit) -> Result<Circuit, UnsupportedGate> {
    let ops: Vec<&Operation> = circuit.iter().collect();
    let mut inv = Circuit::new();

    for &op in ops.iter().rev() {
        inv += invert_gate(op)?;
    }

    Ok(inv)
}

/// Compute → action → uncompute pattern.
///
/// Produces: `compute + action + inverse(compute)`.
///
/// The compute circuit must be unitary (checked via `debug_assert`).
/// The action circuit must not permanently modify qubits that the
/// compute circuit depends on — temporary self-cancelling modifications
/// (e.g., X-MCZ-X) are safe.
pub fn within_apply(compute: &Circuit, action: &Circuit) -> Result<Circuit, UnsupportedGate> {
    debug_assert!(
        is_unitary(compute),
        "within_apply: compute circuit must be unitary"
    );

    let mut result = Circuit::new();
    result += compute.clone();
    result += action.clone();
    result += inverse(compute)?;
    Ok(result)
}

// -----------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------

/// Returns true for non-unitary operations that should be skipped.
fn is_non_unitary(op: &Operation) -> bool {
    matches!(
        op,
        Operation::DefinitionBit(_)
            | Operation::DefinitionFloat(_)
            | Operation::DefinitionComplex(_)
            | Operation::MeasureQubit(_)
            | Operation::PragmaSetNumberOfMeasurements(_)
            | Operation::PragmaRepeatedMeasurement(_)
            | Operation::InputBit(_)
    )
}

/// Produce the inverse of a single gate operation.
fn invert_gate(op: &Operation) -> Result<Circuit, UnsupportedGate> {
    let mut c = Circuit::new();

    match op {
        // Self-inverse gates — clone directly to avoid constructor arg-order issues
        Operation::PauliX(_)
        | Operation::PauliZ(_)
        | Operation::PauliY(_)
        | Operation::Hadamard(_)
        | Operation::CNOT(_)
        | Operation::Toffoli(_)
        | Operation::ControlledPauliZ(_)
        | Operation::ControlledControlledPauliZ(_) => {
            c += op.clone();
        }

        // Rotation gates: negate the angle
        Operation::RotateZ(g) => c += RotateZ::new(*g.qubit(), -g.theta().clone()),
        Operation::RotateY(g) => c += RotateY::new(*g.qubit(), -g.theta().clone()),
        Operation::RotateX(g) => c += RotateX::new(*g.qubit(), -g.theta().clone()),
        Operation::PhaseShiftState1(g) => {
            c += PhaseShiftState1::new(*g.qubit(), -g.theta().clone())
        }

        // SqrtPauliX: inverse is InvSqrtPauliX and vice versa
        Operation::SqrtPauliX(g) => c += InvSqrtPauliX::new(*g.qubit()),
        Operation::InvSqrtPauliX(g) => c += SqrtPauliX::new(*g.qubit()),

        other => {
            return Err(UnsupportedGate {
                gate_name: format!("{:?}", other)
                    .split('(')
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
            });
        }
    }

    Ok(c)
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

    // --- is_unitary ---

    #[test]
    fn test_is_unitary_pure_gates() {
        let mut c = Circuit::new();
        c += PauliX::new(0);
        c += CNOT::new(0, 1);
        c += Hadamard::new(0);
        assert!(is_unitary(&c));
    }

    #[test]
    fn test_is_unitary_with_measurement() {
        let mut c = Circuit::new();
        c += PauliX::new(0);
        c += DefinitionBit::new("m".to_string(), 1, true);
        c += MeasureQubit::new(0, "m".to_string(), 0);
        assert!(!is_unitary(&c));
    }

    #[test]
    fn test_is_unitary_empty() {
        assert!(is_unitary(&Circuit::new()));
    }

    // --- inverse: self-inverse gates ---

    #[test]
    fn test_inverse_x_roundtrip() {
        // X on qubit 0, then inverse → should be identity
        let mut c = Circuit::new();
        c += PauliX::new(0);
        c += PauliX::new(1);

        let inv = inverse(&c).unwrap();
        // inverse of [X(0), X(1)] = [X(1), X(0)]
        let ops: Vec<_> = inv.iter().collect();
        assert_eq!(ops.len(), 2);

        // Apply original + inverse = identity
        let mut full = Circuit::new();
        full += DefinitionBit::new("m".to_string(), 2, true);
        full += c;
        full += inv;
        full += MeasureQubit::new(0, "m".to_string(), 0);
        full += MeasureQubit::new(1, "m".to_string(), 1);
        let results = run(&full, 2);
        assert_eq!(read_register(&results, "m", 2), 0); // back to |00⟩
    }

    #[test]
    fn test_inverse_cnot_toffoli() {
        // Test CNOT: apply X(0) + CNOT(0,1), then inverse
        // Forward: |00⟩ → X(0) → |10⟩ → CNOT → |11⟩
        // Inverse: CNOT(0,1) → |10⟩ → X(0) → |00⟩
        let mut c = Circuit::new();
        c += PauliX::new(0);
        c += CNOT::new(0, 1);

        let inv = inverse(&c).unwrap();

        let mut full = Circuit::new();
        full += DefinitionBit::new("m".to_string(), 2, true);
        full += c;
        full += inv;
        full += MeasureQubit::new(0, "m".to_string(), 0);
        full += MeasureQubit::new(1, "m".to_string(), 1);
        let results = run(&full, 2);
        assert_eq!(read_register(&results, "m", 2), 0, "CNOT inverse failed");

        // Test Toffoli: X(0) + X(1) + Toffoli(2,0,1), then inverse
        let mut c2 = Circuit::new();
        c2 += PauliX::new(0);
        c2 += PauliX::new(1);
        c2 += Toffoli::new(2, 0, 1); // target=2

        let inv2 = inverse(&c2).unwrap();

        let mut full2 = Circuit::new();
        full2 += DefinitionBit::new("m2".to_string(), 3, true);
        full2 += c2;
        full2 += inv2;
        for i in 0..3 {
            full2 += MeasureQubit::new(i, "m2".to_string(), i);
        }
        let results2 = run(&full2, 3);
        assert_eq!(
            read_register(&results2, "m2", 3),
            0,
            "Toffoli inverse failed"
        );
    }

    // --- inverse: rotation gates ---

    #[test]
    fn test_inverse_rotate_z() {
        // RotateZ(π/2) then inverse → identity
        use std::f64::consts::FRAC_PI_2;
        let mut c = Circuit::new();
        c += Hadamard::new(0); // put in superposition to see phase effect
        c += RotateZ::new(0, FRAC_PI_2.into());

        let inv = inverse(&c).unwrap();

        let mut full = Circuit::new();
        full += DefinitionBit::new("m".to_string(), 1, true);
        full += c;
        full += inv;
        full += MeasureQubit::new(0, "m".to_string(), 0);
        let results = run(&full, 1);
        assert_eq!(read_register(&results, "m", 1), 0);
    }

    // --- inverse: skip non-unitary ---

    #[test]
    fn test_inverse_rejects_non_unitary() {
        let mut c = Circuit::new();
        c += DefinitionBit::new("m".to_string(), 1, true);
        c += PauliX::new(0);

        let result = inverse(&c);
        assert!(result.is_err(), "inverse should reject non-unitary ops");
    }

    // --- inverse: unsupported gate ---

    #[test]
    fn test_inverse_unsupported_gate() {
        let mut c = Circuit::new();
        c += GPi::new(0, 0.5.into()); // exotic gate
        let result = inverse(&c);
        assert!(result.is_err());
        assert!(result.unwrap_err().gate_name.contains("GPi"));
    }

    // --- inverse(inverse(c)) == c ---

    #[test]
    fn test_inverse_roundtrip() {
        let mut c = Circuit::new();
        c += PauliX::new(0);
        c += CNOT::new(0, 1);
        c += Hadamard::new(1);

        // c then inverse(c) should be identity
        let inv = inverse(&c).unwrap();
        let roundtrip = inverse(&inv).unwrap();

        // Apply original, then inverse, then roundtrip — should equal original
        let mut full = Circuit::new();
        full += DefinitionBit::new("m".to_string(), 2, true);
        full += c.clone();
        full += inv;
        full += roundtrip;
        full += MeasureQubit::new(0, "m".to_string(), 0);
        full += MeasureQubit::new(1, "m".to_string(), 1);
        let results = run(&full, 2);
        // c + inv = I, then roundtrip = c again
        // So result = c applied to |00⟩ = X(0)→CNOT(0,1)→H(1) = |1⟩|1⟩ superposition
        // H on |1⟩ gives |−⟩, but measurement is deterministic for |1⟩ before H
        // Actually: X(0)|00⟩=|10⟩, CNOT(0,1)|10⟩=|11⟩, H(1)|11⟩ = |1⟩(|0⟩-|1⟩)/√2
        // Measurement of q0 = 1 always
        assert!(results["m"][0][0]); // q0 = 1
    }

    // --- within_apply ---

    #[test]
    fn test_within_apply_basic() {
        // Compute: X on q0 (flip to |1⟩)
        // Action: CNOT(0, 1) (copy q0 to q1)
        // Uncompute: X on q0 (flip back to |0⟩)
        // Net: q0 = |0⟩, q1 = |1⟩
        let mut compute = Circuit::new();
        compute += PauliX::new(0);

        let mut action = Circuit::new();
        action += CNOT::new(0, 1);

        let result_circuit = within_apply(&compute, &action).unwrap();

        let mut full = Circuit::new();
        full += DefinitionBit::new("m".to_string(), 2, true);
        full += result_circuit;
        full += MeasureQubit::new(0, "m".to_string(), 0);
        full += MeasureQubit::new(1, "m".to_string(), 1);

        let results = run(&full, 2);
        assert!(!results["m"][0][0], "q0 should be |0⟩ (uncomputed)");
        assert!(results["m"][0][1], "q1 should be |1⟩ (action result)");
    }

    #[test]
    fn test_within_apply_adder_roundtrip() {
        // Verify within_apply produces correct uncompute for controlled_add
        use crate::circuits::adder;

        let m = 3;
        let control = 0;
        let sum_qubits: Vec<usize> = (1..=m).collect();
        let scratch_count = adder::required_scratch(m);
        let scratch: Vec<usize> = (m + 1..m + 1 + scratch_count).collect();
        let total_qubits = 1 + m + scratch_count;

        // Build compute circuit: add 5 to sum register
        let mut compute = Circuit::new();
        adder::controlled_add(&mut compute, control, &sum_qubits, &scratch, 5);

        // Action: just measure (no-op for this test — we verify sum returns to 0)
        let action = Circuit::new();

        let wa = within_apply(&compute, &action).unwrap();

        let mut full = Circuit::new();
        full += DefinitionBit::new("sum".to_string(), m, true);
        full += PauliX::new(control); // control on
                                      // sum starts at 3
        full += PauliX::new(sum_qubits[0]);
        full += PauliX::new(sum_qubits[1]);

        full += wa;

        for (i, &sq) in sum_qubits.iter().enumerate() {
            full += MeasureQubit::new(sq, "sum".to_string(), i);
        }

        let results = run(&full, total_qubits);
        // within_apply(add5, noop) = add5 + inverse(add5) = identity
        // sum should still be 3
        assert_eq!(read_register(&results, "sum", m), 3);
    }

    #[test]
    fn test_within_apply_empty_compute() {
        let compute = Circuit::new();
        let mut action = Circuit::new();
        action += PauliX::new(0);

        let result = within_apply(&compute, &action).unwrap();
        // Empty compute → result is just the action
        assert_eq!(result.iter().count(), 1);
    }

    // --- inverse matches hand-written controlled_add_inverse ---

    #[test]
    fn test_inverse_matches_handwritten_adder() {
        use crate::circuits::adder;

        let m = 3;
        let control = 0;
        let sum_qubits: Vec<usize> = (1..=m).collect();
        let scratch_count = adder::required_scratch(m);
        let scratch: Vec<usize> = (m + 1..m + 1 + scratch_count).collect();
        let total_qubits = 1 + m + scratch_count;

        // For all initial sums and k values, verify inverse() matches
        // the hand-written controlled_add_inverse
        for k in 1..8u64 {
            for initial in 0..8u64 {
                // Method 1: hand-written inverse
                let mut c1 = Circuit::new();
                c1 += DefinitionBit::new("s1".to_string(), m, true);
                c1 += PauliX::new(control);
                for (i, &sq) in sum_qubits.iter().enumerate() {
                    if (initial >> i) & 1 == 1 {
                        c1 += PauliX::new(sq);
                    }
                }
                adder::controlled_add(&mut c1, control, &sum_qubits, &scratch, k);
                adder::controlled_add_inverse(&mut c1, control, &sum_qubits, &scratch, k);
                for (i, &sq) in sum_qubits.iter().enumerate() {
                    c1 += MeasureQubit::new(sq, "s1".to_string(), i);
                }

                // Method 2: inverse() utility
                let mut add_circuit = Circuit::new();
                adder::controlled_add(&mut add_circuit, control, &sum_qubits, &scratch, k);
                let inv = inverse(&add_circuit).unwrap();

                let mut c2 = Circuit::new();
                c2 += DefinitionBit::new("s2".to_string(), m, true);
                c2 += PauliX::new(control);
                for (i, &sq) in sum_qubits.iter().enumerate() {
                    if (initial >> i) & 1 == 1 {
                        c2 += PauliX::new(sq);
                    }
                }
                adder::controlled_add(&mut c2, control, &sum_qubits, &scratch, k);
                c2 += inv;
                for (i, &sq) in sum_qubits.iter().enumerate() {
                    c2 += MeasureQubit::new(sq, "s2".to_string(), i);
                }

                let r1 = run(&c1, total_qubits);
                let r2 = run(&c2, total_qubits);
                let v1 = read_register(&r1, "s1", m);
                let v2 = read_register(&r2, "s2", m);
                assert_eq!(
                    v1, v2,
                    "inverse() disagrees with hand-written for k={}, initial={}: {} vs {}",
                    k, initial, v1, v2
                );
                assert_eq!(v1, initial); // both should restore original
            }
        }
    }
}
