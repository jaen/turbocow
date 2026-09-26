//! Tests that intentionally leak memory via `mem::forget(Drain)`.
//!
//! These are split into their own test binary because Miri's leak detector
//! flags them. CI runs this binary with `-Zmiri-ignore-leaks`; the main Miri
//! run keeps strict leak detection for all other tests (bug #7).

use std::cell::Cell;
use std::rc::Rc;
use turbocow::EcoVec;

// ── helpers (duplicated from tests/vec_drain.rs to keep this binary standalone) ──

#[derive(Clone)]
struct DropCounter {
    id: u32,
    counter: Rc<Cell<u32>>,
}

impl DropCounter {
    fn new(id: u32, counter: &Rc<Cell<u32>>) -> Self {
        Self { id, counter: counter.clone() }
    }
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

fn ids(v: &EcoVec<DropCounter>) -> Vec<u32> {
    v.iter().map(|d| d.id).collect()
}

// ── leak tests (moved from tests/vec_drain.rs) ───────────────────────────────────

#[test]
fn drain_forget_after_partial_iteration() {
    // After forgetting a partially-iterated Drain, the vec must be the prefix
    // `[..start]` and there must be no double free. The forgotten (un-yielded)
    // drained elements and the tail are leaked, which is sound.
    //
    // EXPECTED TO LEAK — run with `-Zmiri-ignore-leaks`.
    let drops = Rc::new(Cell::new(0));
    let mut v: EcoVec<DropCounter> =
        (0..6).map(|i| DropCounter::new(i, &drops)).collect();

    {
        let mut d = v.drain(1..5);
        let a = d.next().unwrap();
        assert_eq!(a.id, 1);
        std::mem::forget(d);
        drop(a);
    }
    assert_eq!(drops.get(), 1);
    assert_eq!(ids(&v), vec![0], "vec is the prefix [..start]");

    drop(v);
    assert_eq!(drops.get(), 2, "no double free of leaked elements");
}

#[test]
fn drain_forget_with_copy_type() {
    // EXPECTED TO LEAK — run with `-Zmiri-ignore-leaks`.
    let mut v: EcoVec<i32> = (0..6).collect();
    {
        let mut d = v.drain(2..5);
        assert_eq!(d.next(), Some(2));
        std::mem::forget(d);
    }
    assert_eq!(v, [0, 1], "vec is the prefix [..start]");
}

#[test]
fn drain_zst_forget() {
    // EXPECTED TO LEAK — run with `-Zmiri-ignore-leaks`.
    let mut v: EcoVec<()> = std::iter::repeat_n((), 5).collect();
    {
        let mut d = v.drain(1..4);
        let _ = d.next();
        std::mem::forget(d);
    }
    assert_eq!(v.len(), 1, "len left at start");
}
