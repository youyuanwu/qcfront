//! Subset-sum oracle for Grover's algorithm.
//!
//! [`SubsetSumOracle`] evaluates whether a subset of elements sums to a
//! target value, using a reversible controlled-adder circuit. Implements
//! [`Oracle`] directly — no classical pre-solving.
//!
//! [`verify_subset_sum`] provides classical verification of measured results,
//! analogous to [`crate::sat::evaluate_cnf`] for SAT.

use std::cmp::max;
use std::num::NonZeroUsize;

use roqoqo::operations::*;
use roqoqo::Circuit;

use crate::circuits::adder;
use crate::circuits::multi_cz;
use crate::circuits::transform;

use super::Oracle;

/// Circuit-based subset-sum oracle for Grover's algorithm.
///
/// Given elements S = {s₀, …, s_{n-1}} and a target sum T, marks the
/// states where the included elements sum exactly to T. Each data qubit
/// i represents "include element sᵢ".
///
/// The circuit uses a compute → phase-flip → uncompute pattern:
/// 1. Controlled-add each element into a sum accumulator register
/// 2. Phase-flip via X-MCZ-X equality check (sum == T)
/// 3. Reverse all additions (restore ancillas to |0⟩)
///
/// # Panics
/// - If `elements.len() < 2` (Grover requires n ≥ 2 data qubits)
/// - If all elements are zero (sum register would be empty)
/// - If `target == 0` (trivially solved by empty subset)
/// - If `target > sum of elements` (provably impossible)
pub struct SubsetSumOracle {
    elements: Vec<u64>,
    target: u64,
    /// Number of bits needed to represent the maximum possible sum.
    sum_bits: usize,
}

impl SubsetSumOracle {
    /// Build a SubsetSumOracle for a subset-sum instance.
    ///
    /// # Panics
    /// - If `elements.len() < 2` (Grover requires n ≥ 2 data qubits)
    /// - If all elements are zero (sum register would be empty)
    /// - If `target == 0` (trivially solved by empty subset)
    /// - If `target > sum of elements` (provably impossible)
    pub fn new(elements: &[u64], target: u64) -> Self {
        assert!(
            elements.len() >= 2,
            "need at least 2 elements, got {}",
            elements.len()
        );
        let total: u64 = elements.iter().sum();
        assert!(
            total > 0,
            "all elements are zero — sum register would be empty"
        );
        assert!(
            target > 0,
            "target must be > 0 (target=0 is trivially solved by empty subset)"
        );
        assert!(
            target <= total,
            "target {} exceeds sum of all elements {} — provably impossible",
            target,
            total
        );

        let sum_bits = bits_for(total);

        Self {
            elements: elements.to_vec(),
            target,
            sum_bits,
        }
    }
}

/// Number of bits needed to represent values 0..=max_val.
fn bits_for(max_val: u64) -> usize {
    if max_val == 0 {
        return 0;
    }
    64 - max_val.leading_zeros() as usize
}

impl Oracle for SubsetSumOracle {
    fn num_data_qubits(&self) -> usize {
        self.elements.len()
    }

    fn num_ancillas(&self) -> usize {
        let m = self.sum_bits;
        let adder_scratch = adder::required_scratch(m);
        let mcz_scratch = multi_cz::required_ancillas(m);
        // sum register + shared scratch pool (MCX and MCZ temporally disjoint)
        m + max(adder_scratch, mcz_scratch)
    }

    fn num_solutions(&self) -> Option<NonZeroUsize> {
        None // unknown — that's the whole point of quantum search
    }

    fn apply(&self, circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]) {
        // ===================================================================
        // Subset-sum oracle circuit
        // ===================================================================
        //
        // Evaluates f(x) = 1 iff Σ(sᵢ where xᵢ=1) == T on a superposition
        // of all 2^n inclusion/exclusion combinations. Each data qubit i
        // represents "include element sᵢ".
        //
        // ===================================================================
        // Circuit strategy: compute → phase-flip → uncompute
        // ===================================================================
        //
        // Compute: for each element, controlled_add(data[i], sum, scratch, sᵢ)
        //   accumulates the sum of selected elements into the sum register.
        //   The MCX-cascade adder adds a classical constant k controlled by
        //   one qubit — no carry register needed.
        //
        // Action: X-MCZ-X equality check on the sum register.
        //   X on zero-bits of T maps |T⟩ → |11…1⟩, MCZ flips phase of
        //   |11…1⟩ only, then X undoes the mapping. Net effect: phase flip
        //   iff sum == T. The X gates self-cancel so sum register is unchanged.
        //
        // Uncompute: automatic via within_apply — reverses all controlled
        //   additions, restoring the sum register to |0⟩.
        //
        // ===================================================================
        // Ancilla layout
        // ===================================================================
        //
        //   [sum_0, sum_1, ..., sum_{m-1}, scratch...]
        //   ├── sum accumulator register ──┤├─ shared ──┤
        //
        // The scratch pool is reused between controlled additions (MCX
        // decomposition) and the equality comparator (MCZ decomposition).
        // Temporal disjointness: all additions finish before the equality
        // check runs. Each controlled_add fully uncomputes its MCX scratch.

        let m = self.sum_bits;
        let sum_qubits = &ancillas[..m];
        let scratch = &ancillas[m..];

        // --- Compute: accumulate sum of selected elements ---
        let mut compute = Circuit::new();
        for (i, &elem) in self.elements.iter().enumerate() {
            if elem == 0 {
                continue;
            }
            let adder_scratch_needed = adder::required_scratch(m);
            let adder_scratch = &scratch[..adder_scratch_needed];
            adder::controlled_add(
                &mut compute,
                data_qubits[i],
                sum_qubits,
                adder_scratch,
                elem,
            );
        }

        // --- Action: X-MCZ-X equality check (sum == T) ---
        let mut action = Circuit::new();
        for (j, &sq) in sum_qubits.iter().enumerate() {
            if (self.target >> j) & 1 == 0 {
                action += PauliX::new(sq);
            }
        }
        let mcz_scratch_needed = multi_cz::required_ancillas(m);
        let mcz_scratch = &scratch[..mcz_scratch_needed];
        action += multi_cz::build_multi_cz(sum_qubits, mcz_scratch);
        for (j, &sq) in sum_qubits.iter().enumerate() {
            if (self.target >> j) & 1 == 0 {
                action += PauliX::new(sq);
            }
        }

        // compute → action → inverse(compute)
        *circuit += transform::within_apply(&compute, &action)
            .expect("subset sum compute circuit uses only supported gates");
    }
}

/// Classical verification of a measured subset-sum result.
///
/// Returns the selected elements if `state` encodes a valid solution
/// (i.e., the included elements sum to `target`), or `None` otherwise.
///
/// # Arguments
/// * `elements` — the set of integers
/// * `target` — the target sum
/// * `state` — measured state (LSB-first bitmask: bit i = include element i)
pub fn verify_subset_sum(elements: &[u64], target: u64, state: usize) -> Option<Vec<u64>> {
    let mut selected = Vec::new();
    let mut sum = 0u64;
    for (i, &elem) in elements.iter().enumerate() {
        if (state >> i) & 1 == 1 {
            selected.push(elem);
            sum += elem;
        }
    }
    if sum == target {
        Some(selected)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grover::{try_search_with_oracle, GroverConfig, Oracle};
    use crate::runner::BitRegisters;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;
    use std::collections::HashMap;

    fn test_runner(circuit: &Circuit, shots: usize) -> BitRegisters {
        let num_qubits = circuit.number_of_qubits();
        let backend = Backend::new(num_qubits, None);
        let mut combined: BitRegisters = HashMap::new();
        for _ in 0..shots {
            let (bits, _, _) = backend.run_circuit(circuit).unwrap();
            for (name, results) in bits {
                combined.entry(name).or_default().extend(results);
            }
        }
        combined
    }

    // --- Constructor validation ---

    #[test]
    #[should_panic(expected = "need at least 2 elements")]
    fn test_new_single_element() {
        SubsetSumOracle::new(&[5], 5);
    }

    #[test]
    #[should_panic(expected = "all elements are zero")]
    fn test_new_all_zeros() {
        SubsetSumOracle::new(&[0, 0], 1);
    }

    #[test]
    #[should_panic(expected = "target must be > 0")]
    fn test_new_target_zero() {
        SubsetSumOracle::new(&[3, 5], 0);
    }

    #[test]
    #[should_panic(expected = "provably impossible")]
    fn test_new_target_too_large() {
        SubsetSumOracle::new(&[3, 5], 9);
    }

    // --- Ancilla budget ---

    #[test]
    fn test_ancilla_budget() {
        // S = {3, 5, 7}, T = 8: max_sum = 15, m = 4
        let oracle = SubsetSumOracle::new(&[3, 5, 7], 8);
        assert_eq!(oracle.num_data_qubits(), 3);
        let m = 4;
        let expected = m + max(adder::required_scratch(m), multi_cz::required_ancillas(m));
        assert_eq!(oracle.num_ancillas(), expected);
        assert_eq!(expected, 6); // 4 + max(2, 2) = 6
    }

    #[test]
    fn test_ancilla_budget_small() {
        // S = {1, 2}, T = 3: max_sum = 3, m = 2
        let oracle = SubsetSumOracle::new(&[1, 2], 3);
        assert_eq!(oracle.num_data_qubits(), 2);
        // m=2, adder scratch = required_ancillas(2) = 0, mcz scratch = 0
        assert_eq!(oracle.num_ancillas(), 2); // just sum register
    }

    // --- verify_subset_sum ---

    #[test]
    fn test_verify_valid() {
        // S = {3, 5, 7}, state = 0b011 = include s₀=3, s₁=5
        let result = verify_subset_sum(&[3, 5, 7], 8, 0b011);
        assert_eq!(result, Some(vec![3, 5]));
    }

    #[test]
    fn test_verify_invalid() {
        // S = {3, 5, 7}, state = 0b001 = include s₀=3 only → sum = 3 ≠ 8
        let result = verify_subset_sum(&[3, 5, 7], 8, 0b001);
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_all_included() {
        // S = {3, 5, 7}, state = 0b111 → sum = 15
        let result = verify_subset_sum(&[3, 5, 7], 15, 0b111);
        assert_eq!(result, Some(vec![3, 5, 7]));
    }

    // --- Grover integration ---

    #[test]
    fn test_grover_3_5_target_8() {
        // S = {3, 5}, T = 8: only solution is include both (state = 0b11 = 3)
        let oracle = SubsetSumOracle::new(&[3, 5], 8);
        let config = GroverConfig {
            num_qubits: oracle.num_data_qubits(),
            num_shots: 100,
            num_iterations: Some(1), // √(4/1) ≈ 1.57 → 1
        };
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert_eq!(result.measured_state, 0b11); // include both
        assert!(
            verify_subset_sum(&[3, 5], 8, result.measured_state).is_some(),
            "measured state should be a valid solution"
        );
    }

    #[test]
    fn test_grover_3_5_7_target_8() {
        // S = {3, 5, 7}, T = 8: unique solution is {3, 5} → state = 0b011
        let oracle = SubsetSumOracle::new(&[3, 5, 7], 8);
        let config = GroverConfig {
            num_qubits: oracle.num_data_qubits(),
            num_shots: 100,
            num_iterations: Some(2), // ⌊π/4 · √(8/1)⌋ = 2
        };
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert_eq!(result.measured_state, 0b011);
        let selected = verify_subset_sum(&[3, 5, 7], 8, result.measured_state);
        assert_eq!(selected, Some(vec![3, 5]));
    }

    #[test]
    fn test_grover_multi_solution() {
        // S = {2, 3, 5}, T = 5: solutions are {5} (state=0b100=4)
        // and {2, 3} (state=0b011=3). Two solutions out of 8 → k = ⌊π/4·√(8/2)⌋ = 1
        let oracle = SubsetSumOracle::new(&[2, 3, 5], 5);
        let config = GroverConfig {
            num_qubits: oracle.num_data_qubits(),
            num_shots: 100,
            num_iterations: Some(1),
        };
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert!(
            verify_subset_sum(&[2, 3, 5], 5, result.measured_state).is_some(),
            "measured state {} should be a valid solution",
            result.measured_state
        );
    }

    #[test]
    fn test_grover_target_equals_total() {
        // S = {1, 2}, T = 3: only solution is include all (state = 0b11)
        let oracle = SubsetSumOracle::new(&[1, 2], 3);
        let config = GroverConfig {
            num_qubits: oracle.num_data_qubits(),
            num_shots: 100,
            num_iterations: Some(1),
        };
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert_eq!(result.measured_state, 0b11);
    }

    #[test]
    fn test_grover_duplicates() {
        // S = {3, 3, 5}, T = 3: two solutions — include first only (001=1) or
        // second only (010=2). 2 solutions out of 8 → k = ⌊π/4·√(8/2)⌋ = 1
        let oracle = SubsetSumOracle::new(&[3, 3, 5], 3);
        let config = GroverConfig {
            num_qubits: oracle.num_data_qubits(),
            num_shots: 100,
            num_iterations: Some(1),
        };
        let result = try_search_with_oracle(&config, &oracle, &test_runner).unwrap();
        assert!(
            verify_subset_sum(&[3, 3, 5], 3, result.measured_state).is_some(),
            "state {} should solve subset sum",
            result.measured_state
        );
    }

    // --- bits_for ---

    #[test]
    fn test_bits_for() {
        assert_eq!(bits_for(0), 0);
        assert_eq!(bits_for(1), 1);
        assert_eq!(bits_for(2), 2);
        assert_eq!(bits_for(3), 2);
        assert_eq!(bits_for(7), 3);
        assert_eq!(bits_for(8), 4);
        assert_eq!(bits_for(15), 4);
        assert_eq!(bits_for(16), 5);
    }
}
