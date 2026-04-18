# Local Quantum Simulators

## Overview

Every major quantum computing framework includes simulators that run on your local machine. These let you develop, test, and debug quantum programs without cloud access or real hardware. This document catalogs the simulators available across the reference projects, plus notable standalone simulators.

All gate-based simulators face the same fundamental constraint: **simulating n qubits requires 2ⁿ complex amplitudes** for full state-vector simulation. This means memory (and time) grows exponentially with qubit count.

### Memory Requirements (State Vector)

| Qubits | Amplitudes | RAM Required | Feasible On |
|---|---|---|---|
| 10 | 1,024 | 16 KB | Anything |
| 20 | ~1M | 16 MB | Anything |
| 25 | ~33M | 512 MB | Laptop |
| 30 | ~1B | 16 GB | Desktop |
| 35 | ~34B | 512 GB | Server |
| 40 | ~1T | 16 TB | HPC cluster |
| 50 | ~1P | 16 PB | Impossible (local) |

Density matrix simulation squares this: n qubits need 2²ⁿ entries (halving the practical qubit limit).

---

## Simulator Protocols: How Programs Reach the Simulation Engine

Each framework uses a different protocol to pass the user's quantum program from the Python frontend to the simulation engine. This is a critical architectural decision — it determines whether the simulator is an in-process library, a separate binary, or a remote service.

### Protocol Summary

```
QISKIT:
  Python QuantumCircuit
       │
       ▼ (Pybind11 / PyO3)
  Rust circuit data ──→ serialized to Qobj (JSON-like dict)
       │
       ▼ (Pybind11)
  C++ Aer engine (in-process .so)
       │
       ▼
  State vector / density matrix operations
       │
       ▼
  Results dict back to Python


CIRQ (built-in simulator):
  Python cirq.Circuit
       │
       ▼ (direct Python)
  NumPy matrix operations (in-process, same Python process)
       │
       ▼
  State vector as numpy.ndarray


CIRQ + qsim:
  Python cirq.Circuit
       │
       ▼ (serialize to Protocol Buffers)
  protobuf bytes ──→ pybind11 ──→ C++ qsim engine (in-process .so)
       │
       ▼
  Results back via pybind11


Q# / QDK:
  Python qsharp.eval("Q# code")
       │
       ▼ (PyO3 FFI)
  Rust compiler: Q# source → QIR (LLVM IR)
       │
       ▼ (in-process Rust)
  Rust simulator engine (sparse state vector)
       │
       ▼ (PyO3 FFI)
  Results back to Python


pyQuil + QVM:
  Python Program object
       │
       ▼ str(program) → Quil text
  JSON-RPC 2.0 envelope
       │
       ▼ HTTP POST to localhost:5000
  QVM server (separate Common Lisp process)
       │
       ▼
  Quil parsed → state vector simulation
       │
       ▼ JSON-RPC response
  Results back to Python


D-Wave Ocean:
  Python BQM/CQM object
       │
       ▼ (direct Python call)
  dwave-neal: pure Python/C simulated annealing (in-process)
       │
       ▼
  SampleSet back to Python
```

### Protocol Details by Framework

#### Qiskit Aer: Pybind11 + Qobj Serialization

| Step | What Happens | Format |
|---|---|---|
| 1. User builds circuit | `QuantumCircuit` Python object with gates, qubits, measurements | Python objects |
| 2. Transpile (optional) | Circuit optimized for simulator (basis gates, routing) | Python objects (Rust-accelerated) |
| 3. Serialize to Qobj | Circuit converted to **Qobj** — a JSON-like dict describing the full experiment | `dict` with keys: `qubits`, `gates[]`, `measurements[]`, `shots`, `noise_model` |
| 4. Cross into C++ | Qobj dict passed via **Pybind11** to `qiskit_aer` C++ shared library (`.so`/`.dll`) | Pybind11 type conversion (dict → C++ map) |
| 5. C++ simulation | Aer engine reconstructs circuit as C++ `Op` objects, runs matrix math | C++ internal structs |
| 6. Return results | Counts/statevector serialized back to Python dict via Pybind11 | Python dict |

**Key point:** Aer runs **in-process** — no network, no separate server. The C++ code is loaded as a Python extension module.

**Qobj example (what crosses the boundary):**
```json
{
  "qobj_id": "experiment_1",
  "type": "QASM",
  "experiments": [{
    "instructions": [
      {"name": "h", "qubits": [0]},
      {"name": "cx", "qubits": [0, 1]},
      {"name": "measure", "qubits": [0], "memory": [0]},
      {"name": "measure", "qubits": [1], "memory": [1]}
    ],
    "header": {"n_qubits": 2, "memory_slots": 2}
  }],
  "config": {"shots": 1024, "method": "statevector"}
}
```

#### Cirq Built-in: Pure Python (No Boundary)

| Step | What Happens | Format |
|---|---|---|
| 1. User builds circuit | `cirq.Circuit` with `Moment` list | Python objects |
| 2. Simulate | `cirq.Simulator().simulate(circuit)` iterates through Moments | Python method calls |
| 3. Gate application | Each gate's unitary matrix (as `numpy.ndarray`) multiplied into state vector | NumPy arrays |
| 4. Return results | `SimulationTrialResult` with `final_state_vector` as `numpy.ndarray` | NumPy array |

**Key point:** There is **no serialization boundary**. The simulator IS Python code calling NumPy. The circuit objects are iterated directly — `for moment in circuit: for op in moment: apply(op.gate.unitary(), state)`. This is why it's slower but simpler.

#### Cirq + qsim: Protobuf + Pybind11

| Step | What Happens | Format |
|---|---|---|
| 1. User builds circuit | `cirq.Circuit` Python object | Python objects |
| 2. Export to protobuf | Circuit serialized to **Protocol Buffer** binary format | `bytes` (protobuf) |
| 3. Cross into C++ | Protobuf bytes passed via **pybind11** to qsim `.so` | pybind11 call |
| 4. C++ simulation | qsim deserializes protobuf → internal gate list → SIMD-optimized simulation | C++ internal structs |
| 5. Return results | Measurement results serialized back via pybind11 | Python objects |

**Protobuf circuit schema (simplified):**
```protobuf
message Circuit {
  repeated Moment moments = 1;
}
message Moment {
  repeated Operation operations = 1;
}
message Operation {
  string gate = 1;
  repeated uint32 qubits = 2;
  repeated double params = 3;
}
```

#### Q# / QDK: PyO3 FFI to Rust

| Step | What Happens | Format |
|---|---|---|
| 1. User writes Q# | `qsharp.eval("H(q); CNOT(q0,q1);")` or `.qs` file | Q# source string |
| 2. Rust compiler | Q# source parsed and compiled to QIR (LLVM IR) by Rust compiler **in-process** | QIR (LLVM bitcode) |
| 3. Rust simulator | QIR interpreted/executed by Rust sparse state simulator | Rust internal structs |
| 4. Return results | Results marshalled back to Python via **PyO3** FFI | Python objects |

**Key point:** The entire compiler AND simulator are a **single Rust shared library** exposed to Python via PyO3. Q# source code goes in as a string, results come out as Python objects. No network, no subprocess.

#### pyQuil + QVM: HTTP + JSON-RPC (Separate Process)

| Step | What Happens | Format |
|---|---|---|
| 1. User builds program | `Program` object with Quil gates | Python objects |
| 2. Serialize to Quil | `str(program)` → Quil text string | Plain text |
| 3. Wrap in JSON-RPC | Quil string embedded in JSON-RPC 2.0 request envelope | JSON |
| 4. HTTP POST | Sent to QVM server at `localhost:5000` | HTTP request |
| 5. QVM parses | Common Lisp process parses Quil text, builds internal representation | Lisp data structures |
| 6. Simulate | State vector simulation in Common Lisp | Lisp arrays |
| 7. JSON-RPC response | Results serialized as JSON, sent back over HTTP | JSON |
| 8. Deserialize | pyQuil parses JSON response into numpy arrays | Python/numpy |

**JSON-RPC request (what goes over the wire):**
```json
{
  "jsonrpc": "2.0",
  "id": "req-12345",
  "method": "execute_qvm",
  "params": {
    "type": "multishot",
    "compiled-quil": "DECLARE ro BIT[2]\nH 0\nCNOT 0 1\nMEASURE 0 ro[0]\nMEASURE 1 ro[1]",
    "addresses": {"ro": true},
    "trials": 1024
  }
}
```

**Key point:** QVM is a **separate server process**. You must start it before using pyQuil (`qvm -S -p 5000`). This is the only framework where the simulator runs out-of-process by default. The upside: the QVM binary can be used from any language that speaks HTTP/JSON.

#### D-Wave Neal: Direct Python (No Boundary)

| Step | What Happens | Format |
|---|---|---|
| 1. User builds BQM | `dimod.BinaryQuadraticModel` with biases and couplings | Python objects |
| 2. Call sampler | `SimulatedAnnealingSampler().sample(bqm)` | Python method call |
| 3. Anneal | C extension runs simulated annealing iterations on BQM data | C/Python |
| 4. Return results | `SampleSet` with samples, energies, timing | Python objects |

**Key point:** Like Cirq's built-in simulator, there's no serialization boundary. The BQM object is read directly by the sampler code.

### Architecture Comparison

| Framework | Boundary Type | Serialization Format | Process Model | Standalone Simulator? |
|---|---|---|---|---|
| **Qiskit Aer** | Pybind11 (in-process) | Qobj dict | Same process | ✗ (Python extension) |
| **Cirq built-in** | None (pure Python) | N/A (numpy arrays) | Same process | ✗ (Python only) |
| **Cirq + qsim** | Pybind11 (in-process) | Protocol Buffers | Same process | ✗ (Python extension) |
| **Q# / QDK** | PyO3 (in-process) | Q# source → QIR | Same process | ✅ (Rust binary exists) |
| **QVM** | HTTP + JSON-RPC | Quil text in JSON | **Separate process** | ✅ (Common Lisp binary) |
| **D-Wave Neal** | None (Python/C) | N/A (Python objects) | Same process | ✗ (Python only) |

### Implications for Non-Python Usage

| Simulator | Usable without Python? | How? |
|---|---|---|
| Qiskit Aer | ✗ | Embedded in Python via Pybind11 |
| Cirq | ✗ | Pure Python |
| qsim | ⚠️ Partially | C++ library with its own API, but typically used via Cirq |
| Q# simulator | ✅ | `qsc` Rust compiler + simulator can run standalone |
| QVM | ✅ | Standalone binary, accepts Quil over HTTP — use from any language |
| D-Wave Neal | ✗ | Python package |

### Simulator-to-Hardware Protocol Alignment

A critical architectural question: **does the simulator accept the same protocol/format as the real hardware?** If yes, switching from simulator to hardware is a one-line change. If no, there's a translation layer that could introduce discrepancies.

```
IDEAL:   Your code → [same format] → Simulator
         Your code → [same format] → Real Hardware
         (just change the target)

REALITY: Varies by framework ↓
```

#### Alignment by Framework

**IonQ Cloud Simulator — ✅ Identical to QPU**

The IonQ cloud simulator and QPU share the **exact same REST API**. The only difference is the `target` field:

```json
{"target": "simulator", ...}   // cloud simulator
{"target": "qpu.forte-1", ...} // real hardware
```

Same endpoint, same auth, same JSON circuit format, same response schema. Code is 100% portable — change one string and you're on hardware.

```
IonQ REST API (https://api.ionq.co/v0.4/jobs)
    ├── target: "simulator"  → IonQ cloud simulator (ideal, no noise)
    └── target: "qpu"       → IonQ trapped-ion QPU
    
    Same JSON circuit format for both.
```

**IBM Quantum + Qiskit Aer — ✅ Identical interface (Qobj)**

Both Aer and IBM hardware backends implement the same `Backend.run()` interface and accept the same **Qobj** format internally. Switching is a backend swap:

```python
# Simulator
backend = AerSimulator()
# Hardware — same .run() call, same circuit, same result format
backend = service.backend("ibm_kyiv")

job = backend.run(transpiled_circuit, shots=1024)  # identical call
```

The Qobj serialization is the same whether it goes to the C++ Aer engine or to the IBM cloud. The Primitives API (`Sampler`, `Estimator`) also provides an identical interface across local and remote backends.

```
Qiskit Backend.run(circuit)
    ├── AerSimulator     → Qobj → C++ Aer (local)
    └── IBMBackend       → Qobj → IBM Quantum cloud (remote)
    
    Same Qobj format for both.
```

**Google Quantum Engine + qsim — ✅ Identical via Engine API**

When used through Google Quantum Engine (GQE), both the qsim cloud simulator and real hardware (Sycamore/Willow) accept the **same protobuf circuit format** via the same API:

```python
engine = cirq_google.Engine(project_id="my-project")

# Simulator — same Engine API
result = engine.run(circuit, processor_id="qsimulator", repetitions=1000)
# Hardware — same Engine API
result = engine.run(circuit, processor_id="weber", repetitions=1000)
```

**However**, Cirq's **local** built-in simulator (`cirq.Simulator()`) does NOT use protobuf — it operates directly on Python objects. So there's a mismatch between local sim and cloud:

```
cirq.Simulator()              → Python objects (local, no protobuf)
cirq_google.Engine(qsimulator) → Protobuf (cloud, same as hardware) ✅
cirq_google.Engine(weber)      → Protobuf (cloud, real QPU)         ✅
```

**Q# / Azure Quantum — ✅ Same QIR for all targets**

Q# compiles to **QIR** (LLVM IR), which is the universal format for both simulators and hardware in Azure Quantum. The submission API is identical — only the `target` changes:

```python
# Local simulator — QIR executed by Rust simulator
qsharp.eval("BellPair()")

# Azure hardware — same QIR submitted to cloud
target = workspace.get_targets("ionq.qpu")
job = target.submit(qsharp.compile("BellPair()"))
```

```
Q# source → QIR (LLVM IR)
    ├── Local Rust simulator (interprets QIR)
    ├── Azure: ionq.qpu       (QIR → IonQ native)
    ├── Azure: quantinuum.qpu  (QIR → Quantinuum native)
    └── Azure: rigetti.qpu     (QIR → Quil native)
    
    Same QIR for all targets.
```

**Rigetti QVM — ✅ Same protocol as QPU**

QVM and the real QPU both accept **Quil text** via the **same rpcq (JSON-RPC over ZeroMQ)** protocol. pyQuil's `get_qc()` returns an object with identical `.compile()` and `.run()` methods:

```python
qc = get_qc('2q-qvm')     # QVM simulator — Quil via rpcq
qc = get_qc('Aspen-M-3')  # Real QPU — same Quil via rpcq

executable = qc.compile(program)  # same call
result = qc.run(executable)       # same call
```

```
pyQuil Program → Quil text → rpcq (JSON-RPC)
    ├── QVM server (localhost:5000)  → simulation
    └── QPU server (QCS endpoint)   → real hardware
    
    Same Quil text + rpcq protocol for both.
```

**D-Wave — ⚠️ Different protocol for local vs. cloud**

Local simulators (`dwave-neal`, `dimod.ExactSolver`) accept **Python BQM objects** directly. The D-Wave cloud (SAPI) accepts **JSON with Ising/QUBO data** over REST. The data is equivalent but the protocol differs:

```python
# Local — direct Python call
neal_sampler.sample(bqm)

# Cloud — BQM serialized to JSON, sent via REST
dwave_sampler.sample(bqm)  # dwave-cloud-client handles serialization
```

```
dimod BQM object
    ├── dwave-neal         → direct Python method call (local)
    └── dwave-cloud-client → serialize to JSON → SAPI REST API (cloud)
    
    Same BQM, but different wire protocols.
```

The `EmbeddingComposite(DWaveSampler())` wrapper makes this transparent — same `.sample()` call for both — but the underlying protocol is not the same.

#### Summary: Protocol Alignment

| Framework | Local Simulator Protocol | Hardware Protocol | Match? | Switch Effort |
|---|---|---|---|---|
| **IonQ (cloud sim)** | REST + JSON | REST + JSON | ✅ Identical | Change `target` string |
| **Qiskit / IBM** | Qobj via Pybind11 | Qobj via REST | ✅ Same format | Change `backend` |
| **Google / Engine** | Protobuf via gRPC | Protobuf via gRPC | ✅ Identical | Change `processor_id` |
| **Cirq local sim** | Python objects | Protobuf via gRPC | ✗ Different | Must use Engine API |
| **Q# / Azure** | QIR via PyO3 | QIR via REST | ✅ Same format | Change `target` |
| **Rigetti QVM** | Quil via rpcq | Quil via rpcq | ✅ Identical | Change `get_qc()` name |
| **D-Wave local** | Python objects | JSON via REST | ⚠️ Same data, different wire | Transparent via SDK |

**Key takeaway:** Most frameworks deliberately designed their simulators to use the **same protocol as hardware**, making the simulator → hardware transition a one-line change. The notable exception is **Cirq's local Python simulator**, which bypasses the protobuf serialization that real hardware uses.

---

## 1. Qiskit Simulators

### Install

```bash
pip install qiskit qiskit-aer
```

### Available Simulators

| Simulator | Method | State Type | Max Qubits (16 GB) | GPU | Noise | Best For |
|---|---|---|---|---|---|---|
| **Aer StatevectorSimulator** | `statevector` | Pure state (full amplitudes) | ~25 | ✅ | ✅ | Exact simulation, amplitude access |
| **Aer QasmSimulator** | `automatic` | Sampled (shot-based) | ~25 | ✅ | ✅ | Mimicking real hardware output |
| **Aer DensityMatrix** | `density_matrix` | Mixed state (density matrix) | ~13 | ✅ | ✅ | Noise modeling, open systems |
| **Aer Stabilizer** | `stabilizer` | Clifford tableau | ~1000s | ✗ | ✗ | QEC research, Clifford-only circuits |
| **Aer MPS** | `matrix_product_state` | Tensor network | ~50-100 (low ent.) | ✗ | ✅ | Low-entanglement circuits |
| **Aer Extended Stabilizer** | `extended_stabilizer` | Stabilizer + T-gates | ~40-60 | ✗ | ✗ | Near-Clifford circuits |
| **StatevectorSampler** | Reference primitive | Pure state | ~25 | ✗ | ✗ | Algorithm development, unit tests |
| **StatevectorEstimator** | Reference primitive | Pure state | ~25 | ✗ | ✗ | Expectation value computation |

### Usage Examples

```python
# Aer high-performance simulator
from qiskit_aer import AerSimulator
from qiskit import QuantumCircuit, transpile

qc = QuantumCircuit(2, 2)
qc.h(0)
qc.cx(0, 1)
qc.measure([0, 1], [0, 1])

# Shot-based (mimics hardware)
sim = AerSimulator(method='automatic')
result = sim.run(transpile(qc, sim), shots=1024).result()
print(result.get_counts())  # {'00': 512, '11': 512}

# Statevector (exact amplitudes)
sim_sv = AerSimulator(method='statevector')
result = sim_sv.run(transpile(qc, sim_sv)).result()

# Density matrix (with noise)
sim_dm = AerSimulator(method='density_matrix')

# GPU acceleration (requires CUDA + qiskit-aer-gpu)
sim_gpu = AerSimulator(method='statevector', device='GPU')

# Reference primitives (lightweight, no Aer dependency)
from qiskit.primitives import StatevectorSampler
sampler = StatevectorSampler()
result = sampler.run([qc]).result()
```

### Noise Modeling

```python
from qiskit_aer.noise import NoiseModel, depolarizing_error

noise = NoiseModel()
noise.add_all_qubit_quantum_error(depolarizing_error(0.01, 1), ['h', 'x'])
noise.add_all_qubit_quantum_error(depolarizing_error(0.02, 2), ['cx'])

sim = AerSimulator(noise_model=noise)
result = sim.run(transpile(qc, sim), shots=1024).result()
```

---

## 2. Cirq Simulators

### Install

```bash
pip install cirq
```

### Available Simulators

| Simulator | Class | State Type | Max Qubits (16 GB) | Noise | Best For |
|---|---|---|---|---|---|
| **State Vector** | `cirq.Simulator()` | Pure state | ~25 | ✅ (via channels) | General exact simulation |
| **Density Matrix** | `cirq.DensityMatrixSimulator()` | Mixed state | ~13 | ✅ | Noisy circuit simulation |
| **Clifford** | `cirq.CliffordSimulator()` | Stabilizer tableau | ~1000s | ✗ | QEC, Clifford-only |
| **MPS** | `cirq.MPSSimulator()` (contrib) | Tensor network | ~50-100 | ✗ | Low-entanglement circuits |

### Usage Examples

```python
import cirq

q0, q1 = cirq.LineQubit.range(2)
circuit = cirq.Circuit([
    cirq.H(q0),
    cirq.CNOT(q0, q1),
    cirq.measure(q0, q1, key='result')
])

# State vector simulation (exact)
sim = cirq.Simulator()
result = sim.run(circuit, repetitions=1024)
print(result.histogram(key='result'))  # {0: 512, 3: 512}

# Get full state vector (no measurement)
circuit_no_meas = cirq.Circuit([cirq.H(q0), cirq.CNOT(q0, q1)])
result = sim.simulate(circuit_no_meas)
print(result.final_state_vector)  # [0.707, 0, 0, 0.707]

# Density matrix simulation (for noise)
dm_sim = cirq.DensityMatrixSimulator()
result = dm_sim.simulate(circuit_no_meas)
print(result.final_density_matrix)

# Clifford simulator (efficient for Clifford gates)
cliff_sim = cirq.CliffordSimulator()
clifford_circuit = cirq.Circuit([cirq.H(q0), cirq.CNOT(q0, q1)])
result = cliff_sim.simulate(clifford_circuit)
```

### Noise Modeling

```python
# Add depolarizing noise after every gate
noisy_circuit = circuit.with_noise(cirq.ConstantQubitNoiseModel(
    qubit_noise_gate=cirq.DepolarizingChannel(p=0.01)
))
dm_sim = cirq.DensityMatrixSimulator()
result = dm_sim.run(noisy_circuit, repetitions=1024)
```

---

## 3. Microsoft QDK (Q#) Simulators

### Install

```bash
pip install qsharp
# Or: VS Code extension "Microsoft Quantum Development Kit"
```

### Available Simulators

| Simulator | Type | Max Qubits | Noise | Best For |
|---|---|---|---|---|
| **Full State Simulator** | State vector | ~25-28 | ✗ | Default, general purpose |
| **Sparse Simulator** | Sparse state vector | ~30-50 (if sparse) | ✅ (Pauli) | Structured circuits, fewer non-zero amplitudes |
| **Toffoli Simulator** | Classical logic only | ~millions | ✗ | Circuits with only X, CNOT, Toffoli (reversible classical) |
| **Resource Estimator** | Not a simulator — counts resources | Unlimited | ✅ (models) | Estimating qubits, gates, time for fault-tolerant execution |

### Usage Examples

```python
import qsharp

# Default simulator (full state)
qsharp.eval("operation Bell() : Result[] { use qs = Qubit[2]; H(qs[0]); CNOT(qs[0], qs[1]); [M(qs[0]), M(qs[1])] }")

# Resource estimation
qsharp.estimate("Bell()")  # Returns qubit count, gate counts, depth, etc.
```

```qsharp
// Q# with simulator selection
operation BellPair() : (Result, Result) {
    use (q0, q1) = (Qubit(), Qubit());
    H(q0);
    CNOT(q0, q1);
    let r0 = M(q0);
    let r1 = M(q1);
    Reset(q0);
    Reset(q1);
    return (r0, r1);
}
```

### Key Differentiator: Resource Estimator

Unlike simulators, the Resource Estimator doesn't execute the circuit — it analyzes it to estimate what fault-tolerant hardware would need:

```python
result = qsharp.estimate("BellPair()")
# Output: logical qubits, physical qubits, T-gates, runtime, error budget, etc.
```

This can handle circuits with **thousands of qubits** since it doesn't simulate state.

---

## 4. Quil / QVM Simulators

### Install

```bash
# Option A: Binary (recommended)
# Download from https://github.com/quil-lang/qvm/releases

# Option B: Via Conda / pip (pyQuil client)
pip install pyquil

# QVM must be running as a local server
qvm -S -p 5000    # Start QVM server
quilc -S -p 6000  # Start compiler server
```

### Available Simulators

| Simulator | Mode | Max Qubits (16 GB) | Noise | Best For |
|---|---|---|---|---|
| **QVM Wavefunction** | State vector | ~28-30 | ✗ | General Quil program simulation |
| **QVM Density Matrix** | Density matrix | ~14 | ✅ | Noisy Quil simulation |

### Usage Examples

```python
from pyquil import Program, get_qc
from pyquil.gates import H, CNOT, MEASURE
from pyquil.quilbase import Declare

# Build program
p = Program()
ro = p.declare('ro', 'BIT', 2)
p += H(0)
p += CNOT(0, 1)
p += MEASURE(0, ro[0])
p += MEASURE(1, ro[1])
p.wrap_in_numshots_loop(1024)

# Run on local QVM
qc = get_qc('2q-qvm')   # 2-qubit QVM
result = qc.run(qc.compile(p))
print(result.readout_data['ro'])
```

### Architecture Note

QVM runs as a **separate server process** (Common Lisp binary). pyQuil communicates with it via HTTP/rpcq. This is different from Qiskit/Cirq where the simulator is an in-process Python/C++ library.

---

## 5. D-Wave Local Simulators (Annealing)

### Install

```bash
pip install dwave-ocean-sdk
```

### Available Simulators

| Simulator | Class | Type | Max Variables | Best For |
|---|---|---|---|---|
| **SimulatedAnnealingSampler** | `dwave.neal.SimulatedAnnealingSampler` | Classical simulated annealing | ~10,000s | Testing QUBO/Ising without hardware |
| **ExactSolver** | `dimod.ExactSolver` | Brute-force enumeration | ~20 | Verifying small problems exactly |
| **RandomSampler** | `dimod.RandomSampler` | Random sampling | Unlimited | Baseline comparison |
| **ExactCQMSolver** | `dimod.ExactCQMSolver` | Brute-force for CQM | ~20 | Verifying constrained models |

### Usage Examples

```python
import dimod
from dwave.neal import SimulatedAnnealingSampler

# Define problem
Q = {(0, 0): -1, (1, 1): -1, (0, 1): 2}
bqm = dimod.BinaryQuadraticModel.from_qubo(Q)

# Simulated annealing (no quantum hardware needed)
sampler = SimulatedAnnealingSampler()
result = sampler.sample(bqm, num_reads=100, num_sweeps=1000)
print(result.first.sample, result.first.energy)

# Exact solver (brute force — only for small problems)
exact = dimod.ExactSolver()
result = exact.sample(bqm)
print(result.first.sample)  # Guaranteed optimal
```

### Note

These are **classical** simulators. They do not simulate quantum annealing physics — they use classical algorithms (simulated annealing, exhaustive search) to solve the same optimization problems. There is no local quantum annealing simulator that models the actual quantum tunneling process.

---

## 6. Standalone Simulators (Framework-Independent)

These are high-performance simulators that can be used independently or as backends for the frameworks above.

### Google qsim

```bash
pip install qsimcirq    # Cirq integration
```

| Feature | Detail |
|---|---|
| **Language** | C++ with Python bindings |
| **Max qubits** | 30-40 (single node) |
| **GPU** | ✅ via cuQuantum |
| **Integration** | Cirq backend |
| **Best for** | High-performance gate simulation, Google circuit benchmarks |

```python
import cirq
import qsimcirq

q0, q1 = cirq.LineQubit.range(2)
circuit = cirq.Circuit(cirq.H(q0), cirq.CNOT(q0, q1), cirq.measure(q0, q1))

sim = qsimcirq.QSimSimulator()
result = sim.run(circuit, repetitions=1024)
```

### QuEST (Quantum Exact Simulation Toolkit)

```bash
# Build from source: https://github.com/QuEST-Kit/QuEST
git clone https://github.com/QuEST-Kit/QuEST.git
cd QuEST && mkdir build && cd build && cmake .. && make
```

| Feature | Detail |
|---|---|
| **Language** | C with Python wrapper |
| **Max qubits** | 35-40 (CPU), 40+ (MPI/GPU) |
| **GPU** | ✅ CUDA |
| **MPI** | ✅ Distributed simulation |
| **Best for** | HPC, density matrix at scale, research benchmarks |

### PennyLane

```bash
pip install pennylane                    # default.qubit (Python)
pip install pennylane-lightning          # C++ optimized
pip install pennylane-lightning[gpu]     # GPU (cuQuantum)
```

| Simulator | Max Qubits | GPU | Best For |
|---|---|---|---|
| `default.qubit` | ~18-20 | ✗ | Prototyping, ML, autodiff |
| `lightning.qubit` | ~30-35 | ✗ | Fast CPU simulation |
| `lightning.gpu` | ~35-40+ | ✅ | Large circuits with GPU |
| `lightning.kokkos` | ~35-40+ | ✅ | Multi-backend (CPU/GPU) |

```python
import pennylane as qml

dev = qml.device("default.qubit", wires=2)

@qml.qnode(dev)
def bell():
    qml.Hadamard(wires=0)
    qml.CNOT(wires=[0, 1])
    return qml.probs(wires=[0, 1])

print(bell())  # [0.5, 0.0, 0.0, 0.5]
```

### ProjectQ

```bash
pip install projectq
```

| Feature | Detail |
|---|---|
| **Language** | Python with C++ backend |
| **Max qubits** | ~30-34 |
| **GPU** | ✗ |
| **Best for** | Algorithm prototyping, modular compilation |

```python
from projectq import MainEngine
from projectq.ops import H, CNOT, Measure, All

eng = MainEngine()
q = eng.allocate_qureg(2)
H | q[0]
CNOT | (q[0], q[1])
All(Measure) | q
eng.flush()
print(int(q[0]), int(q[1]))
```

### Stim (Stabilizer Simulator)

```bash
pip install stim
```

| Feature | Detail |
|---|---|
| **Language** | C++ with Python bindings (pybind11) |
| **Max qubits** | **Millions** (Clifford-only) |
| **GPU** | ✗ |
| **Repo** | [quantumlib/Stim](https://github.com/quantumlib/Stim) |
| **Best for** | Quantum error correction research, surface codes, Clifford benchmarks |

Stim is by far the fastest Clifford circuit simulator — it can simulate **billions of gates on millions of qubits** in seconds. It's purpose-built for QEC research, not general-purpose quantum computing.

```python
import stim

# Build circuit using Stim's text format or API
circuit = stim.Circuit("""
    H 0
    CNOT 0 1
    M 0 1
""")

# Or programmatic API
circuit = stim.Circuit()
circuit.append("H", [0])
circuit.append("CNOT", [0, 1])
circuit.append("M", [0, 1])

# Sample measurements
sampler = circuit.compile_sampler()
results = sampler.sample(shots=1024)  # numpy bool array
```

**Protocol:** Circuits can be passed as either a **text string** (Stim's own format, similar to Quil/QASM) parsed by C++, or built via the Python API which creates C++ objects directly via **pybind11**. No serialization boundary for the API path — Python calls map directly to C++ `stim::Circuit` methods.

```
Stim Protocol:
  Option A: stim.Circuit("H 0\nCNOT 0 1")  → C++ parses text string
  Option B: circuit.append("H", [0])        → pybind11 → C++ method call
  
  Both produce C++ stim::Circuit object → Clifford tableau simulation
```

**Limitation:** Clifford gates only (H, CNOT, S, SWAP, etc.). No T gates, no Toffoli, no arbitrary rotations.

### Qrack

```bash
pip install pyqrack
```

| Feature | Detail |
|---|---|
| **Language** | C++ with OpenCL + Python bindings |
| **Max qubits** | ~35-40 (CPU), more with GPU |
| **GPU** | ✅ OpenCL (NVIDIA, AMD, Intel) |
| **Repo** | [vm6502q/qrack](https://github.com/vm6502q/qrack) |
| **Best for** | GPU-accelerated simulation, hybrid optimization tricks |

Qrack uses aggressive optimization: Schmidt decomposition, hybrid stabilizer methods, and OpenCL GPU acceleration to push beyond raw state-vector limits.

```python
from pyqrack import QrackSimulator

sim = QrackSimulator(3)   # 3 qubits
sim.h(0)                   # Hadamard
sim.mcx([0], 1)            # CNOT (multi-controlled X)
result = sim.measure_all() # Measure
```

**Protocol:** Qrack uses an **imperative gate-by-gate API** — no circuit object is passed as a block. Each Python method call (`sim.h(0)`) maps via pybind11/ctypes directly to a C++ method that immediately applies the gate to the state vector (potentially on GPU via OpenCL). There is no serialization, no circuit object, no batch submission.

```
Qrack Protocol:
  Python: sim.h(0)     → pybind11/ctypes → C++ QrackSimulator::H(0)
  Python: sim.mcx(...)  → pybind11/ctypes → C++ QrackSimulator::MCX(...)
  
  Gate-by-gate, immediate execution. No circuit serialization.
  OpenCL kernels dispatched internally by C++ — not user-facing.
```

### Intel Quantum Simulator (Intel-QS / qHiPSTER)

```bash
# Build from source: https://github.com/intel/intel-qs
git clone https://github.com/intel/intel-qs.git
cd intel-qs && mkdir build && cd build && cmake .. && make
```

| Feature | Detail |
|---|---|
| **Language** | C++ |
| **Max qubits** | ~40+ (MPI distributed) |
| **GPU** | ✗ |
| **MPI** | ✅ Distributed across HPC nodes |
| **Repo** | [intel/intel-qs](https://github.com/intel/intel-qs) |
| **Best for** | HPC clusters, large-scale simulation on supercomputers |

```cpp
#include "qureg.hpp"

QubitRegister<ComplexDP> psi(num_qubits, "base", 0);
psi.ApplyHadamard(0);
psi.ApplyCPauliX(0, 1);  // CNOT
// MPI handles distributed state automatically
```

**Protocol:** Pure C++ imperative API — no Python bindings, no circuit object, no serialization. Each gate is a **C++ method call** on the `QubitRegister` object. MPI communication for distributed qubits is handled internally and transparently. There is no QASM/JSON input; you write your circuit as C++ code.

```
Intel-QS Protocol:
  C++: psi.ApplyHadamard(0)   → local state-vector operation
  C++: psi.ApplyCPauliX(0,1)  → if qubits span MPI ranks:
                                   internal MPI_Send/Recv to coordinate
                                 else:
                                   local matrix multiply
  
  Gate-by-gate C++ calls. MPI abstracted away from user.
  No Python. No serialization. No circuit object.
```

### Quantum++ (qpp)

```bash
# Header-only C++ library — just include it
git clone https://github.com/softwareQinc/qpp.git
# #include <qpp/qpp.h> in your code
```

| Feature | Detail |
|---|---|
| **Language** | C++17 (header-only) |
| **Max qubits** | ~30 |
| **GPU** | ✗ |
| **Dependencies** | Eigen3 only |
| **Repo** | [softwareQinc/qpp](https://github.com/softwareQinc/qpp) |
| **Best for** | Portable C++ simulation, zero-dependency embedding, research |

Quantum++ is the most portable option — header-only, depends only on Eigen3, compiles anywhere with a C++17 compiler.

```cpp
#include <qpp/qpp.h>
using namespace qpp;

QCircuit qc{2};
qc.gate(gt.H, 0);
qc.CTRL(gt.X, {0}, 1);
qc.measure_all();

QEngine engine{qc};
engine.execute();
std::cout << engine.get_dit(0) << engine.get_dit(1);
```

**Protocol:** Has a proper **`QCircuit` object** (unlike Qrack/Intel-QS) that is passed to `QEngine` for execution. But this is entirely in C++ — the circuit object is a C++ struct passed by reference. No serialization, no Python, no network.

```
Quantum++ Protocol:
  C++: QCircuit qc{2}         → build circuit object in memory
  C++: qc.gate(gt.H, 0)       → append gate to circuit's internal list
  C++: QEngine engine{qc}     → pass circuit by reference to engine
  C++: engine.execute()        → iterate gates, apply matrix operations
  
  Circuit object exists, but pure C++. No serialization boundary.
```

### Qulacs

```bash
pip install qulacs
```

| Feature | Detail |
|---|---|
| **Language** | C++ with Python bindings (pybind11) |
| **Max qubits** | ~30-35 |
| **GPU** | ✅ CUDA |
| **Repo** | [qulacs/qulacs](https://github.com/qulacs/qulacs) |
| **Best for** | Fast variational circuit simulation, Japanese research community |

Very fast state-vector simulator, often benchmarks at the top for variational circuit workloads.

```python
from qulacs import QuantumState, QuantumCircuit

state = QuantumState(2)
circuit = QuantumCircuit(2)
circuit.add_H_gate(0)
circuit.add_CNOT_gate(0, 1)
circuit.update_quantum_state(state)  # apply circuit to state

print(state.get_vector())
```

**Protocol:** Python `QuantumCircuit` is a **thin wrapper** around a C++ object via pybind11. The circuit lives entirely in C++ memory. `update_quantum_state()` passes the C++ circuit to the C++ simulator engine by reference — no serialization, no copy. The Python object is just a handle.

```
Qulacs Protocol:
  Python: QuantumCircuit(2)         → pybind11 → C++ QuantumCircuit allocated
  Python: circuit.add_H_gate(0)     → pybind11 → C++ method appends gate
  Python: circuit.update_quantum_state(state)
                                    → pybind11 → C++ applies all gates to state
  
  Circuit is a C++ object. Python holds a pointer via pybind11.
  No serialization. No copy. Direct C++ execution.
```

### Yao.jl (Julia)

```julia
# In Julia REPL
using Pkg
Pkg.add("Yao")
Pkg.add("CUDA")  # optional, for GPU
```

| Feature | Detail |
|---|---|
| **Language** | Julia |
| **Max qubits** | ~30 (CPU), more with GPU |
| **GPU** | ✅ CUDA via CUDA.jl |
| **Autodiff** | ✅ Native Julia autodiff |
| **Repo** | [QuantumBFS/Yao.jl](https://github.com/QuantumBFS/Yao.jl) |
| **Best for** | Julia users, variational algorithms, GPU simulation |

```julia
using Yao

circuit = chain(2, put(1=>H), control(1, 2=>X))
state = zero_state(2)
result = apply!(state, circuit)

# GPU execution
using CUDA
gpu_state = cu(zero_state(2))
apply!(gpu_state, circuit)  # auto-dispatches GPU kernels
```

**Protocol:** Circuits are **Julia structs** (subtypes of `AbstractBlock`). The state register is a Julia array (`ArrayReg`). `apply!()` uses Julia's **multiple dispatch** — if the state is a `CuArray` (GPU), GPU kernels are dispatched; if `Array` (CPU), CPU code runs. No serialization, no FFI boundary — everything is native Julia.

```
Yao.jl Protocol:
  Julia: chain(2, put(1=>H), ...)  → Julia struct (AbstractBlock tree)
  Julia: zero_state(2)             → ArrayReg (CPU) or cu(zero_state(2)) (GPU)
  Julia: apply!(state, circuit)    → multiple dispatch:
                                       Array → CPU matrix operations
                                       CuArray → CUDA GPU kernels
  
  Pure Julia. No FFI. No serialization. GPU via type dispatch.
```

---

## Comparison Summary

### Gate-Based Simulators

| Simulator | Framework | Language | Max Qubits (16GB) | GPU | Noise | Install Complexity |
|---|---|---|---|---|---|---|
| Qiskit Aer | Qiskit | C++ | ~25 (SV), ~13 (DM) | ✅ | ✅ | `pip install` |
| Cirq Simulator | Cirq | Python/NumPy | ~25 (SV), ~13 (DM) | ✗ | ✅ | `pip install` |
| Q# Sparse | QDK | Rust | ~30-50 (sparse) | ✗ | ✅ (Pauli) | `pip install` |
| QVM | Quil | Common Lisp | ~28-30 (WF) | ✗ | ✅ (DM mode) | Binary + server |
| qsim | Cirq | C++ | ~35-40 | ✅ | ✗ | `pip install` |
| QuEST | Standalone | C | ~35-40 | ✅ | ✅ | Build from source |
| PennyLane Lightning | PennyLane | C++ | ~30-40 | ✅ | ✗ | `pip install` |
| ProjectQ | Standalone | Python/C++ | ~30-34 | ✗ | ✗ | `pip install` |
| Qulacs | Standalone | C++ | ~30-35 | ✅ | ✗ | `pip install` |
| Qrack | Standalone | C++ (OpenCL) | ~35-40 | ✅ | ✅ | `pip install` |
| Intel-QS | Standalone | C++ | ~40+ (MPI) | ✗ | ✗ | Build from source |
| Quantum++ | Standalone | C++17 | ~30 | ✗ | ✅ | Header-only |
| Yao.jl | Standalone | Julia | ~30 | ✅ | ✗ | Julia `Pkg.add` |

### Annealing Simulators

| Simulator | Framework | Type | Max Variables | Install |
|---|---|---|---|---|
| dwave-neal | D-Wave Ocean | Simulated annealing | ~10,000s | `pip install` |
| dimod ExactSolver | D-Wave Ocean | Brute-force | ~20 | `pip install` |

### Specialized / Efficient Simulators

| Simulator | Technique | Max Qubits | Limitation |
|---|---|---|---|
| **Stim** | Clifford tableau (optimized) | **millions** | Clifford gates only — fastest in class |
| Stabilizer (Qiskit/Cirq) | Clifford tableau | ~1000s | Clifford gates only (no T/Toffoli) |
| MPS (Qiskit/Cirq) | Tensor network | ~50-100 | Low entanglement only |
| Extended Stabilizer (Qiskit) | Stabilizer + few T | ~40-60 | Near-Clifford circuits |
| Q# Toffoli Simulator | Classical logic | ~millions | X, CNOT, Toffoli only (reversible classical) |
| Q# Resource Estimator | Static analysis | Unlimited | Not a simulator — counts resources |

### Feature Comprehensiveness Ranking

**Qiskit Aer** is the most feature-complete simulator — no other single tool covers as many simulation scenarios:

| Feature | Qiskit Aer | Cirq | Q# | QVM | qsim | QuEST | Qulacs | Quantum++ |
|---|---|---|---|---|---|---|---|---|
| State vector | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Density matrix | ✅ | ✅ | ✗ | ✅ | ✗ | ✅ | ✗ | ✅ |
| Stabilizer (Clifford) | ✅ | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Extended stabilizer | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| MPS (tensor network) | ✅ | ⚠️ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Custom noise models | ✅ | ✅ | ⚠️ | ✅ | ✗ | ✅ | ✗ | ✅ |
| Device-calibrated noise | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| GPU acceleration | ✅ | ✗ | ✗ | ✗ | ✅ | ✅ | ✅ | ✗ |
| Pulse-level simulation | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Auto method selection | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Save/restore snapshots | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |

### Can Aer Be Used from C++ Directly?

**Technically yes (it builds), practically no (no usable API).**

Aer's simulation engine is C++, but it was designed as a Python extension, not a standalone C++ library:

| Aspect | Status |
|---|---|
| C++ source code available | ✅ (`qiskit-aer/src/`) |
| Builds with CMake | ✅ Produces `.so`/`.a` files |
| Stable public C++ API | ✗ **Does not exist** |
| C++ API documentation | ✗ None |
| Input format expected | **Qobj JSON** — the Python serialization format |
| C++ circuit construction | ✗ No API — must hand-craft JSON |
| ABI stability | ✗ Internal classes change without notice |

The core issue: Aer's C++ backend expects circuits as **Qobj JSON dictionaries**, which is the format Qiskit's Python layer produces. There is no C++ circuit builder, no documented class hierarchy for external use, and no stable ABI. You would have to reverse-engineer the internal headers and hand-construct JSON payloads.

### Best C++ Simulators for Non-Python Usage

If you want Aer-like features from pure C++ without Python, here are the best alternatives ranked by feature coverage:

| Simulator | SV | DM | Noise | GPU | MPI | Circuit Object | C++ API Quality | Closest to Aer? |
|---|---|---|---|---|---|---|---|---|
| **QuEST** | ✅ | ✅ | ✅ | ✅ CUDA | ✅ | ✗ (gate-by-gate) | ✅ Stable C API | **★ Best overall** |
| **Quantum++** | ✅ | ✅ | ✅ | ✗ | ✗ | ✅ `QCircuit` | ✅ Clean C++17 | Best for portability |
| **Qrack** | ✅ | ✅ | ✅ | ✅ OpenCL | ✗ | ✗ (gate-by-gate) | ✅ Documented | Best GPU (AMD+NVIDIA) |
| **Intel-QS** | ✅ | ✗ | ✗ | ✗ | ✅ | ✗ (gate-by-gate) | ✅ Stable | Best for HPC clusters |
| **qsim** | ✅ | ✗ | ✗ | ✅ cuQuantum | ✗ | ✅ (protobuf) | ✅ Usable | Fastest raw SV speed |
| **Qiskit Aer** | ✅ | ✅ | ✅ | ✅ | ✗ | ✗ (Qobj JSON) | ✗ **No public API** | Most features, Python-only |

**Recommendation:** For a C++ quantum simulator that comes closest to Aer's feature set, use **QuEST** — it has state vector, density matrix, noise, GPU, and MPI with a proper C API designed for external use. For the cleanest C++ experience with a circuit object, use **Quantum++**.

---

## Choosing a Local Simulator

| I want to... | Use |
|---|---|
| Quickly test a small circuit (<20 qubits) | Qiskit `StatevectorSampler` or Cirq `Simulator` |
| Simulate with realistic noise | Qiskit Aer (density matrix + NoiseModel) |
| Push to 30+ qubits on CPU | qsim, PennyLane Lightning, Qulacs, or Q# Sparse |
| Use GPU acceleration | Qiskit Aer GPU, qsim (cuQuantum), PennyLane Lightning GPU, Qrack (OpenCL), Qulacs (CUDA) |
| Simulate QEC / Clifford circuits at massive scale | **Stim** (millions of qubits, billions of gates) |
| Simulate QEC / Clifford circuits (smaller) | Any Stabilizer simulator (~1000s of qubits) |
| Estimate resources for large algorithms | Q# Resource Estimator (not a sim, but invaluable) |
| Test annealing/optimization problems locally | dwave-neal SimulatedAnnealingSampler |
| Verify small optimization problems exactly | dimod ExactSolver |
| Run variational / ML quantum workflows | PennyLane (autodiff-native) |
| Simulate in pure C++ (no Python) | Quantum++ (header-only), Intel-QS (MPI), Qrack |
| Simulate in Julia | Yao.jl |
| Run on HPC cluster with MPI | Intel-QS, QuEST |

### Protocol Comparison for Standalone Simulators

| Simulator | Input Protocol | Circuit Object? | Serialization? | Language Boundary |
|---|---|---|---|---|
| **qsim** | Protobuf via pybind11 | Cirq Circuit → protobuf | ✅ Protocol Buffers | Python → C++ |
| **QuEST** | C API calls | ✗ (gate-by-gate) | ✗ | C (native) |
| **PennyLane** | QNode decorator | ✗ (tape-based) | ✗ | Python → C++ (Lightning) |
| **ProjectQ** | Operator overloading (`H \| q[0]`) | ✗ (engine-based) | ✗ | Python → C++ |
| **Stim** | Text string or pybind11 API | ✅ `stim.Circuit` | ✗ (pybind11 direct) | Python → C++ |
| **Qrack** | Imperative API calls | ✗ (gate-by-gate) | ✗ | Python → C++ (OpenCL) |
| **Intel-QS** | C++ method calls | ✗ (gate-by-gate) | ✗ | C++ only (+ MPI internal) |
| **Quantum++** | C++ `QCircuit` object | ✅ `QCircuit` → `QEngine` | ✗ | C++ only |
| **Qulacs** | pybind11 wrapper objects | ✅ `QuantumCircuit` | ✗ (pointer via pybind11) | Python → C++ |
| **Yao.jl** | Julia structs + dispatch | ✅ `AbstractBlock` tree | ✗ | Julia only (GPU via dispatch) |

## References

- [Qiskit Aer Documentation](https://qiskit.github.io/qiskit-aer/)
- [Cirq Simulation Guide](https://quantumai.google/cirq/simulate)
- [Q# Simulators Overview](https://learn.microsoft.com/en-us/azure/quantum/user-guide/machines/)
- [QVM Documentation](https://github.com/quil-lang/qvm)
- [D-Wave Neal](https://docs.ocean.dwavesys.com/en/stable/docs_neal/)
- [qsim GitHub](https://github.com/quantumlib/qsim)
- [QuEST GitHub](https://github.com/QuEST-Kit/QuEST)
- [PennyLane Lightning](https://github.com/PennyLaneAI/pennylane-lightning)
- [ProjectQ GitHub](https://github.com/ProjectQ-Framework/ProjectQ)
- [Stim GitHub](https://github.com/quantumlib/Stim)
- [Qrack GitHub](https://github.com/vm6502q/qrack)
- [Intel-QS GitHub](https://github.com/intel/intel-qs)
- [Quantum++ GitHub](https://github.com/softwareQinc/qpp)
- [Qulacs GitHub](https://github.com/qulacs/qulacs)
- [Yao.jl GitHub](https://github.com/QuantumBFS/Yao.jl)
- [Awesome Quantum Software](https://github.com/qosf/awesome-quantum-software)
