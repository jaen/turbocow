#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{N, SetOp, check_ecoset, check_smallset};

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    if let Ok(ops) = Vec::<SetOp>::arbitrary(&mut u) {
        // N = 4 hits the spill; N = 40 exercises the chunked h2 matcher
        // (SSE2 chunks + scalar tail), mirroring the map fuzz target.
        check_smallset::<N>(&ops);
        check_smallset::<40>(&ops);
        check_ecoset(&ops);
    }
});
