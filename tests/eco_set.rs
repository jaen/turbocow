use turbocow::EcoSet;

#[test]
fn from_array() {
    let set = EcoSet::from([1, 2, 3, 2]);
    assert_eq!(set.len(), 3);
    assert!(set.contains(&1));
    assert!(set.contains(&3));
}

#[test]
fn new_is_empty() {
    let set: EcoSet<&str> = EcoSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn insert_and_contains() {
    let mut set = EcoSet::new();
    assert!(set.insert("a"));
    assert!(set.insert("b"));
    assert!(!set.insert("a")); // duplicate
    assert!(set.contains("a"));
    assert!(set.contains("b"));
    assert!(!set.contains("c"));
    assert_eq!(set.len(), 2);
}

#[test]
fn get() {
    let mut set = EcoSet::new();
    set.insert("hello");
    assert_eq!(set.get("hello"), Some(&"hello"));
    assert_eq!(set.get("world"), None);
}

#[test]
fn remove() {
    let mut set = EcoSet::new();
    set.insert("a");
    set.insert("b");
    assert!(set.remove("a"));
    assert!(!set.remove("a")); // already gone
    assert_eq!(set.len(), 1);
    assert!(!set.contains("a"));
    assert!(set.contains("b"));
}

#[test]
fn clear() {
    let mut set = EcoSet::new();
    set.insert(1);
    set.insert(2);
    set.clear();
    assert!(set.is_empty());
}

#[test]
fn clone_is_cheap_and_independent() {
    let mut set = EcoSet::new();
    set.insert("a");
    set.insert("b");

    let snapshot = set.clone();

    set.insert("c");

    assert_eq!(snapshot.len(), 2);
    assert!(!snapshot.contains("c"));
    assert_eq!(set.len(), 3);
    assert!(set.contains("c"));
}

#[test]
fn retain() {
    let mut set: EcoSet<i32> = (0..6).collect();
    set.retain(|&v| v >= 3);
    assert_eq!(set.len(), 3);
    assert!(!set.contains(&0));
    assert!(set.contains(&3));
    assert!(set.contains(&5));
}

#[test]
fn from_iterator() {
    let set: EcoSet<&str> = vec!["a", "b", "c", "a"].into_iter().collect();
    assert_eq!(set.len(), 3);
    assert!(set.contains("a"));
}

#[test]
fn extend() {
    let mut set = EcoSet::new();
    set.insert("a");
    set.extend(vec!["b", "c"]);
    assert_eq!(set.len(), 3);
}

#[test]
fn iter() {
    let mut set = EcoSet::new();
    set.insert(1);
    set.insert(2);
    set.insert(3);
    let mut vals: Vec<_> = set.iter().copied().collect();
    vals.sort();
    assert_eq!(vals, vec![1, 2, 3]);
}

#[test]
fn into_iter() {
    let mut set = EcoSet::new();
    set.insert(10);
    set.insert(20);
    let mut vals: Vec<_> = set.into_iter().collect();
    vals.sort();
    assert_eq!(vals, vec![10, 20]);
}

#[test]
fn partial_eq() {
    let a: EcoSet<i32> = vec![1, 2, 3].into_iter().collect();
    let b: EcoSet<i32> = vec![3, 2, 1].into_iter().collect();
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different() {
    let a: EcoSet<i32> = vec![1, 2].into_iter().collect();
    let b: EcoSet<i32> = vec![1, 3].into_iter().collect();
    assert_ne!(a, b);
}

#[test]
fn is_subset_superset() {
    let a: EcoSet<i32> = vec![1, 2].into_iter().collect();
    let b: EcoSet<i32> = vec![1, 2, 3].into_iter().collect();
    assert!(a.is_subset(&b));
    assert!(!b.is_subset(&a));
    assert!(b.is_superset(&a));
}

#[test]
fn is_disjoint() {
    let a: EcoSet<i32> = vec![1, 2].into_iter().collect();
    let b: EcoSet<i32> = vec![3, 4].into_iter().collect();
    let c: EcoSet<i32> = vec![2, 3].into_iter().collect();
    assert!(a.is_disjoint(&b));
    assert!(!a.is_disjoint(&c));
}

#[test]
fn debug_format() {
    let mut set = EcoSet::new();
    set.insert(42);
    let s = format!("{:?}", set);
    assert!(s.contains("42"));
}

#[test]
fn default() {
    let set: EcoSet<String> = EcoSet::default();
    assert!(set.is_empty());
}

#[test]
fn with_capacity() {
    let set: EcoSet<i32> = EcoSet::with_capacity(100);
    assert!(set.is_empty());
}

// ── Set-algebra tests ───────────────────────────────────────────────────

#[test]
fn intersection_overlapping() {
    let a: EcoSet<i32> = [1, 2, 3].into();
    let b: EcoSet<i32> = [2, 3, 4].into();
    let mut got: Vec<_> = a.intersection(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![2, 3]);
}

#[test]
fn intersection_disjoint() {
    let a: EcoSet<i32> = [1, 2].into();
    let b: EcoSet<i32> = [3, 4].into();
    assert_eq!(a.intersection(&b).count(), 0);
}

#[test]
fn intersection_empty() {
    let a: EcoSet<i32> = EcoSet::new();
    let b: EcoSet<i32> = [1, 2].into();
    assert_eq!(a.intersection(&b).count(), 0);
}

#[test]
fn union_overlapping() {
    let a: EcoSet<i32> = [1, 2, 3].into();
    let b: EcoSet<i32> = [2, 3, 4].into();
    let mut got: Vec<_> = a.union(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 2, 3, 4]);
}

#[test]
fn union_disjoint() {
    let a: EcoSet<i32> = [1, 2].into();
    let b: EcoSet<i32> = [3, 4].into();
    let mut got: Vec<_> = a.union(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 2, 3, 4]);
}

#[test]
fn union_empty() {
    let a: EcoSet<i32> = EcoSet::new();
    let b: EcoSet<i32> = [1, 2].into();
    let mut got: Vec<_> = a.union(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 2]);
}

#[test]
fn difference_overlapping() {
    let a: EcoSet<i32> = [1, 2, 3].into();
    let b: EcoSet<i32> = [2, 3, 4].into();
    let mut got: Vec<_> = a.difference(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1]);
}

#[test]
fn difference_disjoint() {
    let a: EcoSet<i32> = [1, 2].into();
    let b: EcoSet<i32> = [3, 4].into();
    let mut got: Vec<_> = a.difference(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 2]);
}

#[test]
fn difference_empty_self() {
    let a: EcoSet<i32> = EcoSet::new();
    let b: EcoSet<i32> = [1, 2].into();
    assert_eq!(a.difference(&b).count(), 0);
}

#[test]
fn symmetric_difference_overlapping() {
    let a: EcoSet<i32> = [1, 2, 3].into();
    let b: EcoSet<i32> = [2, 3, 4].into();
    let mut got: Vec<_> = a.symmetric_difference(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 4]);
}

#[test]
fn symmetric_difference_disjoint() {
    let a: EcoSet<i32> = [1, 2].into();
    let b: EcoSet<i32> = [3, 4].into();
    let mut got: Vec<_> = a.symmetric_difference(&b).copied().collect();
    got.sort();
    assert_eq!(got, vec![1, 2, 3, 4]);
}

#[test]
fn symmetric_difference_equal() {
    let a: EcoSet<i32> = [1, 2, 3].into();
    let b: EcoSet<i32> = [1, 2, 3].into();
    assert_eq!(a.symmetric_difference(&b).count(), 0);
}

// ── Capacity tests ─────────────────────────────────────────────────────

#[test]
fn capacity_is_at_least_len() {
    let mut set = EcoSet::new();
    set.insert(1i32);
    set.insert(2i32);
    assert!(set.capacity() >= set.len());
}

#[test]
fn reserve_grows_capacity() {
    let mut set: EcoSet<i32> = EcoSet::new();
    set.reserve(50);
    assert!(set.capacity() >= 50);
    assert_eq!(set.len(), 0);
}

#[test]
fn reserve_preserves_contents() {
    let mut set: EcoSet<i32> = [1, 2, 3].into();
    set.reserve(100);
    assert!(set.contains(&1));
    assert!(set.contains(&2));
    assert!(set.contains(&3));
    assert!(set.capacity() >= 100);
}

#[test]
fn shrink_to_fit_reduces_capacity() {
    let mut set: EcoSet<i32> = EcoSet::with_capacity(100);
    set.insert(1);
    set.shrink_to_fit();
    assert_eq!(set.len(), 1);
    assert!(set.contains(&1));
    assert!(set.capacity() < 50);
}

#[test]
fn shrink_to_fit_cow_isolation() {
    let mut set: EcoSet<i32> = EcoSet::with_capacity(100);
    set.insert(1);
    let snapshot = set.clone(); // shared
    let snap_cap = snapshot.capacity();
    set.shrink_to_fit(); // no-op on shared vecs
    assert_eq!(snapshot.capacity(), snap_cap);
    assert!(snapshot.contains(&1));
}

// ── EcoSet ≡ EcoMap<T, ()> equivalence ─────────────────────────────────────
//
// A set is semantically a map from T to (). Verify the two types agree on
// insertion, containment, and length for the same key sequence.

#[test]
fn ecoset_ecomap_zst_equivalence() {
    use turbocow::EcoMap;
    let mut set: EcoSet<u32> = EcoSet::new();
    let mut map: EcoMap<u32, ()> = EcoMap::new();
    for k in [1u32, 3, 5, 7] {
        set.insert(k);
        map.insert(k, ());
    }
    for k in [1u32, 3, 5, 7] {
        assert!(set.contains(&k));
        assert!(map.contains_key(&k));
    }
    assert_eq!(set.len(), map.len());
}
