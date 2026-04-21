//! State preparation: circuit synthesis from target quantum state.
//!
//! Given a target state |ψ⟩ = Σ αᵢ|i⟩, produces a circuit that transforms
//! |0...0⟩ into |ψ⟩ using Möttönen decomposition (arXiv:quant-ph/0407010).
//!
//! # Qubit Convention
//!
//! This module uses **LSB-first** ordering consistent with the rest of qcfront:
//! amplitude index j = Σ b_q · 2^q, where b_q is the state of qubit q.
//! For example, index 5 = 101₂ means qubit 0 = 1, qubit 1 = 0, qubit 2 = 1.

use std::f64::consts::PI;

use num_complex::Complex64;
use qoqo_calculator::CalculatorFloat;
use roqoqo::operations::*;
use roqoqo::Circuit;

/// Normalization tolerance for floating-point amplitude validation.
/// Amplitudes are accepted if |Σ|αᵢ|² − 1| < NORM_TOLERANCE.
pub const NORM_TOLERANCE: f64 = 1e-10;

// Threshold below which a rotation angle is considered zero and skipped.
const ANGLE_EPSILON: f64 = 1e-12;

/// A validated quantum state vector.
///
/// Invariants enforced at construction:
/// - Length is 2^n for some n ≥ 1
/// - All amplitudes are finite (no NaN or Inf)
/// - Amplitudes are normalized: |Σ|αᵢ|² − 1| < [`NORM_TOLERANCE`]
#[derive(Debug, Clone)]
pub struct QuantumState {
    repr: StateRepr,
}

#[derive(Debug, Clone)]
enum StateRepr {
    Dense(Vec<Complex64>),
    Sparse {
        num_qubits: usize,
        amps: Vec<(usize, Complex64)>,
    },
}

impl QuantumState {
    /// Create from a dense amplitude vector.
    ///
    /// # Panics
    /// - If `amplitudes` is empty
    /// - If `amplitudes` length is 1 (0 qubits is not a valid quantum system)
    /// - If `amplitudes` length is not a power of 2
    /// - If any amplitude contains NaN or Inf
    /// - If amplitudes are not normalized (|Σ|αᵢ|² − 1| ≥ NORM_TOLERANCE)
    pub fn dense(amplitudes: Vec<Complex64>) -> Self {
        assert!(!amplitudes.is_empty(), "amplitude vector must not be empty");
        assert!(
            amplitudes.len() >= 2,
            "need at least 1 qubit (2 amplitudes), got length {}",
            amplitudes.len()
        );
        assert!(
            amplitudes.len().is_power_of_two(),
            "amplitude vector length must be a power of 2, got {}",
            amplitudes.len()
        );
        assert_all_finite(&amplitudes);
        assert_normalized_dense(&amplitudes);

        Self {
            repr: StateRepr::Dense(amplitudes),
        }
    }

    /// Create from sparse (index, amplitude) pairs.
    ///
    /// Amplitudes not listed are zero.
    ///
    /// # Panics
    /// - If `num_qubits < 1`
    /// - If `amplitudes` is empty (all-zero state is not normalized)
    /// - If any index ≥ 2^num_qubits
    /// - If any index appears more than once
    /// - If any amplitude contains NaN or Inf
    /// - If amplitudes are not normalized
    pub fn sparse(num_qubits: usize, amplitudes: Vec<(usize, Complex64)>) -> Self {
        assert!(num_qubits >= 1, "num_qubits must be >= 1, got 0");
        assert!(
            !amplitudes.is_empty(),
            "sparse amplitudes must not be empty (all-zero state is invalid)"
        );

        let dim = 1usize << num_qubits;
        let mut seen = std::collections::HashSet::new();
        for &(idx, amp) in &amplitudes {
            assert!(
                idx < dim,
                "index {} out of range for {} qubits (max {})",
                idx,
                num_qubits,
                dim - 1
            );
            assert!(
                seen.insert(idx),
                "duplicate index {} in sparse amplitudes",
                idx
            );
            assert!(
                amp.re.is_finite() && amp.im.is_finite(),
                "amplitude at index {} is not finite: {:?}",
                idx,
                amp
            );
        }

        let norm_sq: f64 = amplitudes.iter().map(|(_, a)| a.norm_sqr()).sum();
        assert!(
            (norm_sq - 1.0).abs() < NORM_TOLERANCE,
            "amplitudes not normalized: sum of |α|² = {}, expected 1.0",
            norm_sq
        );

        Self {
            repr: StateRepr::Sparse {
                num_qubits,
                amps: amplitudes,
            },
        }
    }

    /// Create a uniform superposition over specific basis states.
    /// All listed states get equal amplitude 1/√k (after deduplication).
    ///
    /// # Panics
    /// - If `num_qubits < 1`
    /// - If `states` is empty
    /// - If any state ≥ 2^num_qubits
    pub fn uniform(num_qubits: usize, states: &[usize]) -> Self {
        assert!(num_qubits >= 1, "num_qubits must be >= 1, got 0");
        assert!(!states.is_empty(), "states must not be empty");

        let dim = 1usize << num_qubits;
        let mut unique: Vec<usize> = states.to_vec();
        unique.sort_unstable();
        unique.dedup();

        for &s in &unique {
            assert!(
                s < dim,
                "state {} out of range for {} qubits (max {})",
                s,
                num_qubits,
                dim - 1
            );
        }

        let amp = Complex64::new(1.0 / (unique.len() as f64).sqrt(), 0.0);
        let amps: Vec<(usize, Complex64)> = unique.into_iter().map(|s| (s, amp)).collect();

        Self {
            repr: StateRepr::Sparse { num_qubits, amps },
        }
    }

    /// Single computational basis state (e.g., |101⟩).
    ///
    /// # Panics
    /// - If `num_qubits < 1`
    /// - If `state >= 2^num_qubits`
    pub fn basis(num_qubits: usize, state: usize) -> Self {
        assert!(num_qubits >= 1, "num_qubits must be >= 1, got 0");
        let dim = 1usize << num_qubits;
        assert!(
            state < dim,
            "state {} out of range for {} qubits (max {})",
            state,
            num_qubits,
            dim - 1
        );

        Self {
            repr: StateRepr::Sparse {
                num_qubits,
                amps: vec![(state, Complex64::new(1.0, 0.0))],
            },
        }
    }

    /// Number of qubits in the state.
    pub fn num_qubits(&self) -> usize {
        match &self.repr {
            StateRepr::Dense(v) => v.len().trailing_zeros() as usize,
            StateRepr::Sparse { num_qubits, .. } => *num_qubits,
        }
    }

    /// Get the amplitude of a specific basis state.
    /// Returns zero for states with zero amplitude.
    pub fn amplitude_at(&self, index: usize) -> Complex64 {
        match &self.repr {
            StateRepr::Dense(v) => {
                if index < v.len() {
                    v[index]
                } else {
                    Complex64::new(0.0, 0.0)
                }
            }
            StateRepr::Sparse { amps, .. } => amps
                .iter()
                .find(|(i, _)| *i == index)
                .map(|(_, a)| *a)
                .unwrap_or(Complex64::new(0.0, 0.0)),
        }
    }

    /// Convert to a dense amplitude vector (always 2^n elements).
    pub fn to_dense(&self) -> Vec<Complex64> {
        match &self.repr {
            StateRepr::Dense(v) => v.clone(),
            StateRepr::Sparse { num_qubits, amps } => {
                let dim = 1usize << num_qubits;
                let mut dense = vec![Complex64::new(0.0, 0.0); dim];
                for &(idx, amp) in amps {
                    dense[idx] = amp;
                }
                dense
            }
        }
    }

    /// Iterate over nonzero amplitudes as (index, amplitude) pairs.
    pub fn iter_nonzero(&self) -> Box<dyn Iterator<Item = (usize, Complex64)> + '_> {
        match &self.repr {
            StateRepr::Dense(v) => Box::new(
                v.iter()
                    .enumerate()
                    .filter(|(_, a)| a.norm_sqr() > 0.0)
                    .map(|(i, a)| (i, *a)),
            ),
            StateRepr::Sparse { amps, .. } => Box::new(amps.iter().copied()),
        }
    }

    /// Whether the state has fewer nonzero amplitudes than the full 2^n.
    pub fn is_sparse(&self) -> bool {
        match &self.repr {
            StateRepr::Dense(v) => v.iter().any(|a| a.norm_sqr() == 0.0),
            StateRepr::Sparse { num_qubits, amps } => amps.len() < (1usize << num_qubits),
        }
    }
}

fn assert_all_finite(amplitudes: &[Complex64]) {
    for (i, a) in amplitudes.iter().enumerate() {
        assert!(
            a.re.is_finite() && a.im.is_finite(),
            "amplitude at index {} is not finite: {:?}",
            i,
            a
        );
    }
}

fn assert_normalized_dense(amplitudes: &[Complex64]) {
    let norm_sq: f64 = amplitudes.iter().map(|a| a.norm_sqr()).sum();
    assert!(
        (norm_sq - 1.0).abs() < NORM_TOLERANCE,
        "amplitudes not normalized: sum of |α|² = {}, expected 1.0",
        norm_sq
    );
}

// ---------------------------------------------------------------------------
// Möttönen decomposition
// ---------------------------------------------------------------------------

/// Synthesize a circuit that prepares the given state from |0...0⟩.
///
/// Uses Möttönen decomposition. Practical for ≤ 15 qubits; at 20 qubits
/// the circuit has ~2M gates.
pub fn prepare_state(state: &QuantumState) -> Circuit {
    let n = state.num_qubits();
    let amps = state.to_dense();

    if n == 1 {
        return prepare_1qubit(amps[0], amps[1]);
    }

    let mut circuit = Circuit::new();

    // Phase 1: Ry rotation tree (amplitudes)
    let ry_angles = compute_ry_angles(&amps, n);
    for (k, angles) in ry_angles.iter().enumerate() {
        emit_uniformly_controlled_ry(&mut circuit, k, angles);
    }

    // Phase 2: Rz rotation tree (phases)
    let rz_angles = compute_rz_angles(&amps, n);
    for (k, angles) in rz_angles.iter().enumerate() {
        emit_uniformly_controlled_rz(&mut circuit, k, angles);
    }

    circuit
}

/// Compute the fidelity |⟨ψ|φ⟩|² between two quantum states.
/// Returns 1.0 for identical states (up to global phase), 0.0 for orthogonal.
pub fn fidelity(a: &QuantumState, b: &QuantumState) -> f64 {
    assert_eq!(
        a.num_qubits(),
        b.num_qubits(),
        "fidelity requires equal qubit counts"
    );
    let da = a.to_dense();
    let db = b.to_dense();
    let inner: Complex64 = da.iter().zip(db.iter()).map(|(x, y)| x.conj() * y).sum();
    inner.norm_sqr()
}

// ---------------------------------------------------------------------------
// 1-qubit special case (correctness anchor)
// ---------------------------------------------------------------------------

fn prepare_1qubit(alpha: Complex64, beta: Complex64) -> Circuit {
    let mut circuit = Circuit::new();

    let a_mag = alpha.norm();

    // Ry(θ) where cos(θ/2) = |α|, sin(θ/2) = |β|
    let theta = 2.0 * clamp_acos(a_mag);

    if theta.abs() > ANGLE_EPSILON {
        circuit += RotateY::new(0, CalculatorFloat::Float(theta));
    }

    // Rz(φ) where φ = arg(β) − arg(α)
    let phi = beta.arg() - alpha.arg();
    // Normalize to [-π, π]
    let phi = normalize_angle(phi);

    if phi.abs() > ANGLE_EPSILON {
        circuit += RotateZ::new(0, CalculatorFloat::Float(phi));
    }

    // Global phase: e^{i·arg(α)} — not physically observable, omit
    circuit
}

// ---------------------------------------------------------------------------
// Möttönen angle computation
// ---------------------------------------------------------------------------

/// Compute Ry angles for each level of the Möttönen tree.
///
/// Level k has 2^k angles. The angle at position c (control pattern)
/// rotates qubit k conditioned on qubits 0..k-1 being in state c.
///
/// LSB-first: amplitude index j has qubit q at bit position q.
/// At level k, we group amplitudes by the state of qubits 0..k-1 (the
/// k least significant bits). Within each group, qubit k selects between
/// "left" (qubit k = 0) and "right" (qubit k = 1) subsets.
fn compute_ry_angles(amps: &[Complex64], n: usize) -> Vec<Vec<f64>> {
    let mut angles = Vec::with_capacity(n);

    for k in 0..n {
        let num_controls = 1usize << k; // 2^k control patterns
        let mut level_angles = Vec::with_capacity(num_controls);

        for c in 0..num_controls {
            let mut left_norm_sq = 0.0f64;
            let mut right_norm_sq = 0.0f64;

            for (j, amp) in amps.iter().enumerate() {
                if k > 0 && (j & ((1 << k) - 1)) != c {
                    continue;
                }
                let bit_k = (j >> k) & 1;
                let norm_sq = amp.norm_sqr();
                if bit_k == 0 {
                    left_norm_sq += norm_sq;
                } else {
                    right_norm_sq += norm_sq;
                }
            }

            let parent_norm = (left_norm_sq + right_norm_sq).sqrt();
            let left_norm = left_norm_sq.sqrt();

            if parent_norm < ANGLE_EPSILON {
                // Zero subtree — no rotation needed
                level_angles.push(0.0);
            } else {
                let cos_half = left_norm / parent_norm;
                let theta = 2.0 * clamp_acos(cos_half);
                level_angles.push(theta);
            }
        }

        angles.push(level_angles);
    }

    angles
}

/// Compute Rz angles for each level of the Möttönen tree.
///
/// The Rz tree sets relative phases. At each level k, for each control
/// pattern c, we compute the phase difference between the "left" and "right"
/// subtrees (qubit k = 0 vs qubit k = 1).
fn compute_rz_angles(amps: &[Complex64], n: usize) -> Vec<Vec<f64>> {
    let mut angles = Vec::with_capacity(n);

    for k in 0..n {
        let num_controls = 1usize << k;
        let mut level_angles = Vec::with_capacity(num_controls);

        for c in 0..num_controls {
            // Sum amplitudes in left and right subtrees (weighted by norm)
            // to get effective phase of each subtree
            let mut left_phase = 0.0f64;
            let mut right_phase = 0.0f64;
            let mut left_weight = 0.0f64;
            let mut right_weight = 0.0f64;

            for (j, amp) in amps.iter().enumerate() {
                if k > 0 && (j & ((1 << k) - 1)) != c {
                    continue;
                }

                let bit_k = (j >> k) & 1;
                let norm = amp.norm();
                if norm < ANGLE_EPSILON {
                    continue;
                }

                if bit_k == 0 {
                    left_phase += amp.arg() * norm;
                    left_weight += norm;
                } else {
                    right_phase += amp.arg() * norm;
                    right_weight += norm;
                }
            }

            if left_weight > ANGLE_EPSILON {
                left_phase /= left_weight;
            }
            if right_weight > ANGLE_EPSILON {
                right_phase /= right_weight;
            }

            let phi = right_phase - left_phase;
            level_angles.push(normalize_angle(phi));
        }

        angles.push(level_angles);
    }

    angles
}

// ---------------------------------------------------------------------------
// Uniformly controlled rotation emission
// ---------------------------------------------------------------------------

/// Emit a uniformly controlled Ry rotation at level k.
/// Level 0: single Ry on qubit 0 (no controls).
/// Level k ≥ 1: decompose into CNOT + Ry pairs using Gray code.
fn emit_uniformly_controlled_ry(circuit: &mut Circuit, k: usize, angles: &[f64]) {
    if k == 0 {
        assert_eq!(angles.len(), 1);
        if angles[0].abs() > ANGLE_EPSILON {
            *circuit += RotateY::new(0, CalculatorFloat::Float(angles[0]));
        }
        return;
    }
    emit_multiplexed_rotation(circuit, k, angles, GateKind::Ry);
}

/// Emit a uniformly controlled Rz rotation at level k.
fn emit_uniformly_controlled_rz(circuit: &mut Circuit, k: usize, angles: &[f64]) {
    if k == 0 {
        assert_eq!(angles.len(), 1);
        if angles[0].abs() > ANGLE_EPSILON {
            *circuit += RotateZ::new(0, CalculatorFloat::Float(angles[0]));
        }
        return;
    }
    emit_multiplexed_rotation(circuit, k, angles, GateKind::Rz);
}

#[derive(Clone, Copy)]
enum GateKind {
    Ry,
    Rz,
}

/// Decompose a uniformly controlled rotation into CNOT + single-qubit gates.
///
/// For k control qubits (qubits 0..k-1) and target qubit k, with 2^k
/// target angles θ[c] (one per control pattern c):
///
/// 1. Compute decomposition angles via inverse Walsh-Hadamard-like transform
/// 2. Apply CNOT+Rotation pairs in Gray code order
///
/// Reference: Möttönen et al., Section III.A
fn emit_multiplexed_rotation(circuit: &mut Circuit, k: usize, angles: &[f64], gate: GateKind) {
    let n = angles.len();
    assert_eq!(n, 1 << k, "expected 2^k = {} angles, got {}", 1 << k, n);

    // Check if all angles are effectively zero
    if angles.iter().all(|a| a.abs() < ANGLE_EPSILON) {
        return;
    }

    // Compute decomposition angles: M^{-1} * angles
    // where M is the Gray-code-ordered Walsh-Hadamard-like matrix
    let decomp_angles = compute_decomposition_angles(angles);

    // Emit gates in Gray code order
    let target = k;
    let gray_code = gray_code_sequence(k);

    for (i, &decomp_angle) in decomp_angles.iter().enumerate() {
        // Emit rotation on target qubit
        if decomp_angle.abs() > ANGLE_EPSILON {
            match gate {
                GateKind::Ry => {
                    *circuit += RotateY::new(target, CalculatorFloat::Float(decomp_angle))
                }
                GateKind::Rz => {
                    *circuit += RotateZ::new(target, CalculatorFloat::Float(decomp_angle))
                }
            }
        }

        // Emit CNOT from the control qubit that changes in Gray code
        if i < n - 1 {
            let control_qubit = gray_code_diff_bit(gray_code[i], gray_code[i + 1]);
            *circuit += CNOT::new(control_qubit, target);
        } else {
            // Last CNOT: flip the bit that differs between last and first Gray code
            let control_qubit = gray_code_diff_bit(gray_code[n - 1], gray_code[0]);
            *circuit += CNOT::new(control_qubit, target);
        }
    }
}

/// Compute decomposition angles from target angles.
///
/// The transform is: decomp = M^{-1} * target_angles
/// where M[i][j] = (-1)^(popcount(gray(i) & j)) / 2^k
///
/// This is computed via the recursive structure of the Walsh-Hadamard transform
/// applied in Gray code order.
fn compute_decomposition_angles(angles: &[f64]) -> Vec<f64> {
    let n = angles.len();
    if n == 1 {
        return angles.to_vec();
    }

    // The relationship between target angles θ[c] and decomposition angles a[i] is:
    //   θ[c] = Σᵢ (-1)^{popcount(gray[i] & c)} · a[i]
    // Inverting: a[i] = (1/n) · Σc (-1)^{popcount(gray[i] & c)} · θ[c]
    let mut result = vec![0.0; n];
    let gray = gray_code_sequence(n.trailing_zeros() as usize);

    for i in 0..n {
        let mut sum = 0.0;
        for (c, &target_angle) in angles.iter().enumerate() {
            let sign = if (gray[i] & c).count_ones().is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            sum += sign * target_angle;
        }
        result[i] = sum / n as f64;
    }

    result
}

/// Generate the standard Gray code sequence for k bits.
/// Returns 2^k values where consecutive values differ by exactly one bit.
fn gray_code_sequence(k: usize) -> Vec<usize> {
    let n = 1usize << k;
    (0..n).map(|i| i ^ (i >> 1)).collect()
}

/// Find which bit differs between two Gray code values.
/// Returns the bit position (0-indexed = qubit index).
fn gray_code_diff_bit(a: usize, b: usize) -> usize {
    let diff = a ^ b;
    debug_assert!(
        diff.is_power_of_two(),
        "Gray code values must differ by 1 bit"
    );
    diff.trailing_zeros() as usize
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Clamp-safe arccos: clamps input to [-1, 1] before computing.
fn clamp_acos(x: f64) -> f64 {
    x.clamp(-1.0, 1.0).acos()
}

/// Normalize angle to [-π, π].
fn normalize_angle(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    }
    if a < -PI {
        a += 2.0 * PI;
    }
    a
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn c(re: f64, im: f64) -> Complex64 {
        Complex64::new(re, im)
    }

    // === QuantumState constructor tests ===

    #[test]
    fn test_dense_valid() {
        let s = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        assert_eq!(s.num_qubits(), 1);
        assert_eq!(s.amplitude_at(0), c(1.0, 0.0));
        assert_eq!(s.amplitude_at(1), c(0.0, 0.0));
    }

    #[test]
    fn test_dense_2qubit() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let s = QuantumState::dense(vec![
            c(inv_sqrt2, 0.0),
            c(0.0, 0.0),
            c(0.0, 0.0),
            c(inv_sqrt2, 0.0),
        ]);
        assert_eq!(s.num_qubits(), 2);
        // Bell state has zeros, so it IS sparse despite Dense storage
        assert!(s.is_sparse());
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn test_dense_empty() {
        QuantumState::dense(vec![]);
    }

    #[test]
    #[should_panic(expected = "at least 1 qubit")]
    fn test_dense_length_1() {
        QuantumState::dense(vec![c(1.0, 0.0)]);
    }

    #[test]
    #[should_panic(expected = "power of 2")]
    fn test_dense_not_power_of_2() {
        let amp = 1.0 / 3.0_f64.sqrt();
        QuantumState::dense(vec![c(amp, 0.0), c(amp, 0.0), c(amp, 0.0)]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn test_dense_nan() {
        QuantumState::dense(vec![c(f64::NAN, 0.0), c(0.0, 0.0)]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn test_dense_inf() {
        QuantumState::dense(vec![c(f64::INFINITY, 0.0), c(0.0, 0.0)]);
    }

    #[test]
    #[should_panic(expected = "not normalized")]
    fn test_dense_unnormalized() {
        QuantumState::dense(vec![c(1.0, 0.0), c(1.0, 0.0)]);
    }

    #[test]
    fn test_sparse_valid() {
        let s = QuantumState::sparse(2, vec![(0, c(1.0, 0.0))]);
        assert_eq!(s.num_qubits(), 2);
        assert_eq!(s.amplitude_at(0), c(1.0, 0.0));
        assert_eq!(s.amplitude_at(1), c(0.0, 0.0));
        assert!(s.is_sparse());
    }

    #[test]
    #[should_panic(expected = "num_qubits must be >= 1")]
    fn test_sparse_zero_qubits() {
        QuantumState::sparse(0, vec![(0, c(1.0, 0.0))]);
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn test_sparse_empty() {
        QuantumState::sparse(2, vec![]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_sparse_out_of_range() {
        QuantumState::sparse(2, vec![(4, c(1.0, 0.0))]);
    }

    #[test]
    #[should_panic(expected = "duplicate index")]
    fn test_sparse_duplicate() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        QuantumState::sparse(2, vec![(0, c(inv_sqrt2, 0.0)), (0, c(inv_sqrt2, 0.0))]);
    }

    #[test]
    fn test_uniform_valid() {
        let s = QuantumState::uniform(2, &[0, 3]);
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        assert!((s.amplitude_at(0).re - inv_sqrt2).abs() < 1e-14);
        assert!((s.amplitude_at(3).re - inv_sqrt2).abs() < 1e-14);
        assert_eq!(s.amplitude_at(1), c(0.0, 0.0));
    }

    #[test]
    fn test_uniform_deduplicates() {
        let s = QuantumState::uniform(2, &[1, 1, 1]);
        assert_eq!(s.amplitude_at(1), c(1.0, 0.0)); // single unique state
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn test_uniform_empty() {
        QuantumState::uniform(2, &[]);
    }

    #[test]
    fn test_basis_valid() {
        let s = QuantumState::basis(3, 5);
        assert_eq!(s.amplitude_at(5), c(1.0, 0.0));
        assert_eq!(s.amplitude_at(0), c(0.0, 0.0));
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_basis_out_of_range() {
        QuantumState::basis(2, 4);
    }

    #[test]
    #[should_panic(expected = "num_qubits must be >= 1")]
    fn test_basis_zero_qubits() {
        QuantumState::basis(0, 0);
    }

    #[test]
    fn test_to_dense_from_sparse() {
        let s = QuantumState::basis(2, 2);
        let d = s.to_dense();
        assert_eq!(d.len(), 4);
        assert_eq!(d[2], c(1.0, 0.0));
        assert_eq!(d[0], c(0.0, 0.0));
    }

    #[test]
    fn test_iter_nonzero() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let s = QuantumState::uniform(2, &[0, 3]);
        let nz: Vec<_> = s.iter_nonzero().collect();
        assert_eq!(nz.len(), 2);
        assert_eq!(nz[0].0, 0);
        assert!((nz[0].1.re - inv_sqrt2).abs() < 1e-14);
    }

    // === 1-qubit preparation tests ===

    #[test]
    fn test_prepare_basis_0_1qubit() {
        // |0⟩ — should produce empty circuit (no rotations needed)
        let s = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        let circuit = prepare_state(&s);
        // Starting from |0⟩, no gates needed
        assert!(circuit.is_empty(), "|0⟩ should need no gates");
    }

    #[test]
    fn test_prepare_basis_1_1qubit() {
        // |1⟩ = Ry(π)|0⟩
        let s = QuantumState::dense(vec![c(0.0, 0.0), c(1.0, 0.0)]);
        let circuit = prepare_state(&s);
        assert!(!circuit.is_empty(), "|1⟩ should need gates");
    }

    #[test]
    fn test_prepare_plus_1qubit() {
        // |+⟩ = (|0⟩ + |1⟩)/√2 = Ry(π/2)|0⟩
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let s = QuantumState::dense(vec![c(inv_sqrt2, 0.0), c(inv_sqrt2, 0.0)]);
        let circuit = prepare_state(&s);
        assert!(!circuit.is_empty());
    }

    // === Fidelity tests ===

    #[test]
    fn test_fidelity_identical() {
        let s = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        assert!((fidelity(&s, &s) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_fidelity_orthogonal() {
        let s0 = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        let s1 = QuantumState::dense(vec![c(0.0, 0.0), c(1.0, 0.0)]);
        assert!(fidelity(&s0, &s1).abs() < 1e-14);
    }

    #[test]
    fn test_fidelity_global_phase() {
        // |0⟩ and e^{iπ/4}|0⟩ should have fidelity 1.0
        let s1 = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        let phase = Complex64::from_polar(1.0, PI / 4.0);
        let s2 = QuantumState::dense(vec![phase, c(0.0, 0.0)]);
        assert!((fidelity(&s1, &s2) - 1.0).abs() < 1e-14);
    }

    // === Gray code tests ===

    #[test]
    fn test_gray_code_k1() {
        assert_eq!(gray_code_sequence(1), vec![0, 1]);
    }

    #[test]
    fn test_gray_code_k2() {
        assert_eq!(gray_code_sequence(2), vec![0, 1, 3, 2]);
    }

    #[test]
    fn test_gray_code_k3() {
        let g = gray_code_sequence(3);
        assert_eq!(g, vec![0, 1, 3, 2, 6, 7, 5, 4]);
        // Verify consecutive values differ by exactly 1 bit
        for i in 0..g.len() - 1 {
            assert!((g[i] ^ g[i + 1]).is_power_of_two());
        }
    }

    // === Decomposition angle transform tests ===

    #[test]
    fn test_decomp_angles_trivial() {
        // Single angle: identity
        assert_eq!(compute_decomposition_angles(&[1.5]), vec![1.5]);
    }

    #[test]
    fn test_decomp_angles_k1_uniform() {
        // k=1: 2 angles both π/2 → decomposition should give [π/2, 0]
        // because uniform rotation = same rotation regardless of control
        let angles = vec![PI / 2.0, PI / 2.0];
        let decomp = compute_decomposition_angles(&angles);
        assert!(
            (decomp[0] - PI / 2.0).abs() < 1e-14,
            "decomp[0] = {}, expected π/2",
            decomp[0]
        );
        assert!(
            decomp[1].abs() < 1e-14,
            "decomp[1] = {}, expected 0",
            decomp[1]
        );
    }

    // === Round-trip tests (roqoqo-quest statevector) ===

    fn run_and_get_statevector(circuit: &Circuit, n: usize) -> Vec<Complex64> {
        use roqoqo::backends::EvaluatingBackend;
        use roqoqo_quest::Backend;

        let dim = 1usize << n;
        let mut c = Circuit::new();
        c += DefinitionComplex::new("sv".to_string(), dim, true);
        c += circuit.clone();
        c += PragmaGetStateVector::new("sv".to_string(), None);
        let backend = Backend::new(n, None);
        let (_, _, complex_regs) = backend.run_circuit(&c).unwrap();
        complex_regs["sv"][0].clone()
    }

    #[test]
    fn roundtrip_1qubit_basis0() {
        let target = QuantumState::dense(vec![c(1.0, 0.0), c(0.0, 0.0)]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 1);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity = {}", f);
    }

    #[test]
    fn roundtrip_1qubit_basis1() {
        let target = QuantumState::dense(vec![c(0.0, 0.0), c(1.0, 0.0)]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 1);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity = {}", f);
    }

    #[test]
    fn roundtrip_1qubit_plus() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let target = QuantumState::dense(vec![c(inv_sqrt2, 0.0), c(inv_sqrt2, 0.0)]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 1);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity = {}", f);
    }

    #[test]
    fn roundtrip_1qubit_with_phase() {
        // |ψ⟩ = (|0⟩ + i|1⟩)/√2
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let target = QuantumState::dense(vec![c(inv_sqrt2, 0.0), c(0.0, inv_sqrt2)]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 1);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity = {}", f);
    }

    #[test]
    fn roundtrip_2qubit_bell() {
        // Bell state (|00⟩ + |11⟩)/√2
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let target = QuantumState::dense(vec![
            c(inv_sqrt2, 0.0),
            c(0.0, 0.0),
            c(0.0, 0.0),
            c(inv_sqrt2, 0.0),
        ]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 2);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity for Bell state = {}", f);
    }

    #[test]
    fn roundtrip_2qubit_basis_state() {
        // |10⟩ = index 2 in LSB-first
        let target = QuantumState::basis(2, 2);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 2);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity for |10⟩ = {}", f);
    }

    #[test]
    fn roundtrip_2qubit_uniform() {
        // Equal superposition over |01⟩ and |10⟩
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let target = QuantumState::dense(vec![
            c(0.0, 0.0),
            c(inv_sqrt2, 0.0),
            c(inv_sqrt2, 0.0),
            c(0.0, 0.0),
        ]);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 2);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity for |01⟩+|10⟩ = {}", f);
    }

    #[test]
    fn roundtrip_3qubit_ghz() {
        // GHZ state (|000⟩ + |111⟩)/√2
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![c(0.0, 0.0); 8];
        amps[0] = c(inv_sqrt2, 0.0);
        amps[7] = c(inv_sqrt2, 0.0);
        let target = QuantumState::dense(amps);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 3);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.9999, "fidelity for GHZ = {}", f);
    }

    #[test]
    fn roundtrip_3qubit_w_state() {
        // W state (|001⟩ + |010⟩ + |100⟩)/√3
        let amp = 1.0 / 3.0_f64.sqrt();
        let mut amps = vec![c(0.0, 0.0); 8];
        amps[1] = c(amp, 0.0); // |001⟩ = qubit 0 is 1
        amps[2] = c(amp, 0.0); // |010⟩ = qubit 1 is 1
        amps[4] = c(amp, 0.0); // |100⟩ = qubit 2 is 1
        let target = QuantumState::dense(amps);
        let circuit = prepare_state(&target);
        let result = run_and_get_statevector(&circuit, 3);
        let result_state = QuantumState::dense(result);
        let f = fidelity(&target, &result_state);
        assert!(f > 0.999, "fidelity for W state = {}", f);
    }
}
