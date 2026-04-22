//! Type-safe qubit handles and allocation.
//!
//! Replaces raw `usize` indices with [`Qubit`] newtypes and [`QubitRange`]
//! named groups, preventing qubit collision and dangling-index bugs at
//! compile time.
//!
//! See `docs/features/QubitTypeSafety.md` for design rationale.

use std::fmt;
use std::ops::RangeBounds;

// ---------------------------------------------------------------------------
// Qubit
// ---------------------------------------------------------------------------

/// A qubit handle — wraps a physical index but prevents accidental
/// misuse as a plain integer.
///
/// Cannot be created from arbitrary `usize` outside this crate.
/// Use [`QubitAllocator::allocate`] to obtain qubits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Qubit(usize);

impl Qubit {
    /// Raw index for interfacing with roqoqo gates.
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }

    /// Crate-internal: wrap a raw index from roqoqo gate accessors.
    ///
    /// Used by circuit transformations (e.g., `controlled()`) that
    /// extract indices from existing gates and construct new ones.
    #[inline]
    #[allow(dead_code)] // used in Phase 2 (circuit primitives migration)
    pub(crate) fn from_raw(index: usize) -> Self {
        Qubit(index)
    }
}

impl fmt::Display for Qubit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "q{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// QubitRange
// ---------------------------------------------------------------------------

/// A contiguous range of qubits with a debug label.
///
/// Stores only the start index and length — no heap allocation for
/// qubit storage. Qubit values are computed on access:
/// `qubit(i) = Qubit(start + i)`.
///
/// Supports sub-register extraction via [`slice`](Self::slice) and
/// [`split_at`](Self::split_at).
#[derive(Debug, Clone)]
pub struct QubitRange {
    label: String,
    start: usize,
    length: usize,
}

impl QubitRange {
    /// Number of qubits in this register.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Whether this register is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get the i-th qubit. Panics if out of bounds.
    #[inline]
    pub fn qubit(&self, i: usize) -> Qubit {
        assert!(
            i < self.length,
            "index {} out of bounds for register '{}' of length {}",
            i,
            self.label,
            self.length
        );
        Qubit(self.start + i)
    }

    /// Iterator over qubits.
    pub fn iter(&self) -> impl Iterator<Item = Qubit> + '_ {
        let start = self.start;
        (0..self.length).map(move |i| Qubit(start + i))
    }

    /// Materialize as `Vec<Qubit>` for passing to `&[Qubit]` APIs
    /// (e.g., `build_multi_cx`, `controlled_add`).
    pub fn to_qubits(&self) -> Vec<Qubit> {
        (self.start..self.start + self.length).map(Qubit).collect()
    }

    /// The debug label for this register.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Extract a sub-register covering `range`.
    ///
    /// The sub-register inherits the parent's qubit indices and gets
    /// an auto-generated label like `"oracle[0..4]"`.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> QubitRange {
        use std::ops::Bound;
        let s = match range.start_bound() {
            Bound::Included(&v) => v,
            Bound::Excluded(&v) => v + 1,
            Bound::Unbounded => 0,
        };
        let e = match range.end_bound() {
            Bound::Included(&v) => v + 1,
            Bound::Excluded(&v) => v,
            Bound::Unbounded => self.length,
        };
        assert!(
            e <= self.length,
            "slice end {} out of bounds for register '{}' of length {}",
            e,
            self.label,
            self.length
        );
        assert!(s <= e, "slice start {} > end {}", s, e);
        QubitRange {
            label: format!("{}[{}..{}]", self.label, s, e),
            start: self.start + s,
            length: e - s,
        }
    }

    /// Split into two sub-registers at index `mid`.
    pub fn split_at(&self, mid: usize) -> (QubitRange, QubitRange) {
        (self.slice(..mid), self.slice(mid..))
    }
}

impl fmt::Display for QubitRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.label, self.length)
    }
}

// ---------------------------------------------------------------------------
// QubitAllocator
// ---------------------------------------------------------------------------

/// Allocates non-overlapping qubit registers.
///
/// Each circuit construction should use exactly one allocator.
/// Qubit equality is index-based, so qubits from different allocators
/// would falsely compare equal.
pub struct QubitAllocator {
    next: usize,
}

impl QubitAllocator {
    /// Create a new allocator starting at index 0.
    pub fn new() -> Self {
        Self { next: 0 }
    }

    /// Allocate a named register of `n` qubits.
    ///
    /// Returns a register with qubit indices `[next, next+n)`.
    /// Panics if the total allocation would overflow `usize`.
    pub fn allocate(&mut self, label: &str, n: usize) -> QubitRange {
        let new_next = self.next.checked_add(n).expect("qubit index overflow");
        let reg = QubitRange {
            label: label.to_string(),
            start: self.next,
            length: n,
        };
        self.next = new_next;
        reg
    }

    /// Total qubits allocated so far.
    pub fn total(&self) -> usize {
        self.next
    }
}

impl Default for QubitAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qubit_index() {
        let q = Qubit::from_raw(42);
        assert_eq!(q.index(), 42);
        assert_eq!(format!("{}", q), "q42");
    }

    #[test]
    fn test_qubit_equality() {
        assert_eq!(Qubit::from_raw(3), Qubit::from_raw(3));
        assert_ne!(Qubit::from_raw(3), Qubit::from_raw(4));
    }

    #[test]
    fn test_allocator_basic() {
        let mut alloc = QubitAllocator::new();
        let data = alloc.allocate("data", 3);
        let scratch = alloc.allocate("scratch", 2);

        assert_eq!(data.len(), 3);
        assert_eq!(scratch.len(), 2);
        assert_eq!(alloc.total(), 5);

        // Data qubits: 0, 1, 2
        assert_eq!(data.qubit(0).index(), 0);
        assert_eq!(data.qubit(2).index(), 2);
        // Scratch qubits: 3, 4
        assert_eq!(scratch.qubit(0).index(), 3);
        assert_eq!(scratch.qubit(1).index(), 4);
    }

    #[test]
    fn test_allocator_disjoint() {
        let mut alloc = QubitAllocator::new();
        let a = alloc.allocate("a", 4);
        let b = alloc.allocate("b", 3);

        // No overlap
        for qa in a.iter() {
            for qb in b.iter() {
                assert_ne!(qa, qb);
            }
        }
    }

    #[test]
    fn test_allocator_empty_register() {
        let mut alloc = QubitAllocator::new();
        let empty = alloc.allocate("empty", 0);
        let next = alloc.allocate("next", 2);

        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.to_qubits(), vec![]);
        assert_eq!(next.qubit(0).index(), 0);
        assert_eq!(next.qubit(1).index(), 1);
    }

    #[test]
    fn test_register_qubit_access() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 4);

        assert_eq!(reg.qubit(0), Qubit(0));
        assert_eq!(reg.qubit(3), Qubit(3));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_register_qubit_oob() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 3);
        reg.qubit(3); // out of bounds
    }

    #[test]
    fn test_register_to_qubits() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 3);
        let qubits = reg.to_qubits();

        assert_eq!(qubits.len(), 3);
        assert_eq!(qubits[0].index(), 0);
        assert_eq!(qubits[2].index(), 2);
    }

    #[test]
    fn test_register_iter() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 3);
        let collected: Vec<usize> = reg.iter().map(|q| q.index()).collect();
        assert_eq!(collected, vec![0, 1, 2]);
    }

    #[test]
    fn test_register_slice() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 5);

        let sub = reg.slice(1..4);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.qubit(0).index(), 1);
        assert_eq!(sub.qubit(2).index(), 3);
        assert!(sub.label().contains("data"));
    }

    #[test]
    fn test_register_slice_unbounded() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 5);

        let head = reg.slice(..3);
        assert_eq!(head.len(), 3);
        assert_eq!(head.qubit(2).index(), 2);

        let tail = reg.slice(3..);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail.qubit(0).index(), 3);
        assert_eq!(tail.qubit(1).index(), 4);
    }

    #[test]
    fn test_register_split_at() {
        let mut alloc = QubitAllocator::new();
        let _ = alloc.allocate("prefix", 10); // offset so indices aren't 0-based
        let reg = alloc.allocate("data", 6);

        let (left, right) = reg.split_at(4);
        assert_eq!(left.len(), 4);
        assert_eq!(right.len(), 2);
        assert_eq!(left.qubit(0).index(), 10);
        assert_eq!(left.qubit(3).index(), 13);
        assert_eq!(right.qubit(0).index(), 14);
        assert_eq!(right.qubit(1).index(), 15);
    }

    #[test]
    fn test_register_split_at_zero() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 3);

        let (left, right) = reg.split_at(0);
        assert!(left.is_empty());
        assert_eq!(right.len(), 3);
    }

    #[test]
    fn test_register_slice_of_slice() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 10);

        let sub = reg.slice(2..8);
        let subsub = sub.slice(1..4);
        assert_eq!(subsub.qubit(0).index(), 3);
        assert_eq!(subsub.qubit(1).index(), 4);
        assert_eq!(subsub.qubit(2).index(), 5);
    }

    #[test]
    fn test_register_label() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("oracle", 5);
        assert_eq!(reg.label(), "oracle");
        assert_eq!(format!("{}", reg), "oracle[5]");

        let sub = reg.slice(1..3);
        assert_eq!(sub.label(), "oracle[1..3]");
    }

    #[test]
    fn test_register_display() {
        let mut alloc = QubitAllocator::new();
        let reg = alloc.allocate("data", 3);
        assert_eq!(format!("{}", reg), "data[3]");
    }

    #[test]
    fn test_allocator_default() {
        let mut alloc = QubitAllocator::default();
        let reg = alloc.allocate("test", 2);
        assert_eq!(reg.qubit(0).index(), 0);
        assert_eq!(reg.qubit(1).index(), 1);
    }
}
