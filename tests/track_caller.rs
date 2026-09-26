//! Verifies that turbocow's panicking public APIs are `#[track_caller]`, so a
//! panic reports the *user's* call site rather than an internal `src/...`
//! frame — matching upstream ecow's ergonomics.
//!
//! The panic hook is process-global, so every case runs sequentially inside a
//! single `#[test]` with the hook installed once and restored at the end. Each
//! case captures the panic `Location` reported to the hook and asserts the file
//! is *this* test file, not a turbocow source file.

use std::panic::{self, Location};
use std::sync::{Arc, Mutex};

use turbocow::{EcoBytes, EcoString, EcoVec, SmallVec};

/// Run `f` (expected to panic) and return the file path the panic hook saw via
/// `info.location()`. Panics in this function would be the test harness's, not
/// the closure's; we only ever feed it closures that panic.
fn captured_panic_file<F: FnOnce() + panic::UnwindSafe>(
    slot: &Arc<Mutex<Option<String>>>,
    f: F,
) -> String {
    slot.lock().unwrap().take(); // clear any prior capture
    let result = panic::catch_unwind(f);
    assert!(result.is_err(), "closure was expected to panic but did not");
    slot.lock()
        .unwrap()
        .take()
        .expect("panic hook did not capture a location")
}

#[test]
fn panics_report_caller_location() {
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // Install a hook that records the panic location's file. Save & restore the
    // previous hook so this test does not disturb the rest of the suite.
    let prev = panic::take_hook();
    let sink = Arc::clone(&captured);
    panic::set_hook(Box::new(move |info| {
        if let Some(loc) = info.location() {
            *sink.lock().unwrap() = Some(loc.file().to_string());
        }
    }));

    // Sanity check: a plain `Location::caller()` here resolves to this file, so
    // "ends with tests/track_caller.rs" is the right assertion shape.
    assert!(
        Location::caller().file().ends_with("tests/track_caller.rs"),
        "test self-check: caller file was {}",
        Location::caller().file()
    );

    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Box<dyn FnOnce() + panic::UnwindSafe>)> = vec![
        // ── EcoVec ───────────────────────────────────────────────────────
        (
            "EcoVec::insert out of bounds",
            Box::new(|| {
                let mut v: EcoVec<i32> = EcoVec::from(&[1, 2, 3][..]);
                v.insert(99, 0); // index > len
            }),
        ),
        (
            "EcoVec::remove out of bounds",
            Box::new(|| {
                let mut v: EcoVec<i32> = EcoVec::from(&[1, 2, 3][..]);
                v.remove(99); // index >= len
            }),
        ),
        (
            "EcoVec::replace out of bounds",
            Box::new(|| {
                let mut v: EcoVec<i32> = EcoVec::from(&[1, 2, 3][..]);
                v.replace(99, 0); // index >= len
            }),
        ),
        (
            "EcoVec::drain out of bounds",
            Box::new(|| {
                let mut v: EcoVec<i32> = EcoVec::from(&[1, 2, 3][..]);
                let _ = v.drain(0..99); // end > len
            }),
        ),
        // ── SmallVec ─────────────────────────────────────────────────────
        (
            "SmallVec index out of bounds",
            Box::new(|| {
                let mut v = SmallVec::<i32, 4>::new();
                v.push(1);
                v.push(2);
                let _ = v[10]; // Index impl -> slice index panic
            }),
        ),
        (
            "SmallVec::insert out of bounds",
            Box::new(|| {
                let mut v = SmallVec::<i32, 4>::new();
                v.push(1);
                v.insert(10, 0); // index > len
            }),
        ),
        (
            "SmallVec::remove out of bounds",
            Box::new(|| {
                let mut v = SmallVec::<i32, 4>::new();
                v.push(1);
                v.remove(10); // index >= len
            }),
        ),
        (
            "SmallVec::swap_remove out of bounds",
            Box::new(|| {
                let mut v = SmallVec::<i32, 4>::new();
                v.push(1);
                v.swap_remove(10); // index >= len
            }),
        ),
        // ── EcoString / EcoBytes ────────────────────────────────────
        (
            "EcoString::insert at non-char-boundary",
            Box::new(|| {
                let mut s = EcoString::from("héllo"); // 'é' spans bytes 1..3
                s.insert(2, 'x'); // byte 2 is not a char boundary
            }),
        ),
        (
            "EcoString::remove out of bounds",
            Box::new(|| {
                let mut s = EcoString::from("abc");
                s.remove(99); // not a char boundary / out of bounds
            }),
        ),
        (
            "EcoString::truncate at non-char-boundary",
            Box::new(|| {
                let mut s = EcoString::from("héllo");
                s.truncate(2); // byte 2 splits 'é'
            }),
        ),
        (
            "EcoBytes::inline exceeds capacity",
            Box::new(|| {
                let oversized = vec![0u8; EcoBytes::INLINE_LIMIT + 1];
                let _ = EcoBytes::inline(&oversized);
            }),
        ),
        (
            "EcoBytes::insert out of bounds",
            Box::new(|| {
                let mut b = EcoBytes::from(b"abc");
                b.insert(4, b'x'); // index > len
            }),
        ),
        (
            "EcoBytes::insert_slice out of bounds (spilled)",
            Box::new(|| {
                let mut b = EcoBytes::from(&[0u8; 32]);
                b.insert_slice(33, b"x"); // index > len
            }),
        ),
        (
            "EcoBytes::remove out of bounds (borrowed)",
            Box::new(|| {
                let mut b = EcoBytes::from_static(b"abc");
                b.remove(3); // index >= len
            }),
        ),
    ];

    let mut failures = Vec::new();
    for (name, case) in cases {
        let file = captured_panic_file(&captured, case);
        if !file.ends_with("tests/track_caller.rs") {
            failures.push(format!(
                "  - {name}: panic reported `{file}` (expected the caller's site, \
                 tests/track_caller.rs)"
            ));
        }
    }

    // Restore the previous hook before asserting, so a failure here doesn't
    // leave our capturing hook installed for the rest of the process.
    panic::set_hook(prev);

    assert!(
        failures.is_empty(),
        "the following panic paths did NOT report the caller's location:\n{}",
        failures.join("\n")
    );
}
