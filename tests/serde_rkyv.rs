//! Integration tests for serde and rkyv support on SmallMap, SmallSet, EcoMap, EcoSet,
//! EcoVec, EcoString, and EcoBytes.
//!
//! Run with: `cargo test --features "serde,rkyv" --test serde_rkyv`

// Only used by the `serde`/`rkyv`-gated test modules below.
#[cfg(any(feature = "serde", feature = "rkyv"))]
use turbocow::{EcoBytes, EcoMap, EcoSet, EcoString, EcoVec, SmallMap, SmallSet};

// ═══════════════════════════════════════════════════════════════════════════
// serde — SmallMap
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;

    // JSON has no byte type: bytes serialize as a number array, which the
    // deserializer must accept back (it used to reject sequences).
    #[test]
    fn eco_bytes_json_roundtrip() {
        for b in [
            EcoBytes::new(),
            EcoBytes::from(b"ab"),
            EcoBytes::from(&[0xffu8; 40]),
            EcoBytes::from_static(b"borrowed"),
        ] {
            let json = serde_json::to_string(&b).unwrap();
            let back: EcoBytes = serde_json::from_str(&json).unwrap();
            assert_eq!(back, b);
        }
        assert_eq!(serde_json::to_string(&EcoBytes::from(b"ab")).unwrap(), "[97,98]");
    }

    #[test]
    fn eco_bytes_deserialize_from_str() {
        let b: EcoBytes = serde_json::from_str("\"ab\"").unwrap();
        assert_eq!(b, b"ab");
    }

    #[test]
    fn map_empty() {
        let map = SmallMap::<String, i32>::new();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(json, "{}");
        let back: SmallMap<String, i32> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn map_inline() {
        let mut map = SmallMap::<String, i32>::new();
        map.insert("a".into(), 1);
        map.insert("b".into(), 2);
        assert!(map.is_inline());

        let json = serde_json::to_string(&map).unwrap();
        let back: SmallMap<String, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.get("a"), Some(&1));
        assert_eq!(back.get("b"), Some(&2));
    }

    #[test]
    fn map_heap() {
        let mut map = SmallMap::<i32, String, 2>::new();
        for i in 0..10 {
            map.insert(i, format!("val_{i}"));
        }
        assert!(!map.is_inline());

        let json = serde_json::to_string(&map).unwrap();
        let back: SmallMap<i32, String, 2> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 10);
        for i in 0..10 {
            assert_eq!(back.get(&i), Some(&format!("val_{i}")));
        }
    }

    #[test]
    fn map_roundtrip_preserves_values() {
        let mut map = SmallMap::<&str, Vec<i32>>::new();
        map.insert("nums", vec![1, 2, 3]);
        map.insert("empty", vec![]);

        let json = serde_json::to_string(&map).unwrap();
        let back: SmallMap<String, Vec<i32>> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get("nums"), Some(&vec![1, 2, 3]));
        assert_eq!(back.get("empty"), Some(&vec![]));
    }

    #[test]
    fn set_empty() {
        let set = SmallSet::<String>::new();
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(json, "[]");
        let back: SmallSet<String> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn set_inline() {
        let mut set = SmallSet::<i32>::new();
        set.insert(1);
        set.insert(2);
        set.insert(3);
        assert!(set.is_inline());

        let json = serde_json::to_string(&set).unwrap();
        let back: SmallSet<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 3);
        assert!(back.contains(&1));
        assert!(back.contains(&2));
        assert!(back.contains(&3));
    }

    #[test]
    fn set_heap() {
        let mut set = SmallSet::<i32, 2>::new();
        for i in 0..10 {
            set.insert(i);
        }
        assert!(!set.is_inline());

        let json = serde_json::to_string(&set).unwrap();
        let back: SmallSet<i32, 2> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 10);
        for i in 0..10 {
            assert!(back.contains(&i));
        }
    }

    // ── EcoMap serde ────────────────────────────────────────────────────

    #[test]
    fn eco_map_empty() {
        let map = EcoMap::<String, i32>::new();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(json, "{}");
        let back: EcoMap<String, i32> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn eco_map_roundtrip() {
        let mut map = EcoMap::new();
        map.insert("x".to_string(), 10);
        map.insert("y".to_string(), 20);

        let json = serde_json::to_string(&map).unwrap();
        let back: EcoMap<String, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.get("x"), Some(&10));
        assert_eq!(back.get("y"), Some(&20));
    }

    // ── EcoVec serde ────────────────────────────────────────────────────

    #[test]
    fn eco_vec_empty() {
        let vec = EcoVec::<i32>::new();
        let json = serde_json::to_string(&vec).unwrap();
        assert_eq!(json, "[]");
        let back: EcoVec<i32> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn eco_vec_roundtrip() {
        let mut vec = EcoVec::<i32>::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let json = serde_json::to_string(&vec).unwrap();
        let back: EcoVec<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back.as_slice(), &[1, 2, 3]);
    }

    // ── EcoSet serde ────────────────────────────────────────────────────

    #[test]
    fn eco_set_empty() {
        let set = EcoSet::<String>::new();
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(json, "[]");
        let back: EcoSet<String> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn eco_set_roundtrip() {
        let mut set = EcoSet::new();
        set.insert(1);
        set.insert(2);
        set.insert(3);

        let json = serde_json::to_string(&set).unwrap();
        let back: EcoSet<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 3);
        assert!(back.contains(&1));
        assert!(back.contains(&2));
        assert!(back.contains(&3));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// rkyv — SmallMap + SmallSet
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "rkyv")]
mod rkyv_tests {
    use super::*;
    use rkyv::api::high::{from_bytes, to_bytes_in};
    use rkyv::rancor::Error;
    use rkyv::util::AlignedVec;

    /// Helper: serialize → validate → deserialize back.
    fn roundtrip_map<const N: usize>(
        map: &SmallMap<i32, i32, N>,
    ) -> SmallMap<i32, i32, N> {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(map, AlignedVec::new()).unwrap();
        from_bytes::<SmallMap<i32, i32, N>, Error>(&bytes).unwrap()
    }

    fn roundtrip_set<const N: usize>(set: &SmallSet<i32, N>) -> SmallSet<i32, N> {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(set, AlignedVec::new()).unwrap();
        from_bytes::<SmallSet<i32, N>, Error>(&bytes).unwrap()
    }

    #[test]
    fn map_inline_roundtrip() {
        let mut map = SmallMap::<i32, i32>::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);
        assert!(map.is_inline());

        let back = roundtrip_map(&map);
        assert_eq!(back.len(), 3);
        assert_eq!(back.get(&1), Some(&10));
        assert_eq!(back.get(&2), Some(&20));
        assert_eq!(back.get(&3), Some(&30));
    }

    #[test]
    fn map_heap_roundtrip() {
        let mut map = SmallMap::<i32, i32, 2>::new();
        for i in 0..10 {
            map.insert(i, i * 100);
        }
        assert!(!map.is_inline());

        let back = roundtrip_map(&map);
        assert_eq!(back.len(), 10);
        for i in 0..10 {
            assert_eq!(back.get(&i), Some(&(i * 100)));
        }
    }

    #[test]
    fn map_empty_roundtrip() {
        let map = SmallMap::<i32, i32>::new();
        let back = roundtrip_map(&map);
        assert!(back.is_empty());
    }

    #[test]
    fn set_inline_roundtrip() {
        let mut set = SmallSet::<i32>::new();
        set.insert(10);
        set.insert(20);
        set.insert(30);
        assert!(set.is_inline());

        let back = roundtrip_set(&set);
        assert_eq!(back.len(), 3);
        assert!(back.contains(&10));
        assert!(back.contains(&20));
        assert!(back.contains(&30));
    }

    #[test]
    fn set_heap_roundtrip() {
        let mut set = SmallSet::<i32, 2>::new();
        for i in 0..10 {
            set.insert(i);
        }
        assert!(!set.is_inline());

        let back = roundtrip_set(&set);
        assert_eq!(back.len(), 10);
        for i in 0..10 {
            assert!(back.contains(&i));
        }
    }

    #[test]
    fn set_empty_roundtrip() {
        let set = SmallSet::<i32>::new();
        let back = roundtrip_set(&set);
        assert!(back.is_empty());
    }

    // ── EcoMap rkyv ─────────────────────────────────────────────────────

    fn roundtrip_eco_map(map: &EcoMap<i32, i32>) -> EcoMap<i32, i32> {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(map, AlignedVec::new()).unwrap();
        from_bytes::<EcoMap<i32, i32>, Error>(&bytes).unwrap()
    }

    fn roundtrip_eco_set(set: &EcoSet<i32>) -> EcoSet<i32> {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(set, AlignedVec::new()).unwrap();
        from_bytes::<EcoSet<i32>, Error>(&bytes).unwrap()
    }

    #[test]
    fn eco_map_roundtrip() {
        let mut map = EcoMap::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);

        let back = roundtrip_eco_map(&map);
        assert_eq!(back.len(), 3);
        assert_eq!(back.get(&1), Some(&10));
        assert_eq!(back.get(&2), Some(&20));
        assert_eq!(back.get(&3), Some(&30));
    }

    #[test]
    fn eco_map_empty_roundtrip() {
        let map = EcoMap::<i32, i32>::new();
        let back = roundtrip_eco_map(&map);
        assert!(back.is_empty());
    }

    // ── EcoSet rkyv ─────────────────────────────────────────────────────

    #[test]
    fn eco_set_roundtrip() {
        let mut set = EcoSet::new();
        set.insert(10);
        set.insert(20);
        set.insert(30);

        let back = roundtrip_eco_set(&set);
        assert_eq!(back.len(), 3);
        assert!(back.contains(&10));
        assert!(back.contains(&20));
        assert!(back.contains(&30));
    }

    #[test]
    fn eco_set_empty_roundtrip() {
        let set = EcoSet::<i32>::new();
        let back = roundtrip_eco_set(&set);
        assert!(back.is_empty());
    }

    // ── EcoVec rkyv ─────────────────────────────────────────────────────

    fn roundtrip_eco_vec(vec: &EcoVec<i32>) -> EcoVec<i32> {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(vec, AlignedVec::new()).unwrap();
        from_bytes::<EcoVec<i32>, Error>(&bytes).unwrap()
    }

    #[test]
    fn eco_vec_rkyv_empty_roundtrip() {
        let vec = EcoVec::<i32>::new();
        let back = roundtrip_eco_vec(&vec);
        assert!(back.is_empty());
    }

    #[test]
    fn eco_vec_rkyv_roundtrip() {
        let mut vec = EcoVec::<i32>::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let back = roundtrip_eco_vec(&vec);
        assert_eq!(back.len(), 3);
        assert_eq!(back.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn eco_vec_rkyv_len_accessible_before_deserialize() {
        // Check that the archived form carries the correct length.
        let mut vec = EcoVec::<i32>::new();
        for i in 0..8 {
            vec.push(i * 10);
        }
        let bytes: AlignedVec = to_bytes_in::<_, Error>(&vec, AlignedVec::new()).unwrap();
        let back = from_bytes::<EcoVec<i32>, Error>(&bytes).unwrap();
        assert_eq!(back.len(), 8);
        for i in 0..8 {
            assert_eq!(back.as_slice()[i], (i as i32) * 10);
        }
    }

    // ── EcoString rkyv ──────────────────────────────────────────────────

    fn roundtrip_eco_string(s: &EcoString) -> EcoString {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(s, AlignedVec::new()).unwrap();
        from_bytes::<EcoString, Error>(&bytes).unwrap()
    }

    #[test]
    fn eco_string_rkyv_empty_roundtrip() {
        let s = EcoString::new();
        let back = roundtrip_eco_string(&s);
        assert!(back.is_empty());
    }

    #[test]
    fn eco_string_rkyv_inline_roundtrip() {
        // Fits inline in rkyv's ArchivedString (≤ 14 bytes).
        let s = EcoString::from("hello, world!");
        let back = roundtrip_eco_string(&s);
        assert_eq!(back.as_str(), "hello, world!");
    }

    #[test]
    fn eco_string_rkyv_heap_roundtrip() {
        // Longer than inline capacity — forces out-of-line storage in ArchivedString.
        let s = EcoString::from("this string is definitely longer than fourteen bytes");
        let back = roundtrip_eco_string(&s);
        assert_eq!(back.as_str(), "this string is definitely longer than fourteen bytes");
    }

    // ── EcoBytes rkyv ──────────────────────────────────────────────

    fn roundtrip_eco_bytes(b: &EcoBytes) -> EcoBytes {
        let bytes: AlignedVec = to_bytes_in::<_, Error>(b, AlignedVec::new()).unwrap();
        from_bytes::<EcoBytes, Error>(&bytes).unwrap()
    }

    #[test]
    fn eco_bytes_rkyv_empty_roundtrip() {
        let b = EcoBytes::new();
        let back = roundtrip_eco_bytes(&b);
        assert!(back.is_empty());
    }

    #[test]
    fn eco_bytes_rkyv_roundtrip() {
        let b = EcoBytes::from(&b"hello bytes"[..]);
        let back = roundtrip_eco_bytes(&b);
        assert_eq!(back.as_slice(), b"hello bytes");
    }

    #[test]
    fn eco_bytes_rkyv_non_utf8_roundtrip() {
        // Arbitrary non-UTF-8 content round-trips correctly.
        let b = EcoBytes::from(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0xff][..]);
        let back = roundtrip_eco_bytes(&b);
        assert_eq!(back.as_slice(), &[0xde, 0xad, 0xbe, 0xef, 0x00, 0xff]);
    }
}
