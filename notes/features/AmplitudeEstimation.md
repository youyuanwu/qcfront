# Amplitude Estimation and Quantum Counting

Two related features: **amplitude estimation** is a general algorithm for
estimating probabilities in quantum circuits; **quantum counting** is its
application to Grover search for estimating solution counts.

## Architecture

```
amplitude_estimation.rs   ← general algorithm (like qpe.rs)
grover/counting.rs        ← thin wrapper: quantum counting = AE + Grover oracle
```

This mirrors the existing pattern: QPE is a general tool in `qpe.rs`, Shor
uses it from `shor.rs`. Amplitude estimation is the general tool, quantum
counting uses it from the Grover module.

## Part 1: Amplitude estimation

### Algorithm

Given a state preparation circuit A and a set of "good" qubits, estimate the
probability of measuring a good outcome. Uses QPE on the Grover-like operator
Q = A·S₀·A†·Sχ where:

- A prepares an arbitrary state from |0⟩
- Sχ reflects about the "good" subspace (phase flip on good states)
- S₀ reflects about |0⟩ (phase flip on the zero state)

QPE on Q yields a fraction φ ∈ [0,1) where:

```
probability_of_good = sin²(πφ)
```

With t counting qubits, the additive error on the estimated probability is
O(1/2^t). Recommend t ≥ ⌈log₂(N)⌉ + 2 for useful precision.

Note: QPE measures a superposition of two eigenphases (φ and 1−φ), both
yielding the same sin². The `measured_phase` field in results may correspond
to either value.

### Circuit structure

```
|0⟩^⊗t ── H ── controlled-Q^(2^0) ── … ── controlled-Q^(2^(t-1)) ── inverse-QFT ── measure
                    │                               │
|0⟩^⊗n ── A ───── Q operator ──────────────────────────────────
```

### API

```rust
// amplitude_estimation.rs

/// Result of amplitude estimation.
pub struct AmplitudeResult {
    /// Estimated probability of the "good" outcome.
    pub estimated_probability: f64,
    /// Raw phase measured by QPE.
    pub measured_phase: f64,
}

/// Problem definition for amplitude estimation.
pub struct AmplitudeEstimationProblem {
    /// Number of data qubits the state prep acts on.
    pub num_data_qubits: usize,
    /// State preparation circuit A (applied to |0⟩^⊗n).
    pub state_prep: Circuit,
    /// Compute circuit for the good-state marker (U in U · MCZ · U†).
    pub marker_compute: Circuit,
    /// Qubits participating in the good-state MCZ.
    pub marker_mcz_qubits: QubitRange,
    /// Scratch qubits for the marker MCZ (sized for uncontrolled).
    pub marker_mcz_scratch: QubitRange,
}

/// Estimate the probability of a "good" outcome.
///
/// Internally constructs controlled versions of Sχ (marker) and S₀
/// (reflection about |0⟩). The controlled MCZ requires additional scratch
/// beyond what the problem provides: `required_ancillas(n + 1)` for S₀
/// and `required_ancillas(marker_mcz_qubits.len() + 1)` for Sχ.
pub fn estimate_amplitude<R: QuantumRunner>(
    problem: &AmplitudeEstimationProblem,
    config: &AmplitudeEstimationConfig,
    runner: &R,
) -> AmplitudeResult

/// Configuration for amplitude estimation.
pub struct AmplitudeEstimationConfig {
    /// Number of QPE counting qubits (precision bits). Must be ≥ 2.
    pub counting_bits: usize,
    /// Number of measurement shots.
    pub num_shots: usize,
}

impl Default for AmplitudeEstimationConfig {
    fn default() -> Self {
        Self { counting_bits: 8, num_shots: 100 }
    }
}
```

### Applications beyond Grover

| Application | State prep A | Good states |
|---|---|---|
| **Quantum counting** | H⊗n (uniform) | Oracle-marked solutions |
| **Monte Carlo** | Encode distribution | Outcome above threshold |
| **Integration** | Encode function | Accumulator bits |
| **Risk analysis** | Encode portfolio | Loss exceeds VaR |

## Part 2: Quantum counting

### Algorithm

Quantum counting is amplitude estimation where:
- A = H⊗n (uniform superposition over search space)
- Sχ = Grover oracle (marks solutions)

The estimated probability gives the solution count:

```
M = N · sin²(π · measured_phase)    where N = 2^n
```

With t counting qubits, additive error on M is ±N/2^t.

### Practical behavior

- M = 0: φ = 0, QPE measures all zeros → M̂ = 0
- M = N: φ = 0.5, QPE measures 2^(t-1) → M̂ = N
- M = 1, N = 8: φ ≈ 0.115, QPE (t=4) measures ~2 → M̂ ≈ 1.17

### API

```rust
// grover/counting.rs

/// Configuration for quantum counting.
pub struct CountingConfig {
    /// QPE precision bits. Must be ≥ 2.
    pub counting_bits: usize,
    /// Measurement shots.
    pub num_shots: usize,
}

impl Default for CountingConfig {
    fn default() -> Self {
        Self { counting_bits: 8, num_shots: 100 }
    }
}

/// Result of quantum counting.
pub struct CountingResult {
    /// Estimated number of solutions (clamped to [0, N]).
    pub estimated_count: f64,
    /// Raw phase measured by QPE (may be θ/π or 1−θ/π; both
    /// yield the same estimated_count via sin²).
    pub measured_phase: f64,
    /// Optimal Grover iterations for this solution count.
    /// 0 when estimated_count ≥ N/2. None when estimated_count < 1.
    pub optimal_iterations: Option<usize>,
}

/// Estimate the number of solutions using quantum counting.
///
/// Builds an AmplitudeEstimationProblem from the oracle and delegates
/// to estimate_amplitude(). Requires oracle to support decompose().
///
/// The controlled MCZ requires additional scratch beyond what
/// decompose() returns — see "Scratch budget" section.
pub fn quantum_counting<O: Decomposable, R: QuantumRunner>(
    oracle: &O,
    config: &CountingConfig,
    runner: &R,
) -> CountingResult
```

### Auto-tuned Grover search

Quantum counting enables fully automatic Grover search for oracles that
don't know their solution count (CnfOracle, SubsetSumOracle):

```rust
/// Grover search with automatic iteration count.
///
/// 1. Fast-path: if oracle.num_solutions() is Some, skip counting
/// 2. Otherwise: run quantum counting to estimate M
/// 3. Clamp M̂ to [1, N] and compute optimal iterations: ⌊π/4 · √(N/M̂)⌋
///    Cap at ⌊π/4 · √N⌋ (the M=1 upper bound) for robustness
/// 4. If M̂ rounds to 0, return GroverError::NoSolutions
/// 5. Run Grover search with the computed iteration count
pub fn auto_search<O: Decomposable, R: QuantumRunner>(
    oracle: &O,
    config: &CountingConfig,
    runner: &R,
) -> Result<GroverResult, GroverError>
```

This eliminates the `GroverError::IterationsRequired` failure mode —
callers never need to guess the iteration count.

## Key insight: the conjugation trick

Both amplitude estimation and quantum counting need controlled versions of
reflection operators. The Grover operator contains Hadamard gates (in the
diffuser), which our `controlled()` function doesn't support. Naively, we'd
need to extend `controlled()` to handle H, rotations, etc.

But there's a shortcut. The standard construction for phase oracles uses:

```
reflection = U · MCZ · U†
```

where U is a basis-change and MCZ is the phase flip. The conjugation identity:

```
controlled-(U · MCZ · U†) = U · controlled-MCZ · U†
```

**Proof**: When control = |0⟩: U · I · U† = I (identity). When control = |1⟩:
U · MCZ · U† (the desired operation). ✓

We only need to control the MCZ — not the Hadamards, X gates, or adder
circuits. Controlled-MCZ on n qubits with 1 additional control qubit is
MCZ on n+1 qubits.

**Important**: This pattern covers the standard compute-MCZ-uncompute
construction used by all three built-in oracles, but is not universal to all
possible phase oracles. Oracles using other phase-flip mechanisms (e.g.,
controlled rotations, diagonal unitaries) would be valid `Oracle` implementations
but cannot implement `Decomposable`. This is why `Decomposable` is a separate
supertrait.

### Implementation: controlled_multi_cz

The current `build_multi_cz` takes `&QubitRange` (contiguous indices). The
controlled version needs MCZ on non-contiguous qubits (counting qubit ∪ data
qubits). Two options:

1. **Generalize `build_multi_cz`** to accept `&[Qubit]` (like `build_multi_cx`
   already does), then pass `[ctrl] + data.to_qubits()`
2. **Build `controlled_multi_cz(control, qubits, scratch)`** that weaves the
   control qubit into the V-chain decomposition directly

Option 1 is simpler and consistent with the `multi_cx` API. The V-chain
decomposition is qubit-index-agnostic — it only needs to address individual
qubits, not assume contiguity.

### Verification: all Grover components have this form

**Diffuser** (build_diffuser):

```
H⊗n · X⊗n · MCZ(data) · X⊗n · H⊗n
 └── U ──┘               └ U† ┘
```

controlled-diffuser = H⊗n · X⊗n · MCZ(data ∪ {ctrl}) · X⊗n · H⊗n ✓

**IndexOracle** (single target): X(zero-bits) · MCZ(data) · X(zero-bits) ✓

**IndexOracle** (multi-target): Product of independent reflections
`(X₁·MCZ·X₁)·(X₂·MCZ·X₂)·…`. Each target decomposes independently —
`decompose()` returns a `Vec<OracleDecomposition>`, and the controlled
version applies the conjugation trick to each. Alternatively, multi-target
`IndexOracle` already knows `num_solutions`, making `Decomposable` optional
(callers use `try_search_with_oracle` directly).

**CnfOracle**: clause_eval · MCZ(clause_anc) · inverse(clause_eval) ✓

**SubsetSumOracle**: (adder · X_eq) · MCZ(sum_reg) · (X_eq · inverse(adder)) ✓

## Oracle trait extension

`decompose()` lives on a separate `Decomposable` supertrait, not on `Oracle`.
This gives compile-time enforcement: `quantum_counting<O: Decomposable>()`
won't compile if the oracle doesn't support decomposition. No runtime errors,
no `Option` wrapping.

```rust
pub struct OracleDecomposition {
    /// Basis-change circuit (the U in U · MCZ · U†).
    /// Must be unitary. Must leave mcz_scratch qubits in |0⟩.
    pub compute: Circuit,
    /// Qubit range for the MCZ phase flip.
    pub mcz_qubits: QubitRange,
    /// Scratch qubits for the uncontrolled MCZ.
    /// The controlled version may need more scratch
    /// (required_ancillas(mcz_qubits.len() + 1) vs
    ///  required_ancillas(mcz_qubits.len())).
    /// The quantum counting circuit allocates the difference.
    pub mcz_scratch: QubitRange,
}

/// Core oracle trait. Required for Grover search.
pub trait Oracle {
    fn num_data_qubits(&self) -> usize;
    fn num_ancillas(&self) -> usize;
    fn num_solutions(&self) -> Option<NonZeroUsize>;
    fn apply(&self, circuit: &mut Circuit, data: &QubitRange, anc: &QubitRange);
}

/// Oracles that expose their compute/MCZ structure.
/// Required for quantum counting and amplitude estimation.
///
/// Not all valid phase oracles can implement this trait — only those
/// using the standard compute-MCZ-uncompute construction.
pub trait Decomposable: Oracle {
    fn decompose(&self, data: &QubitRange, anc: &QubitRange)
        -> OracleDecomposition;
}

/// Build an oracle circuit from its decomposition.
/// Shared helper — all Decomposable oracles can delegate apply() here.
pub fn apply_decomposed(
    oracle: &dyn Decomposable,
    circuit: &mut Circuit,
    data: &QubitRange,
    anc: &QubitRange,
) {
    let d = oracle.decompose(data, anc);
    let mut action = Circuit::new();
    action += build_multi_cz(&d.mcz_qubits, &d.mcz_scratch);
    *circuit += within_apply(&d.compute, &action).unwrap();
}
```

### What each function requires

```rust
// Grover search: any oracle
fn try_search_with_oracle<O: Oracle>(...) -> Result<GroverResult, GroverError>

// Quantum counting: decomposable only (compile-time check)
fn quantum_counting<O: Decomposable>(...) -> CountingResult

// Auto search: decomposable only
fn auto_search<O: Decomposable>(...) -> GroverResult
```

### Two authoring paths

| Path | Implements | Grover search | Quantum counting |
|---|---|---|---|
| **Quick & dirty** | `Oracle` only | ✓ | ✗ (won't compile) |
| **Full support** | `Oracle` + `Decomposable` | ✓ | ✓ |

For full-support oracles, `apply()` delegates via the shared helper:

```rust
impl Oracle for CnfOracle {
    fn apply(&self, circuit: &mut Circuit, data: &QubitRange, anc: &QubitRange) {
        apply_decomposed(self, circuit, data, anc);
    }
    // ... other Oracle methods ...
}

impl Decomposable for CnfOracle {
    fn decompose(&self, data: &QubitRange, anc: &QubitRange)
        -> OracleDecomposition
    {
        let (clause_anc, scratch) = anc.split_at(c);
        let mut compute = Circuit::new();
        // ... build clause evaluation ...
        OracleDecomposition {
            compute,
            mcz_qubits: clause_anc,
            mcz_scratch: scratch.slice(..mcz_scratch_needed),
        }
    }
}
```

Note: SubsetSumOracle's `decompose()` must move the equality-check X gates
from the action into the compute circuit (X is self-inverse, so the inverse
of `compute · X_eq` is `X_eq · inverse(compute)`, matching the original).

All three built-in oracles (IndexOracle, CnfOracle, SubsetSumOracle) implement
both traits. Third-party oracles can implement just `Oracle` for basic search.

User code:

```rust
// Works — CnfOracle implements Decomposable
let oracle = CnfOracle::new(3, &clauses);
let count = quantum_counting(3, 8, &oracle, &runner);

// Won't compile — MyQuickOracle only implements Oracle
let oracle = MyQuickOracle::new();
let count = quantum_counting(3, 8, &oracle, &runner);
//                                   ^^^^^^ error: Decomposable not implemented
```

## Diffuser decomposition

The diffuser follows the same pattern:

```rust
fn build_controlled_diffuser(
    circuit: &mut Circuit,
    control: Qubit,
    data_qubits: &QubitRange,
    ancillas: &QubitRange,
) {
    // Compute: H then X on all data qubits (applied unconditionally)
    let mut compute = Circuit::new();
    for q in data_qubits.iter() {
        compute += Hadamard::new(q.index());
    }
    for q in data_qubits.iter() {
        compute += PauliX::new(q.index());
    }

    // Action: MCZ with control qubit included
    let mut action = Circuit::new();
    action += controlled_mcz(data_qubits, ancillas, control);

    *circuit += within_apply(&compute, &action).unwrap();
}
```

## Qubit layout (quantum counting)

```
[counting (t)] [data (n)] [diffuser scratch (d')] [oracle scratch (a')]
```

### Scratch budget for controlled mode

The controlled MCZ operates on n+1 qubits (original + control), which may
require more scratch than the uncontrolled version. The step function in
`required_ancillas` makes this critical at boundary values:

| MCZ qubits (n) | Scratch (uncontrolled) | Scratch (controlled, n+1) | Delta |
|---|---|---|---|
| 2 | 0 | 0 | 0 |
| 3 | 0 | **2** | **+2** |
| 4 | 2 | 3 | +1 |
| 5 | 3 | 4 | +1 |

Allocation rules:
- **Diffuser scratch** d' = `required_ancillas(n + 1)` (not `required_ancillas(n)`)
- **Oracle scratch** a' = `max(oracle.num_ancillas(), oracle_ancilla_count +
  required_ancillas(oracle_mcz_count + 1))` where `oracle_mcz_count` comes
  from `decompose().mcz_qubits.len()`

The oracle's `decompose()` returns scratch sized for the uncontrolled MCZ.
The quantum counting builder must allocate additional scratch for the
controlled case. This is computed at circuit construction time by inspecting
`decompose().mcz_qubits.len()`.

## Implementation plan

### Phase 1: Decomposable trait + Oracle refactor

Add `Decomposable` supertrait, `OracleDecomposition` struct, and
`apply_decomposed()` helper to the grover module. Implement `Decomposable`
for CnfOracle and SubsetSumOracle — mechanical refactor extracting compute
circuit and MCZ targets from existing `apply()` bodies. For SubsetSumOracle,
move equality-check X gates from action into compute.

For IndexOracle: implement `Decomposable` for single-target case. Multi-target
IndexOracle already knows `num_solutions` — defer decomposition support
(or return `Vec<OracleDecomposition>` if needed later).

Each oracle's `apply()` delegates via `apply_decomposed()`. All existing
tests pass unchanged. Validate compute circuit unitarity with `is_unitary()`
assertion in `apply_decomposed()`.

### Phase 2: Generalize build_multi_cz to &[Qubit]

Change `build_multi_cz` signature from `(&QubitRange, &QubitRange)` to
`(&[Qubit], &[Qubit])`, matching `build_multi_cx`'s existing convention.
This enables non-contiguous qubit sets needed for controlled-MCZ.

Callers that pass `QubitRange` use `.to_qubits()` (same bridge as multi_cx
callers). The V-chain decomposition is already qubit-index-agnostic.

### Phase 3: Controlled Grover operator

Add `build_controlled_diffuser(circuit, control, data, scratch)` using the
conjugation trick. Add `controlled_oracle()` helper that calls
`oracle.decompose()` and extends the MCZ with the control qubit.

Both must allocate scratch for `required_ancillas(mcz_count + 1)`, not
the uncontrolled budget. Extract `build_controlled_reflection(compute,
mcz_qubits, control, scratch)` as a shared utility in `circuits/` for
reuse by both the Grover module and amplitude estimation.

### Phase 4: Amplitude estimation module

Build `amplitude_estimation.rs` with `estimate_amplitude()`. General algorithm:
QPE on Q = A·S₀·A†·Sχ. Uses the shared `build_controlled_reflection` utility
for both S₀ (reflection about |0⟩) and Sχ (marker). S₀ is built internally
from `problem.num_data_qubits` — compute = X⊗n, MCZ on data qubits.

Validate `counting_bits >= 2` and `marker_compute` unitarity at entry.

### Phase 5: Quantum counting + auto-tuned search

Build `grover/counting.rs` with `quantum_counting<O: Decomposable>()`. Thin
wrapper: constructs state prep (H⊗n), builds marker from `oracle.decompose()`,
delegates to `estimate_amplitude()`, converts phase → solution count via
M = N·sin²(π·φ). Clamp M̂ to [0, N].

Build `auto_search<O: Decomposable>()`:
- Fast-path: if `oracle.num_solutions().is_some()`, skip counting
- Clamp M̂ ≥ 1 or return `GroverError::NoSolutions` when M̂ < 1
- Cap iterations at `⌊π/4·√N⌋` (M=1 upper bound) for robustness
- Run Grover search with computed iteration count

## Comparison with Qiskit

Qiskit implements quantum counting by calling `.control()` on the `GroverOperator`:

```python
grover_op = GroverOperator(oracle)
controlled_grover = grover_op.control(1)  # generic method
```

Under the hood, `.control()` treats the Grover operator as an opaque unitary:
1. Decomposes the circuit into elementary gates
2. Builds the full unitary matrix (2^n × 2^n)
3. Synthesizes a controlled version via matrix decomposition (Quantum Shannon
   Decomposition / Barenco et al.)
4. Outputs a CNOT + rotation sequence

This is general but expensive — it works for any unitary but doesn't exploit
the circuit's internal structure, producing O(4^n) gates.

Our approach uses the conjugation trick to control only the MCZ, adding O(1)
gate overhead:

| | Qiskit `.control()` | Our conjugation trick |
|---|---|---|
| **Works on** | Any unitary | Only `U · MCZ · U†` structure |
| **Gate overhead** | O(4^n) — matrix synthesis | O(1) — extend the MCZ by 1 qubit |
| **Exploits structure** | No | Yes |
| **User effort** | Zero (call `.control()`) | Zero if oracle implements `Decomposable` |
| **Scalability** | Tiny circuits only | Same as uncontrolled version |

This is consistent with qcfront's design philosophy: algorithm-aware
implementations over generic black-box methods. Qiskit's approach is "it just
works" but doesn't scale. Ours exploits the standard compute-MCZ-uncompute
structure of phase oracles to produce efficient circuits.

## References

- Brassard, Høyer, Mosca, Tapp (2000). "Quantum Amplitude Amplification and
  Estimation." arXiv:quant-ph/0005055
- Brassard, Høyer, Tapp (1998). "Quantum Counting." arXiv:quant-ph/9805082
- Nielsen & Chuang, Section 6.3: "Quantum counting"
- Qiskit: `qiskit.algorithms.AmplitudeEstimation`
- Q#: `Microsoft.Quantum.AmplitudeEstimation`
