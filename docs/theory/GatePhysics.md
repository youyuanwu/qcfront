# Gate Physics

How abstract quantum gates map to physical operations on real hardware.
This doc connects the math in [Gates.md](Gates.md) to what actually
happens inside a quantum computer.

## The Abstraction Stack

```
Algorithm level:    circuit += Hadamard::new(0)
                         ↓
Gate level:         H = (1/√2)[[1,1],[1,-1]]    ← Gates.md lives here
                         ↓
Pulse level:        microwave pulse, 25 ns, 5.1 GHz
                         ↓
Physics level:      electromagnetic field rotates electron spin
```

Quantum software (including qcfront) works at the gate level. The
hardware provider's compiler translates gates into physical pulses.
This doc explains that bottom layer.

## How Each Hardware Realizes Gates

### Superconducting Qubits (IBM, Google, Rigetti)

**Qubit**: a tiny superconducting circuit (transmon) cooled to 15 mK.
Two lowest energy levels of an anharmonic oscillator serve as |0⟩ and |1⟩.
The energy gap corresponds to a microwave frequency, typically 4–6 GHz.

**Single-qubit gates**: Apply a calibrated microwave pulse at the qubit's
resonance frequency. The pulse parameters control which gate is performed:

| Gate parameter | Physical control |
|----------------|-----------------|
| Rotation axis (X, Y, Z) | Pulse phase (0°, 90°, or virtual-Z via frame tracking) |
| Rotation angle $\theta$ | Pulse duration × amplitude (area under the pulse envelope) |

- $R_x(\pi)$ = full-power pulse at 0° phase for ~25 ns
- $R_x(\pi/2)$ = same pulse for half the duration (or half amplitude)
- $R_z(\theta)$ = no physical pulse needed — implemented by shifting the
  phase reference frame of all subsequent pulses (virtual-Z gate, ~0 ns)

**Two-qubit gates (CNOT, CZ)**: Enabled by coupling two transmons through
a resonator bus or direct capacitive coupling:
- **Cross-resonance** (IBM): Drive qubit A at qubit B's frequency. The
  off-resonant drive creates an effective ZX interaction.
- **Tunable coupler** (Google Sycamore): Flux-tune a coupler between two
  qubits to turn the interaction on/off.
- Typical CNOT duration: 200–400 ns (much slower than single-qubit gates).

**Why CNOT is expensive**: Single-qubit gates take ~25 ns with >99.9%
fidelity. Two-qubit gates take ~300 ns with ~99–99.5% fidelity. This
is why gate counts (especially CNOT counts) matter for circuit depth.

### Trapped Ions (IonQ, Quantinuum)

**Qubit**: a single ion (e.g., Yb⁺, Ba⁺, Ca⁺) trapped in an
electromagnetic field. Two hyperfine or optical energy levels serve as
|0⟩ and |1⟩.

**Single-qubit gates**: Focused laser beams drive transitions between
the two levels (Rabi oscillation):

| Gate parameter | Physical control |
|----------------|-----------------|
| Rotation axis | Laser beam phase and polarization |
| Rotation angle $\theta$ | Pulse duration × Rabi frequency (laser intensity) |

$R_y(\pi/2)$: laser pulse of duration $t = \pi/(2\Omega)$ where $\Omega$
is the Rabi frequency (~MHz range). Typical gate time: 1–10 μs.

**Two-qubit gates (Mølmer-Sørensen / XX gate)**: Two laser beams create
a spin-dependent force on the ion chain's shared vibrational mode
(phonon bus). The ions' motion mediates entanglement:

1. Lasers excite the collective motion of two ions
2. Spin-motion coupling entangles the internal states
3. Motion returns to ground state, leaving only spin entanglement

Gate time: 50–200 μs. Fidelity: 99–99.9%.

**Key difference from superconducting**: All-to-all connectivity — any
ion can interact with any other ion (no nearest-neighbor restriction).
But gates are much slower (~1000× longer than superconducting).

### Photonic (Xanadu, PsiQuantum)

**Qubit**: a single photon. Unlike other platforms where the qubit
*persists* in a trap or circuit, a photonic qubit is **short-lived** —
it is generated on demand, flies through optical components at the speed
of light, and is destroyed upon detection. The entire computation
happens during the photon's flight (~nanoseconds).

| Property | Photonic | Other platforms |
|----------|---------|----------------|
| Qubit lifetime | ~ns (flight time) | μs to hours |
| Decoherence | None (photons don't interact with environment) | Major challenge |
| Main error source | Photon loss (absorption/scattering in waveguide) | Decoherence |
| Reusability | Destroyed on measurement | Can be re-measured (but collapses) |

**Photon source**: Single photons are generated from quantum dots,
spontaneous parametric down-conversion (SPDC) crystals, or four-wave
mixing in silicon waveguides. Generating truly single photons reliably
is itself a major engineering challenge.

Encoding varies:
- **Polarization**: horizontal |H⟩ = |0⟩, vertical |V⟩ = |1⟩
- **Path**: photon in upper waveguide = |0⟩, lower = |1⟩
- **Time-bin**: early arrival = |0⟩, late = |1⟩

**Single-qubit gates**: Optical elements manipulate the photon in flight:

| Gate | Physical element |
|------|-----------------|
| $R_z(\theta)$ | Phase shifter (voltage-controlled refractive index change) |
| $R_y(\theta)$ | Beamsplitter with tunable reflectivity |
| $H$ | 50:50 beamsplitter |
| $X$ | Waveguide crossing or polarization rotator |

**Two-qubit gates**: The hard part. Photons don't naturally interact.
Approaches:
- **Measurement-based** (KLM protocol): Entangle via post-selected
  measurements on ancilla photons. Probabilistic — requires many attempts.
- **Fusion gates**: Merge photonic resource states via Bell measurements.

Because two-qubit gates are probabilistic, many photonic quantum computers
use **measurement-based quantum computing** (MBQC) instead of the circuit
model: generate a large entangled cluster state of many photons, then
compute by measuring individual photons in chosen bases. The computation
emerges from the measurement pattern, not from applying gates in sequence.

### Neutral Atoms (QuEra, Pasqal)

**Qubit**: individual atoms (Rb, Cs) held in optical tweezers (focused
laser beams). Hyperfine ground states or Rydberg excited states encode
|0⟩ and |1⟩.

**Single-qubit gates**: Raman transitions via laser pulses (similar to
trapped ions).

**Two-qubit gates (Rydberg blockade)**: Excite one atom to a highly
excited Rydberg state. The enormous electric dipole of the Rydberg atom
shifts the energy levels of a nearby atom, preventing double excitation:
- If atom A is in Rydberg state, atom B *cannot* be excited → CZ gate
- Blockade radius: ~5–10 μm
- Gate time: ~0.5–1 μs

## Gate Fidelities by Platform (approximate, 2024)

| Platform | 1-qubit fidelity | 2-qubit fidelity | Gate time (2Q) |
|----------|:---:|:---:|:---:|
| Superconducting | 99.9% | 99–99.5% | 200–400 ns |
| Trapped ions | 99.99% | 99–99.9% | 50–200 μs |
| Neutral atoms | 99.5% | 97–99% | 0.5–1 μs |
| Photonic | 99.9% | ~90–95% (heralded) | ~ns (but probabilistic) |

These numbers determine how many gates you can apply before errors
accumulate. With 99.5% two-qubit fidelity, a 100-CNOT circuit has
expected fidelity ~0.995¹⁰⁰ ≈ 0.61 — already marginal.

## Native Gate Sets

Hardware doesn't implement arbitrary gates directly. Each platform has
a **native gate set** — the physically calibrated operations. The
compiler decomposes your circuit into these:

| Platform | Native gates | CNOT decomposition |
|----------|-------------|-------------------|
| IBM (Eagle+) | $\sqrt{X}$, $R_z$, CX | native |
| Google (Sycamore) | $\sqrt{W}$, $R_z$, $\sqrt{\text{iSWAP}}$ | 2 native gates |
| IonQ (Aria) | $R_x$, $R_y$, $R_z$, XX | 1 XX + single-qubit |
| Quantinuum (H2) | $R_z$, $R_y$, ZZ | 1 ZZ + single-qubit |
| Rigetti (Ankaa) | $R_x$, $R_z$, CZ | H·CZ·H |

**Virtual-Z optimization**: On most superconducting platforms, $R_z$ is
"free" — it's implemented by changing the software phase reference, not
by applying a physical pulse. Compilers exploit this aggressively.

## Measurement

**Standard measurement**: Project onto Z basis (|0⟩ or |1⟩).

| Platform | How measurement works | Time |
|----------|---------------------|------|
| Superconducting | Probe dispersive shift of readout resonator | ~300 ns |
| Trapped ions | Fluorescence detection — |1⟩ scatters photons, |0⟩ is dark | ~100 μs |
| Photonic | Single-photon detector (click = |1⟩, no click = |0⟩) | ~ns |
| Neutral atoms | Fluorescence imaging on CCD camera | ~1 ms |

**Measuring in other bases**: To measure in the X basis, apply H before
measuring in Z. To measure in an arbitrary basis defined by angles
$(\theta, \phi)$, apply $R_z(-\phi) \cdot R_y(-\theta)$ before measuring.
The detector doesn't rotate — the state does.

## Why This Matters for Software

1. **Gate count → circuit fidelity**: More gates = more error accumulation.
   Our Möttönen state preparation uses O(2ⁿ) CNOTs — only practical for
   small qubit counts on current hardware.

2. **Connectivity constraints**: Superconducting qubits have nearest-neighbor
   connectivity. A CNOT between distant qubits requires SWAP gates to move
   data — the transpiler adds these automatically. Trapped ions don't have
   this problem.

3. **Native gate translation**: When we export QIR for Azure Quantum, the
   hardware provider's compiler handles translation to native gates. Our
   circuit uses {H, CNOT, Ry, Rz, Toffoli} which all decompose cleanly.

4. **Virtual-Z means Rz is free**: Circuit optimizations that convert gates
   into Rz equivalents save real time on superconducting hardware.

## Beyond Qubits: Qudits

Qubits use 2 energy levels, but physical systems often have more.
A **qudit** uses d levels (qutrit for d=3, ququart for d=4). A single
qutrit carries $\log_2 3 \approx 1.58$ bits — more information per
particle, potentially fewer particles and fewer two-particle gates.

| System | Levels used | Who |
|--------|:-:|-----|
| Transmon higher levels | 3–4 | Google, Yale |
| Trapped ion Zeeman states | up to 7 | Innsbruck, Duke |
| Photon orbital angular momentum | unbounded | Vienna |

**Why qubits still dominate**: Error correction for d>2 is harder —
each additional level adds decay and leakage channels. The entire
software stack (algorithms, compilers, cloud APIs) assumes d=2.
Superconducting transmons actively *avoid* their third level by
engineering the anharmonicity so the 0↔1 transition frequency differs
from 1↔2. The marginal information gain from d=3 doesn't yet justify
rebuilding the ecosystem.

qcfront and roqoqo are qubit-only, matching the industry standard.

## Sources

- Krantz et al., "A Quantum Engineer's Guide to Superconducting Qubits",
  arXiv:1904.06560 (2019)
- Bruzewicz et al., "Trapped-ion quantum computing: Progress and challenges",
  arXiv:1904.04178 (2019)
- Saffman, "Quantum computing with neutral atoms", National Science Review (2019)
- IBM Quantum documentation: https://docs.quantum.ibm.com
- Google Quantum AI: https://quantumai.google
