use turbocow::EcoMap;
use turbocow::eco_map::Entry;

#[test]
fn new_is_empty() {
    let map: EcoMap<&str, i32> = EcoMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn insert_and_get() {
    let mut map = EcoMap::new();
    assert_eq!(map.insert("a", 1), None);
    assert_eq!(map.insert("b", 2), None);
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));
    assert_eq!(map.get("c"), None);
    assert_eq!(map.len(), 2);
}

#[test]
fn insert_overwrite() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    assert_eq!(map.insert("a", 10), Some(1));
    assert_eq!(map.get("a"), Some(&10));
    assert_eq!(map.len(), 1);
}

#[test]
fn contains_key() {
    let mut map = EcoMap::new();
    map.insert("x", 42);
    assert!(map.contains_key("x"));
    assert!(!map.contains_key("y"));
}

#[test]
fn get_key_value() {
    let mut map = EcoMap::new();
    map.insert("hello", 5);
    let (k, v) = map.get_key_value("hello").unwrap();
    assert_eq!(*k, "hello");
    assert_eq!(*v, 5);
}

#[test]
fn get_mut() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    *map.get_mut("a").unwrap() = 99;
    assert_eq!(map.get("a"), Some(&99));
}

#[test]
fn remove() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3);
    assert_eq!(map.remove("b"), Some(2));
    assert_eq!(map.len(), 2);
    assert!(map.contains_key("a"));
    assert!(!map.contains_key("b"));
    assert!(map.contains_key("c"));
}

#[test]
fn remove_nonexistent() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    assert_eq!(map.remove("z"), None);
    assert_eq!(map.len(), 1);
}

#[test]
fn clear() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.clear();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn clone_is_cheap_and_independent() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);

    let snapshot = map.clone();

    // Mutate original — should not affect clone
    map.insert("c", 3);
    *map.get_mut("a").unwrap() = 100;

    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot.get("a"), Some(&1));
    assert_eq!(snapshot.get("c"), None);

    assert_eq!(map.len(), 3);
    assert_eq!(map.get("a"), Some(&100));
}

#[test]
fn retain() {
    let mut map = EcoMap::new();
    for i in 0..6 {
        map.insert(i, i * 10);
    }
    map.retain(|_k, v| *v >= 30);
    assert_eq!(map.len(), 3);
    assert!(!map.contains_key(&0));
    assert!(!map.contains_key(&1));
    assert!(!map.contains_key(&2));
    assert!(map.contains_key(&3));
    assert!(map.contains_key(&4));
    assert!(map.contains_key(&5));
}

#[test]
fn from_iterator() {
    let map: EcoMap<&str, i32> = vec![("a", 1), ("b", 2), ("c", 3)].into_iter().collect();
    assert_eq!(map.len(), 3);
    assert_eq!(map.get("b"), Some(&2));
}

#[test]
fn extend() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.extend(vec![("b", 2), ("c", 3)]);
    assert_eq!(map.len(), 3);
}

#[test]
fn iter() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let mut pairs: Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
    pairs.sort();
    assert_eq!(pairs, vec![("a", 1), ("b", 2)]);
}

#[test]
fn iter_mut() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    for (_k, v) in map.iter_mut() {
        *v *= 10;
    }
    assert_eq!(map.get("a"), Some(&10));
    assert_eq!(map.get("b"), Some(&20));
}

#[test]
fn into_iter() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let mut pairs: Vec<_> = map.into_iter().collect();
    pairs.sort();
    assert_eq!(pairs, vec![("a", 1), ("b", 2)]);
}

#[test]
fn keys_and_values() {
    let mut map = EcoMap::new();
    map.insert("x", 10);
    map.insert("y", 20);
    let mut keys: Vec<_> = map.keys().copied().collect();
    keys.sort();
    assert_eq!(keys, vec!["x", "y"]);
    let mut vals: Vec<_> = map.values().copied().collect();
    vals.sort();
    assert_eq!(vals, vec![10, 20]);
}

#[test]
fn values_mut() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    for v in map.values_mut() {
        *v += 100;
    }
    assert_eq!(map.get("a"), Some(&101));
}

#[test]
fn index() {
    let mut map = EcoMap::new();
    map.insert("key", 42);
    assert_eq!(map["key"], 42);
}

#[test]
#[should_panic(expected = "no entry found for key")]
fn index_missing_panics() {
    let map: EcoMap<&str, i32> = EcoMap::new();
    let _ = map["missing"];
}

#[test]
fn partial_eq() {
    let mut a = EcoMap::new();
    a.insert("x", 1);
    a.insert("y", 2);

    let mut b = EcoMap::new();
    b.insert("y", 2);
    b.insert("x", 1);

    assert_eq!(a, b);
}

#[test]
fn partial_eq_different() {
    let mut a = EcoMap::new();
    a.insert("x", 1);

    let mut b = EcoMap::new();
    b.insert("x", 2);

    assert_ne!(a, b);
}

#[test]
fn debug_format() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    let s = format!("{:?}", map);
    assert!(s.contains("\"a\""));
    assert!(s.contains("1"));
}

#[test]
fn default() {
    let map: EcoMap<String, String> = EcoMap::default();
    assert!(map.is_empty());
}

#[test]
fn with_capacity() {
    let map: EcoMap<i32, i32> = EcoMap::with_capacity(100);
    assert!(map.is_empty());
}

#[test]
fn cow_independence_after_get_mut() {
    let mut original = EcoMap::new();
    original.insert("a", vec![1, 2, 3]);

    let clone = original.clone();

    // Mutate via get_mut — triggers COW
    original.get_mut("a").unwrap().push(4);

    assert_eq!(clone.get("a").unwrap(), &vec![1, 2, 3]);
    assert_eq!(original.get("a").unwrap(), &vec![1, 2, 3, 4]);
}

#[test]
fn cow_independence_after_iter_mut() {
    let mut original = EcoMap::new();
    original.insert("a", 1);
    original.insert("b", 2);

    let clone = original.clone();

    for (_k, v) in original.iter_mut() {
        *v *= 10;
    }

    assert_eq!(clone.get("a"), Some(&1));
    assert_eq!(original.get("a"), Some(&10));
}

// ── Entry API ───────────────────────────────────────────────────────────

#[test]
fn entry_or_insert() {
    let mut map = EcoMap::new();
    map.entry("a").or_insert(1);
    map.entry("a").or_insert(999);
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.len(), 1);
}

#[test]
fn entry_or_insert_with() {
    let mut map = EcoMap::new();
    map.entry("x").or_insert_with(|| 42);
    assert_eq!(map["x"], 42);
}

#[test]
fn entry_or_insert_with_key() {
    let mut map: EcoMap<String, usize> = EcoMap::new();
    map.entry("hello".to_string()).or_insert_with_key(|k| k.len());
    assert_eq!(map.get("hello"), Some(&5));
}

#[test]
fn entry_or_default() {
    let mut map: EcoMap<&str, Vec<i32>> = EcoMap::new();
    map.entry("list").or_default().push(1);
    map.entry("list").or_default().push(2);
    assert_eq!(map["list"], vec![1, 2]);
}

#[test]
fn entry_and_modify() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.entry("a").and_modify(|v| *v += 10).or_insert(0);
    map.entry("b").and_modify(|v| *v += 10).or_insert(0);
    assert_eq!(map["a"], 11);
    assert_eq!(map["b"], 0);
}

#[test]
fn entry_occupied_key_get_insert_remove() {
    let mut map = EcoMap::new();
    map.insert("k", 1);
    match map.entry("k") {
        Entry::Occupied(mut o) => {
            assert_eq!(o.key(), &"k");
            assert_eq!(o.get(), &1);
            assert_eq!(o.insert(2), 1);
            assert_eq!(o.get(), &2);
            assert_eq!(o.remove(), 2);
        }
        _ => panic!("expected occupied"),
    }
    assert!(map.is_empty());
}

#[test]
fn entry_vacant_key_into_key_insert() {
    let mut map: EcoMap<&str, i32> = EcoMap::new();
    match map.entry("new") {
        Entry::Vacant(v) => {
            assert_eq!(v.key(), &"new");
            let r = v.insert(99);
            assert_eq!(*r, 99);
        }
        _ => panic!("expected vacant"),
    }
    assert_eq!(map["new"], 99);
}

// ── From array ──────────────────────────────────────────────────────────

#[test]
fn from_array() {
    let map = EcoMap::from([("a", 1), ("b", 2), ("c", 3)]);
    assert_eq!(map.len(), 3);
    assert_eq!(map["b"], 2);
}

// ── into_keys / into_values ─────────────────────────────────────────────

#[test]
fn into_keys() {
    let map = EcoMap::from([("x", 10), ("y", 20)]);
    let mut keys: Vec<_> = map.into_keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["x", "y"]);
}

#[test]
fn into_values() {
    let map = EcoMap::from([("x", 10), ("y", 20)]);
    let mut vals: Vec<_> = map.into_values().collect();
    vals.sort();
    assert_eq!(vals, vec![10, 20]);
}

// ── drain ───────────────────────────────────────────────────────────────

#[test]
fn drain() {
    let mut map = EcoMap::from([("a", 1), ("b", 2), ("c", 3)]);
    let mut drained: Vec<_> = map.drain().collect();
    drained.sort();
    assert_eq!(drained, vec![("a", 1), ("b", 2), ("c", 3)]);
    assert!(map.is_empty());
}

#[test]
fn drain_then_reuse() {
    let mut map = EcoMap::from([("a", 1)]);
    let _ = map.drain().count();
    map.insert("b", 2);
    assert_eq!(map.len(), 1);
    assert_eq!(map["b"], 2);
}

// ── Capacity tests ─────────────────────────────────────────────────────

#[test]
fn capacity_is_at_least_len() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    assert!(map.capacity() >= map.len());
}

#[test]
fn reserve_grows_capacity() {
    let mut map: EcoMap<&str, i32> = EcoMap::new();
    map.reserve(50);
    assert!(map.capacity() >= 50);
    assert_eq!(map.len(), 0);
}

#[test]
fn reserve_preserves_contents() {
    let mut map = EcoMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.reserve(100);
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));
    assert!(map.capacity() >= 100);
}

#[test]
fn shrink_to_fit_reduces_capacity() {
    let mut map: EcoMap<&str, i32> = EcoMap::with_capacity(100);
    map.insert("a", 1);
    map.shrink_to_fit();
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("a"), Some(&1));
    assert!(map.capacity() < 50);
}

#[test]
fn shrink_to_fit_cow_isolation() {
    let mut map: EcoMap<&str, i32> = EcoMap::with_capacity(100);
    map.insert("a", 1);
    let snapshot = map.clone(); // shared
    let snapshot_cap = snapshot.capacity();
    map.shrink_to_fit(); // no-op on shared vecs
    assert_eq!(snapshot.capacity(), snapshot_cap);
    assert_eq!(snapshot.get("a"), Some(&1));
    assert_eq!(map.get("a"), Some(&1));
}

// ── insert_unique / extend_unique (bulk-build fast path) ────────────────

#[test]
fn insert_unique_builds_correct_map() {
    let mut map: EcoMap<u32, u32> = EcoMap::new();
    for i in 0..50u32 {
        map.insert_unique(i, i * 10);
    }
    assert_eq!(map.len(), 50);
    for i in 0..50u32 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
    assert_eq!(map.get(&999), None);
}

#[test]
fn extend_unique_matches_dedup_build() {
    let mut map: EcoMap<u32, u32> = EcoMap::new();
    map.extend_unique((0..100u32).map(|i| (i, i + 1)));
    assert_eq!(map.len(), 100);
    assert_eq!(map.get(&0), Some(&1));
    assert_eq!(map.get(&99), Some(&100));
    // For distinct keys the unique path must agree with the dedup-aware build.
    // (`assert!(==)` rather than `assert_eq!` — EcoMap is PartialEq but not Debug.)
    let via_collect: EcoMap<u32, u32> = (0..100u32).map(|i| (i, i + 1)).collect();
    assert!(map == via_collect, "unique-built map must equal dedup-built map");
}

#[test]
fn extend_unique_appends_to_existing() {
    let mut map: EcoMap<u32, u32> = EcoMap::new();
    map.insert(0, 0);
    map.extend_unique((1..10u32).map(|i| (i, i)));
    assert_eq!(map.len(), 10);
    for i in 0..10u32 {
        assert_eq!(map.get(&i), Some(&i));
    }
}

// ── EcoMap::retain panic-safety (bug #6) ────────────────────────────────────
//
// retain used a local `len` counter and only called truncate at the end of
// the loop. If the predicate panicked mid-loop, removed keys remained
// visible. Fixed version uses symmetric pop + tuple drop per removal.
//
// Regression test for bug #6 (EcoMap::retain panicking predicate).

#[test]
fn retain_panicking_predicate_no_spurious_keys() {
    use std::panic::AssertUnwindSafe;
    use turbocow::EcoMap;

    let mut map: EcoMap<u32, u32> = EcoMap::new();
    map.insert(10, 100);
    map.insert(20, 200);
    map.insert(30, 300);
    map.insert(40, 400);
    map.insert(50, 500);

    // Predicate: keep 10, remove 20 (returns false), then panic.
    let mut calls = 0u32;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        map.retain(|_k, _v| {
            calls += 1;
            match calls {
                1 => true,                               // keep 10
                2 => false,                              // remove 20
                _ => panic!("boom in retain predicate"), // panic next
            }
        })
    }));
    assert!(result.is_err(), "expected retain to propagate the panic");

    // After unwind, key 20 (for which f returned false) must be gone.
    // The bug: truncate never ran, so 20 was still in the vec tail.
    assert!(
        !map.contains_key(&20),
        "key 20 should be removed; keys = {:?}",
        map.iter().map(|(k, _)| *k).collect::<Vec<_>>()
    );
    assert_eq!(
        map.len(),
        4,
        "map should have 4 keys (10, 30, 40, 50); got {} keys: {:?}",
        map.len(),
        map.iter().map(|(k, _)| *k).collect::<Vec<_>>()
    );
}

// `remove` COWs `keys` and swaps it, THEN COWs `vals` and swaps it. If the map
// is shared and `V::clone` panics during the `vals` copy-on-write, `keys` is
// already swapped but `vals` is not, so the surviving key→value pairing is
// corrupted after the caught unwind (no length desync, no UB — a logic bug).
// Both vecs must be COW'd before either swap (as `retain` does).
#[test]
fn remove_panicking_val_clone_during_cow_keeps_pairing() {
    use std::cell::Cell;
    use std::panic::AssertUnwindSafe;
    use turbocow::EcoMap;

    thread_local! {
        static CLONES: Cell<u32> = const { Cell::new(0) };
    }

    #[derive(PartialEq, Debug)]
    struct PanicClone(u32);
    impl Clone for PanicClone {
        fn clone(&self) -> Self {
            let n = CLONES.with(|c| {
                let n = c.get();
                c.set(n + 1);
                n
            });
            if n >= 1 {
                panic!("boom on the 2nd value clone");
            }
            PanicClone(self.0)
        }
    }

    let mut a: EcoMap<u32, PanicClone> = EcoMap::new();
    a.insert(0, PanicClone(100));
    a.insert(1, PanicClone(111));
    a.insert(2, PanicClone(222));
    let _b = a.clone(); // O(1) share → refcount 2 → make_mut now COWs
    CLONES.with(|c| c.set(0));

    let r = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // keys COW + swap(0, 2) succeeds; the vals COW panics on the 2nd clone.
        a.remove(&0);
    }));
    assert!(r.is_err(), "expected the value clone panic to propagate");

    // The map must remain correctly paired: key 2 → 222 (not 100).
    assert_eq!(a.get(&2), Some(&PanicClone(222)), "key 2 must still map to 222");
    assert_eq!(a.get(&0), Some(&PanicClone(100)), "key 0 must still map to 100");
    assert_eq!(a.get(&1), Some(&PanicClone(111)), "key 1 must still map to 111");
}

// ── OccupiedEntry::insert needless clone (bug #13) ──────────────────────────
//
// OccupiedEntry::insert cloned the old value before overwriting, while the
// top-level EcoMap::insert used mem::replace. Inconsistent and silently slower.
//
// Regression test for bug #13 (OccupiedEntry::insert needless clone).

#[test]
fn occupied_entry_insert_no_extra_clone() {
    use std::cell::Cell;
    use std::rc::Rc;
    use turbocow::EcoMap;

    struct CloneCounter {
        id: u32,
        counter: Rc<Cell<u32>>,
    }
    impl CloneCounter {
        fn new(id: u32, counter: &Rc<Cell<u32>>) -> Self {
            Self { id, counter: counter.clone() }
        }
    }
    impl Clone for CloneCounter {
        fn clone(&self) -> Self {
            self.counter.set(self.counter.get() + 1);
            Self { id: self.id, counter: self.counter.clone() }
        }
    }
    impl PartialEq for CloneCounter {
        fn eq(&self, o: &Self) -> bool {
            self.id == o.id
        }
    }
    impl Eq for CloneCounter {}
    impl std::hash::Hash for CloneCounter {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state)
        }
    }

    let counter = Rc::new(Cell::new(0u32));
    let mut map: EcoMap<u32, CloneCounter> = EcoMap::new();
    map.insert(1, CloneCounter::new(100, &counter));

    // Replace via entry API. The old value (id=100) should be moved out,
    // not cloned. clone count should stay at 0.
    let old = match map.entry(1) {
        Entry::Occupied(mut o) => o.insert(CloneCounter::new(200, &counter)),
        _ => panic!("expected occupied entry for key 1"),
    };
    assert_eq!(old.id, 100, "should return the old value");
    assert_eq!(
        counter.get(),
        0,
        "no clones should have happened during OccupiedEntry::insert; got {}",
        counter.get()
    );
}

// ── ZST value support ──────────────────────────────────────────────────────
//
// `EcoMap<K, ()>` mirrors the `EcoSet<K>` use case (set-as-map) and must
// work without special-casing: () is Clone, Default, and zero-sized.

#[test]
fn ecomap_zst_value() {
    let mut m: EcoMap<u32, ()> = EcoMap::new();
    m.insert(1, ());
    m.insert(2, ());
    m.insert(3, ());
    assert_eq!(m.len(), 3);
    assert_eq!(m.get(&1), Some(&()));
    assert!(m.contains_key(&2));
    m.remove(&2);
    assert_eq!(m.len(), 2);
    let keys: Vec<u32> = m.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys.len(), 2);
}
