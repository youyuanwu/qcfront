//! Run a Bell state on Azure Quantum's IonQ simulator.
//!
//! Demonstrates the full pipeline: roqoqo circuit → OpenQASM → az CLI → IonQ.
//!
//! Prerequisites:
//!   1. `az login` (authenticated)
//!   2. Azure Quantum workspace with IonQ provider
//!
//! Usage:
//!   cargo run -p examples --bin azure_bell -- --workspace <ws> --resource-group <rg>

use algos::runner::QuantumRunner;
use examples::azure_runner::AzureCliRunner;
use roqoqo::operations::*;
use roqoqo::Circuit;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let workspace = get_arg(&args, "--workspace").unwrap_or_else(|| "qcfront-ws".to_string());
    let rg = get_arg(&args, "--resource-group").unwrap_or_else(|| "qcfront-rg".to_string());
    let target = get_arg(&args, "--target").unwrap_or_else(|| "quantinuum.sim.h2-1e".to_string());
    let shots: usize = get_arg(&args, "--shots")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    println!("=== Azure Quantum Bell State ===\n");
    println!("Workspace:      {}", workspace);
    println!("Resource group: {}", rg);
    println!("Target:         {}", target);
    println!("Shots:          {}\n", shots);

    // Build Bell state circuit
    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("result".to_string(), 2, true);
    circuit += Hadamard::new(0);
    circuit += CNOT::new(0, 1);
    circuit += MeasureQubit::new(0, "result".to_string(), 0);
    circuit += MeasureQubit::new(1, "result".to_string(), 1);

    // Run on Azure via az CLI
    let runner = AzureCliRunner::new(&workspace, &rg, &target);
    let results = runner.run(&circuit, shots);

    // Display results
    if let Some(shot_results) = results.get("result") {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for bits in shot_results {
            let key: String = bits.iter().map(|&b| if b { '1' } else { '0' }).collect();
            *counts.entry(key).or_insert(0) += 1;
        }

        println!("Results ({} shots):", shot_results.len());
        let mut entries: Vec<_> = counts.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        for (state, count) in &entries {
            let pct = **count as f64 / shot_results.len() as f64 * 100.0;
            println!("  |{}⟩: {} ({:.1}%)", state, count, pct);
        }

        // Verify Bell state: should only see |00⟩ and |11⟩
        let correlated = counts.get("00").unwrap_or(&0) + counts.get("11").unwrap_or(&0);
        let total = shot_results.len();
        println!(
            "\nCorrelated outcomes: {}/{} ({:.1}%)",
            correlated,
            total,
            correlated as f64 / total as f64 * 100.0
        );
    } else {
        println!("No results returned");
    }
}

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
