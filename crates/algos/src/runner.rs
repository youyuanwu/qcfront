//! Backend abstraction for quantum circuit execution.
//!
//! The [`QuantumRunner`] trait decouples algorithm implementations from
//! specific backends (simulators, hardware). Algorithms accept
//! `&impl QuantumRunner` instead of raw closures.

use roqoqo::Circuit;
use std::collections::HashMap;

/// Measurement results: register name → shots × bits.
pub type BitRegisters = HashMap<String, Vec<Vec<bool>>>;

/// A quantum circuit runner.
///
/// Implementations connect to simulators or hardware backends.
/// The runner handles backend construction, shot batching, and error
/// handling internally.
///
/// # Examples
///
/// Using a concrete runner:
/// ```ignore
/// let runner = QuestRunner;
/// let result = grover::search(&config, target, &runner);
/// ```
///
/// Using a closure (via blanket impl):
/// ```ignore
/// let result = grover::search(&config, target, &|circuit, shots| {
///     // run circuit for `shots` repetitions, return all results
/// });
/// ```
pub trait QuantumRunner {
    /// Execute a quantum circuit for the given number of shots.
    ///
    /// Returns all measurement results at once. Each register maps to
    /// a `Vec<Vec<bool>>` where the outer Vec has `shots` entries.
    ///
    /// Implementations derive the qubit count from the circuit via
    /// [`Circuit::number_of_qubits()`].
    ///
    /// # Arguments
    /// * `circuit` — the quantum circuit to execute
    /// * `shots` — number of measurement repetitions
    fn run(&self, circuit: &Circuit, shots: usize) -> BitRegisters;
}

/// Blanket implementation: any `Fn(&Circuit, usize) -> BitRegisters` is a runner.
impl<F> QuantumRunner for F
where
    F: Fn(&Circuit, usize) -> BitRegisters,
{
    fn run(&self, circuit: &Circuit, shots: usize) -> BitRegisters {
        self(circuit, shots)
    }
}
