// Deutsch-Jozsa Algorithm
//
// Determines whether a function f: {0,1}^n → {0,1} is constant (same
// output for all inputs) or balanced (returns 0 for exactly half the
// inputs and 1 for the other half) — in a SINGLE query.
//
// Classically, you need 2^(n-1) + 1 queries in the worst case.
// Quantum: exactly 1 query, regardless of n.
//
// Circuit:
//   |0⟩^⊗n ── H⊗n ── oracle ── H⊗n ── measure
//   |1⟩     ── H   ─┘
//
// If all measured qubits are 0: f is constant.
// If any measured qubit is 1: f is balanced.

use algos::qubit::{Qubit, QubitAllocator};
use roqoqo::backends::EvaluatingBackend;
use roqoqo::operations::*;
use roqoqo::Circuit;
use roqoqo_quest::Backend;

// ---------------------------------------------------------------------------
// Oracle definition — one description, two implementations
// ---------------------------------------------------------------------------

/// An oracle defined by its parameters. Both classical evaluation and
/// quantum circuit are derived from the same definition.
enum Oracle {
    /// f(x) = b for all x. Constant.
    Constant(bool),
    /// f(x) = s · x (mod 2) for hidden string s. Balanced when s ≠ 0.
    InnerProduct(Vec<bool>),
}

impl Oracle {
    /// Classical evaluation: compute f(x) directly.
    fn eval(&self, x: u64) -> u8 {
        match self {
            Oracle::Constant(c) => *c as u8,
            Oracle::InnerProduct(s) => {
                let mut dot = 0u8;
                for (i, &bit) in s.iter().enumerate() {
                    if bit {
                        dot ^= ((x >> i) & 1) as u8;
                    }
                }
                dot
            }
        }
    }

    /// Quantum implementation: append gates that flip output when f(x) = 1.
    ///   |x⟩|y⟩ → |x⟩|y ⊕ f(x)⟩
    fn apply(&self, circuit: &mut Circuit, inputs: &[Qubit], output: Qubit) {
        match self {
            Oracle::Constant(c) => {
                if *c {
                    *circuit += PauliX::new(output.index());
                }
            }
            Oracle::InnerProduct(s) => {
                for (i, &bit) in s.iter().enumerate() {
                    if bit {
                        *circuit += CNOT::new(inputs[i].index(), output.index());
                    }
                }
            }
        }
    }

    fn is_constant(&self) -> bool {
        matches!(self, Oracle::Constant(_))
    }
}

struct TestCase {
    label: &'static str,
    oracle: Oracle,
}

fn main() {
    println!("=== Deutsch-Jozsa Algorithm ===\n");

    let n = 4; // number of input qubits
    let n_total = 1u64 << n; // N = 2^n = 16

    let cases = vec![
        TestCase {
            label: "f(x) = 0            ",
            oracle: Oracle::Constant(false),
        },
        TestCase {
            label: "f(x) = 1            ",
            oracle: Oracle::Constant(true),
        },
        TestCase {
            label: "f(x) = x₀           ",
            oracle: Oracle::InnerProduct(vec![true, false, false, false]),
        },
        TestCase {
            label: "f(x) = x₀⊕x₁⊕x₂⊕x₃",
            oracle: Oracle::InnerProduct(vec![true, true, true, true]),
        },
        TestCase {
            label: "f(x) = x₁⊕x₃       ",
            oracle: Oracle::InnerProduct(vec![false, true, false, true]),
        },
    ];

    // -----------------------------------------------------------------------
    // Classical approach: must query f until we can distinguish constant/balanced
    // -----------------------------------------------------------------------
    println!("--- Classical approach ---");
    println!(
        "  Worst case: need {} queries (2^(n-1) + 1) for n={}\n",
        n_total / 2 + 1,
        n
    );

    for case in &cases {
        let (classical_result, queries) = classical_deutsch_jozsa(n, |x| case.oracle.eval(x));
        println!(
            "  {} → {:8} after {:>2} queries",
            case.label,
            if classical_result {
                "CONSTANT"
            } else {
                "BALANCED"
            },
            queries,
        );
        assert_eq!(case.oracle.is_constant(), classical_result);
    }

    // -----------------------------------------------------------------------
    // Quantum approach: always exactly 1 query
    // -----------------------------------------------------------------------
    println!("\n--- Quantum approach ---");
    println!("  Always: exactly 1 query for any n\n");

    for case in &cases {
        let result = quantum_deutsch_jozsa(n, |circuit, inputs, output| {
            case.oracle.apply(circuit, inputs, output);
        });
        let quantum_constant = result == 0;
        println!(
            "  {} → {:8} (measured {:0>width$b})",
            case.label,
            if quantum_constant {
                "CONSTANT"
            } else {
                "BALANCED"
            },
            result,
            width = n,
        );
        assert_eq!(case.oracle.is_constant(), quantum_constant);
    }

    println!("\n✓ Classical and quantum agree on all oracles!");
    println!(
        "  Classical worst case: {} queries, Quantum: 1 query ({:.0}× speedup)",
        n_total / 2 + 1,
        (n_total / 2 + 1) as f64,
    );
}

// ---------------------------------------------------------------------------
// Classical approach
// ---------------------------------------------------------------------------

/// Classical Deutsch-Jozsa: query f one input at a time.
///
/// Strategy: query f(0), f(1), f(2), ... If we ever see two different
/// outputs, f is balanced. If the first 2^(n-1) + 1 outputs are all the
/// same, f must be constant (a balanced function must differ on at least
/// one of the first half + 1 inputs).
///
/// Returns (is_constant, num_queries).
fn classical_deutsch_jozsa<F>(n: usize, f: F) -> (bool, u64)
where
    F: Fn(u64) -> u8,
{
    let first = f(0);
    let max_queries = (1u64 << n) / 2 + 1;
    for x in 1..max_queries {
        if f(x) != first {
            return (false, x + 1); // balanced — found a difference
        }
    }
    (true, max_queries) // constant — all same after 2^(n-1)+1 queries
}

// ---------------------------------------------------------------------------
// Quantum approach
// ---------------------------------------------------------------------------

/// Quantum Deutsch-Jozsa: determine constant vs balanced in one query.
///
/// Returns 0 if f is constant, non-zero if f is balanced.
fn quantum_deutsch_jozsa<F>(n: usize, oracle: F) -> usize
where
    F: FnOnce(&mut Circuit, &[Qubit], Qubit),
{
    assert!(n >= 1, "need at least 1 input qubit");

    let mut alloc = QubitAllocator::new();
    let input_reg = alloc.allocate("input", n);
    let output_reg = alloc.allocate("output", 1);
    let output = output_reg.qubit(0);

    let mut circuit = Circuit::new();
    circuit += DefinitionBit::new("result".to_string(), n, true);

    // Prepare output qubit in |1⟩ then H → |−⟩ (phase kickback ancilla)
    circuit += PauliX::new(output.index());

    // Hadamard on all qubits (input + output)
    for q in input_reg.iter() {
        circuit += Hadamard::new(q.index());
    }
    circuit += Hadamard::new(output.index());

    // Apply oracle: |x⟩|−⟩ → (−1)^f(x)|x⟩|−⟩
    let input_qubits = input_reg.to_qubits();
    oracle(&mut circuit, &input_qubits, output);

    // Hadamard on input qubits
    for q in input_reg.iter() {
        circuit += Hadamard::new(q.index());
    }

    // Measure input qubits
    for (i, q) in input_reg.iter().enumerate() {
        circuit += MeasureQubit::new(q.index(), "result".to_string(), i);
    }

    // Run
    let backend = Backend::new(alloc.total(), None);
    let (bits, _, _) = backend.run_circuit(&circuit).unwrap();
    let measured = &bits["result"][0];

    // Convert to integer (LSB-first)
    measured
        .iter()
        .enumerate()
        .fold(0usize, |acc, (i, &b)| if b { acc | (1 << i) } else { acc })
}
