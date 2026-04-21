# Missing Features

Gap analysis comparing qcfront to Qiskit's feature set.
Prioritized by impact for a Rust quantum computing research framework.

## What qcfront has today

| Area | Module | Description |
|------|--------|-------------|
| Grover search | `grover` | Configurable oracle, optimal iteration count |
| Shor factoring | `shor` | QPE-based period finding, mod-15 arithmetic |
| SAT solver | `sat` | CNF → Grover oracle with validated `Literal` type |
| State preparation | `state` | Möttönen decomposition, `QuantumState` newtype, fidelity |
| Phase estimation | `qpe` | Reusable QPE with closure-based controlled unitaries |
| Runner abstraction | `runner` | `QuantumRunner` trait, backend-agnostic algorithms |
| Circuit primitives | `circuits` | Multi-CZ, multi-CX, modular multiplication |
| Number theory | `math` | Continued fractions, mod_pow, GCD |
| Bit utilities | `qpe` | `bits_to_int_lsb/msb`, `extract_phase` |
| Local simulation | `quest_runner` | QuEST backend via roqoqo-quest |
| Cloud execution | `azure_runner` | Azure Quantum via QIR export + az CLI |

## Priority 1 — High-impact new features

**Measurement statistics (`Counts` type)**
- Currently algorithms return raw `BitRegisters` (`HashMap<String, Vec<Vec<bool>>>`)
- Need: `Counts` type with histogram, most-frequent result, probability distribution
- Qiskit equivalent: `qiskit.result.Counts`, `plot_histogram()`
- Would simplify every algorithm's result handling

**Circuit optimization (peephole)**
- Currently circuits are emitted with no optimization
- Minimum viable: cancel adjacent inverses (H·H, X·X), merge consecutive
  same-axis rotations (Rz(a)·Rz(b) → Rz(a+b)), remove zero-angle gates
- Qiskit equivalent: `qiskit.transpiler` (thousands of lines — we'd do 1%)
- State preparation circuits are especially bloated without this

**Parameterized circuits**
- roqoqo already supports `CalculatorFloat::Str("theta")` for symbolic angles
- Need: wrapper API for binding parameter values to circuits
- Qiskit equivalent: `Parameter`, `ParameterVector`, `circuit.bind_parameters()`
- Required before implementing VQE/QAOA

## Priority 2 — Algorithm extensions

**VQE (Variational Quantum Eigensolver)**
- Hybrid classical-quantum optimization loop
- Requires: parameterized circuits, expectation value estimation, classical optimizer
- Qiskit equivalent: `qiskit_algorithms.VQE`
- Most requested quantum algorithm after Grover/Shor

**Amplitude estimation**
- Generalization of Grover's counting problem
- Iterative and maximum-likelihood variants avoid QPE overhead
- Qiskit equivalent: `qiskit_algorithms.AmplitudeEstimation`

**Quantum error correction codes**
- Repetition code, Steane code, surface code basics
- Qiskit equivalent: `qiskit.quantum_info` + Qiskit QEC experiments
- Important for understanding fault tolerance

## Priority 3 — Infrastructure

**Noise modeling**
- Depolarizing, amplitude damping, measurement error channels
- Qiskit equivalent: `qiskit_aer.noise.NoiseModel`
- roqoqo has `PragmaDepolarising` etc. — need to wire them

**Circuit visualization**
- Text-based circuit drawing (ASCII art)
- Qiskit equivalent: `circuit.draw('text')`
- Nice-to-have for debugging, not blocking

**OpenQASM export**
- roqoqo-qasm exists but isn't wired into qcfront
- Would enable interop with other frameworks
- Lower priority since QIR export already works for Azure

## Not planned

These are Qiskit features we intentionally skip:

- **IBM runtime integration** — we use Azure instead
- **Pulse-level control** — hardware-specific, not relevant for simulation
- **Qiskit Experiments** — calibration/characterization for real hardware
- **QSVM / QNN** — quantum ML is better served by specialized frameworks

## Sources

- Qiskit circuit library: https://docs.quantum.ibm.com/api/qiskit/circuit_library
- Qiskit algorithms: https://qiskit-community.github.io/qiskit-algorithms/
- roqoqo operations: https://docs.rs/roqoqo/latest/roqoqo/operations/
