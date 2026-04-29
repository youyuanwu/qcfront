# Feature: Circuit Utilities

Higher-level circuit manipulation in `circuits/transform.rs`, inspired
by Qiskit's `circuit.inverse()` / `gate.control()` and Q#'s `Adjoint` /
`Controlled` / `within { } apply { }`.

## Status

**Implemented ✅** — `inverse()`, `within_apply()`, `is_unitary()`,
`controlled()`, `controlled_scratch_required()`.

## API

### `inverse(circuit) -> Result<Circuit, UnsupportedGate>`

Reverses gate order, replaces each gate with its inverse. Self-inverse
gates (X, CNOT, Toffoli, CZ, CCZ, H) are cloned; rotation gates negate
the angle. Returns `Err` for non-unitary or unsupported gate types.

```rust
let inv = inverse(&compute_circuit)?;
```

### `within_apply(compute, action) -> Result<Circuit, UnsupportedGate>`

Automates compute → action → uncompute: `compute + action + inverse(compute)`.
The compute circuit must be unitary (`debug_assert`). The action must not
permanently modify qubits that compute depends on.

```rust
*circuit += within_apply(&compute, &action)?;
```

### `controlled(circuit, ctrl, scratch) -> Result<Circuit, UnsupportedGate>`

Adds a control qubit to every gate. Promotion chain:
- `X → CNOT`
- `CNOT → Toffoli`
- `Toffoli → MCX (3 controls)`

Scoped to X/CNOT/Toffoli only. Use `controlled_scratch_required(circuit)`
to compute scratch size.

### `is_unitary(circuit) -> bool`

Returns false if any non-unitary operation is present. Structural check,
not matrix verification.

## Where Used

| Utility | Used by |
|---------|---------|
| `inverse()` | Adder tests (replaces deleted `controlled_add_inverse`) |
| `within_apply()` | `CnfOracle::apply`, `SubsetSumOracle::apply`, `apply_target_oracle`, `build_diffuser` |
| `is_unitary()` | `within_apply` precondition |
| `controlled()` | Available for future oracles |

## Gate Inverse Table

| Gate | Inverse | Self-inverse? |
|------|---------|:---:|
| PauliX, PauliY, PauliZ | Same | ✅ |
| Hadamard | Hadamard | ✅ |
| CNOT | CNOT | ✅ |
| Toffoli | Toffoli | ✅ |
| ControlledPauliZ | ControlledPauliZ | ✅ |
| ControlledControlledPauliZ | ControlledControlledPauliZ | ✅ |
| RotateX/Y/Z(θ) | RotateX/Y/Z(−θ) | ❌ |
| PhaseShiftState1(θ) | PhaseShiftState1(−θ) | ❌ |
| SqrtPauliX | InvSqrtPauliX | ❌ |

Non-unitary operations (DefinitionBit, MeasureQubit, pragmas) return
`Err(UnsupportedGate)`.

## Gaps vs Qiskit

| Feature | Qiskit | qcfront | Gap reason |
|---------|--------|---------|------------|
| `inverse()` | Any gate (each class implements `.inverse()`) | Explicit gate table | We don't own roqoqo types |
| `control(n)` | Any gate (unitary decomposition) | X/CNOT/Toffoli only | General decomposition needs matrix algebra |
| `control(n)` ancillas | Automatic | Caller provides scratch | Keeps allocation explicit |
| `within/apply` | No equivalent | `within_apply()` | **We go beyond Qiskit** (Q# has this) |
| `is_unitary()` | Full matrix verification | Structural check | Cheap, sufficient |

## Tests (21 in `circuits::transform`)

- `is_unitary`: pure gates, with measurement, empty
- `inverse`: X roundtrip, CNOT/Toffoli, RotateZ, rejects non-unitary,
  unsupported gate, double-inverse roundtrip, exhaustive adder
- `within_apply`: basic compute/action, adder roundtrip, empty compute
- `controlled`: X→CNOT, CNOT→Toffoli, Toffoli→MCX, scratch sizing,
  unsupported gate, multi-gate circuit, controlled+inverse roundtrip

## References

- Qiskit [`QuantumCircuit.inverse()`](https://docs.quantum.ibm.com/api/qiskit/qiskit.circuit.QuantumCircuit#inverse)
- Qiskit [`Gate.control()`](https://docs.quantum.ibm.com/api/qiskit/qiskit.circuit.Gate#control)
- Q# [Adjoint / Controlled functors](https://learn.microsoft.com/en-us/azure/quantum/user-guide/language/expressions/functorapplication)
- Q# [within { } apply { }](https://learn.microsoft.com/en-us/azure/quantum/user-guide/language/statements/conjugations)
