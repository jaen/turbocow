use turbocow::SmallSet;

#[test]
fn new_is_inline_and_empty() {
    let set = SmallSet::<i32>::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
    assert!(set.is_inline());
}

#[test]
fn insert_and_contains() {
    let mut set = SmallSet::<&str>::new();
    assert!(set.insert("hello"));
    assert!(set.insert("world"));
    assert!(!set.insert("hello")); // duplicate
    assert!(set.contains("hello"));
    assert!(set.contains("world"));
    assert!(!set.contains("missing"));
    assert_eq!(set.len(), 2);
}

#[test]
fn get() {
    let mut set = SmallSet::<String>::new();
    set.insert("abc".into());
    assert_eq!(set.get("abc"), Some(&String::from("abc")));
    assert_eq!(set.get("xyz"), None);
}

#[test]
fn remove() {
    let mut set = SmallSet::<i32>::new();
    set.insert(1);
    set.insert(2);
    set.insert(3);
    assert!(set.remove(&2));
    assert!(!set.remove(&2)); // already gone
    assert_eq!(set.len(), 2);
    assert!(!set.contains(&2));
    assert!(set.contains(&1));
    assert!(set.contains(&3));
}

#[test]
fn take() {
    let mut set = SmallSet::<String>::new();
    set.insert("hello".into());
    let taken = set.take("hello");
    assert_eq!(taken, Some(String::from("hello")));
    assert!(set.is_empty());
    assert_eq!(set.take("hello"), None);
}

#[test]
fn clear() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..5 {
        set.insert(i);
    }
    assert_eq!(set.len(), 5);
    set.clear();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn retain() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..8 {
        set.insert(i);
    }
    set.retain(|&v| v % 2 == 0);
    assert_eq!(set.len(), 4);
    for i in 0..8 {
        assert_eq!(set.contains(&i), i % 2 == 0);
    }
}

#[test]
fn spill_to_heap() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..8 {
        set.insert(i);
    }
    assert!(set.is_inline());
    set.insert(8);
    assert!(!set.is_inline());
    for i in 0..9 {
        assert!(set.contains(&i));
    }
}

#[test]
fn with_capacity() {
    let set = SmallSet::<i32>::with_capacity(100);
    assert!(!set.is_inline());
    assert!(set.is_empty());
}

#[test]
fn iter() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..5 {
        set.insert(i);
    }
    let mut collected: Vec<i32> = set.iter().copied().collect();
    collected.sort();
    assert_eq!(collected, vec![0, 1, 2, 3, 4]);
}

#[test]
fn into_iter() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..5 {
        set.insert(i);
    }
    let mut collected: Vec<i32> = set.into_iter().collect();
    collected.sort();
    assert_eq!(collected, vec![0, 1, 2, 3, 4]);
}

#[test]
fn clone() {
    let mut set = SmallSet::<i32>::new();
    for i in 0..5 {
        set.insert(i);
    }
    let cloned = set.clone();
    assert_eq!(set, cloned);
}

#[test]
fn debug_format() {
    let mut set = SmallSet::<i32>::new();
    set.insert(1);
    let debug = format!("{:?}", set);
    assert!(debug.contains('1'));
}

#[test]
fn partial_eq() {
    let mut a = SmallSet::<i32>::new();
    let mut b = SmallSet::<i32>::new();
    a.insert(1);
    a.insert(2);
    b.insert(2);
    b.insert(1);
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different() {
    let mut a = SmallSet::<i32>::new();
    let mut b = SmallSet::<i32>::new();
    a.insert(1);
    b.insert(2);
    assert_ne!(a, b);
}

#[test]
fn default() {
    let set = SmallSet::<i32>::default();
    assert!(set.is_empty());
    assert!(set.is_inline());
}

#[test]
fn extend() {
    let mut set = SmallSet::<i32>::new();
    set.extend(0..5);
    assert_eq!(set.len(), 5);
    for i in 0..5 {
        assert!(set.contains(&i));
    }
}

#[test]
fn extend_ref() {
    let mut set = SmallSet::<i32>::new();
    let vals = [1, 2, 3];
    set.extend(&vals);
    assert_eq!(set.len(), 3);
}

#[test]
fn from_iterator() {
    let set: SmallSet<i32> = (0..5).collect();
    assert_eq!(set.len(), 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// Set operations
// ═══════════════════════════════════════════════════════════════════════════

fn make_set(vals: &[i32]) -> SmallSet<i32> {
    vals.iter().copied().collect()
}

#[test]
fn intersection() {
    let a = make_set(&[1, 2, 3, 4]);
    let b = make_set(&[3, 4, 5, 6]);
    let mut result: Vec<i32> = a.intersection(&b).copied().collect();
    result.sort();
    assert_eq!(result, vec![3, 4]);
}

#[test]
fn intersection_empty() {
    let a = make_set(&[1, 2]);
    let b = make_set(&[3, 4]);
    let result: Vec<i32> = a.intersection(&b).copied().collect();
    assert!(result.is_empty());
}

#[test]
fn union() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[3, 4, 5]);
    let mut result: Vec<i32> = a.union(&b).copied().collect();
    result.sort();
    assert_eq!(result, vec![1, 2, 3, 4, 5]);
}

#[test]
fn difference() {
    let a = make_set(&[1, 2, 3, 4]);
    let b = make_set(&[3, 4, 5, 6]);
    let mut result: Vec<i32> = a.difference(&b).copied().collect();
    result.sort();
    assert_eq!(result, vec![1, 2]);
}

#[test]
fn symmetric_difference() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[3, 4, 5]);
    let mut result: Vec<i32> = a.symmetric_difference(&b).copied().collect();
    result.sort();
    assert_eq!(result, vec![1, 2, 4, 5]);
}

#[test]
fn is_disjoint() {
    let a = make_set(&[1, 2]);
    let b = make_set(&[3, 4]);
    let c = make_set(&[2, 3]);
    assert!(a.is_disjoint(&b));
    assert!(!a.is_disjoint(&c));
}

#[test]
fn is_subset() {
    let a = make_set(&[1, 2]);
    let b = make_set(&[1, 2, 3]);
    assert!(a.is_subset(&b));
    assert!(!b.is_subset(&a));
}

#[test]
fn is_superset() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[1, 2]);
    assert!(a.is_superset(&b));
    assert!(!b.is_superset(&a));
}

// ═══════════════════════════════════════════════════════════════════════════
// Bitwise operators
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitand_intersection() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[2, 3, 4]);
    let mut result: Vec<i32> = (&a & &b).into_iter().collect();
    result.sort();
    assert_eq!(result, vec![2, 3]);
}

#[test]
fn bitor_union() {
    let a = make_set(&[1, 2]);
    let b = make_set(&[2, 3]);
    let mut result: Vec<i32> = (&a | &b).into_iter().collect();
    result.sort();
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn sub_difference() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[2, 3, 4]);
    let result: Vec<i32> = (&a - &b).into_iter().collect();
    assert_eq!(result, vec![1]);
}

#[test]
fn bitxor_symmetric_difference() {
    let a = make_set(&[1, 2, 3]);
    let b = make_set(&[2, 3, 4]);
    let mut result: Vec<i32> = (&a ^ &b).into_iter().collect();
    result.sort();
    assert_eq!(result, vec![1, 4]);
}

// ═══════════════════════════════════════════════════════════════════════════
// Edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn string_values() {
    let mut set = SmallSet::<String>::new();
    set.insert("hello".into());
    set.insert("world".into());
    assert!(set.contains("hello"));
    assert!(!set.contains("missing"));
}

#[test]
fn n4_small_capacity() {
    let mut set = SmallSet::<i32, 4>::new();
    for i in 0..4 {
        set.insert(i);
    }
    assert!(set.is_inline());
    set.insert(4);
    assert!(!set.is_inline());
    assert_eq!(set.len(), 5);
}

#[test]
fn drop_values() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Hash, PartialEq, Eq)]
    struct Tracked(i32);
    impl Drop for Tracked {
        fn drop(&mut self) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    COUNT.store(0, Ordering::Relaxed);
    {
        let mut set = SmallSet::<Tracked>::new();
        set.insert(Tracked(1));
        set.insert(Tracked(2));
        set.insert(Tracked(3));
    }
    assert_eq!(COUNT.load(Ordering::Relaxed), 3);
}

// ── Capacity tests ─────────────────────────────────────────────────────

#[test]
fn capacity_inline() {
    let set = SmallSet::<i32>::new(); // N=8
    assert_eq!(set.capacity(), 8);
    assert!(set.is_inline());
}

#[test]
fn capacity_heap() {
    let set = SmallSet::<i32>::with_capacity(100);
    assert!(set.capacity() >= 100);
    assert!(!set.is_inline());
}

#[test]
fn reserve_forces_spill() {
    let mut set = SmallSet::<i32, 2>::new();
    set.insert(1);
    assert!(set.is_inline());
    set.reserve(10); // > N=2
    assert!(!set.is_inline());
    assert!(set.capacity() >= 10);
    assert!(set.contains(&1));
}

#[test]
fn reserve_preserves_contents() {
    let mut set: SmallSet<i32> = SmallSet::new();
    set.insert(42);
    set.reserve(100);
    assert!(set.contains(&42));
    assert!(set.capacity() >= 100);
}

#[test]
fn shrink_to_fit_heap() {
    let mut set = SmallSet::<i32>::with_capacity(100);
    set.insert(1);
    set.shrink_to_fit();
    assert!(set.contains(&1));
    assert!(set.capacity() < 50);
}

#[test]
fn shrink_to_fit_inline_noop() {
    let mut set = SmallSet::<i32>::new();
    set.insert(1);
    assert!(set.is_inline());
    let cap = set.capacity();
    set.shrink_to_fit(); // no-op
    assert_eq!(set.capacity(), cap);
    assert!(set.contains(&1));
}
