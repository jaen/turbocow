//! Shared model-based differential-testing harness.
//!
//! A single `arbitrary`-derived op model per collection family is applied in
//! lockstep to a turbocow type and a `std` (or `ecow`) oracle, asserting
//! observable equivalence after every operation. The same model drives both the
//! proptest integration tests (`tests/proptest_*.rs`) and the cargo-fuzz
//! targets (`fuzz/fuzz_targets/*.rs`), which `#[path]`-include this file.
//!
//! `std` is the primary oracle (it is the definition of correct); comparing
//! against another small-collection crate risks "bug-compatible" agreement.

#![allow(dead_code)] // each consumer (test binary / fuzz target) uses a subset
#![allow(clippy::vec_box)] // Box<u32> is the point: real Drop glue for the non-Copy model

use arbitrary::Arbitrary;
use std::collections::{HashMap, HashSet};
use turbocow::{
    EcoBytes, EcoMap, EcoSet, EcoStr, EcoString, EcoVec, SmallMap, SmallSet, SmallVec,
};

/// Inline capacity used for the inline-storage types. Deliberately small so op
/// sequences cross the inline⇄heap (and spill) boundaries frequently.
pub const N: usize = 4;

// ─────────────────────────────────────────────────────────────────────────
// Vector ops (shared by SmallVec and EcoVec; std `Vec` is the oracle)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug, Clone)]
pub enum VecOp {
    Push(u32),
    Pop,
    Insert {
        index: usize,
        value: u32,
    },
    Remove(usize),
    Truncate(usize),
    Clear,
    ExtendFromSlice(Vec<u32>),
    Reserve(usize),
    RetainEven,
    RetainNone,
    /// Clone, mutate the clone, and assert the original is unchanged (exercises
    /// clone-on-write / refcount sharing).
    CloneMutateCheck(u32),
    Replace {
        index: usize,
        value: u32,
    },
    Drain {
        start: usize,
        end: usize,
        consume: usize,
    },
}

/// SmallVec also supports `swap_remove` and a zero-copy Referenced variant.
#[derive(Arbitrary, Debug, Clone)]
pub enum SmallVecOp {
    Common(VecOp),
    SwapRemove(usize),
    MakeMut,
}

fn bound(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index % len }
}

/// Differential check: a turbocow `SmallVec<u32, N>` against a `Vec<u32>`.
pub fn check_smallvec(ops: &[SmallVecOp]) {
    let mut sv: SmallVec<'static, u32, N> = SmallVec::new();
    let mut model: Vec<u32> = Vec::new();

    for op in ops {
        match op {
            SmallVecOp::Common(c) => apply_vec_op(&mut sv, &mut model, c),
            SmallVecOp::SwapRemove(i) => {
                if !model.is_empty() {
                    let i = bound(*i, model.len());
                    assert_eq!(sv.swap_remove(i), model.swap_remove(i));
                }
            }
            SmallVecOp::MakeMut => {
                sv.make_mut();
            }
        }
        assert_eq!(sv.as_slice(), model.as_slice(), "smallvec mismatch after {op:?}");
        assert_eq!(sv.len(), model.len());
        assert_eq!(sv.is_empty(), model.is_empty());
    }
}

/// Differential check starting from `SmallVec::from_ref` (Referenced variant).
///
/// The initial `data` has 8 elements, which exceeds `N` (= 4) so the
/// `SmallVec` starts life as a Referenced variant. The first mutation triggers
/// `materialise_referenced` (bug #12 fix path) — either to Inline or Heap
/// depending on the new length. The rest of the op sequence then exercises
/// the post-materialisation storage.
pub fn check_smallvec_from_ref<const N: usize>(ops: &[SmallVecOp]) {
    let data: Vec<u32> = (0..8).collect();
    let mut sv: SmallVec<'_, u32, N> = SmallVec::from_ref(&data);
    let mut model: Vec<u32> = data.clone();

    for op in ops {
        match op {
            SmallVecOp::Common(c) => {
                // `drain` panics on the Referenced variant (by design — see
                // SmallVec::drain). Force materialisation before draining so
                // the op sequence still exercises the post-materialisation
                // storage path. Other mutating ops materialise on their own.
                if matches!(c, VecOp::Drain { .. }) {
                    sv.make_mut();
                }
                apply_vec_op(&mut sv, &mut model, c);
            }
            SmallVecOp::SwapRemove(i) => {
                if !model.is_empty() {
                    let i = bound(*i, model.len());
                    assert_eq!(sv.swap_remove(i), model.swap_remove(i));
                }
            }
            SmallVecOp::MakeMut => {
                sv.make_mut();
            }
        }
        assert_eq!(
            sv.as_slice(),
            model.as_slice(),
            "smallvec_from_ref mismatch after {op:?}"
        );
        assert_eq!(sv.len(), model.len());
        assert_eq!(sv.is_empty(), model.is_empty());
    }
}

/// Differential check: a turbocow `EcoVec<u32>` against a `Vec<u32>`.
pub fn check_ecovec(ops: &[VecOp]) {
    let mut ev: EcoVec<u32> = EcoVec::new();
    let mut model: Vec<u32> = Vec::new();

    for op in ops {
        apply_vec_op(&mut ev, &mut model, op);
        assert_eq!(ev.as_slice(), model.as_slice(), "ecovec mismatch after {op:?}");
        assert_eq!(ev.len(), model.len());
        assert_eq!(ev.is_empty(), model.is_empty());
    }
}

/// Apply a `VecOp` to any vector exposing the shared push/pop/insert/... API and
/// to the `Vec` oracle. The bound on `trait VecLike` keeps SmallVec and EcoVec
/// going through one code path.
fn apply_vec_op<V: VecLike>(v: &mut V, model: &mut Vec<u32>, op: &VecOp) {
    match op {
        VecOp::Push(x) => {
            v.push(*x);
            model.push(*x);
        }
        VecOp::Pop => {
            assert_eq!(v.pop(), model.pop());
        }
        VecOp::Insert { index, value } => {
            let i = bound(*index, model.len() + 1);
            v.insert(i, *value);
            model.insert(i, *value);
        }
        VecOp::Remove(index) => {
            if !model.is_empty() {
                let i = bound(*index, model.len());
                assert_eq!(v.remove(i), model.remove(i));
            }
        }
        VecOp::Truncate(n) => {
            let n = n % 32;
            v.truncate(n);
            model.truncate(n);
        }
        VecOp::Clear => {
            v.clear();
            model.clear();
        }
        VecOp::ExtendFromSlice(s) => {
            v.extend_from_slice(s);
            model.extend_from_slice(s);
        }
        VecOp::Reserve(n) => {
            v.reserve(n % 128);
        }
        VecOp::RetainEven => {
            v.retain_even();
            model.retain(|x| x % 2 == 0);
        }
        VecOp::RetainNone => {
            v.retain_none();
            model.retain(|_| false);
        }
        VecOp::CloneMutateCheck(x) => {
            let mut c = v.clone_box();
            c.push(*x);
            // Mutating the clone must not disturb the original.
            assert_eq!(v.slice(), model.as_slice());
        }
        VecOp::Replace { index, value } => {
            if !model.is_empty() {
                let i = bound(*index, model.len());
                assert_eq!(
                    v.replace(i, *value),
                    std::mem::replace(&mut model[i], *value)
                );
            }
        }
        VecOp::Drain { start, end, consume } => {
            let len = model.len();
            if len > 0 {
                let s = bound(*start, len + 1);
                let e = if *end % (len + 1) >= s { *end % (len + 1) } else { s };
                let c = *consume % (e - s + 1).max(1);
                v.drain_partial(s, e, c);
                model.drain(s..e).for_each(drop);
            }
        }
    }
}

/// Minimal shared surface so one `apply_vec_op` covers both turbocow vectors.
pub trait VecLike: Clone {
    fn push(&mut self, v: u32);
    fn pop(&mut self) -> Option<u32>;
    fn insert(&mut self, i: usize, v: u32);
    fn remove(&mut self, i: usize) -> u32;
    fn truncate(&mut self, n: usize);
    fn clear(&mut self);
    fn extend_from_slice(&mut self, s: &[u32]);
    fn reserve(&mut self, n: usize);
    // EcoVec::retain takes FnMut(&mut T) while SmallVec::retain takes
    // FnMut(&T), so expose the two concrete predicates instead of a closure.
    fn retain_even(&mut self);
    fn retain_none(&mut self);
    fn slice(&self) -> &[u32];
    fn clone_box(&self) -> Self;
    fn replace(&mut self, i: usize, v: u32) -> u32;
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize);
}

impl<'a, const N: usize> VecLike for SmallVec<'a, u32, N> {
    fn push(&mut self, v: u32) {
        SmallVec::push(self, v)
    }
    fn pop(&mut self) -> Option<u32> {
        SmallVec::pop(self)
    }
    fn insert(&mut self, i: usize, v: u32) {
        SmallVec::insert(self, i, v)
    }
    fn remove(&mut self, i: usize) -> u32 {
        SmallVec::remove(self, i)
    }
    fn truncate(&mut self, n: usize) {
        SmallVec::truncate(self, n)
    }
    fn clear(&mut self) {
        SmallVec::clear(self)
    }
    fn extend_from_slice(&mut self, s: &[u32]) {
        SmallVec::extend_from_slice(self, s)
    }
    fn reserve(&mut self, n: usize) {
        SmallVec::reserve(self, n)
    }
    fn retain_even(&mut self) {
        SmallVec::retain(self, |x| x % 2 == 0)
    }
    fn retain_none(&mut self) {
        SmallVec::retain(self, |_| false)
    }
    fn slice(&self) -> &[u32] {
        self.as_slice()
    }
    fn clone_box(&self) -> Self {
        self.clone()
    }
    fn replace(&mut self, i: usize, v: u32) -> u32 {
        let old = SmallVec::remove(self, i);
        SmallVec::insert(self, i, v);
        old
    }
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize) {
        let mut d = SmallVec::drain(self, start..end);
        for _ in 0..consume {
            d.next();
        }
        drop(d);
    }
}

impl VecLike for EcoVec<u32> {
    fn push(&mut self, v: u32) {
        EcoVec::push(self, v)
    }
    fn pop(&mut self) -> Option<u32> {
        EcoVec::pop(self)
    }
    fn insert(&mut self, i: usize, v: u32) {
        EcoVec::insert(self, i, v)
    }
    fn remove(&mut self, i: usize) -> u32 {
        EcoVec::remove(self, i)
    }
    fn truncate(&mut self, n: usize) {
        EcoVec::truncate(self, n)
    }
    fn clear(&mut self) {
        EcoVec::clear(self)
    }
    fn extend_from_slice(&mut self, s: &[u32]) {
        EcoVec::extend_from_slice(self, s)
    }
    fn reserve(&mut self, n: usize) {
        EcoVec::reserve(self, n)
    }
    fn retain_even(&mut self) {
        // EcoVec::retain hands out `&mut T`.
        EcoVec::retain(self, |x: &mut u32| *x % 2 == 0)
    }
    fn retain_none(&mut self) {
        EcoVec::retain(self, |_: &mut u32| false)
    }
    fn slice(&self) -> &[u32] {
        self.as_slice()
    }
    fn clone_box(&self) -> Self {
        self.clone()
    }
    fn replace(&mut self, i: usize, v: u32) -> u32 {
        let old = self.as_slice()[i];
        EcoVec::replace(self, i, v);
        old
    }
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize) {
        let mut d = EcoVec::drain(self, start..end);
        for _ in 0..consume {
            d.next();
        }
        drop(d);
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Non-Copy variant: Box<u32>. Exposes drop-related bugs (double-drop, leak,
// drop-order) that the Copy u32 model cannot catch. Run under Miri for full
// value — Miri catches the UB that Copy types mask.
// ─────────────────────────────────────────────────────────────────────────

/// Differential check: a turbocow `EcoVec<Box<u32>>` against `Vec<Box<u32>>`.
pub fn check_ecovec_boxed(ops: &[VecOp]) {
    let mut ev: EcoVec<Box<u32>> = EcoVec::new();
    let mut model: Vec<Box<u32>> = Vec::new();

    for op in ops {
        apply_vec_op_boxed(&mut ev, &mut model, op);
        // Compare by dereferencing Box<u32> → u32.
        let sut: Vec<u32> = ev.iter().map(|b| **b).collect();
        let oracle: Vec<u32> = model.iter().map(|b| **b).collect();
        assert_eq!(sut, oracle, "ecovec_boxed mismatch after {op:?}");
        assert_eq!(ev.len(), model.len());
    }
}

/// Differential check: a turbocow `SmallVec<'static, Box<u32>, N>` against
/// `Vec<Box<u32>>`.
pub fn check_smallvec_boxed<const N: usize>(ops: &[SmallVecOp]) {
    let mut sv: SmallVec<'static, Box<u32>, N> = SmallVec::new();
    let mut model: Vec<Box<u32>> = Vec::new();

    for op in ops {
        match op {
            SmallVecOp::Common(c) => apply_vec_op_boxed(&mut sv, &mut model, c),
            SmallVecOp::SwapRemove(i) => {
                if !model.is_empty() {
                    let i = bound(*i, model.len());
                    let a = sv.swap_remove(i);
                    let b = model.swap_remove(i);
                    assert_eq!(*a, *b);
                }
            }
            SmallVecOp::MakeMut => {
                sv.make_mut();
            }
        }
        let sut: Vec<u32> = sv.iter().map(|b| **b).collect();
        let oracle: Vec<u32> = model.iter().map(|b| **b).collect();
        assert_eq!(sut, oracle, "smallvec_boxed mismatch after {op:?}");
        assert_eq!(sv.len(), model.len());
    }
}

/// Same as `apply_vec_op` but for `Box<u32>`. Separated from the `VecLike`
/// trait (which is u32-specific) to avoid making the model generic over `T`.
fn apply_vec_op_boxed(v: &mut impl BoxVecLike, model: &mut Vec<Box<u32>>, op: &VecOp) {
    match op {
        VecOp::Push(x) => {
            v.push(Box::new(*x));
            model.push(Box::new(*x));
        }
        VecOp::Pop => {
            assert_eq!(v.pop().map(|b| *b), model.pop().map(|b| *b));
        }
        VecOp::Insert { index, value } => {
            let i = bound(*index, model.len() + 1);
            v.insert(i, Box::new(*value));
            model.insert(i, Box::new(*value));
        }
        VecOp::Remove(index) => {
            if !model.is_empty() {
                let i = bound(*index, model.len());
                assert_eq!(*v.remove(i), *model.remove(i));
            }
        }
        VecOp::Truncate(n) => {
            let n = n % 32;
            v.truncate(n);
            model.truncate(n);
        }
        VecOp::Clear => {
            v.clear();
            model.clear();
        }
        VecOp::ExtendFromSlice(s) => {
            v.extend_from_slice_boxed(s);
            model.extend(s.iter().map(|&x| Box::new(x)));
        }
        VecOp::Reserve(n) => {
            v.reserve(n % 128);
        }
        VecOp::RetainEven => {
            v.retain_boxed(|x| *x % 2 == 0);
            model.retain(|b| **b % 2 == 0);
        }
        VecOp::RetainNone => {
            v.clear();
            model.clear();
        }
        VecOp::CloneMutateCheck(x) => {
            let mut c = v.clone_box();
            c.push(Box::new(*x));
            // Mutating clone must not disturb original.
        }
        VecOp::Replace { index, value } => {
            if !model.is_empty() {
                let i = bound(*index, model.len());
                let old_sut = v.replace(i, Box::new(*value));
                let old_model = std::mem::replace(&mut model[i], Box::new(*value));
                assert_eq!(*old_sut, *old_model);
            }
        }
        VecOp::Drain { start, end, consume } => {
            let len = model.len();
            if len > 0 {
                let s = bound(*start, len + 1);
                let e = if *end % (len + 1) >= s { *end % (len + 1) } else { s };
                let c = *consume % (e - s + 1).max(1);
                v.drain_partial(s, e, c);
                model.drain(s..e).for_each(drop);
            }
        }
    }
}

/// `Box<u32>`-specific trait, separate from `VecLike` (which is u32-specific)
/// to avoid making the model generic over `T`.
pub trait BoxVecLike: Clone {
    fn push(&mut self, v: Box<u32>);
    fn pop(&mut self) -> Option<Box<u32>>;
    fn insert(&mut self, i: usize, v: Box<u32>);
    fn remove(&mut self, i: usize) -> Box<u32>;
    fn truncate(&mut self, n: usize);
    fn clear(&mut self);
    fn extend_from_slice_boxed(&mut self, s: &[u32]);
    fn reserve(&mut self, n: usize);
    fn retain_boxed(&mut self, f: impl FnMut(&u32) -> bool);
    fn replace(&mut self, i: usize, v: Box<u32>) -> Box<u32>;
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize);
    fn clone_box(&self) -> Self;
}

impl BoxVecLike for EcoVec<Box<u32>> {
    fn push(&mut self, v: Box<u32>) {
        EcoVec::push(self, v)
    }
    fn pop(&mut self) -> Option<Box<u32>> {
        EcoVec::pop(self)
    }
    fn insert(&mut self, i: usize, v: Box<u32>) {
        EcoVec::insert(self, i, v)
    }
    fn remove(&mut self, i: usize) -> Box<u32> {
        EcoVec::remove(self, i)
    }
    fn truncate(&mut self, n: usize) {
        EcoVec::truncate(self, n)
    }
    fn clear(&mut self) {
        EcoVec::clear(self)
    }
    fn extend_from_slice_boxed(&mut self, s: &[u32]) {
        let boxed: Vec<Box<u32>> = s.iter().map(|&x| Box::new(x)).collect();
        EcoVec::extend_from_slice(self, &boxed);
    }
    fn reserve(&mut self, n: usize) {
        EcoVec::reserve(self, n)
    }
    fn retain_boxed(&mut self, mut f: impl FnMut(&u32) -> bool) {
        EcoVec::retain(self, |b: &mut Box<u32>| f(b));
    }
    /// `EcoVec::replace` returns `()` (drops old value internally). Use
    /// remove + insert to recover the old value for differential comparison.
    fn replace(&mut self, i: usize, v: Box<u32>) -> Box<u32> {
        let old = EcoVec::remove(self, i);
        EcoVec::insert(self, i, v);
        old
    }
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize) {
        let mut d = EcoVec::drain(self, start..end);
        for _ in 0..consume {
            d.next();
        }
        drop(d);
    }
    fn clone_box(&self) -> Self {
        self.clone()
    }
}

impl<const N: usize> BoxVecLike for SmallVec<'static, Box<u32>, N> {
    fn push(&mut self, v: Box<u32>) {
        SmallVec::push(self, v)
    }
    fn pop(&mut self) -> Option<Box<u32>> {
        SmallVec::pop(self)
    }
    fn insert(&mut self, i: usize, v: Box<u32>) {
        SmallVec::insert(self, i, v)
    }
    fn remove(&mut self, i: usize) -> Box<u32> {
        SmallVec::remove(self, i)
    }
    fn truncate(&mut self, n: usize) {
        SmallVec::truncate(self, n)
    }
    fn clear(&mut self) {
        SmallVec::clear(self)
    }
    fn extend_from_slice_boxed(&mut self, s: &[u32]) {
        let boxed: Vec<Box<u32>> = s.iter().map(|&x| Box::new(x)).collect();
        SmallVec::extend_from_slice(self, &boxed);
    }
    fn reserve(&mut self, n: usize) {
        SmallVec::reserve(self, n)
    }
    fn retain_boxed(&mut self, mut f: impl FnMut(&u32) -> bool) {
        SmallVec::retain(self, |b| f(b));
    }
    /// SmallVec has no native `replace`; use remove + insert (mirrors the
    /// existing `VecLike` impl for `SmallVec<u32, N>`).
    fn replace(&mut self, i: usize, v: Box<u32>) -> Box<u32> {
        let old = SmallVec::remove(self, i);
        SmallVec::insert(self, i, v);
        old
    }
    fn drain_partial(&mut self, start: usize, end: usize, consume: usize) {
        let mut d = SmallVec::drain(self, start..end);
        for _ in 0..consume {
            d.next();
        }
        drop(d);
    }
    fn clone_box(&self) -> Self {
        self.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────
// String ops (EcoString vs std `String`)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug, Clone)]
pub enum StringOp {
    PushChar(char),
    PushStr(String),
    Pop,
    Insert { index: usize, ch: char },
    InsertStr { index: usize, s: String },
    Remove(usize),
    Truncate(usize),
    Clear,
    Reserve(usize),
}

/// Largest char boundary `<= raw % (len + 1)` — a valid `insert`/`truncate`
/// index (std `String` panics on a non-boundary, so both sides must agree).
fn boundary_le(s: &str, raw: usize) -> usize {
    let mut i = raw % (s.len() + 1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn check_ecostring(ops: &[StringOp]) {
    let mut es = EcoString::new();
    let mut model = String::new();

    for op in ops {
        match op {
            StringOp::PushChar(c) => {
                es.push(*c);
                model.push(*c);
            }
            StringOp::PushStr(s) => {
                es.push_str(s);
                model.push_str(s);
            }
            StringOp::Pop => {
                assert_eq!(es.pop(), model.pop());
            }
            StringOp::Insert { index, ch } => {
                let i = boundary_le(&model, *index);
                es.insert(i, *ch);
                model.insert(i, *ch);
            }
            StringOp::InsertStr { index, s } => {
                let i = boundary_le(&model, *index);
                es.insert_str(i, s);
                model.insert_str(i, s);
            }
            StringOp::Remove(index) => {
                if !model.is_empty() {
                    // A char-start strictly inside the string.
                    let mut i = index % model.len();
                    while !model.is_char_boundary(i) {
                        i -= 1;
                    }
                    assert_eq!(es.remove(i), model.remove(i));
                }
            }
            StringOp::Truncate(n) => {
                let n = boundary_le(&model, *n);
                es.truncate(n);
                model.truncate(n);
            }
            StringOp::Clear => {
                es.clear();
                model.clear();
            }
            StringOp::Reserve(n) => {
                let n = n % 128;
                es.reserve(n);
                model.reserve(n);
                assert!(
                    es.capacity() - es.len() >= n,
                    "reserve({n}) left too little room"
                );
            }
        }
        assert!(es.capacity() >= es.len());
        assert_eq!(es.as_str(), model.as_str(), "ecostring mismatch after {op:?}");
        assert_eq!(es.len(), model.len());
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Byte-string ops (EcoBytes vs std `Vec<u8>`)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug, Clone)]
pub enum BytesOp {
    Push(u8),
    Extend(Vec<u8>),
    Pop,
    Truncate(usize),
    Clear,
    Insert {
        index: usize,
        byte: u8,
    },
    InsertSlice {
        index: usize,
        bytes: Vec<u8>,
    },
    Remove(usize),
    Reserve(usize),
    /// Mutate a clone (shared allocation → copy-on-write) and check that the
    /// original is unaffected.
    CloneMutateCheck(u8),
}

pub fn check_ecobytes(ops: &[BytesOp]) {
    check_ecobytes_from(EcoBytes::new(), ops);
}

/// Differential check starting from `EcoBytes::from_static` (Referenced
/// variant), once with data that fits inline and once with data that does
/// not, so the first mutation materialises to Inline or Spilled respectively.
pub fn check_ecobytes_referenced(ops: &[BytesOp]) {
    check_ecobytes_from(EcoBytes::from_static(b"short"), ops);
    check_ecobytes_from(
        EcoBytes::from_static(b"a borrowed byte buffer longer than inline"),
        ops,
    );
}

pub fn check_ecobytes_from(start: EcoBytes, ops: &[BytesOp]) {
    let mut model: Vec<u8> = start.to_vec();
    let mut bs = start;

    for op in ops {
        match op {
            BytesOp::Push(b) => {
                bs.push(*b);
                model.push(*b);
            }
            BytesOp::Extend(bytes) => {
                bs.extend_from_slice(bytes);
                model.extend_from_slice(bytes);
            }
            BytesOp::Pop => {
                assert_eq!(bs.pop(), model.pop());
            }
            BytesOp::Truncate(n) => {
                // Bound to `[0, len]` so the op actually shrinks (a raw usize is
                // almost always > len, i.e. a no-op). EcoBytes::truncate and
                // Vec::truncate are both no-ops for n >= len, so this is faithful.
                let n = *n % (model.len() + 1);
                bs.truncate(n);
                model.truncate(n);
            }
            BytesOp::Clear => {
                bs.clear();
                model.clear();
            }
            BytesOp::Insert { index, byte } => {
                let i = index % (model.len() + 1);
                bs.insert(i, *byte);
                model.insert(i, *byte);
            }
            BytesOp::InsertSlice { index, bytes } => {
                let i = index % (model.len() + 1);
                bs.insert_slice(i, bytes);
                model.splice(i..i, bytes.iter().copied());
            }
            BytesOp::Remove(index) => {
                if !model.is_empty() {
                    let i = index % model.len();
                    assert_eq!(bs.remove(i), model.remove(i));
                }
            }
            BytesOp::Reserve(n) => {
                let n = n % 128;
                bs.reserve(n);
                assert!(
                    bs.capacity() - bs.len() >= n,
                    "reserve({n}) left too little room"
                );
            }
            BytesOp::CloneMutateCheck(b) => {
                let mut clone = bs.clone();
                clone.push(*b);
                assert_eq!(bs.as_slice(), model.as_slice(), "clone mutation leaked");
                assert_eq!(clone.last(), Some(b));
            }
        }
        assert_eq!(bs.as_slice(), model.as_slice(), "ecobytes mismatch after {op:?}");
        assert_eq!(bs.len(), model.len());
        assert!(bs.capacity() >= bs.len());
        assert!(!bs.is_inline() || bs.len() <= EcoBytes::INLINE_LIMIT);
    }
}

/// Differential check starting from `EcoStr::from(borrow)` (Referenced variant).
///
/// The initial borrow exceeds the inline limit (15 bytes) so the `EcoStr`
/// starts life as a Referenced variant. The first length-changing mutation
/// triggers `materialise_referenced` — either to Inline or Spilled depending
/// on the new length.
pub fn check_ecostr_referenced(ops: &[StringOp]) {
    let source = String::from("hello world, this is a borrowed string!");
    let mut es: EcoStr<'_> = EcoStr::from(source.as_str());
    let mut model = source.clone();

    for op in ops {
        match op {
            StringOp::PushChar(c) => {
                es.push(*c);
                model.push(*c);
            }
            StringOp::PushStr(s) => {
                es.push_str(s);
                model.push_str(s);
            }
            StringOp::Pop => {
                assert_eq!(es.pop(), model.pop());
            }
            StringOp::Insert { index, ch } => {
                let i = boundary_le(&model, *index);
                es.insert(i, *ch);
                model.insert(i, *ch);
            }
            StringOp::InsertStr { index, s } => {
                let i = boundary_le(&model, *index);
                es.insert_str(i, s);
                model.insert_str(i, s);
            }
            StringOp::Remove(index) => {
                if !model.is_empty() {
                    let mut i = index % model.len();
                    while !model.is_char_boundary(i) {
                        i -= 1;
                    }
                    assert_eq!(es.remove(i), model.remove(i));
                }
            }
            StringOp::Truncate(n) => {
                let n = boundary_le(&model, *n);
                es.truncate(n);
                model.truncate(n);
            }
            StringOp::Clear => {
                es.clear();
                model.clear();
            }
            StringOp::Reserve(n) => {
                let n = n % 128;
                es.reserve(n);
                model.reserve(n);
                assert!(
                    es.capacity() - es.len() >= n,
                    "reserve({n}) left too little room"
                );
            }
        }
        assert!(es.capacity() >= es.len());
        assert_eq!(
            es.as_str(),
            model.as_str(),
            "ecostr_referenced mismatch after {op:?}"
        );
        assert_eq!(es.len(), model.len());
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Map ops (SmallMap / EcoMap vs std `HashMap`). Small key space → collisions,
// overwrites and (for SmallMap) inline⇄spill transitions.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug, Clone)]
pub enum MapOp {
    Insert(u8, u16),
    Remove(u8),
    Get(u8),
    ContainsKey(u8),
    Clear,
    /// Retain only keys whose value's low byte equals the supplied byte.
    /// Exercises the panic-free retain path differentially. (Panic-mode
    /// retain is covered by unit tests; proptest + catch_unwind don't mix.)
    Retain(u8),
    Entry(u8, u16),
}

pub fn check_smallmap<const N: usize>(ops: &[MapOp]) {
    let mut m: SmallMap<u8, u16, N> = SmallMap::new();
    let mut model: HashMap<u8, u16> = HashMap::new();
    run_map_ops(ops, &mut model, |op| match op {
        MapOp::Insert(k, v) => MapRet::Opt(m.insert(*k, *v)),
        MapOp::Remove(k) => MapRet::Opt(m.remove(k)),
        MapOp::Get(k) => MapRet::OptRef(m.get(k).copied()),
        MapOp::ContainsKey(k) => MapRet::Bool(m.contains_key(k)),
        MapOp::Clear => {
            m.clear();
            MapRet::Unit
        }
        MapOp::Retain(mask) => {
            m.retain(|_k, v| (*v as u8) == *mask);
            MapRet::Unit
        }
        MapOp::Entry(k, v) => {
            m.entry(*k).or_insert(*v);
            MapRet::Unit
        }
    });
    for k in 0u8..=u8::MAX {
        assert_eq!(m.get(&k).copied(), model.get(&k).copied());
    }
    assert_eq!(m.len(), model.len());
}

pub fn check_ecomap(ops: &[MapOp]) {
    let mut m: EcoMap<u8, u16> = EcoMap::new();
    let mut model: HashMap<u8, u16> = HashMap::new();
    run_map_ops(ops, &mut model, |op| match op {
        MapOp::Insert(k, v) => MapRet::Opt(m.insert(*k, *v)),
        MapOp::Remove(k) => MapRet::Opt(m.remove(k)),
        MapOp::Get(k) => MapRet::OptRef(m.get(k).copied()),
        MapOp::ContainsKey(k) => MapRet::Bool(m.contains_key(k)),
        MapOp::Clear => {
            m.clear();
            MapRet::Unit
        }
        MapOp::Retain(mask) => {
            m.retain(|_k, v| (*v as u8) == *mask);
            MapRet::Unit
        }
        MapOp::Entry(k, v) => {
            m.entry(*k).or_insert(*v);
            MapRet::Unit
        }
    });
    for k in 0u8..=u8::MAX {
        assert_eq!(m.get(&k).copied(), model.get(&k).copied());
    }
    assert_eq!(m.len(), model.len());
}

enum MapRet {
    Opt(Option<u16>),
    OptRef(Option<u16>),
    Bool(bool),
    Unit,
}

fn run_map_ops(
    ops: &[MapOp],
    model: &mut HashMap<u8, u16>,
    mut apply: impl FnMut(&MapOp) -> MapRet,
) {
    for op in ops {
        let got = apply(op);
        match (op, got) {
            (MapOp::Insert(k, v), MapRet::Opt(r)) => assert_eq!(r, model.insert(*k, *v)),
            (MapOp::Remove(k), MapRet::Opt(r)) => assert_eq!(r, model.remove(k)),
            (MapOp::Get(k), MapRet::OptRef(r)) => assert_eq!(r, model.get(k).copied()),
            (MapOp::ContainsKey(k), MapRet::Bool(r)) => {
                assert_eq!(r, model.contains_key(k))
            }
            (MapOp::Clear, MapRet::Unit) => model.clear(),
            (MapOp::Retain(mask), MapRet::Unit) => {
                model.retain(|_k, v| (*v as u8) == *mask)
            }
            (MapOp::Entry(k, v), MapRet::Unit) => {
                model.entry(*k).or_insert(*v);
            }
            _ => unreachable!("op/return mismatch"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Set ops (SmallSet / EcoSet vs std `HashSet`).
// ─────────────────────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug, Clone)]
pub enum SetOp {
    Insert(u8),
    Remove(u8),
    Contains(u8),
    Clear,
}

pub fn check_smallset<const N: usize>(ops: &[SetOp]) {
    let mut s: SmallSet<u8, N> = SmallSet::new();
    let mut model: HashSet<u8> = HashSet::new();
    for op in ops {
        match op {
            SetOp::Insert(t) => {
                assert_eq!(s.insert(*t), model.insert(*t), "after {op:?}")
            }
            SetOp::Remove(t) => assert_eq!(s.remove(t), model.remove(t), "after {op:?}"),
            SetOp::Contains(t) => {
                assert_eq!(s.contains(t), model.contains(t), "after {op:?}")
            }
            SetOp::Clear => {
                s.clear();
                model.clear();
            }
        }
        assert_eq!(s.len(), model.len());
    }
    for t in 0u8..=u8::MAX {
        assert_eq!(s.contains(&t), model.contains(&t));
    }
}

pub fn check_ecoset(ops: &[SetOp]) {
    let mut s: EcoSet<u8> = EcoSet::new();
    let mut model: HashSet<u8> = HashSet::new();
    for op in ops {
        match op {
            SetOp::Insert(t) => {
                assert_eq!(s.insert(*t), model.insert(*t), "after {op:?}")
            }
            SetOp::Remove(t) => assert_eq!(s.remove(t), model.remove(t), "after {op:?}"),
            SetOp::Contains(t) => {
                assert_eq!(s.contains(t), model.contains(t), "after {op:?}")
            }
            SetOp::Clear => {
                s.clear();
                model.clear();
            }
        }
        assert_eq!(s.len(), model.len());
    }
    for t in 0u8..=u8::MAX {
        assert_eq!(s.contains(&t), model.contains(&t));
    }
}
