# Feature: Grover's Search Algorithm in Rust

## Goal

Implement Grover's quantum search algorithm in Rust using roqoqo, capable of finding
marked items in an unstructured search space of N = 2ⁿ items in O(√N) queries.

## Status

**Phase 1 (Core n ≤ 3) ✅** — `algos/src/grover.rs`: `search()`, `GroverConfig`/`GroverResult`.

**Phase 2 (Scaling n ≥ 4) ✅** — `algos/src/circuits/multi_cz.rs`: Barenco V-chain for
arbitrary n. CCZ convention verified.

**Phase 3 (General Oracle Framework) ✅** — `GroverOracle` struct with `single()`/`multi()`,
`search_with_oracle()`, `success: Option<bool>`, multi-target dedup, M-aware iteration count.

**Phase 4 (SAT Solver) ✅** — `algos/src/sat.rs`: `Literal` newtype, `SatOracle`,
`circuits/multi_cx.rs`: multi-controlled-X. Full pipeline: CNF → Grover → satisfying assignment.

- `examples/src/bin/grover.rs` — single-target and multi-target search demos.
- `examples/src/bin/sat_grover.rs` — solves (x₁) ∧ (x₂ ∨ x₃) ∧ (¬x₂ ∨ x₃) via Grover.
- 68 tests total across workspace.

## Algorithm Overview

Grover's algorithm searches an unsorted database of N = 2ⁿ items for a marked element
in O(√N) queries, a quadratic speedup over classical O(N) search.

```
INPUT: N = 2^n (search space size), oracle f(x) that marks solutions

1. INITIALIZE:  |0⟩^⊗n → H^⊗n → equal superposition |s⟩ = (1/√N) Σ|x⟩
2. REPEAT k ≈ ⌊(π/4)√(N/M)⌋ times (M = number of solutions):
   a. ORACLE:    flip phase of marked states  |x⟩ → (-1)^f(x) |x⟩
   b. DIFFUSER:  reflect about the mean       2|s⟩⟨s| - I
3. MEASURE:     observe the marked state with probability ≈ 1
```

Each Grover iteration rotates the state vector toward the solution subspace by
θ = arcsin(√(M/N)). After k iterations the success probability is sin²((2k+1)θ).

### Key Formulas

| Formula | Meaning |
|---|---|
| k = ⌊(π/4) × √(N/M)⌋ | Optimal number of Grover iterations |
| P(success) = sin²((2k+1)θ), sin θ = √(M/N) | Probability of measuring a solution |
| N = 2ⁿ | Search space size for n qubits |

When M > 1 solutions exist, optimal iterations decrease: k ≈ (π/4)√(N/M).
If M is unknown, use exponential search: try k = 1, 2, 4, 8, … until measurement
succeeds. Expected total queries: O(√(N/M)).

## Circuit Decomposition

### Oracle (Phase Flip for Target |t⟩)

```
1. For each qubit i where bit i of t is 0: apply X(i)
2. Apply multi-controlled-Z on all n qubits
3. Undo the X gates from step 1
```

This maps |t⟩ → |11…1⟩, applies the phase flip, then restores the encoding.

### Diffuser

The diffuser reflects the state about |s⟩. The H-X-MCZ-X-H circuit implements
−(2|s⟩⟨s| − I) = I − 2|s⟩⟨s|, which differs from 2|s⟩⟨s| − I by a global phase
of −1 (unobservable). This works because MCZ = I − 2|1…1⟩⟨1…1|, so
X⊗n·MCZ·X⊗n = I − 2|0…0⟩⟨0…0|, and sandwiching with H⊗n gives I − 2|s⟩⟨s|.

### Multi-Controlled-Z / Multi-Controlled-X

Both the oracle and diffuser require an n-qubit controlled-Z gate (phase flip of |11…1⟩).
The SAT oracle additionally needs multi-controlled-X (bit flip, generalized Toffoli).
roqoqo provides native gates up to 3 qubits:

| n | MCZ gate | MCX gate |
|---|---|---|
| 1 | `PauliZ` | `PauliX` |
| 2 | `ControlledPauliZ` | `CNOT` |
| 3 | `ControlledControlledPauliZ` | `Toffoli` |
| 4+ | V-chain: 2(n−2) Toffoli + 1 CZ | V-chain: 2(n−2) Toffoli + 1 CNOT |

Both decompositions use n−2 ancilla qubits and rely on the Toffoli self-inverse
property for uncomputation. CCZ is symmetric (argument order irrelevant).

**Qubit layout:** n data qubits + max(0, n−2) ancillas. Practical cutoff: ~n=15
(q=28 → ~2 GB memory).

## SAT Oracle Circuit Construction

Each CNF clause is evaluated using De Morgan's law: `a OR b = NOT(NOT a AND NOT b)`.
Results are stored in clause ancilla qubits using multi-controlled-X (not MCZ — clause
evaluation flips bit values, not phases).

For clause (x₁ OR x₂): negate inputs → Toffoli into clause ancilla → restore → flip.
For clause (¬x₁ OR x₃): negate non-negated inputs → Toffoli → restore → flip.

Clause results are ANDed via Toffoli into a sat ancilla. PauliZ on the (entangled) sat
ancilla produces the phase flip via standard phase kickback: (-1)^f(x) on each branch.
All workspace qubits are uncomputed in reverse order.

The current implementation uses classical brute-force to enumerate solutions for small
instances, then creates a `GroverOracle::multi`. A fully circuit-based oracle (building
the De Morgan gates directly) is a future enhancement.

## How qpp Implements Grover (State-Vector, Not Gates)

qpp's `grover.cpp` uses direct state-vector manipulation:

```cpp
psi(marked) = -psi(marked);                   // Oracle: direct amplitude flip
cmat G = 2 * prj(psi_initial) - gt.Id(N);     // Diffuser: dense N×N matrix
psi = (G * psi).eval();
```

| Aspect | qpp (state-vector) | roqoqo (gate-based) |
|---|---|---|
| Oracle | Direct amplitude flip | X gates + multi-CZ + X gates |
| Diffuser | Dense N×N matrix multiply | H + X + multi-CZ + X + H |
| Cost per iteration | O(N²) = O(4ⁿ) time | O(n) gates, each O(2^q) in simulation |
| Memory | O(4ⁿ) (precomputed G + state) | O(2^q) (state vector only) |
| Hardware-realistic | No | Gate-level (all-to-all connectivity assumed) |

## roqoqo Gate Convention Notes

- **`Toffoli::new(target, ctrl1, ctrl2)`** — first argument is the **target**
- **`ControlledControlledPauliZ::new(q0, q1, q2)`** — symmetric (order irrelevant)
- See ShorFactoring.md for the Toffoli convention lesson and Fredkin decomposition

## Applications

- **Unstructured search**: Find a specific item among N possibilities in O(√N)
- **Cryptographic key search**: Reduce brute-force key space from 2ⁿ to 2^(n/2)
- **SAT solving**: Search for satisfying assignments (with appropriate oracle)
- **Amplitude amplification**: General technique — boost success probability of any
  quantum subroutine (Grover is a special case)

## References

- qpp grover.cpp: <https://github.com/softwareQinc/qpp/blob/main/examples/grover.cpp>
- Grover, "A Fast Quantum Mechanical Algorithm for Database Search", 1996:
  <https://arxiv.org/abs/quant-ph/9605043>
- Barenco et al., "Elementary gates for quantum computation", 1995:
  <https://arxiv.org/abs/quant-ph/9503016> (multi-controlled gate decomposition)
- Nielsen & Chuang, Section 6.1–6.2: Grover's algorithm
- roqoqo gate docs: <https://hqsquantumsimulations.github.io/qoqo/gate_operations/multi_qubit_gates.html>
