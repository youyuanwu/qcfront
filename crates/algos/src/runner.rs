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

// ---------------------------------------------------------------------------
// Counts — measurement histogram
// ---------------------------------------------------------------------------

/// Measurement histogram: maps decoded basis states to their shot counts.
///
/// Replaces the manual pattern of iterating `BitRegisters`, decoding
/// bit-vectors to integers, and computing frequencies.
///
/// # Example
///
/// ```ignore
/// let bit_registers = runner.run(&circuit, 100);
/// let counts = Counts::from_register(&bit_registers, "result", 3);
/// let (state, count) = counts.most_frequent();
/// let p = counts.probability(state);
/// ```
#[derive(Debug, Clone)]
pub struct Counts {
    histogram: HashMap<usize, usize>,
    total: usize,
}

impl Counts {
    /// Build from a `BitRegisters` measurement result.
    ///
    /// Decodes bit-vectors from the named register into integer states
    /// using LSB-first ordering (qubit 0 = bit 0), taking the first
    /// `num_qubits` bits from each shot.
    ///
    /// Returns an empty `Counts` if the register name is not found.
    pub fn from_register(registers: &BitRegisters, name: &str, num_qubits: usize) -> Self {
        let mut histogram = HashMap::new();
        let mut total = 0;

        if let Some(shots) = registers.get(name) {
            for bits in shots {
                let state = bits_to_state(bits, num_qubits);
                *histogram.entry(state).or_insert(0) += 1;
                total += 1;
            }
        }

        Self { histogram, total }
    }

    /// Build directly from a state→count map.
    pub fn from_map(histogram: HashMap<usize, usize>) -> Self {
        let total = histogram.values().sum();
        Self { histogram, total }
    }

    /// Total number of shots.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Number of distinct states observed.
    pub fn num_states(&self) -> usize {
        self.histogram.len()
    }

    /// Count for a specific state. Returns 0 if not observed.
    pub fn count(&self, state: usize) -> usize {
        self.histogram.get(&state).copied().unwrap_or(0)
    }

    /// Probability of a specific state (count / total).
    /// Returns 0.0 if total is 0.
    pub fn probability(&self, state: usize) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.count(state) as f64 / self.total as f64
    }

    /// Most frequently measured state and its count.
    /// Returns `(0, 0)` if no measurements.
    pub fn most_frequent(&self) -> (usize, usize) {
        self.histogram
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&state, &count)| (state, count))
            .unwrap_or((0, 0))
    }

    /// Iterate over (state, count) pairs sorted by count descending.
    pub fn sorted(&self) -> Vec<(usize, usize)> {
        let mut entries: Vec<(usize, usize)> =
            self.histogram.iter().map(|(&s, &c)| (s, c)).collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        entries
    }

    /// Reference to the underlying histogram.
    pub fn as_map(&self) -> &HashMap<usize, usize> {
        &self.histogram
    }
}

/// Decode a bit-vector to an integer (LSB-first).
fn bits_to_state(bits: &[bool], num_qubits: usize) -> usize {
    let mut state = 0;
    for (i, &bit) in bits.iter().enumerate().take(num_qubits) {
        if bit {
            state |= 1 << i;
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registers(name: &str, shots: Vec<Vec<bool>>) -> BitRegisters {
        let mut regs = HashMap::new();
        regs.insert(name.to_string(), shots);
        regs
    }

    #[test]
    fn test_counts_from_register() {
        // 3-qubit register, 5 shots: states 5, 5, 3, 5, 0
        let regs = make_registers(
            "r",
            vec![
                vec![true, false, true],   // 5
                vec![true, false, true],   // 5
                vec![true, true, false],   // 3
                vec![true, false, true],   // 5
                vec![false, false, false], // 0
            ],
        );
        let counts = Counts::from_register(&regs, "r", 3);

        assert_eq!(counts.total(), 5);
        assert_eq!(counts.count(5), 3);
        assert_eq!(counts.count(3), 1);
        assert_eq!(counts.count(0), 1);
        assert_eq!(counts.count(7), 0); // not observed
        assert_eq!(counts.num_states(), 3);
    }

    #[test]
    fn test_counts_most_frequent() {
        let regs = make_registers(
            "r",
            vec![
                vec![true, true],  // 3
                vec![true, true],  // 3
                vec![false, true], // 2
            ],
        );
        let counts = Counts::from_register(&regs, "r", 2);
        let (state, count) = counts.most_frequent();
        assert_eq!(state, 3);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_counts_probability() {
        let regs = make_registers(
            "r",
            vec![
                vec![true],  // 1
                vec![true],  // 1
                vec![false], // 0
                vec![true],  // 1
            ],
        );
        let counts = Counts::from_register(&regs, "r", 1);
        assert!((counts.probability(1) - 0.75).abs() < 1e-10);
        assert!((counts.probability(0) - 0.25).abs() < 1e-10);
        assert!((counts.probability(2) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_counts_sorted() {
        let regs = make_registers(
            "r",
            vec![
                vec![true, false], // 1
                vec![false, true], // 2
                vec![true, false], // 1
                vec![false, true], // 2
                vec![false, true], // 2
                vec![true, true],  // 3
            ],
        );
        let counts = Counts::from_register(&regs, "r", 2);
        let sorted = counts.sorted();
        assert_eq!(sorted[0], (2, 3)); // most frequent
        assert_eq!(sorted[1], (1, 2));
        assert_eq!(sorted[2], (3, 1));
    }

    #[test]
    fn test_counts_empty_register() {
        let regs: BitRegisters = HashMap::new();
        let counts = Counts::from_register(&regs, "missing", 3);
        assert_eq!(counts.total(), 0);
        assert_eq!(counts.most_frequent(), (0, 0));
        assert!((counts.probability(0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_counts_from_map() {
        let mut map = HashMap::new();
        map.insert(5, 10);
        map.insert(3, 5);
        let counts = Counts::from_map(map);
        assert_eq!(counts.total(), 15);
        assert_eq!(counts.most_frequent(), (5, 10));
    }
}
