// Bell State
//
// Prepares the Bell state |Φ+⟩ = (|00⟩ + |11⟩)/√2 and measures it.
// Expected output: ~50% |00⟩, ~50% |11⟩ — no |01⟩ or |10⟩.

use roqoqo::backends::EvaluatingBackend;
use roqoqo::operations::*;
use roqoqo::Circuit;
use roqoqo_quest::Backend;
use std::collections::HashMap;

const NUM_SHOTS: usize = 1000;

fn main() {
    let mut circuit = Circuit::new();

    // Classical bit register for measurement output
    circuit += DefinitionBit::new("ro".to_string(), 2, true);

    // Create Bell state: |00⟩ → |Φ+⟩ = (|00⟩ + |11⟩) / √2
    circuit += Hadamard::new(0);
    circuit += CNOT::new(0, 1);

    // Measure both qubits
    circuit += PragmaRepeatedMeasurement::new("ro".to_string(), NUM_SHOTS, None);

    println!("Circuit:");
    println!("{}", circuit);

    // Simulate with QuEST backend
    let backend = Backend::new(2, None);
    let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("Simulation failed");

    // Count measurement outcomes
    if let Some(results) = bit_registers.get("ro") {
        let mut counts: HashMap<String, u32> = HashMap::new();
        for shot in results {
            let key = format!("{}{}", shot[0] as u8, shot[1] as u8);
            *counts.entry(key).or_insert(0) += 1;
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
