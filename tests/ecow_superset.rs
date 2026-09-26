//! Compile-only proof that turbocow's public surface is a **complete superset**
//! of ecow 0.3.1's public API.
//!
//! Every assertion below pins one concrete public item of ecow 0.3.1
//! (`src/{lib,vec,string,bytes}.rs` of the `ecow-0.3.1` crate), exercised through the
//! corresponding turbocow type / path. If turbocow ever drops or changes the
//! signature of one of these items, this file stops compiling and the superset
//! guarantee is known to have regressed.
//!
//! The file is organized into sections mirroring the ecow source:
//!   * EcoVec — inherent methods
//!   * EcoVec — trait impls
//!   * EcoVec — iterator types (`IntoIter`, `Drain`, `Splice`)
//!   * EcoString — inherent methods
//!   * EcoString — trait impls
//!   * EcoBytes — inherent methods
//!   * EcoBytes — trait impls
//!   * Macros (`eco_vec!`, `eco_format!`)
//!   * `ToEcoString` / `AsRef`
//!   * Module paths & re-exports
//!   * serde (feature-gated)
//!
//! No runtime assertions are needed — successful compilation *is* the test.
//! turbocow's public `EcoVec<T>` is `EcoVec<T, Global>` and its `EcoString`
//! matches ecow's; both are checked through their public re-export paths.

#![allow(dead_code, unused_imports, clippy::all)]

use core::cmp::Ordering;
use std::borrow::Cow;

use turbocow::{EcoBytes, EcoString, EcoVec, eco_format, eco_vec};

// Generic helpers used to assert trait bounds without running anything.
fn assert_default<T: Default>() {}
fn assert_clone<T: Clone>() {}
fn assert_debug<T: core::fmt::Debug>() {}
fn assert_display<T: core::fmt::Display>() {}
fn assert_hash<T: core::hash::Hash>() {}
fn assert_eq_trait<T: Eq>() {}
fn assert_partial_eq<T: PartialEq<R> + ?Sized, R: ?Sized>() {}
fn assert_ord<T: Ord>() {}
fn assert_partial_ord<T: PartialOrd>() {}
fn assert_deref<T: core::ops::Deref<Target = U> + ?Sized, U: ?Sized>() {}
fn assert_borrow<T: core::borrow::Borrow<U>, U: ?Sized>() {}
fn assert_as_ref<T: AsRef<U>, U: ?Sized>() {}
fn assert_from<T: From<F>, F>() {}
fn assert_try_from<T: TryFrom<F>, F>() {}
fn assert_try_from_err<T: TryFrom<F, Error = E>, F, E>() {}
fn assert_from_iter<T: FromIterator<I>, I>() {}
fn assert_extend<T: Extend<I>, I>() {}
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_into_iterator<T: IntoIterator<Item = I>, I>() {}
fn assert_iterator<T: Iterator<Item = I>, I>() {}
fn assert_double_ended<T: DoubleEndedIterator>() {}
fn assert_exact_size<T: ExactSizeIterator>() {}
fn assert_fromstr<T: core::str::FromStr>() {}
fn assert_add<T: core::ops::Add<R, Output = O>, R, O>() {}
fn assert_add_assign<T: core::ops::AddAssign<R>, R>() {}

// =====================================================================
// EcoVec<T> — inherent methods (ecow vec.rs)
// =====================================================================

// `pub const fn new() -> Self` — usable in const context.
const _ECOVEC_NEW: EcoVec<i32> = EcoVec::new();

fn _ecovec_inherent(v: &mut EcoVec<i32>) {
    // `pub fn with_capacity(usize) -> Self`
    let mut owned: EcoVec<i32> = EcoVec::with_capacity(8);
    // `pub const fn is_empty(&self) -> bool`
    let _b: bool = v.is_empty();
    // `pub const fn len(&self) -> usize`
    let _n: usize = v.len();
    // `pub fn capacity(&self) -> usize`
    let _c: usize = v.capacity();
    // `pub fn as_slice(&self) -> &[T]`
    let _s: &[i32] = v.as_slice();
    // `pub fn clear(&mut self)`
    owned.clear();
    // `pub fn make_mut(&mut self) -> &mut [T]` (T: Clone)
    let _m: &mut [i32] = v.make_mut();
    // `pub fn push(&mut self, value: T)` (T: Clone)
    v.push(1);
    // `pub fn pop(&mut self) -> Option<T>` (T: Clone)
    let _p: Option<i32> = v.pop();
    // `pub fn insert(&mut self, index: usize, value: T)` (T: Clone)
    v.insert(0, 7);
    // `pub fn remove(&mut self, index: usize) -> T` (T: Clone)
    let _r: i32 = v.remove(0);
    // `pub fn retain<F: FnMut(&mut T) -> bool>(&mut self, f: F)` (T: Clone)
    v.retain(|x| *x > 0);
    // `pub fn truncate(&mut self, target: usize)` (T: Clone)
    v.truncate(0);
    // `pub fn reserve(&mut self, additional: usize)` (T: Clone)
    v.reserve(4);
    // `pub fn extend_from_slice(&mut self, slice: &[T])` (T: Clone)
    v.extend_from_slice(&[1, 2, 3]);
    // `pub fn is_unique(&mut self) -> bool`
    let _u: bool = v.is_unique();
    // `pub fn from_elem(value: T, n: usize) -> Self` (T: Clone)
    let _fe: EcoVec<i32> = EcoVec::from_elem(0, 3);
}

// `pub unsafe fn extend_from_trusted<I>(&mut self, iter: I)` where
// `I: IntoIterator<Item = T>, I::IntoIter: ExactSizeIterator`.
fn _ecovec_extend_from_trusted(v: &mut EcoVec<i32>) {
    unsafe {
        v.extend_from_trusted([1, 2, 3]);
    }
}

// `pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> Drain<'_, T>`
fn _ecovec_drain(v: &mut EcoVec<i32>) {
    let _d: turbocow::vec::Drain<'_, i32> = v.drain(0..0);
}

// `pub fn splice<R, I>(&mut self, range: R, replace_with: I)
//      -> Splice<'_, I::IntoIter>`
fn _ecovec_splice(v: &mut EcoVec<i32>) {
    let _s: turbocow::vec::Splice<'_, core::array::IntoIter<i32, 0>> = v.splice(0..0, []);
}

// =====================================================================
// EcoVec<T> — trait impls (ecow vec.rs)
// =====================================================================

fn _ecovec_traits() {
    // Clone, Default, Debug, Hash, Eq, Ord, PartialOrd
    assert_clone::<EcoVec<i32>>();
    assert_default::<EcoVec<i32>>();
    assert_debug::<EcoVec<i32>>();
    assert_hash::<EcoVec<i32>>();
    assert_eq_trait::<EcoVec<i32>>();
    assert_ord::<EcoVec<i32>>();
    assert_partial_ord::<EcoVec<i32>>();

    // Send + Sync (for Send+Sync T)
    assert_send::<EcoVec<i32>>();
    assert_sync::<EcoVec<i32>>();

    // Deref<Target = [T]>, Borrow<[T]>, AsRef<[T]>
    assert_deref::<EcoVec<i32>, [i32]>();
    assert_borrow::<EcoVec<i32>, [i32]>();
    assert_as_ref::<EcoVec<i32>, [i32]>();

    // PartialEq family
    assert_partial_eq::<EcoVec<i32>, EcoVec<i32>>();
    assert_partial_eq::<EcoVec<i32>, [i32]>();
    assert_partial_eq::<EcoVec<i32>, &[i32]>();
    assert_partial_eq::<EcoVec<i32>, [i32; 3]>();
    assert_partial_eq::<EcoVec<i32>, &[i32; 3]>();
    assert_partial_eq::<EcoVec<i32>, Vec<i32>>();
    // reverse-direction PartialEq impls
    assert_partial_eq::<[i32], EcoVec<i32>>();
    assert_partial_eq::<[i32; 3], EcoVec<i32>>();
    assert_partial_eq::<Vec<i32>, EcoVec<i32>>();

    // From<&[T]>, From<[T; N]>, From<Vec<T>>
    assert_from::<EcoVec<i32>, &[i32]>();
    assert_from::<EcoVec<i32>, [i32; 3]>();
    assert_from::<EcoVec<i32>, Vec<i32>>();

    // TryFrom<EcoVec<T>> for [T; N] (Error = EcoVec<T>)
    assert_try_from::<[i32; 3], EcoVec<i32>>();

    // FromIterator<T>, Extend<T>
    assert_from_iter::<EcoVec<i32>, i32>();
    assert_extend::<EcoVec<i32>, i32>();

    // IntoIterator for value / &
    assert_into_iterator::<EcoVec<i32>, i32>();
    assert_into_iterator::<&EcoVec<i32>, &i32>();
}

// TryFrom error type is exactly `EcoVec<T>`.
fn _ecovec_tryfrom_err(v: EcoVec<i32>) -> Result<[i32; 3], EcoVec<i32>> {
    <[i32; 3]>::try_from(v)
}

// =====================================================================
// EcoVec<T> — iterator types: IntoIter, Drain, Splice (ecow vec.rs)
// =====================================================================

// ecow exposes a single-type-param `vec::IntoIter<T>`.
fn _ecovec_into_iter() -> turbocow::vec::IntoIter<i32> {
    EcoVec::<i32>::new().into_iter()
}

// `IntoIter<T>` is also re-exported at the crate root in turbocow.
fn _crate_root_into_iter() -> turbocow::IntoIter<i32> {
    EcoVec::<i32>::new().into_iter()
}

fn _ecovec_iter_traits() {
    // IntoIter: Iterator<Item = T> + DoubleEndedIterator + ExactSizeIterator + Debug
    assert_iterator::<turbocow::vec::IntoIter<i32>, i32>();
    assert_double_ended::<turbocow::vec::IntoIter<i32>>();
    assert_exact_size::<turbocow::vec::IntoIter<i32>>();
    assert_debug::<turbocow::vec::IntoIter<i32>>();
}

// `IntoIter::as_slice(&self) -> &[T]`
fn _ecovec_into_iter_as_slice(it: &turbocow::vec::IntoIter<i32>) -> &[i32] {
    it.as_slice()
}

// `Drain::as_slice(&self) -> &[T]` and `AsRef<[T]>`.
fn _ecovec_drain_as_slice(v: &mut EcoVec<i32>) {
    let d: turbocow::vec::Drain<'_, i32> = v.drain(0..0);
    let _s: &[i32] = d.as_slice();
    let _r: &[i32] = d.as_ref();
}

// Drain trait set: Iterator + DoubleEndedIterator + ExactSizeIterator +
// FusedIterator + Debug (ecow vendor/vec/drain.rs).
fn _ecovec_drain_traits() {
    fn assert_fused<T: core::iter::FusedIterator>() {}
    assert_iterator::<turbocow::vec::Drain<'_, i32>, i32>();
    assert_double_ended::<turbocow::vec::Drain<'_, i32>>();
    assert_exact_size::<turbocow::vec::Drain<'_, i32>>();
    assert_fused::<turbocow::vec::Drain<'_, i32>>();
    assert_debug::<turbocow::vec::Drain<'_, i32>>();
}

// Splice trait set: Iterator + DoubleEndedIterator + ExactSizeIterator
// (ecow vendor/vec/splice.rs).
fn _ecovec_splice_traits() {
    type Repl = core::array::IntoIter<i32, 0>;
    assert_iterator::<turbocow::vec::Splice<'_, Repl>, i32>();
    assert_double_ended::<turbocow::vec::Splice<'_, Repl>>();
    assert_exact_size::<turbocow::vec::Splice<'_, Repl>>();
}

// =====================================================================
// EcoString — inherent methods (ecow string.rs)
// =====================================================================

// `pub const INLINE_LIMIT: usize`
const _ECOSTRING_INLINE_LIMIT: usize = EcoString::INLINE_LIMIT;

// `pub const fn new() -> Self` — usable in const context.
const _ECOSTRING_NEW: EcoString = EcoString::new();

// `pub const fn inline(&str) -> Self` — usable in const context.
const _ECOSTRING_INLINE: EcoString = EcoString::inline("hi");

// `pub const fn try_inline(&str) -> Option<Self>` — usable in const context.
const _ECOSTRING_TRY_INLINE: Option<EcoString> = EcoString::try_inline("hi");

fn _ecostring_inherent(s: &mut EcoString) {
    // `pub fn with_capacity(usize) -> Self`
    let mut owned: EcoString = EcoString::with_capacity(8);
    // `pub fn is_empty(&self) -> bool`
    let _b: bool = s.is_empty();
    // `pub fn len(&self) -> usize`
    let _n: usize = s.len();
    // `pub fn capacity(&self) -> usize`
    let _c: usize = s.capacity();
    // `pub fn is_inline(&self) -> bool`
    let _i: bool = s.is_inline();
    // `pub fn as_str(&self) -> &str`
    let _a: &str = s.as_str();
    // `pub fn make_mut(&mut self) -> &mut str`
    let _m: &mut str = s.make_mut();
    // `pub fn push(&mut self, c: char)`
    s.push('x');
    // `pub fn push_str(&mut self, &str)`
    s.push_str("yz");
    // `pub fn insert(&mut self, usize, char)`
    s.insert(0, 'x');
    // `pub fn insert_str(&mut self, usize, &str)`
    s.insert_str(0, "yz");
    // `pub fn pop(&mut self) -> Option<char>`
    let _p: Option<char> = s.pop();
    // `pub fn clear(&mut self)`
    owned.clear();
    // `pub fn truncate(&mut self, usize)`
    s.truncate(0);
    // `pub fn reserve(&mut self, usize)`
    s.reserve(8);
    // `pub fn remove(&mut self, usize) -> char`
    let _r: char = s.remove(0);
    // `pub fn replace(&self, &str, &str) -> Self`
    let _rep: EcoString = s.replace("a", "b");
    // `pub fn replacen(&self, &str, &str, usize) -> Self`
    let _repn: EcoString = s.replacen("a", "b", 1);
    // `pub fn to_lowercase(&self) -> Self`
    let _lo: EcoString = s.to_lowercase();
    // `pub fn to_uppercase(&self) -> Self`
    let _up: EcoString = s.to_uppercase();
    // `pub fn to_ascii_lowercase(&self) -> Self`
    let _alo: EcoString = s.to_ascii_lowercase();
    // `pub fn to_ascii_uppercase(&self) -> Self`
    let _aup: EcoString = s.to_ascii_uppercase();
    // `pub fn repeat(&self, usize) -> Self`
    let _rp: EcoString = s.repeat(2);
}

// =====================================================================
// EcoString — trait impls (ecow string.rs)
// =====================================================================

fn _ecostring_traits() {
    // Clone, Default, Debug, Display, Eq, Ord, PartialOrd, Hash
    assert_clone::<EcoString>();
    assert_default::<EcoString>();
    assert_debug::<EcoString>();
    assert_display::<EcoString>();
    assert_eq_trait::<EcoString>();
    assert_ord::<EcoString>();
    assert_partial_ord::<EcoString>();
    assert_hash::<EcoString>();

    // Deref to `str` (Borrow<str>, AsRef<str>, AsRef<[u8]>).
    //
    // ecow has `impl Deref<Target = str> for EcoString`. turbocow now matches
    // this exactly: `<EcoString as Deref>::Target == str` (asserted in
    // `_assert_eco_string_derefs_to_str` below). The inherent `str`-style
    // methods are delegated to the inner `EcoStr<'static>`.
    assert_borrow::<EcoString, str>();
    assert_as_ref::<EcoString, str>();
    assert_as_ref::<EcoString, [u8]>();

    // PartialEq family
    assert_partial_eq::<EcoString, EcoString>();
    assert_partial_eq::<EcoString, str>();
    assert_partial_eq::<EcoString, &str>();
    assert_partial_eq::<EcoString, String>();
    // reverse-direction PartialEq impls
    assert_partial_eq::<str, EcoString>();
    assert_partial_eq::<&str, EcoString>();
    assert_partial_eq::<String, EcoString>();

    // Add / AddAssign (Self and &str rhs)
    assert_add::<EcoString, EcoString, EcoString>();
    assert_add::<EcoString, &str, EcoString>();
    assert_add_assign::<EcoString, EcoString>();
    assert_add_assign::<EcoString, &str>();

    // From conversions into EcoString
    assert_from::<EcoString, char>();
    assert_from::<EcoString, &str>();
    assert_from::<EcoString, String>();
    assert_from::<EcoString, &String>();
    assert_from::<EcoString, &EcoString>();
    // ecow string.rs:519 `impl From<Cow<'_, str>> for EcoString` — matched.
    assert_from::<EcoString, Cow<'_, str>>();

    // From EcoString into String
    assert_from::<String, EcoString>();
    assert_from::<String, &EcoString>();

    // Byte conversions (ecow 0.3.1)
    assert_from::<EcoVec<u8>, EcoString>();
    assert_from::<EcoBytes, EcoString>();
    assert_try_from_err::<EcoString, EcoVec<u8>, core::str::Utf8Error>();
    assert_try_from_err::<EcoString, EcoBytes, core::str::Utf8Error>();

    // FromStr (Err = Infallible)
    assert_fromstr::<EcoString>();

    // FromIterator<char>, FromIterator<&str>, FromIterator<EcoString>
    assert_from_iter::<EcoString, char>();
    assert_from_iter::<EcoString, &str>();
    assert_from_iter::<EcoString, EcoString>();

    // Extend<char>, Extend<&str>
    assert_extend::<EcoString, char>();
    assert_extend::<EcoString, &str>();
}

// Exact Deref target: `<EcoString as Deref>::Target == str`, matching ecow
// 0.3.0. This is the strict-parity guarantee (generic code bounded
// `T: Deref<Target = str>` now accepts turbocow's `EcoString`).
fn _assert_eco_string_derefs_to_str() {
    fn bound<T: core::ops::Deref<Target = str>>() {}
    bound::<turbocow::EcoString>();
}

// Behavioral Deref guarantee: `&EcoString` coerces to `&str` and `str`
// inherent methods are reachable through `Deref<Target = str>`.
fn _ecostring_deref_str(s: &EcoString) {
    // Explicit coercion to `&str` through `Deref`.
    let _coerced: &str = s;
    // `str`-only inherent methods reachable via auto-deref.
    let _starts: bool = s.starts_with("a");
    let _chars = s.chars();
}

// FromStr error is exactly `core::convert::Infallible`.
fn _ecostring_fromstr_err() {
    fn check<T: core::str::FromStr<Err = core::convert::Infallible>>() {}
    check::<EcoString>();
}

// Path-call syntax resolves to `FromStr::from_str` (no inherent `from_str`
// shadowing it), so ecow code like `EcoString::from_str(s)?` compiles.
fn _ecostring_fromstr_call() -> Result<EcoString, core::convert::Infallible> {
    use core::str::FromStr;
    EcoString::from_str("abc")
}

// `core::fmt::Write for EcoString` (write_str / write_char).
fn _ecostring_fmt_write(s: &mut EcoString) -> core::fmt::Result {
    use core::fmt::Write;
    s.write_str("a")?;
    s.write_char('b')
}

// =====================================================================
// EcoBytes — inherent methods (ecow bytes.rs)
// =====================================================================

// `pub const INLINE_LIMIT: usize`
const _ECOBYTES_INLINE_LIMIT: usize = EcoBytes::INLINE_LIMIT;

// `pub const fn new() -> Self` — usable in const context.
const _ECOBYTES_NEW: EcoBytes = EcoBytes::new();

// `pub const fn inline(&[u8]) -> Self` — usable in const context.
const _ECOBYTES_INLINE: EcoBytes = EcoBytes::inline(b"hi");

// `pub const fn try_inline(&[u8]) -> Option<Self>` — usable in const context.
const _ECOBYTES_TRY_INLINE: Option<EcoBytes> = EcoBytes::try_inline(b"hi");

fn _ecobytes_inherent(b: &mut EcoBytes) {
    // `pub fn with_capacity(usize) -> Self`
    let mut owned: EcoBytes = EcoBytes::with_capacity(8);
    // `pub fn is_empty(&self) -> bool`
    let _e: bool = b.is_empty();
    // `pub fn len(&self) -> usize`
    let _n: usize = b.len();
    // `pub fn capacity(&self) -> usize`
    let _c: usize = b.capacity();
    // `pub fn is_inline(&self) -> bool`
    let _i: bool = b.is_inline();
    // `pub fn as_slice(&self) -> &[u8]`
    let _s: &[u8] = b.as_slice();
    // `pub fn make_mut(&mut self) -> &mut [u8]`
    let _m: &mut [u8] = b.make_mut();
    // `pub fn push(&mut self, u8)`
    b.push(1);
    // `pub fn pop(&mut self) -> Option<u8>`
    let _p: Option<u8> = b.pop();
    // `pub fn insert(&mut self, usize, u8)`
    b.insert(0, 1);
    // `pub fn remove(&mut self, usize) -> u8`
    let _r: u8 = b.remove(0);
    // `pub fn extend_from_slice(&mut self, &[u8])`
    b.extend_from_slice(b"ab");
    // `pub fn insert_slice(&mut self, usize, &[u8])`
    b.insert_slice(0, b"ab");
    // `pub fn clear(&mut self)`
    owned.clear();
    // `pub fn truncate(&mut self, usize)`
    b.truncate(0);
    // `pub fn reserve(&mut self, usize)`
    b.reserve(8);
    // `pub fn repeat(&self, usize) -> Self`
    let _rp: EcoBytes = b.repeat(2);
}

// =====================================================================
// EcoBytes — trait impls (ecow bytes.rs)
// =====================================================================

fn _ecobytes_traits() {
    // Clone, Default, Debug, Eq, Ord, PartialOrd, Hash, Send, Sync
    assert_clone::<EcoBytes>();
    assert_default::<EcoBytes>();
    assert_debug::<EcoBytes>();
    assert_eq_trait::<EcoBytes>();
    assert_ord::<EcoBytes>();
    assert_partial_ord::<EcoBytes>();
    assert_hash::<EcoBytes>();
    assert_send::<EcoBytes>();
    assert_sync::<EcoBytes>();

    // Deref<Target = [u8]>, AsRef<[u8]>, Borrow<[u8]>
    assert_deref::<EcoBytes, [u8]>();
    assert_as_ref::<EcoBytes, [u8]>();
    assert_borrow::<EcoBytes, [u8]>();

    // PartialEq family
    assert_partial_eq::<EcoBytes, EcoBytes>();
    assert_partial_eq::<EcoBytes, [u8]>();
    assert_partial_eq::<EcoBytes, &[u8]>();
    assert_partial_eq::<EcoBytes, [u8; 3]>();
    assert_partial_eq::<EcoBytes, &[u8; 3]>();
    assert_partial_eq::<EcoBytes, Vec<u8>>();
    assert_partial_eq::<EcoBytes, EcoVec<u8>>();
    // reverse-direction PartialEq impls
    assert_partial_eq::<[u8], EcoBytes>();
    assert_partial_eq::<[u8; 3], EcoBytes>();
    assert_partial_eq::<Vec<u8>, EcoBytes>();
    assert_partial_eq::<EcoVec<u8>, EcoBytes>();

    // From conversions into EcoBytes
    assert_from::<EcoBytes, &[u8]>();
    assert_from::<EcoBytes, &[u8; 3]>();
    assert_from::<EcoBytes, [u8; 3]>();
    assert_from::<EcoBytes, Vec<u8>>();
    assert_from::<EcoBytes, &Vec<u8>>();
    assert_from::<EcoBytes, EcoVec<u8>>();
    assert_from::<EcoBytes, &EcoBytes>();
    assert_from::<EcoBytes, Cow<'_, [u8]>>();

    // From EcoBytes into Vec<u8> / EcoVec<u8>
    assert_from::<Vec<u8>, EcoBytes>();
    assert_from::<Vec<u8>, &EcoBytes>();
    assert_from::<EcoVec<u8>, EcoBytes>();
    assert_from::<EcoVec<u8>, &EcoBytes>();

    // FromIterator<u8>, Extend<u8>, Extend<&u8>
    assert_from_iter::<EcoBytes, u8>();
    assert_extend::<EcoBytes, u8>();
    assert_extend::<EcoBytes, &u8>();
}

// `impl<'a> IntoIterator for &'a EcoBytes` (Item = &u8, IntoIter = slice::Iter).
fn _ecobytes_into_iter(b: &EcoBytes) {
    let _it: core::slice::Iter<'_, u8> = b.into_iter();
    for _byte in b {}
}

// `std::io::Write for EcoBytes` (std-only).
#[cfg(feature = "std")]
fn _ecobytes_io_write(b: &mut EcoBytes) {
    use std::io::Write;
    let _ = b.write_all(b"abc");
    let _ = b.flush();
}

// =====================================================================
// Macros (ecow vec.rs / string.rs)
// =====================================================================

fn _macros() {
    // eco_vec! — empty / repeat / list forms
    let _e: EcoVec<i32> = eco_vec![];
    let _r: EcoVec<i32> = eco_vec![1; 4];
    let _l: EcoVec<i32> = eco_vec![1, 2, 3];
    // eco_format! — produces an EcoString
    let _f: EcoString = eco_format!("Hello, {}!", 123);
}

// =====================================================================
// ToEcoString / AsRef<OsStr>/<Path> (ecow string.rs)
// =====================================================================

// `ToEcoString` is auto-implemented for every `T: Display`.
fn _to_eco_string() {
    use turbocow::string::ToEcoString;
    let _: EcoString = 42i32.to_eco_string();
    let _: EcoString = "abc".to_eco_string();
    // Bound-level proof that the trait is in scope at the crate root too.
    fn check<T: turbocow::ToEcoString>() {}
    check::<i32>();
}

// `AsRef<OsStr>` and `AsRef<Path>` for EcoString (std-only in ecow; turbocow
// has `std` in its default feature set).
#[cfg(feature = "std")]
fn _ecostring_as_ref_os(s: &EcoString) {
    let _os: &std::ffi::OsStr = s.as_ref();
    let _p: &std::path::Path = s.as_ref();
}

// =====================================================================
// std::io::Write for EcoVec<u8> (ecow vec.rs, std-only)
// =====================================================================

#[cfg(feature = "std")]
fn _ecovec_io_write(v: &mut EcoVec<u8>) {
    use std::io::Write;
    let _ = v.write_all(b"abc");
}

// =====================================================================
// Module paths & re-exports (ecow lib.rs)
// =====================================================================

// Crate-root re-exports: `EcoVec`, `EcoString`, `EcoBytes`.
type _RootEcoVec = turbocow::EcoVec<u8>;
type _RootEcoString = turbocow::EcoString;
type _RootEcoBytes = turbocow::EcoBytes;

// `ecow::bytes` submodule: `EcoBytes`.
type _ModBytesEcoBytes = turbocow::bytes::EcoBytes;

// `ecow::vec` submodule: `EcoVec`, `IntoIter`, `Drain`, `Splice`.
type _ModVecEcoVec = turbocow::vec::EcoVec<u8>;
type _ModVecIntoIter = turbocow::vec::IntoIter<u8>;
type _ModVecDrain<'a> = turbocow::vec::Drain<'a, u8>;
type _ModVecSplice<'a> = turbocow::vec::Splice<'a, core::array::IntoIter<u8, 0>>;

// `ecow::string` submodule: `EcoString`, `ToEcoString`.
type _ModStrEcoString = turbocow::string::EcoString;
fn _mod_str_to_eco_string<T: turbocow::string::ToEcoString>() {}

// =====================================================================
// serde (feature-gated; ecow vec.rs / string.rs `mod serde`)
// =====================================================================

#[cfg(feature = "serde")]
fn _serde() {
    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: for<'de> serde::Deserialize<'de>>() {}

    // EcoVec<T>: Serialize (T: Serialize) + Deserialize (T: Deserialize + Clone)
    assert_serialize::<EcoVec<i32>>();
    assert_deserialize::<EcoVec<i32>>();

    // EcoString: Serialize + Deserialize
    assert_serialize::<EcoString>();
    assert_deserialize::<EcoString>();

    // EcoBytes: Serialize + Deserialize
    assert_serialize::<EcoBytes>();
    assert_deserialize::<EcoBytes>();
}
