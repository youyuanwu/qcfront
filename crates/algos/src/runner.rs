//! Backend abstraction for quantum circuit execution.
//!
//! The [`QuantumRunner`] trait decouples algorithm implementations from
//! specific backends (simulators, hardware). Algorithms accept
//! `&impl QuantumRunner` instead of raw closures.

use roqoqo::Circuit;
use std::collections::HashMap;

/// Result of measuring a single qubit: collapsed to |0⟩ or |1⟩.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bit {
    Zero,
    One,
}

impl Bit {
    /// Convert from a raw measurement bool (roqoqo convention: false=0, true=1).
    pub fn from_bool(b: bool) -> Self {
        if b {
            Bit::One
        } else {
            Bit::Zero
        }
    }

    /// Whether this is |1⟩.
    pub fn is_one(self) -> bool {
        self == Bit::One
    }

    /// Whether this is |0⟩.
    pub fn is_zero(self) -> bool {
        self == Bit::Zero
    }
}

impl From<bool> for Bit {
    fn from(b: bool) -> Self {
        Bit::from_bool(b)
    }
}

impl From<Bit> for bool {
    fn from(b: Bit) -> Self {
        b.is_one()
    }
}

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
