# Feature: Grover's Search Algorithm

For the theory (how the algorithm works, why it's fast), see
[docs/theory/Grover.md](../theory/Grover.md).

## Status

**Phase 1 (Core) ✅** — `algos/src/grover.rs`: `search()`,
`GroverConfig`, `GroverResult`.

**Phase 2 (Scaling n ≥ 4) ✅** — `algos/src/circuits/multi_cz.rs`:
Barenco V-chain decomposition with `required_ancillas()` helper.

**Phase 3 (Oracle Trait) ✅** — `Oracle` trait with `IndexOracle`
(marks by index) and `CnfOracle` (reversible SAT circuit).
`try_search_with_oracle()` returns `Result`.

**Phase 4 (SAT Solver) ✅** — `algos/src/sat.rs`: `CnfOracle` with
De Morgan circuit construction, clause canonicalization,
`evaluate_cnf()` free function. `circuits/multi_cx.rs` for MCX.

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
be restored to |0⟩ after each application. See trait doc for full
contract.

### Implementations

| Type | Marks by | `num_solutions()` | Use case |
|------|----------|:---:|----------|
| `IndexOracle` | State index (X+MCZ+X per target) | `Some(m)` | Testing, known targets |
| `CnfOracle` | Reversible CNF evaluation | `None` | SAT solving, quantum advantage |

### Search Functions

```rust
// Simple: search for a known target
let result = search(&config, target, &runner);

// Generic: any Oracle, returns Result
let result = try_search_with_oracle(&config, &oracle, &runner)?;
```

### SAT Utilities

```rust
// Build circuit-based oracle (no classical pre-solving)
let oracle = CnfOracle::new(num_vars, &clauses);

// Classical verification of results
let is_sat = evaluate_cnf(&clauses, assignment);
```

`CnfOracle::new()` canonicalizes clauses: deduplicates literals, drops
tautological clauses (`x ∨ ¬x`).

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

MCX scratch reuses across sequential clauses. MCZ scratch reuses the
same pool (non-overlapping temporally).

## Files

| File | Contents |
|------|----------|
| `crates/algos/src/grover.rs` | `Oracle` trait, `IndexOracle`, `GroverConfig/Result/Error`, `search`, `try_search_with_oracle` |
| `crates/algos/src/sat.rs` | `Literal`, `CnfOracle`, `evaluate_cnf` |
| `crates/algos/src/circuits/multi_cz.rs` | `build_multi_cz`, `required_ancillas` |
| `crates/algos/src/circuits/multi_cx.rs` | `build_multi_cx`, `required_ancillas` |
| `crates/examples/src/bin/grover.rs` | Single-target and multi-target demos |
| `crates/examples/src/bin/sat_grover.rs` | CNF SAT → CnfOracle → Grover |

## Tests (124 total across workspace)

- Grover: 2-qubit (all targets), 3-qubit, explicit iterations,
  custom Oracle trait, IterationsRequired error
- IndexOracle: single/multi, dedup, panics (range, empty, shots)
- CnfOracle: evaluation, canonicalization, ancilla budget, Grover
  integration (single + multi-solution SAT)
- MCZ/MCX: 1–5 qubit decomposition, ancilla uncomputation, CCZ symmetry

## roqoqo Gate Conventions

- `Toffoli::new(target, ctrl1, ctrl2)` — first arg is **target**
- `ControlledControlledPauliZ::new(q0, q1, q2)` — symmetric
- `CNOT::new(control, target)` — standard ordering

## Open Questions

- **Ancilla sharing**: Disjoint pools waste qubits for `IndexOracle`
  (duplicates diffuser MCZ scratch). A shared pool of
  `max(diffuser, oracle)` would suffice. Trade-off: clarity vs qubits.

- **Unsatisfiable formulas**: `CnfOracle` on UNSAT produces
  near-uniform distribution with no error signal. Consider
  `GroverResult::is_likely_unsatisfiable()`.

- **Variable density**: `Literal::qubit()` assumes dense 1..=num_vars.
  Sparse DIMACS needs a remapping pass.

- **Unit propagation**: Not implemented. Separable optimization.
