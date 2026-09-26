//! Tests for EcoString: insert / insert_str / remove / ToEcoString

use turbocow::EcoString;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Returns a string that is guaranteed to be stored inline (≤ 15 bytes).
fn inline_str() -> EcoString {
    EcoString::from("hello")
}

/// Returns a string that is guaranteed to spill to the heap (> 15 bytes).
fn spilled_str() -> EcoString {
    EcoString::from("hello world, long string here!")
}

// ─── insert / insert_str ────────────────────────────────────────────────────

#[test]
fn test_insert_at_start_inline() {
    let mut s = inline_str();
    s.insert(0, 'X');
    assert_eq!(s, "Xhello");
}

#[test]
fn test_insert_at_middle_inline() {
    let mut s = inline_str();
    s.insert(2, 'X');
    assert_eq!(s, "heXllo");
}

#[test]
fn test_insert_at_end_inline() {
    let mut s = inline_str();
    let len = s.len();
    s.insert(len, '!');
    assert_eq!(s, "hello!");
}

#[test]
fn test_insert_at_start_spilled() {
    let mut s = spilled_str();
    s.insert(0, 'X');
    assert!(s.starts_with('X'));
}

#[test]
fn test_insert_at_middle_spilled() {
    let mut s = spilled_str();
    // Insert after "hello"
    s.insert(5, '_');
    assert_eq!(&s.as_str()[..6], "hello_");
}

#[test]
fn test_insert_at_end_spilled() {
    let mut s = spilled_str();
    let expected = s.as_str().to_owned() + "!";
    let len = s.len();
    s.insert(len, '!');
    assert_eq!(s.as_str(), expected.as_str());
}

#[test]
fn test_insert_multibyte_char_inline() {
    let mut s = EcoString::from("ab");
    // '🌱' is 4 bytes
    s.insert(1, '🌱');
    assert_eq!(s, "a🌱b");
}

#[test]
fn test_insert_multibyte_char_spilled() {
    let mut s = spilled_str();
    let mid = 5; // char boundary at "hello|..."
    let prefix = s.as_str()[..mid].to_owned();
    let suffix = s.as_str()[mid..].to_owned();
    s.insert(mid, '🌱');
    let expected = format!("{prefix}🌱{suffix}");
    assert_eq!(s.as_str(), expected.as_str());
}

#[test]
fn test_insert_str_at_start_inline() {
    let mut s = inline_str();
    s.insert_str(0, ">>>");
    assert_eq!(s, ">>>hello");
}

#[test]
fn test_insert_str_at_end_inline() {
    let mut s = inline_str();
    let len = s.len();
    s.insert_str(len, "<<<");
    assert_eq!(s, "hello<<<");
}

#[test]
fn test_insert_str_at_middle_spilled() {
    let mut s = spilled_str();
    let before = s.as_str()[..5].to_owned();
    let after = s.as_str()[5..].to_owned();
    s.insert_str(5, "[MID]");
    let expected = format!("{before}[MID]{after}");
    assert_eq!(s.as_str(), expected.as_str());
}

// ─── insert panics ──────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_insert_out_of_bounds() {
    let mut s = inline_str();
    s.insert(999, 'x');
}

#[test]
#[should_panic]
fn test_insert_non_char_boundary() {
    // '🌱' = 0xF0 0x9F 0x8C 0xB1 — byte 1 is not a char boundary
    let mut s = EcoString::from("🌱abc");
    s.insert(1, 'x');
}

// ─── remove ─────────────────────────────────────────────────────────────────

#[test]
fn test_remove_first_char_inline() {
    let mut s = inline_str(); // "hello"
    let c = s.remove(0);
    assert_eq!(c, 'h');
    assert_eq!(s, "ello");
}

#[test]
fn test_remove_middle_char_inline() {
    let mut s = inline_str(); // "hello"
    let c = s.remove(2);
    assert_eq!(c, 'l');
    assert_eq!(s, "helo");
}

#[test]
fn test_remove_last_char_inline() {
    let mut s = inline_str(); // "hello"
    let last_idx = s.len() - 'o'.len_utf8();
    let c = s.remove(last_idx);
    assert_eq!(c, 'o');
    assert_eq!(s, "hell");
}

#[test]
fn test_remove_multibyte_char_inline() {
    let mut s = EcoString::from("a🌱b");
    let c = s.remove(1);
    assert_eq!(c, '🌱');
    assert_eq!(s, "ab");
}

#[test]
fn test_remove_first_char_spilled() {
    let mut s = spilled_str();
    let first_char = s.chars().next().unwrap();
    let expected = s.as_str()[first_char.len_utf8()..].to_owned();
    let c = s.remove(0);
    assert_eq!(c, first_char);
    assert_eq!(s.as_str(), expected.as_str());
}

#[test]
fn test_remove_last_char_spilled() {
    let mut s = spilled_str();
    let last_char = s.chars().next_back().unwrap();
    let expected_len = s.len() - last_char.len_utf8();
    let expected = s.as_str()[..expected_len].to_owned();
    let idx = expected_len;
    let c = s.remove(idx);
    assert_eq!(c, last_char);
    assert_eq!(s.as_str(), expected.as_str());
}

// ─── remove panics ──────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_remove_out_of_bounds() {
    let mut s = inline_str();
    s.remove(999);
}

#[test]
#[should_panic]
fn test_remove_non_char_boundary() {
    let mut s = EcoString::from("🌱abc");
    s.remove(1); // inside the 4-byte '🌱'
}

// ─── clone-on-write: insert/remove do not affect the original ───────────────

#[test]
fn test_insert_cow_spilled() {
    let original = spilled_str();
    let mut cloned = original.clone();
    cloned.insert(0, 'Z');
    // original must be unaffected
    assert_eq!(original, spilled_str());
    assert!(cloned.starts_with('Z'));
}

#[test]
fn test_remove_cow_spilled() {
    let original = spilled_str();
    let mut cloned = original.clone();
    cloned.remove(0);
    assert_eq!(original, spilled_str());
    assert_ne!(cloned.as_str(), original.as_str());
}

// ─── ToEcoString ────────────────────────────────────────────────────────────

#[test]
fn test_to_eco_string_int() {
    use turbocow::ToEcoString;
    assert_eq!(42i32.to_eco_string(), EcoString::from("42"));
}

#[test]
fn test_to_eco_string_str() {
    use turbocow::ToEcoString;
    assert_eq!("hello".to_eco_string(), EcoString::from("hello"));
}

#[test]
fn test_to_eco_string_via_crate_root() {
    use turbocow::ToEcoString;
    assert_eq!(2.5f64.to_eco_string(), EcoString::from("2.5"));
}

// ─── PartialEq must be content-based regardless of inline representation ──────
// Regression tests: the inline `eq` fast path used to compare the raw [u8; N]
// buffer, which (1) assumed tail bytes [len..N) stay zero (false after
// shrinking ops like truncate/clear) and (2) byte-compared Inline against
// Referenced (different encodings of the same content). Both made
// semantically-equal strings compare unequal.

#[test]
fn test_eq_after_inline_truncate() {
    let mut a = EcoString::from("abcde");
    a.truncate(3);
    assert_eq!(a.as_str(), "abc");
    assert_eq!(a, EcoString::from("abc"));
    assert_eq!(EcoString::from("abc"), a);
}

#[test]
fn test_eq_after_clear_and_rebuild() {
    let mut a = EcoString::from("xxxxx");
    a.clear();
    a.push_str("ab");
    assert_eq!(a, EcoString::from("ab"));
}

#[test]
fn test_eq_inline_vs_referenced() {
    let inline = EcoString::from("hello");
    let referenced = EcoString::from_static("hello");
    assert_eq!(inline, referenced);
    assert_eq!(referenced, inline);
}

#[test]
fn test_eq_hash_contract_after_truncate() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(EcoString::from("abc"));
    let mut probe = EcoString::from("abcde");
    probe.truncate(3);
    assert!(
        set.contains(&probe),
        "a truncated \"abc\" must hash and compare equal to a freshly-built \"abc\""
    );
}

// ─── ecow 0.3.1 API parity ──────────────────────────────────────────────────

#[test]
fn try_inline_is_const() {
    const OK: Option<EcoString> = EcoString::try_inline("hello");
    const TOO_LONG: Option<EcoString> =
        EcoString::try_inline("this is too long to be inline");
    assert_eq!(OK.unwrap(), "hello");
    assert!(TOO_LONG.is_none());
}

#[test]
fn is_inline_and_capacity() {
    assert!(inline_str().is_inline());
    assert_eq!(inline_str().capacity(), EcoString::INLINE_LIMIT);
    let spilled = spilled_str();
    assert!(!spilled.is_inline());
    assert!(spilled.capacity() >= spilled.len());
    // A borrowed string owns no storage.
    let borrowed = EcoString::from_static("borrowed static string!");
    assert!(!borrowed.is_inline());
    assert_eq!(borrowed.capacity(), borrowed.len());
}

#[test]
fn reserve_guarantees_room_and_ownership() {
    let mut s = inline_str();
    s.reserve(100);
    assert!(s.capacity() - s.len() >= 100);
    assert_eq!(s, "hello");

    let mut shared = spilled_str();
    let clone = shared.clone();
    shared.reserve(1);
    shared.push('!');
    assert_eq!(clone, "hello world, long string here!");

    let mut borrowed = EcoString::from_static("borrowed static string!");
    borrowed.reserve(4);
    assert!(borrowed.capacity() - borrowed.len() >= 4);
    assert_eq!(borrowed, "borrowed static string!");
}

#[test]
fn from_str_resolves_to_the_trait() {
    // An inherent `from_str` used to shadow `FromStr::from_str` here.
    use std::str::FromStr;
    let s = EcoString::from_str("hi").unwrap();
    assert_eq!(s, "hi");
}
