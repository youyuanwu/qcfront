# Feature: Qubit Type Safety

Type-safe qubit handles and allocation, replacing raw `usize` indices
throughout the codebase. **Status: implemented.**

## Problem Solved

Raw `usize` qubit indices allowed two bug classes:

1. **Qubit collision** — registers accidentally overlap with no detection
2. **Dangling indices** — index refers to nothing, simulator allocates
   more qubits than intended

Argument swap (`data` vs `ancillas`) is **not** prevented — both are
the same type. Phantom-typed ranges would fix this but add
disproportionate complexity.

## Types

### `Qubit`

Zero-cost newtype wrapping `usize`. Cannot be created outside the
crate — only via `QubitAllocator` or `Qubit::from_raw()` (crate-internal,
used by circuit transforms that extract indices from roqoqo gates).

`.index()` unwraps to `usize` at the roqoqo gate boundary.

### `QubitRange`

A contiguous range of qubits: `start + len`, no heap allocation.
`qubit(i)` computes `Qubit(start + i)` on the fly.

Key methods:
- `qubit(i) -> Qubit` — access by index (panics if OOB)
- `iter() -> impl Iterator<Item = Qubit>` — iterate
- `slice(range) -> QubitRange` — sub-range extraction
- `split_at(mid) -> (QubitRange, QubitRange)` — split in two
- `to_qubits() -> Vec<Qubit>` — materialize for `&[Qubit]` APIs

No `Index<usize>` trait — Rust requires `&T` return, but we compute
values on the fly. Use `.qubit(i)` instead.

### `QubitAllocator`

Bump allocator producing disjoint `QubitRange`s. `checked_add`
prevents overflow. One allocator per circuit construction.

## API Split

Primitives take different types based on calling patterns:

| Function | Qubit params | Why |
|----------|-------------|-----|
| `build_multi_cz` | `&QubitRange, &QubitRange` | Always contiguous register data |
| `build_diffuser` | `&QubitRange, &QubitRange` | Same |
| `build_multi_cx` | `Qubit, &[Qubit], &[Qubit]` | Ad-hoc control lists (adder, SAT) |
| `controlled_add` | `Qubit, &[Qubit], &[Qubit]` | Mixed sources in inner loop |
| `controlled_modmul_15` | `Qubit, &[Qubit]` | Work register as slice |
| `Oracle::apply` | `&QubitRange, &QubitRange` | Named data + ancilla ranges |
| `build_qpe_circuit` | Allocator internal, callback `Qubit` | QPE owns counting qubits |
| `transform::controlled` | `Qubit, &[Qubit]` | Inlined V-chain at roqoqo boundary |

`to_qubits()` bridges `QubitRange` → `&[Qubit]` where needed (e.g.,
`SubsetSumOracle` passing sub-ranges to `controlled_add`).

## Scratch Sharing

`SubsetSumOracle` shares scratch qubits between temporally disjoint
phases via `m + max(adder_scratch, mcz_scratch)`. The allocator only
governs top-level allocation; oracles manage internal layout via
`split_at()` / `slice()`. No `alias()` mechanism needed.

## Coverage

| Level | Protection |
|-------|-----------|
| Allocation (`QubitAllocator`) | Disjoint by construction |
| Function boundaries (`&QubitRange`, `Qubit`) | Can't mix qubit with integer |
| Inside oracles (`slice`, `split_at`) | Sub-range extraction replaces raw index math |
| Gate construction (`.index()`) | No protection — roqoqo boundary |

## Not Implemented

- **Argument-swap prevention** — phantom-typed ranges
- **Automatic qubit recycling** — circuits are built ahead of time
- **Qubit borrowing** — overkill for upfront allocation pattern
