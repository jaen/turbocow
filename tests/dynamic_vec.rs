//! Tests for DynamicVec internals exercised through the public EcoString API,
//! plus direct unit-style tests for the Referenced variant and inline/spill
//! transitions.

use turbocow::EcoString;

/// Build a string of exactly `n` ASCII bytes (cycling `a`..=`z`).
///
/// `EcoString::INLINE_LIMIT` is target-dependent — notably *larger* on
/// big-endian, where the inline union carries its tag past the EcoVec — so
/// inline-capacity tests must generate `INLINE_LIMIT` bytes rather than slice a
/// fixed-length literal (which panics once the limit exceeds the literal's
/// length). Distinct cycling letters also make a corrupted round-trip visible.
fn ascii_filling(n: usize) -> String {
    (0..n).map(|i| (b'a' + (i % 26) as u8) as char).collect()
}

// ---------------------------------------------------------------------------
// Basic inline storage
// ---------------------------------------------------------------------------

#[test]
fn inline_short_string() {
    let s = EcoString::from("hello");
    assert_eq!(s.as_str(), "hello");
    assert_eq!(s.len(), 5);
}

#[test]
fn inline_max_ascii() {
    let limit = EcoString::INLINE_LIMIT;
    let full = ascii_filling(limit);
    assert_eq!(full.len(), limit);
    let s = EcoString::from(full.as_str());
    assert_eq!(s.as_str(), full);
}

#[test]
fn inline_empty() {
    let s = EcoString::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert_eq!(s.as_str(), "");
}

// ---------------------------------------------------------------------------
// Spill to heap
// ---------------------------------------------------------------------------

#[test]
fn spill_on_long_string() {
    let text = "this string is definitely longer than 15 bytes";
    let s = EcoString::from(text);
    assert_eq!(s.as_str(), text);
    assert_eq!(s.len(), text.len());
}

#[test]
fn spill_via_push() {
    let limit = EcoString::INLINE_LIMIT;
    let text = ascii_filling(limit);
    let mut s = EcoString::from(text.as_str());
    assert_eq!(s.len(), limit);
    s.push('p'); // should spill
    assert_eq!(s.len(), limit + 1);
}

#[test]
fn spill_via_push_str() {
    let mut s = EcoString::from("hello");
    s.push_str(" world, this is a long string that will definitely spill");
    assert!(s.len() > EcoString::INLINE_LIMIT);
    assert!(s.starts_with("hello world"));
}

// ---------------------------------------------------------------------------
// Clone-on-write semantics
// ---------------------------------------------------------------------------

#[test]
fn clone_shares_heap() {
    let a = EcoString::from("this is a heap-allocated string!");
    let b = a.clone();
    assert_eq!(a, b);
    assert_eq!(a.as_str(), b.as_str());
}

#[test]
fn make_mut_copies_on_shared() {
    let a = EcoString::from("this is a heap-allocated string!");
    let mut b = a.clone();
    b.make_mut().make_ascii_uppercase();
    assert_ne!(a, b);
    assert_eq!(a.as_str(), "this is a heap-allocated string!");
    assert_eq!(b.as_str(), "THIS IS A HEAP-ALLOCATED STRING!");
}

#[test]
fn clone_inline_is_independent() {
    let a = EcoString::from("short");
    let mut b = a.clone();
    b.push('!');
    assert_eq!(a.as_str(), "short");
    assert_eq!(b.as_str(), "short!");
}

// ---------------------------------------------------------------------------
// Truncate / clear / pop
// ---------------------------------------------------------------------------

#[test]
fn truncate_inline() {
    let mut s = EcoString::from("hello");
    s.truncate(3);
    assert_eq!(s.as_str(), "hel");
}

#[test]
fn truncate_heap() {
    let mut s = EcoString::from("this is a long heap string!!!!");
    s.truncate(4);
    assert_eq!(s.as_str(), "this");
}

#[test]
fn clear_inline() {
    let mut s = EcoString::from("hi");
    s.clear();
    assert!(s.is_empty());
}

#[test]
fn clear_heap() {
    let mut s = EcoString::from("a]long heap string that spills");
    s.clear();
    assert!(s.is_empty());
}

#[test]
fn pop_multibyte() {
    let mut s = EcoString::from("café");
    assert_eq!(s.pop(), Some('é'));
    assert_eq!(s.as_str(), "caf");
}

// ---------------------------------------------------------------------------
// Replace / case conversion (exercises extend_from_slice paths)
// ---------------------------------------------------------------------------

#[test]
fn replace_inline() {
    let s = EcoString::from("aabaa");
    assert_eq!(s.replace("a", "x"), "xxbxx");
}

#[test]
fn to_uppercase_spills() {
    let s = EcoString::from("hello world!!");
    let u = s.to_uppercase();
    assert_eq!(u.as_str(), "HELLO WORLD!!");
}

// ---------------------------------------------------------------------------
// with_capacity
// ---------------------------------------------------------------------------

#[test]
fn with_capacity_inline() {
    let s = EcoString::with_capacity(5);
    assert!(s.is_empty());
}

#[test]
fn with_capacity_heap() {
    let mut s = EcoString::with_capacity(100);
    assert!(s.is_empty());
    s.push_str("test");
    assert_eq!(s.as_str(), "test");
}

// ---------------------------------------------------------------------------
// Size / layout sanity
// ---------------------------------------------------------------------------

#[test]
fn eco_string_size() {
    use core::mem;
    let word = mem::size_of::<usize>();
    // On 64-bit LE: 16 bytes.  On 64-bit BE: 24 bytes.
    assert!(mem::size_of::<EcoString>() <= 3 * word);
    assert!(mem::size_of::<EcoString>() >= 2 * word);
}

// ---------------------------------------------------------------------------
// Repeat (exercises extend_from_slice in a loop)
// ---------------------------------------------------------------------------

#[test]
fn repeat_inline() {
    let s = EcoString::from("ab");
    let r = s.repeat(3);
    assert_eq!(r.as_str(), "ababab");
}

#[test]
fn repeat_spills() {
    let s = EcoString::from("hello ");
    let r = s.repeat(10);
    assert_eq!(r.len(), 60);
    assert!(r.starts_with("hello hello "));
}

// ---------------------------------------------------------------------------
// FromIterator / Extend
// ---------------------------------------------------------------------------

#[test]
fn from_iter_chars() {
    let s: EcoString = "hello".chars().collect();
    assert_eq!(s.as_str(), "hello");
}

#[test]
fn from_iter_strs() {
    let parts = ["hello", " ", "world"];
    let s: EcoString = parts.iter().copied().collect();
    assert_eq!(s.as_str(), "hello world");
}

#[test]
fn extend_chars() {
    let mut s = EcoString::from("hi");
    s.extend(" there".chars());
    assert_eq!(s.as_str(), "hi there");
}

#[test]
fn extend_strs() {
    let mut s = EcoString::from("a");
    s.extend(["b", "c", "d"]);
    assert_eq!(s.as_str(), "abcd");
}

// ---------------------------------------------------------------------------
// eco_format! macro
// ---------------------------------------------------------------------------

#[test]
fn eco_format_basic() {
    let s = turbocow::eco_format!("x={}", 42);
    assert_eq!(s, "x=42");
}

#[test]
fn eco_format_long() {
    let s = turbocow::eco_format!("{:>50}", "right");
    assert_eq!(s.len(), 50);
}
