# Feature: Circuit Utilities

Higher-level circuit manipulation utilities inspired by Qiskit and Q#,
built on top of roqoqo's gate-level API.

## Motivation

roqoqo is gate-level — every gate and qubit index is explicit. This
gives full control but leads to repetitive patterns across oracles:

- Every oracle writes compute → action → uncompute as 3 manual blocks
- Inverse circuits are hand-written copies with reversed gate/loop order
- Adding a control qubit means wrapping every gate individually
- Qubit index arithmetic is scattered through oracle constructors

Qiskit provides `circuit.inverse()`, `gate.control()`, and stdlib
adders. Q# goes further with `Adjoint`, `Controlled`, and
`within { } apply { }` as language primitives. We can capture the
most valuable patterns as Rust utilities in `circuits/`.

## Proposed Utilities

### 1. `inverse(circuit) -> Result<Circuit>` — Priority: High

Reverse gate order, replace each gate with its inverse.

```rust
pub fn inverse(circuit: &Circuit) -> Result<Circuit, UnsupportedGate>
```

Most gates we use are self-inverse (X, CNOT, Toffoli, CZ, CCZ).
Rotation gates negate the angle. The function pattern-matches on
roqoqo's `Operation` enum.

Returns `Result` instead of panicking — roqoqo may add new gate
types in minor versions (`roqoqo = "1"` allows any 1.x), so
callers must handle `UnsupportedGate` gracefully.

**Non-unitary operations** (`DefinitionBit`, `MeasureQubit`, pragmas)
are silently skipped — `inverse()` only reverses unitary gates. This
lets it safely process sub-circuits extracted from full measurement
circuits.

**Current pain**: `controlled_add_inverse` is a hand-written mirror
of `controlled_add`. CnfOracle and SubsetSumOracle both have explicit
uncompute loops that duplicate the compute phase in reverse.

**After**: `inverse(&compute_circuit)?` — one call, no duplication,
no risk of forgetting to reverse a gate.

**Scope**: all gate types emitted by our `circuits/` module — see
Gate Inverse Table below.

### 2. `within_apply(compute, action) -> Circuit` — Priority: High

Automates the compute → action → uncompute pattern.

```rust
pub fn within_apply(
    compute: &Circuit,
    action: &Circuit,
) -> Circuit
```

Produces: `compute + action + inverse(compute)`.

**Contract**: the action circuit must not permanently modify qubits
that the compute circuit depends on. Temporary modifications that
self-cancel (e.g., X-MCZ-X on sum qubits) are safe. Violations
produce silent wrong results — entangled garbage in ancillas.

**Current pain**: every oracle manually implements three phases.
The uncompute phase must exactly mirror the compute phase — a
subtle reversal bug produces silent wrong results (entangled
garbage in ancillas).

**After**:
```rust
let mut compute = Circuit::new();
// ... build compute phase ...

let mut action = Circuit::new();
// ... phase flip ...

circuit += within_apply(&compute, &action);
// uncompute is automatic and correct
```

**Impact on existing oracles**:

| Oracle | Current | After |
|--------|---------|-------|
| CnfOracle::apply | 3 loops (compute, MCZ, uncompute) | `within_apply(&clause_eval, &mcz)` |
| SubsetSumOracle::apply | 3 loops (add, X-MCZ-X, inverse add) | `within_apply(&additions, &eq_check)` |
| Any future oracle | Copy-paste the 3-phase pattern | One call |

### 3. `controlled(circuit, ctrl) -> Result<Circuit>` — Priority: Medium

Add a control qubit to every gate in a circuit.

```rust
pub fn controlled(
    circuit: &Circuit,
    control: usize,
    scratch: &[usize],
) -> Result<Circuit, UnsupportedGate>
```

Gate promotion chain:
```
X       → CNOT(ctrl, target)
CNOT    → Toffoli(target, ctrl, original_ctrl)
Toffoli → MCX with 3 controls (uses scratch)
```

**Scope limitation**: supports X, CNOT, and Toffoli promotion only.
Phase gates (Z, CZ, CCZ) are not supported — promoting CCZ requires
MCZ(4) decomposition which adds architectural complexity for limited
benefit. Returns `UnsupportedGate` for other gate types.

The primary use case is wrapping adder circuits (which only contain
CNOT and Toffoli from MCX decomposition).

**Scratch sizing**: callers need `controlled_scratch_required(circuit)`
to compute how much scratch the promoted circuit needs:

```rust
pub fn controlled_scratch_required(circuit: &Circuit) -> usize
```

Scans the circuit for the maximum control count after promotion and
returns the MCX ancilla requirement.

**Validation**: `debug_assert!` that the control qubit is not already
used in the circuit (prevents silent qubit collision bugs).

**Current pain**: `controlled_add` manually adds the control qubit
to every MCX control list. Building a controlled version of any
existing circuit requires rewriting it.

**After**: `controlled(&adder_circuit, data_qubit, &scratch)?`

### 4. `QubitAllocator` — Priority: Low

Track qubit indices and hand out fresh ranges.

```rust
let mut alloc = QubitAllocator::new();
let data = alloc.allocate(n);       // [0, 1, ..., n-1]
let sum = alloc.allocate(m);        // [n, n+1, ..., n+m-1]
let scratch = alloc.allocate(s);    // [n+m, ...]
let total = alloc.total();
```

**Current pain**: manual index arithmetic in every oracle constructor
and in `build_grover_circuit`. Error-prone when regions overlap.

**After**: allocator guarantees disjoint ranges. Lower priority because
the current approach works and the index math is straightforward.

**Location**: `src/alloc.rs` (not `circuits/`) — its consumers are
algorithms (`grover/`, `shor.rs`), not circuit primitives. The
allocator is a resource manager at a different abstraction level.

## Implementation Plan

| Phase | Utility | Location | Depends on |
|-------|---------|----------|------------|
| 1 | `inverse()` | `circuits/transform.rs` | — |
| 2 | `within_apply()` | `circuits/transform.rs` | `inverse()` |
| 3 | Refactor oracles to use `within_apply` | `grover/sat.rs`, `grover/subset_sum.rs` | phase 2 |
| 4 | `controlled()` + `controlled_scratch_required()` | `circuits/transform.rs` | — |
| 5 | `QubitAllocator` | `src/alloc.rs` | — |

`circuits/transform.rs` is a **circuit-to-circuit meta-utility** module,
distinct from the existing gate primitives (multi_cx, multi_cz, adder)
which are qubit-indices-to-circuit. Phase 3 tests should explicitly
cover X-MCX-X interleaving patterns from existing oracles.

## Gate Inverse Table

All gate types emitted by our `circuits/` module:

| Gate | Inverse | Self-inverse? | Emitted by |
|------|---------|:---:|---|
| PauliX | PauliX | ✅ | oracles, adder |
| PauliZ | PauliZ | ✅ | diffuser |
| Hadamard | Hadamard | ✅ | diffuser, QPE |
| CNOT | CNOT | ✅ | adder, multi_cx |
| Toffoli | Toffoli | ✅ | multi_cx V-chain |
| ControlledPauliZ | ControlledPauliZ | ✅ | multi_cz (n=2, V-chain) |
| ControlledControlledPauliZ | ControlledControlledPauliZ | ✅ | multi_cz (n=3) |
| RotateZ(θ) | RotateZ(−θ) | ❌ | QPE, state prep |
| PhaseShift(θ) | PhaseShift(−θ) | ❌ | QPE, state prep |
| SqrtPauliX | SqrtPauliX† | ❌ | state prep |

**Non-unitary operations** (DefinitionBit, MeasureQubit, pragmas)
are skipped by `inverse()` — they are not gates and have no inverse.

## Unitarity Check

`inverse()` and `within_apply()` assume their input is a unitary
circuit (only reversible gates). Rather than silently producing
wrong results on non-unitary input, add a validation helper:

```rust
pub fn is_unitary(circuit: &Circuit) -> bool
```

Scans operations and returns `false` if any non-unitary operation
is present (MeasureQubit, reset, classical conditionals). This is
a structural check (does it contain measurement ops?), not a matrix
verification.

Use it as a precondition:
```rust
debug_assert!(is_unitary(&compute), "within_apply requires unitary compute circuit");
```

Qiskit's `Operator.is_unitary()` does full matrix verification —
overkill for us. Our check is cheap (single pass, no simulation)
and catches the common mistake of passing a full measurement
circuit to `inverse()`.

## Gaps vs Qiskit

Deliberate scope limitations compared to Qiskit's equivalents:

| Feature | Qiskit | qcfront | Gap reason |
|---------|--------|---------|------------|
| `inverse()` | Any gate — each class implements `.inverse()` | Explicit gate table, `Result` for unknown | We don't own roqoqo's gate types |
| `control(n)` | Any gate — general unitary decomposition | X/CNOT/Toffoli only | General decomposition requires matrix algebra |
| `control(n)` ancillas | Automatic (transparent) | Caller provides scratch | Keeps allocation explicit |
| `compose(qubits)` | Append circuit at specific qubit mapping | `+=` only (no remapping) | Could add later if needed |
| `repeat(n)` | Repeat circuit n times | Not proposed | Trivial loop, low value |
| `depth()` | Circuit depth metric | Not proposed | Useful for optimization; add when needed |
| `count_ops()` | Gate count by type | Not proposed | Good for debugging; easy to add |
| `within/apply` | No equivalent | `within_apply()` | **We go beyond Qiskit** (Q# has this) |
| `is_unitary()` | Full matrix verification | Structural check (no measure/reset) | Cheap, sufficient for our needs |

The main architectural gap is `controlled()`: Qiskit decomposes
arbitrary unitaries into controlled form using matrix math. We
restrict to known gate promotions. This is intentional — general
decomposition is complex and our oracles only need controlled
X/CNOT/Toffoli circuits.

## References

- Qiskit `QuantumCircuit.inverse()`:
  [docs](https://docs.quantum.ibm.com/api/qiskit/qiskit.circuit.QuantumCircuit#inverse)
- Qiskit `Gate.control()`:
  [docs](https://docs.quantum.ibm.com/api/qiskit/qiskit.circuit.Gate#control)
- Q# `Adjoint` and `Controlled` functors:
  [docs](https://learn.microsoft.com/en-us/azure/quantum/user-guide/language/expressions/functorapplication)
- Q# `within { } apply { }`:
  [docs](https://learn.microsoft.com/en-us/azure/quantum/user-guide/language/statements/conjugations)
