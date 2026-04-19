//! Classical math utilities for Shor's algorithm.
//!
//! GCD and LCM are provided by the `num-integer` crate.
//! This module adds continued fractions and helper functions.

use num_integer::Integer;

/// Compute continued fraction convergents of the rational number `numerator / denominator`.
///
/// Returns a list of (p, q) pairs where p/q are successively better
/// rational approximations. Used in Shor's algorithm to extract the
/// order r from a quantum phase estimation measurement.
///
/// The `max_terms` parameter limits the depth of the expansion.
pub fn convergents(numerator: u64, denominator: u64, max_terms: usize) -> Vec<(u64, u64)> {
    if denominator == 0 {
        return vec![];
    }

    let mut result = Vec::new();
    let mut n = numerator;
    let mut d = denominator;

    // Previous two convergents: p_{-1}/q_{-1} = 1/0, p_{-2}/q_{-2} = 0/1 (by convention)
    let (mut p_prev2, mut q_prev2): (u64, u64) = (0, 1);
    let (mut p_prev1, mut q_prev1): (u64, u64) = (1, 0);

    for _ in 0..max_terms {
        if d == 0 {
            break;
        }

        let a = n / d; // integer part (continued fraction coefficient)
        let rem = n % d;

        // Convergent recurrence: p_k = a_k * p_{k-1} + p_{k-2}
        let p = a.checked_mul(p_prev1).and_then(|v| v.checked_add(p_prev2));
        let q = a.checked_mul(q_prev1).and_then(|v| v.checked_add(q_prev2));

        match (p, q) {
            (Some(p), Some(q)) if q > 0 => {
                result.push((p, q));
                p_prev2 = p_prev1;
                q_prev2 = q_prev1;
                p_prev1 = p;
                q_prev1 = q;
            }
            _ => break, // overflow — stop
        }

        n = d;
        d = rem;
    }

    result
}

/// Pick a random integer in [2, n-1] that is coprime to n.
pub fn random_coprime(n: u64) -> u64 {
    loop {
        let a = rand::random_range(2..n);
        if a.gcd(&n) == 1 {
            return a;
        }
    }
}

/// Classical modular exponentiation: base^exp mod modulus.
/// Uses repeated squaring. Works for values that fit in u64.
pub fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result: u128 = 1;
    let m = modulus as u128;
    base %= modulus;
    let mut b = base as u128;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * b) % m;
        }
        exp /= 2;
        b = (b * b) % m;
    }
    result as u64
}

/// Find the multiplicative order of a mod n.
/// Returns the smallest r > 0 such that a^r ≡ 1 (mod n).
/// Used to verify quantum results classically.
pub fn find_order(a: u64, n: u64) -> Option<u64> {
    if a.gcd(&n) != 1 {
        return None;
    }
    let mut current = a % n;
    for r in 1..n {
        if current == 1 {
            return Some(r);
        }
        current = ((current as u128 * a as u128) % n as u128) as u64;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_integer::Integer;

    #[test]
    fn test_convergents_simple() {
        // 3/8 = 0 + 1/(2 + 1/(1 + 1/2))
        // Convergents: 0/1, 1/3, 1/2... let's verify
        let result = convergents(3, 8, 10);
        assert!(!result.is_empty());
        // The last convergent should equal the original fraction
        let (p, q) = *result.last().unwrap();
        assert_eq!(p * 8, q * 3); // p/q == 3/8
    }

    #[test]
    fn test_convergents_for_shor() {
        // Shor's algorithm for N=15, a=7: order r=4
        // QPE measures multiples of 2^n / r = 256/4 = 64
        // So measured values are 0, 64, 128, 192 out of 256
        // For measurement = 64: fraction = 64/256 = 1/4
        let result = convergents(64, 256, 10);
        // Should find convergent with denominator 4
        assert!(result.iter().any(|&(_, q)| q == 4));
    }

    #[test]
    fn test_convergents_measurement_192() {
        // measurement = 192: fraction = 192/256 = 3/4
        let result = convergents(192, 256, 10);
        assert!(result.iter().any(|&(_, q)| q == 4));
    }

    #[test]
    fn test_convergents_zero() {
        let result = convergents(0, 256, 10);
        assert!(result.is_empty() || result[0] == (0, 1));
    }

    #[test]
    fn test_convergents_denominator_zero() {
        let result = convergents(5, 0, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mod_pow() {
        assert_eq!(mod_pow(7, 0, 15), 1);
        assert_eq!(mod_pow(7, 1, 15), 7);
        assert_eq!(mod_pow(7, 2, 15), 4); // 49 mod 15
        assert_eq!(mod_pow(7, 3, 15), 13); // 343 mod 15
        assert_eq!(mod_pow(7, 4, 15), 1); // 2401 mod 15 — order is 4
    }

    #[test]
    fn test_mod_pow_large() {
        // Verify no overflow for larger values
        assert_eq!(mod_pow(2, 32, 1_000_000_007), 4_294_967_296 % 1_000_000_007);
    }

    #[test]
    fn test_find_order() {
        assert_eq!(find_order(7, 15), Some(4));
        assert_eq!(find_order(2, 15), Some(4));
        assert_eq!(find_order(11, 15), Some(2));
        assert_eq!(find_order(13, 15), Some(4));
        assert_eq!(find_order(4, 15), Some(2));
    }

    #[test]
    fn test_find_order_not_coprime() {
        assert_eq!(find_order(3, 15), None); // gcd(3,15) = 3
        assert_eq!(find_order(5, 15), None); // gcd(5,15) = 5
    }

    #[test]
    fn test_random_coprime() {
        for _ in 0..100 {
            let a = random_coprime(15);
            assert!((2..15).contains(&a));
            assert_eq!(a.gcd(&15), 1);
        }
    }

    #[test]
    fn test_gcd_from_num_integer() {
        // Verify the num-integer crate works as expected
        assert_eq!(12u64.gcd(&8), 4);
        assert_eq!(15u64.gcd(&7), 1);
        assert_eq!(15u64.gcd(&3), 3);
    }
}
