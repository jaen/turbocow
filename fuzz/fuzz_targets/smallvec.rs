#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{SmallVecOp, check_smallvec, check_smallvec_from_ref};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let flag = data[0]; // first byte decides Referenced vs new()
    let rest = &data[1..];
    let mut u = Unstructured::new(rest);
    if let Ok(ops) = Vec::<SmallVecOp>::arbitrary(&mut u) {
        if flag % 2 == 0 {
            check_smallvec(&ops);
        } else {
            check_smallvec_from_ref::<4>(&ops);
        }
    }
});
