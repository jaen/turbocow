//! Property-based differential tests: turbocow sets vs `std::collections::HashSet`.

mod common;

use arbitrary::{Arbitrary, Unstructured};
use common::{N, SetOp, check_ecoset, check_smallset};
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
    fn smallset_matches_std(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<SetOp>::arbitrary(&mut u) {
            check_smallset::<N>(&ops);
        }
    }

    #[test]
    fn ecoset_matches_std(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<SetOp>::arbitrary(&mut u) {
            check_ecoset(&ops);
        }
    }
}

/// Differential model run at N ∈ {8, 16, 17, 24, 32, 48, 64} (covering N > 32
/// now that the cap is gone) vs `HashSet`. Mirrors
/// `proptest_smallmap_n_dispatch_matrix`; case count honours `PROPTEST_CASES`.
#[test]
fn proptest_smallset_n_dispatch_matrix() {
    fn run<const N: usize>(cases: u32) {
        let mut config = ProptestConfig::with_cases(cases);
        if cfg!(miri) {
            config.failure_persistence = None;
        }
        proptest!(
            config,
            |(data in proptest::collection::vec(any::<u8>(), 0..1024))| {
                let mut u = Unstructured::new(&data);
                if let Ok(ops) = Vec::<SetOp>::arbitrary(&mut u) {
                    check_smallset::<N>(&ops);
                }
            }
        );
    }

    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if cfg!(miri) { 8 } else { 512 });
    run::<8>(cases);
    run::<16>(cases);
    run::<17>(cases);
    run::<24>(cases);
    run::<32>(cases);
    run::<48>(cases);
    run::<64>(cases);
}
