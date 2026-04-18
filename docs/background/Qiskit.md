# Qiskit

> Repository: <https://github.com/Qiskit/qiskit>

## Overview

Qiskit is IBM's open-source Python SDK for quantum computing. It provides a complete stack from circuit construction to transpilation, simulation, and execution on real quantum hardware. Qiskit follows a **Python-native** approach — quantum circuits are Python objects that get transpiled into hardware-specific instructions and submitted to backends via a provider interface.

## Architecture & Key Components

```
┌──────────────────────────────────────────────────────────────┐
│              User Code (Python + QuantumCircuit)              │
├──────────────────────────────────────────────────────────────┤
│                      Transpiler                              │
│  Init → Layout → Routing → Translation → Optimization       │
│                  → Scheduling                                │
├──────────────────────────────────────────────────────────────┤
│              Transpiled Circuit (Basis Gates)                 │
│              + OpenQASM 2.0/3.0 serialization                │
├──────────────────────────────────────────────────────────────┤
│                   Provider Interface                         │
│          Backend.run() / Primitives (Sampler, Estimator)     │
├──────────┬──────────────┬────────────────────────────────────┤
│ IBM      │ IonQ         │ Rigetti                            │
│ Quantum  │ (qiskit-ionq)│ (qiskit-rigetti)                   │
├──────────┴──────────────┴────────────────────────────────────┤
│                    Quantum Hardware                           │
└──────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Description |
|---|---|
| **QuantumCircuit** | Core data structure representing a quantum circuit as a DAG of gates and measurements |
| **Transpiler** | Multi-stage pipeline that converts abstract circuits into hardware-executable form |
| **Provider / Backend** | Abstract interfaces for connecting to quantum hardware or simulators |
| **Primitives** | High-level abstractions (`Sampler`, `Estimator`) for common quantum workloads |
| **Circuit Library** | Pre-built parameterized circuits (QFT, Grover, VQE ansätze, etc.) |
| **Qiskit Aer** | High-performance simulator with noise modeling |

### Repository Structure (Simplified)

```
qiskit/
├── circuit/
│   ├── library/          # Pre-built circuits (QFT, Grover, TwoLocal, etc.)
│   ├── quantumcircuit.py # Core QuantumCircuit class
│   └── gate.py           # Gate definitions
├── transpiler/
│   ├── passes/           # Individual transpiler passes
│   │   ├── optimization/ # Gate cancellation, consolidation, etc.
│   │   ├── routing/      # SABRE, stochastic SWAP, etc.
│   │   ├── layout/       # Trivial, dense, SABRE layout
│   │   └── basis/        # Basis translation, unrolling
│   ├── preset_passmanagers/ # Optimization level 0-3 presets
│   └── passmanager.py    # PassManager orchestration
├── providers/
│   ├── backend.py        # Abstract Backend class
│   ├── provider.py       # Abstract Provider class
│   └── job.py            # Abstract Job class
├── primitives/
│   ├── sampler.py        # Sampler primitive
│   └── estimator.py      # Estimator primitive
├── qasm3/                # OpenQASM 3.0 import/export
└── visualization/        # Circuit drawing, histogram plotting
```

## Compilation Pipeline: Python → Hardware Instructions

### Stage 1: Circuit Construction (Python)

```python
from qiskit import QuantumCircuit

qc = QuantumCircuit(2, 2)
qc.h(0)
qc.cx(0, 1)
qc.measure([0, 1], [0, 1])
```

At this stage, the circuit uses **abstract gates** (H, CX) without hardware constraints.

### Stage 2: Transpilation

The transpiler converts the abstract circuit into a form compatible with the target backend. It runs through multiple **stages**, each consisting of one or more **passes**:

```python
from qiskit import transpile

transpiled_qc = transpile(qc, backend, optimization_level=2)
```

#### Transpiler Stages

| Stage | Purpose | Example Passes |
|---|---|---|
| **Init** | Prepare circuit for processing | Unroll custom gates, remove barriers |
| **Layout** | Map logical qubits → physical qubits | `TrivialLayout`, `SabreLayout`, `DenseLayout` |
| **Routing** | Insert SWAP gates for connectivity constraints | `SabreSwap`, `StochasticSwap`, `BasicSwap` |
| **Translation** | Decompose to backend's basis gates | `BasisTranslator`, `Unroller` |
| **Optimization** | Reduce gate count/depth | `Optimize1qGates`, `CXCancellation`, `CommutativeCancellation` |
| **Scheduling** | Add timing/delay information | `ALAPSchedule`, `ASAPSchedule` |

#### Gate Decomposition Example

An abstract Toffoli (CCX) gate on an IBM backend with basis gates `{CX, ID, RZ, SX, X}`:

```
CCX(q0, q1, q2)
    ↓ decompose
H(q2), CX(q1,q2), Tdg(q2), CX(q0,q2), T(q2), CX(q1,q2), Tdg(q2), CX(q0,q2), ...
    ↓ translate to basis
RZ, SX, CX sequences
```

### Stage 3: Serialization

The transpiled circuit can be serialized to **OpenQASM** for interoperability:

**OpenQASM 2.0:**
```qasm
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
measure q[0] -> c[0];
measure q[1] -> c[1];
```

**OpenQASM 3.0:**
```qasm
OPENQASM 3;
qubit[2] q;
bit[2] c;
h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
```

### Stage 4: Backend Execution

```python
job = backend.run(transpiled_qc, shots=1024)
result = job.result()
counts = result.get_counts()  # e.g. {'00': 512, '11': 512}
```

The provider package serializes the transpiled circuit (as QASM or JSON) and sends it to the hardware vendor's cloud API.

## Hardware Interaction: Provider Interface

### Provider / Backend Architecture

```python
# Abstract interface that all providers implement
class Provider:
    def get_backend(name: str) -> Backend
    def backends() -> List[Backend]

class Backend:
    def run(circuit, shots) -> Job
    def configuration() -> BackendConfiguration  # basis_gates, coupling_map, etc.

class Job:
    def result() -> Result
    def status() -> JobStatus
```

Each hardware vendor provides a Qiskit provider package:

| Package | Provider | Installation |
|---|---|---|
| `qiskit-ibm-runtime` | IBM Quantum | `pip install qiskit-ibm-runtime` |
| `qiskit-ionq` | IonQ | `pip install qiskit-ionq` |
| `qiskit-rigetti` | Rigetti | `pip install qiskit-rigetti` |

### IBM Quantum Example

```python
from qiskit_ibm_runtime import QiskitRuntimeService, Sampler

service = QiskitRuntimeService(channel="ibm_quantum", token="...")
backend = service.backend("ibm_kyiv")
sampler = Sampler(backend)
result = sampler.run(qc).result()
```

### IonQ Example

```python
from qiskit_ionq import IonQProvider

provider = IonQProvider(token="YOUR_IONQ_API_KEY")
backend = provider.get_backend("ionq_qpu")
# Transpile for IonQ's native gates (GPI, GPI2, MS)
transpiled = transpile(qc, backend)
job = backend.run(transpiled)
result = job.result()
```

Under the hood, `qiskit-ionq` converts the circuit to IonQ's JSON format and calls the IonQ REST API.

### Rigetti Example

```python
from qiskit_rigetti import RigettiQCSProvider

provider = RigettiQCSProvider()
backend = provider.get_backend("Aspen-M-3")
transpiled = transpile(qc, backend)
job = backend.run(transpiled)
result = job.result()
```

Under the hood, `qiskit-rigetti` converts the circuit to Quil and submits via Rigetti QCS.

## Hardware Provider APIs

### IonQ REST API

```
Base URL: https://api.ionq.co/v0.4

POST /jobs          — Submit a quantum job
GET  /jobs          — List jobs
GET  /jobs/{id}     — Get job status/results
PUT  /jobs/{id}/status/cancel — Cancel a job
GET  /backends      — List available devices

Authentication: Authorization: apiKey <key>
```

**Job Submission Payload:**
```json
{
  "type": "ionq.circuit.v1",
  "name": "Bell State",
  "shots": 1024,
  "backend": "qpu.forte-1",
  "input": {
    "qubits": 2,
    "gateset": "qis",
    "circuit": [
      {"gate": "h", "target": 0},
      {"gate": "cnot", "control": 0, "target": 1}
    ]
  }
}
```

**IonQ Native Gates:**
- `gpi`, `gpi2` — single-qubit gates (native trapped-ion operations)
- `ms` — Mølmer-Sørensen entangling gate (two-qubit native gate)
- `qis` gateset — standard gates (`h`, `cnot`, `rx`, `ry`, `rz`, etc.) decomposed by IonQ

### Rigetti QCS API

```
Base URL: https://api.qcs.rigetti.com

Authentication: OAuth2 Bearer JWT (via Okta)
```

Rigetti uses a hybrid API model:
- **REST** endpoints for account management, reservations, device topology
- **gRPC/ZeroMQ (rpcq)** for actual QPU job execution (abstracted by pyQuil/qcs-sdk)

**Job submission is typically done via the pyQuil SDK**, not raw REST:

```python
from pyquil import Program, get_qc
from pyquil.gates import H, CNOT, MEASURE

p = Program()
ro = p.declare('ro', 'BIT', 2)
p += H(0)
p += CNOT(0, 1)
p += MEASURE(0, ro[0])
p += MEASURE(1, ro[1])

qc = get_qc('Aspen-M-3')
result = qc.run(p)
```

**Rigetti Native Gates:**
- `RX(θ)`, `RZ(θ)` — single-qubit rotations
- `CZ` — controlled-Z (two-qubit native gate)
- `I` — identity

## Key Differentiators

- **Python-native**: Circuits are Python objects — no separate language needed
- **Modular transpiler**: Highly configurable pass-based compilation pipeline
- **Provider ecosystem**: Plugin architecture allows any hardware vendor to integrate
- **Primitives API**: Modern `Sampler`/`Estimator` interface for algorithm development
- **OpenQASM**: Supports both 2.0 and 3.0 for circuit interchange
- **Largest user community** in quantum computing

## References

- [Qiskit GitHub](https://github.com/Qiskit/qiskit)
- [Qiskit Documentation](https://docs.quantum.ibm.com/)
- [Qiskit Transpiler Docs](https://docs.quantum.ibm.com/api/qiskit/transpiler)
- [Qiskit Provider Interface](https://docs.quantum.ibm.com/api/qiskit/providers)
- [OpenQASM 3.0 Spec](https://github.com/openqasm/openqasm)
- [IonQ API Docs](https://docs.ionq.com/)
- [Rigetti QCS Docs](https://docs.rigetti.com/qcs)
- [qiskit-ionq](https://github.com/qiskit-community/qiskit-ionq)
