use turbocow::SmallMap;

#[test]
fn new_is_inline_and_empty() {
    let map = SmallMap::<&str, i32>::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
    assert!(map.is_inline());
}

#[test]
fn insert_and_get() {
    let mut map = SmallMap::<&str, i32>::new();
    assert_eq!(map.insert("a", 1), None);
    assert_eq!(map.insert("b", 2), None);
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));
    assert_eq!(map.get("c"), None);
    assert_eq!(map.len(), 2);
}

#[test]
fn insert_replace() {
    let mut map = SmallMap::<&str, i32>::new();
    assert_eq!(map.insert("a", 1), None);
    assert_eq!(map.insert("a", 2), Some(1));
    assert_eq!(map.get("a"), Some(&2));
    assert_eq!(map.len(), 1);
}

#[test]
fn get_mut() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("x", 10);
    *map.get_mut("x").unwrap() = 20;
    assert_eq!(map["x"], 20);
}

#[test]
fn get_key_value() {
    let mut map = SmallMap::<String, i32>::new();
    map.insert("hello".to_string(), 42);
    let (k, v) = map.get_key_value("hello").unwrap();
    assert_eq!(k, "hello");
    assert_eq!(*v, 42);
}

#[test]
fn contains_key() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    assert!(map.contains_key("a"));
    assert!(!map.contains_key("b"));
}

#[test]
fn remove() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    assert_eq!(map.remove("a"), Some(1));
    assert_eq!(map.get("a"), None);
    assert_eq!(map.len(), 1);
    assert_eq!(map.remove("a"), None);
}

#[test]
fn remove_entry() {
    let mut map = SmallMap::<String, i32>::new();
    map.insert("a".to_string(), 1);
    let (k, v) = map.remove_entry("a").unwrap();
    assert_eq!(k, "a");
    assert_eq!(v, 1);
    assert!(map.is_empty());
}

#[test]
fn clear() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.clear();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn spill_to_heap() {
    // Use N=2 to force early spill.
    let mut map = SmallMap::<&str, i32, 2>::new();
    assert!(map.is_inline());

    map.insert("a", 1);
    map.insert("b", 2);
    assert!(map.is_inline());

    // This should spill.
    map.insert("c", 3);
    assert!(!map.is_inline());

    // All entries should still be accessible.
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));
    assert_eq!(map.get("c"), Some(&3));
    assert_eq!(map.len(), 3);
}

#[test]
fn spill_replace_does_not_spill() {
    let mut map = SmallMap::<&str, i32, 2>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    // Replacing existing key should NOT spill.
    map.insert("a", 10);
    assert!(map.is_inline());
    assert_eq!(map.get("a"), Some(&10));
}

#[test]
fn operations_after_spill() {
    let mut map = SmallMap::<&str, i32, 2>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3); // spill

    // insert/replace on heap
    assert_eq!(map.insert("a", 10), Some(1));
    assert_eq!(map.insert("d", 4), None);

    // remove on heap
    assert_eq!(map.remove("b"), Some(2));
    assert_eq!(map.len(), 3);

    // contains_key on heap
    assert!(map.contains_key("c"));
    assert!(!map.contains_key("b"));
}

#[test]
fn retain_inline() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3);
    map.retain(|_, v| *v > 1);
    assert_eq!(map.len(), 2);
    assert!(!map.contains_key("a"));
    assert!(map.contains_key("b"));
    assert!(map.contains_key("c"));
}

#[test]
fn retain_heap() {
    let mut map = SmallMap::<&str, i32, 2>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3); // spill
    map.retain(|_, v| *v <= 2);
    assert_eq!(map.len(), 2);
    assert!(map.contains_key("a"));
    assert!(map.contains_key("b"));
    assert!(!map.contains_key("c"));
}

#[test]
fn iter() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let mut entries: Vec<_> = map.iter().map(|(&k, &v)| (k, v)).collect();
    entries.sort();
    assert_eq!(entries, vec![("a", 1), ("b", 2)]);
}

#[test]
fn iter_mut() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    for (_, v) in map.iter_mut() {
        *v *= 10;
    }
    assert_eq!(map.get("a"), Some(&10));
    assert_eq!(map.get("b"), Some(&20));
}

#[test]
fn into_iter_inline() {
    let mut map = SmallMap::<String, i32>::new();
    map.insert("a".to_string(), 1);
    map.insert("b".to_string(), 2);
    let mut entries: Vec<_> = map.into_iter().collect();
    entries.sort();
    assert_eq!(entries, vec![("a".to_string(), 1), ("b".to_string(), 2)]);
}

#[test]
fn into_iter_heap() {
    let mut map = SmallMap::<String, i32, 2>::new();
    map.insert("a".to_string(), 1);
    map.insert("b".to_string(), 2);
    map.insert("c".to_string(), 3); // spill
    let mut entries: Vec<_> = map.into_iter().collect();
    entries.sort();
    assert_eq!(
        entries,
        vec![("a".to_string(), 1), ("b".to_string(), 2), ("c".to_string(), 3),]
    );
}

#[test]
fn keys_and_values() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let mut keys: Vec<_> = map.keys().copied().collect();
    keys.sort();
    assert_eq!(keys, vec!["a", "b"]);
    let mut values: Vec<_> = map.values().copied().collect();
    values.sort();
    assert_eq!(values, vec![1, 2]);
}

#[test]
fn values_mut() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    for v in map.values_mut() {
        *v += 100;
    }
    assert_eq!(map.get("a"), Some(&101));
    assert_eq!(map.get("b"), Some(&102));
}

#[test]
fn clone_inline() {
    let mut map = SmallMap::<String, i32>::new();
    map.insert("a".to_string(), 1);
    let clone = map.clone();
    assert_eq!(map, clone);
    assert!(clone.is_inline());
}

#[test]
fn clone_heap() {
    let mut map = SmallMap::<String, i32, 2>::new();
    map.insert("a".to_string(), 1);
    map.insert("b".to_string(), 2);
    map.insert("c".to_string(), 3); // spill
    let clone = map.clone();
    assert_eq!(map, clone);
    assert!(!clone.is_inline());
}

#[test]
fn partial_eq() {
    let mut a = SmallMap::<&str, i32>::new();
    a.insert("x", 1);
    a.insert("y", 2);
    let mut b = SmallMap::<&str, i32>::new();
    b.insert("y", 2);
    b.insert("x", 1);
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different_len() {
    let mut a = SmallMap::<&str, i32>::new();
    a.insert("x", 1);
    let mut b = SmallMap::<&str, i32>::new();
    b.insert("x", 1);
    b.insert("y", 2);
    assert_ne!(a, b);
}

#[test]
fn partial_eq_inline_vs_heap() {
    let mut inline = SmallMap::<&str, i32, 4>::new();
    inline.insert("a", 1);
    inline.insert("b", 2);
    assert!(inline.is_inline());

    let mut heap = SmallMap::<&str, i32, 1>::new();
    heap.insert("a", 1);
    heap.insert("b", 2);
    assert!(!heap.is_inline());

    // Can't directly compare different N, but verify both contain same data.
    assert_eq!(inline.get("a"), heap.get("a"));
    assert_eq!(inline.get("b"), heap.get("b"));
}

#[test]
fn debug_format() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    let s = format!("{:?}", map);
    assert!(s.contains("\"a\""));
    assert!(s.contains("1"));
}

#[test]
fn default_is_empty() {
    let map: SmallMap<String, String> = Default::default();
    assert!(map.is_empty());
    assert!(map.is_inline());
}

#[test]
fn index() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 42);
    assert_eq!(map[&"a"], 42);
}

#[test]
#[should_panic(expected = "no entry found for key")]
fn index_missing_key_panics() {
    let map = SmallMap::<&str, i32>::new();
    let _ = map[&"missing"];
}

#[test]
fn extend() {
    let mut map = SmallMap::<&str, i32>::new();
    map.extend([("a", 1), ("b", 2), ("c", 3)]);
    assert_eq!(map.len(), 3);
    assert_eq!(map.get("b"), Some(&2));
}

#[test]
fn from_iterator() {
    let map: SmallMap<&str, i32> = [("x", 10), ("y", 20)].into_iter().collect();
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("x"), Some(&10));
}

#[test]
fn with_capacity_small() {
    let map = SmallMap::<&str, i32>::with_capacity(4);
    assert!(map.is_inline());
    assert!(map.is_empty());
}

#[test]
fn with_capacity_large() {
    let map = SmallMap::<&str, i32>::with_capacity(100);
    assert!(!map.is_inline());
    assert!(map.is_empty());
}

#[test]
fn with_hasher() {
    use core::hash::BuildHasherDefault;
    use std::collections::hash_map::DefaultHasher;
    let map = SmallMap::<&str, i32, 8, BuildHasherDefault<DefaultHasher>>::with_hasher(
        BuildHasherDefault::default(),
    );
    assert!(map.is_inline());
    assert!(map.is_empty());
}

#[test]
fn drop_values_on_clear() {
    use std::sync::Arc;
    let a = Arc::new(1);
    let b = Arc::new(2);
    let mut map = SmallMap::<&str, Arc<i32>>::new();
    map.insert("a", a.clone());
    map.insert("b", b.clone());
    assert_eq!(Arc::strong_count(&a), 2);
    assert_eq!(Arc::strong_count(&b), 2);
    map.clear();
    assert_eq!(Arc::strong_count(&a), 1);
    assert_eq!(Arc::strong_count(&b), 1);
}

#[test]
fn drop_values_on_remove() {
    use std::sync::Arc;
    let a = Arc::new(1);
    let mut map = SmallMap::<&str, Arc<i32>>::new();
    map.insert("a", a.clone());
    assert_eq!(Arc::strong_count(&a), 2);
    map.remove("a");
    assert_eq!(Arc::strong_count(&a), 1);
}

#[test]
fn drop_values_on_spill() {
    use std::sync::Arc;
    let a = Arc::new(1);
    let b = Arc::new(2);
    let mut map = SmallMap::<&str, Arc<i32>, 2>::new();
    map.insert("a", a.clone());
    map.insert("b", b.clone());
    assert_eq!(Arc::strong_count(&a), 2);
    assert_eq!(Arc::strong_count(&b), 2);
    // Spill — entries move to heap, refcounts should stay at 2.
    map.insert("c", Arc::new(3));
    assert_eq!(Arc::strong_count(&a), 2);
    assert_eq!(Arc::strong_count(&b), 2);
    assert!(!map.is_inline());
}

#[test]
fn drop_values_on_map_drop() {
    use std::sync::Arc;
    let a = Arc::new(1);
    {
        let mut map = SmallMap::<&str, Arc<i32>>::new();
        map.insert("a", a.clone());
        assert_eq!(Arc::strong_count(&a), 2);
    }
    assert_eq!(Arc::strong_count(&a), 1);
}

#[test]
fn drop_values_on_map_drop_after_spill() {
    use std::sync::Arc;
    let a = Arc::new(1);
    {
        let mut map = SmallMap::<&str, Arc<i32>, 1>::new();
        map.insert("a", a.clone());
        map.insert("b", Arc::new(2)); // spill
        assert_eq!(Arc::strong_count(&a), 2);
    }
    assert_eq!(Arc::strong_count(&a), 1);
}

#[test]
fn into_iter_drops_remaining() {
    use std::sync::Arc;
    let a = Arc::new(1);
    let b = Arc::new(2);
    let mut map = SmallMap::<&str, Arc<i32>>::new();
    map.insert("a", a.clone());
    map.insert("b", b.clone());
    {
        let mut iter = map.into_iter();
        // Consume only one element.
        let _ = iter.next();
        // Drop the iterator — remaining element should be dropped.
    }
    // One of a or b should have been consumed, the other dropped.
    assert_eq!(Arc::strong_count(&a) + Arc::strong_count(&b), 2);
}

#[test]
fn exact_size_iterator() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3);

    let mut iter = map.iter();
    assert_eq!(iter.len(), 3);
    iter.next();
    assert_eq!(iter.len(), 2);
    iter.next();
    assert_eq!(iter.len(), 1);
    iter.next();
    assert_eq!(iter.len(), 0);
}

#[test]
fn string_keys() {
    let mut map = SmallMap::<String, String>::new();
    map.insert("hello".to_string(), "world".to_string());
    // Lookup with &str (Borrow<str> for String).
    assert_eq!(map.get("hello"), Some(&"world".to_string()));
    assert!(map.contains_key("hello"));
}

#[test]
fn integer_keys() {
    let mut map = SmallMap::<i32, &str>::new();
    for i in 0..8 {
        map.insert(i, "inline");
    }
    assert!(map.is_inline());
    assert_eq!(map.len(), 8);
    for i in 0..8 {
        assert_eq!(map.get(&i), Some(&"inline"));
    }
}

#[test]
fn fill_and_spill_boundary() {
    // Exactly fill inline (N=4), then one more to spill.
    let mut map = SmallMap::<i32, i32, 4>::new();
    for i in 0..4 {
        map.insert(i, i * 10);
    }
    assert!(map.is_inline());
    assert_eq!(map.len(), 4);

    map.insert(4, 40);
    assert!(!map.is_inline());
    assert_eq!(map.len(), 5);

    for i in 0..5 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 2 — h2 sidecar / adaptive threshold tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn h2_filtered_get_above_threshold() {
    // With > 3 entries the h2-filtered path is taken.
    let mut map = SmallMap::<String, i32>::new();
    for i in 0..8 {
        map.insert(format!("key_{i}"), i);
    }
    assert!(map.is_inline());
    assert_eq!(map.len(), 8);

    // All keys must be found.
    for i in 0..8 {
        assert_eq!(map.get(&format!("key_{i}") as &str), Some(&i));
    }
    // Missing key must return None.
    assert_eq!(map.get("missing"), None);
}

#[test]
fn h2_filtered_get_mut_above_threshold() {
    let mut map = SmallMap::<String, i32>::new();
    for i in 0..6 {
        map.insert(format!("k{i}"), i);
    }
    // Mutate through h2 path.
    for i in 0..6 {
        if let Some(v) = map.get_mut(&format!("k{i}") as &str) {
            *v += 100;
        }
    }
    for i in 0..6 {
        assert_eq!(map.get(&format!("k{i}") as &str), Some(&(i + 100)));
    }
}

#[test]
fn h2_filtered_remove_above_threshold() {
    let mut map = SmallMap::<i32, i32>::new();
    for i in 0..8 {
        map.insert(i, i * 10);
    }
    // Remove in non-sequential order to exercise swap-delete of h2 bytes.
    assert_eq!(map.remove(&3), Some(30));
    assert_eq!(map.remove(&0), Some(0));
    assert_eq!(map.remove(&7), Some(70));

    assert_eq!(map.len(), 5);
    assert!(map.is_inline());

    // Remaining entries must still be found.
    for &i in &[1, 2, 4, 5, 6] {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
    // Removed entries must be gone.
    for &i in &[0, 3, 7] {
        assert_eq!(map.get(&i), None);
    }
}

#[test]
fn h2_filtered_remove_entry_above_threshold() {
    let mut map = SmallMap::<String, i32>::new();
    for i in 0..6 {
        map.insert(format!("k{i}"), i);
    }
    let entry = map.remove_entry("k2");
    assert_eq!(entry, Some((String::from("k2"), 2)));
    assert_eq!(map.len(), 5);
    assert_eq!(map.get("k2"), None);

    // Others still present.
    for i in [0, 1, 3, 4, 5] {
        assert_eq!(map.get(&format!("k{i}") as &str), Some(&i));
    }
}

#[test]
fn h2_get_key_value_above_threshold() {
    let mut map = SmallMap::<String, i32>::new();
    for i in 0..5 {
        map.insert(format!("key_{i}"), i);
    }
    let (k, v) = map.get_key_value("key_3").unwrap();
    assert_eq!(k, "key_3");
    assert_eq!(*v, 3);
    assert_eq!(map.get_key_value("nope"), None);
}

#[test]
fn h2_insert_replace_above_threshold() {
    // Replace existing key via h2-filtered path.
    let mut map = SmallMap::<i32, i32>::new();
    for i in 0..8 {
        map.insert(i, i);
    }
    // Replace key 5.
    assert_eq!(map.insert(5, 999), Some(5));
    assert_eq!(map.get(&5), Some(&999));
    assert_eq!(map.len(), 8);
    assert!(map.is_inline());
}

#[test]
fn n16_simd_path() {
    // N=16 exercises the SSE2 code path on x86_64.
    let mut map = SmallMap::<i32, i32, 16>::new();
    for i in 0..16 {
        map.insert(i, i * 10);
    }
    assert!(map.is_inline());
    assert_eq!(map.len(), 16);

    for i in 0..16 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
    assert_eq!(map.get(&16), None);
    assert_eq!(map.get(&-1), None);

    // Remove and re-check.
    assert_eq!(map.remove(&8), Some(80));
    assert_eq!(map.remove(&0), Some(0));
    assert_eq!(map.len(), 14);

    for i in 1..16 {
        if i == 8 {
            assert_eq!(map.get(&i), None);
        } else {
            assert_eq!(map.get(&i), Some(&(i * 10)));
        }
    }
}

#[test]
fn n16_spill_and_lookup() {
    let mut map = SmallMap::<i32, i32, 16>::new();
    for i in 0..17 {
        map.insert(i, i);
    }
    assert!(!map.is_inline());
    for i in 0..17 {
        assert_eq!(map.get(&i), Some(&i));
    }
}

#[test]
fn retain_preserves_h2_integrity() {
    let mut map = SmallMap::<i32, i32>::new();
    for i in 0..8 {
        map.insert(i, i);
    }
    // Retain only even keys — this exercises h2 sidecar swap-delete.
    map.retain(|k, _| k % 2 == 0);
    assert_eq!(map.len(), 4);
    assert!(map.is_inline());

    for i in 0..8 {
        if i % 2 == 0 {
            assert_eq!(map.get(&i), Some(&i));
        } else {
            assert_eq!(map.get(&i), None);
        }
    }

    // Insert more to re-use h2 slots, ensure lookups still work.
    for i in 10..14 {
        map.insert(i, i);
    }
    assert_eq!(map.len(), 8);
    for i in 10..14 {
        assert_eq!(map.get(&i), Some(&i));
    }
}

#[test]
fn clone_preserves_h2_integrity() {
    let mut map = SmallMap::<i32, i32>::new();
    for i in 0..8 {
        map.insert(i, i * 10);
    }
    let cloned = map.clone();

    // Both original and clone use h2-filtered path (len > 3).
    for i in 0..8 {
        assert_eq!(cloned.get(&i), Some(&(i * 10)));
    }
    assert_eq!(cloned.get(&99), None);
}

#[test]
fn mixed_linear_and_h2_paths() {
    // Start below threshold (linear), grow past it (h2), shrink back.
    let mut map = SmallMap::<i32, i32>::new();

    // Linear path (len ≤ 3).
    map.insert(1, 10);
    map.insert(2, 20);
    map.insert(3, 30);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));

    // Cross into h2 path.
    map.insert(4, 40);
    map.insert(5, 50);
    assert_eq!(map.get(&4), Some(&40));
    assert_eq!(map.get(&5), Some(&50));

    // Shrink back below threshold.
    map.remove(&5);
    map.remove(&4);
    map.remove(&3);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry API
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn entry_or_insert_vacant() {
    let mut map = SmallMap::<&str, i32>::new();
    let v = map.entry("a").or_insert(42);
    assert_eq!(*v, 42);
    assert_eq!(map.get("a"), Some(&42));
}

#[test]
fn entry_or_insert_occupied() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    let v = map.entry("a").or_insert(99);
    assert_eq!(*v, 1); // not replaced
}

#[test]
// Deliberately exercises `or_insert_with`; clippy's `or_default()`
// rewrite would defeat the point of this test.
#[allow(clippy::unwrap_or_default)]
fn entry_or_insert_with() {
    let mut map = SmallMap::<&str, Vec<i32>>::new();
    map.entry("a").or_insert_with(Vec::new).push(1);
    map.entry("a").or_insert_with(Vec::new).push(2);
    assert_eq!(map.get("a"), Some(&vec![1, 2]));
}

#[test]
fn entry_or_insert_with_key() {
    let mut map = SmallMap::<String, usize>::new();
    map.entry("hello".into()).or_insert_with_key(|k| k.len());
    assert_eq!(map.get("hello"), Some(&5));
}

#[test]
fn entry_or_default() {
    let mut map = SmallMap::<&str, i32>::new();
    *map.entry("a").or_default() += 10;
    *map.entry("a").or_default() += 20;
    assert_eq!(map.get("a"), Some(&30));
}

#[test]
fn entry_and_modify() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.entry("a").and_modify(|v| *v += 10).or_insert(0);
    assert_eq!(map.get("a"), Some(&11));

    // Vacant case — and_modify is a no-op, or_insert runs.
    map.entry("b").and_modify(|v| *v += 10).or_insert(0);
    assert_eq!(map.get("b"), Some(&0));
}

#[test]
fn entry_key() {
    let mut map = SmallMap::<String, i32>::new();
    map.insert("existing".into(), 1);

    let entry = map.entry("existing".into());
    assert_eq!(entry.key(), "existing");

    let entry = map.entry("new_key".into());
    assert_eq!(entry.key(), "new_key");
}

#[test]
fn occupied_entry_get_and_get_mut() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);

    match map.entry("a") {
        Entry::Occupied(o) => {
            assert_eq!(*o.get(), 1);
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }

    match map.entry("a") {
        Entry::Occupied(mut o) => {
            *o.get_mut() = 42;
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }
    assert_eq!(map.get("a"), Some(&42));
}

#[test]
fn occupied_entry_insert() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);

    match map.entry("a") {
        Entry::Occupied(mut o) => {
            let old = o.insert(99);
            assert_eq!(old, 1);
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }
    assert_eq!(map.get("a"), Some(&99));
}

#[test]
fn occupied_entry_remove() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    map.insert("b", 2);

    match map.entry("a") {
        Entry::Occupied(o) => {
            let val = o.remove();
            assert_eq!(val, 1);
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("a"), None);
}

#[test]
fn occupied_entry_remove_entry() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<String, i32>::new();
    map.insert("x".into(), 42);

    match map.entry("x".into()) {
        Entry::Occupied(o) => {
            let (k, v) = o.remove_entry();
            assert_eq!(k, "x");
            assert_eq!(v, 42);
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }
    assert!(map.is_empty());
}

#[test]
fn occupied_entry_into_mut() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);

    match map.entry("a") {
        Entry::Occupied(o) => {
            let v = o.into_mut();
            *v = 100;
        }
        Entry::Vacant(_) => panic!("expected occupied"),
    }
    assert_eq!(map.get("a"), Some(&100));
}

#[test]
fn vacant_entry_insert() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<&str, i32>::new();

    match map.entry("a") {
        Entry::Occupied(_) => panic!("expected vacant"),
        Entry::Vacant(v) => {
            let val = v.insert(42);
            assert_eq!(*val, 42);
        }
    }
    assert_eq!(map.get("a"), Some(&42));
}

#[test]
fn vacant_entry_key_and_into_key() {
    use turbocow::small_map::Entry;

    let mut map = SmallMap::<String, i32>::new();
    match map.entry("hello".into()) {
        Entry::Vacant(v) => {
            assert_eq!(v.key(), "hello");
            let k = v.into_key();
            assert_eq!(k, "hello");
        }
        Entry::Occupied(_) => panic!("expected vacant"),
    }
}

#[test]
fn entry_spill_on_insert() {
    // Fill inline (N=4), then entry().or_insert triggers spill.
    let mut map = SmallMap::<i32, i32, 4>::new();
    for i in 0..4 {
        map.insert(i, i * 10);
    }
    assert!(map.is_inline());

    // This should spill to heap.
    let v = map.entry(4).or_insert(40);
    assert_eq!(*v, 40);
    assert!(!map.is_inline());
    assert_eq!(map.len(), 5);

    // All entries present.
    for i in 0..5 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
}

#[test]
fn entry_on_heap_variant() {
    let mut map = SmallMap::<i32, i32>::with_capacity(100);
    assert!(!map.is_inline());

    map.entry(1).or_insert(10);
    map.entry(2).or_insert(20);
    assert_eq!(map.get(&1), Some(&10));

    // Occupied entry on heap.
    *map.entry(1).or_insert(0) += 5;
    assert_eq!(map.get(&1), Some(&15));
}

#[test]
fn entry_counting_pattern() {
    // Classic word-count pattern.
    let words = ["hello", "world", "hello", "rust", "world", "hello"];
    let mut counts = SmallMap::<&str, usize>::new();
    for &w in &words {
        *counts.entry(w).or_insert(0) += 1;
    }
    assert_eq!(counts.get("hello"), Some(&3));
    assert_eq!(counts.get("world"), Some(&2));
    assert_eq!(counts.get("rust"), Some(&1));
}

#[test]
fn entry_with_many_keys_h2_path() {
    // With > 3 entries, entry() uses the h2-filtered path.
    let mut map = SmallMap::<String, i32>::new();
    for i in 0..8 {
        map.entry(format!("key_{i}")).or_insert(i);
    }
    assert_eq!(map.len(), 8);
    assert!(map.is_inline());

    // Re-entering existing keys should find them.
    for i in 0..8 {
        let v = map.entry(format!("key_{i}")).or_insert(-1);
        assert_eq!(*v, i);
    }
    assert_eq!(map.len(), 8); // no new entries
}

// ── Capacity tests ─────────────────────────────────────────────────────

#[test]
fn capacity_inline() {
    // Default N=8: inline capacity is exactly 8.
    let map = SmallMap::<&str, i32>::new();
    assert_eq!(map.capacity(), 8);
    assert!(map.is_inline());
}

#[test]
fn capacity_heap() {
    let map = SmallMap::<&str, i32>::with_capacity(100);
    assert!(map.capacity() >= 100);
    assert!(!map.is_inline());
}

#[test]
fn reserve_within_inline_capacity() {
    // Reserving <= N on an inline map stays inline.
    let mut map = SmallMap::<&str, i32>::new(); // N=8
    map.reserve(4);
    assert!(map.is_inline()); // still inline
    assert!(map.capacity() >= 4);
}

#[test]
fn reserve_forces_spill() {
    // Reserving > N on an inline map spills to heap.
    let mut map = SmallMap::<&str, i32, 2>::new();
    map.insert("a", 1);
    assert!(map.is_inline());
    map.reserve(10); // > N=2, must spill
    assert!(!map.is_inline());
    assert!(map.capacity() >= 10);
    assert_eq!(map.get("a"), Some(&1));
}

#[test]
fn reserve_on_heap() {
    let mut map = SmallMap::<&str, i32>::with_capacity(10);
    map.reserve(50);
    assert!(map.capacity() >= 50);
}

#[test]
fn reserve_preserves_contents() {
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("x", 42);
    map.reserve(100);
    assert_eq!(map.get("x"), Some(&42));
    assert!(map.capacity() >= 100);
}

#[test]
fn shrink_to_fit_heap() {
    let mut map = SmallMap::<&str, i32>::with_capacity(100);
    map.insert("a", 1);
    map.shrink_to_fit();
    assert_eq!(map.get("a"), Some(&1));
    // hashbrown may not shrink below its min bucket count, but
    // capacity should be much less than 100.
    assert!(map.capacity() < 50);
}

#[test]
fn shrink_to_fit_inline_noop() {
    // Inline maps have fixed capacity N; shrink_to_fit is a documented no-op.
    let mut map = SmallMap::<&str, i32>::new();
    map.insert("a", 1);
    assert!(map.is_inline());
    let cap_before = map.capacity();
    map.shrink_to_fit();
    assert_eq!(map.capacity(), cap_before); // unchanged
    assert_eq!(map.get("a"), Some(&1));
}

// ── SIMD OOB read regression (bug #1) ───────────────────────────────────────
//
// For N ∈ [17,31] on x86_64, the SSE2 second-load guard was `if len > 16`
// (runtime) instead of `if N >= 32` (compile-time). This caused
// _mm_loadu_si128 to read 16 bytes from offset 16 of an N-byte array,
// which is OOB when N < 32. The result was masked correctly but the load
// itself is UB. Only Miri catches this; regular hardware passes silently.
//
// Regression test for bug #1 (SmallMap SIMD out-of-bounds read).

#[cfg(miri)]
#[cfg(target_arch = "x86_64")]
#[test]
fn simd_oob_n17_to_31_under_miri() {
    // N=24, len=17 → match_h2_sse2 reads h2_bytes[16..32] from a 24-byte
    // array. The 8-byte excess is OOB and is flagged by Miri.
    let mut map: SmallMap<u64, u32, 24> = SmallMap::new();
    for k in 0u64..17 {
        map.insert(k, k as u32);
    }
    // Touch as many keys as possible to force match_h2 on every slot.
    for k in 0u64..17 {
        assert_eq!(map.get(&k), Some(&(k as u32)), "key {k} missing");
    }
}

#[cfg(miri)]
#[cfg(target_arch = "x86_64")]
#[test]
fn simd_n32_boundary_under_miri() {
    // N=32 is the maximum valid. match_h2_sse2 should perform the second
    // load safely (h2_bytes[16..32] is exactly in bounds).
    let mut map: SmallMap<u64, u32, 32> = SmallMap::new();
    for k in 0u64..32 {
        map.insert(k, k as u32);
    }
    for k in 0u64..32 {
        assert_eq!(map.get(&k), Some(&(k as u32)), "key {k} missing");
    }
}

// Regression: SmallMap must support N > 32. The old fixed u32 `BitMask` matcher
// scanned only slots [0, 32) (two SSE2 loads), so keys stored at an inline slot
// >= 32 were silently never found. The chunked matcher scans every slot.
#[test]
fn large_n_lookups_beyond_32_slots() {
    let mut map: SmallMap<u32, u32, 64> = SmallMap::new();
    for i in 0..50u32 {
        assert_eq!(map.insert(i, i * 2), None);
    }
    assert!(map.is_inline(), "50 < 64 must stay inline");
    assert_eq!(map.len(), 50);
    for i in 0..50u32 {
        assert_eq!(
            map.get(&i),
            Some(&(i * 2)),
            "key {i} (inline slot {i}) must be found"
        );
    }
    assert_eq!(map.get(&50), None);
    assert_eq!(map.insert(40, 999), Some(80), "overwrite a high-slot key");
    assert_eq!(map.get(&40), Some(&999));
    assert_eq!(map.len(), 50, "overwrite must not grow");
}

// N not a multiple of 16 exercises the chunked matcher's scalar tail (the
// final chunk where a 16-byte SIMD load would run past the N-byte array).
#[test]
fn large_n_non_multiple_of_16() {
    let mut map: SmallMap<u32, u32, 50> = SmallMap::new();
    for i in 0..50u32 {
        map.insert(i, i);
    }
    assert!(map.is_inline());
    for i in 0..50u32 {
        assert_eq!(map.get(&i), Some(&i), "key {i} missing");
    }
    assert_eq!(map.get(&100), None);
}

// Miri: large-N lookups must not read out of bounds at the 16 / 32 / 48 chunk
// edges or in the scalar tail.
#[cfg(miri)]
#[test]
fn large_n_no_oob_under_miri() {
    fn fill_and_check<const N: usize>(fill: u32) {
        let mut m: SmallMap<u32, u32, N> = SmallMap::new();
        for i in 0..fill {
            m.insert(i, i);
        }
        for i in 0..fill {
            assert_eq!(m.get(&i), Some(&i));
        }
        assert_eq!(m.get(&9999), None);
    }
    fill_and_check::<33>(33); // one past a chunk edge
    fill_and_check::<47>(40); // partial tail, partly filled
    fill_and_check::<48>(48); // exact 3-chunk multiple, full
    fill_and_check::<64>(60); // 4 chunks, partly filled
}

// ── SmallMap::force_spill hasher double-drop (bug #4) ───────────────────────
//
// force_spill bitwise-read the hasher out via ptr::read, but then dropped
// the old InlineMap, whose field-drop glue re-dropped the hasher. Default
// DefaultHashBuilder is Copy so the bug was hidden; non-Copy S triggers it.
//
// Regression test for bug #4 (force_spill hasher double-drop, non-Copy S).

#[test]
fn force_spill_hasher_dropped_once() {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Default)]
    struct CountingHasher(std::collections::hash_map::DefaultHasher);
    impl Hasher for CountingHasher {
        fn write(&mut self, bytes: &[u8]) {
            self.0.write(bytes)
        }
        fn finish(&self) -> u64 {
            self.0.finish()
        }
    }

    #[derive(Clone)]
    struct CountingBuilder;
    impl BuildHasher for CountingBuilder {
        type Hasher = CountingHasher;
        fn build_hasher(&self) -> Self::Hasher {
            CountingHasher(std::collections::hash_map::DefaultHasher::new())
        }
    }
    impl Drop for CountingBuilder {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    DROP_COUNT.store(0, Ordering::SeqCst);
    {
        let mut map: SmallMap<u64, u32, 4, CountingBuilder> =
            SmallMap::with_hasher(CountingBuilder);
        for k in 0u64..10 {
            map.insert(k, k as u32); // forces force_spill at the 5th insert
        }
        // map drops here: heap side drops the hasher it received; the bug
        // causes the inline side to ALSO drop the (moved-out) hasher.
    }
    let drops = DROP_COUNT.load(Ordering::SeqCst);
    assert_eq!(
        drops, 1,
        "CountingBuilder should be dropped exactly once; got {drops} (the bug gives 2)"
    );
}

// ── SmallMap::IntoIter hasher leak (bug #11) ────────────────────────────────
//
// IntoIter used mem::forget(inline) to skip Drop on the entries (correctly
// moved into the iterator), but forget also skipped the hasher's Drop → leak.
//
// Regression test for bug #11 (IntoIter leaks a non-Copy hasher).

#[test]
fn into_iter_drops_hasher() {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Default)]
    struct CountingHasher(std::collections::hash_map::DefaultHasher);
    impl Hasher for CountingHasher {
        fn write(&mut self, bytes: &[u8]) {
            self.0.write(bytes)
        }
        fn finish(&self) -> u64 {
            self.0.finish()
        }
    }

    #[derive(Clone)]
    struct CountingBuilder;
    impl BuildHasher for CountingBuilder {
        type Hasher = CountingHasher;
        fn build_hasher(&self) -> Self::Hasher {
            CountingHasher(std::collections::hash_map::DefaultHasher::new())
        }
    }
    impl Drop for CountingBuilder {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    DROP_COUNT.store(0, Ordering::SeqCst);
    {
        let mut map: SmallMap<u64, u32, 4, CountingBuilder> =
            SmallMap::with_hasher(CountingBuilder);
        for k in 0u64..3 {
            map.insert(k, k as u32);
        }
        // Consume the iterator — this is where bug #11 leaks the hasher.
        let collected: Vec<_> = map.into_iter().collect();
        assert_eq!(collected.len(), 3);
    }
    let drops = DROP_COUNT.load(Ordering::SeqCst);
    assert_eq!(
        drops, 1,
        "CountingBuilder should be dropped exactly once; got {drops} (the bug leaks it)"
    );
}

// ── ZST value support ──────────────────────────────────────────────────────
//
// `SmallMap<K, (), N>` must work like a set-with-ordered-storage.

#[test]
fn smallmap_zst_value() {
    let mut m: SmallMap<u32, (), 4> = SmallMap::new();
    m.insert(1, ());
    m.insert(2, ());
    assert_eq!(m.len(), 2);
    assert_eq!(m.get(&1), Some(&()));
    m.remove(&2);
    assert_eq!(m.len(), 1);
}
