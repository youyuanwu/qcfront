# Qnect: Quantum Networking in Rust

## Overview

Qnect is a pure-Rust quantum computing and networking framework focused on
simulating distributed quantum networks. Unlike roqoqo (which targets
gate-based circuit execution on hardware), qnect's primary focus is quantum
networking — multi-node entanglement distribution, quantum repeaters,
teleportation protocols, and quantum key distribution (QKD).

- **Repository**: https://github.com/theis-maker/qnect-rs
- **Crate**: https://crates.io/crates/qnect (v0.3.0)
- **License**: MIT/Apache-2.0
- **Size**: ~7K SLoC, pure Rust, async (tokio)

## Simulation Only

Qnect is entirely simulation-based. All quantum operations run on built-in
classical simulators — there is no connection to real quantum hardware.
The `QuantumBackend` trait has these implementations:

| Backend | Method | Qubit Limit | Use Case |
|---------|--------|-------------|----------|
| `StateVector` | Full state-vector (2^n amplitudes) | ~30 qubits | Exact simulation, arbitrary gates |
| `Stabilizer` | Clifford tableau (O(n²) memory) | 5000+ qubits | H, S, CNOT only — no T gate or Toffoli |
| `NoisyBackend` | Wraps any backend with error model | Same as inner | Depolarizing + measurement errors |
| `MockQnpu` | Simulated hardware API endpoint | N/A | Testing hardware integration code |
| `NetworkBackend` | Delegates to per-node backends | Per-node | Distributed network simulation |

The `MockQnpu` backend simulates a quantum network processing unit (QNPU)
API for testing, but does not connect to real hardware. The project lists
"IBMQ, IonQ, QuTech" as future backends but none are implemented.

## Architecture

### Core Trait

```rust
#[async_trait]
pub trait QuantumBackend: Send + Sync {
    async fn apply_single_gate(&mut self, qubit: usize, gate: Gate1) -> Result<()>;
    async fn apply_two_gate(&mut self, q1: usize, q2: usize, gate: Gate2) -> Result<()>;
    async fn measure(&mut self, qubit: usize) -> Result<u8>;
    async fn create_entanglement(&mut self, q1: usize, q2: usize) -> Result<()>;
    fn qubit_count(&self) -> usize;
}
```

All operations are async — designed for networked scenarios where
entanglement distribution may involve real network I/O.

### Gate Set

Single-qubit (`Gate1`): H, X, Y, Z, S, T, Rx, Ry, Rz
Two-qubit (`Gate2`): CNOT, CZ, SWAP

No multi-controlled gates (no Toffoli, CCZ). The stabilizer backend
further restricts to Clifford gates only (H, S, CNOT).

### Key Modules

| Module | Purpose |
|--------|---------|
| `quantum` | Qubit states, entanglement, quantum systems |
| `backend` | Simulation backends (state-vector, stabilizer, noisy) |
| `network` | Quantum networks, nodes, repeaters, hubs, topologies |
| `physics` | Realistic channel models (fiber loss, detector efficiency, decoherence) |
| `protocol` | Quantum protocols (BB84 QKD, anonymous transmission, blind computing) |
| `builder` | Fluent API for constructing quantum systems |

## Quantum Networking Features

Qnect's main differentiator is its quantum networking stack:

- **Multi-hop entanglement**: Entanglement swapping through repeater chains
- **Hub routing**: Star/ring/mesh/hierarchical topologies with quantum hubs
- **BB84 QKD**: Quantum key distribution protocol implementation
- **Anonymous transmission**: Based on Christandl & Wehner 2004
- **Blind quantum computing**: UBQC protocol (client delegates without revealing data)
- **Quantum chat**: Demo application with QKD-secured messaging
- **Physics models**: Fiber loss (dB/km), detector dark counts, memory decoherence

## QASM Support

Qnect can import and export OpenQASM circuits via `execute_qasm()` and
QASM import/export functions. It also generates NetQASM code for
QuTech-compatible hardware (though this targets simulation, not actual
QuTech hardware submission).

## Comparison with roqoqo

| Aspect | roqoqo/qcfront | qnect |
|--------|---------------|-------|
| **Focus** | Gate-based algorithms (Shor, Grover) | Quantum networking protocols |
| **Hardware** | QuEST, IQM, Braket, Azure (real) | Simulation only |
| **Gate set** | Full (including Toffoli, CCZ) | Basic (no multi-controlled gates) |
| **Async** | Sync API | Async (tokio) throughout |
| **Multi-node** | Single circuit | Multi-node networks with routing |
| **Noise** | Via backend (QuEST) | Built-in depolarizing/measurement model |
| **Maturity** | Established (roqoqo v1.21) | Early (v0.3.0) |

## Simulation vs. Real Security

Qnect's QKD and quantum networking protocols run entirely in simulation.
This has fundamental implications for security:

**Real hardware QKD (e.g., BB84 over fiber):**
- Photons physically travel through a channel — measuring them disturbs
  the quantum state (no-cloning theorem)
- An eavesdropper introduces detectable errors (~25% quantum bit error rate)
- Security is guaranteed by the laws of physics

**Simulated QKD (qnect):**
- All qubits are complex numbers in RAM on the same machine
- There is no physical channel to intercept — "eavesdropping" is just
  another function call within the simulation
- The simulator correctly models protocol logic: if an eavesdropper
  measurement is simulated, the error rate increases and gets detected
- But anyone with process memory access can read all qubit states directly
- The "quantum-secured chat" demo uses classical encryption with a key
  produced by a BB84 simulation — no actual quantum security

Simulated QKD is valuable for protocol research, algorithm validation,
and education. For actual security, real photons on real fiber are
required (hardware from ID Quantique, Toshiba, etc.).

## Relevance to qcfront

Qnect occupies a different niche — quantum networking vs. quantum
algorithms. Key observations:

- **Not a replacement for roqoqo**: No hardware backends, limited gate set,
  no multi-controlled gates needed for Grover/Shor
- **Interesting async pattern**: The async `QuantumBackend` trait is a design
  alternative to our sync `QuantumRunner`, more natural for networked scenarios
- **Networking protocols**: QKD, anonymous transmission, and blind computing
  are algorithms we don't implement and wouldn't need roqoqo for
- **Pure Rust**: No C/C++ dependencies (unlike roqoqo-quest which wraps QuEST)

## Sources

- GitHub: https://github.com/theis-maker/qnect-rs
- docs.rs: https://docs.rs/qnect/latest/qnect/
- lib.rs: https://lib.rs/crates/qnect
