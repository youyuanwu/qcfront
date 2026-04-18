# Quantum RSA Factoring: State of the Art

## Overview

RSA encryption relies on the computational difficulty of factoring large semiprimes (N = p × q). Quantum computers offer two fundamentally different approaches to this problem: **Shor's algorithm** (gate-based) and **QUBO optimization** (quantum annealing). This document surveys the current state of the art, what has actually been demonstrated, what each framework can do, and the resource gap to threatening real-world RSA.

## Two Quantum Approaches to Factoring

| Aspect | Shor's Algorithm (Gate-Based) | QUBO Factoring (Annealing) |
|---|---|---|
| **Model** | Quantum circuit — order finding via QPE | Energy minimization — encode N = p×q as QUBO |
| **Complexity** | Polynomial: O(n³) for n-bit number | Heuristic — no proven speedup |
| **Hardware** | IBM, Google, IonQ, Rigetti (gate-based QPUs) | D-Wave (quantum annealer) |
| **Framework** | Qiskit, Cirq, Q#, Quil | D-Wave Ocean SDK |
| **Largest N factored (real HW)** | 21 (= 3 × 7) | 8,219,999 (= 32,749 × 251) |

---

## Approach 1: Shor's Algorithm (Gate-Based)

### How It Works

Shor's algorithm reduces integer factoring to **order finding**: given random `a`, find the smallest `r` such that `a^r ≡ 1 (mod N)`. This is solved using **Quantum Phase Estimation (QPE)** on the modular exponentiation unitary.

```
1. Pick random a, check GCD(a, N) = 1
2. QUANTUM: Find order r of a mod N using QPE
   - Apply Hadamards to counting register
   - Apply controlled-U^(2^k) for modular exponentiation
   - Apply inverse QFT
   - Measure → gives s/r (continued fraction → r)
3. CLASSICAL: If r is even, compute GCD(a^(r/2) ± 1, N) → factors
```

### Circuit Structure

```
|0⟩ ─── H ─── ctrl-U^1 ──────────────────── QFT† ─── Measure
|0⟩ ─── H ────────────── ctrl-U^2 ───────── QFT† ─── Measure
|0⟩ ─── H ─────────────────────── ctrl-U^4 ─ QFT† ─── Measure
 ...         (counting register: 2n qubits)
|1⟩ ──── U^1 ─── U^2 ─── U^4 ───  ...
 ...     (work register: n qubits)
```

### Qubit Requirements by Problem Size

| Number Size | Counting Qubits | Work Qubits | Total Qubits (Beauregard) | Circuit Depth |
|---|---|---|---|---|
| **4-bit** (up to 15) | 8 | 4 | 12 | ~100s of gates |
| **8-bit** (up to 255) | 16 | 8 | ~19 (optimized) | ~1,000s of gates |
| **16-bit** (up to 65,535) | 32 | 16 | ~35 | ~100,000s of gates |
| **512-bit** (RSA-512) | 1024 | 512 | ~5,000 logical | ~10¹² T-gates |
| **2048-bit** (RSA-2048) | 4096 | 2048 | ~20,000 logical | ~10¹⁴ T-gates |

### Classical Simulation Requirements

Simulating n qubits requires 2ⁿ complex amplitudes × 16 bytes:

| Qubits | RAM Required | Feasible? |
|---|---|---|
| 12 (factor 15) | 64 KB | ✅ Laptop |
| 19 (factor 255) | 8 MB | ✅ Laptop |
| 35 (factor 65K) | 512 GB | ⚠️ High-end server |
| 50+ | 16+ PB | ✗ Impossible classically |

### What Has Been Demonstrated on Real Hardware

| Number | Factors | Year | Hardware | Qubits Used | Method | Citation |
|---|---|---|---|---|---|---|
| **15** | 3 × 5 | 2001 | NMR (7 qubits) | 7 | Compiled Shor's | Vandersypen et al., Nature 2001 |
| **15** | 3 × 5 | 2012 | Photonic | 4 | Compiled Shor's | Martín-López et al., Nature Photonics |
| **15** | 3 × 5 | 2019 | IBM ibmqx4 | 5 | Compiled Shor's | Phys. Rev. A 100, 062330 |
| **21** | 3 × 7 | 2012 | Photonic | 10 | Compiled Shor's | Martín-López et al. |
| **21** | 3 × 7 | 2021 | IBM 27-qubit | 8-9 | Compiled Shor's (qubit recycling) | Fedorov et al., IEEE Trans. QE |

**Key observations:**
- All demonstrations use **compiled/simplified** circuits, not the general-purpose Shor circuit
- The modular exponentiation is hard-coded for the specific N, not a general implementation
- Beyond 21, noise destroys the signal before the circuit completes
- **21 remains the record for Shor's algorithm on real gate-based hardware** (as of 2025)

### Qiskit Implementation: Factoring 15

```python
from qiskit import QuantumCircuit
from qiskit.circuit.library import QFT
from qiskit.primitives import Sampler
import numpy as np

N = 15
a = 7   # coprime to 15
n = 4   # bits for N
t = 8   # counting qubits (2n)

qc = QuantumCircuit(t + n, t)

# 1. Hadamard on counting register
qc.h(range(t))

# 2. Initialize work register to |1⟩
qc.x(t + n - 1)

# 3. Controlled modular exponentiation: a^(2^k) mod 15
def c_amod15(a, power):
    """Controlled multiplication by a^power mod 15 (compiled for a=7, N=15)"""
    U = QuantumCircuit(4)
    for _ in range(power % 4):
        U.swap(2, 3)
        U.swap(1, 2)
        U.swap(0, 1)
    U = U.to_gate()
    U.name = f"{a}^{power} mod 15"
    return U.control()

for q in range(t):
    qc.append(c_amod15(a, 2**q), [q] + [i + t for i in range(n)])

# 4. Inverse QFT on counting register
qc.append(QFT(t, inverse=True).to_gate(), range(t))

# 5. Measure counting register
qc.measure(range(t), range(t))

# 6. Execute
sampler = Sampler()
result = sampler.run(qc, shots=1024).result()
# Post-process: continued fractions → order r → GCD → factors
```

**Critical note:** The `c_amod15` function above is **hard-coded** for a=7, N=15. A general modular exponentiation circuit is far more complex and deep.

### Factoring 255 (RSA-8) on Simulator

```python
# Requires ~19 qubits → ~8 MB RAM — feasible on a laptop
# But the modular exponentiation circuit for general 8-bit numbers
# requires implementing:
#   - Controlled modular multiplication
#   - Modular addition (using QFT-based adder)
#   - Modular exponentiation by repeated squaring
# Circuit depth: thousands of gates
# Simulation time: minutes to hours depending on implementation
```

### Why Real Hardware Can't Go Beyond ~21

| Problem | Impact |
|---|---|
| **Gate errors** | ~0.1-1% error per gate; thousands of gates → result is noise |
| **Decoherence** | Qubits lose quantum state in ~100 μs; deep circuits take longer |
| **No error correction** | Current NISQ machines lack fault-tolerant error correction |
| **Circuit depth** | Modular exponentiation depth grows as O(n³); overwhelming for NISQ |
| **Connectivity** | Limited qubit connectivity → extra SWAP gates → more depth |

---

## Approach 2: QUBO Factoring (D-Wave Annealing)

### How It Works

Encode N = p × q as an optimization problem: find binary variables (bits of p and q) that minimize the cost function `C = (p × q − N)²`.

### QUBO Formulation

```
Given: N (number to factor)
Find:  p, q such that p × q = N

Step 1: Write p and q in binary
  p = Σᵢ 2ⁱ pᵢ    where pᵢ ∈ {0, 1}
  q = Σⱼ 2ʲ qⱼ    where qⱼ ∈ {0, 1}

Step 2: Cost function
  C = (p × q − N)²

Step 3: Expand p × q
  p × q = Σᵢ Σⱼ 2^(i+j) pᵢqⱼ

Step 4: Expand C, collect quadratic terms
  C = Σ linear terms + Σ quadratic terms + constant

Step 5: Reduce higher-order terms using ancilla variables
  e.g., replace pᵢqⱼqₖ with auxiliary variable z + penalty (z − qⱼqₖ)²

Result: QUBO matrix Q where E(x) = xᵀQx
```

### Variable Count by Problem Size

| Number Size | Factor Bits | Primary Variables | Carry Ancillas | Total Variables (approx) |
|---|---|---|---|---|
| **4-bit** (15) | 2 + 2 | 4 | 2-4 | ~8 |
| **8-bit** (255) | 4 + 4 | 8 | 8-16 | ~24 |
| **16-bit** (65K) | 8 + 8 | 16 | 32-64 | ~80 |
| **23-bit** (8.2M) | 15 + 8 | 23 | ~100+ | ~150+ |

### D-Wave Implementation

```python
import dimod
from dwave.system import DWaveSampler, EmbeddingComposite

def factor_qubo(N, num_p_bits, num_q_bits):
    """Construct QUBO for factoring N = p × q"""
    # Binary variables for p and q
    p_vars = [f'p{i}' for i in range(num_p_bits)]
    q_vars = [f'q{j}' for j in range(num_q_bits)]

    # Build cost function C = (p*q - N)^2
    # Expand binary multiplication and square
    Q = {}

    # ... (expansion of (Σ 2^(i+j) pᵢqⱼ - N)²)
    # Results in quadratic terms over pᵢ, qⱼ, and ancilla variables

    bqm = dimod.BinaryQuadraticModel.from_qubo(Q)
    return bqm

# Submit to D-Wave
bqm = factor_qubo(N=15, num_p_bits=2, num_q_bits=2)
sampler = EmbeddingComposite(DWaveSampler())
result = sampler.sample(bqm, num_reads=1000, chain_strength=4.0)

# Decode: read p and q bits from best sample
best = result.first.sample
p = sum(best[f'p{i}'] * 2**i for i in range(2))
q = sum(best[f'q{j}'] * 2**j for j in range(2))
print(f"{N} = {p} × {q}")
```

### What Has Been Demonstrated on D-Wave Hardware

| Number | Factors | Year | Hardware | Logical Qubits | Citation |
|---|---|---|---|---|---|
| **15** | 3 × 5 | 2018 | D-Wave 2000Q | ~8 | Multiple groups |
| **143** | 11 × 13 | 2018 | D-Wave 2000Q | ~20 | Jiang et al. |
| **376,289** | Various | 2022 | D-Wave Advantage | ~80 | Mengoni et al. |
| **8,219,999** | 32,749 × 251 | 2024 | D-Wave Advantage (5760 qubits) | ~150+ | Nature Sci. Rep. 2024 |

**Key observations:**
- D-Wave can factor much larger numbers than gate-based Shor's (8.2M vs. 21)
- But this is **not Shor's algorithm** — it's optimization-based heuristic search
- No proven polynomial speedup over classical methods
- Scaling is limited by embedding overhead and energy landscape complexity
- The 8,219,999 record used a novel modular locally-structured encoding for Pegasus topology

### Why D-Wave Can't Scale to RSA

| Problem | Impact |
|---|---|
| **Variable explosion** | n-bit factors need O(n²) auxiliary variables for carry bits |
| **Embedding overhead** | Logical-to-physical qubit ratio of 3-10× due to limited connectivity |
| **Energy landscape** | Exponentially many local minima for large problems |
| **No proven speedup** | May not be faster than classical simulated annealing |
| **Precision limits** | Coupler precision limits the size of representable coefficients |

---

## Approach 3: Hybrid Classical-Quantum (Controversial)

### Schnorr-QAOA Method (2022-2023)

A Chinese research group claimed that by combining Schnorr's lattice-based factoring with QAOA (Quantum Approximate Optimization Algorithm), they could factor RSA-2048 with only **372 physical qubits**.

**The claim:** Factor a 48-bit number on a real 10-qubit IBMQ device, and extrapolate to RSA-2048.

**Reality check (2023-2024):**
- Independent researchers (including from Google) reproduced and tested the approach
- The lattice reduction method fails beyond ~70 bits
- Schnorr's lattice approach remains exponentially hard as N grows
- The QAOA quantum speedup is negligible for this problem class
- **Consensus: the claim does not hold up** — RSA-2048 is not threatened

### Regev's Algorithm (2023)

Oded Regev proposed a new quantum factoring approach requiring **exponentially fewer qubits** than Shor's:

| | Shor's | Regev's |
|---|---|---|
| **Qubits** | O(n) | O(log n) |
| **Time complexity** | Polynomial: O(n³) | Subexponential: exp(O(√(n log n))) |
| **Practical impact** | Breaks RSA if hardware scales | Matches classical GNFS speed, but with far fewer qubits |

**Significance:** Not faster than classical methods, but shows quantum computers with far fewer qubits could participate in factoring. Does not threaten RSA in the near term.

---

## Resource Gap: Today vs. Breaking RSA

### Current Hardware (2025)

| Platform | Qubits | Gate Fidelity | Max Circuit Depth | Factoring Capability |
|---|---|---|---|---|
| IBM Heron | ~156 | ~99.5% 2-qubit | ~100-300 layers | N ≤ 21 |
| Google Willow | ~105 | ~99.7% 2-qubit | ~100 layers | N ≤ 21 |
| IonQ Forte | ~36 (#AQ) | ~99.5% 2-qubit | ~200-500 layers | N ≤ 15 (theoretical) |
| D-Wave Advantage | ~5,700 | N/A (annealing) | N/A | N ≤ 8,219,999 |

### What's Needed to Break RSA (Gidney & Ekerå, 2021)

| RSA Key Size | Logical Qubits | Physical Qubits (surface code) | T-Gates | Estimated Time |
|---|---|---|---|---|
| **RSA-512** | ~5,000 | ~200,000 | ~10¹² | Hours |
| **RSA-1024** | ~8,000 | ~500,000 | ~10¹³ | Hours |
| **RSA-2048** | ~20,000 | ~20,000,000 | ~10¹⁴ | ~8 hours |

### The Gap

```
TODAY (2025):                    NEEDED FOR RSA-2048:
~156 noisy qubits         →     ~20,000,000 physical qubits
~99.5% gate fidelity      →     ~99.9%+ (with error correction)
~300 circuit depth         →     ~10¹⁴ gates
No error correction       →     Full fault-tolerant surface code

Gap factor: ~100,000× in qubits, ~10¹¹× in circuit depth
```

---

## Framework Comparison for Factoring Research

| Framework | Shor's Support | Annealing/QUBO | Best For |
|---|---|---|---|
| **Qiskit** | ✅ Built-in `Shor` class, tutorials, simulators | ✗ | Learning/simulating Shor's, RSA-4 to RSA-8 on simulator |
| **Cirq** | ✅ Can implement, Google hardware access | ✗ | Custom circuit optimization, Google QPU experiments |
| **Q# (QDK)** | ✅ Can implement, resource estimation | ✗ | Resource estimation for large instances |
| **Quil/QVM** | ✅ Can implement | ✗ | Rigetti hardware experiments |
| **D-Wave Ocean** | ✗ | ✅ BQM/QUBO factoring | Annealing-based factoring up to ~23 bits |

### Recommendation for RSA Factoring Research

- **Start with Qiskit** — best tutorials, built-in Shor implementation, easy simulator access
- **Use Q# Resource Estimator** — to understand how many qubits/gates you'd need for larger instances
- **Try D-Wave** — for QUBO-based factoring of small numbers (different perspective)
- **Use Cirq** — if targeting Google hardware or needing fine-grained circuit control

---

## Timeline: When Could Quantum Computers Break RSA?

| Milestone | Estimated Timeline | Status |
|---|---|---|
| Factor 15 (Shor's, real HW) | 2001 | ✅ Done |
| Factor 21 (Shor's, real HW) | 2021 | ✅ Done |
| Factor 100+ (Shor's, real HW) | 2028-2032? | Not yet — needs ~1,000+ error-corrected qubits |
| Break RSA-512 | 2033-2038? | Needs ~200K physical qubits |
| Break RSA-2048 | 2035-2045? | Needs ~20M physical qubits |

**Note:** These timelines are highly speculative. NIST has already standardized post-quantum cryptography algorithms (CRYSTALS-Kyber, CRYSTALS-Dilithium) for migration away from RSA.

## References

- Vandersypen et al., "Experimental realization of Shor's quantum factoring algorithm", Nature 414, 2001
- Gidney & Ekerå, "How to factor 2048 bit RSA integers in 8 hours using 20 million noisy qubits", Quantum 5, 2021 ([arXiv:1905.09749](https://arxiv.org/abs/1905.09749))
- Regev, "An Efficient Quantum Factoring Algorithm", 2023 ([arXiv:2308.06572](https://arxiv.org/abs/2308.06572))
- "Effective prime factorization via quantum annealing by modular locally-structured embedding", Scientific Reports, 2024
- "The State of Factoring on Quantum Computers", arXiv:2410.14397, 2024
- Fedorov et al., "Experimental realization of Shor's quantum factoring algorithm using qubit recycling", IEEE Trans. QE, 2021
- Martín-López et al., "Experimental realization of Shor's quantum factoring algorithm using qubit recycling", Nature Photonics, 2012
- D-Wave factoring notebook: [github.com/dwave-examples/factoring-notebook](https://github.com/dwave-examples/factoring-notebook)
- NIST Post-Quantum Cryptography: [csrc.nist.gov/projects/post-quantum-cryptography](https://csrc.nist.gov/projects/post-quantum-cryptography)
