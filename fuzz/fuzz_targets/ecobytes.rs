#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{BytesOp, check_ecobytes, check_ecobytes_referenced};

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    // Drives EcoBytes through push / extend / pop / truncate / clear / insert /
    // remove / reserve against a Vec<u8> oracle, exercising the inline⇄spill
    // transitions, copy-on-write, and the byte-level API (the StringOp model only
    // covers EcoString — char/str ops EcoBytes does not have) — once from empty
    // and once from a borrowed (`from_static`) start. Shares the model with the
    // proptest entry points, so any fuzzer-found input reproduces in `cargo test`.
    if let Ok(ops) = Vec::<BytesOp>::arbitrary(&mut u) {
        check_ecobytes(&ops);
        check_ecobytes_referenced(&ops);
    }
});
