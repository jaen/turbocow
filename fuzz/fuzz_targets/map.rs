#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{MapOp, N, check_ecomap, check_smallmap};

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    if let Ok(ops) = Vec::<MapOp>::arbitrary(&mut u) {
        // Same op stream against both turbocow maps and the std oracle.
        // N = 4 hits the inline→heap spill cheaply; N = 40 exercises the
        // chunked h2 matcher (two SSE2 chunks + an 8-byte scalar tail) — the
        // most unsafe code, otherwise only covered by proptest.
        check_smallmap::<N>(&ops);
        check_smallmap::<40>(&ops);
        check_ecomap(&ops);
    }
});
