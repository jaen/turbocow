//! Tests for `EcoVec::splice` and the public `vec::Splice` iterator.
//!
//! These mirror `std::vec::Vec::splice` semantics and ecow 0.3.0's own splice
//! test, with extra drop-safety / leak-amplification coverage to be run under
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

fn dc_vec(ids: &[i32], drops: &Rc<Cell<usize>>) -> EcoVec<DropCounter> {
    ids.iter().map(|&id| DropCounter::new(id, drops)).collect()
}

// ── Basic semantics: contents + yielded items (i32) ──────────────────────

#[test]
fn splice_equal_length() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let removed: Vec<i32> = v.splice(1..3, [10, 11]).collect();
    assert_eq!(removed, vec![1, 2]);
    assert_eq!(v, [0, 10, 11, 3, 4]);
}

#[test]
fn splice_shorter_replacement() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let removed: Vec<i32> = v.splice(1..4, [99]).collect();
    assert_eq!(removed, vec![1, 2, 3]);
    assert_eq!(v, [0, 99, 4]);
}

#[test]
fn splice_longer_replacement() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let removed: Vec<i32> = v.splice(1..3, [20, 21, 22, 23]).collect();
    assert_eq!(removed, vec![1, 2]);
    assert_eq!(v, [0, 20, 21, 22, 23, 3, 4]);
}

#[test]
fn splice_empty_range_pure_insertion() {
    let mut v: EcoVec<i32> = (0..4).collect();
    let removed: Vec<i32> = v.splice(2..2, [100, 101]).collect();
    assert_eq!(removed, vec![] as Vec<i32>);
    assert_eq!(v, [0, 1, 100, 101, 2, 3]);
}

#[test]
fn splice_insert_at_front() {
    let mut v: EcoVec<i32> = (0..3).collect();
    let removed: Vec<i32> = v.splice(0..0, [7, 8]).collect();
    assert_eq!(removed, vec![] as Vec<i32>);
    assert_eq!(v, [7, 8, 0, 1, 2]);
}

#[test]
fn splice_insert_at_end() {
    let mut v: EcoVec<i32> = (0..3).collect();
    let removed: Vec<i32> = v.splice(3..3, [7, 8]).collect();
    assert_eq!(removed, vec![] as Vec<i32>);
    assert_eq!(v, [0, 1, 2, 7, 8]);
}

#[test]
fn splice_drain_to_empty_and_replace() {
    let mut v: EcoVec<i32> = (0..4).collect();
    let removed: Vec<i32> = v.splice(.., [9, 8, 7]).collect();
    assert_eq!(removed, vec![0, 1, 2, 3]);
    assert_eq!(v, [9, 8, 7]);
}

#[test]
fn splice_replace_with_empty_is_pure_removal() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let removed: Vec<i32> = v.splice(1..4, []).collect();
    assert_eq!(removed, vec![1, 2, 3]);
    assert_eq!(v, [0, 4]);
}

#[test]
fn splice_drop_without_consuming_still_applies() {
    // Not iterating the Splice (just dropping it) must still remove the range
    // and insert the replacement.
    let mut v: EcoVec<i32> = (0..5).collect();
    v.splice(1..3, [50, 51, 52]);
    assert_eq!(v, [0, 50, 51, 52, 3, 4]);
}

#[test]
fn splice_partial_iteration_then_drop() {
    let mut v: EcoVec<i32> = (0..6).collect();
    {
        let mut s = v.splice(1..5, [80, 81]);
        // Yield only the first removed element, then drop.
        assert_eq!(s.next(), Some(1));
    }
    // The remaining removed elements (2,3,4) are dropped; replacement applied.
    assert_eq!(v, [0, 80, 81, 5]);
}

#[test]
fn splice_double_ended() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let mut s = v.splice(1..4, [60]);
    assert_eq!(s.next_back(), Some(3));
    assert_eq!(s.next(), Some(1));
    assert_eq!(s.next(), Some(2));
    assert_eq!(s.next(), None);
    drop(s);
    assert_eq!(v, [0, 60, 4]);
}

#[test]
fn splice_exact_size() {
    let mut v: EcoVec<i32> = (0..5).collect();
    let mut s = v.splice(1..4, [60]);
    assert_eq!(s.len(), 3);
    s.next();
    assert_eq!(s.len(), 2);
    drop(s);
    assert_eq!(v, [0, 60, 4]);
}

#[test]
fn splice_non_exact_size_hint_replacement() {
    // Replacement iterator without an exact size hint (filter), longer than the
    // drained range — exercises the `collected` overflow path.
    let mut v: EcoVec<i32> = (0..6).collect();
    v.splice(1..3, [10, 11, 12].into_iter().filter(|_| true));
    assert_eq!(v, [0, 10, 11, 12, 3, 4, 5]);
}

// ── CoW: splicing a clone must not affect the original ────────────────────

#[test]
fn splice_cow_does_not_affect_clone() {
    let a: EcoVec<i32> = (0..5).collect();
    let mut b = a.clone();
    b.splice(1..2, [100, 101]);
    assert_eq!(a, [0, 1, 2, 3, 4]);
    assert_eq!(b, [0, 100, 101, 2, 3, 4]);
}

// ── Drop-count correctness (miri) ─────────────────────────────────────────

#[test]
fn splice_drop_counts_equal_len() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2, 3, 4], &drops);
        let removed: Vec<DropCounter> = v
            .splice(1..3, [DropCounter::new(10, &drops), DropCounter::new(11, &drops)])
            .collect();
        // Removed elements are yielded to the caller (not yet dropped here).
        assert_eq!(removed.iter().map(|d| d.id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(ids(&v), vec![0, 10, 11, 3, 4]);
        drop(removed); // drops 1, 2 -> +2
    }
    // All elements that ever existed: originals 0..4 (5) + replacements 10,11 (2)
    // = 7 distinct logical values; each constructed once must drop exactly once.
    assert_eq!(drops.get(), 7);
}

#[test]
fn splice_drop_counts_longer() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2], &drops);
        v.splice(
            1..2,
            [
                DropCounter::new(10, &drops),
                DropCounter::new(11, &drops),
                DropCounter::new(12, &drops),
            ],
        )
        .for_each(drop); // removed element 1 dropped here
        assert_eq!(ids(&v), vec![0, 10, 11, 12, 2]);
    }
    // originals 0,1,2 (3) + replacements 10,11,12 (3) = 6 constructions, 6 drops.
    assert_eq!(drops.get(), 6);
}

#[test]
fn splice_drop_counts_shorter() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2, 3, 4], &drops);
        v.splice(1..4, [DropCounter::new(10, &drops)]).for_each(drop);
        assert_eq!(ids(&v), vec![0, 10, 4]);
    }
    // originals 0..4 (5) + replacement 10 (1) = 6 constructions, 6 drops.
    assert_eq!(drops.get(), 6);
}

#[test]
fn splice_drop_counts_partial_iteration() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2, 3, 4, 5], &drops);
        {
            let mut s = v.splice(
                1..5,
                [DropCounter::new(80, &drops), DropCounter::new(81, &drops)],
            );
            // Yield one removed element, drop it immediately.
            let first = s.next().unwrap();
            assert_eq!(first.id, 1);
            drop(first); // +1
        } // Splice::drop: removed 2,3,4 dropped (+3), replacement applied.
        assert_eq!(ids(&v), vec![0, 80, 81, 5]);
    }
    // originals 0..5 (6) + replacements 80,81 (2) = 8 constructions, 8 drops.
    assert_eq!(drops.get(), 8);
}

#[test]
fn splice_drop_counts_pure_removal() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2, 3, 4], &drops);
        v.splice(1..4, []).for_each(drop);
        assert_eq!(ids(&v), vec![0, 4]);
    }
    // originals 0..4 (5), no replacements, 5 drops.
    assert_eq!(drops.get(), 5);
}

#[test]
fn splice_panic_in_replacement_is_sound() {
    // A replacement iterator that panics after yielding some elements must
    // leave the vector sound: the already-written replacement elements and the
    // preserved tail are dropped exactly once, no double-free, no uninit read.
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct PanicAfter {
        n: usize,
        drops: Rc<Cell<usize>>,
        next_id: i32,
    }
    impl Iterator for PanicAfter {
        type Item = DropCounter;
        fn next(&mut self) -> Option<DropCounter> {
            if self.n == 0 {
                panic!("boom");
            }
            self.n -= 1;
            let id = self.next_id;
            self.next_id += 1;
            Some(DropCounter::new(id, &self.drops))
        }
    }

    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2, 3], &drops);
        let replace = PanicAfter { n: 2, drops: drops.clone(), next_id: 100 };
        let res = catch_unwind(AssertUnwindSafe(|| {
            // Replace 1 element with an iterator that yields 2 then panics.
            v.splice(1..2, replace).for_each(drop);
        }));
        assert!(res.is_err());
        // The vector must still be in a sound, usable state. The exact length
        // is unspecified after a panic, but all live elements must be dropped
        // exactly once when `v` is dropped below.
        let _ = ids(&v);
    }
    // originals 0,1,2,3 (4) + 2 replacement elements written before the panic
    // (ids 100,101) = 6 constructions; all must be dropped exactly once.
    assert_eq!(drops.get(), 6);
}

#[test]
fn splice_drop_counts_pure_insertion() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut v = dc_vec(&[0, 1, 2], &drops);
        v.splice(2..2, [DropCounter::new(50, &drops)]).for_each(drop);
        assert_eq!(ids(&v), vec![0, 1, 50, 2]);
    }
    // originals 0,1,2 (3) + replacement 50 (1) = 4 constructions, 4 drops.
    assert_eq!(drops.get(), 4);
}

// ── ZST splice (miri): `fill` / `move_tail` reached with a non-empty tail ─
//
// For ZSTs, elements have no identity, so length bookkeeping (not pointer
// moves) must be correct. A non-empty tail forces the `fill`/`move_tail` path
// rather than the `tail_len == 0` `extend` shortcut. `()` is `Copy`, so
// yielded removed elements need no explicit drop accounting; we assert the
// resulting length and equality with a `Vec<()>` oracle.

#[test]
fn splice_zst_longer_replacement() {
    let mut v: EcoVec<()> = core::iter::repeat_n((), 5).collect();
    // Replace 1 with 3 (longer): tail `[2..5)` is non-empty, so `move_tail`
    // grows the logical length and `fill` writes the extra ZSTs.
    let removed: Vec<()> = v.splice(1..2, [(), (), ()]).collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(v.len(), 7);
    assert_eq!(v, vec![(); 7]);
}

#[test]
fn splice_zst_shorter_replacement() {
    let mut v: EcoVec<()> = core::iter::repeat_n((), 5).collect();
    // Replace 3 with 1 (shorter): `fill` writes one, `Drain::drop` moves the
    // non-empty tail `[4..5)` left.
    let removed: Vec<()> = v.splice(1..4, [()]).collect();
    assert_eq!(removed.len(), 3);
    assert_eq!(v.len(), 3);
    assert_eq!(v, vec![(); 3]);
}

#[test]
fn splice_zst_equal_replacement() {
    let mut v: EcoVec<()> = core::iter::repeat_n((), 5).collect();
    // Replace 2 with 2 (equal): hole filled exactly, non-empty tail `[3..5)`
    // moved back to the same place by `Drain::drop`.
    let removed: Vec<()> = v.splice(1..3, [(), ()]).collect();
    assert_eq!(removed.len(), 2);
    assert_eq!(v.len(), 5);
    assert_eq!(v, vec![(); 5]);
}

#[test]
fn splice_zst_drop_without_consuming() {
    // Dropping the Splice unconsumed still applies the (longer) replacement.
    let mut v: EcoVec<()> = core::iter::repeat_n((), 4).collect();
    v.splice(1..2, [(), (), ()]);
    assert_eq!(v.len(), 6);
    assert_eq!(v, vec![(); 6]);
}
