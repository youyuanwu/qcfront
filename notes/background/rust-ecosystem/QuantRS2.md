# QuantRS2: Comprehensive Quantum Framework in Rust

## Overview

QuantRS2 is an ambitious pure-Rust quantum computing framework organized as
a multi-crate workspace. It aims to be a full-stack quantum toolkit covering
gate-based circuits, simulation, hardware integration, quantum ML, quantum
annealing, and error correction — all without C/C++/Fortran dependencies.

- **Repository**: https://github.com/cool-japan/quantrs
- **Crate prefix**: `quantrs2-*` on crates.io (v0.1.3)
- **License**: Apache-2.0
- **Size**: ~965K total LoC, 777K Rust LoC, 4,707 tests
- **Status**: Alpha

Note: The `quantrs` crate on crates.io is a separate quantitative finance
library. The quantum project is `quantrs2-*`.

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `quantrs2-core` | Core types, gates, traits, error correction, QML primitives, noise models |
| `quantrs2-circuit` | Circuit representation with const-generic qubit count (`Circuit<N>`) |
| `quantrs2-sim` | Simulators: state-vector (CPU/GPU), tensor network, stabilizer |
| `quantrs2-device` | Hardware connectors: IBM Quantum, Azure Quantum, AWS Braket |
| `quantrs2-ml` | Quantum machine learning: QNNs, GANs, HEP classifiers |
| `quantrs2-anneal` | Quantum annealing, QUBO/Ising problems |
| `quantrs2-tytan` | High-level annealing API |
| `quantrs2-py` | Python bindings via PyO3 |
| `quantrs2-symengine-pure` | Pure Rust symbolic computation |
| `quantrs2-examples` | Example programs |

## Simulation Backends

QuantRS2 uses its own simulation implementations (no external C/C++ engines):

| Backend | Method | Qubit Limit | Notes |
|---------|--------|-------------|-------|
| State-vector (CPU) | Full 2^n amplitudes | ~30 qubits | SIMD-accelerated |
| State-vector (GPU) | CUDA/OpenCL/Metal | ~30+ qubits | Optional feature |
| Tensor network | Contraction-based | Varies | For limited-entanglement circuits |
| Stabilizer | Clifford tableau | Large | O(n²), Clifford gates only |

Performance claims from README (Apple Silicon):
- 4 qubits: 57 ns (H), 100 ns (CNOT)
- 12 qubits: 3.88 µs (H), 9.34 µs (CNOT)
- Bell state circuit: 12.2 µs

## Hardware Integration

The `quantrs2-device` crate claims connectors for:

- **IBM Quantum**: Authentication, transpilation, job submission
- **Azure Quantum**: Cloud integration
- **AWS Braket**: Cloud integration

However, the project is in alpha (v0.1.3). The hardware integration
maturity is unclear — the device crate has 406 tests, but it's unknown
how many are integration tests against real hardware vs. mocked.

## Key Features

### Gate Set
Standard gates (H, X, Y, Z, S, T, CNOT, CZ, SWAP) plus S†, T†, √X,
controlled variants, and parametric rotations (Rx, Ry, Rz).

### Const-Generic Circuits
Qubit count is a compile-time parameter via Rust const generics:
```rust
let mut circuit = Circuit::<2>::new(); // 2 qubits, checked at compile time
```

### Quantum Algorithms
Built-in implementations of QAOA, Grover, QFT, QPE, and simplified Shor.

### Error Correction
Bit flip code, phase flip code, Shor code, 5-qubit perfect code.

### Noise Models
Bit flip, phase flip, depolarizing, amplitude/phase damping channels.
IBM device-specific T1/T2 relaxation models.

### Quantum Machine Learning
QNNs, quantum GANs, variational circuits, gradient computation.

### Quantum Annealing
QUBO/Ising model solvers with simulated annealing. Example problems:
MaxCut, graph coloring, TSP.

## Comparison with roqoqo/qcfront

| Aspect | roqoqo/qcfront | QuantRS2 |
|--------|---------------|----------|
| **Focus** | Circuit execution on real hardware | Comprehensive framework (sim + ML + annealing) |
| **Simulation** | QuEST (external C engine) | Pure Rust state-vector/tensor/stabilizer |
| **Hardware** | Verified working (QuEST, Azure QIR) | Claimed (IBM, Azure, Braket) — alpha |
| **Dependencies** | roqoqo + QuEST (C library) | Pure Rust (no C/C++) |
| **Gate typing** | Runtime qubit indices | Const-generic qubit count |
| **Scope** | Focused (circuits + algorithms) | Broad (ML, annealing, error correction, etc.) |
| **Maturity** | Stable (roqoqo v1.21) | Alpha (v0.1.3) |
| **Code size** | ~2K LoC (algos crate) | ~777K Rust LoC |
| **Python** | qoqo (PyO3 wrapper) | quantrs2-py (PyO3) |

## Observations

**Strengths:**
- Pure Rust with no C/C++ dependencies — easier cross-compilation
- Very broad scope covering many quantum computing paradigms
- Const-generic circuits catch qubit-count errors at compile time
- Includes quantum annealing alongside gate-based computing

**Concerns:**
- 777K LoC for an alpha project is unusually large — raises questions
  about code generation or AI-assisted bulk creation
- Hardware integrations are "claimed" but alpha status makes verification
  difficult
- The project depends heavily on `scirs2` (Scientific Rust), another
  project by the same author, creating a deep dependency chain
- Very broad scope for a small team may mean shallow coverage in each area

## Sources

- GitHub: https://github.com/cool-japan/quantrs
- docs.rs: https://docs.rs/quantrs2-core/latest/quantrs2_core/
- Device crate: https://docs.rs/quantrs2-device/latest/quantrs2_device/
