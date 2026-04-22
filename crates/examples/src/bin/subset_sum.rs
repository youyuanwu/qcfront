//! Solve a subset-sum problem using Grover's quantum search.
//!
//! Demonstrates the full pipeline: elements + target → SubsetSumOracle
//! (reversible controlled-adder circuit) → Grover search → selected subset.
//! Uses `verify_subset_sum` for classical verification.

use algos::grover::{
    try_search_with_oracle, verify_subset_sum, GroverConfig, Oracle, SubsetSumOracle,
};
use examples::quest_runner::QuestRunner;

fn main() {
    println!("=== Quantum Subset Sum Solver (Grover's Algorithm) ===\n");

    let elements = vec![2, 3, 5, 7];
    let target = 10;

    println!(
        "Elements: {:?}, Target: {}, Search space: {}",
        elements,
        target,
        1u64 << elements.len()
    );

    // Classical brute-force for comparison
    let n = elements.len();
    let mut solutions = Vec::new();
    for state in 0..(1 << n) {
        if verify_subset_sum(&elements, target, state).is_some() {
            solutions.push(state);
        }
    }
    println!("Classical analysis: {} solution(s)", solutions.len());
    for &state in &solutions {
        let selected = verify_subset_sum(&elements, target, state).unwrap();
        let bits: String = (0..n)
            .map(|b| if (state >> b) & 1 == 1 { '1' } else { '0' })
            .collect();
        println!("  |{}⟩ = {:?} (sum = {})", bits, selected, target);
    }

    // Build quantum oracle
    let oracle = SubsetSumOracle::new(&elements, target);
    println!(
        "\nOracle: {} data qubits, {} ancillas",
        oracle.num_data_qubits(),
        oracle.num_ancillas()
    );

    // Run Grover's search
    println!("Running Grover's search...");
    let runner = QuestRunner;
    let config = GroverConfig {
        num_qubits: oracle.num_data_qubits(),
        num_iterations: Some(2), // ⌊π/4 · √(16/2)⌋ = 2
        num_shots: 100,
    };
    let result = try_search_with_oracle(&config, &oracle, &runner).unwrap();

    let bits: String = (0..n)
        .map(|b| {
            if (result.measured_state >> b) & 1 == 1 {
                '1'
            } else {
                '0'
            }
        })
        .collect();

    println!(
        "  Found: |{}⟩ (state {}, probability {:.1}%, {} iterations)",
        bits,
        result.measured_state,
        result.probability * 100.0,
        result.num_iterations,
    );

    if let Some(selected) = verify_subset_sum(&elements, target, result.measured_state) {
        println!("  ✓ Valid subset: {:?}, sum = {}", selected, target);
    } else {
        println!("  ✗ Not a valid solution");
    }

    // Measurement distribution
    println!("\n--- Measurement distribution ---");
    let mut entries: Vec<_> = result.counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (state, count) in &entries {
        let label: String = (0..n)
            .map(|b| if (*state >> b) & 1 == 1 { '1' } else { '0' })
            .collect();
        let bar: String = "█".repeat(**count);
        let marker = if verify_subset_sum(&elements, target, **state).is_some() {
            " ✓"
        } else {
            ""
        };
        println!("  |{}⟩ = {:2}: {}{}", label, count, bar, marker);
    }
}
