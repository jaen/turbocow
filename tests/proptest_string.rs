//! Property-based differential tests: turbocow `EcoString` vs `std::string::String`.

mod common;

use arbitrary::{Arbitrary, Unstructured};
use common::{
    BytesOp, StringOp, check_ecobytes, check_ecobytes_referenced,
    check_ecostr_referenced, check_ecostring,
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
    fn ecostring_matches_std_string(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<StringOp>::arbitrary(&mut u) {
            check_ecostring(&ops);
        }
    }

    #[test]
    fn ecobytes_matches_vec_u8(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<BytesOp>::arbitrary(&mut u) {
            check_ecobytes(&ops);
        }
    }

    #[test]
    fn ecobytes_referenced_matches_vec_u8(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<BytesOp>::arbitrary(&mut u) {
            check_ecobytes_referenced(&ops);
        }
    }

    #[test]
    fn proptest_ecostr_referenced(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<StringOp>::arbitrary(&mut u) {
            check_ecostr_referenced(&ops);
        }
    }
}
