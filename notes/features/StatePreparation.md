# State Preparation

Synthesize a circuit that transforms |0...0⟩ into an arbitrary target
state |ψ⟩ = Σ αᵢ|i⟩ using elementary gates.

Implemented in `crates/algos/src/state.rs`.

## Algorithm

Uses **Möttönen decomposition** (arXiv:quant-ph/0407010): a binary tree
of uniformly controlled rotations.

**Qiskit comparison**: Qiskit's `StatePreparation` uses Iten et al.'s
Isometry decomposition (arXiv:1501.06911) — column-by-column elimination
with multi-controlled gates. Same O(2ⁿ) asymptotic cost, but Möttönen
is simpler to implement and only needs CNOT + Ry/Rz.

1. **Ry tree** — sets the probability distribution level by level.
   At level k, a rotation controlled by qubits 0..k-1 acts on qubit k.
   Angle: θ = 2·arccos(‖left subtree‖ / ‖parent subtree‖).

2. **Rz tree** — sets relative phases with the same tree structure.

Each uniformly controlled rotation (k controls, 2^k angles) decomposes
into CNOT + Ry/Rz pairs in Gray code order. The target-to-decomposition
angle transform is a Walsh-Hadamard-like computation:
  a[i] = (1/n) · Σ_c (-1)^{popcount(gray[i] & c)} · θ[c]

**Complexity**: O(2ⁿ) CNOTs + O(2ⁿ) single-qubit gates.
This is a proven lower bound (Knill 1995, Shende-Bullock-Markov 2006):
an n-qubit state has 2^(n+1) − 2 real parameters, each gate sets O(1).

### Special cases with efficient circuits

| State Type | Cost | Example |
|------------|------|---------|
| Computational basis | O(n) | X gates |
| Uniform superposition | O(n) | H gates |
| Product states | O(n) | No entanglement |
| Sparse (k nonzero) | O(kn) | Few amplitudes |

## API

```rust
use algos::state::{QuantumState, prepare_state, fidelity, NORM_TOLERANCE};

// Constructors (all validate invariants, panic on invalid input)
let s = QuantumState::dense(vec![...]);           // 2^n complex amplitudes
let s = QuantumState::sparse(3, vec![(0, a)]);    // (index, amplitude) pairs
let s = QuantumState::uniform(3, &[0, 7]);        // equal superposition
let s = QuantumState::basis(3, 5);                // single basis state |101⟩

// Accessors
s.num_qubits();                   // inferred from dimension
s.amplitude_at(idx);              // single amplitude lookup
s.to_dense();                     // full Vec<Complex64>
s.iter_nonzero();                 // sparse iteration
s.is_sparse();                    // has any zero amplitudes?

// Circuit synthesis (Möttönen decomposition)
let circuit: Circuit = prepare_state(&s);

// State comparison (global-phase-insensitive)
let f: f64 = fidelity(&a, &b);   // |⟨ψ|φ⟩|², 1.0 = identical
```

### Validation rules

- All amplitudes must be finite (no NaN/Inf)
- Normalization: |Σ|αᵢ|² − 1| < `NORM_TOLERANCE` (1e-10)
- `dense()`: length must be ≥ 2 and a power of 2
- `sparse()`: no duplicate indices, panics on duplicates
- `uniform()`: silently deduplicates (matches `GroverOracle::multi`)

### Internal representation

Enum of `Dense(Vec<Complex64>)` | `Sparse { num_qubits, amps }`.
Constructors choose the variant; `to_dense()` converts on demand.

## Reverse Direction (future)

`analyze_state()` extracts the exact state vector from a circuit using
`PragmaGetStateVector` (QuEST-specific). Requires `roqoqo_quest::Backend`
directly — `QuantumRunner` only returns `BitRegisters`.

Round-trip precision: approximate due to floating-point accumulation
in O(2ⁿ) gate angles and global phase ambiguity. Use `fidelity()` for
comparison, not exact equality.

## Sources

- Möttönen et al., arXiv:quant-ph/0407010 (2004)
- Shende, Bullock, Markov, arXiv:quant-ph/0406176 (2006)
- Knill, arXiv:quant-ph/9508006 (1995)
- Iten et al., arXiv:1501.06911 (2016)
