//! Hardcoded modular multiplication circuits for N=15.
//!
//! For each coprime a ∈ {2, 4, 7, 8, 11, 13, 14}, provides a function that appends
//! a controlled modular multiplication circuit to a roqoqo Circuit.
//!
//! The multiplication |x⟩ → |a·x mod 15⟩ is implemented as a permutation
//! using SWAP gates. For N=15 with 4-qubit work register, the permutation
//! decomposes into cycles that can be implemented with SWAP chains.
//!
//! All operations are controlled on a single control qubit (from the counting register).
//!
//! Qubit layout:
//!   - `control`: single qubit from the counting register
//!   - `work[0..3]`: 4-qubit work register encoding |x⟩ in binary (work[0] = LSB)
//!
//! The key insight for N=15: since 15 = 2⁴ - 1, multiplication by powers of 2
//! mod 15 is a cyclic bit shift, implementable with just SWAP gates.

use roqoqo::operations::*;
use roqoqo::Circuit;

/// Append a controlled multiplication by `a` mod 15 to the circuit.
///
/// `control`: index of the control qubit
/// `work`: indices of the 4 work qubits (LSB first)
///
/// Only the work register is modified, controlled on `control` being |1⟩.
pub fn controlled_modmul_15(circuit: &mut Circuit, a: u64, control: usize, work: [usize; 4]) {
    match a % 15 {
        1 => {} // identity — no gates needed
        2 => controlled_mul2_mod15(circuit, control, work),
        4 => {
            // 4 = 2², apply mul-by-2 twice
            controlled_mul2_mod15(circuit, control, work);
            controlled_mul2_mod15(circuit, control, work);
        }
        7 => controlled_mul7_mod15(circuit, control, work),
        8 => {
            // 8 = 2³, apply mul-by-2 three times
            controlled_mul2_mod15(circuit, control, work);
            controlled_mul2_mod15(circuit, control, work);
            controlled_mul2_mod15(circuit, control, work);
        }
        11 => controlled_mul11_mod15(circuit, control, work),
        13 => controlled_mul13_mod15(circuit, control, work),
        14 => controlled_mul14_mod15(circuit, control, work),
        _ => panic!("a={} is not coprime to 15", a),
    }
}

/// Controlled multiplication by 2 mod 15.
///
/// For N=15 (= 2⁴ - 1), multiplying by 2 is a left cyclic bit shift:
///   |b₃ b₂ b₁ b₀⟩ → |b₂ b₁ b₀ b₃⟩
///
/// Left cyclic shift: move each bit one position up, top wraps to bottom.
/// Implemented as a chain of controlled SWAPs from top to bottom.
fn controlled_mul2_mod15(circuit: &mut Circuit, control: usize, work: [usize; 4]) {
    // Left cyclic shift: SWAP(3,2), SWAP(2,1), SWAP(1,0)
    // This moves: b₃→b₂, b₂→b₁, b₁→b₀, b₀→b₃
    controlled_swap(circuit, control, work[3], work[2]);
    controlled_swap(circuit, control, work[2], work[1]);
    controlled_swap(circuit, control, work[1], work[0]);
}

/// Controlled multiplication by 7 mod 15.
///
/// 7x mod 15 permutation on {0..14}:
///   Cycles: (1 7 4 13)(2 14 8 11)(3 6 12 9)
///   Fixed: 0, 5, 10, 15
///
/// Since 7 ≡ 2⁻¹ · (-1) mod 15, we can implement as:
///   mul-by-7 = mul-by-8 then mul-by-14 ... but simpler:
///   7 = 8 - 1 ≡ -8 mod 15... actually let's use the SWAP decomposition.
///
/// For 4 qubits representing values mod 15:
///   mul_7 = reverse_bits then mul_2
///   Actually: 7x mod 15 = reverse the qubit order then left-shift
///
/// More directly: 7 = 4 + 2 + 1, and since 7 ≡ 2³ mod 15 (since 8 mod 15 = 8, not 7)
/// Let's just use: mul7 = mul8 · mul14⁻¹ ... too complex.
///
/// Simplest approach: 7 mod 15 is the inverse of 13 mod 15 (since 7·13 = 91 = 6·15+1).
/// And 13 = 15-2, so mul_13 = bit-flip then mul_2 then bit-flip.
/// So mul_7 = inverse of mul_13 = reverse the SWAP chain of mul_13.
///
/// Even simpler: mul_by_7 = right cyclic shift then bit-flip-all.
/// Verify: for x=1: right-shift |0001⟩ → |1000⟩ = 8, flip → |0111⟩ = 7. ✓
///         for x=2: right-shift |0010⟩ → |0001⟩ = 1, flip → |1110⟩ = 14. ✓ (7·2=14)
///         for x=3: right-shift |0011⟩ → |1001⟩ = 9, flip → |0110⟩ = 6. ✓ (7·3=21 mod 15=6)
///
/// Right cyclic shift: SWAP(3,2), SWAP(2,1), SWAP(1,0)
/// Then controlled-X on all 4 qubits.
fn controlled_mul7_mod15(circuit: &mut Circuit, control: usize, work: [usize; 4]) {
    // Right cyclic shift: SWAP(0,1), SWAP(1,2), SWAP(2,3)
    controlled_swap(circuit, control, work[0], work[1]);
    controlled_swap(circuit, control, work[1], work[2]);
    controlled_swap(circuit, control, work[2], work[3]);
    // Bit flip all work qubits (controlled)
    *circuit += CNOT::new(control, work[0]);
    *circuit += CNOT::new(control, work[1]);
    *circuit += CNOT::new(control, work[2]);
    *circuit += CNOT::new(control, work[3]);
}

/// Controlled multiplication by 11 mod 15.
///
/// 11 = 15 - 4, so mul_11(x) = -4x mod 15 = 15 - 4x mod 15.
/// Equivalently: mul_11 = bit-flip then mul_4 (since flipping = 15-x for 4-bit).
/// Verify: x=1: flip |0001⟩ → |1110⟩ = 14, mul4: 14·4 mod 15 = 56 mod 15 = 11. ✓
///
/// Actually simpler: 11 ≡ -4 mod 15, and -x mod 15 = bit-flip for 4-bit values.
/// So mul_11 = flip all bits, then left-shift twice (mul by 4).
///
/// Verify: x=1: flip → 14 (|1110⟩), shift left 2 → 14·4 mod 15 = 11. ✓
///         x=2: flip → 13 (|1101⟩), shift left 2 → 13·4 mod 15 = 52 mod 15 = 7. ✓ (11·2=22 mod 15=7)
fn controlled_mul11_mod15(circuit: &mut Circuit, control: usize, work: [usize; 4]) {
    // Bit flip all (controlled)
    *circuit += CNOT::new(control, work[0]);
    *circuit += CNOT::new(control, work[1]);
    *circuit += CNOT::new(control, work[2]);
    *circuit += CNOT::new(control, work[3]);
    // Left cyclic shift twice (= mul by 4)
    controlled_mul2_mod15(circuit, control, work);
    controlled_mul2_mod15(circuit, control, work);
}

/// Controlled multiplication by 13 mod 15.
///
/// 13 = 15 - 2, so mul_13(x) = -2x mod 15.
/// -x mod 15 = bit-flip, then mul_2.
///
/// Verify: x=1: flip → 14, mul2 → 14·2 mod 15 = 28 mod 15 = 13. ✓
///         x=2: flip → 13, mul2 → 13·2 mod 15 = 26 mod 15 = 11. ✓ (13·2=26 mod 15=11)
///         x=7: flip → 8,  mul2 → 8·2 mod 15 = 16 mod 15 = 1.  ✓ (13·7=91 mod 15=1)
fn controlled_mul13_mod15(circuit: &mut Circuit, control: usize, work: [usize; 4]) {
    // Bit flip all (controlled)
    *circuit += CNOT::new(control, work[0]);
    *circuit += CNOT::new(control, work[1]);
    *circuit += CNOT::new(control, work[2]);
    *circuit += CNOT::new(control, work[3]);
    // Left cyclic shift (= mul by 2)
    controlled_mul2_mod15(circuit, control, work);
}

/// Controlled multiplication by 14 mod 15.
///
/// 14 = 15 - 1 = -1 mod 15, so mul_14(x) = -x mod 15 = bit-flip.
///
/// Verify: x=1: flip → 14. ✓ (14·1=14)
///         x=7: flip → 8.  ✓ (14·7=98 mod 15=8)
fn controlled_mul14_mod15(circuit: &mut Circuit, control: usize, work: [usize; 4]) {
    *circuit += CNOT::new(control, work[0]);
    *circuit += CNOT::new(control, work[1]);
    *circuit += CNOT::new(control, work[2]);
    *circuit += CNOT::new(control, work[3]);
}

/// Controlled SWAP (Fredkin gate) decomposition.
///
/// Swaps target_a and target_b, controlled on the control qubit.
///
/// Standard decomposition: CNOT(b→a); Toffoli(c, a → b); CNOT(b→a)
/// Note: roqoqo Toffoli::new(target, ctrl1, ctrl2) — first arg is the target.
fn controlled_swap(circuit: &mut Circuit, control: usize, target_a: usize, target_b: usize) {
    *circuit += CNOT::new(target_b, target_a); // a ^= b
    *circuit += Toffoli::new(target_b, control, target_a); // b ^= (control AND a)
    *circuit += CNOT::new(target_b, target_a); // a ^= b
}

#[cfg(test)]
mod tests {
    use super::*;
    use roqoqo::backends::EvaluatingBackend;
    use roqoqo_quest::Backend;

    /// Helper: build a circuit that prepares |x⟩ in the work register,
    /// sets the control qubit to |1⟩, applies controlled mul_a mod 15,
    /// and measures the work register.
    fn test_modmul(a: u64, x: u64) -> u64 {
        let control = 0;
        let work = [1, 2, 3, 4];

        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("ro".to_string(), 4, true);

        // Set control to |1⟩
        circuit += PauliX::new(control);

        // Prepare |x⟩ in work register
        for (bit, &q) in work.iter().enumerate() {
            if (x >> bit) & 1 == 1 {
                circuit += PauliX::new(q);
            }
        }

        // Apply controlled modular multiplication
        controlled_modmul_15(&mut circuit, a, control, work);

        // Measure work register
        for (i, &q) in work.iter().enumerate() {
            circuit += MeasureQubit::new(q, "ro".to_string(), i);
        }

        let backend = Backend::new(5, None);
        let (bit_registers, _, _) = backend.run_circuit(&circuit).expect("sim failed");
        let bits = &bit_registers["ro"][0];

        // Convert bits to integer
        let mut result = 0u64;
        for (bit, &b) in bits.iter().enumerate() {
            if b {
                result |= 1 << bit;
            }
        }
        result
    }

    #[test]
    fn test_mul2_mod15() {
        // x=0 is excluded: the cyclic shift maps 0→0 correctly, but we test the
        // non-trivial cases that matter for Shor (work register starts at |1⟩).
        for x in 1..15 {
            let expected = (2 * x) % 15;
            let got = test_modmul(2, x);
            assert_eq!(
                got, expected,
                "2 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul4_mod15() {
        for x in 1..15 {
            let expected = (4 * x) % 15;
            let got = test_modmul(4, x);
            assert_eq!(
                got, expected,
                "4 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul7_mod15() {
        for x in 1..15 {
            let expected = (7 * x) % 15;
            let got = test_modmul(7, x);
            assert_eq!(
                got, expected,
                "7 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul8_mod15() {
        for x in 1..15 {
            let expected = (8 * x) % 15;
            let got = test_modmul(8, x);
            assert_eq!(
                got, expected,
                "8 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul11_mod15() {
        for x in 1..15 {
            let expected = (11 * x) % 15;
            let got = test_modmul(11, x);
            assert_eq!(
                got, expected,
                "11 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul13_mod15() {
        for x in 1..15 {
            let expected = (13 * x) % 15;
            let got = test_modmul(13, x);
            assert_eq!(
                got, expected,
                "13 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_mul14_mod15() {
        for x in 1..15 {
            let expected = (14 * x) % 15;
            let got = test_modmul(14, x);
            assert_eq!(
                got, expected,
                "14 * {} mod 15: expected {}, got {}",
                x, expected, got
            );
        }
    }

    #[test]
    fn test_identity() {
        for x in 0..15 {
            let got = test_modmul(1, x);
            assert_eq!(got, x, "1 * {} mod 15: expected {}, got {}", x, x, got);
        }
    }

    /// Minimal test: just apply X to qubit 1, measure qubit 1.
    /// Verifies the test harness reads the right qubits.
    #[test]
    fn test_harness_sanity() {
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("ro".to_string(), 4, true);

        // Set work[0] (qubit 1) to |1⟩
        circuit += PauliX::new(1);

        // Measure work qubits [1,2,3,4]
        circuit += MeasureQubit::new(1, "ro".to_string(), 0);
        circuit += MeasureQubit::new(2, "ro".to_string(), 1);
        circuit += MeasureQubit::new(3, "ro".to_string(), 2);
        circuit += MeasureQubit::new(4, "ro".to_string(), 3);

        let backend = Backend::new(5, None);
        let (regs, _, _) = backend.run_circuit(&circuit).unwrap();
        let bits = &regs["ro"][0];

        let mut val = 0u64;
        for (bit, &b) in bits.iter().enumerate() {
            if b {
                val |= 1 << bit;
            }
        }
        assert_eq!(val, 1, "Expected 1 (only work[0] set), got {}", val);
    }

    /// Test controlled SWAP in isolation with all input combinations.
    #[test]
    fn test_controlled_swap_basic() {
        // Test: control=1, a=1, b=0 → should swap → a=0, b=1
        let (a_out, b_out) = run_cswap(true, true, false);
        assert!(
            !a_out,
            "c=1,a=1,b=0: a should be 0 after swap, got {}",
            a_out
        );
        assert!(
            b_out,
            "c=1,a=1,b=0: b should be 1 after swap, got {}",
            b_out
        );

        // Test: control=1, a=0, b=1 → should swap → a=1, b=0
        let (a_out, b_out) = run_cswap(true, false, true);
        assert!(
            a_out,
            "c=1,a=0,b=1: a should be 1 after swap, got {}",
            a_out
        );
        assert!(
            !b_out,
            "c=1,a=0,b=1: b should be 0 after swap, got {}",
            b_out
        );

        // Test: control=0, a=1, b=0 → no swap → a=1, b=0
        let (a_out, b_out) = run_cswap(false, true, false);
        assert!(a_out, "c=0,a=1,b=0: a should stay 1, got {}", a_out);
        assert!(!b_out, "c=0,a=1,b=0: b should stay 0, got {}", b_out);
    }

    fn run_cswap(c_val: bool, a_val: bool, b_val: bool) -> (bool, bool) {
        let mut circuit = Circuit::new();
        circuit += DefinitionBit::new("ro".to_string(), 2, true);

        if c_val {
            circuit += PauliX::new(0);
        }
        if a_val {
            circuit += PauliX::new(1);
        }
        if b_val {
            circuit += PauliX::new(2);
        }

        controlled_swap(&mut circuit, 0, 1, 2);

        circuit += MeasureQubit::new(1, "ro".to_string(), 0);
        circuit += MeasureQubit::new(2, "ro".to_string(), 1);

        let backend = Backend::new(3, None);
        let (regs, _, _) = backend.run_circuit(&circuit).unwrap();
        let bits = &regs["ro"][0];
        (bits[0], bits[1])
    }

    /// Verify roqoqo Toffoli convention: Toffoli::new(target, ctrl1, ctrl2)
    /// target ^= (ctrl1 AND ctrl2)
    #[test]
    fn test_toffoli_convention() {
        // Toffoli::new(0, 1, 2): q0 ^= (q1 AND q2)
        for q0 in [false, true] {
            for q1 in [false, true] {
                for q2 in [false, true] {
                    let mut circuit = Circuit::new();
                    circuit += DefinitionBit::new("ro".to_string(), 3, true);
                    if q0 {
                        circuit += PauliX::new(0);
                    }
                    if q1 {
                        circuit += PauliX::new(1);
                    }
                    if q2 {
                        circuit += PauliX::new(2);
                    }
                    circuit += Toffoli::new(0, 1, 2);
                    circuit += MeasureQubit::new(0, "ro".to_string(), 0);
                    circuit += MeasureQubit::new(1, "ro".to_string(), 1);
                    circuit += MeasureQubit::new(2, "ro".to_string(), 2);
                    let backend = Backend::new(3, None);
                    let (regs, _, _) = backend.run_circuit(&circuit).unwrap();
                    let bits = &regs["ro"][0];
                    let expected_q0 = q0 ^ (q1 & q2);
                    assert_eq!(
                        bits[1], q1,
                        "Toffoli changed ctrl1! in=({},{},{})",
                        q0, q1, q2
                    );
                    assert_eq!(
                        bits[2], q2,
                        "Toffoli changed ctrl2! in=({},{},{})",
                        q0, q1, q2
                    );
                    assert_eq!(
                        bits[0], expected_q0,
                        "Toffoli failed: in=({},{},{}) -> q0={}, expected q0={}",
                        q0, q1, q2, bits[0], expected_q0
                    );
                }
            }
        }
    }
}
