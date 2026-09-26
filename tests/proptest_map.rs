//! Property-based differential tests: turbocow maps vs `std::collections::HashMap`.

mod common;

use arbitrary::{Arbitrary, Unstructured};
use common::{MapOp, N, check_ecomap, check_smallmap};
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
    fn smallmap_matches_std(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<MapOp>::arbitrary(&mut u) {
            check_smallmap::<N>(&ops);
        }
    }

    #[test]
    fn ecomap_matches_std(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<MapOp>::arbitrary(&mut u) {
            check_ecomap(&ops);
        }
    }
}

/// Differential model run at N ∈ {8, 16, 17, 24, 32, 48, 64} to put every
/// chunked-matcher path under differential testing vs `HashMap`: scalar-only
/// (N < 16), single SSE2 chunk (16, 17), the [16,len) tail (17, 24), a third
/// full chunk plus tail (48), and four full chunks (64). Bug #1 (SIMD OOB for
/// N ∈ [17,31]) was hidden because no test exercised this range; this matrix
/// closes that gap and covers N > 32 now that the cap is gone.
///
/// Case count honours `PROPTEST_CASES`; under Miri it defaults low (`cfg!(miri)`)
/// so the extra N values don't blow up the ~100× slower run.
#[test]
fn proptest_smallmap_n_dispatch_matrix() {
    fn run<const N: usize>(cases: u32) {
        let mut config = ProptestConfig::with_cases(cases);
        if cfg!(miri) {
            config.failure_persistence = None;
        }
        proptest!(
            config,
            |(data in proptest::collection::vec(any::<u8>(), 0..1024))| {
                let mut u = Unstructured::new(&data);
                if let Ok(ops) = Vec::<MapOp>::arbitrary(&mut u) {
                    check_smallmap::<N>(&ops);
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
