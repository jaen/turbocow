//! Tests for `EcoVec::drain` and the public `vec::Drain` iterator.
//!
//! These mirror `std::vec::Vec::drain` semantics and ecow 0.3.0's own drain
//! tests, with extra drop-safety / leak-amplification coverage to be run under
//! miri.

use std::cell::Cell;
use std::rc::Rc;

use turbocow::EcoVec;

/// A drop-counted, non-`Copy` element. Increments the shared counter exactly
/// once when dropped.
#[derive(Clone, Debug, PartialEq)]
struct DropCounter {
    id: i32,
    drops: Rc<Cell<usize>>,
}

impl DropCounter {
    fn new(id: i32, drops: &Rc<Cell<usize>>) -> Self {
        Self { id, drops: drops.clone() }
    }
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

fn ids(v: &EcoVec<DropCounter>) -> Vec<i32> {
    v.iter().map(|d| d.id).collect()
}

// ── Basic semantics: contents + drained items ────────────────────────────

#[test]
fn drain_full_range() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(..).collect();
    assert_eq!(drained, vec![0, 1, 2, 3, 4]);
    assert_eq!(v, [] as [i32; 0]);
    assert!(v.is_empty());
}

#[test]
fn drain_prefix() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(..2).collect();
    assert_eq!(drained, vec![0, 1]);
    assert_eq!(v, [2, 3, 4]);
}

#[test]
fn drain_suffix() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(3..).collect();
    assert_eq!(drained, vec![3, 4]);
    assert_eq!(v, [0, 1, 2]);
}

#[test]
fn drain_middle() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(1..4).collect();
    assert_eq!(drained, vec![1, 2, 3]);
    assert_eq!(v, [0, 4]);
}

#[test]
fn drain_empty_range() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(2..2).collect();
    assert!(drained.is_empty());
    assert_eq!(v, [0, 1, 2, 3, 4]);
}

#[test]
fn drain_inclusive_range() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let drained: Vec<i32> = v.drain(1..=3).collect();
    assert_eq!(drained, vec![1, 2, 3]);
    assert_eq!(v, [0, 4]);
}

#[test]
fn drain_double_ended() {
    let mut v: EcoVec<i32> = (0..6).collect();
    let mut d = v.drain(1..5);
    assert_eq!(d.next(), Some(1));
    assert_eq!(d.next_back(), Some(4));
    assert_eq!(d.next(), Some(2));
    assert_eq!(d.next_back(), Some(3));
    assert_eq!(d.next(), None);
    assert_eq!(d.next_back(), None);
    drop(d);
    assert_eq!(v, [0, 5]);
}

#[test]
fn drain_exact_size_and_fused() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let mut d = v.drain(1..4);
    assert_eq!(d.len(), 3);
    assert_eq!(d.next(), Some(1));
    assert_eq!(d.len(), 2);
    let _ = d.next();
    let _ = d.next();
    assert_eq!(d.len(), 0);
    // Fused: keeps returning None.
    assert_eq!(d.next(), None);
    assert_eq!(d.next(), None);
}

// ── as_slice reflects not-yet-yielded drained elements ───────────────────

#[test]
fn drain_as_slice_mid_iteration() {
    let mut v: EcoVec<i32> = (0..6).collect();
    let mut d = v.drain(1..5);
    assert_eq!(d.as_slice(), &[1, 2, 3, 4]);
    assert_eq!(d.next(), Some(1));
    assert_eq!(d.as_slice(), &[2, 3, 4]);
    assert_eq!(d.next_back(), Some(4));
    assert_eq!(d.as_slice(), &[2, 3]);
    let _: &[i32] = d.as_ref();
}

// ── Drop semantics: drained dropped once, remaining intact ───────────────

#[test]
fn drain_drops_drained_exactly_once() {
    let drops = Rc::new(Cell::new(0));
    let mut v: EcoVec<DropCounter> =
        (0..5).map(|i| DropCounter::new(i, &drops)).collect();

    // Drain the middle without consuming the iterator -> Drain::drop must
    // drop the drained elements exactly once.
    {
        let _d = v.drain(1..4);
    }
    assert_eq!(drops.get(), 3, "three drained elements dropped once each");
    assert_eq!(ids(&v), vec![0, 4]);

    // Dropping the vec drops the remaining two.
    drop(v);
    assert_eq!(drops.get(), 5);
}

#[test]
fn drain_partial_iteration_then_drop() {
    let drops = Rc::new(Cell::new(0));
    let mut v: EcoVec<DropCounter> =
        (0..6).map(|i| DropCounter::new(i, &drops)).collect();

    {
        let mut d = v.drain(1..5);
        // Consume two of the four drained elements (these are dropped by the
        // caller as they go out of scope here).
        let a = d.next().unwrap();
        let b = d.next().unwrap();
        assert_eq!((a.id, b.id), (1, 2));
        drop(a);
        drop(b);
        assert_eq!(drops.get(), 2);
        // Dropping `d` must drop the remaining two drained (ids 3, 4) and
        // restore the tail.
    }
    assert_eq!(drops.get(), 4, "all four drained dropped once");
    assert_eq!(ids(&v), vec![0, 5], "tail restored");

    drop(v);
    assert_eq!(drops.get(), 6);
}

#[test]
fn drain_consumed_fully_then_drop() {
    let drops = Rc::new(Cell::new(0));
    let mut v: EcoVec<DropCounter> =
        (0..5).map(|i| DropCounter::new(i, &drops)).collect();
    {
        let d = v.drain(0..3);
        let collected: Vec<i32> = d.map(|x| x.id).collect();
        assert_eq!(collected, vec![0, 1, 2]);
        // The three drained items are dropped here as `collected`'s
        // DropCounters were moved out and `collected` holds i32 — so the
        // DropCounters were dropped during `.map`/collect.
    }
    assert_eq!(drops.get(), 3);
    assert_eq!(ids(&v), vec![3, 4]);
    drop(v);
    assert_eq!(drops.get(), 5);
}

// Leak-amplification tests (`drain_forget_*`) have been moved to
// tests/leak_tests.rs so Miri's strict leak detector can stay on for
// the rest of the suite. See tests/leak_tests.rs and spec §5.1.

// ── CoW: draining a clone must not affect the original ───────────────────

#[test]
fn drain_clone_does_not_affect_other_owner() {
    let a: EcoVec<i32> = (0..5).collect();
    let mut b = a.clone();
    let drained: Vec<i32> = b.drain(1..4).collect();
    assert_eq!(drained, vec![1, 2, 3]);
    assert_eq!(b, [0, 4]);
    // `a` must be untouched.
    assert_eq!(a, [0, 1, 2, 3, 4]);
}

#[test]
fn drain_clone_cow_drop_counts() {
    let drops = Rc::new(Cell::new(0));
    let a: EcoVec<DropCounter> = (0..5).map(|i| DropCounter::new(i, &drops)).collect();
    let mut b = a.clone();
    {
        let _d = b.drain(0..5);
    }
    // Draining a clone forces a unique copy first (CoW). The drained elements
    // are the *copies*, dropped here. `a`'s originals are untouched.
    assert_eq!(ids(&a), vec![0, 1, 2, 3, 4]);
    assert!(b.is_empty());
    // 5 cloned elements were created (for b) and then all 5 dropped by drain.
    assert_eq!(drops.get(), 5);
    drop(b);
    assert_eq!(drops.get(), 5); // b had nothing left
    drop(a);
    assert_eq!(drops.get(), 10); // original 5 dropped now
}

// ── Public type resolution: turbocow::vec::Drain<'_, i32> ─────────────────

#[test]
fn drain_public_type_resolves() {
    let mut v: EcoVec<i32> = (0..3).collect();
    let d: turbocow::vec::Drain<'_, i32> = v.drain(..);
    let collected: Vec<i32> = d.collect();
    assert_eq!(collected, vec![0, 1, 2]);
}

// ── ZST handling ─────────────────────────────────────────────────────────

#[test]
fn drain_zst() {
    let mut v: EcoVec<()> = std::iter::repeat_n((), 5).collect();
    let drained: Vec<()> = v.drain(1..4).collect();
    assert_eq!(drained.len(), 3);
    assert_eq!(v.len(), 2);
}

// ── Debug ────────────────────────────────────────────────────────────────

#[test]
fn drain_debug() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let d = v.drain(1..4);
    assert_eq!(format!("{d:?}"), "Drain([1, 2, 3])");
}

// ── Panic parity with std::Vec::drain ────────────────────────────────────

#[test]
#[should_panic]
fn drain_start_greater_than_end_panics() {
    let mut v: EcoVec<i32> = (0..5).collect();
    #[allow(clippy::reversed_empty_ranges)]
    let _ = v.drain(3..1);
}

#[test]
#[should_panic]
fn drain_end_out_of_bounds_panics() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let _ = v.drain(0..6);
}

#[test]
#[should_panic]
fn drain_inclusive_end_overflow_panics() {
    // `..=usize::MAX` resolves the end to `usize::MAX + 1`, which must panic on
    // overflow (matching std) rather than silently wrapping to `0` and draining
    // nothing.
    let mut v: EcoVec<i32> = (0..5).collect();
    let _ = v.drain(..=usize::MAX);
}

#[test]
#[should_panic]
fn drain_excluded_start_overflow_panics() {
    // An exclusive start of `usize::MAX` resolves the start to `usize::MAX + 1`,
    // which must panic on overflow (matching std) rather than silently wrapping
    // to `0` and draining everything.
    use std::ops::Bound;
    let mut v: EcoVec<i32> = (0..5).collect();
    let _ = v.drain((Bound::Excluded(usize::MAX), Bound::Unbounded));
}
