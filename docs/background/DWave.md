# D-Wave Ocean SDK & Quantum Annealing

> Repository: <https://github.com/dwavesystems/dwave-ocean-sdk>

## Overview

D-Wave is fundamentally different from the other reference projects (QDK, Qiskit, Cirq, Quil/QVM). While those frameworks use the **gate-based** quantum computing model — building circuits from quantum logic gates — D-Wave uses **quantum annealing**, a physics-based approach that finds low-energy solutions to optimization problems. There are no quantum gates, no circuits, no transpilers. Instead, users formulate problems as energy functions and the hardware physically evolves toward the minimum-energy state.

## Gate-Based vs. Quantum Annealing

| Aspect | Gate-Based (Qiskit, Cirq, etc.) | Quantum Annealing (D-Wave) |
|---|---|---|
| **Computation model** | Quantum circuit (gates + measurements) | Energy minimization |
| **Input** | Quantum circuit (sequence of gates) | Optimization problem (QUBO / Ising) |
| **Compilation** | Gate decomposition, qubit routing | Problem embedding onto hardware graph |
| **Output** | Measurement bitstrings | Low-energy solution samples |
| **Use cases** | General quantum algorithms, simulation | Combinatorial optimization, sampling |
| **Hardware** | Superconducting (IBM, Google), trapped ion (IonQ) | Superconducting annealing qubits (D-Wave) |

## Architecture & Key Components

```
┌──────────────────────────────────────────────────────────┐
│          User Code (Python)                               │
│     Problem formulation as BQM / CQM / DQM               │
├──────────────────────────────────────────────────────────┤
│                   dimod                                   │
│        BinaryQuadraticModel, ConstrainedQuadraticModel    │
├──────────────────────────────────────────────────────────┤
│                dwave-system                               │
│     DWaveSampler, EmbeddingComposite, LeapHybridSampler   │
├──────────────────────────────────────────────────────────┤
│              dwave-cloud-client                           │
│     SAPI REST calls, authentication, job polling          │
├──────────────────────────────────────────────────────────┤
│                D-Wave Leap Cloud                          │
│           (SAPI — Solver API)                             │
├──────────────┬───────────────────────────────────────────┤
│   QPU        │   Hybrid Solvers                          │
│ (Advantage)  │ (classical + quantum)                     │
├──────────────┴───────────────────────────────────────────┤
│            Quantum Annealing Hardware                     │
│         5000+ qubits, Pegasus topology                    │
└──────────────────────────────────────────────────────────┘
```

### Ocean SDK Package Ecosystem

The Ocean SDK is a **metapackage** that bundles several independent repositories:

| Package | Repository | Purpose |
|---|---|---|
| **dimod** | [dwavesystems/dimod](https://github.com/dwavesystems/dimod) | Core model abstractions — BQM, CQM, DQM, samplers interface |
| **dwave-system** | [dwavesystems/dwave-system](https://github.com/dwavesystems/dwave-system) | High-level sampler interfaces, embedding composites |
| **dwave-cloud-client** | [dwavesystems/dwave-cloud-client](https://github.com/dwavesystems/dwave-cloud-client) | REST API client for D-Wave Leap cloud |
| **dwave-networkx** | [dwavesystems/dwave-networkx](https://github.com/dwavesystems/dwave-networkx) | Graph algorithms (MaxCut, coloring) mapped to quantum |
| **dwave-preprocessing** | [dwavesystems/dwave-preprocessing](https://github.com/dwavesystems/dwave-preprocessing) | Problem simplification and fixing variables |
| **minorminer** | [dwavesystems/minorminer](https://github.com/dwavesystems/minorminer) | Minor embedding algorithms |
| **dwave-neal** | [dwavesystems/dwave-neal](https://github.com/dwavesystems/dwave-neal) | Simulated annealing sampler (classical) |

## Problem Formulation (Instead of Circuits)

In the gate-based world, you build circuits. In D-Wave, you build **energy functions**:

### Binary Quadratic Model (BQM) — QUBO / Ising

The fundamental model. Variables are binary (0/1 for QUBO, -1/+1 for Ising).

**QUBO form:**
```
E(x) = Σᵢ aᵢxᵢ + Σᵢ<ⱼ bᵢⱼxᵢxⱼ     where xᵢ ∈ {0, 1}
```

**Ising form:**
```
E(s) = Σᵢ hᵢsᵢ + Σᵢ<ⱼ Jᵢⱼsᵢsⱼ     where sᵢ ∈ {-1, +1}
```

```python
import dimod

# QUBO formulation
Q = {(0, 0): -1, (1, 1): -1, (0, 1): 2}
bqm = dimod.BinaryQuadraticModel.from_qubo(Q)

# Ising formulation
h = {0: -1, 1: 1}         # linear biases
J = {(0, 1): 0.5}         # quadratic couplings
bqm = dimod.BinaryQuadraticModel.from_ising(h, J)
```

### Constrained Quadratic Model (CQM)

Modern extension supporting constraints and integer variables — used with hybrid solvers:

```python
import dimod

cqm = dimod.ConstrainedQuadraticModel()
x = [dimod.Binary(f'x_{i}') for i in range(5)]

# Objective: minimize sum
cqm.set_objective(sum(x))

# Constraint: at least 2 selected
cqm.add_constraint(sum(x) >= 2, label='min_selection')
```

### Discrete Quadratic Model (DQM)

Variables can take discrete (non-binary) values:

```python
import dimod

dqm = dimod.DiscreteQuadraticModel()
dqm.add_variable(3, label='color_node_0')  # 3 possible values
dqm.add_variable(3, label='color_node_1')
```

## "Compilation" Pipeline: Problem → Hardware

D-Wave doesn't have a transpiler in the gate-model sense. Instead, it has an **embedding** pipeline:

### Stage 1: Problem Graph

The user's BQM defines a **logical problem graph** where:
- Nodes = variables
- Edges = interactions (non-zero couplings)

```
Example: 4-variable problem with all-to-all connectivity
    0 --- 1
    |  X  |
    3 --- 2
```

### Stage 2: Minor Embedding

The QPU has a **fixed hardware topology** (Chimera or Pegasus) where not all qubits are connected. The logical problem graph must be **embedded** into the hardware graph.

**Minor embedding** maps each logical variable to a **chain** of connected physical qubits that collectively represent one variable:

```
Logical variable 0  →  Physical qubits {Q5, Q13}  (chain)
Logical variable 1  →  Physical qubits {Q6}       (single qubit)
Logical variable 2  →  Physical qubits {Q14, Q22} (chain)
Logical variable 3  →  Physical qubits {Q7}       (single qubit)
```

Qubits within a chain are coupled with a strong **chain strength** to keep them aligned.

```python
from dwave.system import DWaveSampler, EmbeddingComposite

# EmbeddingComposite automatically finds a minor embedding
sampler = EmbeddingComposite(DWaveSampler())
result = sampler.sample(bqm, num_reads=100)
```

### Hardware Topologies

| Topology | System | Qubits | Connectivity | Max Connections/Qubit |
|---|---|---|---|---|
| **Chimera** | D-Wave 2000Q | ~2048 | Bipartite K₄,₄ unit cells in grid | 6 |
| **Pegasus** | D-Wave Advantage | ~5000 | Denser cross-connected | 15 |
| **Zephyr** | D-Wave Advantage2 | ~7000+ | Even denser | 20 |

Higher connectivity = shorter chains = better solution quality.

### Stage 3: Parameter Setting

Before annealing, the system configures:
- **Biases** (hᵢ): magnetic field on each qubit
- **Couplings** (Jᵢⱼ): interaction strength between connected qubits
- **Chain strengths**: coupling within chains
- **Annealing schedule**: time profile of quantum → classical transition
- **num_reads**: how many annealing cycles to run

### Stage 4: Quantum Annealing

The physical process:
1. System starts in a **superposition** of all possible states (high transverse field)
2. Transverse field is slowly reduced while the problem Hamiltonian is turned on
3. System settles into a **low-energy state** of the problem Hamiltonian
4. Qubits are measured → one sample

This is repeated `num_reads` times to collect multiple samples.

### Stage 5: Result Decoding

Physical qubit measurements are mapped back to logical variables:
- Chain qubits are decoded (majority vote or other strategy)
- Results returned as `SampleSet` with energies and timing info

```python
sampleset = sampler.sample(bqm, num_reads=1000)
print(sampleset.first)          # Best sample
print(sampleset.first.energy)   # Lowest energy found
for sample, energy in sampleset.data(['sample', 'energy']):
    print(sample, energy)
```

## D-Wave Cloud API (SAPI)

### SAPI REST API

```
Base URL: https://cloud.dwavesys.com/sapi/v2

GET  /solvers/              — List available solvers (QPUs, hybrid)
POST /problems/             — Submit a problem
GET  /problems/{id}/        — Get problem status and results

Authentication: X-Auth-Token: <SAPI_TOKEN>
```

### Problem Submission Payload

```json
{
  "solver": "Advantage_system6.4",
  "type": "ising",
  "data": {
    "linear": {"0": -1.0, "1": 0.5},
    "quadratic": {"0,1": 1.0}
  },
  "params": {
    "num_reads": 100,
    "annealing_time": 20,
    "chain_strength": 2.0
  }
}
```

### Solver Types

| Solver | Type | Description |
|---|---|---|
| `Advantage_system6.4` | QPU | Pure quantum annealing on Pegasus hardware |
| `hybrid_binary_quadratic_model_version2` | Hybrid | Classical-quantum hybrid for BQMs |
| `hybrid_constrained_quadratic_model_version1` | Hybrid | Hybrid solver for CQMs |
| `hybrid_discrete_quadratic_model_version1` | Hybrid | Hybrid solver for DQMs |

### Authentication

```bash
# Configure via CLI
dwave config create

# Or environment variable
export DWAVE_API_TOKEN="ABC-1234567890..."

# Or in dwave.conf file
[defaults]
token = ABC-1234567890...
endpoint = https://cloud.dwavesys.com/sapi/
```

OAuth2 authentication also supported (SDK 6.6+).

## Complete Example: Max-Cut Problem

```python
import dimod
import dwave.inspector
from dwave.system import DWaveSampler, EmbeddingComposite

# Define a graph (Max-Cut problem)
# Maximize the number of edges between two groups
J = {(0,1): 1, (0,2): 1, (1,2): 1, (1,3): 1, (2,3): 1}
h = {}
bqm = dimod.BinaryQuadraticModel.from_ising(h, J)

# Submit to D-Wave QPU (with automatic embedding)
sampler = EmbeddingComposite(DWaveSampler())
sampleset = sampler.sample(bqm, num_reads=100, chain_strength=2.0)

# Results
print("Best solution:", sampleset.first.sample)
print("Energy:", sampleset.first.energy)

# Visualize embedding and results
dwave.inspector.show(sampleset)
```

## Comparison with Gate-Based Pipeline

```
GATE-BASED (Qiskit/Cirq/Q#/Quil):
  Python circuit → Transpile (decompose gates, map qubits) → QASM/QIR/Quil → Hardware API → Gate execution → Measure

D-WAVE:
  Python BQM/CQM → Embed (map variables to qubit chains) → SAPI JSON → Anneal → Sample → Decode
```

| Concept (Gate-Based) | D-Wave Equivalent |
|---|---|
| Quantum circuit | BQM / CQM / DQM |
| Quantum gates | Biases and couplings (hᵢ, Jᵢⱼ) |
| Transpilation | Minor embedding |
| Basis gates | Native topology connectivity |
| Qubit routing (SWAPs) | Chain formation |
| Circuit depth | Annealing time |
| Shots / repetitions | num_reads |
| OpenQASM / QIR / Quil | SAPI JSON (Ising/QUBO data) |

## Key Differentiators

- **Optimization-native**: Designed for combinatorial optimization, not general quantum algorithms
- **No gates or circuits**: Completely different computational paradigm
- **Thousands of qubits**: D-Wave Advantage has 5000+ qubits (vs. ~100 for gate-based machines), but less versatile
- **Embedding challenge**: Problem-to-hardware mapping (minor embedding) is the key compilation step
- **Hybrid solvers**: Combine classical and quantum resources for larger problems than the QPU alone can handle
- **Fast time-to-solution**: Annealing takes microseconds per sample; embedding is the bottleneck

## References

- [D-Wave Ocean SDK](https://github.com/dwavesystems/dwave-ocean-sdk)
- [Ocean Documentation](https://docs.ocean.dwavesys.com/)
- [D-Wave System Documentation](https://docs.dwavequantum.com/)
- [dimod GitHub](https://github.com/dwavesystems/dimod)
- [dwave-system GitHub](https://github.com/dwavesystems/dwave-system)
- [minorminer GitHub](https://github.com/dwavesystems/minorminer)
- [SAPI REST API Reference](https://docs.dwavequantum.com/en/latest/leap_sapi/sapi_rest.html)
- [D-Wave Leap Cloud](https://cloud.dwavesys.com/leap/)
