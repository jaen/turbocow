//! Tests for `EcoStr<'a>` Referenced (zero-copy borrow) variant.
//!
//! This is the most unsafe-heavy code in the crate: a raw pointer in a union,
//! tag-bit encoding, and a lifetime transmute in `into_owned`. These tests
//! verify creation, mutation (materialisation), `into_owned`, equality, clone,
//! and drop soundness (the latter two under Miri).

use turbocow::{EcoStr, EcoString};

#[test]
fn referenced_from_borrow() {
    let source = String::from("hello world, this is a long borrowed string!");
    let s: EcoStr<'_> = EcoStr::from(source.as_str());
    assert_eq!(s.as_str(), source.as_str());
    assert_eq!(s.len(), source.len());
}

#[test]
fn referenced_short_string_also_referenced() {
    // Even short strings (<=15 bytes) create Referenced via EcoStr::from.
    let source = "hi";
    let s: EcoStr<'_> = EcoStr::from(source);
    assert_eq!(s.as_str(), "hi");
}

#[test]
fn referenced_push_materializes_inline() {
    let source = String::from("hello"); // <=15 bytes -> materialises to Inline
    let mut s: EcoStr<'_> = EcoStr::from(source.as_str());
    s.push('!');
    assert_eq!(s.as_str(), "hello!");
}

#[test]
fn referenced_push_str_materializes_spilled() {
    let source = String::from("hello world, this is short");
    let mut s: EcoStr<'_> = EcoStr::from(source.as_str());
    s.push_str(" but now it is a much longer string that spills!");
    assert!(s.len() > 15);
    assert_eq!(
        s.as_str(),
        "hello world, this is short but now it is a much longer string that spills!"
    );
}

#[test]
fn referenced_into_owned_copies() {
    let source = String::from("hello world, borrowed data here");
    let s: EcoStr<'_> = EcoStr::from(source.as_str());
    let owned: EcoString = s.into_owned();
    assert_eq!(owned.as_str(), source.as_str());
    // owned is independent — dropping source doesn't affect it.
    drop(source);
    assert_eq!(owned.as_str(), "hello world, borrowed data here");
}

#[test]
fn referenced_eq_inline_same_content() {
    let source = "hello";
    let referenced: EcoStr<'_> = EcoStr::from(source);
    let inline = EcoString::from("hello");
    assert_eq!(referenced.as_str(), inline.as_str());
    // EcoStr and EcoString both deref to str; compare via str.
    assert_eq!(&*referenced, &*inline);
}

#[test]
fn referenced_clone_is_referenced() {
    let source = String::from("hello world, borrowed");
    let s: EcoStr<'_> = EcoStr::from(source.as_str());
    let cloned = s.clone();
    assert_eq!(cloned.as_str(), s.as_str());
    // Note: we intentionally do NOT `drop(source)` here. Both `s` and
    // `cloned` borrow `source` for `'a`, so dropping `source` while either
    // is alive would be a use-after-free. The borrow checker enforces this:
    // `drop(source)` would fail to compile because `s` and `cloned` still
    // hold borrows of `source`. They are dropped at end of scope, after
    // which `source` is dropped naturally (reverse declaration order).
}

#[test]
fn referenced_truncate_stays_referenced() {
    let source = String::from("hello world, borrowed data");
    let mut s: EcoStr<'_> = EcoStr::from(source.as_str());
    s.truncate(5);
    assert_eq!(s.as_str(), "hello");
    // truncate on Referenced adjusts the len tag without materialising.
}

#[test]
fn referenced_pop_then_push_roundtrip() {
    // pop on a Referenced variant triggers materialisation (truncate-like
    // length adjustment but pop reads the removed char first).
    let source = String::from("hello world, borrowed data");
    let mut s: EcoStr<'_> = EcoStr::from(source.as_str());
    assert_eq!(s.pop(), Some('a'));
    assert_eq!(s.as_str(), "hello world, borrowed dat");
    s.push('a');
    assert_eq!(s.as_str(), "hello world, borrowed data");
}

#[cfg(miri)]
#[test]
fn referenced_materialise_then_drop_miri() {
    let source = String::from("hello world, this is a long borrowed string!");
    {
        let mut s: EcoStr<'_> = EcoStr::from(source.as_str());
        s.push_str(" More data to force spill!");
        // s drops here — materialised to Spilled, then EcoVec drops.
    }
    // Miri verifies no UB in the materialise + drop path.
}

#[cfg(miri)]
#[test]
fn referenced_clone_drop_one_miri() {
    // Clone the Referenced variant, drop one copy, then materialise the
    // other via into_owned. Miri verifies the raw pointer in the union
    // remains valid through clone/drop.
    let source = String::from("hello world, this is a long borrowed string!");
    let s: EcoStr<'_> = EcoStr::from(source.as_str());
    let cloned = s.clone();
    drop(s);
    // `cloned` still borrows `source` — must still be readable.
    let mut owned = cloned.into_owned();
    owned.push('!');
    assert_eq!(owned.as_str(), "hello world, this is a long borrowed string!!");
}

#[cfg(miri)]
#[test]
fn referenced_into_owned_lifetime_widen_miri() {
    let source = String::from("hello world, borrowed");
    let s: EcoStr<'_> = EcoStr::from(source.as_str());
    let owned: EcoString = s.into_owned();
    drop(source);
    // owned must still be valid — into_owned copies for Referenced.
    assert_eq!(owned.as_str(), "hello world, borrowed");
    // Miri verifies no dangling pointer.
}
