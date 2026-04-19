//! Solve a SAT problem using Grover's quantum search.
//!
//! Demonstrates the full pipeline: CNF formula → SAT oracle → Grover search
//! → satisfying assignment.

use algos::grover::{search_with_oracle, GroverConfig};
use algos::sat::{sat_oracle, Literal};
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
    println!("=== Quantum SAT Solver (Grover's Algorithm) ===\n");

    // Define CNF formula: (x₁) AND (x₂ OR x₃) AND (¬x₂ OR x₃)
    let clauses = vec![
        vec![Literal(1)],              // x₁ must be true
        vec![Literal(2), Literal(3)],  // x₂ OR x₃
        vec![Literal(-2), Literal(3)], // ¬x₂ OR x₃
    ];
    let num_vars = 3;

    println!("Formula: (x₁) ∧ (x₂ ∨ x₃) ∧ (¬x₂ ∨ x₃)");
    println!("Variables: {}, Search space: {}\n", num_vars, 1 << num_vars);

    // Build SAT oracle and find solutions classically (for verification)
    let sat = sat_oracle(num_vars, &clauses);
    let grover_oracle = sat.to_grover_oracle();
    let m = grover_oracle.num_solutions();

    println!("Classical analysis: {} satisfying assignment(s)", m);
    for i in 0..(1 << num_vars) {
        if sat.evaluate(i) {
            let bits: String = (0..num_vars)
                .map(|b| if (i >> b) & 1 == 1 { '1' } else { '0' })
                .collect();
            println!("  x₁x₂x₃ = {} (decimal {})", bits, i);
        }
    }

    // Run Grover's search
    println!("\nRunning Grover's search...");
    let config = GroverConfig {
        num_qubits: num_vars,
        num_shots: 100,
        ..Default::default()
    };
    let result = search_with_oracle(&config, &grover_oracle, run_circuit);

    let bits: String = (0..num_vars)
        .map(|b| {
            if (result.measured_state >> b) & 1 == 1 {
                '1'
            } else {
                '0'
            }
        })
        .collect();

    println!(
        "  Found: x₁x₂x₃ = {} (decimal {}, probability {:.1}%, {} iterations)",
        bits,
        result.measured_state,
        result.probability * 100.0,
        result.num_iterations,
    );

    if sat.evaluate(result.measured_state) {
        println!("  ✓ Satisfying assignment found!");
    } else {
        println!("  ✗ Not a satisfying assignment");
    }

    // Show measurement distribution
    println!("\n--- Measurement distribution ---");
    let mut entries: Vec<_> = result.counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (state, count) in &entries {
        let label: String = (0..num_vars)
            .map(|b| if (*state >> b) & 1 == 1 { '1' } else { '0' })
            .collect();
        let bar: String = "█".repeat(**count);
        let marker = if sat.evaluate(**state) { " ✓" } else { "" };
        println!("  |{}⟩ = {:2}: {}{}", label, count, bar, marker);
    }
}
