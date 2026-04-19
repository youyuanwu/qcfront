//! Grover's quantum search algorithm demo.
//!
//! Searches for a random target in N=8 (3 qubits) and demonstrates
//! multi-target search in N=4 (2 qubits).

use algos::grover::{search, search_with_oracle, GroverConfig, GroverOracle};
use roqoqo::backends::EvaluatingBackend;
use roqoqo_quest::Backend;

fn run_circuit(
    circuit: &roqoqo::Circuit,
    total_qubits: usize,
) -> std::collections::HashMap<String, Vec<Vec<bool>>> {
    let backend = Backend::new(total_qubits, None);
    let (bits, _, _) = backend.run_circuit(circuit).expect("simulation failed");
    bits
}

fn main() {
    println!("=== Grover's Search Algorithm ===\n");

    // --- Single-target search (N=8) ---
    let target = rand::random_range(0..8usize);
    println!("--- 3 qubits: searching N=8 for target={} ---", target);

    let config = GroverConfig {
        num_qubits: 3,
        num_shots: 100,
        ..Default::default()
    };
    let result = search(&config, target, run_circuit);

    println!(
        "  Found: {} (probability {:.1}%, {} iterations)",
        result.measured_state,
        result.probability * 100.0,
        result.num_iterations,
    );
    match result.success {
        Some(true) => println!("  ✓ Correct!"),
        Some(false) => println!("  ✗ Wrong (expected {})", target),
        None => println!("  ? Unknown"),
    }

    // --- Multi-target search (N=8, find one of {2, 5}) ---
    println!("\n--- 3 qubits: searching N=8 for targets={{2, 5}} ---");

    let oracle = GroverOracle::multi(3, &[2, 5]);
    let config_multi = GroverConfig {
        num_qubits: 3,
        num_shots: 100,
        ..Default::default()
    };
    let result_multi = search_with_oracle(&config_multi, &oracle, run_circuit);

    println!(
        "  Found: {} (probability {:.1}%, {} iterations)",
        result_multi.measured_state,
        result_multi.probability * 100.0,
        result_multi.num_iterations,
    );
    if result_multi.is_match(2) || result_multi.is_match(5) {
        println!("  ✓ Found a valid target!");
    } else {
        println!("  ✗ Missed (got {})", result_multi.measured_state);
    }

    // --- Show distribution for the single-target case ---
    println!("\n--- Measurement distribution (3-qubit) ---");
    let mut entries: Vec<_> = result.counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (state, count) in &entries {
        let bar: String = "█".repeat(**count);
        let marker = if **state == target { " ◄ target" } else { "" };
        println!("  |{:03b}⟩ = {:2}: {}{}", state, count, bar, marker);
    }
}
