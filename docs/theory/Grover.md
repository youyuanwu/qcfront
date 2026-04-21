# Grover's Algorithm

A beginner-friendly guide to Grover's quantum search — what it does,
why it's faster than classical search, and how it works step by step.
For gate definitions used below, see [Gates.md](Gates.md).

## The Core Question: How Many Queries Does It Take?

Suppose you have a black-box function $f(x)$ that answers "yes" or "no"
for each input. You want to find an $x$ where $f(x) = 1$. The only way
to learn anything about $f$ is to **query** it — feed in an $x$ and
read the answer. Every query costs time (and on real hardware, money).

**The goal is to minimize the number of queries.**

Classically, if there are $N$ possible inputs and only one correct
answer, you have no choice but to try inputs one at a time. On average
you need $N/2$ queries, and in the worst case all $N$.

Grover's algorithm solves this with only $O(\sqrt{N})$ queries by
exploiting quantum superposition — querying the function on all inputs
*simultaneously* and then amplifying the answer that comes back "yes."

| Inputs ($N$) | Classical queries | Grover queries |
|:---:|:---:|:---:|
| 100 | ~50 | ~8 |
| 10,000 | ~5,000 | ~79 |
| 1,000,000 | ~500,000 | ~785 |

This is a **quadratic speedup**. Not exponential like Shor's factoring
algorithm, but it applies to *any* problem that can be phrased as
"search for an input satisfying some condition" — database lookup,
constraint satisfaction (SAT), optimization, cryptographic key search,
and more. That generality makes Grover one of the most broadly useful
quantum algorithms.

## The Oracle: A Quantum Yes/No Black Box

There are two layers to understand:

1. **$f(x)$** — the abstract **function** that defines the problem.
   It maps each input to 0 ("no") or 1 ("yes"). For example,
   "is $x$ a prime factor of 21?" This is pure math — it says *what*
   you're searching for.

2. **$U_f$** — the **oracle circuit** that implements $f$ on a quantum
   computer. It's the physical realization — a sequence of quantum gates
   that evaluates $f$ on a superposition of all inputs at once.

You don't need to know the internals of $f$ to understand Grover's
algorithm — it works with *any* yes/no function. That's why $f$ is
called a "black box."

The key trick is *how* $U_f$ reports its answer. Instead of writing a
classical 0 or 1 to an output register, it **flips the phase** (sign)
of solution states:

$$U_f|x\rangle = (-1)^{f(x)}|x\rangle$$

- If $f(x) = 0$ (not a solution): the state is unchanged
- If $f(x) = 1$ (a solution): the amplitude gets a minus sign

This is called a **phase oracle**. The minus sign is invisible if you
measure immediately — $|{-}\alpha|^2 = |\alpha|^2$ — but it creates a
pattern that the rest of the algorithm can exploit.

**Why a phase flip instead of a bit flip?** Because Grover's algorithm
works by interference. The negative amplitude of the solution state
interferes constructively with itself during the diffuser step (below),
causing its probability to grow. A classical 0/1 output can't produce
this interference effect.

**Building real oracle circuits**: For a simple "find value 5" search,
$U_f$ is just a few X and multi-controlled-Z gates (shown in the
walkthrough below). For harder problems like SAT, $U_f$ encodes the
entire formula as a reversible circuit — see `sat_grover.rs` for
a working example.

## Key Idea: Amplitude Amplification

Quantum states carry **amplitudes** (complex numbers whose squares give
probabilities). Grover's algorithm works by manipulating amplitudes:

1. Start with all items equally likely (uniform superposition)
2. Repeatedly **shrink** the amplitude of wrong answers and **grow**
   the amplitude of right answers
3. Measure — the right answer pops out with high probability

Each "shrink + grow" cycle is one **Grover iteration**. After
$\lfloor\frac{\pi}{4}\sqrt{N/M}\rfloor$ iterations (where $M$ is the
number of solutions), the success probability is nearly 100%.

## The Circuit

```
     ┌───┐ ┌────────┐ ┌─────────┐       ┌────────┐ ┌─────────┐ ┌───┐
|0⟩──┤ H ├─┤        ├─┤         ├─ ... ─┤        ├─┤         ├─┤ M ├
     └───┘ │        │ │         │       │        │ │         │ └───┘
     ┌───┐ │ Oracle │ │ Diffuser│       │ Oracle │ │ Diffuser│ ┌───┐
|0⟩──┤ H ├─┤  U_f   ├─┤  U_s    ├─ ... ─┤  U_f   ├─┤  U_s    ├─┤ M ├
     └───┘ │        │ │         │       │        │ │         │ └───┘
     ┌───┐ │        │ │         │       │        │ │         │ ┌───┐
|0⟩──┤ H ├─┤        ├─┤         ├─ ... ─┤        ├─┤  U_s    ├─┤ M ├
     └───┘ └────────┘ └─────────┘       └────────┘ └─────────┘ └───┘
           ├────── one iteration ──────┤
                    × k iterations
```

Three stages:
1. **Initialization** — Hadamard on every qubit → uniform superposition
2. **Grover iterations** — repeat (Oracle + Diffuser) $k$ times
3. **Measurement** — read out the answer

## Step-by-Step Walkthrough (3 qubits, target = 5)

### Step 1: Superposition

Apply $H^{\otimes n}$ to $|000\rangle$:

$$|s\rangle = \frac{1}{\sqrt{8}} \sum_{x=0}^{7} |x\rangle$$

Every state has amplitude $\frac{1}{\sqrt{8}} \approx 0.354$.
Probability of measuring any specific state is $\frac{1}{8} = 12.5\%$.

### Step 2: Oracle ($U_f$) — "Mark" the Winner

The oracle flips the *sign* (phase) of the solution:

$$U_f|x\rangle = (-1)^{f(x)}|x\rangle$$

For target $|5\rangle = |101\rangle$:

| State | Before oracle | After oracle |
|-------|:---:|:---:|
| $\|0\rangle$ through $\|4\rangle$ | $+\frac{1}{\sqrt{8}}$ | $+\frac{1}{\sqrt{8}}$ |
| $\|5\rangle$ | $+\frac{1}{\sqrt{8}}$ | $\mathbf{-\frac{1}{\sqrt{8}}}$ |
| $\|6\rangle$, $\|7\rangle$ | $+\frac{1}{\sqrt{8}}$ | $+\frac{1}{\sqrt{8}}$ |

**No measurement probabilities change yet** — the minus sign is hidden
in the phase. But it sets up the diffuser to amplify the target.

#### How the oracle circuit works

To flip only $|101\rangle$:
1. Apply $X$ to qubits where the target bit is 0 (qubit 1) — this maps
   $|101\rangle \to |111\rangle$
2. Apply a multi-controlled-$Z$ gate (MCZ) — flips the phase of $|111\rangle$
3. Undo the $X$ gates

In qcfront, MCZ is built from Toffoli + CZ gates via a V-chain
decomposition (see `multi_cz.rs`).

### Step 3: Diffuser ($U_s$) — "Amplify" the Winner

The diffuser reflects every amplitude about the mean:

$$U_s = 2|s\rangle\langle s| - I$$

where $|s\rangle = \frac{1}{\sqrt{N}}\sum_x |x\rangle$ is the uniform
superposition.

**Concrete math for our example:**

After the oracle, the mean amplitude is:

$$\bar{a} = \frac{7 \times \frac{1}{\sqrt{8}} + 1 \times (-\frac{1}{\sqrt{8}})}{8} = \frac{6}{8\sqrt{8}} = \frac{3}{4\sqrt{8}}$$

The diffuser maps each amplitude $a_x \to 2\bar{a} - a_x$:

- Non-targets: $2\bar{a} - \frac{1}{\sqrt{8}} \approx 0.177$
- Target $|5\rangle$: $2\bar{a} + \frac{1}{\sqrt{8}} \approx 0.884$

After **1 iteration**, the target has amplitude $\approx 0.884$,
giving $P(|5\rangle) \approx 78\%$.

#### How the diffuser circuit works

$$U_s = H^{\otimes n} \cdot (2|0\rangle\langle 0| - I) \cdot H^{\otimes n}$$

Implemented as: H on all → X on all → MCZ → X on all → H on all.

This is the same MCZ gate used in the oracle, but sandwiched in
Hadamard + X gates to reflect about $|0\rangle$ instead of $|1\dots 1\rangle$.

### Step 4: More Iterations

For $n = 3$, $N = 8$, $M = 1$:

$$k = \left\lfloor \frac{\pi}{4}\sqrt{\frac{8}{1}} \right\rfloor = \left\lfloor 2.22 \right\rfloor = 2$$

| After | Target amplitude | $P$(target) |
|-------|:---:|:---:|
| Init | 0.354 | 12.5% |
| 1 iteration | 0.884 | 78.1% |
| 2 iterations | 0.973 | 94.5% |

Our implementation uses $k = 2$ for 3 qubits, achieving >94% success.

### Step 5: Measure

Measure all qubits → read out the binary string → decode as integer.
With 94.5% probability, you get 5 ($= 101_2$).

## The Geometry

Grover's algorithm has an elegant geometric interpretation in a
2D plane spanned by:

- $|w\rangle$ = the target state (or uniform superposition of all targets)
- $|w^\perp\rangle$ = the uniform superposition of non-targets

The initial state $|s\rangle$ makes angle $\theta/2$ with $|w^\perp\rangle$
where $\sin(\theta/2) = \sqrt{M/N}$.

Each Grover iteration rotates the state by $\theta$ toward $|w\rangle$:

```
        |w⟩
         ↑
         |   /  ← after 2 iterations (angle 5θ/2)
         |  /
         | /  ← after 1 iteration (angle 3θ/2)
         |/
    ─────┼───── |w⊥⟩
   θ/2 ↗
  |s⟩ = initial state
```

After $k$ iterations, the angle is $(2k+1)\theta/2$. The algorithm works
best when this angle is close to $\pi/2$ (pointing at $|w\rangle$).

**Overshooting**: If you apply too many iterations, the state rotates
*past* $|w\rangle$ and success probability drops. This is why the optimal
iteration count matters — Grover is not "more iterations = better."

## Multiple Solutions

When there are $M > 1$ solutions, the optimal iteration count drops:

$$k = \left\lfloor \frac{\pi}{4}\sqrt{\frac{N}{M}} \right\rfloor$$

| $n$ | $N$ | $M$ | $k$ | Note |
|-----|-----|-----|-----|------|
| 3 | 8 | 1 | 2 | Standard case |
| 3 | 8 | 2 | 1 | Fewer iterations needed |
| 3 | 8 | 4 | 1 | Even fewer |
| 3 | 8 | 7 | 0 | Nearly all states are solutions — no iteration needed |

When $M > N/2$, random measurement is already likely to succeed,
so $k = 0$ (no Grover iterations at all).

## Oracle Complexity

The oracle is the expensive part. Grover's speedup is measured in
**oracle queries**, not total gates. A single oracle query for our
simple "find value $x$" costs just $O(n)$ gates (X gates + one MCZ).
But for real problems (SAT, optimization), the oracle encodes problem
logic and can be much more complex.

**SAT oracle example**: Given a CNF formula, the oracle computes the
formula on a superposition of all assignments. If the formula is
satisfiable, Grover finds a satisfying assignment in $\sqrt{N/M}$
oracle calls instead of exhaustive search. See `sat_grover.rs` for
a working example.

## What Grover Cannot Do

- **Unknown $M$**: If you don't know how many solutions exist, you can't
  compute the optimal iteration count. Workaround: quantum counting
  (QPE on the Grover operator) to estimate $M$ first, or randomized
  approaches with $O(\sqrt{N})$ expected queries.

- **Unstructured search only**: Problems with structure (sorting, graph
  search) often have better classical algorithms. Grover shines when
  the only access to the problem is a black-box oracle.

- **No exponential speedup**: The $\sqrt{N}$ speedup is provably optimal
  for unstructured search (BBBV theorem). No quantum algorithm can do
  better with black-box oracle access.

## qcfront Implementation

See [GroverSearch.md](../features/GroverSearch.md) for implementation
details, API reference, and the Oracle trait design.

## Further Reading

- Grover, L. K. (1996). "A fast quantum mechanical algorithm for database
  search." *Proceedings of STOC '96*, pp. 212–219.
- Nielsen & Chuang, *Quantum Computation and Quantum Information*,
  Chapter 6: "Quantum search algorithms."
- Boyer et al. (1998). "Tight bounds on quantum searching."
  *Fortschritte der Physik*, 46(4-5), pp. 493–505.
