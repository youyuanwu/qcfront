# qcfront

Quantum computing algorithms in Rust, built on [roqoqo](https://github.com/HQSquantumsimulations/qoqo) + [QuEST](https://quest.qtechtheory.org/) simulator.

## Crates

| Crate | Description |
|-------|-------------|
| `algos` | Algorithm library — Grover search, Shor factoring, QPE, state preparation |
| `examples` | Runnable demos and Azure Quantum integration |

## Algorithms

**Grover's Search** — O(√N) unstructured search with pluggable oracles:
- `IndexOracle` — mark states by index
- `CnfOracle` — SAT solving via reversible De Morgan circuit
- `SubsetSumOracle` — subset sum via controlled adder + equality check

**Shor's Factoring** — period-finding via QPE, demonstrated on N=15.

**Quantum Phase Estimation** — generic QPE circuit builder.

**State Preparation** — arbitrary amplitude encoding.

## Quick Start

```bash
cargo test                              # run all 152 tests
cargo run -p examples --bin grover      # Grover search demo
cargo run -p examples --bin sat_grover  # SAT solver
cargo run -p examples --bin subset_sum  # subset sum solver
cargo run -p examples --bin shor_15     # factor 15
```

## Circuit Primitives

Reusable building blocks in `algos/src/circuits/`:

| Module | Gate | Use |
|--------|------|-----|
| `multi_cz` | Multi-controlled-Z (V-chain) | Diffuser, phase oracles |
| `multi_cx` | Multi-controlled-X (generalized Toffoli) | Boolean functions |
| `adder` | Controlled classical adder (MCX-cascade) | Arithmetic oracles |
| `modmul_15` | Controlled modular multiplication | Shor's algorithm |

## Project Structure

```
crates/
  algos/src/
    grover/          Grover search + Oracle trait + oracles
    sat/             Literal, Clause, evaluate_cnf
    circuits/        Reusable gate decompositions
    shor.rs          Shor's factoring
    qpe.rs           Quantum phase estimation
    state.rs         State preparation
    runner.rs        QuantumRunner trait
  examples/src/bin/  Runnable demos
docs/              Published mdBook site sources (see book.toml)
notes/             Working design notes and research
  theory/            Algorithm theory (Grover, gates, Bloch sphere)
  features/          Implementation docs (API, architecture)
  background/        Framework and hardware research
```

## Backend

Uses roqoqo for circuit construction and roqoqo-quest (QuEST
statevector simulator) for execution. The `QuantumRunner` trait
abstracts the backend — an Azure Quantum runner is included for
cloud execution.

## License

MIT
