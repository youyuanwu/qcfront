# Feature: Grover's Search Algorithm

For the theory (how the algorithm works, why it's fast), see
[docs/theory/Grover.md](../theory/Grover.md).

## Status

**Phase 1 (Core) ✅** — `search()`, `GroverConfig`, `GroverResult`.

**Phase 2 (Scaling n ≥ 4) ✅** — `circuits/multi_cz.rs`: Barenco
V-chain decomposition with `required_ancillas()` helper.

**Phase 3 (Oracle Trait) ✅** — `Oracle` trait with `IndexOracle`
and `CnfOracle`. `try_search_with_oracle()` returns `Result`.

**Phase 4 (SAT Solver) ✅** — `CnfOracle` with De Morgan circuit,
clause canonicalization, `evaluate_cnf()`. `circuits/multi_cx.rs`.

**Phase 5 (Subset Sum) ✅** — `SubsetSumOracle` with MCX-cascade
controlled adder + X-MCZ-X equality comparator. `circuits/adder.rs`.
`verify_subset_sum()` for classical verification.

## Public API

### Oracle Trait

```rust
pub trait Oracle {
    fn num_data_qubits(&self) -> usize;
    fn num_ancillas(&self) -> usize;
    fn num_solutions(&self) -> Option<NonZeroUsize>;
    fn apply(&self, circuit: &mut Circuit, data_qubits: &[usize], ancillas: &[usize]);
}
```

Phase oracle invariant: `O|x⟩|0⟩ = (-1)^f(x)|x⟩|0⟩`. Ancillas must
be restored to |0⟩ after each application.

### Implementations

| Type | Marks by | `num_solutions()` | Use case |
|------|----------|:---:|----------|
| `IndexOracle` | State index (X+MCZ+X per target) | `Some(m)` | Testing, known targets |
| `CnfOracle` | Reversible CNF evaluation | `None` | SAT solving |
| `SubsetSumOracle` | Sum accumulator + equality check | `None` | Subset sum |

### Search Functions

```rust
// Simple: search for a known target
let result = search(&config, target, &runner);

// Generic: any Oracle, returns Result
let result = try_search_with_oracle(&config, &oracle, &runner)?;
```

### Verification Utilities

```rust
// SAT: classical check of CNF assignment
let is_sat = evaluate_cnf(&clauses, assignment);

// Subset sum: returns selected elements if valid, None otherwise
let selected = verify_subset_sum(&elements, target, measured_state);
```

## Architecture

```
search() ─────────────────────────────┐
try_search_with_oracle<O: Oracle>() ──┤
                                      ▼
                        build_grover_circuit()
                              │
            ┌─────────────────┼──────────────────┐
            ▼                 ▼                  ▼
    H on all qubits    oracle.apply()    build_diffuser()
                       │                 │
                       ▼                 ▼
                IndexOracle:         H·X·MCZ·X·H
                  X+MCZ+X
                CnfOracle:
                  De Morgan circuit
                SubsetSumOracle:
                  controlled_add → X-MCZ-X → uncompute
```

### Qubit Layout (disjoint pools)

```
|← data (n) →|← diffuser MCZ (d) →|← oracle ancillas (a) →|
  qubits 0..n     qubits n..n+d        qubits n+d..n+d+a
```

The diffuser and oracle each get their own MCZ/MCX scratch qubits.
No sharing — clear ownership, no ambiguity.

### CnfOracle Ancilla Budget

```
clause_ancillas = c
mcx_scratch     = max over clauses of required_ancillas(clause.len())
final_mcz       = required_ancillas(c)
total           = c + max(mcx_scratch, final_mcz)
```

### SubsetSumOracle Ancilla Budget

```
sum_reg   = m = ⌈log₂(Σsᵢ + 1)⌉
scratch   = max(0, m-2)     // shared MCX + MCZ pool
total     = m + max(0, m-2)
```

MCX scratch (controlled additions) and MCZ scratch (equality check)
are temporally disjoint — share a single pool. Implementation must
slice to exact size before passing to `build_multi_cz`.

## SubsetSumOracle

### Circuit Strategy

Compute → phase-flip → uncompute:

1. **Compute**: for each element, `controlled_add(data[i], sum, scratch, sᵢ)`
2. **Phase flip**: X-MCZ-X equality check (sum == T)
3. **Uncompute**: reverse additions in reverse order

The controlled adder uses an MCX-cascade incrementer — for each
set bit of k, a carry cascade from MSB down flips the sum register
in-place. No carry register needed.

### Constructor Contract

```rust
/// # Panics
/// - If `elements.len() < 2` (Grover requires n ≥ 2)
/// - If all elements are zero (empty sum register)
/// - If `target == 0` (trivially solved by empty subset)
/// - If `target > sum of elements` (provably impossible)
pub fn new(elements: &[u64], target: u64) -> Self
```

### Qubit Scaling

| Elements | n | Total qubits | Notes |
|----------|---|---|---|
| `[3, 5, 7]` | 3 | 9 | instant |
| `[2, 3, 5, 7]` | 4 | 14 | fast |
| `[1, 2, 3, 5, 7]` | 5 | 16 | fine |
| `[1, 2, 3, 4, 5, 6]` | 6 | 18 | fine |
| `[1, 2, 3, 4, 5, 6, 7]` | 7 | 20 | borderline |

Gate count: O(nm³) per oracle invocation, O(nm³·√(2ⁿ/M)) total.

### References

- Cuccaro et al., [arXiv:quant-ph/0410184](https://arxiv.org/abs/quant-ph/0410184)
  — ripple-carry adder with MAJ/UMA primitives
- Draper, [arXiv:quant-ph/0008033](https://arxiv.org/abs/quant-ph/0008033)
  — QFT-based adder (future optimization path)
- Qiskit `CDKMRippleCarryAdder`, `DraperQFTAdder`

## Files

| File | Contents |
|------|----------|
| `grover/mod.rs` | `Oracle` trait, `GroverConfig/Result/Error`, `search`, `try_search_with_oracle` |
| `grover/index.rs` | `IndexOracle` (X+MCZ+X marking) |
| `grover/sat.rs` | `CnfOracle` (De Morgan circuit) |
| `grover/subset_sum.rs` | `SubsetSumOracle`, `verify_subset_sum` |
| `sat/mod.rs` | `Literal`, `Clause`, `evaluate_cnf` |
| `circuits/multi_cz.rs` | `build_multi_cz`, `required_ancillas` |
| `circuits/multi_cx.rs` | `build_multi_cx`, `required_ancillas` |
| `circuits/adder.rs` | `controlled_add`, `controlled_add_inverse`, `required_scratch` |
| `examples/.../grover.rs` | Single/multi-target demos |
| `examples/.../sat_grover.rs` | CNF SAT → CnfOracle → Grover |
| `examples/.../subset_sum.rs` | Subset sum → SubsetSumOracle → Grover |

## Tests (152 total across workspace)

- Grover core: 2-qubit, 3-qubit, explicit iterations, custom Oracle,
  IterationsRequired error
- IndexOracle: single/multi, dedup, panics
- CnfOracle: evaluation, canonicalization, ancilla budget, Grover
  integration
- SubsetSumOracle: constructor validation (4 panic cases), ancilla
  budget, Grover integration (single/multi solution, duplicates),
  verify_subset_sum
- Adder: concrete additions, carry propagation, control-off, boundary
  m=1/2/3, inverse roundtrip, exhaustive m=3, scratch reset
- MCZ/MCX: 1–5 qubit decomposition, ancilla uncomputation, CCZ symmetry

## roqoqo Gate Conventions

- `Toffoli::new(target, ctrl1, ctrl2)` — first arg is **target**
- `ControlledControlledPauliZ::new(q0, q1, q2)` — symmetric
- `CNOT::new(control, target)` — standard ordering

## Open Questions

- **Ancilla sharing**: Disjoint pools waste qubits for `IndexOracle`.
  A shared pool of `max(diffuser, oracle)` would suffice.

- **Unsatisfiable/impossible instances**: Both `CnfOracle` on UNSAT and
  `SubsetSumOracle` on no-solution produce near-uniform distributions
  with no error signal. Consider `GroverResult::is_likely_unsatisfiable()`.

- **Variable density**: `Literal::qubit()` assumes dense 1..=num_vars.

- **QFT adder**: Would reduce gate count from O(nm³) to O(nm²) for
  subset sum, enabling larger instances.
