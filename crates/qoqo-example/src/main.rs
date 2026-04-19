use roqoqo::backends::EvaluatingBackend;
use roqoqo::operations::*;
use roqoqo::Circuit;
use roqoqo_quest::Backend;

const NUM_SHOTS: usize = 1000;

fn main() {
    // Build a Bell state circuit: H(0) → CNOT(0,1) → Measure
    let mut circuit = Circuit::new();

    // Define a classical bit register "ro" of length 2 for measurement output
    circuit += DefinitionBit::new("ro".to_string(), 2, true);

    // Create Bell state: |00⟩ → |Φ+⟩ = (|00⟩ + |11⟩) / √2
    circuit += Hadamard::new(0);
    circuit += CNOT::new(0, 1);

    // Measure all qubits 1000 times
    circuit += PragmaRepeatedMeasurement::new("ro".to_string(), NUM_SHOTS, None);

    println!("Circuit:");
    println!("{}", circuit);

    // Create QuEST backend with 2 qubits
    let backend = Backend::new(2, None);

    // Run the circuit
    let (bit_registers, _float_registers, _complex_registers) =
        backend.run_circuit(&circuit).expect("Simulation failed");

    // Count measurement outcomes
    if let Some(results) = bit_registers.get("ro") {
        let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for shot in results {
            let key = format!("{}{}", shot[0] as u8, shot[1] as u8);
            *counts.entry(key).or_insert(0u32) += 1;
        }
        println!("\nMeasurement results ({NUM_SHOTS} shots):");
        for (state, count) in &counts {
            println!(
                "  |{}⟩: {} ({:.1}%)",
                state,
                count,
                *count as f64 / NUM_SHOTS as f64 * 100.0
            );
        }
    }
}
