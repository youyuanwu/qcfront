//! Solve a SAT problem using Grover's quantum search.
//!
//! Demonstrates the full pipeline: CNF formula → CnfOracle (reversible
//! circuit evaluation) → Grover search → satisfying assignment.
//! Uses SatOracle.evaluate() for classical verification only.

use algos::grover::{try_search_with_oracle, GroverConfig};
use algos::sat::{evaluate_cnf, CnfOracle, Literal};
use examples::quest_runner::QuestRunner;

fn main() {
    println!("=== Quantum SAT Solver (Grover's Algorithm) ===\n");

    // Define CNF formula: (x₁) AND (x₂ OR x₃) AND (¬x₂ OR x₃)
    let clauses = vec![
        vec![Literal::pos(1)],                  // x₁ must be true
        vec![Literal::pos(2), Literal::pos(3)], // x₂ OR x₃
        vec![Literal::neg(2), Literal::pos(3)], // ¬x₂ OR x₃
    ];
    let num_vars = 3;

    println!("Formula: (x₁) ∧ (x₂ ∨ x₃) ∧ (¬x₂ ∨ x₃)");
    println!("Variables: {}, Search space: {}\n", num_vars, 1 << num_vars);

    // Classical verification
    let num_solutions = (0..(1 << num_vars))
        .filter(|&i| evaluate_cnf(&clauses, i))
        .count();
    println!(
        "Classical analysis: {} satisfying assignment(s)",
        num_solutions
    );
    for i in 0..(1 << num_vars) {
        if evaluate_cnf(&clauses, i) {
            let bits: String = (0..num_vars)
                .map(|b| if (i >> b) & 1 == 1 { '1' } else { '0' })
                .collect();
            println!("  x₁x₂x₃ = {} (decimal {})", bits, i);
        }
    }

    // Build circuit-based oracle (no classical pre-solving!)
    let cnf_oracle = CnfOracle::new(num_vars, &clauses);

    // Run Grover's search — must provide iteration count since CnfOracle
    // doesn't know M (that's the whole point of quantum search)
    println!("\nRunning Grover's search (CnfOracle, circuit-based)...");
    let runner = QuestRunner;
    let config = GroverConfig {
        num_qubits: num_vars,
        num_iterations: Some(1), // k=1 works well for M=2, N=8
        num_shots: 100,
    };
    let result = try_search_with_oracle(&config, &cnf_oracle, &runner).unwrap();

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

    if evaluate_cnf(&clauses, result.measured_state) {
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
        let marker = if evaluate_cnf(&clauses, **state) {
            " ✓"
        } else {
            ""
        };
        println!("  |{}⟩ = {:2}: {}{}", label, count, bar, marker);
    }
}
