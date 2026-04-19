// Bell Inequality (CHSH) Violation
//
// Demonstrates that quantum mechanics violates the CHSH inequality:
//   |S| = |<QS> + <QT> + <RS> - <RT>| <= 2  (classical bound)
//
// Quantum mechanics predicts |S| = 2√2 ≈ 2.828 for a Bell singlet state,
// violating the classical bound and proving entanglement is "real."
//
// Reference: qpp/examples/bell_inequalities.cpp
//
// CHSH Setup:
//   Alice measures in bases Q (Z) or R (X)
//   Bob measures in bases S ((Z+X)/√2) or T ((Z-X)/√2)
//
// We prepare a Bell singlet |Ψ⁻⟩ = (|01⟩ − |10⟩)/√2
// and run N shots for each of the 4 measurement setting combinations.
//
// Circuit approach: instead of changing the measurement basis, we rotate
// the qubits so that measuring in the Z basis is equivalent to measuring
// in the desired basis. For operator A(θ) = cos(θ)Z + sin(θ)X,
// the rotation is Ry(-θ) before Z-measurement.

use qoqo_calculator::CalculatorFloat;
use roqoqo::backends::EvaluatingBackend;
use roqoqo::operations::*;
use roqoqo::Circuit;
use roqoqo_quest::Backend;
use std::f64::consts::PI;

const NUM_SHOTS: usize = 10000;

/// Measurement settings for Alice and Bob.
struct MeasurementSetting {
    name: &'static str,
    alice_angle: f64,
    bob_angle: f64,
}

/// Build a circuit that:
/// 1. Prepares the Bell singlet |Ψ⁻⟩ = (|01⟩ − |10⟩)/√2
/// 2. Rotates Alice's qubit (0) by alice_angle
/// 3. Rotates Bob's qubit (1) by bob_angle
/// 4. Measures both qubits NUM_SHOTS times
fn build_chsh_circuit(alice_angle: f64, bob_angle: f64) -> Circuit {
    let mut circuit = Circuit::new();

    // Classical register for measurement results
    circuit += DefinitionBit::new("ro".to_string(), 2, true);

    // Prepare Bell singlet |Ψ⁻⟩ = (|01⟩ − |10⟩)/√2
    // X(0) → H(0) → CNOT(0,1) → Z(0)
    circuit += PauliX::new(0);
    circuit += Hadamard::new(0);
    circuit += CNOT::new(0, 1);
    circuit += PauliZ::new(0);

    // Rotate to measurement basis: Ry(-θ) then measure in Z
    if alice_angle.abs() > 1e-10 {
        circuit += RotateY::new(0, CalculatorFloat::Float(-alice_angle));
    }
    if bob_angle.abs() > 1e-10 {
        circuit += RotateY::new(1, CalculatorFloat::Float(-bob_angle));
    }

    // Measure both qubits
    circuit += PragmaRepeatedMeasurement::new("ro".to_string(), NUM_SHOTS, None);

    circuit
}

/// Convert measurement bit to eigenvalue: |0⟩ → +1, |1⟩ → −1
fn bit_to_eigenvalue(bit: bool) -> f64 {
    if bit {
        -1.0
    } else {
        1.0
    }
}

fn main() {
    // CHSH measurement angles for maximum violation with Bell singlet:
    //   For |Ψ⁻⟩, correlation is E(a,b) = -cos(a - b).
    //   Alice: Q → θ=0 (Z), R → θ=π/2 (X)
    //   Bob:   S → θ=π/4 ((Z+X)/√2), T → θ=-π/4 ((Z-X)/√2)
    //   S = E(QS) + E(QT) + E(RS) - E(RT) = -2√2
    let settings = [
        MeasurementSetting {
            name: "QS",
            alice_angle: 0.0,
            bob_angle: PI / 4.0,
        },
        MeasurementSetting {
            name: "QT",
            alice_angle: 0.0,
            bob_angle: -PI / 4.0,
        },
        MeasurementSetting {
            name: "RS",
            alice_angle: PI / 2.0,
            bob_angle: PI / 4.0,
        },
        MeasurementSetting {
            name: "RT",
            alice_angle: PI / 2.0,
            bob_angle: -PI / 4.0,
        },
    ];

    println!("CHSH Bell Inequality Violation Test");
    println!("===================================");
    println!("N = {} experiments per setting\n", NUM_SHOTS);
    println!(
        "{:<4} {:>8} {:>8} {:>8} {:>8}   {:>8}",
        "    ", "N(++)", "N(+-)", "N(-+)", "N(--)  ", "<AB>"
    );

    let mut correlations = Vec::new();

    for setting in &settings {
        let circuit = build_chsh_circuit(setting.alice_angle, setting.bob_angle);
        let backend = Backend::new(2, None);
        let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("Simulation failed");

        let results = bit_registers.get("ro").expect("No measurement results");

        // Count correlations
        let mut n_pp: usize = 0; // +1, +1
        let mut n_pm: usize = 0; // +1, -1
        let mut n_mp: usize = 0; // -1, +1
        let mut n_mm: usize = 0; // -1, -1

        for shot in results {
            let a = bit_to_eigenvalue(shot[0]);
            let b = bit_to_eigenvalue(shot[1]);
            match (a > 0.0, b > 0.0) {
                (true, true) => n_pp += 1,
                (true, false) => n_pm += 1,
                (false, true) => n_mp += 1,
                (false, false) => n_mm += 1,
            }
        }

        // Correlation: E = (N++ + N-- - N+- - N-+) / N
        let e = (n_pp + n_mm) as f64 - (n_pm + n_mp) as f64;
        let avg = e / NUM_SHOTS as f64;
        correlations.push((setting.name, avg));

        println!(
            "{:<4} {:>8} {:>8} {:>8} {:>8}   {:>+8.4}",
            setting.name, n_pp, n_pm, n_mp, n_mm, avg
        );
    }

    // CHSH value: S = <QS> + <QT> + <RS> - <RT>
    let s_chsh = correlations[0].1 + correlations[1].1 + correlations[2].1 - correlations[3].1;

    println!("\n--- Results ---");
    println!(
        "Experimental CHSH value:  S = <QS> + <QT> + <RS> - <RT> = {:.4}",
        s_chsh
    );
    println!(
        "Theoretical  CHSH value:  S = 2√2 ≈ {:.4}",
        2.0 * 2.0_f64.sqrt()
    );
    println!("Classical bound:          |S| ≤ 2");
    println!();

    if s_chsh.abs() > 2.0 {
        println!("✓ CHSH INEQUALITY VIOLATED! (|{:.4}| > 2)", s_chsh);
        println!("  This proves the correlations cannot be explained by local hidden variables.");
    } else {
        println!("✗ No violation detected (increase NUM_SHOTS or check circuit).");
    }
}
