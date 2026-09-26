use turbocow::SmallVec;

// ── Construction ────────────────────────────────────────────────────────

#[test]
fn new_is_inline_and_empty() {
    let v = SmallVec::<i32, 4>::new();
    assert!(v.is_inline());
    assert!(v.is_empty());
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), 4);
}

#[test]
fn with_capacity_stays_inline_when_small() {
    let v = SmallVec::<i32, 8>::with_capacity(4);
    assert!(v.is_inline());
}

#[test]
fn with_capacity_spills_when_large() {
    let v = SmallVec::<i32, 4>::with_capacity(100);
    assert!(!v.is_inline());
    assert!(v.capacity() >= 100);
}

// ── Push / Pop ──────────────────────────────────────────────────────────

#[test]
fn push_stays_inline() {
    let mut v = SmallVec::<i32, 4>::new();
    for i in 0..4 {
        v.push(i);
        assert!(v.is_inline());
    }
    assert_eq!(v.as_slice(), &[0, 1, 2, 3]);
}

#[test]
fn push_spills_on_overflow() {
    let mut v = SmallVec::<i32, 2>::new();
    v.push(1);
    v.push(2);
    assert!(v.is_inline());
    v.push(3);
    assert!(!v.is_inline());
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn pop_returns_elements() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(10);
    v.push(20);
    assert_eq!(v.pop(), Some(20));
    assert_eq!(v.pop(), Some(10));
    assert_eq!(v.pop(), None);
}

#[test]
fn pop_after_spill() {
    let mut v = SmallVec::<i32, 2>::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert!(!v.is_inline());
    assert_eq!(v.pop(), Some(3));
    assert_eq!(v.pop(), Some(2));
    assert_eq!(v.pop(), Some(1));
    assert_eq!(v.pop(), None);
}

// ── Insert / Remove ─────────────────────────────────────────────────────

#[test]
fn insert_at_beginning() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(2);
    v.push(3);
    v.insert(0, 1);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn insert_at_end() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(1);
    v.push(2);
    v.insert(2, 3);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn insert_in_middle() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(1);
    v.push(3);
    v.insert(1, 2);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn insert_triggers_spill() {
    let mut v = SmallVec::<i32, 2>::new();
    v.push(1);
    v.push(3);
    v.insert(1, 2); // should spill
    assert!(!v.is_inline());
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn remove_from_middle() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert_eq!(v.remove(1), 2);
    assert_eq!(v.as_slice(), &[1, 3]);
}

#[test]
fn swap_remove_inline() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert_eq!(v.swap_remove(0), 1);
    assert_eq!(v.as_slice(), &[3, 2]);
}

// ── Clear / Truncate ────────────────────────────────────────────────────

#[test]
fn clear_inline() {
    let mut v = SmallVec::<String, 4>::new();
    v.push("a".into());
    v.push("b".into());
    v.clear();
    assert!(v.is_empty());
    assert!(v.is_inline());
}

#[test]
fn clear_heap() {
    let mut v = SmallVec::<String, 2>::new();
    v.push("a".into());
    v.push("b".into());
    v.push("c".into());
    v.clear();
    assert!(v.is_empty());
}

#[test]
fn truncate_shortens() {
    let mut v = SmallVec::<i32, 4>::new();
    v.extend([1, 2, 3, 4]);
    v.truncate(2);
    assert_eq!(v.as_slice(), &[1, 2]);
}

#[test]
fn truncate_noop_when_longer() {
    let mut v = SmallVec::<i32, 4>::new();
    v.push(1);
    v.truncate(10);
    assert_eq!(v.len(), 1);
}

// ── Retain ──────────────────────────────────────────────────────────────

#[test]
fn retain_inline() {
    let mut v = SmallVec::<i32, 8>::new();
    v.extend(0..6);
    v.retain(|x| x % 2 == 0);
    assert_eq!(v.as_slice(), &[0, 2, 4]);
}

#[test]
fn retain_heap() {
    let mut v = SmallVec::<i32, 2>::new();
    v.extend(0..10);
    v.retain(|x| *x >= 5);
    assert_eq!(v.as_slice(), &[5, 6, 7, 8, 9]);
}

// ── Clone ───────────────────────────────────────────────────────────────

#[test]
fn clone_inline() {
    let mut v = SmallVec::<String, 4>::new();
    v.push("hello".into());
    v.push("world".into());
    let v2 = v.clone();
    assert_eq!(v, v2);
    assert!(v2.is_inline());
}

#[test]
fn clone_heap() {
    let mut v = SmallVec::<String, 2>::new();
    v.push("a".into());
    v.push("b".into());
    v.push("c".into());
    let v2 = v.clone();
    assert_eq!(v, v2);
    assert!(!v2.is_inline());
}

// ── Comparison ──────────────────────────────────────────────────────────

#[test]
fn eq_inline_vs_heap() {
    let mut inline = SmallVec::<i32, 8>::new();
    inline.extend([1, 2, 3]);
    let mut heap = SmallVec::<i32, 2>::new();
    heap.extend([1, 2, 3]);
    // Different N, can't compare directly — compare slices.
    assert_eq!(inline.as_slice(), heap.as_slice());
}

#[test]
fn eq_with_slice() {
    let mut v = SmallVec::<i32, 4>::new();
    v.extend([1, 2, 3]);
    assert_eq!(v, [1, 2, 3]);
}

#[test]
fn ord() {
    let a: SmallVec<i32, 4> = [1, 2].into_iter().collect();
    let b: SmallVec<i32, 4> = [1, 3].into_iter().collect();
    assert!(a < b);
}

// ── From conversions ────────────────────────────────────────────────────

#[test]
fn from_vec_inline() {
    let v = vec![1, 2, 3];
    let sv = SmallVec::<i32, 8>::from(v);
    assert!(sv.is_inline());
    assert_eq!(sv.as_slice(), &[1, 2, 3]);
}

#[test]
fn from_vec_heap() {
    let v = vec![1, 2, 3, 4, 5];
    let sv = SmallVec::<i32, 2>::from(v);
    assert!(!sv.is_inline());
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn from_array_inline() {
    let sv = SmallVec::<i32, 4>::from([1, 2, 3]);
    assert!(sv.is_inline());
    assert_eq!(sv.as_slice(), &[1, 2, 3]);
}

#[test]
fn from_array_spills_when_too_big() {
    let sv = SmallVec::<i32, 2>::from([1, 2, 3, 4]);
    assert!(!sv.is_inline());
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn from_slice() {
    let sv = SmallVec::<i32, 4>::from(&[10, 20, 30][..]);
    assert_eq!(sv.as_slice(), &[10, 20, 30]);
}

#[test]
fn into_vec() {
    let mut sv = SmallVec::<i32, 4>::new();
    sv.extend([1, 2, 3]);
    let v: Vec<i32> = sv.into();
    assert_eq!(v, vec![1, 2, 3]);
}

// ── Iteration ───────────────────────────────────────────────────────────

#[test]
fn into_iter_inline() {
    let mut sv = SmallVec::<i32, 4>::new();
    sv.extend([1, 2, 3]);
    let collected: Vec<_> = sv.into_iter().collect();
    assert_eq!(collected, vec![1, 2, 3]);
}

#[test]
fn into_iter_heap() {
    let mut sv = SmallVec::<i32, 2>::new();
    sv.extend([1, 2, 3, 4]);
    let collected: Vec<_> = sv.into_iter().collect();
    assert_eq!(collected, vec![1, 2, 3, 4]);
}

#[test]
fn into_iter_rev() {
    let sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let collected: Vec<_> = sv.into_iter().rev().collect();
    assert_eq!(collected, vec![3, 2, 1]);
}

#[test]
fn into_iter_exact_size() {
    let sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let iter = sv.into_iter();
    assert_eq!(iter.len(), 3);
}

#[test]
fn iter_ref() {
    let sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let sum: i32 = sv.iter().sum();
    assert_eq!(sum, 6);
}

#[test]
fn iter_mut() {
    let mut sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    for x in sv.iter_mut() {
        *x *= 2;
    }
    assert_eq!(sv.as_slice(), &[2, 4, 6]);
}

#[test]
fn from_iterator() {
    let sv: SmallVec<i32, 8> = (0..5).collect();
    assert!(sv.is_inline());
    assert_eq!(sv.as_slice(), &[0, 1, 2, 3, 4]);
}

#[test]
fn from_iterator_spills() {
    let sv: SmallVec<i32, 2> = (0..10).collect();
    assert!(!sv.is_inline());
    assert_eq!(sv.len(), 10);
}

#[test]
fn extend_from_slice() {
    let mut sv = SmallVec::<i32, 8>::new();
    sv.extend_from_slice(&[1, 2, 3]);
    sv.extend_from_slice(&[4, 5, 6]);
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4, 5, 6]);
}

// ── Drain ───────────────────────────────────────────────────────────────

#[test]
fn drain_all() {
    let mut sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let drained: Vec<_> = sv.drain(..).collect();
    assert_eq!(drained, vec![1, 2, 3]);
    assert!(sv.is_empty());
}

#[test]
fn drain_range() {
    let mut sv: SmallVec<i32, 8> = (0..5).collect();
    let drained: Vec<_> = sv.drain(1..3).collect();
    assert_eq!(drained, vec![1, 2]);
    assert_eq!(sv.as_slice(), &[0, 3, 4]);
}

#[test]
fn drain_front() {
    let mut sv: SmallVec<i32, 4> = [10, 20, 30].into_iter().collect();
    let drained: Vec<_> = sv.drain(0..2).collect();
    assert_eq!(drained, vec![10, 20]);
    assert_eq!(sv.as_slice(), &[30]);
}

// ── Deref ───────────────────────────────────────────────────────────────

#[test]
fn deref_to_slice() {
    let sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let slice: &[i32] = &sv;
    assert_eq!(slice, &[1, 2, 3]);
}

#[test]
fn deref_mut_to_slice() {
    let mut sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let slice: &mut [i32] = &mut sv;
    slice[0] = 99;
    assert_eq!(sv[0], 99);
}

// ── Index ───────────────────────────────────────────────────────────────

#[test]
fn index_access() {
    let sv: SmallVec<i32, 4> = [10, 20, 30].into_iter().collect();
    assert_eq!(sv[0], 10);
    assert_eq!(sv[2], 30);
}

#[test]
fn index_mut_access() {
    let mut sv: SmallVec<i32, 4> = [10, 20, 30].into_iter().collect();
    sv[1] = 99;
    assert_eq!(sv[1], 99);
}

// ── Drop correctness ────────────────────────────────────────────────────

#[test]
fn drop_inline_strings() {
    // This test primarily checks that we don't leak or double-free.
    let mut sv = SmallVec::<String, 4>::new();
    sv.push("hello".into());
    sv.push("world".into());
    drop(sv);
}

#[test]
fn drop_heap_strings() {
    let mut sv = SmallVec::<String, 2>::new();
    sv.push("a".into());
    sv.push("b".into());
    sv.push("c".into());
    drop(sv);
}

#[test]
fn drop_after_partial_into_iter() {
    let mut sv = SmallVec::<String, 4>::new();
    sv.push("a".into());
    sv.push("b".into());
    sv.push("c".into());
    let mut iter = sv.into_iter();
    let _ = iter.next(); // consume one
    drop(iter); // remaining should be dropped
}

#[test]
fn retain_panic_in_predicate_does_not_double_drop() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone)]
    struct D(#[allow(dead_code)] u32);
    impl Drop for D {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    DROPS.store(0, Ordering::SeqCst);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut sv = SmallVec::<D, 2>::new(); // N=2, 5 elements → heap
        for id in 0..5 {
            sv.push(D(id));
        }
        // Remove id 0 first (this back-shifts the tail and leaves a bitwise
        // duplicate of the last element in the vacated slot), then panic on
        // id 2. On unwind, Drop must free each of the 5 elements exactly once.
        sv.retain(|d| {
            if d.0 == 2 {
                panic!("boom");
            }
            d.0 != 0
        });
    }));

    assert!(result.is_err(), "predicate was expected to panic");
    // Exactly the 5 constructed elements, dropped once each. Before the
    // panic-safety fix the shifted-out tail duplicate was double-dropped (6).
    assert_eq!(DROPS.load(Ordering::SeqCst), 5, "retain double-dropped on unwind");
}

// ── Reserve ─────────────────────────────────────────────────────────────

#[test]
fn reserve_stays_inline() {
    let mut sv = SmallVec::<i32, 8>::new();
    sv.push(1);
    sv.reserve(3);
    assert!(sv.is_inline());
}

#[test]
fn reserve_triggers_spill() {
    let mut sv = SmallVec::<i32, 4>::new();
    sv.push(1);
    sv.reserve(100);
    assert!(!sv.is_inline());
    assert!(sv.capacity() >= 101);
}

// ── Debug ───────────────────────────────────────────────────────────────

#[test]
fn debug_format() {
    let sv: SmallVec<i32, 4> = [1, 2, 3].into_iter().collect();
    let s = format!("{:?}", sv);
    assert_eq!(s, "[1, 2, 3]");
}

// ── ZST support ─────────────────────────────────────────────────────

#[test]
fn zst_push_pop() {
    let mut v = SmallVec::<(), 4>::new();
    v.push(());
    v.push(());
    assert_eq!(v.len(), 2);
    assert_eq!(v.pop(), Some(()));
    assert_eq!(v.len(), 1);
}

// ── Referenced variant ──────────────────────────────────────────────

#[test]
fn from_ref_basic() {
    let data = [1, 2, 3, 4, 5];
    let sv = SmallVec::<i32, 8>::from_ref(&data);
    assert!(sv.is_referenced());
    assert_eq!(sv.len(), 5);
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn from_ref_empty() {
    let data: [i32; 0] = [];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    assert!(sv.is_referenced());
    assert!(sv.is_empty());
}

#[test]
fn from_ref_index() {
    let data = [10, 20, 30];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    assert_eq!(sv[0], 10);
    assert_eq!(sv[2], 30);
}

#[test]
fn from_ref_iter() {
    let data = [1, 2, 3];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    let sum: i32 = sv.iter().sum();
    assert_eq!(sum, 6);
}

#[test]
fn from_ref_clone_is_cheap() {
    let data = [1, 2, 3];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    let sv2 = sv.clone();
    assert!(sv2.is_referenced());
    assert_eq!(sv, sv2);
}

#[test]
fn from_ref_eq() {
    let data = [1, 2, 3];
    let sv_ref = SmallVec::<i32, 4>::from_ref(&data);
    let mut sv_owned: SmallVec<i32, 4> = SmallVec::new();
    sv_owned.extend([1, 2, 3]);
    assert_eq!(sv_ref, sv_owned);
}

#[test]
fn from_ref_into_owned_inline() {
    let data = [1, 2, 3];
    let sv = SmallVec::<i32, 8>::from_ref(&data);
    let owned = sv.into_owned();
    assert!(owned.is_inline());
    assert_eq!(owned.as_slice(), &[1, 2, 3]);
}

#[test]
fn from_ref_into_owned_heap() {
    let data = [1, 2, 3, 4, 5];
    let sv = SmallVec::<i32, 2>::from_ref(&data);
    let owned = sv.into_owned();
    assert!(!owned.is_inline());
    assert_eq!(owned.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn from_ref_push_materializes() {
    let data = [1, 2, 3];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    assert!(sv.is_referenced());
    sv.push(4);
    assert!(!sv.is_referenced());
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn from_ref_push_materializes_to_heap() {
    let data = [1, 2, 3];
    let mut sv = SmallVec::<i32, 2>::from_ref(&data);
    sv.push(4);
    assert!(!sv.is_referenced());
    assert!(!sv.is_inline());
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn from_ref_clear() {
    let data = [1, 2, 3];
    let mut sv = SmallVec::<i32, 4>::from_ref(&data);
    sv.clear();
    assert!(sv.is_empty());
    assert!(sv.is_inline()); // clear resets to inline
}

#[test]
fn from_ref_truncate() {
    let data = [1, 2, 3, 4, 5];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    sv.truncate(3);
    assert!(sv.is_referenced()); // truncate just shortens the view
    assert_eq!(sv.as_slice(), &[1, 2, 3]);
}

#[test]
fn from_ref_make_mut() {
    let data = [1, 2, 3];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    sv.make_mut();
    assert!(!sv.is_referenced());
    assert_eq!(sv.as_slice(), &[1, 2, 3]);
}

#[test]
fn from_ref_into_vec() {
    let data = [1, 2, 3];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    let v: Vec<i32> = sv.into_vec();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn from_ref_into_iter() {
    let data = [1, 2, 3];
    let sv = SmallVec::<i32, 4>::from_ref(&data);
    let collected: Vec<_> = sv.into_iter().collect();
    assert_eq!(collected, vec![1, 2, 3]);
}

#[test]
fn from_ref_retain_materializes() {
    let data = [1, 2, 3, 4, 5];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    sv.retain(|x| x % 2 == 0);
    assert!(!sv.is_referenced());
    assert_eq!(sv.as_slice(), &[2, 4]);
}

#[test]
fn from_ref_remove_materializes() {
    let data = [10, 20, 30];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    assert_eq!(sv.remove(1), 20);
    assert!(!sv.is_referenced());
    assert_eq!(sv.as_slice(), &[10, 30]);
}

#[test]
fn from_ref_swap_remove_materializes() {
    let data = [10, 20, 30];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    assert_eq!(sv.swap_remove(0), 10);
    assert_eq!(sv.as_slice(), &[30, 20]);
}

#[test]
fn from_ref_insert_materializes() {
    let data = [1, 3];
    let mut sv = SmallVec::<i32, 8>::from_ref(&data);
    sv.insert(1, 2);
    assert!(!sv.is_referenced());
    assert_eq!(sv.as_slice(), &[1, 2, 3]);
}

// ── Layout size ────────────────────────────────────────────────────────

/// Expected SmallVec size: tagged_len (usize) + union (max of inline array, pointer).
fn expected_size<T, const N: usize>() -> usize {
    use std::mem::{align_of, size_of};
    let union_payload = size_of::<T>().checked_mul(N).unwrap().max(size_of::<*const T>());
    let union_align = align_of::<T>().max(align_of::<*const T>());
    // repr(C): tagged_len at offset 0, then data aligned to union_align.
    let data_offset = size_of::<usize>().next_multiple_of(union_align);
    let raw = data_offset + union_payload;
    // Struct alignment = max(align of usize, union_align).
    let struct_align = align_of::<usize>().max(union_align);
    raw.next_multiple_of(struct_align)
}

#[test]
fn layout_size_u64_8() {
    assert_eq!(std::mem::size_of::<SmallVec<u64, 8>>(), expected_size::<u64, 8>());
}

#[test]
fn layout_size_u8_8() {
    assert_eq!(std::mem::size_of::<SmallVec<u8, 8>>(), expected_size::<u8, 8>());
}

#[test]
fn layout_size_u32_4() {
    assert_eq!(std::mem::size_of::<SmallVec<u32, 4>>(), expected_size::<u32, 4>());
}

// ── SmallVec Referenced variant — additional N=4 coverage ──────────────────
//
// The block above covers N=2 (data > N → forced heap) and N=8 (data ≤ N →
// inline materialisation). These tests pin N=4 with 3- and 5-element source
// slices so the Referenced variant has to traverse every storage transition
// (Referenced → Inline → Heap) as elements are pushed.

#[test]
fn smallvec_from_ref_correct() {
    // 5 elements > N=4: Referenced can't fit inline, so this exercises the
    // Referenced-with-long-data path.
    let data = [1u32, 2, 3, 4, 5];
    let sv: SmallVec<'_, u32, 4> = SmallVec::from_ref(&data);
    assert_eq!(sv.as_slice(), &data);
}

#[test]
fn smallvec_from_ref_push_materialises() {
    // Start Referenced with 3 elements (≤ N). First push materialises to
    // Inline (4 elements). Second push forces a spill (5 > N).
    let data = [1u32, 2, 3];
    let mut sv: SmallVec<'_, u32, 4> = SmallVec::from_ref(&data);
    sv.push(4);
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4]);
    sv.push(5);
    assert_eq!(sv.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn smallvec_from_ref_clone_shares_borrow() {
    // Clone of a Referenced variant with data > N stays Referenced and shares
    // the underlying borrow.
    let data = [1u32, 2, 3, 4, 5];
    let sv: SmallVec<'_, u32, 4> = SmallVec::from_ref(&data);
    let cloned = sv.clone();
    assert_eq!(cloned.as_slice(), sv.as_slice());
    assert_eq!(cloned.as_slice(), &data);
}
