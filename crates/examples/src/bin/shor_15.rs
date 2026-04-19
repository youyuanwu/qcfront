//! Factor N=15 using Shor's quantum algorithm.
//!
//! Demonstrates the full pipeline using `algos::shor::factor_verbose`.

use algos::math::mod_pow;
use algos::shor::{factor_verbose, ShorConfig};
use roqoqo::backends::EvaluatingBackend;
use roqoqo_quest::Backend;

const N: u64 = 15;

fn main() {
    println!("=== Shor's Algorithm: Factoring N={} ===\n", N);

    let config = ShorConfig::default();

    let result = factor_verbose(
        N,
        &config,
        // Backend-specific runner closure
        |circuit, total_qubits| {
            let backend = Backend::new(total_qubits, None);
            let (bits, _, _) = backend.run_circuit(circuit).expect("simulation failed");
            bits
        },
        // Progress callback
        |attempt| {
            print!("  a={}: ", attempt.a);
            match (attempt.order, attempt.factors) {
                (_, Some((p, q))) if attempt.shots_used == 0 => {
                    println!("trivial factor (gcd)");
                    println!("\n✓ {} = {} × {}", N, p, q);
                }
                (Some(r), Some((p, q))) => {
                    println!(
                        "order r={} (a^r mod N = {}) in {} shot(s)",
                        r,
                        mod_pow(attempt.a, r, N),
                        attempt.shots_used,
                    );
                    println!("\n✓ {} = {} × {}", N, p, q);
                }
                (Some(r), None) => {
                    println!("order r={} but no factors (odd or trivial root)", r);
                }
                (None, _) => {
                    println!("no order found in {} shots", attempt.shots_used);
                }
            }
        },
    );

    if result.is_none() {
        println!("\n✗ Failed to factor {}", N);
    }
}
