//! Index-based oracle for Grover's algorithm.
//!
//! [`IndexOracle`] marks solution states by their integer index using
//! an X-MCZ-X circuit pattern.

use std::collections::HashSet;
use std::num::NonZeroUsize;

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::multi_cz::{build_multi_cz, required_ancillas};
use crate::circuits::transform;

use super::Oracle;

/// Oracle that marks solution states by index with a phase flip.
///
/// For each target, applies X gates to map the target to |11…1⟩, then
/// a multi-controlled-Z to flip the phase, then undoes the X gates.
/// Each target is marked independently — the X-MCZ-X pattern is selective
/// to exactly one computational basis state per target.
pub struct IndexOracle {
    targets: Vec<usize>,
    num_qubits: usize,
}

/// Backward-compatible alias for [`IndexOracle`].
#[deprecated(note = "renamed to IndexOracle")]
pub type GroverOracle = IndexOracle;

impl IndexOracle {
    /// Mark a single target state.
    ///
    /// # Panics
    /// If `target >= 2^num_qubits`.
    pub fn single(num_qubits: usize, target: usize) -> Self {
        assert!(
            target < (1 << num_qubits),
            "target {} out of range for {} qubits (max {})",
            target,
            num_qubits,
            (1 << num_qubits) - 1
        );
        Self {
            targets: vec![target],
            num_qubits,
        }
    }

    /// Mark multiple target states.
    ///
    /// Deduplicates via `HashSet` (prevents phase-flip cancellation from duplicates).
    ///
    /// # Panics
    /// - If `targets` is empty
    /// - If any target >= 2^num_qubits
    pub fn multi(num_qubits: usize, targets: &[usize]) -> Self {
        assert!(!targets.is_empty(), "targets must not be empty");
        let max_val = 1 << num_qubits;
        for &t in targets {
            assert!(
                t < max_val,
                "target {} out of range for {} qubits (max {})",
                t,
                num_qubits,
                max_val - 1
            );
        }
        let unique: HashSet<usize> = targets.iter().copied().collect();
        Self {
            targets: unique.into_iter().collect(),
            num_qubits,
        }
    }
}

impl Oracle for IndexOracle {
    fn num_data_qubits(&self) -> usize {
        self.num_qubits
    }

    fn num_ancillas(&self) -> usize {
        required_ancillas(self.num_qubits)
    }

    fn num_solutions(&self) -> Option<NonZeroUsize> {
        NonZeroUsize::new(self.targets.len())
    }

    fn apply(&self, circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]) {
        for &target in &self.targets {
            apply_target_oracle(circuit, data_qubits, ancillas, target);
        }
    }
}

/// Build the oracle sub-circuit that flips the phase of a single |target⟩.
///
/// The problem: MCZ only flips the phase of |11…1⟩ (all-ones state).
/// We need to flip the phase of an arbitrary basis state |target⟩.
///
/// The trick: temporarily remap |target⟩ to |11…1⟩ using X gates,
/// apply MCZ, then undo the X gates:
///
///   Example: target = 5 = |101⟩ (3 qubits, LSB-first: q₀=1, q₁=0, q₂=1)
///
///   Step 1 — X on zero-bits: flip q₁ (the only 0-bit)
///     |101⟩ → |111⟩   (target becomes all-ones)
///     |011⟩ → |001⟩   (other states move away from all-ones)
///
///   Step 2 — MCZ: flips phase of |111⟩ only
///     |111⟩ gets phase −1 (this was our target |101⟩)
///     all other states unchanged
///
///   Step 3 — undo X on q₁: restore original encoding
///     |111⟩ → |101⟩   (back to original, but now has phase −1)
///
/// This works because X is self-inverse (X² = I) and MCZ only sees
/// the remapped state. Each target is marked independently — the
/// X-MCZ-X pattern is selective to exactly one computational basis state.
fn apply_target_oracle(
    circuit: &mut Circuit,
    data_qubits: &[usize],
    ancillas: &[usize],
    target: usize,
) {
    // Compute: X on qubits where target bit is 0 (remap |target⟩ → |11…1⟩)
    let mut compute = Circuit::new();
    for (i, &q) in data_qubits.iter().enumerate() {
        if (target >> i) & 1 == 0 {
            compute += PauliX::new(q);
        }
    }

    // Action: MCZ flips phase of |11…1⟩ (which is |target⟩ after remapping)
    let mut action = Circuit::new();
    action += build_multi_cz(data_qubits, ancillas);

    // compute → action → inverse(compute)
    *circuit +=
        transform::within_apply(&compute, &action).expect("index oracle compute uses only X gates");
}
