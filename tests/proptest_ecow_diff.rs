//! Behavioral superset proof: turbocow's ecow-derived types must observably
//! match upstream `ecow` 0.3.1 on the shared API surface, op-for-op.
//!
//! `ecow_superset.rs` proves the API exists at *compile time*; this proves the
//! *runtime behavior* is identical, driven by the same `arbitrary` op model as
//! the std-differential tests.

mod common;

use arbitrary::{Arbitrary, Unstructured};
use common::{BytesOp, StringOp, VecOp};
use proptest::prelude::*;

/// Largest char boundary `<= raw % (len + 1)`.
fn boundary_le(s: &str, raw: usize) -> usize {
    let mut i = raw % (s.len() + 1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn check_ecovec_vs_ecow(ops: &[VecOp]) {
    let mut tc: turbocow::EcoVec<u32> = turbocow::EcoVec::new();
    let mut ec: ecow::EcoVec<u32> = ecow::EcoVec::new();

    for op in ops {
        match op {
            VecOp::Push(x) => {
                tc.push(*x);
                ec.push(*x);
            }
            VecOp::Pop => assert_eq!(tc.pop(), ec.pop()),
            VecOp::Insert { index, value } => {
                let i = index % (ec.len() + 1);
                tc.insert(i, *value);
                ec.insert(i, *value);
            }
            VecOp::Remove(index) => {
                if !ec.is_empty() {
                    let i = index % ec.len();
                    assert_eq!(tc.remove(i), ec.remove(i));
                }
            }
            VecOp::Truncate(n) => {
                let n = n % 32;
                tc.truncate(n);
                ec.truncate(n);
            }
            VecOp::Clear => {
                tc.clear();
                ec.clear();
            }
            VecOp::ExtendFromSlice(s) => {
                tc.extend_from_slice(s);
                ec.extend_from_slice(s);
            }
            VecOp::Reserve(n) => {
                tc.reserve(n % 128);
                ec.reserve(n % 128);
            }
            VecOp::RetainEven => {
                tc.retain(|x: &mut u32| *x % 2 == 0);
                ec.retain(|x: &mut u32| *x % 2 == 0);
            }
            VecOp::RetainNone => {
                tc.retain(|_: &mut u32| false);
                ec.retain(|_: &mut u32| false);
            }
            VecOp::CloneMutateCheck(x) => {
                let mut tcc = tc.clone();
                tcc.push(*x);
                let mut ecc = ec.clone();
                ecc.push(*x);
                assert_eq!(tcc.as_slice(), ecc.as_slice());
            }
            VecOp::Replace { index, value } => {
                if !ec.is_empty() {
                    let i = index % ec.len();
                    // Neither turbocow nor ecow 0.3.0 exposes a replace-returning
                    // method on EcoVec; emulate via remove+insert (both sides).
                    let old_tc = tc.remove(i);
                    tc.insert(i, *value);
                    let old_ec = ec.remove(i);
                    ec.insert(i, *value);
                    assert_eq!(old_tc, old_ec);
                }
            }
            VecOp::Drain { start, end, consume } => {
                let len = ec.len();
                if len > 0 {
                    let s = start % (len + 1);
                    let e = if end % (len + 1) >= s { end % (len + 1) } else { s };
                    let c = consume % (e - s + 1).max(1);
                    let mut dtc = tc.drain(s..e);
                    let mut dec = ec.drain(s..e);
                    for _ in 0..c {
                        assert_eq!(dtc.next(), dec.next());
                    }
                    drop(dtc);
                    drop(dec);
                }
            }
        }
        assert_eq!(tc.as_slice(), ec.as_slice(), "ecovec vs ecow after {op:?}");
        assert_eq!(tc.len(), ec.len());
    }
}

fn check_ecostring_vs_ecow(ops: &[StringOp]) {
    let mut tc = turbocow::EcoString::new();
    let mut ec = ecow::EcoString::new();

    for op in ops {
        match op {
            StringOp::PushChar(c) => {
                tc.push(*c);
                ec.push(*c);
            }
            StringOp::PushStr(s) => {
                tc.push_str(s);
                ec.push_str(s);
            }
            StringOp::Pop => assert_eq!(tc.pop(), ec.pop()),
            StringOp::Insert { index, ch } => {
                let i = boundary_le(tc.as_str(), *index);
                tc.insert(i, *ch);
                ec.insert(i, *ch);
            }
            StringOp::InsertStr { index, s } => {
                let i = boundary_le(tc.as_str(), *index);
                tc.insert_str(i, s);
                ec.insert_str(i, s);
            }
            StringOp::Remove(index) => {
                if !tc.as_str().is_empty() {
                    let mut i = index % tc.len();
                    while !tc.as_str().is_char_boundary(i) {
                        i -= 1;
                    }
                    assert_eq!(tc.remove(i), ec.remove(i));
                }
            }
            StringOp::Truncate(n) => {
                let n = boundary_le(tc.as_str(), *n);
                tc.truncate(n);
                ec.truncate(n);
            }
            StringOp::Clear => {
                tc.clear();
                ec.clear();
            }
            StringOp::Reserve(n) => {
                let n = n % 128;
                tc.reserve(n);
                ec.reserve(n);
                assert!(tc.capacity() - tc.len() >= n);
            }
        }
        assert_eq!(tc.as_str(), ec.as_str(), "ecostring vs ecow after {op:?}");
        assert_eq!(tc.len(), ec.len());
    }
}

fn check_ecobytes_vs_ecow(ops: &[BytesOp]) {
    let mut tc = turbocow::EcoBytes::new();
    let mut ec = ecow::EcoBytes::new();

    for op in ops {
        match op {
            BytesOp::Push(b) => {
                tc.push(*b);
                ec.push(*b);
            }
            BytesOp::Extend(bytes) => {
                tc.extend_from_slice(bytes);
                ec.extend_from_slice(bytes);
            }
            BytesOp::Pop => assert_eq!(tc.pop(), ec.pop()),
            BytesOp::Truncate(n) => {
                let n = n % (ec.len() + 1);
                tc.truncate(n);
                ec.truncate(n);
            }
            BytesOp::Clear => {
                tc.clear();
                ec.clear();
            }
            BytesOp::Insert { index, byte } => {
                let i = index % (ec.len() + 1);
                tc.insert(i, *byte);
                ec.insert(i, *byte);
            }
            BytesOp::InsertSlice { index, bytes } => {
                let i = index % (ec.len() + 1);
                tc.insert_slice(i, bytes);
                ec.insert_slice(i, bytes);
            }
            BytesOp::Remove(index) => {
                if !ec.is_empty() {
                    let i = index % ec.len();
                    assert_eq!(tc.remove(i), ec.remove(i));
                }
            }
            BytesOp::Reserve(n) => {
                let n = n % 128;
                tc.reserve(n);
                ec.reserve(n);
                assert!(tc.capacity() - tc.len() >= n);
            }
            BytesOp::CloneMutateCheck(b) => {
                let mut tcc = tc.clone();
                tcc.push(*b);
                let mut ecc = ec.clone();
                ecc.push(*b);
                assert_eq!(tcc.as_slice(), ecc.as_slice());
            }
        }
        assert_eq!(tc.as_slice(), ec.as_slice(), "ecobytes vs ecow after {op:?}");
        assert_eq!(tc.len(), ec.len());
        // Same inline limit and the same inline⇄spill transitions as ecow.
        assert_eq!(tc.is_inline(), ec.is_inline(), "is_inline vs ecow after {op:?}");
    }
}

proptest! {
    // Keep proptest's failure-persistence (the `.proptest-regressions` file) on
    // native runs, but disable it under Miri, whose isolation blocks the
    // `current_dir()`/`getcwd` the regressions-file path lookup needs.
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
    fn ecovec_matches_ecow(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<VecOp>::arbitrary(&mut u) {
            check_ecovec_vs_ecow(&ops);
        }
    }

    #[test]
    fn ecostring_matches_ecow(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<StringOp>::arbitrary(&mut u) {
            check_ecostring_vs_ecow(&ops);
        }
    }

    #[test]
    fn ecobytes_matches_ecow(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let mut u = Unstructured::new(&data);
        if let Ok(ops) = Vec::<BytesOp>::arbitrary(&mut u) {
            check_ecobytes_vs_ecow(&ops);
        }
    }
}
