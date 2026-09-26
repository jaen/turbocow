#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[path = "../../tests/common/mod.rs"]
mod common;
use common::{VecOp, check_ecovec};

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    if let Ok(ops) = Vec::<VecOp>::arbitrary(&mut u) {
        check_ecovec(&ops);
    }
});
