# Quantum Gates

A reference for the quantum gates used in qcfront — what they do
mathematically and when to use them. For how gates map to physical
hardware operations (microwave pulses, lasers, etc.), see
[GatePhysics.md](GatePhysics.md).

All gates are available in roqoqo as `roqoqo::operations::*`.

## Qubit Basics

A qubit has two basis states: |0⟩ and |1⟩. A general qubit state is
α|0⟩ + β|1⟩ where α and β are complex numbers with |α|² + |β|² = 1.
|α|² is the probability of measuring 0, |β|² of measuring 1.

The **Bloch sphere** visualizes a qubit as a point on a unit sphere:
- |0⟩ is the north pole
- |1⟩ is the south pole
- |+⟩ = (|0⟩+|1⟩)/√2 is on the equator

Gates are rotations and reflections on this sphere.

### Bloch Sphere

Open [bloch.html](bloch.html) for an interactive 3D visualization.
Drag to rotate, scroll to zoom, apply gates to see how they move the state vector.

- **Z axis**: |0⟩ (north) ↔ |1⟩ (south). Rz rotates around this axis.
- **X axis**: |+⟩ ↔ |−⟩. Rx rotates around this axis.
- **Y axis**: |+i⟩ ↔ |−i⟩. Ry rotates around this axis.
- **Hadamard**: reflects through the XZ plane (swaps Z and X axes).

## Single-Qubit Gates

### Pauli Gates

**PauliX** (X gate, NOT gate, bit-flip)

$$X = \begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix} \qquad X\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \beta \\ \alpha \end{pmatrix}$$

Flips the qubit: $X|0\rangle = |1\rangle$, $X|1\rangle = |0\rangle$.
Like a classical NOT gate. On the Bloch sphere: 180° rotation around X.

```rust
circuit += PauliX::new(0); // flip qubit 0
```

**PauliY** (Y gate)

$$Y = \begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix} \qquad Y\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} -i\beta \\ i\alpha \end{pmatrix}$$

Flips the qubit and adds a phase: $Y|0\rangle = i|1\rangle$, $Y|1\rangle = -i|0\rangle$.
180° rotation around Y axis. Rarely used directly; appears in decompositions.

**PauliZ** (Z gate, phase-flip)

$$Z = \begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix} \qquad Z\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha \\ -\beta \end{pmatrix}$$

Doesn't change measurement probabilities — only flips the phase of |1⟩.
$Z|0\rangle = |0\rangle$, $Z|1\rangle = -|1\rangle$. 180° rotation around Z.

Key insight: Z does nothing to basis states individually (they're
eigenstates). It matters in superposition:
$Z|+\rangle = Z\frac{|0\rangle + |1\rangle}{\sqrt{2}} = \frac{|0\rangle - |1\rangle}{\sqrt{2}} = |-\rangle$.

### Hadamard Gate

**Hadamard** (H gate)

$$H = \frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix} \qquad H\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \frac{1}{\sqrt{2}}\begin{pmatrix} \alpha + \beta \\ \alpha - \beta \end{pmatrix}$$

Creates superposition from a basis state:
$H|0\rangle = \frac{|0\rangle + |1\rangle}{\sqrt{2}} = |+\rangle$, $\quad H|1\rangle = \frac{|0\rangle - |1\rangle}{\sqrt{2}} = |-\rangle$.

The most important gate in quantum computing — almost every algorithm
starts with H on all qubits. $H \cdot H = I$ (self-inverse).

```rust
// Create equal superposition over all 3-qubit states
for q in 0..3 {
    circuit += Hadamard::new(q);
}
```

Used in: Grover (initialization), Shor/QPE (counting register),
Deutsch-Jozsa, Bernstein-Vazirani, teleportation.

### Rotation Gates

**RotateX(θ)**, **RotateY(θ)**, **RotateZ(θ)**

Rotate the qubit by angle θ around the X, Y, or Z axis of the Bloch
sphere. These are the continuous-parameter gates — any single-qubit
gate can be decomposed into a sequence of rotations.

$$R_x(\theta) = \begin{pmatrix} \cos\frac{\theta}{2} & -i\sin\frac{\theta}{2} \\ -i\sin\frac{\theta}{2} & \cos\frac{\theta}{2} \end{pmatrix} \qquad R_x(\theta)\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha\cos\frac{\theta}{2} - i\beta\sin\frac{\theta}{2} \\ -i\alpha\sin\frac{\theta}{2} + \beta\cos\frac{\theta}{2} \end{pmatrix}$$

$$R_y(\theta) = \begin{pmatrix} \cos\frac{\theta}{2} & -\sin\frac{\theta}{2} \\ \sin\frac{\theta}{2} & \cos\frac{\theta}{2} \end{pmatrix} \qquad R_y(\theta)\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha\cos\frac{\theta}{2} - \beta\sin\frac{\theta}{2} \\ \alpha\sin\frac{\theta}{2} + \beta\cos\frac{\theta}{2} \end{pmatrix}$$

$$R_z(\theta) = \begin{pmatrix} e^{-i\theta/2} & 0 \\ 0 & e^{i\theta/2} \end{pmatrix} \qquad R_z(\theta)\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha \, e^{-i\theta/2} \\ \beta \, e^{i\theta/2} \end{pmatrix}$$

$R_y$ changes amplitudes (measurement probabilities). $R_z$ changes only
phases (no effect on probabilities). This is why Möttönen state preparation
uses $R_y$ for amplitude trees and $R_z$ for phase trees.

#### How rotations move states on the Bloch sphere

Try these in [bloch.html](bloch.html): start from |0⟩ and apply gates.

**Key intuition**:
- **Ry(θ)** changes how much |0⟩ vs |1⟩ — it moves the state between
  the poles. This controls **measurement probabilities**.
  Ry(π/2) takes |0⟩ → |+⟩ → |1⟩ → |−⟩ → |0⟩.
- **Rz(θ)** changes the phase between |0⟩ and |1⟩ without changing
  probabilities. It spins the state around the equator.
  Rz(π/2) takes |+⟩ → |+i⟩ → |−⟩ → |−i⟩ → |+⟩.
- **Rx(θ)** is like Ry but in the Z-Y plane.

This is why Möttönen state preparation uses Ry for amplitudes (setting
probabilities) and Rz for phases (setting interference patterns).

#### Common rotation angles

| Angle | $R_y(\theta)$ effect | $R_z(\theta)$ effect |
|-------|-------------|-------------|
| $0$ | Identity | Identity |
| $\pi/4$ | Slight tilt toward equator | 45° phase rotation |
| $\pi/2$ | $|0\rangle \to |+\rangle$ (equal superposition) | $|+\rangle \to |{+i}\rangle$ |
| $\pi$ | $|0\rangle \to |1\rangle$ (full flip) | $|+\rangle \to |-\rangle$ (like Z) |
| $2\pi$ | Back to start (with $-1$ phase) | Back to start (with $-1$ phase) |

Note: $\theta$ is the rotation angle, but the state-vector coefficients use
$\theta/2$ (half-angle). $R_y(\pi/2)$ produces $\cos(\pi/4)|0\rangle + \sin(\pi/4)|1\rangle = |+\rangle$.

Special cases:
- $R_x(\pi) = -iX$ (same as X up to global phase)
- $R_y(\pi) = -iY$
- $R_z(\pi) = -iZ$

```rust
use qoqo_calculator::CalculatorFloat;
// Rotate qubit 0 by π/4 around Y axis: tilt toward equator
circuit += RotateY::new(0, CalculatorFloat::Float(std::f64::consts::PI / 4.0));

// Rotate qubit 1 by π/2 around Z axis: add 90° relative phase
circuit += RotateZ::new(1, CalculatorFloat::Float(std::f64::consts::FRAC_PI_2));
```

Used in: State preparation (Möttönen decomposition uses Ry and Rz trees).

### Phase Gates

**SGate** (√Z, phase gate)

$$S = \begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix} \qquad S\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha \\ i\beta \end{pmatrix}$$

Adds a $\pi/2$ phase to |1⟩. $S^2 = Z$.

**TGate** (π/8 gate)

$$T = \begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix} \qquad T\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha \\ e^{i\pi/4}\beta \end{pmatrix}$$

Adds a $\pi/4$ phase to |1⟩. $T^2 = S$. Important for universal gate sets
and fault-tolerant quantum computing (T gate is the expensive one
to implement with error correction).

**PhaseShiftGate1(θ)**

$$P(\theta) = \begin{pmatrix} 1 & 0 \\ 0 & e^{i\theta} \end{pmatrix} \qquad P(\theta)\begin{pmatrix} \alpha \\ \beta \end{pmatrix} = \begin{pmatrix} \alpha \\ e^{i\theta}\beta \end{pmatrix}$$

General phase shift. $S = P(\pi/2)$, $T = P(\pi/4)$, $Z = P(\pi)$.

## Two-Qubit Gates

### CNOT (Controlled-NOT, CX)

$$\text{CNOT} = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \end{pmatrix} \qquad \text{CNOT}\begin{pmatrix} a_{00} \\ a_{01} \\ a_{10} \\ a_{11} \end{pmatrix} = \begin{pmatrix} a_{00} \\ a_{01} \\ a_{11} \\ a_{10} \end{pmatrix}$$

Flips the target qubit if the control qubit is $|1\rangle$:
$\text{CNOT}|c,t\rangle = |c, t \oplus c\rangle$. The fundamental entangling gate.

```rust
circuit += CNOT::new(0, 1); // control=0, target=1
```

**Bell state creation**: $H(0)$ then $\text{CNOT}(0,1)$ creates $(|00\rangle+|11\rangle)/\sqrt{2}$.
This is the "hello world" of entanglement.

Used in: Everything — Bell states, teleportation, Grover diffusion,
QPE controlled unitaries, state preparation (Möttönen decomposition),
error correction.

### ControlledPauliZ (CZ)

$$\text{CZ} = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 0 & 0 & -1 \end{pmatrix} \qquad \text{CZ}\begin{pmatrix} a_{00} \\ a_{01} \\ a_{10} \\ a_{11} \end{pmatrix} = \begin{pmatrix} a_{00} \\ a_{01} \\ a_{10} \\ -a_{11} \end{pmatrix}$$

Phase-flips only $|11\rangle$. Symmetric: $\text{CZ}(a,b) = \text{CZ}(b,a)$.
$\text{CZ} = (I \otimes H) \cdot \text{CNOT} \cdot (I \otimes H)$.

```rust
circuit += ControlledPauliZ::new(0, 1);
```

Used in: Grover phase oracle (multi-CZ marks solutions).

### SWAP

$$\text{SWAP} = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \end{pmatrix} \qquad \text{SWAP}\begin{pmatrix} a_{00} \\ a_{01} \\ a_{10} \\ a_{11} \end{pmatrix} = \begin{pmatrix} a_{00} \\ a_{10} \\ a_{01} \\ a_{11} \end{pmatrix}$$

Exchanges two qubit states: $\text{SWAP}|a,b\rangle = |b,a\rangle$.
Built from 3 CNOTs: $\text{SWAP} = \text{CNOT}(a,b) \cdot \text{CNOT}(b,a) \cdot \text{CNOT}(a,b)$.

Used in: Shor (controlled modular multiplication swaps work qubits).

## Three-Qubit Gates

### Toffoli (CCX, CCNOT)

$$\text{Toffoli}|c_1, c_2, t\rangle = |c_1, c_2, t \oplus (c_1 \wedge c_2)\rangle$$

Flips the target only when both controls are $|1\rangle$. Classical AND gate
made reversible. Universal for classical computation.

```rust
// roqoqo convention: first arg is TARGET
circuit += Toffoli::new(2, 0, 1); // target=2, ctrl1=0, ctrl2=1
```

⚠️ **roqoqo convention**: `Toffoli::new(target, control_1, control_2)` —
the first argument is the **target**, not a control. This differs from
some other frameworks.

Used in: SAT oracle (clause evaluation), modular arithmetic.

### ControlledControlledPauliZ (CCZ)

$$\text{CCZ}|a,b,c\rangle = (-1)^{a \wedge b \wedge c}|a,b,c\rangle$$

Phase-flips only $|111\rangle$. Symmetric in all three qubits.
$\text{CCZ} = (I \otimes I \otimes H) \cdot \text{Toffoli} \cdot (I \otimes I \otimes H)$.

Used in: Multi-target Grover oracle (marks multiple solutions).

## Composite Operations

### QFT (Quantum Fourier Transform)

Not a single gate but a built-in roqoqo operation that emits the full
QFT circuit (Hadamards + controlled phase rotations + swaps).

```rust
let qubits = vec![0, 1, 2, 3];
circuit += QFT::new(qubits, true, true); // inverse=true, swap=true
```

Used in: Shor/QPE (inverse QFT extracts phase from counting register).

### MeasureQubit

Collapses a qubit to |0⟩ or |1⟩ and records the classical result.
Irreversible — destroys superposition.

```rust
circuit += MeasureQubit::new(0, "result".to_string(), 0);
//                          qubit  register_name     bit_index
```

### PragmaGetStateVector

QuEST-specific: extracts the full state vector without measurement.
Deterministic (no collapse). Used for verification/debugging.

```rust
circuit += DefinitionComplex::new("sv".to_string(), dim, true);
circuit += PragmaGetStateVector::new("sv".to_string(), None);
```

## Ancilla Qubits

An **ancilla** (Latin: "servant") is a helper qubit added to a circuit
that doesn't hold input or output data. Ancillas serve as scratch space
for computations that can't be done directly on the data qubits.

### Why ancillas are needed

Quantum gates are reversible — every gate has an inverse. But many
useful operations (like AND, OR, multi-controlled gates) are
irreversible classically. To make them reversible for a quantum circuit,
we store intermediate results in ancilla qubits and **uncompute** them
afterward.

Example: a 5-qubit controlled-Z can't be built from 1- and 2-qubit
gates alone. The V-chain decomposition uses 3 ancilla qubits as a
"Toffoli ladder" to cascade the AND of controls:

```
controls: ─●──●─────────────────●──●─
           │  │                 │  │
           ├──┤                 ├──┤
controls: ─●──┼──●──────────●──┼──●─
              │  │          │  │
ancilla₀: ───⊕──●──────────●──⊕─── (back to |0⟩)
              │  │          │  │
ancilla₁: ──────⊕──●────●──⊕────── (back to |0⟩)
                    │    │
ancilla₂: ─────────⊕─CZ─⊕──────── (back to |0⟩)
                      │
target:   ────────────●──────────── (phase flipped)
           forward    ↑   reverse
           pass      gate  pass
```

### The golden rule: clean up after yourself

Ancillas must be returned to |0⟩ after use. If an ancilla is left
entangled with the data qubits, subsequent operations (like the
diffuser in Grover's algorithm) will act on the wrong subspace,
silently producing wrong results.

The standard pattern is **compute → use → uncompute**:
1. **Compute**: fill ancillas with intermediate results
2. **Use**: apply the target operation (e.g., phase flip)
3. **Uncompute**: run the compute steps in reverse to erase ancillas

Since every quantum gate is its own inverse or has a known inverse,
uncomputation is always possible — just replay the gates backward.

### Common uses in qcfront

| Where | Ancilla purpose | Count |
|-------|----------------|:---:|
| `build_multi_cz` | Toffoli V-chain for n-qubit CZ | max(0, n−2) |
| `build_multi_cx` | Toffoli V-chain for n-qubit CNOT | max(0, n−2) |
| `CnfOracle` | Clause evaluation + MCZ/MCX scratch | c + max(mcx, mcz) |
| Grover diffuser | Shares MCZ pattern with oracle | max(0, n−2) |

### Ancilla vs workspace vs scratch

These terms are used interchangeably in the literature. In qcfront:
- **ancilla** = any non-data qubit
- The Oracle trait's `num_ancillas()` reports total scratch needed
- The driver allocates ancillas and passes them via `apply()`

## Gate Universality

Any quantum computation can be built from:
- $\{H, T, \text{CNOT}\}$ — universal gate set (discrete)
- $\{R_y(\theta), R_z(\theta), \text{CNOT}\}$ — universal gate set (continuous)

qcfront's state preparation uses $\{R_y, R_z, \text{CNOT}\}$. The algorithms
use $\{H, X, Z, \text{CNOT}, \text{CZ}, \text{Toffoli}\}$ for clarity.

## Quick Reference

| Gate | Qubits | roqoqo | Effect |
|------|--------|--------|--------|
| X | 1 | `PauliX` | Bit flip |
| Y | 1 | `PauliY` | Bit + phase flip |
| Z | 1 | `PauliZ` | Phase flip |
| H | 1 | `Hadamard` | Create superposition |
| S | 1 | `SGate` | π/2 phase |
| T | 1 | `TGate` | π/4 phase |
| Ry(θ) | 1 | `RotateY` | Y-axis rotation |
| Rz(θ) | 1 | `RotateZ` | Z-axis rotation |
| CNOT | 2 | `CNOT` | Controlled NOT |
| CZ | 2 | `ControlledPauliZ` | Controlled phase |
| SWAP | 2 | `SWAP` | Exchange qubits |
| Toffoli | 3 | `Toffoli` | Doubly-controlled NOT |
| CCZ | 3 | `ControlledControlledPauliZ` | Doubly-controlled phase |
