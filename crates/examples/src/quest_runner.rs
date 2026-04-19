//! QuEST-based quantum circuit runner.

use algos::runner::{BitRegisters, QuantumRunner};
use roqoqo::backends::EvaluatingBackend;
use roqoqo::Circuit;
use roqoqo_quest::Backend;
use std::collections::HashMap;

/// Runs quantum circuits on the QuEST state-vector simulator.
///
/// Derives qubit count from the circuit via [`Circuit::number_of_qubits()`]
/// and executes each shot independently.
pub struct QuestRunner;

impl QuantumRunner for QuestRunner {
    fn run(&self, circuit: &Circuit, shots: usize) -> BitRegisters {
        let num_qubits = circuit.number_of_qubits();
        let backend = Backend::new(num_qubits, None);
        let mut combined: BitRegisters = HashMap::new();
        for _ in 0..shots {
            let (bits, _, _) = backend.run_circuit(circuit).expect("simulation failed");
            for (name, results) in bits {
                combined.entry(name).or_default().extend(results);
            }
        }
        combined
    }
}
