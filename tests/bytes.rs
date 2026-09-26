use std::borrow::Cow;

use turbocow::{EcoBytes, EcoString, EcoVec};

#[test]
fn test_new_empty() {
    let s = EcoBytes::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert_eq!(&*s, b"");
}

#[test]
fn test_from_slice_inline() {
    let s = EcoBytes::from(b"hello");
    assert_eq!(s.len(), 5);
    assert_eq!(&*s, b"hello");
}

#[test]
fn test_from_slice_spill() {
    let data = vec![0xffu8; 64];
    let s = EcoBytes::from(&data);
    assert_eq!(s.len(), 64);
    assert_eq!(&*s, &data[..]);
}

#[test]
fn test_inline_exact_limit() {
    let data = vec![0x42u8; EcoBytes::INLINE_LIMIT];
    let s = EcoBytes::inline(&data);
    assert_eq!(s.len(), EcoBytes::INLINE_LIMIT);
    assert_eq!(&*s, &data[..]);
}

#[test]
#[should_panic(expected = "exceeded inline capacity")]
fn test_inline_exceeds_limit() {
    let data = vec![0x42u8; EcoBytes::INLINE_LIMIT + 1];
    let _ = EcoBytes::inline(&data);
}

#[test]
fn test_from_static() {
    let s = EcoBytes::from_static(b"static bytes");
    assert_eq!(&*s, b"static bytes");
}

#[test]
fn test_push() {
    let mut s = EcoBytes::new();
    s.push(0x41);
    s.push(0x42);
    s.push(0x43);
    assert_eq!(&*s, b"ABC");
}

#[test]
fn test_push_spill() {
    let mut s = EcoBytes::from(&[0u8; 15]);
    assert_eq!(s.len(), 15);
    s.push(0xff);
    assert_eq!(s.len(), 16);
    assert_eq!(s[15], 0xff);
}

#[test]
fn test_extend_from_slice() {
    let mut s = EcoBytes::from(b"hello");
    s.extend_from_slice(b" world");
    assert_eq!(&*s, b"hello world");
}

#[test]
fn test_extend_from_slice_spill() {
    let mut s = EcoBytes::from(b"hello");
    s.extend_from_slice(&[0xffu8; 20]);
    assert_eq!(s.len(), 25);
    assert_eq!(&s[..5], b"hello");
    assert!(s[5..].iter().all(|&b| b == 0xff));
}

#[test]
fn test_pop() {
    let mut s = EcoBytes::from(b"abc");
    assert_eq!(s.pop(), Some(b'c'));
    assert_eq!(s.pop(), Some(b'b'));
    assert_eq!(s.pop(), Some(b'a'));
    assert_eq!(s.pop(), None);
}

#[test]
fn test_clear() {
    let mut s = EcoBytes::from(b"hello");
    s.clear();
    assert!(s.is_empty());
}

#[test]
fn test_truncate() {
    let mut s = EcoBytes::from(b"hello world");
    s.truncate(5);
    assert_eq!(&*s, b"hello");
}

#[test]
fn test_truncate_noop() {
    let mut s = EcoBytes::from(b"hello");
    s.truncate(100);
    assert_eq!(&*s, b"hello");
}

#[test]
fn test_make_mut() {
    let mut s = EcoBytes::from(b"hello");
    let slice = s.make_mut();
    slice[0] = b'H';
    assert_eq!(&*s, b"Hello");
}

#[test]
fn test_clone_on_write() {
    let s1 = EcoBytes::from(&[0xaau8; 32]);
    let mut s2 = s1.clone();
    s2.make_mut()[0] = 0xbb;
    assert_eq!(s1[0], 0xaa);
    assert_eq!(s2[0], 0xbb);
}

#[test]
fn test_eq() {
    let a = EcoBytes::from(b"hello");
    let b = EcoBytes::from(b"hello");
    let c = EcoBytes::from(b"world");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_eq_slice() {
    let s = EcoBytes::from(b"hello");
    assert_eq!(s, b"hello"[..]);
    assert_eq!(&b"hello"[..], s);
}

#[test]
fn test_eq_array() {
    let s = EcoBytes::from(b"hello");
    assert_eq!(s, *b"hello");
    assert_eq!(s, b"hello");
}

#[test]
fn test_ord() {
    let a = EcoBytes::from(b"abc");
    let b = EcoBytes::from(b"abd");
    assert!(a < b);
}

#[test]
fn test_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(EcoBytes::from(b"hello"));
    set.insert(EcoBytes::from(b"hello"));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_debug() {
    let s = EcoBytes::from(b"hi");
    let debug = format!("{:?}", s);
    assert!(debug.contains("104")); // 'h' = 104
}

#[test]
fn test_default() {
    let s = EcoBytes::default();
    assert!(s.is_empty());
}

#[test]
fn test_from_slice_ref() {
    let s: EcoBytes = (&b"hello"[..]).into();
    assert_eq!(&*s, b"hello");
}

#[test]
fn test_from_array_ref() {
    let s: EcoBytes = b"hello".into();
    assert_eq!(&*s, b"hello");
}

#[test]
fn test_from_vec() {
    let v = vec![1u8, 2, 3];
    let s: EcoBytes = v.into();
    assert_eq!(&*s, &[1, 2, 3]);
}

#[test]
fn bytes_from_vec_long_no_intermediate_copy() {
    // For len > LIMIT, `From<Vec<u8>>` takes the spill path: it moves
    // the bytes into a fresh `EcoVec` via `EcoVec::from(Vec<u8>)` rather
    // than going through `from_slice` (which would always copy). We
    // can't directly observe the move from outside the crate, but we
    // verify the resulting content, length, and mutation semantics are
    // correct on the spill path.
    let input: Vec<u8> = (0..100).collect();
    assert!(input.len() > EcoBytes::INLINE_LIMIT);

    let bs = EcoBytes::from(input.clone());
    assert_eq!(bs.len(), 100);
    assert_eq!(bs.as_slice(), &input[..]);
    for (i, &b) in bs.as_slice().iter().enumerate() {
        assert_eq!(b, i as u8);
    }

    // Mutation on the (uniquely-owned) spill path must not disturb content.
    let mut bs_mut = EcoBytes::from(input.clone());
    bs_mut.push(0xff);
    assert_eq!(bs_mut.len(), 101);
    assert_eq!(&bs_mut[..100], &input[..]);
    assert_eq!(bs_mut[100], 0xff);
}

#[test]
fn test_into_vec() {
    let s = EcoBytes::from(b"abc");
    let v: Vec<u8> = s.into();
    assert_eq!(v, vec![b'a', b'b', b'c']);
}

#[test]
fn test_from_iter_bytes() {
    let s: EcoBytes = (0u8..5).collect();
    assert_eq!(&*s, &[0, 1, 2, 3, 4]);
}

#[test]
fn test_from_iter_slices() {
    let slices: Vec<&[u8]> = vec![b"hello", b" ", b"world"];
    let s: EcoBytes = slices.into_iter().collect();
    assert_eq!(&*s, b"hello world");
}

#[test]
fn test_extend_bytes() {
    let mut s = EcoBytes::from(b"hi");
    s.extend(*b"!!");
    assert_eq!(&*s, b"hi!!");
}

#[test]
fn test_extend_slices() {
    let mut s = EcoBytes::from(b"hi");
    s.extend([&b" world"[..], &b"!"[..]]);
    assert_eq!(&*s, b"hi world!");
}

#[test]
fn test_repeat() {
    let s = EcoBytes::from(b"ab");
    let r = s.repeat(3);
    assert_eq!(&*r, b"ababab");
}

#[test]
fn test_with_capacity() {
    let mut s = EcoBytes::with_capacity(100);
    assert!(s.is_empty());
    s.extend_from_slice(&[0x42u8; 50]);
    assert_eq!(s.len(), 50);
}

#[test]
fn test_non_utf8_data() {
    // EcoBytes should handle arbitrary bytes, including invalid UTF-8.
    let data: Vec<u8> = (0u8..=255).collect();
    let s = EcoBytes::from(&data);
    assert_eq!(s.len(), 256);
    assert_eq!(&*s, &data[..]);
}

#[test]
fn test_borrow_and_asref() {
    use std::borrow::Borrow;
    let s = EcoBytes::from(b"test");
    let _: &[u8] = s.borrow();
    let _: &[u8] = s.as_ref();
}

#[test]
fn test_clone_from_static() {
    let s = EcoBytes::from_static(b"static data here!!");
    let s2 = s.clone();
    assert_eq!(&*s, &*s2);
}

#[test]
fn test_from_static_mutate() {
    let mut s = EcoBytes::from_static(b"hello world!!!!!");
    assert_eq!(s.len(), 16);
    s.push(b'!');
    assert_eq!(s.len(), 17);
    assert_eq!(&s[..16], b"hello world!!!!!");
    assert_eq!(s[16], b'!');
}

// --- Conversion tests ---

#[test]
fn test_try_from_eco_bytes_valid_utf8() {
    let bs = EcoBytes::from(b"hello");
    let s = EcoString::try_from(bs).unwrap();
    assert_eq!(s, "hello");
}

#[test]
fn test_try_from_eco_bytes_invalid_utf8() {
    let bs = EcoBytes::from(&[b'o', b'k', 0xff, 0xfe]);
    let err = EcoString::try_from(bs).unwrap_err();
    // Same error as `core::str::from_utf8` (and ecow's `TryFrom`).
    assert_eq!(err.valid_up_to(), 2);
}

#[test]
fn test_into_eco_string_lossy() {
    let bs = EcoBytes::from(b"hello \xff world");
    let s = bs.into_eco_string_lossy();
    assert!(s.contains('\u{FFFD}'));
    assert!(s.starts_with("hello "));
    assert!(s.ends_with(" world"));
}

#[test]
fn test_into_eco_string_lossy_valid() {
    let bs = EcoBytes::from(b"perfectly valid utf8");
    let s = bs.into_eco_string_lossy();
    assert_eq!(s, "perfectly valid utf8");
}

#[test]
fn test_from_eco_string() {
    let s = EcoString::from("hello world");
    let bs: EcoBytes = s.into();
    assert_eq!(&*bs, b"hello world");
}

#[test]
fn test_from_eco_string_ref() {
    let s = EcoString::from("hello");
    let bs: EcoBytes = (&s).into();
    assert_eq!(&*bs, b"hello");
    assert_eq!(s, "hello"); // original still alive
}

#[test]
fn test_roundtrip_eco_string_byte_string() {
    let original = EcoString::from("roundtrip test!");
    let bs: EcoBytes = original.into();
    let back = EcoString::try_from(bs).unwrap();
    assert_eq!(back, "roundtrip test!");
}

#[test]
fn test_try_from_eco_bytes_spilled() {
    // Spilled (>15 bytes) valid UTF-8
    let data = "this string is longer than fifteen bytes";
    let bs = EcoBytes::from(data.as_bytes());
    let s = EcoString::try_from(bs).unwrap();
    assert_eq!(s.as_str(), data);
}

#[test]
fn test_from_eco_string_spilled() {
    let s = EcoString::from("this is a long string that spills to heap");
    let bs: EcoBytes = s.into();
    assert_eq!(&*bs, b"this is a long string that spills to heap");
}

// ── EcoBytes Referenced variant ────────────────────────────────────────
//
// `EcoBytes::from_static` is the only public constructor that exercises
// the Referenced variant of the underlying `DynamicVec` (zero-copy borrow of a
// `'static [u8]`). `EcoBytes::from(&[u8])`, by contrast, always copies (`DynamicVec::from_slice`)
// and produces an Inline or Spilled variant.
//
// These tests cover the Referenced path: construction, clone-on-borrow,
// and materialisation-on-mutation for both short (≤ INLINE_LIMIT) and long
// (> INLINE_LIMIT) borrowed data.

#[test]
fn bytes_referenced_from_static_small() {
    // Data ≤ INLINE_LIMIT would normally fit inline, but `from_static` forces
    // the Referenced variant (no copy, borrows the static bytes).
    let data: &'static [u8] = b"hello";
    let bs = EcoBytes::from_static(data);
    assert_eq!(bs.as_slice(), data);
    assert_eq!(bs.len(), data.len());
}

#[test]
fn bytes_referenced_from_static_long() {
    // Data > INLINE_LIMIT exercises the Referenced variant with a long borrow.
    static LONG: &[u8] = b"hello world, this is borrowed byte data!";
    let bs = EcoBytes::from_static(LONG);
    assert_eq!(bs.as_slice(), LONG);
    assert_eq!(bs.len(), LONG.len());
}

#[test]
fn bytes_referenced_from_static_empty() {
    let bs = EcoBytes::from_static(b"");
    assert!(bs.is_empty());
    assert_eq!(bs.len(), 0);
}

#[test]
fn bytes_referenced_push_materialises_short() {
    // Push triggers materialisation (Referenced → Inline).
    let mut bs = EcoBytes::from_static(b"hello");
    bs.push(b'!');
    assert_eq!(bs.as_slice(), b"hello!");
}

#[test]
fn bytes_referenced_push_materialises_long() {
    // Push on a long Referenced variant materialises to Spilled (heap).
    static LONG: &[u8] = b"hello world, borrowed bytes";
    let mut bs = EcoBytes::from_static(LONG);
    bs.push(b'!');
    assert_eq!(bs.len(), LONG.len() + 1);
    assert_eq!(&bs[..LONG.len()], LONG);
    assert_eq!(bs[LONG.len()], b'!');
}

#[test]
fn bytes_referenced_extend_from_slice_materialises() {
    static LONG: &[u8] = b"hello world, this is borrowed";
    let mut bs = EcoBytes::from_static(LONG);
    bs.extend_from_slice(b"!!!");
    assert_eq!(bs.len(), LONG.len() + 3);
    assert_eq!(&bs[..LONG.len()], LONG);
    assert_eq!(&bs[LONG.len()..], b"!!!");
}

#[test]
fn bytes_referenced_make_mut_materialises() {
    static LONG: &[u8] = b"hello world, borrowed";
    let mut bs = EcoBytes::from_static(LONG);
    {
        let s = bs.make_mut();
        s[0] = b'H';
    }
    assert_eq!(bs[0], b'H');
    assert_eq!(&bs[1..], &LONG[1..]);
}

#[test]
fn bytes_referenced_clone_preserves_data() {
    static LONG: &[u8] = b"hello world, borrowed bytes here";
    let bs = EcoBytes::from_static(LONG);
    let cloned = bs.clone();
    assert_eq!(cloned.as_slice(), bs.as_slice());
    assert_eq!(cloned.as_slice(), LONG);
}

#[test]
fn bytes_referenced_truncate_preserves_prefix() {
    // Truncate on a Referenced variant shortens the view (still no copy).
    static LONG: &[u8] = b"hello world, borrowed";
    let mut bs = EcoBytes::from_static(LONG);
    bs.truncate(5);
    assert_eq!(bs.len(), 5);
    assert_eq!(bs.as_slice(), b"hello");
}

#[test]
fn bytes_referenced_truncate_noop_when_longer() {
    let mut bs = EcoBytes::from_static(b"hello");
    bs.truncate(100);
    assert_eq!(bs.as_slice(), b"hello");
}

#[test]
fn bytes_referenced_clear() {
    let mut bs = EcoBytes::from_static(b"hello world, borrowed bytes");
    bs.clear();
    assert!(bs.is_empty());
}

#[test]
fn bytes_referenced_pop() {
    static LONG: &[u8] = b"hello world, borrowed bytes";
    let mut bs = EcoBytes::from_static(LONG);
    assert_eq!(bs.pop(), Some(b's'));
    assert_eq!(bs.len(), LONG.len() - 1);
    assert_eq!(&bs[..], &LONG[..LONG.len() - 1]);
}

#[test]
fn bytes_referenced_eq_from_static_and_from_slice() {
    // Same content constructed via Referenced vs owned-copy paths must compare
    // equal. This exercises the Inline-vs-Referenced equality path in
    // `DynamicVec`'s `PartialEq` impl.
    static LONG: &[u8] = b"hello world, borrowed bytes";
    let bs_ref = EcoBytes::from_static(LONG);
    let bs_owned = EcoBytes::from(LONG);
    assert_eq!(bs_ref, bs_owned);
    assert_eq!(bs_ref.as_slice(), bs_owned.as_slice());
}

#[test]
fn bytes_referenced_debug() {
    let bs = EcoBytes::from_static(b"hi");
    let debug = format!("{:?}", bs);
    assert!(debug.contains("104")); // 'h' = 104
}

// ── EcoBytes basic correctness (From<&[u8]> path) ───────────────────────
//
// `EcoBytes::from(&[u8])` always copies — these are sanity checks for the owned-copy
// path, complementing the Referenced tests above.

#[test]
fn bytes_from_slice_basic_correctness() {
    let data = b"hello world, this is borrowed byte data!";
    let bs = EcoBytes::from(data);
    assert_eq!(bs.as_slice(), data);
}

#[test]
fn bytes_from_slice_push() {
    let data = b"hello";
    let mut bs = EcoBytes::from(data);
    bs.push(b'!');
    assert_eq!(bs.as_slice(), b"hello!");
}

#[test]
fn bytes_from_slice_independence() {
    let data = b"hello world, borrowed bytes";
    let bs = EcoBytes::from(data);
    // EcoBytes is fully owned ('static). Verify the data is independent
    // of the source slice.
    assert_eq!(bs.as_slice(), data);
}

// ── ecow 0.3.1 API parity ──────────────────────────────────────────────────
//
// Methods and impls that ecow 0.3.1's `EcoBytes` has, exercised across the
// Inline, Spilled (unique and shared) and Referenced (`from_static`) variants.

const LONG: &[u8] = b"a byte buffer that is definitely longer than inline";

#[test]
fn inline_and_try_inline_are_const() {
    const INLINE: EcoBytes = EcoBytes::inline(b"hi");
    const TRY_OK: Option<EcoBytes> = EcoBytes::try_inline(b"hi");
    const TRY_TOO_LONG: Option<EcoBytes> = EcoBytes::try_inline(LONG);
    assert_eq!(INLINE, b"hi");
    assert!(INLINE.is_inline());
    assert_eq!(TRY_OK.unwrap(), b"hi");
    assert!(TRY_TOO_LONG.is_none());
}

#[test]
fn is_inline_per_variant() {
    assert!(EcoBytes::new().is_inline());
    assert!(EcoBytes::from(b"short").is_inline());
    assert!(!EcoBytes::from(LONG).is_inline());
    // Borrowed data is neither inline nor spilled.
    assert!(!EcoBytes::from_static(b"short").is_inline());
    assert!(!EcoBytes::from_static(LONG).is_inline());
    // Spilled stays spilled even when it would fit (like ecow).
    let mut b = EcoBytes::from(LONG);
    b.truncate(2);
    assert!(!b.is_inline());
}

#[test]
fn capacity_per_variant() {
    assert_eq!(EcoBytes::new().capacity(), EcoBytes::INLINE_LIMIT);
    let spilled = EcoBytes::with_capacity(100);
    assert!(spilled.capacity() >= 100);
    assert!(!spilled.is_inline());
    // A borrowed buffer owns no storage.
    assert_eq!(EcoBytes::from_static(LONG).capacity(), LONG.len());
    assert_eq!(EcoBytes::from_static(b"abc").capacity(), 3);
}

#[test]
fn as_slice_matches_deref() {
    for b in [EcoBytes::from(b"abc"), EcoBytes::from(LONG), EcoBytes::from_static(LONG)] {
        assert_eq!(b.as_slice(), &*b);
    }
}

#[test]
fn insert_and_remove_inline() {
    let mut b = EcoBytes::from(b"ace");
    b.insert(1, b'b');
    b.insert(3, b'd');
    b.insert(5, b'f');
    assert_eq!(b, b"abcdef");
    assert!(b.is_inline());
    assert_eq!(b.remove(0), b'a');
    assert_eq!(b.remove(4), b'f');
    assert_eq!(b, b"bcde");
}

#[test]
fn insert_spills_when_full() {
    let mut b = EcoBytes::from([b'x'; EcoBytes::INLINE_LIMIT]);
    assert!(b.is_inline());
    b.insert(0, b'y');
    assert!(!b.is_inline());
    assert_eq!(b.len(), EcoBytes::INLINE_LIMIT + 1);
    assert_eq!(b[0], b'y');
}

#[test]
fn insert_slice_inline_spilled_and_shared() {
    let mut b = EcoBytes::from(b"ad");
    b.insert_slice(1, b"bc");
    assert_eq!(b, b"abcd");
    assert!(b.is_inline());

    // Overflowing the inline storage spills, preserving order.
    b.insert_slice(2, LONG);
    let mut expected = b"ab".to_vec();
    expected.extend_from_slice(LONG);
    expected.extend_from_slice(b"cd");
    assert_eq!(b, expected);
    assert!(!b.is_inline());

    // Shared spilled: copy-on-write leaves the clone untouched.
    let clone = b.clone();
    b.insert_slice(0, b"!");
    assert_eq!(clone, expected);
    assert_eq!(b[0], b'!');

    // Empty insert at the end is a no-op.
    let len = b.len();
    b.insert_slice(len, b"");
    assert_eq!(b.len(), len);
}

#[test]
fn remove_from_shared_spilled_is_cow() {
    let mut b = EcoBytes::from(LONG);
    let clone = b.clone();
    assert_eq!(b.remove(0), LONG[0]);
    assert_eq!(clone, LONG);
    assert_eq!(b, &LONG[1..]);
}

#[test]
fn referenced_insert_remove_materialise() {
    // Short borrowed data materialises inline on mutation.
    let mut short = EcoBytes::from_static(b"abc");
    short.insert(0, b'_');
    assert_eq!(short, b"_abc");
    assert!(short.is_inline());

    // Long borrowed data materialises onto the heap.
    let mut long = EcoBytes::from_static(LONG);
    long.insert_slice(1, b"--");
    assert_eq!(&long[..3], &[LONG[0], b'-', b'-']);
    assert_eq!(long.len(), LONG.len() + 2);

    // Removing a middle byte copies the borrowed data first.
    let mut mid = EcoBytes::from_static(LONG);
    assert_eq!(mid.remove(1), LONG[1]);
    assert_eq!(mid.len(), LONG.len() - 1);
    assert!(!mid.is_inline());

    // Removing the last byte just shortens the borrowed view.
    let mut last = EcoBytes::from_static(LONG);
    assert_eq!(last.remove(LONG.len() - 1), LONG[LONG.len() - 1]);
    assert_eq!(last, &LONG[..LONG.len() - 1]);
    assert_eq!(last.capacity(), LONG.len() - 1);
}

#[test]
fn reserve_per_variant() {
    // Inline with enough room: stays inline.
    let mut b = EcoBytes::from(b"abc");
    b.reserve(2);
    assert!(b.is_inline());
    // Inline without enough room: spills with room to spare.
    b.reserve(64);
    assert!(!b.is_inline());
    assert!(b.capacity() - b.len() >= 64);
    assert_eq!(b, b"abc");

    // Shared spilled: reserve unshares (the clone keeps its data).
    let mut shared = EcoBytes::from(LONG);
    let clone = shared.clone();
    shared.reserve(1);
    shared.push(b'!');
    assert_eq!(clone, LONG);

    // Borrowed: reserve copies the data into owned storage.
    let mut borrowed = EcoBytes::from_static(LONG);
    borrowed.reserve(10);
    assert!(borrowed.capacity() - borrowed.len() >= 10);
    assert_eq!(borrowed, LONG);
    let mut short = EcoBytes::from_static(b"abc");
    short.reserve(0);
    assert!(short.is_inline());
    assert_eq!(short, b"abc");
}

#[test]
fn pop_per_variant() {
    let mut b = EcoBytes::from_static(b"xy");
    assert_eq!(b.pop(), Some(b'y'));
    assert_eq!(b.pop(), Some(b'x'));
    assert_eq!(b.pop(), None);
}

#[test]
fn eq_with_vec_and_ecovec() {
    let b = EcoBytes::from(LONG);
    let v = LONG.to_vec();
    let e = EcoVec::from(LONG);
    assert_eq!(b, v);
    assert_eq!(v, b);
    assert_eq!(b, e);
    assert_eq!(e, b);
    assert_eq!(*b"abc", EcoBytes::from(b"abc"));
}

#[test]
fn from_array_cow_and_ecovec() {
    assert_eq!(EcoBytes::from([1u8, 2, 3]), [1, 2, 3]);
    assert_eq!(EcoBytes::from(Cow::Borrowed(&b"abc"[..])), b"abc");
    assert_eq!(EcoBytes::from(Cow::<[u8]>::Owned(LONG.to_vec())), LONG);

    // From<EcoVec<u8>> reuses the allocation and stays spilled.
    let e = EcoVec::from(b"abc".as_slice());
    let ptr = e.as_ptr();
    let b = EcoBytes::from(e);
    assert!(!b.is_inline());
    assert_eq!(b.as_ptr(), ptr);
}

#[test]
fn into_ecovec_reuses_spilled_allocation() {
    let b = EcoBytes::from(LONG);
    let ptr = b.as_ptr();
    let e = EcoVec::from(b);
    assert_eq!(e.as_ptr(), ptr);
    assert_eq!(e, LONG);

    // Inline and borrowed data is copied.
    assert_eq!(EcoVec::from(EcoBytes::from(b"abc")), b"abc");
    assert_eq!(EcoVec::from(EcoBytes::from_static(LONG)), LONG);
    let borrowed = EcoBytes::from_static(LONG);
    assert_eq!(EcoVec::<u8>::from(&borrowed), LONG);
    assert_eq!(borrowed, LONG);
}

#[test]
fn extend_ref_and_into_iter_ref() {
    let mut b = EcoBytes::new();
    b.extend(&[1u8, 2, 3]);
    b.extend([4u8, 5].iter());
    assert_eq!(b, [1, 2, 3, 4, 5]);
    let sum: u32 = (&b).into_iter().map(|&x| u32::from(x)).sum();
    assert_eq!(sum, 15);
    let mut seen = Vec::new();
    for byte in &b {
        seen.push(*byte);
    }
    assert_eq!(seen, [1, 2, 3, 4, 5]);
}

#[cfg(feature = "std")]
#[test]
fn io_write() {
    use std::io::Write;
    let mut b = EcoBytes::new();
    let n = 12;
    write!(b, "{n}-ab").unwrap();
    b.write_all(LONG).unwrap();
    b.flush().unwrap();
    let mut expected = b"12-ab".to_vec();
    expected.extend_from_slice(LONG);
    assert_eq!(b, expected);
}

#[test]
fn eco_string_try_from_ecovec_and_back() {
    let e = EcoVec::from(b"hello".as_slice());
    let s = EcoString::try_from(e).unwrap();
    assert_eq!(s, "hello");
    let bytes: EcoVec<u8> = s.into();
    assert_eq!(bytes, b"hello");
    let err = EcoString::try_from(EcoVec::from([0xffu8].as_slice())).unwrap_err();
    assert_eq!(err.valid_up_to(), 0);
}

#[test]
fn into_eco_string_lossy_borrowed() {
    let s = EcoBytes::from_static(b"static text").into_eco_string_lossy();
    assert_eq!(s, "static text");
    let lossy = EcoBytes::from_static(b"bad \xff").into_eco_string_lossy();
    assert_eq!(lossy, "bad \u{FFFD}");
}

#[test]
fn empty_ecovec_conversions() {
    // An empty `EcoVec` has no allocation (dangling pointer, capacity 0).
    let mut b = EcoBytes::from(EcoVec::<u8>::new());
    assert!(b.is_empty());
    assert!(!b.is_inline());
    b.push(1);
    b.insert_slice(0, LONG);
    assert_eq!(b.len(), LONG.len() + 1);
    assert_eq!(EcoVec::from(EcoBytes::new()), EcoVec::<u8>::new());

    let s = EcoString::try_from(EcoVec::<u8>::new()).unwrap();
    assert!(s.is_empty());
    assert_eq!(EcoVec::<u8>::from(EcoString::new()), EcoVec::<u8>::new());
}
