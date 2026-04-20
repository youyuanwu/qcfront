# Quil Language & QVM (Quantum Virtual Machine)

> Repository: <https://github.com/quil-lang/qvm>

## Overview

Quil (Quantum Instruction Language) is a low-level quantum programming language developed by Rigetti Computing, designed as the "assembly language" for quantum computers. The **QVM** (Quantum Virtual Machine) is a high-performance simulator that executes Quil programs on classical hardware. Together with the **Quilc** compiler and **pyQuil** Python SDK, they form Rigetti's open-source quantum computing stack.

## Ecosystem Overview

```
┌──────────────────────────────────────────────────────────┐
│             User Code (Python via pyQuil)                 │
├──────────────────────────────────────────────────────────┤
│                    pyQuil SDK                             │
│          Program construction + job management            │
├──────────────────────────────────────────────────────────┤
│                  Quilc Compiler                           │
│  Parsing → Decomposition → Optimization → Native Gates   │
├──────────────────────────────────────────────────────────┤
│            Native Quil (hardware-ready)                   │
├──────────────┬───────────────────────────────────────────┤
│     QVM      │        Rigetti QPU                        │
│  (Simulator) │    (via QCS API)                          │
├──────────────┴───────────────────────────────────────────┤
│           Results (measurement bitstrings)                │
└──────────────────────────────────────────────────────────┘
```

### Related Repositories

| Repository | Description | Language |
|---|---|---|
| [quil-lang/qvm](https://github.com/quil-lang/qvm) | Quantum Virtual Machine (simulator) | Common Lisp |
| [quil-lang/quilc](https://github.com/quil-lang/quilc) | Quil compiler (to native gates) | Common Lisp |
| [quil-lang/quil](https://github.com/quil-lang/quil) | Quil language specification | — |
| [rigetti/pyquil](https://github.com/rigetti/pyquil) | Python SDK for Quil | Python |
| [rigetti/qcs-sdk-python](https://github.com/rigetti/qcs-sdk-python) | QCS Python SDK | Python/Rust |

## The Quil Language

Quil is a **gate-based quantum instruction language** — similar in spirit to classical assembly:

```quil
# Declare classical memory
DECLARE ro BIT[2]

# Quantum gates
H 0
CNOT 0 1

# Measurement
MEASURE 0 ro[0]
MEASURE 1 ro[1]
```

### Key Language Features

| Feature | Syntax | Description |
|---|---|---|
| **Gate application** | `H 0`, `CNOT 0 1` | Apply quantum gate to qubit(s) |
| **Parameterized gates** | `RX(pi/2) 0` | Gates with angle parameters |
| **Measurement** | `MEASURE 0 ro[0]` | Measure qubit into classical register |
| **Classical memory** | `DECLARE ro BIT[2]` | Declare classical bit/int/real registers |
| **Gate definitions** | `DEFGATE ... AS MATRIX` | Define custom gates via unitary matrices |
| **Classical control** | `JUMP`, `JUMP-WHEN`, `JUMP-UNLESS` | Conditional branching on measurement results |
| **Pragmas** | `PRAGMA ...` | Compiler hints and device-specific directives |
| **RESET** | `RESET 0` | Reset qubit to |0⟩ |

### Quil Program Structure

```quil
# Full Quil program example: GHZ state
DECLARE ro BIT[3]

# Prepare GHZ state
H 0
CNOT 0 1
CNOT 1 2

# Measure all qubits
MEASURE 0 ro[0]
MEASURE 1 ro[1]
MEASURE 2 ro[2]
```

## QVM Architecture (Simulator)

The QVM is a **classical simulator** written in Common Lisp that interprets and executes Quil programs.

### QVM Repository Structure

```
qvm/
├── src/
│   ├── qvm.lisp             # Main QVM entry point and execution loop
│   ├── classical-memory.lisp # Classical register management
│   ├── wavefunction.lisp     # State vector representation
│   ├── density-matrix.lisp   # Density matrix simulation
│   ├── instruction.lisp      # Quil instruction interpretation
│   ├── gate.lisp             # Gate application logic
│   ├── measurement.lisp      # Measurement and sampling
│   ├── noise.lisp            # Noise model support
│   └── compile-gate.lisp     # Gate compilation utilities
├── app-ng/                    # HTTP server for QVM-as-a-service
├── tests/                     # Test suite
├── Makefile                   # Build system
└── qvm.asd                   # ASDF system definition (Lisp build)
```

### QVM Simulation Modes

| Mode | Description | Use Case |
|---|---|---|
| **Pure state** | Full state vector simulation | Exact simulation of ideal circuits |
| **Density matrix** | Full density matrix | Noise modeling, mixed states |
| **Stabilizer / Clifford** | Efficient Clifford simulation | Circuits with only Clifford gates |

### QVM Execution Flow

```
Quil Program (text)
    ↓
Parse to instruction list
    ↓
For each instruction:
    ├── Gate? → Apply unitary to state vector
    ├── MEASURE? → Sample from qubit probability, collapse state
    ├── Classical op? → Update classical registers
    └── Control flow? → Jump to target instruction
    ↓
Return classical register contents
```

### QVM Server Mode

The QVM can run as an HTTP server, accepting Quil programs via JSON-RPC:

```bash
qvm -S -p 5000   # Start QVM server on port 5000
```

```json
// JSON-RPC request
{
  "type": "multishot",
  "qubits": 2,
  "trials": 1000,
  "compiled-quil": "H 0\nCNOT 0 1\nMEASURE 0 ro[0]\nMEASURE 1 ro[1]"
}
```

## Quilc Compiler: Quil → Native Gates

Quilc is the **optimizing compiler** that translates general Quil into hardware-native Quil.

### Quilc Repository Structure

```
quilc/
├── src/
│   ├── quilc.lisp              # Main compiler entry point
│   ├── parser.lisp             # Quil text → AST
│   ├── ast.lisp                # Abstract syntax tree representation
│   ├── compressor/             # Gate sequence optimization
│   ├── addresser/              # Qubit mapping and routing
│   ├── chip/
│   │   ├── chip-specification.lisp  # Hardware topology definition
│   │   └── chip-reader.lisp         # Parse ISA JSON
│   ├── clifford/               # Clifford gate utilities
│   └── translators/            # Gate decomposition routines
├── app/                         # CLI application
├── tests/                       # Test suite
└── quilc.asd                   # ASDF system definition
```

### Compilation Pipeline

```
Input: General Quil + Chip Specification (ISA JSON)
    ↓
1. PARSING
   Quil text → Internal AST representation
    ↓
2. GATE DECOMPOSITION (translators/)
   Arbitrary gates → Sequences of 1-qubit + 2-qubit gates
   e.g., CCNOT → CNOT + single-qubit rotation sequences
    ↓
3. QUBIT ADDRESSING (addresser/)
   Logical qubits → Physical qubits
   Insert SWAP gates for connectivity constraints
    ↓
4. NATIVE GATE TRANSLATION
   Generic 2-qubit gates → Hardware native gates
   e.g., CNOT → CZ + single-qubit rotations
    ↓
5. OPTIMIZATION (compressor/)
   Merge adjacent single-qubit gates
   Cancel redundant operations
   Reduce circuit depth
    ↓
Output: Native Quil (only uses hardware-supported gates)
```

### Chip Specification (ISA)

Hardware topology is described as a JSON ISA (Instruction Set Architecture):

```json
{
  "1Q": {
    "0": {"type": "Xhalves"},
    "1": {"type": "Xhalves"},
    "2": {"type": "Xhalves"}
  },
  "2Q": {
    "0-1": {"type": "CZ"},
    "1-2": {"type": "CZ"}
  }
}
```

This tells quilc:
- Which qubits exist and what single-qubit gates they support
- Which qubit pairs are connected and what two-qubit gates they support

### Rigetti Native Gates

| Gate | Type | Description |
|---|---|---|
| `RX(θ)` | Single-qubit | Rotation around X axis |
| `RZ(θ)` | Single-qubit | Rotation around Z axis (virtual, zero-duration) |
| `CZ` | Two-qubit | Controlled-Z (native entangling gate) |
| `I` | Single-qubit | Identity |
| `XY(θ)` | Two-qubit | Parametric XY interaction (on some devices) |

### Compilation Example

**Input (general Quil):**
```quil
H 0
CNOT 0 1
```

**Output (native Quil for Rigetti Aspen):**
```quil
RZ(pi/2) 0
RX(pi/2) 0
RZ(pi/2) 0
RZ(-pi/2) 1
RX(pi/2) 1
CZ 0 1
RZ(-pi/2) 1
RX(-pi/2) 1
RZ(pi/2) 1
```

## Python Frontend: pyQuil

pyQuil is the Python SDK that provides the user-facing API:

```python
from pyquil import Program, get_qc
from pyquil.gates import H, CNOT, MEASURE
from pyquil.quilbase import Declare

# Construct program
p = Program()
ro = p.declare('ro', 'BIT', 2)
p += H(0)
p += CNOT(0, 1)
p += MEASURE(0, ro[0])
p += MEASURE(1, ro[1])

# Choose target (QVM simulator or real QPU)
qc = get_qc('2q-qvm')       # Simulator
# qc = get_qc('Aspen-M-3')  # Real hardware

# Compile and run
executable = qc.compile(p)   # Calls quilc for native gate compilation
result = qc.run(executable)
print(result.readout_data['ro'])
```

### pyQuil Execution Flow

```
Python (pyQuil Program object)
    ↓
Serialize to Quil text
    ↓
Send to Quilc for compilation (via rpcq)
    ↓
Receive native Quil
    ↓
Send to QVM (simulation) or QPU (hardware) via rpcq/QCS
    ↓
Receive measurement results
    ↓
Return as numpy array
```

## Hardware Interaction: Rigetti QCS

### Rigetti QCS (Quantum Cloud Services) API

```
Documentation: https://docs.api.qcs.rigetti.com
Authentication: OAuth2 Bearer JWT (via Okta)
```

**API Model:**
- **REST endpoints** — account management, reservations, device topology
- **gRPC/rpcq** — actual QPU job execution (abstracted by SDKs)

### REST Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/v1/quantumProcessors` | GET | List available QPUs |
| `/v1/quantumProcessors/{id}/instructionSetArchitecture` | GET | Get ISA (native gates, connectivity) |
| `/v1/reservations` | POST/GET | Manage QPU time reservations |
| `/v1/engagements` | POST | Start a QPU engagement session |

### QPU Execution Protocol

```
1. Authenticate (OAuth2 → JWT)
2. Reserve QPU time (POST /v1/reservations)
3. Get ISA for target QPU (GET /v1/quantumProcessors/{id}/instructionSetArchitecture)
4. Compile program using quilc (with ISA)
5. Start engagement (POST /v1/engagements)
6. Submit compiled Quil via rpcq (ZeroMQ-based RPC)
7. Receive results (bitstring arrays)
```

### QCS Python SDK

```python
from qcs_sdk.qvm import QVMClient
from qcs_sdk.compiler.quilc import QuilcClient

# Low-level API
quilc_client = QuilcClient.new_http("http://localhost:6000")
qvm_client = QVMClient.new_http("http://localhost:5000")

# Or high-level via pyQuil
from pyquil import get_qc
qc = get_qc('Aspen-M-3')
```

## Key Differentiators

- **Assembly-level language**: Quil is the lowest-level quantum programming language in common use — closest to hardware instructions
- **Common Lisp implementation**: QVM and Quilc are written in Common Lisp, optimized for mathematical performance
- **Separation of concerns**: Distinct tools for language (Quil), compilation (Quilc), simulation (QVM), and hardware access (QCS)
- **Classical control flow**: Quil supports conditional jumps based on measurement results — real-time classical feedback
- **Open ISA specification**: Hardware topology and gate support described in machine-readable JSON
- **rpcq protocol**: Custom ZeroMQ-based RPC for low-latency QPU communication

## References

- [QVM GitHub](https://github.com/quil-lang/qvm)
- [Quilc GitHub](https://github.com/quil-lang/quilc)
- [Quil Language Specification](https://github.com/quil-lang/quil)
- [pyQuil Documentation](https://pyquil-docs.rigetti.com/)
- [Rigetti QCS Documentation](https://docs.rigetti.com/qcs)
- [QCS API Reference](https://docs.api.qcs.rigetti.com)
- [qcs-sdk-python](https://github.com/rigetti/qcs-sdk-python)
