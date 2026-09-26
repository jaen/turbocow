//! Property-based differential tests: turbocow vectors vs `std::vec::Vec`.
//!
//! A random byte string is decoded (via `arbitrary`) into an op sequence and
//! applied in lockstep to a turbocow vector and a `Vec` oracle. The same
//! decoder drives the `fuzz/` targets, so a fuzzer-found crash reproduces here.

mod common;

use arbitrary::{Arbitrary, Unstructured};
use common::{
    N, SmallVecOp, VecOp, check_ecovec, check_ecovec_boxed, check_smallvec,
    check_smallvec_boxed, check_smallvec_from_ref,
};
use proptest::prelude::*;

proptest! {
    // Keep failure-persistence (.proptest-regressions) on native; disable it
    // under Miri, whose isolation blocks the getcwd the path lookup needs.
    #![proptest_config({
        // Miri (~100x slower) runs a small case count — enough to exercise the
        // op sequences for UB; the full count runs natively. (`with_cases` is
        // compile-time, unlike PROPTEST_CASES which these module blocks ignore.)
        let mut config = ProptestConfig::with_cases(if cfg!(miri) { 32 } else { 4096 });
        if cfg!(miri) {
            config.failure_persistence = None;
        }
        config
    })]

    #[test]
    fn smallvec_matches_std_vec(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<SmallVecOp>::arbitrary(&mut u) {
            check_smallvec(&ops);
        }
    }

    #[test]
    fn ecovec_matches_std_vec(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<VecOp>::arbitrary(&mut u) {
            check_ecovec(&ops);
        }
    }

    #[test]
    fn proptest_ecovec_boxed(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<VecOp>::arbitrary(&mut u) {
            check_ecovec_boxed(&ops);
        }
    }

    #[test]
    fn proptest_smallvec_boxed(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<SmallVecOp>::arbitrary(&mut u) {
            check_smallvec_boxed::<N>(&ops);
        }
    }

    #[test]
    fn proptest_smallvec_from_ref(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<SmallVecOp>::arbitrary(&mut u) {
            check_smallvec_from_ref::<N>(&ops);
        }
    }
}
