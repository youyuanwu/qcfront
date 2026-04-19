# Feature: Shor's Algorithm for RSA Factoring in Rust

## Goal

Implement Shor's quantum factoring algorithm in Rust using roqoqo,
capable of factoring small semiprimes (starting with N=15, targeting up to N=35).

## Status

**Phase 1 (Classical math) ✅** — `algos/src/math.rs`: continued fractions, mod_pow,
find_order, random_coprime. 11 tests.

**Phase 2 (Hardcoded Shor for N=15) ✅** — Full pipeline working:
- `algos/src/circuits/modmul_15.rs` — hardcoded controlled modular multiplication for all
  coprimes of 15 (a ∈ {2, 4, 7, 8, 11, 13, 14}), using cyclic bit shifts and controlled SWAPs.
- `algos/src/shor.rs` — `factor()` accepts a backend-runner closure, keeping the lib
  backend-agnostic. Also exposes `build_order_finding_circuit`, `extract_order`, `find_factors`.
- `examples/src/bin/shor_15.rs` — factors 15 = 3 × 5 reliably.
- 33 tests total across workspace.

**Phase 3 (RSA demo) — TODO**

**Phase 4 (General modular arithmetic) — Future**
- `draper_adder.rs` — QFT-based quantum adder
- `mod_adder.rs` — modular adder (overflow detection + conditional subtract)
- `mod_mul.rs` — modular multiplier from modular adder
- Would enable factoring arbitrary N without per-N hardcoded circuits.

## Reference Implementations

- **Python (Qiskit):** [rsa_factoring_shor_demo.py](https://github.com/elia1359/Shor_Algorithm-Quantum-Computing-/blob/main/rsa_factoring_shor_demo.py) — 10 lines, uses `Shor().factor(N=143)` which hides everything
- **C++ (qpp):** [shor.cpp](https://github.com/softwareQinc/qpp/blob/main/examples/shor.cpp) — ~120 lines, uses `gt.MODMUL`, `applyTFQ`, `convergents` built-in primitives
- **Rust (RustQuantum):** [im4vk/RustQuantum](https://github.com/im4vk/RustQuantum) — ~400 lines, but uses classical order-finding (not a real circuit simulation)

## How qpp Actually Works Internally (Not Gate Decomposition)

qpp is fundamentally a **linear algebra library with quantum semantics**, not a circuit
simulator. This is the key architectural difference that makes its Shor implementation
so concise (~120 lines).

### `applyCTRL`: Direct Amplitude Manipulation

qpp does NOT decompose controlled-U into smaller gates. It does NOT build a full 2ⁿ×2ⁿ
matrix either. Instead, it **directly manipulates the state vector amplitudes**:

```cpp
// Pseudocode of what qpp's applyCTRL does internally:
applyCTRL(psi, U, controls, targets):
  for each basis state index i in 0..2^n:
    if control qubits of |i⟩ are all |1⟩:
      extract the target-qubit subspace amplitudes
      multiply by the small U matrix (e.g., 2^k × 2^k for k target qubits)
      write updated amplitudes back to state vector
    else:
      leave amplitude unchanged  // identity on non-controlled subspace
```

This is a **single pass** over the state vector. For n qubits, it's O(2ⁿ) time regardless
of how complex U is. Whether U is a 2×2 Hadamard or a 16×16 modular multiplier, the cost
is the same — one sweep through the state vector.

### `gt.MODMUL(a, N, n)`: Classical Matrix Construction

The `MODMUL` gate is constructed by **computing the permutation classically**:

```cpp
gt.MODMUL(a=7, N=15, n=4):
  Create 16×16 matrix M (all zeros)
  For x in 0..14:
    y = (7 * x) % 15
    M[y][x] = 1                         // permutation: column x maps to row y
  For x in 15..15:                      // states >= N: identity (required for unitarity)
    M[x][x] = 1
  Return M
```

The matrix is computed **classically** in O(2ⁿ) time, then `applyCTRL` applies it as
described above. The simulator never decomposes this into quantum gates.

### The Shor Circuit in qpp (What Actually Happens)

```
qpp's shor.cpp execution trace:

Step 1..n:    apply(psi, H, {k})   → n amplitude sweeps

Step n+1..2n: applyCTRL(psi, MODMUL(a^(2^k), N, n), {k}, work)
              → for each: compute 2^n × 2^n permutation matrix (O(2^n))
                          + one amplitude sweep (O(2^2n))

Step 2n+1:    applyTFQ(psi, counting)
              → inverse QFT as direct matrix operation

Total simulator operations: ~3n amplitude sweeps over 2^(2n) entries
```

Compare to Qiskit/roqoqo which execute **thousands of individual gate operations**,
each one sweeping through the state vector. For N=35: qpp does ~36 operations vs
~8,000 for gate-based simulation.

## Algorithm Overview

```
INPUT: N (number to factor)

1. CLASSICAL: Pick random a coprime to N
2. QUANTUM:   Find the order r of a mod N using quantum period finding
3. CLASSICAL: Use r to compute factors via GCD

The quantum part (step 2):
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  |0⟩⊗n ─── H⊗n ─── ctrl-U^(2^0) ──── ... ──── QFT⁻¹ ── Measure │
│                      │                                          │
│  |0...01⟩ ────────── U^(2^0) ─── U^(2^1) ─── ... ─────────────  │
│                                                                 │
│  where U = modular multiplication by a mod N                    │
└─────────────────────────────────────────────────────────────────┘
```

## How Qiskit Implements This (Gate-Level, General N)

Qiskit's `Shor` class builds a **real gate-level circuit** for any N, fully decomposed
into elementary quantum gates from nested sub-circuits:

```
ModularExponentiation(a, N)
  ├── For each bit i of the exponent register:
  │     └── Controlled ModularMultiplication(a^(2^i), N)
  │           ├── For each bit j of the multiplier:
  │           │     └── Controlled ModularAddition(a^(2^i) · 2^j, N)
  │           │           ├── QFT on accumulator register
  │           │           ├── PhaseAdd(value)          ← controlled phase rotations
  │           │           ├── Inverse QFT
  │           │           ├── Compare: is result ≥ N?  ← overflow detection
  │           │           ├── If overflow: PhaseAdd(-N) ← subtract N
  │           │           └── Uncompute comparison ancilla
  │           └── SWAP accumulator ↔ target (copy result back)
  └── Inverse QFT on exponent register (for phase estimation)
```

The foundation is the **Draper QFT-based adder** (Draper 2000): adds a classical constant
to a quantum register via phase rotations in Fourier space. Everything else stacks on top.

### Qubit Budget

| Construction | Formula | N=15 | N=21 | N=143 |
|---|---|---|---|---|
| Standard (textbook) | 3n | 12 | 15 | 24 |
| Beauregard (optimized) | 2n+3 | 11 | 13 | 19 |

Our implementation uses the standard construction (simpler to build).

### Gate Count

Gate count grows as **O(n³)** for general N. Hardcoded N=15 is O(n) per operation.

| N | Bits (n) | Qubits (standard) | Approximate gates |
|---|---|---|---|
| 15 | 4 | 12 | ~150-350 (hardcoded) |
| 21 | 5 | 15 | ~2,000-5,000 (general) |
| 35 | 6 | 18 | ~5,000-10,000 |
| 143 | 8 | 24 | ~30,000-80,000 |

## Comparison of Approaches

| | Hardcoded (our Phase 2) | General Draper (Qiskit-style) | Matrix (qpp-style) |
|---|---|---|---|
| **Circuit type** | Hand-written SWAPs | QFT adder → mod adder → mod mul | Unitary matrix |
| **Works for any N?** | ✗ One N per impl | ✅ Yes | ✅ Yes |
| **Gate count** | O(n) per operation | O(n³) total | 1 operation |
| **Honest quantum sim?** | ✅ Real gates | ✅ Real gates | ⚠️ Matrix shortcut |
| **Implementation effort** | ~100 lines per N | ~500-1000 lines once | ~10 lines |

## Key Differences: qpp vs. Qiskit vs. roqoqo

| | qpp (C++) | Qiskit/Aer (Python/C++) | roqoqo (Rust) |
|---|---|---|---|
| **Core abstraction** | State vector + matrices | Gate circuits | Gate circuits |
| **Controlled-U** | Direct amplitude manipulation | Decompose to gates | Decompose to gates |
| **MODMUL** | Permutation matrix, 1 sweep | QFT adder stack (thousands of gates) | Same as Qiskit |
| **Cost per MODMUL** | O(2ⁿ) | O(n³ · 2ⁿ) | O(n³ · 2ⁿ) |

| Feature | qpp | Qiskit | roqoqo |
|---|---|---|---|
| Arbitrary unitary on qubits | ✅ | ✅ | ✗ Must decompose |
| Built-in MODMUL | ✅ | ✅ (gate-level) | ✗ Must build |
| Inverse QFT | ✅ | ✅ | ✅ Built-in |
| Controlled-U on subsystem | ✅ | ✅ | ✗ Must decompose |

**The tradeoff:** qpp hides complexity in the simulator, Qiskit hides it in library code,
roqoqo exposes it all to the user. But roqoqo circuits are real gate sequences that could
run on actual hardware.

## roqoqo Gate Convention Notes

**`Toffoli::new(target, ctrl1, ctrl2)`** — first argument is the **target**, not a control.
This is the opposite of many textbook conventions (`CCX(c0, c1, target)`). The Fredkin
(controlled SWAP) decomposition must account for this:

```rust
// Correct Fredkin gate in roqoqo:
CNOT::new(target_b, target_a);                    // a ^= b
Toffoli::new(target_b, control, target_a);         // b ^= (control AND a)
CNOT::new(target_b, target_a);                    // a ^= b
```

**`QFT::new(qubits: Vec<usize>, swaps: bool, inverse: bool)`** — takes qubit indices,
not just a count.

## References

- qpp shor.cpp: <https://github.com/softwareQinc/qpp/blob/main/examples/shor.cpp>
- RustQuantum (Rust Shor reference): <https://github.com/im4vk/RustQuantum>
- Qiskit Textbook Shor's: <https://github.com/Qiskit/textbook/blob/main/notebooks/ch-algorithms/shor.ipynb>
- Beauregard, "Circuit for Shor's algorithm using 2n+3 qubits", 2002: <https://arxiv.org/abs/quant-ph/0205095>
- Draper, "Addition on a Quantum Computer", 2000: <https://arxiv.org/abs/quant-ph/0008033>
- Nielsen & Chuang, Section 5.3: Quantum factoring
