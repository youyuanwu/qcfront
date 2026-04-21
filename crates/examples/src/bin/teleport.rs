// Quantum Teleportation
//
// Teleports an arbitrary qubit state from Alice to Bob using a shared
// Bell pair and two classical bits of communication.
//
// Protocol:
//   1. Alice and Bob share a Bell pair (qubits 1, 2)
//   2. Alice has a state |ψ⟩ to teleport (qubit 0)
//   3. Alice applies CNOT(0,1) then H(0) and measures qubits 0,1
//   4. Bob applies corrections based on Alice's measurements:
//      - If Alice's qubit 1 = 1: Bob applies X
//      - If Alice's qubit 0 = 1: Bob applies Z
//   5. Bob's qubit 2 is now in state |ψ⟩
//
// We verify by extracting the statevector, projecting onto Alice's
// measurement outcomes, applying Bob's corrections, and checking fidelity.

use algos::runner::Bit;
use algos::state::{fidelity, prepare_state, QuantumState};
use num_complex::Complex64;
use roqoqo::backends::EvaluatingBackend;
use roqoqo::operations::*;
use roqoqo::Circuit;
use roqoqo_quest::Backend;
use std::collections::HashMap;

fn main() {
    // The state to teleport: |ψ⟩ = cos(π/8)|0⟩ + e^{iπ/3}sin(π/8)|1⟩
    let theta = std::f64::consts::PI / 4.0;
    let phi = std::f64::consts::PI / 3.0;
    let alpha = Complex64::new((theta / 2.0).cos(), 0.0);
    let beta = Complex64::from_polar((theta / 2.0).sin(), phi);
    let input_state = QuantumState::dense(vec![alpha, beta]);

    println!("=== Quantum Teleportation ===\n");
    println!("Input state: {:.4}|0⟩ + {:.4}|1⟩", alpha, beta);

    // Deterministic verification for all 4 measurement outcomes
    println!("\nVerification (all 4 Alice measurement outcomes):\n");
    for m0 in [Bit::Zero, Bit::One] {
        for m1 in [Bit::Zero, Bit::One] {
            let f = verify_teleportation(&input_state, m0, m1);
            println!(
                "  Alice measures ({}, {}): fidelity = {:.6}",
                m0.is_one() as u8,
                m1.is_one() as u8,
                f
            );
            assert!(
                f > 0.9999,
                "Teleportation failed for ({}, {})",
                m0.is_one() as u8,
                m1.is_one() as u8
            );
        }
    }
    println!("\n✓ Teleportation verified for all outcomes!");

    // Shot-based execution to show probabilistic behavior
    let num_shots = 1000;
    println!("\nShot-based ({} shots):", num_shots);

    let backend = Backend::new(3, None);
    let (bit_regs, _, _) = backend
        .run_circuit(&build_shot_circuit(&input_state, num_shots))
        .unwrap();

    let mut counts: HashMap<String, u32> = HashMap::new();
    for shot in &bit_regs["ro"] {
        let key = format!("{}{}", shot[0] as u8, shot[1] as u8);
        *counts.entry(key).or_insert(0) += 1;
    }
    for (bits, count) in &counts {
        println!(
            "  Alice ({}, {}): {} ({:.0}%)",
            &bits[0..1],
            &bits[1..2],
            count,
            *count as f64 / num_shots as f64 * 100.0
        );
    }
    println!("  (All outcomes equally likely ≈ 25% each)");
}

/// Verify teleportation for a specific Alice measurement outcome.
///
/// Builds the circuit without measurement, projects onto Alice's outcome,
/// applies Bob's corrections, and checks Bob's qubit matches the input.
fn verify_teleportation(input: &QuantumState, m0: Bit, m1: Bit) -> f64 {
    // Build circuit: prepare |ψ⟩, create Bell pair, Alice's CNOT+H
    let mut circuit = Circuit::new();
    circuit += DefinitionComplex::new("sv".to_string(), 8, true);

    // Prepare input on qubit 0
    circuit += prepare_state(input);

    // Bell pair on qubits 1,2
    circuit += Hadamard::new(1);
    circuit += CNOT::new(1, 2);

    // Alice: CNOT(0,1) + H(0)
    circuit += CNOT::new(0, 1);
    circuit += Hadamard::new(0);

    // Get full 3-qubit state vector
    circuit += PragmaGetStateVector::new("sv".to_string(), None);

    let backend = Backend::new(3, None);
    let (_, _, complex_regs) = backend.run_circuit(&circuit).unwrap();
    let full_state = &complex_regs["sv"][0];

    // Project onto Alice's measurement outcome (m0, m1)
    // 3-qubit index: j = q0 + 2*q1 + 4*q2 (LSB-first)
    // Alice's qubits are 0 and 1. Bob's qubit is 2.
    let mut bob_amp_0 = Complex64::new(0.0, 0.0); // Bob's |0⟩
    let mut bob_amp_1 = Complex64::new(0.0, 0.0); // Bob's |1⟩

    for (j, &amp) in full_state.iter().enumerate() {
        let q0 = Bit::from_bool((j & 1) != 0);
        let q1 = Bit::from_bool((j >> 1) & 1 != 0);
        let q2_is_one = (j >> 2) & 1 != 0;

        if q0 == m0 && q1 == m1 {
            if !q2_is_one {
                bob_amp_0 += amp;
            } else {
                bob_amp_1 += amp;
            }
        }
    }

    // Normalize Bob's state (projection removes probability)
    let norm = (bob_amp_0.norm_sqr() + bob_amp_1.norm_sqr()).sqrt();
    bob_amp_0 /= norm;
    bob_amp_1 /= norm;

    // Apply Bob's corrections
    if m1.is_one() {
        // X gate: swap |0⟩ and |1⟩
        std::mem::swap(&mut bob_amp_0, &mut bob_amp_1);
    }
    if m0.is_one() {
        // Z gate: negate |1⟩
        bob_amp_1 = -bob_amp_1;
    }

    // Compare with input state
    let bob_state = QuantumState::dense(vec![bob_amp_0, bob_amp_1]);
    fidelity(input, &bob_state)
}

/// Build a shot-based circuit (for demonstrating probabilistic outcomes).
fn build_shot_circuit(input: &QuantumState, shots: usize) -> Circuit {
    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("ro".to_string(), 3, true);

    circuit += prepare_state(input);
    circuit += Hadamard::new(1);
    circuit += CNOT::new(1, 2);
    circuit += CNOT::new(0, 1);
    circuit += Hadamard::new(0);

    circuit += PragmaRepeatedMeasurement::new("ro".to_string(), shots, None);
    circuit
}
