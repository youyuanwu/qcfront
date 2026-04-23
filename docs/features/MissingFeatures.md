# Missing Features

Gap analysis comparing qcfront to Qiskit's feature set.
Prioritized by impact for a Rust quantum computing research framework.

## What qcfront has today

| Area | Module | Description |
|------|--------|-------------|
| Grover search | `grover` | Configurable oracle, optimal iteration count |
| Shor factoring | `shor` | QPE-based period finding, mod-15 arithmetic |
| SAT solver | `sat` | CNF → Grover oracle with validated `Literal` type |
| Subset sum | `grover/subset_sum` | Controlled-adder oracle for subset-sum instances |
| State preparation | `state` | Möttönen decomposition, `QuantumState` newtype, fidelity |
| Phase estimation | `qpe` | Reusable QPE with closure-based controlled unitaries |
| Qubit type safety | `qubit` | `Qubit` newtype, `QubitRange`, `QubitAllocator` |
| Runner abstraction | `runner` | `QuantumRunner` trait, backend-agnostic algorithms |
| Circuit primitives | `circuits` | Multi-CZ, multi-CX, adder, modular multiplication |
| Circuit transforms | `circuits/transform` | `inverse`, `within_apply`, `controlled` |
| Number theory | `math` | Continued fractions, mod_pow, GCD |
| Bit utilities | `qpe` | `bits_to_int_lsb/msb`, `extract_phase` |
| Local simulation | `quest_runner` | QuEST backend via roqoqo-quest |
| Cloud execution | `azure_runner` | Azure Quantum via QIR export + az CLI |
| Teleportation | `examples/teleport` | Quantum teleportation demo |
| Deutsch-Jozsa | `examples/deutsch_jozsa` | Constant vs balanced in 1 query, classical comparison |

## Priority 1 — High-impact new features

**Amplitude estimation / quantum counting**
- QPE on the Grover operator to estimate solution count
- Enables auto-tuned Grover (no manual iteration count)
- General amplitude estimation for Monte Carlo, integration
- Design complete: `docs/features/AmplitudeEstimation.md`
- Qiskit equivalent: `qiskit.algorithms.AmplitudeEstimation`

**Circuit optimization (peephole)**
- Currently circuits are emitted with no optimization
- Minimum viable: cancel adjacent inverses (H·H, X·X), merge consecutive
  same-axis rotations (Rz(a)·Rz(b) → Rz(a+b)), remove zero-angle gates
- Qiskit equivalent: `qiskit.transpiler` (thousands of lines — we'd do 1%)
- State preparation circuits are especially bloated without this

**Resource estimation**
- Count total gates, qubit count, circuit depth, T-count without simulating
- Answers "how expensive is this circuit?" for any algorithm
- Qiskit equivalent: none (Q#'s killer feature)
- Useful for understanding algorithm scalability

**Parameterized circuits**
- roqoqo already supports `CalculatorFloat::Str("theta")` for symbolic angles
- Need: wrapper API for binding parameter values to circuits
- Qiskit equivalent: `Parameter`, `ParameterVector`, `circuit.bind_parameters()`
- Required before implementing VQE/QAOA

## Priority 2 — Textbook algorithms

**Bernstein-Vazirani**
- Find a hidden bitstring s in one query (classically needs n queries)
- Uses H-oracle-H-measure pattern with inner-product oracle
- ~80 lines, demonstrates quantum parallelism

**Simon's algorithm**
- Find hidden period of a 2-to-1 function
- Predecessor to Shor — period finding via interference
- Requires classical post-processing (linear algebra over GF(2))
- Qiskit equivalent: `qiskit.algorithms.Simon`

## Priority 3 — Advanced algorithms

**VQE (Variational Quantum Eigensolver)**
- Hybrid classical-quantum optimization loop
- Requires: parameterized circuits, expectation value estimation, classical optimizer
- Qiskit equivalent: `qiskit_algorithms.VQE`
- Most requested quantum algorithm after Grover/Shor

**QAOA (Quantum Approximate Optimization)**
- Approximate solutions for combinatorial optimization (MaxCut, etc.)
- Requires: parameterized circuits
- Near-term practical quantum algorithm
- Qiskit equivalent: `qiskit_algorithms.QAOA`

**Quantum walk search**
- Graph-structured search alternative to Grover
- Quadratic speedup with graph topology awareness
- More complex than Grover but applicable to structured problems

**Quantum error correction codes**
- Repetition code, Steane code, surface code basics
- Qiskit equivalent: `qiskit.quantum_info` + Qiskit QEC experiments
- Important for understanding fault tolerance

## Priority 4 — Infrastructure

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
