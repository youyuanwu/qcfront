# Google Cirq

> Repository: <https://github.com/quantumlib/Cirq>

## Overview

Cirq is Google's open-source Python framework for writing, manipulating, and optimizing quantum circuits, then running them on simulators or real quantum hardware. It has a strong focus on **near-term quantum computing (NISQ)** and tight integration with Google's Sycamore processors, but also supports IonQ, Pasqal, and other hardware via modular sub-packages.

## Architecture & Key Components

```
┌──────────────────────────────────────────────────────────┐
│            User Code (Python + cirq.Circuit)              │
├──────────────────────────────────────────────────────────┤
│                   Transformers                            │
│  Decomposition → Optimization → Device Mapping            │
├──────────────────────────────────────────────────────────┤
│            Device-Compatible Circuit                      │
│         (native gate set, physical qubits)                │
├──────────────────────────────────────────────────────────┤
│                  Service / Sampler                         │
│        Serialization → API call → Result retrieval        │
├──────────┬───────────────┬───────────────────────────────┤
│ Google   │ IonQ           │ Pasqal / AQT / Other         │
│ Engine   │ ionq.Service   │ (via sub-packages)            │
├──────────┴───────────────┴───────────────────────────────┤
│                  Quantum Hardware                         │
└──────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Description |
|---|---|
| **cirq.Circuit** | Core data structure — a circuit is a list of `Moment`s, each containing non-overlapping `Operation`s |
| **cirq.ops** | Gate and operation definitions (H, CNOT, Rz, FSim, custom gates) |
| **cirq.devices** | Device specifications — qubit connectivity, supported gates, noise models |
| **cirq.transformers** | Modular, composable circuit transformations (like Qiskit transpiler passes) |
| **cirq.sim** | Simulators — full state vector, density matrix, Clifford, MPS |
| **cirq-google** | Google Quantum AI integration — Engine API, Sycamore device, compilation |
| **cirq-ionq** | IonQ integration — Service API, device definitions, circuit conversion |
| **cirq-aqt** | Alpine Quantum Technologies integration |
| **cirq-pasqal** | Pasqal neutral-atom integration |

### Repository Structure (Simplified)

```
Cirq/
├── cirq-core/cirq/
│   ├── circuits/        # Circuit and Moment classes
│   ├── ops/             # Gates and operations
│   ├── devices/         # Device abstractions
│   ├── transformers/    # Circuit transformations and optimization
│   ├── sim/             # Simulators
│   ├── protocols/       # Protocol definitions (unitary, measurement, etc.)
│   └── qis/             # Quantum information science utilities
├── cirq-google/cirq_google/
│   ├── engine/          # Google Quantum Engine client
│   ├── devices/         # Sycamore, Bristlecone device definitions
│   ├── transformers/    # Google-specific compilation transforms
│   └── serialization/   # Proto/JSON serialization for Engine API
├── cirq-ionq/cirq_ionq/
│   ├── service.py       # IonQ cloud API client
│   ├── sampler.py       # IonQ sampler implementation
│   ├── ionq_devices.py  # IonQ device definitions
│   └── serializer.py    # Cirq → IonQ JSON conversion
├── cirq-aqt/            # AQT integration
├── cirq-pasqal/         # Pasqal integration
└── docs/                # Documentation
```

## Compilation Pipeline: Python → Hardware Instructions

### Stage 1: Circuit Construction (Python)

```python
import cirq

q0, q1 = cirq.LineQubit.range(2)
circuit = cirq.Circuit([
    cirq.H(q0),
    cirq.CNOT(q0, q1),
    cirq.measure(q0, q1, key='result')
])
```

Cirq circuits use a **Moment-based** representation:
- Each `Moment` is a set of operations that can execute in parallel
- Operations within a moment must act on different qubits
- This gives Cirq natural awareness of circuit depth and parallelism

### Stage 2: Transformation (Compilation)

Cirq uses **Transformers** — modular, composable circuit transformations:

```python
# Decompose to a target gate set
optimized = cirq.optimize_for_target_gateset(
    circuit,
    gateset=cirq.SqrtIswapTargetGateset()  # Google Sycamore native gates
)

# Or apply individual transformers
circuit = cirq.drop_negligible_operations(circuit)
circuit = cirq.merge_single_qubit_gates_to_phased_x_and_z(circuit)
```

#### Key Transformers

| Transformer | Purpose |
|---|---|
| `optimize_for_target_gateset` | Decompose all gates to a specific native gate set |
| `drop_negligible_operations` | Remove operations with negligible effect |
| `merge_single_qubit_gates_to_phased_x_and_z` | Combine single-qubit gates |
| `expand_composite` | Expand composite/meta gates into primitives |
| `defer_measurements` | Move measurements to end of circuit |
| `align_left` / `align_right` | Compact circuit moments |

#### Target Gate Sets

| Hardware | Native Gate Set | Cirq Gateset Class |
|---|---|---|
| Google Sycamore | `√iSWAP`, `PhasedXZ` | `SqrtIswapTargetGateset` |
| Google (general) | `CZ`, `PhasedXZ` | `CZTargetGateset` |
| IonQ | `XX` (MS gate), `Rx`, `Ry`, `Rz` | (handled by cirq-ionq serializer) |

### Stage 3: Serialization & Submission

For each hardware target, the circuit is serialized into the target's expected format:

**Google Engine** — Protocol Buffers (protobuf):
```python
from cirq_google import Engine

engine = Engine(project_id="my-project")
result = engine.run(circuit, processor_id="weber", repetitions=1000)
```

**IonQ** — JSON format:
```python
from cirq_ionq import Service

service = Service(api_key="YOUR_KEY")
result = service.run(circuit, target="qpu", repetitions=1000)
```

### Stage 4: What Gets Sent to Hardware

#### Google Quantum Engine

Cirq serializes to protobuf, sent via the Google Cloud Quantum Engine API:

```protobuf
// Simplified representation
message Circuit {
  repeated Moment moments = 1;
}
message Moment {
  repeated Operation operations = 1;
}
message Operation {
  Gate gate = 1;
  repeated Qubit qubits = 2;
}
```

#### IonQ JSON Format

`cirq-ionq` serializes to IonQ's native JSON:

```json
{
  "qubits": 2,
  "circuit": [
    {"gate": "h", "target": 0},
    {"gate": "cnot", "control": 0, "target": 1}
  ]
}
```

## Hardware Provider APIs

### Google Quantum Engine API

```
Authentication: Google Cloud OAuth2
Protocol: gRPC / REST via Google Cloud client libraries

Key operations:
- Create program (upload circuit as protobuf)
- Create job (specify processor, repetitions)
- Get job results
- List processors and calibrations
```

```python
import cirq_google as cg

# Engine-based execution
engine = cg.Engine(project_id="my-gcp-project")
job = engine.run_sweep(
    program=circuit,
    processor_id="weber",
    params=None,
    repetitions=1000
)
results = job.results()
```

**Google Native Gates:**
- `SYC` (Sycamore gate) — two-qubit entangling gate
- `√iSWAP` — two-qubit gate
- `CZ` — controlled-Z
- `PhasedXZ` — general single-qubit rotation

### IonQ API (via cirq-ionq)

```
Base URL: https://api.ionq.co/v0.4
Authentication: apiKey header

POST /jobs     — Submit circuit
GET  /jobs     — List jobs
GET  /jobs/{id} — Get results
GET  /backends — List devices
```

```python
import cirq_ionq as ionq

service = ionq.Service(api_key="YOUR_KEY")

# Run on QPU
result = service.run(
    circuit=circuit,
    target="qpu",
    name="Bell State",
    repetitions=1000
)

# Run on simulator
result = service.run(circuit=circuit, target="simulator", repetitions=1000)
```

**IonQ Native Gates:**
- `gpi(φ)` — single-qubit gate
- `gpi2(φ)` — single-qubit gate
- `ms(φ0, φ1)` — Mølmer-Sørensen entangling gate

### IonQ Job Submission (Full JSON)

```json
{
  "type": "ionq.circuit.v1",
  "name": "cirq-job",
  "shots": 1000,
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

## Key Design Differences from Other Frameworks

- **Moment-based representation**: Circuits are explicit about parallelism — each moment is a time slice
- **Device-aware from the start**: Cirq was designed with specific hardware constraints in mind
- **Transformer architecture**: Modular transforms replace a monolithic transpiler
- **No separate IR**: Circuits stay as Python objects; serialization happens at the boundary when talking to hardware
- **Sub-package modularity**: Each hardware vendor is a separate pip-installable package (`cirq-google`, `cirq-ionq`, etc.)
- **NISQ focus**: Optimized for near-term, noisy devices rather than fault-tolerant abstractions

## References

- [Cirq GitHub](https://github.com/quantumlib/Cirq)
- [Cirq Documentation](https://quantumai.google/cirq)
- [cirq-google Documentation](https://quantumai.google/cirq/google)
- [cirq-ionq Documentation](https://quantumai.google/cirq/ionq)
- [Cirq Transformers Guide](https://quantumai.google/cirq/transform)
- [IonQ API Documentation](https://docs.ionq.com/)
- [Google Quantum AI](https://quantumai.google/)
