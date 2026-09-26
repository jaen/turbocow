use turbocow::{EcoVec, eco_vec};

// Guarding against something like:
// https://github.com/servo/rust-smallvec/issues/96 aka RUSTSEC-2018-0003
// If length isn't updated defensively then a panic when iterating could
// double-free a value.
#[test]
#[should_panic(expected = "Panic on next")]
fn panicky_iterator_unwinds_correctly() {
    struct PanicIter;

    impl Iterator for PanicIter {
        type Item = u32;

        fn size_hint(&self) -> (usize, Option<usize>) {
            (1, None)
        }

        fn next(&mut self) -> Option<Self::Item> {
            panic!("Panic on next");
        }
    }

    let mut v = eco_vec![1, 2, 3];
    v.extend(PanicIter);
}

// Guarding against something like:
// https://github.com/servo/rust-smallvec/issues/252 aka RUSTSEC-2021-0003
// size_hint should only be treated as a hint, nothing more.
#[test]
fn small_size_hint_is_fine() {
    let mut v = EcoVec::new();
    v.push(123);

    let iter = (0u8..=255).filter(|n| n % 2 == 0);
    assert_eq!(iter.size_hint().0, 0);

    v.extend(iter);

    assert_eq!(
        v,
        core::iter::once(123)
            .chain((0u8..=255).filter(|n| n % 2 == 0))
            .collect::<Vec<_>>()
    );
}

// Guarding against something like:
// https://github.com/Alexhuszagh/rust-stackvector/issues/2 aka RUSTSEC-2021-0048
// size_hint should only be treated as a hint, nothing more.
#[test]
fn wacky_size_hint_is_fine() {
    struct IncorrectIterator(core::iter::Take<core::iter::Repeat<u8>>);

    impl IncorrectIterator {
        pub fn new() -> Self {
            // Intentionally a `Repeat::take` (not `repeat_n`): the field type is
            // `Take<Repeat<u8>>` and this test deliberately exercises a lying
            // `size_hint`, so the concrete iterator type must be preserved.
            #[allow(clippy::manual_repeat_n)]
            IncorrectIterator(core::iter::repeat(1).take(20))
        }
    }

    impl Iterator for IncorrectIterator {
        type Item = u8;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (20, Some(0))
        }
    }

    let mut v = EcoVec::new();
    v.extend(IncorrectIterator::new());

    assert_eq!(v, IncorrectIterator::new().collect::<Vec<_>>())
}

// ─────────────────────────────────────────────────────────────────────────
// SmallVec: owning iteration over a Referenced (zero-copy borrow) variant.
//
// `SmallVec::from_ref` builds a Referenced variant that borrows the caller's
// slice. Owning iteration (`IntoIterator for SmallVec`) must NOT bitwise-move
// owned values out of borrowed memory — doing so would leave both the
// collected container and the caller's original slice owning the same values,
// causing a double-free / use-after-free when both drop. These tests use a
// non-`Copy` type (`String`) so that `ptr::read` is a genuine move and drop
// glue actually runs, which is what makes the UB observable under miri.
// (With `i32`, the move is a harmless bit-copy and drop is a no-op, which is
// why the bug stayed latent.)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn smallvec_referenced_into_iter_full_consume_no_double_drop() {
    // Predates the `DropCounter` drop-count harness below; kept as a simpler
    // `String`-based miri-only anchor for the original double-free repro.
    use turbocow::SmallVec;
    let src = vec![String::from("a"), String::from("b"), String::from("c")];
    let sv: SmallVec<'_, String, 4> = SmallVec::from_ref(&src);
    let collected: Vec<String> = sv.into_iter().collect();
    assert_eq!(collected, src);
    // src must still be valid & owned (no double free / move-from-borrow).
    assert_eq!(src.len(), 3);
    assert_eq!(src[0], "a");
}

#[test]
fn smallvec_referenced_into_iter_partial_then_drop() {
    // Predates the `DropCounter` drop-count harness below; kept as a simpler
    // `String`-based miri-only anchor for the original panic-on-drop repro.
    use turbocow::SmallVec;
    let src = vec![String::from("x"), String::from("y"), String::from("z")];
    let sv: SmallVec<'_, String, 4> = SmallVec::from_ref(&src);
    let mut it = sv.into_iter();
    let first = it.next().unwrap();
    assert_eq!(first, "x");
    // Dropping a partially-consumed IntoIter must NOT panic (the old code hit
    // an `unreachable!()` in `mut_data_ptr` for the Referenced variant) and
    // must NOT double-drop the remaining elements still owned by `src`.
    drop(it);
    assert_eq!(src.len(), 3);
    assert_eq!(src[1], "y");
}

// ─────────────────────────────────────────────────────────────────────────
// Backfill: drop-glue & move-out coverage for the deviated-from-upstream
// unsafe paths in SmallVec/SmallMap that use `ptr::read` / `drop_in_place`.
//
// These run under miri to catch double-free / use-after-free / uninitialized
// reads, and additionally use a drop counter to catch leaks and
// missed/extra drops that miri's leak checker (disabled here via
// `-Zmiri-ignore-leaks`) would not flag.
// ─────────────────────────────────────────────────────────────────────────

mod drop_count {
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Process-wide live count of `DropCounter` instances. Net zero at the
    /// end of a test means every constructed value was dropped exactly once.
    pub static LIVE: AtomicUsize = AtomicUsize::new(0);

    /// A non-`Copy` payload that tracks live instances and can be marked to
    /// panic on its own first clone, to exercise unwinding during
    /// materialisation. The panic trigger is PER-INSTANCE (a flag carried by
    /// the value itself), not a global clone counter — so it has no ordering
    /// dependency on other tests or on how many clones ran before it.
    pub struct DropCounter {
        pub id: u32,
        /// If `true`, this value panics the first time it is cloned.
        panic_on_clone: bool,
    }

    impl DropCounter {
        pub fn new(id: u32) -> Self {
            LIVE.fetch_add(1, Ordering::SeqCst);
            DropCounter { id, panic_on_clone: false }
        }

        /// A value that panics when cloned (used to force an unwind partway
        /// through materialisation).
        pub fn panicking(id: u32) -> Self {
            LIVE.fetch_add(1, Ordering::SeqCst);
            DropCounter { id, panic_on_clone: true }
        }
    }

    impl Clone for DropCounter {
        fn clone(&self) -> Self {
            if self.panic_on_clone {
                panic!("DropCounter {} panicked on clone", self.id);
            }
            LIVE.fetch_add(1, Ordering::SeqCst);
            DropCounter { id: self.id, panic_on_clone: false }
        }
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            LIVE.fetch_sub(1, Ordering::SeqCst);
        }
    }

    use std::sync::{Mutex, MutexGuard};

    /// Serializes drop-counted tests. The `LIVE` counter is process-global,
    /// so tests that assert absolute deltas must not run concurrently with
    /// one another. Each such test holds this guard for its whole body. A
    /// poisoned mutex (from another test panicking *on purpose*, e.g. the
    /// panicking-clone test) is recovered.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Acquire the serial guard and return the starting `LIVE` baseline.
    pub fn lock() -> (MutexGuard<'static, ()>, usize) {
        let guard = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        (guard, LIVE.load(Ordering::SeqCst))
    }
}

use drop_count::{DropCounter, lock};
use std::sync::atomic::Ordering;

// ── SmallVec::into_iter over owned (Inline) storage ──────────────────────

#[test]
fn smallvec_inline_into_iter_full_consume_drops_each_once() {
    use turbocow::SmallVec;
    let (_guard, live0) = lock();
    {
        let mut sv: SmallVec<'_, DropCounter, 4> = SmallVec::new();
        sv.push(DropCounter::new(1));
        sv.push(DropCounter::new(2));
        assert!(sv.is_inline());
        let collected: Vec<DropCounter> = sv.into_iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].id, 1);
        // Two live values now owned solely by `collected`.
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 2);
    }
    // All dropped exactly once; net live back to baseline.
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

// ── SmallVec::into_iter over owned (Heap) storage ────────────────────────

#[test]
fn smallvec_heap_into_iter_full_consume_drops_each_once() {
    use turbocow::SmallVec;
    let (_guard, live0) = lock();
    {
        let mut sv: SmallVec<'_, DropCounter, 2> = SmallVec::new();
        for i in 0..6 {
            sv.push(DropCounter::new(i));
        }
        assert!(!sv.is_inline()); // spilled to heap
        let collected: Vec<DropCounter> = sv.into_iter().collect();
        assert_eq!(collected.len(), 6);
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 6);
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

// ── SmallVec::into_iter partial consume then drop (Heap) ─────────────────

#[test]
fn smallvec_heap_into_iter_partial_then_drop_no_leak_no_double_drop() {
    use turbocow::SmallVec;
    let (_guard, live0) = lock();
    {
        let mut sv: SmallVec<'_, DropCounter, 2> = SmallVec::new();
        for i in 0..5 {
            sv.push(DropCounter::new(i));
        }
        let mut it = sv.into_iter();
        let _a = it.next().unwrap(); // moved out, dropped at end of block
        let _b = it.next_back().unwrap(); // moved out via DoubleEndedIterator
        // 3 remaining elements still owned by `it`; IntoIter::drop must
        // drop_in_place exactly those, with no double-drop of _a/_b.
        drop(it);
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 2); // _a, _b
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

// ── SmallVec::into_iter over empty vec ───────────────────────────────────

#[test]
fn smallvec_empty_into_iter_is_sound() {
    use turbocow::SmallVec;
    // Holds the serial guard because it constructs `DropCounter`s (touching
    // the global `LIVE` counter), so it must not run concurrently with the
    // tests that assert absolute `LIVE` deltas.
    let (_guard, _live0) = lock();
    let sv: SmallVec<'_, DropCounter, 4> = SmallVec::new();
    let collected: Vec<DropCounter> = sv.into_iter().collect();
    assert!(collected.is_empty());

    // Also exercise the empty Referenced case (the false SAFETY comment
    // claimed Referenced always has len>0; `from_ref(&[])` disproves it).
    let empty: [DropCounter; 0] = [];
    let sv2: SmallVec<'_, DropCounter, 4> = SmallVec::from_ref(&empty);
    let collected2: Vec<DropCounter> = sv2.into_iter().collect();
    assert!(collected2.is_empty());
}

// ── Panic during materialisation (clone) must not leak or double-free ────

#[test]
fn smallvec_referenced_into_iter_panicking_clone_unwinds_cleanly() {
    use turbocow::SmallVec;
    let (_guard, live0) = lock();
    // The third element panics on its own first clone. Materialisation clones
    // src[0], src[1] (OK), then src[2] panics mid-way. The two already-cloned
    // values must be dropped during unwinding; `src` itself stays fully owned
    // and intact. The trigger is per-instance, so this has no dependency on a
    // global clone count or on which other tests ran first.
    let src = vec![DropCounter::new(0), DropCounter::new(1), DropCounter::panicking(2)];
    let live_with_src = drop_count::LIVE.load(Ordering::SeqCst);
    assert_eq!(live_with_src, live0 + 3);

    let sv: SmallVec<'_, DropCounter, 4> = SmallVec::from_ref(&src);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = sv.into_iter().collect::<Vec<_>>();
    }));
    assert!(result.is_err(), "expected the panicking clone to unwind");

    // The originals (`src`) are untouched: still exactly 3 live from this
    // test. The partially-cloned values were dropped during unwinding (no
    // leak, no double-free — miri would flag a double-free).
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 3);
    assert_eq!(src.len(), 3);
    drop(src);
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

// ── SmallMap: read / drop_in_place / Drop glue paths ─────────────────────

#[test]
fn smallmap_into_iter_full_consume_drops_each_once() {
    use turbocow::SmallMap;
    let (_guard, live0) = lock();
    {
        let mut map: SmallMap<String, DropCounter> = SmallMap::new();
        map.insert("a".to_string(), DropCounter::new(1));
        map.insert("b".to_string(), DropCounter::new(2));
        let mut entries: Vec<(String, DropCounter)> = map.into_iter().collect();
        entries.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "a");
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

#[test]
fn smallmap_into_iter_partial_then_drop_no_double_drop() {
    use turbocow::SmallMap;
    let (_guard, live0) = lock();
    {
        let mut map: SmallMap<String, DropCounter> = SmallMap::new();
        for i in 0..3 {
            map.insert(format!("k{i}"), DropCounter::new(i));
        }
        let mut it = map.into_iter();
        let _first = it.next().unwrap(); // moved out via ptr::read
        // IntoIter::drop must drop_in_place the 2 remaining entries only.
        drop(it);
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 1); // _first
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

#[test]
fn smallmap_drop_drops_each_entry_once() {
    use turbocow::SmallMap;
    let (_guard, live0) = lock();
    {
        let mut map: SmallMap<String, DropCounter> = SmallMap::new();
        // Stay inline (default N is small) then also force heap by inserting
        // many entries — both Drop paths exercised across the two blocks.
        for i in 0..2 {
            map.insert(format!("k{i}"), DropCounter::new(i));
        }
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 2);
        // map dropped here without iterating: exercises InlineMap/heap Drop.
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);

    {
        let mut map: SmallMap<String, DropCounter> = SmallMap::new();
        for i in 0..32 {
            map.insert(format!("k{i}"), DropCounter::new(i));
        }
        assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0 + 32);
    }
    assert_eq!(drop_count::LIVE.load(Ordering::SeqCst), live0);
}

// ── EcoVec::replace panic-safety (bug #3) ───────────────────────────────────
//
// EcoVec::replace did drop_in_place(at) then ptr::write(at, value). If
// T::drop panicked, the slot was uninitialized but len was unchanged →
// EcoVec::drop re-dropped it → panic-in-destructor → process abort.
//
// Regression test for bug #3 (EcoVec::replace double-drop on panicking Drop).

#[test]
fn ecovec_replace_panicking_drop_no_double_drop() {
    use std::cell::RefCell;
    use std::panic::AssertUnwindSafe;
    use turbocow::EcoVec;

    thread_local! {
        static DROP_LOG: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    #[derive(Clone)]
    struct PanickyDrop(u32);
    impl Drop for PanickyDrop {
        fn drop(&mut self) {
            DROP_LOG.with(|c| c.borrow_mut().push(self.0));
            if self.0 == 1 {
                panic!("boom in Drop for id=1");
            }
        }
    }

    let mut v: EcoVec<PanickyDrop> = vec![PanickyDrop(0), PanickyDrop(1), PanickyDrop(2)]
        .into_iter()
        .collect();

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        v.replace(1, PanickyDrop(99));
    }));
    assert!(result.is_err(), "expected the drop panic to propagate");

    // Drop v explicitly so EcoVec::drop runs over the surviving slots
    // [0, 99, 2] before we read the log. v is still alive here because the
    // catch_unwind closure only borrowed it by &mut.
    drop(v);

    // After unwind + EcoVec::drop, the drop log should contain:
    //   - id=1 once (the original at slot 1, dropped during replace)
    //   - id=0 once (slot 0, dropped during EcoVec::drop after unwind)
    //   - id=2 once (slot 2, dropped during EcoVec::drop after unwind)
    //   - id=99 once (the replacement at slot 1, dropped during EcoVec::drop)
    //
    // The bug (current behavior): abort during unwind, so we never reach here.
    // After the fix: we reach here with each id present exactly once (except
    // id=1 which is the panicking one and is also dropped once).
    let log: Vec<u32> = DROP_LOG.with(|c| c.borrow().clone());
    eprintln!("drop log: {log:?}");

    let count = |id: u32| log.iter().filter(|&&x| x == id).count();
    assert_eq!(count(0), 1, "id=0 dropped exactly once");
    assert_eq!(count(1), 1, "id=1 dropped exactly once (the panicking one)");
    assert_eq!(count(2), 1, "id=2 dropped exactly once");
    assert_eq!(count(99), 1, "id=99 (replacement) dropped exactly once");
}

// ── SmallMap::force_spill entry double-drop (bug #5) ────────────────────────
//
// force_spill moved entries out via ptr::read but didn't decrement
// inline.len until after the loop. If heap.insert panicked mid-loop,
// InlineMap::drop iterated 0..original_len and re-dropped already-moved
// entries.
//
// Regression test for bug #5 (force_spill entry double-drop on panicking insert).

#[test]
fn force_spill_panicking_insert_no_double_drop() {
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::hash::Hasher;
    use std::panic::AssertUnwindSafe;
    use std::rc::Rc;
    use turbocow::SmallMap;

    // K that panics on hash after N calls.
    struct PanickyHash {
        id: u32,
        counter: Rc<Cell<u32>>,
        panic_at: u32,
    }
    impl std::hash::Hash for PanickyHash {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let n = self.counter.get();
            self.counter.set(n + 1);
            if n >= self.panic_at {
                panic!("boom in hash");
            }
            self.id.hash(state);
        }
    }
    impl PartialEq for PanickyHash {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for PanickyHash {}

    // Count drops via Rc; we panic if any id is dropped twice.
    thread_local! {
        static DROPS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    // We need Drop on K. But PanickyHash needs to be the K type. So:
    impl Drop for PanickyHash {
        fn drop(&mut self) {
            DROPS.with(|c| c.borrow_mut().push(self.id));
        }
    }

    let counter = Rc::new(Cell::new(0u32));
    let mut map: SmallMap<PanickyHash, u32, 4> = SmallMap::new();

    // Insert 4 entries (fills inline); the 5th insert triggers force_spill,
    // which calls hash() during the spill. All entries share panic_at=6 so
    // the panic fires during force_spill's re-insert (3rd re-insert sees
    // counter=6); with panic_at=100 the panic only happens in the
    // post-spill heap.insert, by which point force_spill has finished
    // cleanly and bug #5 is never exercised.
    map.insert(PanickyHash { id: 0, counter: counter.clone(), panic_at: 6 }, 0);
    map.insert(PanickyHash { id: 1, counter: counter.clone(), panic_at: 6 }, 1);
    map.insert(PanickyHash { id: 2, counter: counter.clone(), panic_at: 6 }, 2);
    map.insert(PanickyHash { id: 3, counter: counter.clone(), panic_at: 6 }, 3);
    // counter is now 4 (4 hashes during the 4 inserts).

    // 5th insert triggers force_spill; we want hash to panic during the spill.
    // The spill re-inserts all 4 entries, calling hash 4 more times. Set
    // panic_at = 6 so the 2nd spill-insert (counter=6) panics.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        map.insert(PanickyHash { id: 4, counter: counter.clone(), panic_at: 6 }, 4);
    }));
    assert!(result.is_err(), "expected hash panic to propagate");

    // Drop map explicitly so SmallMap::drop runs over the surviving entries
    // before we read the log. map is still alive here because the
    // catch_unwind closure only borrowed it by &mut.
    drop(map);

    let drops: Vec<u32> = DROPS.with(|c| c.borrow().clone());
    eprintln!("drop log: {drops:?}");

    // Each id 0..4 should appear exactly once in the drop log.
    // The bug would show id 0 and id 1 twice (re-dropped by InlineMap::drop).
    for id in 0..4 {
        let count = drops.iter().filter(|&&x| x == id).count();
        assert_eq!(count, 1, "id={id} should be dropped exactly once; got {count}");
    }
}

// force_spill ptr::read's the hasher out BEFORE the spill loop and only
// mem::forget()s the moved-out inline AFTER it. If K::hash / K::eq panics during
// the loop, `self` stays Inline, so the inline's hasher field (a bitwise copy of
// the one now owned by the heap) is dropped by field glue AND the heap drops its
// own copy → the hasher is dropped twice. Inert for the default ZST hasher
// (which is why the bug #4/#5 tests above miss it), but a soundness bug for any
// `S: BuildHasher + Drop`. Combines bug #4 (non-Copy hasher) with bug #5's
// panicking-insert path.
#[test]
fn force_spill_panicking_insert_drops_hasher_once() {
    use std::cell::Cell;
    use std::hash::{BuildHasher, Hasher};
    use std::panic::AssertUnwindSafe;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use turbocow::SmallMap;

    static HASHER_DROPS: AtomicUsize = AtomicUsize::new(0);

    // Non-Copy BuildHasher with a Drop impl.
    struct DropHasher;
    impl BuildHasher for DropHasher {
        type Hasher = std::collections::hash_map::DefaultHasher;
        fn build_hasher(&self) -> Self::Hasher {
            std::collections::hash_map::DefaultHasher::new()
        }
    }
    impl Drop for DropHasher {
        fn drop(&mut self) {
            HASHER_DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    // K that panics on its Nth hash call (calibrated to fire during force_spill).
    struct PanickyHash {
        id: u32,
        counter: Rc<Cell<u32>>,
        panic_at: u32,
    }
    impl std::hash::Hash for PanickyHash {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let n = self.counter.get();
            self.counter.set(n + 1);
            if n >= self.panic_at {
                panic!("boom in hash");
            }
            self.id.hash(state);
        }
    }
    impl PartialEq for PanickyHash {
        fn eq(&self, o: &Self) -> bool {
            self.id == o.id
        }
    }
    impl Eq for PanickyHash {}

    let counter = Rc::new(Cell::new(0u32));
    HASHER_DROPS.store(0, Ordering::SeqCst);
    {
        let mut map: SmallMap<PanickyHash, u32, 4, DropHasher> =
            SmallMap::with_hasher(DropHasher);
        for id in 0..4 {
            map.insert(PanickyHash { id, counter: counter.clone(), panic_at: 6 }, id);
        }
        // The 5th insert triggers force_spill; a spill re-insert (counter == 6)
        // panics mid-loop, after the hasher has been moved into the heap.
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| {
            map.insert(PanickyHash { id: 4, counter: counter.clone(), panic_at: 6 }, 4);
        }));
        assert!(r.is_err(), "expected the hash panic to propagate");
        // `map` drops here (still Inline under the bug) → second hasher drop.
    }
    assert_eq!(
        HASHER_DROPS.load(Ordering::SeqCst),
        1,
        "hasher must be dropped exactly once; got {} (the bug double-drops it)",
        HASHER_DROPS.load(Ordering::SeqCst)
    );
}

// ── EcoMap::remove panic-safety (bug #14) ───────────────────────────────────
//
// remove did keys.pop() then vals.pop() sequentially. If K::drop panicked
// (inside keys.pop's tail), vals.pop never ran → keys.len() != vals.len(),
// breaking the map invariant. Subsequent indexing was unsound.
//
// Regression test for bug #14 (EcoMap::remove keys/vals length desync).

#[test]
fn ecomap_remove_panicking_key_drop_keeps_vecs_aligned() {
    use std::borrow::Borrow;
    use std::cell::RefCell;
    use std::panic::AssertUnwindSafe;
    use turbocow::EcoMap;

    thread_local! {
        static DROPS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    #[derive(Clone)]
    struct PanickyKey(u32);
    impl PartialEq for PanickyKey {
        fn eq(&self, o: &Self) -> bool {
            self.0 == o.0
        }
    }
    impl Eq for PanickyKey {}
    impl std::hash::Hash for PanickyKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state)
        }
    }
    impl Borrow<u32> for PanickyKey {
        fn borrow(&self) -> &u32 {
            &self.0
        }
    }
    impl Drop for PanickyKey {
        fn drop(&mut self) {
            DROPS.with(|c| c.borrow_mut().push(self.0));
            if self.0 == 1 {
                panic!("boom in PanickyKey::drop");
            }
        }
    }

    let mut map: EcoMap<PanickyKey, u32> = EcoMap::new();
    map.insert(PanickyKey(0), 100);
    map.insert(PanickyKey(1), 200); // this one panics on drop
    map.insert(PanickyKey(2), 300);

    // Look up by &u32 (via Borrow<u32>) rather than &PanickyKey(1) so the
    // lookup temporary has no Drop and won't cause a double-panic abort
    // during unwind cleanup.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        map.remove(&1u32);
    }));
    assert!(result.is_err(), "expected key drop panic to propagate");

    // After unwind, the keys and vals vecs must remain length-aligned. `len()`
    // and `iter()` both read the keys side (len() == keys.len(), iter() zips to
    // the shorter), so they cannot detect a `keys.len() != vals.len()` skew on
    // their own — that was a fake RED. `keys()` and `values()` iterate the two
    // backing vecs independently, so comparing their counts is what actually
    // catches the bug #14 desync.
    assert_eq!(
        map.keys().count(),
        map.values().count(),
        "keys ({}) and vals ({}) must stay length-aligned after a panicking remove",
        map.keys().count(),
        map.values().count()
    );
    // len()/iter() must also stay self-consistent.
    assert_eq!(map.iter().count(), map.len());

    let drops: Vec<u32> = DROPS.with(|c| c.borrow().clone());
    eprintln!("drop log: {drops:?}");
    // Each id should be dropped at most once during the panic path. id=1
    // is the panicking one; id=0 and id=2 are still in the map after catch.
    for id in [0u32, 2] {
        let count = drops.iter().filter(|&&x| x == id).count();
        assert_eq!(count, 0, "id={id} should not be dropped (still in map); got {count}");
    }
}

// ── SmallVec::materialise_referenced heap-path clone leak (bug #12) ────────
//
// materialise_referenced's heap branch pre-allocates with new_heap(len)
// (tagged_len = encode(TAG_HEAP, 0)) then uses raw ptr::write in a loop.
// If T::clone panics on iteration i, the i already-written elements are
// untracked (tagged_len == 0) → SmallVec::drop skips their Drop glue → leak.
//
// The crate's own Clone impl uses a CloneGuard for exactly this case; the
// materialise_referenced heap path should too.
//
// Regression test for bug #12 (SmallVec::materialise_referenced heap-path leak).

#[test]
fn smallvec_materialise_referenced_panicking_clone_no_leak() {
    use std::cell::RefCell;
    use std::panic::AssertUnwindSafe;
    use turbocow::SmallVec;

    thread_local! {
        static DROP_LOG: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    struct PanickyClone {
        id: u32,
        panic_on_clone: bool,
    }
    impl Clone for PanickyClone {
        fn clone(&self) -> Self {
            if self.panic_on_clone {
                panic!("boom in Clone for id={}", self.id);
            }
            Self { id: self.id, panic_on_clone: self.panic_on_clone }
        }
    }
    impl Drop for PanickyClone {
        fn drop(&mut self) {
            DROP_LOG.with(|c| c.borrow_mut().push(self.id));
        }
    }

    // Source data for the Referenced variant. N=4 means 5+ elements spill
    // to heap during materialise_referenced.
    let source: Vec<PanickyClone> = (0..6u32)
        .map(|i| PanickyClone { id: i, panic_on_clone: i == 3 })
        .collect();
    let source_ref: &[PanickyClone] = &source;

    // Create a Referenced SmallVec, then trigger materialise by pushing.
    // The 5th clone (id=3) panics. Elements 0,1,2 were already cloned into
    // the heap buffer; without CloneGuard they leak.
    let mut sv: SmallVec<'_, PanickyClone, 4> = SmallVec::from_ref(source_ref);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        sv.push(PanickyClone { id: 99, panic_on_clone: false });
    }));
    assert!(result.is_err(), "expected clone panic to propagate");

    // Drop sv explicitly so its Drop runs before we read the log.
    drop(sv);

    let log: Vec<u32> = DROP_LOG.with(|c| c.borrow().clone());
    eprintln!("drop log: {log:?}");

    // After the fix: elements 0, 1, 2 (cloned into heap before the panic)
    // must each appear exactly once in the drop log (dropped by CloneGuard
    // during unwind). Element 99 (the push value) also appears once.
    // Element 3 never got cloned (panic), so it only appears via the
    // original source Vec drop — not here (source is dropped at end of test).
    for id in [0u32, 1, 2] {
        let count = log.iter().filter(|&&x| x == id).count();
        assert_eq!(
            count, 1,
            "id={id} (cloned into heap) should be dropped exactly once; got {count}"
        );
    }
    let count_99 = log.iter().filter(|&&x| x == 99).count();
    assert_eq!(
        count_99, 1,
        "id=99 (push value) should be dropped exactly once; got {count_99}"
    );
}

// ── Drain offset_from provenance (drain.rs:172-185) ─────────────────────────
//
// When `Drain` is dropped with un-yielded elements, `Drain::drop` reconstructs
// a mutable pointer to the remaining slice via `offset_from` and runs
// `drop_in_place` on it. This is only sound when the source pointer carries
// the vector's mutable provenance; constructing it from the `slice::Iter`
// backing pointer (which is `const`-provenance) would be UB.
//
// This test is a Miri-specific regression guard (`#[cfg(miri)]`) that makes
// the offset_from path explicit. It overlaps with `drain_partial_iteration_then_drop`
// in `tests/vec_drain.rs` but uses a non-`Copy` element so the drop glue is
// observable under `-Zmiri-strict-provenance`. Miri would flag any
// provenance mismatch as a UB report.

#[cfg(miri)]
#[test]
fn drain_offset_from_provenance_miri() {
    use std::cell::Cell;
    use std::rc::Rc;
    use turbocow::EcoVec;

    // Simple DropCounter: a non-`Copy` wrapper around a shared drop tally.
    // Each instance holds its own `Rc` into the same `Cell`; dropping bumps
    // the tally, so the final count is the total number of `D` drops.
    #[derive(Clone)]
    struct D(Rc<Cell<u32>>);
    impl Drop for D {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Rc::new(Cell::new(0u32));
    let mut v: EcoVec<D> = (0..6).map(|_| D(Rc::clone(&drops))).collect();

    {
        let mut d = v.drain(1..5);
        let _yielded = d.next(); // yield 1 element (id=1); dropped at end of block
        // `d` is dropped here with 3 un-yielded elements (ids 2, 3, 4).
        // `Drain::drop` runs the offset_from path in drain.rs:172-185:
        //   drop_ptr = iter.as_slice().as_ptr();
        //   vec_ptr  = vec.as_mut().data_mut();
        //   drop_offset = drop_ptr.offset_from(vec_ptr);
        //   drop_in_place(slice_from_raw_parts_mut(vec_ptr.add(drop_offset), 3));
        // If `drop_ptr` had lost mutable provenance, Miri's strict-provenance
        // check would report UB here.
    }

    // 1 yielded (dropped by caller) + 3 dropped by Drain::drop = 4.
    // The 2 elements outside the drain range (ids 0, 5) are still in `v`
    // and contribute 0 to the tally here.
    assert_eq!(
        drops.get(),
        4,
        "exactly 4 elements dropped (1 yielded + 3 by Drain::drop)"
    );

    drop(v);
    // All 6 now dropped.
    assert_eq!(drops.get(), 6);
}
