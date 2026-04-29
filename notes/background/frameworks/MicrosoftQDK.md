# Microsoft Quantum Development Kit (QDK)

> Repository: <https://github.com/microsoft/qdk>

## Overview

The Microsoft Quantum Development Kit (QDK) is Microsoft's full-stack quantum computing platform. It centers on **Q#**, a domain-specific programming language for quantum algorithms, and uses **Azure Quantum** as its cloud execution layer. The QDK supports simulation on classical hardware as well as real quantum hardware from providers like IonQ, Quantinuum, and Rigetti — all routed through Azure.

## Architecture & Key Components

```
┌──────────────────────────────────────────────────────────┐
│                     User Code (Q#)                       │
├──────────────────────────────────────────────────────────┤
│                    Q# Compiler                           │
│  Parsing → Semantic Analysis → Code Generation           │
├──────────────────────────────────────────────────────────┤
│           QIR (Quantum Intermediate Representation)      │
│                  LLVM IR extension                       │
├──────────────────────────────────────────────────────────┤
│                Azure Quantum Service                     │
│      Workspace → Provider → Target selection             │
├───────────┬──────────────┬───────────────────────────────┤
│  IonQ API │ Quantinuum   │ Rigetti QCS API               │
│  (JSON)   │ (QIR/OpenQASM│ (Quil)                        │
├───────────┴──────────────┴───────────────────────────────┤
│                  Quantum Hardware                        │
└──────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Description |
|---|---|
| **Q# Language** | Dedicated quantum programming language with classical control flow, qubit management, and quantum-specific type system |
| **Q# Compiler** | Compiles Q# source into QIR. Handles parsing, semantic analysis, optimization, and code generation |
| **QIR** | Quantum Intermediate Representation — an LLVM IR extension that is hardware-agnostic |
| **Resource Estimator** | Static analysis tool that counts qubits, gates, depth without simulation |
| **Azure Quantum SDK** | Python/CLI layer for workspace management, job submission, and result retrieval |
| **Simulators** | Full-state simulator, Toffoli simulator, sparse simulator, noise simulator |

### Repository Structure (Simplified)

```
qdk/
├── compiler/           # Q# compiler (Rust-based as of modern QDK)
├── library/            # Q# standard library (quantum operations, math, etc.)
├── resource_estimator/ # Resource estimation engine
├── pip/                # Python package (qsharp)
├── vscode/             # VS Code extension
├── samples/            # Example Q# programs
└── wasm/               # WebAssembly bindings for browser-based Q#
```

## Compilation Pipeline: Q# → Hardware Instructions

### Stage 1: Q# Source Code

```qsharp
operation BellPair() : (Result, Result) {
    use (q0, q1) = (Qubit(), Qubit());
    H(q0);
    CNOT(q0, q1);
    return (M(q0), M(q1));
}
```

### Stage 2: Q# Compiler → QIR

The Q# compiler (rewritten in Rust in modern QDK) produces **QIR** — an LLVM IR extension where quantum operations are represented as calls to runtime intrinsic functions.

```llvm
; QIR output (LLVM IR)
declare void @__quantum__qis__h__body(%Qubit*)
declare void @__quantum__qis__cx__body(%Qubit*, %Qubit*)
declare %Result* @__quantum__qis__m__body(%Qubit*)

define void @BellPair() {
entry:
  %q0 = call %Qubit* @__quantum__rt__qubit_allocate()
  %q1 = call %Qubit* @__quantum__rt__qubit_allocate()
  call void @__quantum__qis__h__body(%Qubit* %q0)
  call void @__quantum__qis__cx__body(%Qubit* %q0, %Qubit* %q1)
  %r0 = call %Result* @__quantum__qis__m__body(%Qubit* %q0)
  %r1 = call %Result* @__quantum__qis__m__body(%Qubit* %q1)
  ret void
}
```

**QIR Naming Convention:**
- Gates: `@__quantum__qis__<gate>__body`
- Adjoint: `@__quantum__qis__<gate>__adj`
- Controlled: `@__quantum__qis__<gate>__ctl`
- Runtime: `@__quantum__rt__<operation>`

**QIR Types:**
- `%Qubit*` — opaque qubit pointer
- `%Result*` — opaque measurement result pointer

### Stage 3: Azure Quantum Service

QIR is submitted to Azure Quantum, which acts as a broker:

1. **Workspace** — top-level container for quantum resources
2. **Provider** — hardware vendor (IonQ, Quantinuum, Rigetti)
3. **Target** — specific device or simulator (e.g., `ionq.qpu.forte-1`)

Azure Quantum translates QIR into hardware-native instruction format using provider-specific plugins.

### Stage 4: Hardware Execution

The provider receives the translated job and executes on quantum hardware. Results (measurement bitstrings) are returned through Azure Quantum.

## Hardware Provider APIs (via Azure Quantum)

### Azure Quantum REST API

```
Base URL: https://<region>.quantum.azure.com

POST /subscriptions/{subId}/resourceGroups/{rg}/providers/Microsoft.Quantum/Workspaces/{ws}/jobs
GET  /subscriptions/{subId}/resourceGroups/{rg}/providers/Microsoft.Quantum/Workspaces/{ws}/jobs/{jobId}
GET  /subscriptions/{subId}/resourceGroups/{rg}/providers/Microsoft.Quantum/Workspaces/{ws}/providers
```

**Job Submission Payload:**
```json
{
  "name": "BellPairJob",
  "target": "ionq.qpu",
  "input_data_uri": "https://<blob_storage>/bellpair.qir",
  "input_params": {
    "shots": 1000
  }
}
```

**Authentication:** Azure Active Directory (OAuth2 Bearer tokens).

### Supported Targets (via Azure Quantum)

| Provider | Target IDs | Hardware Type |
|---|---|---|
| IonQ | `ionq.qpu`, `ionq.simulator` | Trapped ion |
| Quantinuum | `quantinuum.qpu.h1-1`, `quantinuum.sim.h1-1e` | Trapped ion |
| Rigetti | `rigetti.qpu.aspen-m-3`, `rigetti.sim.qvm` | Superconducting |

### IonQ Native Format (via Azure Quantum Translation)

Azure converts QIR to IonQ's JSON circuit format:
```json
{
  "qubits": 2,
  "circuit": [
    {"gate": "h", "target": 0},
    {"gate": "cnot", "control": 0, "target": 1}
  ]
}
```

### Rigetti Native Format (via Azure Quantum Translation)

Azure converts QIR to Quil:
```
H 0
CNOT 0 1
MEASURE 0 ro[0]
MEASURE 1 ro[1]
```

## Key Differentiators

- **Language-first approach**: Q# is a dedicated quantum language (not a Python DSL)
- **LLVM-based IR**: QIR leverages decades of LLVM tooling and optimization
- **Cloud-mediated hardware access**: All hardware access goes through Azure Quantum
- **Resource estimation**: First-class support for estimating quantum resources without simulation
- **QIR Alliance**: Cross-industry standard — QIR is not Microsoft-only; adopted by the QIR Alliance

## References

- [Microsoft QDK GitHub](https://github.com/microsoft/qdk)
- [QIR Specification](https://github.com/qir-alliance/qir-spec)
- [QIR Alliance](https://qir-alliance.org/)
- [Azure Quantum Documentation](https://learn.microsoft.com/en-us/azure/quantum/)
- [Azure Quantum REST API](https://learn.microsoft.com/en-us/rest/api/azurequantum/)
- [Q# Language Reference](https://learn.microsoft.com/en-us/azure/quantum/user-guide/)
