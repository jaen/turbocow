//! A clone-on-write, small-string-optimized alternative to [`String`].

use core::borrow::Borrow;
use core::cmp::Ordering;

extern crate alloc;
use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt::{self, Debug, Display, Formatter, Write};
use core::hash::{Hash, Hasher};
use core::ops::{Add, AddAssign, Deref};
use core::str::FromStr;

use crate::allocator::Global;
use crate::dynamic::{DynamicVec, InlineVec, LIMIT};

/// Create a new [`EcoString`] from a format string.
/// ```
/// # use turbocow::eco_format;
/// assert_eq!(eco_format!("Hello, {}!", 123), "Hello, 123!");
/// ```
#[macro_export]
macro_rules! eco_format {
    ($($tts:tt)*) => {
        $crate::EcoString::_format(::core::format_args!($($tts)*))
    };
}

/// A lifetime-carrying, clone-on-write string with inline storage.
///
/// `EcoStr<'a>` is the general form of turbocow's string type. It supports
/// three storage variants:
///
/// - **Inline** — short strings (up to [`EcoStr::INLINE_LIMIT`] bytes) are
///   stored directly in the struct with no heap allocation.
/// - **Heap-owned** — longer strings are stored in a reference-counted
///   `EcoVec<u8>` allocation; cloning is O(1) (atomic refcount bump), and
///   mutation triggers copy-on-write.
/// - **Referenced (zero-copy borrow)** — the string borrows a `&'a str`
///   without copying any bytes. The lifetime `'a` tracks how long the borrow
///   is valid.
///
/// [`EcoString`] is a type alias for `EcoStr<'static>`, which holds either an
/// inline string or a heap-owned string (no borrow lifetime needed). Use
/// `EcoString` for most owned-string use cases.  Use `EcoStr<'a>` when you
/// want zero-copy borrows from non-`'static` data.
///
/// The type has a size of 16 bytes on little-endian 32-bit or 64-bit systems
/// (24 bytes on 64-bit big-endian). The inline capacity is 15 bytes
/// (23 bytes on 64-bit big-endian).
///
/// # Example
/// ```
/// use turbocow::{EcoStr, EcoString};
///
/// // Borrowed (zero-copy): ties the lifetime to `source`.
/// let source = String::from("hello, world");
/// let borrowed: EcoStr<'_> = EcoStr::from(source.as_str());
/// assert_eq!(borrowed, "hello, world");
///
/// // Convert to an owned EcoString when you need 'static.
/// let owned: EcoString = borrowed.into_owned();
/// assert_eq!(owned, "hello, world");
/// ```
#[derive(Clone)]
pub struct EcoStr<'a>(DynamicVec<'a>);

/// A fully-owned [`EcoStr`] with no lifetime parameter.
///
/// This is the standard string type for most use cases.  It wraps
/// `EcoStr<'static>` so that callers never need to write a lifetime.
///
/// Use [`EcoStr`] directly when you want zero-copy borrows from
/// non-`'static` data.
#[derive(Clone)]
pub struct EcoString(EcoStr<'static>);

// ── EcoStr inherent methods ─────────────────────────────────────────────

impl<'a> EcoStr<'a> {
    /// Maximum number of bytes for an inline string before spilling on
    /// the heap.
    ///
    /// The exact value for this is architecture dependent.
    ///
    /// # Note
    /// This value is semver exempt and can be changed with any update.
    pub const INLINE_LIMIT: usize = LIMIT;

    /// Whether the string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The length of the string in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// How many bytes the string can hold without (re-)allocating.
    ///
    /// If the string's heap allocation is shared, mutation can still allocate
    /// even when the requested length fits within this capacity. A borrowed
    /// (Referenced) string owns no storage, so its capacity is its length.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Whether this string is stored inline.
    ///
    /// A borrowed (Referenced) string is not inline.
    #[inline]
    pub fn is_inline(&self) -> bool {
        self.0.is_pure_inline()
    }

    /// A string slice containing the entire string.
    #[inline]
    pub fn as_str(&self) -> &str {
        // Safety:
        // The buffer contents stem from correct UTF-8 sources:
        // - Valid ASCII characters
        // - Other string slices
        // - Chars that were encoded with char::encode_utf8
        unsafe { core::str::from_utf8_unchecked(self.0.as_slice()) }
    }

    /// Produce a mutable slice containing the entire string.
    ///
    /// Clones the string if its reference count is larger than 1.
    #[inline]
    pub fn make_mut(&mut self) -> &mut str {
        // Safety:
        // The buffer contents stem from correct UTF-8 sources:
        // - Valid ASCII characters
        // - Other string slices
        // - Chars that were encoded with char::encode_utf8
        unsafe { core::str::from_utf8_unchecked_mut(self.0.make_mut(0)) }
    }

    /// Append the given character at the end.
    #[inline]
    pub fn push(&mut self, c: char) {
        if c.len_utf8() == 1 {
            self.0.push(c as u8);
        } else {
            self.push_str(c.encode_utf8(&mut [0; 4]));
        }
    }

    /// Append the given string slice at the end.
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        self.0.extend_from_slice(string.as_bytes());
    }

    /// Remove the last character from the string.
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        let slice = self.as_str();
        let c = slice.chars().next_back()?;
        self.0.truncate(slice.len() - c.len_utf8());
        Some(c)
    }

    /// Clear the string.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Shortens the string to the specified length.
    ///
    /// If `new_len` is greater than or equal to the string's current length,
    /// this has no effect.
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len <= self.len() {
            assert!(self.is_char_boundary(new_len));
            self.0.truncate(new_len);
        }
    }

    /// Reserves space for at least `additional` more bytes.
    ///
    /// Guarantees that the resulting string has space for `additional` more
    /// bytes and owns its storage: if spilled, it uniquely owns its backing
    /// allocation, and a borrowed (Referenced) string is copied.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Insert a string slice at the given byte `index`.
    ///
    /// `index` must be a valid UTF-8 character boundary.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[track_caller]
    pub fn insert_str(&mut self, index: usize, string: &str) {
        assert!(self.is_char_boundary(index), "insertion index is not a char boundary");
        // Reconstruct: [..index] + string + [index..]
        // We reuse `self` in-place to avoid an extra allocation when possible.
        // Build a new EcoString, then replace self with it.
        let old_len = self.len();
        let new_len = old_len + string.len();
        let mut result = EcoString::with_capacity(new_len);
        // Safety: index was checked to be a char boundary, so it is a valid
        // byte offset into self's UTF-8 data.
        result.push_str(unsafe { self.get_unchecked(..index) });
        result.push_str(string);
        result.push_str(unsafe { self.get_unchecked(index..) });
        *self = result.into_raw_eco_str();
    }

    /// Insert a character at the given byte `index`.
    ///
    /// `index` must be a valid UTF-8 character boundary.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn insert(&mut self, index: usize, c: char) {
        self.insert_str(index, c.encode_utf8(&mut [0; 4]));
    }

    /// Remove the character at the given byte `index` and return it.
    ///
    /// The subsequent characters are shifted left. `index` must be a valid
    /// UTF-8 character boundary and in bounds.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> char {
        assert!(self.is_char_boundary(index), "removal index is not a char boundary");
        // Safety: index is a valid char boundary.
        let c = unsafe { self.get_unchecked(index..) }
            .chars()
            .next()
            .expect("removal index is out of bounds");
        let char_end = index + c.len_utf8();
        let old_len = self.len();
        let mut result = EcoString::with_capacity(old_len - c.len_utf8());
        // Safety: both index and char_end are valid byte boundaries.
        result.push_str(unsafe { self.get_unchecked(..index) });
        result.push_str(unsafe { self.get_unchecked(char_end..) });
        *self = result.into_raw_eco_str();
        c
    }

    /// Convert into a fully owned [`EcoString`] (no lifetime parameter).
    ///
    /// Inline and spilled variants are already owned and converted via a
    /// zero-cost transmute.  The borrowed variant copies the data.
    #[inline]
    pub fn into_owned(self) -> EcoString {
        EcoString(EcoStr(self.0.to_owned()))
    }

    /// Replaces all matches of a string with another string.
    ///
    /// This is a bit less general that [`str::replace`] because the `Pattern`
    /// trait is unstable. In return, it can produce an `EcoString` without
    /// any intermediate [`String`] allocation.
    pub fn replace(&self, pat: &str, to: &str) -> EcoString {
        self.replacen(pat, to, usize::MAX)
    }

    /// Replaces the first N matches of a string with another string.
    ///
    /// This is a bit less general that [`str::replacen`] because the `Pattern`
    /// trait is unstable. In return, it can produce an `EcoString` without
    /// any intermediate [`String`] allocation.
    pub fn replacen(&self, pat: &str, to: &str, count: usize) -> EcoString {
        // Copied from the standard library: https://github.com/rust-lang/rust
        let mut result = EcoString::with_capacity(self.len());
        let mut last_end = 0;
        for (start, part) in self.match_indices(pat).take(count) {
            // Safety: Copied from std.
            result.push_str(unsafe { self.get_unchecked(last_end..start) });
            result.push_str(to);
            last_end = start + part.len();
        }
        // Safety: Copied from std.
        result.push_str(unsafe { self.get_unchecked(last_end..self.len()) });
        result
    }

    /// Returns the lowercase equivalent of this string.
    pub fn to_lowercase(&self) -> EcoString {
        let str = self.as_str();
        let bytes = str.as_bytes();
        let mut lower = EcoString::with_capacity(str.len());

        // Fast path: bulk-copy the ASCII prefix, lowercasing in-place.
        let mut ascii_end = 0;
        for &b in bytes {
            if b.is_ascii() {
                ascii_end += 1;
            } else {
                break;
            }
        }
        if ascii_end > 0 {
            lower.push_str(unsafe { str.get_unchecked(..ascii_end) });
            // Safety: we just pushed valid ASCII bytes.
            lower.make_mut().make_ascii_lowercase();
        }

        // Slow path for the remainder (non-ASCII characters).
        for c in unsafe { str.get_unchecked(ascii_end..) }.chars() {
            if c == 'Σ' {
                return str.to_lowercase().into();
            }
            for v in c.to_lowercase() {
                lower.push(v);
            }
        }
        lower
    }

    /// Returns the uppercase equivalent of this string.
    pub fn to_uppercase(&self) -> EcoString {
        let str = self.as_str();
        let bytes = str.as_bytes();
        let mut upper = EcoString::with_capacity(str.len());

        // Fast path: bulk-copy the ASCII prefix, uppercasing in-place.
        let mut ascii_end = 0;
        for &b in bytes {
            if b.is_ascii() {
                ascii_end += 1;
            } else {
                break;
            }
        }
        if ascii_end > 0 {
            upper.push_str(unsafe { str.get_unchecked(..ascii_end) });
            upper.make_mut().make_ascii_uppercase();
        }

        // Slow path for the remainder.
        for c in unsafe { str.get_unchecked(ascii_end..) }.chars() {
            for v in c.to_uppercase() {
                upper.push(v);
            }
        }
        upper
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII lowercase equivalent.
    pub fn to_ascii_lowercase(&self) -> EcoString {
        let mut s = self.clone().into_owned();
        s.make_mut().make_ascii_lowercase();
        s
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII uppercase equivalent.
    pub fn to_ascii_uppercase(&self) -> EcoString {
        let mut s = self.clone().into_owned();
        s.make_mut().make_ascii_uppercase();
        s
    }

    /// Repeat this string `n` times.
    pub fn repeat(&self, n: usize) -> EcoString {
        let slice = self.as_bytes();
        let capacity = slice.len().saturating_mul(n);
        let mut vec = DynamicVec::with_capacity(capacity);
        for _ in 0..n {
            vec.extend_from_slice(slice);
        }
        EcoString(EcoStr(vec))
    }

    /// Create an `EcoStr` from a pre-validated `DynamicVec`.
    ///
    /// # Safety
    /// The caller must ensure the bytes in `vec` are valid UTF-8.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn from_raw(vec: DynamicVec<'a>) -> Self {
        Self(vec)
    }

    /// Unwrap this `EcoStr` into the underlying `DynamicVec`.
    ///
    /// This is used for zero-cost conversions into byte buffers.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn into_raw(self) -> DynamicVec<'a> {
        self.0
    }
}

// ── EcoString constructors ──────────────────────────────────────────────

impl EcoString {
    /// Maximum number of bytes for an inline `EcoString` before spilling on
    /// the heap.
    ///
    /// The exact value for this is architecture dependent.
    ///
    /// # Note
    /// This value is semver exempt and can be changed with any update.
    pub const INLINE_LIMIT: usize = LIMIT;

    /// Create a new, empty string.
    #[inline]
    pub const fn new() -> Self {
        Self(EcoStr(DynamicVec::new()))
    }

    /// Create a new, inline string.
    ///
    /// Panics if the string's length exceeds the capacity of the inline
    /// storage.
    ///
    /// This is a `const fn` so it can be used in constant contexts:
    /// ```
    /// use turbocow::EcoString;
    /// const GREETING: EcoString = EcoString::inline("hello");
    /// ```
    #[inline]
    pub const fn inline(string: &str) -> Self {
        let Ok(inline) = InlineVec::from_slice(string.as_bytes()) else {
            exceeded_inline_capacity();
        };
        Self(EcoStr(DynamicVec::from_inline(inline)))
    }

    /// Try to create a new, inline string.
    ///
    /// Returns `None` if the string's length exceeds the capacity of the
    /// inline storage.
    #[inline]
    pub const fn try_inline(string: &str) -> Option<Self> {
        match InlineVec::from_slice(string.as_bytes()) {
            Ok(inline) => Some(Self(EcoStr(DynamicVec::from_inline(inline)))),
            Err(()) => None,
        }
    }

    /// Create a string that **borrows** a `'static` string slice without
    /// copying (the Referenced variant).
    ///
    /// Prefer [`EcoStr::from`] when working with non-`'static` borrows.
    #[inline]
    pub fn from_static(string: &'static str) -> Self {
        Self(EcoStr(DynamicVec::from_slice_in(string.as_bytes(), Global)))
    }

    /// Create a new, empty string with the given `capacity`.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(EcoStr(DynamicVec::with_capacity(capacity)))
    }

    /// Construct an `EcoString` from format arguments.
    ///
    /// When the `nightly-fmt` feature is enabled, this uses
    /// [`fmt::Arguments::as_str`] to short-circuit plain string literals
    /// (no formatting overhead at all).
    #[doc(hidden)]
    #[inline]
    #[must_use]
    pub fn _format(args: fmt::Arguments<'_>) -> Self {
        #[cfg(feature = "nightly-fmt")]
        if let Some(s) = args.as_str() {
            return Self::from(s);
        }

        let mut s = Self::new();
        s.write_fmt(args).unwrap();
        s
    }

    /// Create an instance from a string slice (always copies).
    ///
    /// Deliberately not named `from_str`: an inherent `from_str` would shadow
    /// [`FromStr::from_str`] and break `EcoString::from_str(s)?`-style code
    /// written against ecow.
    #[inline]
    fn copy_str(string: &str) -> Self {
        Self(EcoStr(DynamicVec::from_slice(string.as_bytes())))
    }

    /// Create an `EcoString` from a pre-validated `DynamicVec`.
    ///
    /// # Safety
    /// The caller must ensure the bytes in `vec` are valid UTF-8.
    #[inline]
    pub(crate) fn from_raw(vec: DynamicVec<'static>) -> Self {
        Self(EcoStr(vec))
    }

    /// Unwrap this `EcoString` into the underlying `DynamicVec`.
    #[inline]
    pub(crate) fn into_raw(self) -> DynamicVec<'static> {
        (self.0).0
    }

    /// Consume `self` and return the inner `EcoStr<'static>`.
    ///
    /// Used internally by `EcoStr::insert_str` / `EcoStr::remove` to replace
    /// the `EcoStr` backing store with a freshly constructed one.
    #[inline]
    fn into_raw_eco_str(self) -> EcoStr<'static> {
        self.0
    }

    // ── String accessors / mutators (delegating to the inner EcoStr) ──────
    //
    // These thin inherent methods give `EcoString` the exact inherent API of
    // ecow 0.3.0's `EcoString` (whose methods are inherent rather than reached
    // through `Deref`). The actual logic lives on `EcoStr<'static>`; we simply
    // forward to `self.0` so behavior is identical and not duplicated.

    /// Whether the string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The length of the string in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// How many bytes the string can hold without (re-)allocating.
    ///
    /// If the string's heap allocation is shared, mutation can still allocate
    /// even when the requested length fits within this capacity. A string
    /// created with [`from_static`](Self::from_static) owns no storage, so its
    /// capacity is its length.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Whether this string is stored inline.
    ///
    /// A string created with [`from_static`](Self::from_static) is borrowed,
    /// not inline.
    #[inline]
    pub fn is_inline(&self) -> bool {
        self.0.is_inline()
    }

    /// A string slice containing the entire string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Produce a mutable slice containing the entire string.
    ///
    /// Clones the string if its reference count is larger than 1.
    #[inline]
    pub fn make_mut(&mut self) -> &mut str {
        self.0.make_mut()
    }

    /// Append the given character at the end.
    #[inline]
    pub fn push(&mut self, c: char) {
        self.0.push(c);
    }

    /// Append the given string slice at the end.
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        self.0.push_str(string);
    }

    /// Insert a character at the given byte `index`.
    ///
    /// `index` must be a valid UTF-8 character boundary.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn insert(&mut self, index: usize, c: char) {
        self.0.insert(index, c);
    }

    /// Insert a string slice at the given byte `index`.
    ///
    /// `index` must be a valid UTF-8 character boundary.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn insert_str(&mut self, index: usize, string: &str) {
        self.0.insert_str(index, string);
    }

    /// Remove the last character from the string.
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        self.0.pop()
    }

    /// Clear the string.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Shortens the string to the specified length.
    ///
    /// If `new_len` is greater than or equal to the string's current length,
    /// this has no effect.
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn truncate(&mut self, new_len: usize) {
        self.0.truncate(new_len);
    }

    /// Reserves space for at least `additional` more bytes.
    ///
    /// Guarantees that the resulting string has space for `additional` more
    /// bytes and owns its storage: if spilled, it uniquely owns its backing
    /// allocation, and a string created with [`from_static`](Self::from_static)
    /// is copied.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Remove the character at the given byte `index` and return it.
    ///
    /// The subsequent characters are shifted left. `index` must be a valid
    /// UTF-8 character boundary and in bounds.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds or does not lie on a [`char`] boundary.
    #[inline]
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> char {
        self.0.remove(index)
    }

    /// Replaces all matches of a string with another string.
    ///
    /// This is a bit less general that [`str::replace`] because the `Pattern`
    /// trait is unstable. In return, it can produce an `EcoString` without
    /// any intermediate [`String`] allocation.
    #[inline]
    pub fn replace(&self, pat: &str, to: &str) -> Self {
        self.0.replace(pat, to)
    }

    /// Replaces the first N matches of a string with another string.
    ///
    /// This is a bit less general that [`str::replacen`] because the `Pattern`
    /// trait is unstable. In return, it can produce an `EcoString` without
    /// any intermediate [`String`] allocation.
    #[inline]
    pub fn replacen(&self, pat: &str, to: &str, count: usize) -> Self {
        self.0.replacen(pat, to, count)
    }

    /// Returns the lowercase equivalent of this string.
    #[inline]
    pub fn to_lowercase(&self) -> Self {
        self.0.to_lowercase()
    }

    /// Returns the uppercase equivalent of this string.
    #[inline]
    pub fn to_uppercase(&self) -> Self {
        self.0.to_uppercase()
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII lowercase equivalent.
    #[inline]
    pub fn to_ascii_lowercase(&self) -> Self {
        self.0.to_ascii_lowercase()
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII uppercase equivalent.
    #[inline]
    pub fn to_ascii_uppercase(&self) -> Self {
        self.0.to_ascii_uppercase()
    }

    /// Repeat this string `n` times.
    #[inline]
    pub fn repeat(&self, n: usize) -> Self {
        self.0.repeat(n)
    }
}

// ── EcoStr Deref → str ──────────────────────────────────────────────────

impl Deref for EcoStr<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

// ── EcoString Deref → str ───────────────────────────────────────────────
//
// Exact ecow 0.3.0 parity: `EcoString` derefs straight to `str` (and has no
// `DerefMut`; mutation goes through the inherent methods above). The inner
// `EcoStr<'static>` is reached via `self.0` for borrowed views.

impl Deref for EcoString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

// ── EcoStr trait impls ──────────────────────────────────────────────────

impl Default for EcoStr<'_> {
    #[inline]
    fn default() -> Self {
        EcoStr(DynamicVec::new())
    }
}

impl Debug for EcoStr<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for EcoStr<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl Eq for EcoStr<'_> {}

impl PartialEq for EcoStr<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<str> for EcoStr<'_> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<&str> for EcoStr<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str().eq(*other)
    }
}

impl PartialEq<String> for EcoStr<'_> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl PartialEq<EcoStr<'_>> for str {
    #[inline]
    fn eq(&self, other: &EcoStr<'_>) -> bool {
        self.eq(other.as_str())
    }
}

impl PartialEq<EcoStr<'_>> for &str {
    #[inline]
    fn eq(&self, other: &EcoStr<'_>) -> bool {
        (*self).eq(other.as_str())
    }
}

impl PartialEq<EcoStr<'_>> for String {
    #[inline]
    fn eq(&self, other: &EcoStr<'_>) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl Ord for EcoStr<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for EcoStr<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for EcoStr<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Write for EcoStr<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl AsRef<str> for EcoStr<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for EcoStr<'_> {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for EcoStr<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

impl<'a> From<&'a str> for EcoStr<'a> {
    /// Zero-copy borrow: stores a pointer to the string data without copying.
    #[inline]
    fn from(s: &'a str) -> Self {
        Self(DynamicVec::from_slice_in(s.as_bytes(), Global))
    }
}

// ── EcoString trait impls ───────────────────────────────────────────────

impl Default for EcoString {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for EcoString {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for EcoString {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl Eq for EcoString {}

impl PartialEq for EcoString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<str> for EcoString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<&str> for EcoString {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str().eq(*other)
    }
}

impl PartialEq<String> for EcoString {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl PartialEq<EcoString> for str {
    #[inline]
    fn eq(&self, other: &EcoString) -> bool {
        self.eq(other.as_str())
    }
}

impl PartialEq<EcoString> for &str {
    #[inline]
    fn eq(&self, other: &EcoString) -> bool {
        (*self).eq(other.as_str())
    }
}

impl PartialEq<EcoString> for String {
    #[inline]
    fn eq(&self, other: &EcoString) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl PartialEq<EcoString> for EcoStr<'_> {
    #[inline]
    fn eq(&self, other: &EcoString) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl PartialEq<EcoStr<'_>> for EcoString {
    #[inline]
    fn eq(&self, other: &EcoStr<'_>) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl Ord for EcoString {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for EcoString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for EcoString {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Write for EcoString {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl Add for EcoString {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl AddAssign for EcoString {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.push_str(rhs.as_str());
    }
}

impl Add<&str> for EcoString {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: &str) -> Self::Output {
        self += rhs;
        self
    }
}

impl AddAssign<&str> for EcoString {
    #[inline]
    fn add_assign(&mut self, rhs: &str) {
        self.push_str(rhs);
    }
}

impl AsRef<str> for EcoString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for EcoString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for EcoString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

#[cfg(feature = "std")]
impl AsRef<std::ffi::OsStr> for EcoString {
    #[inline]
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.as_str().as_ref()
    }
}

#[cfg(feature = "std")]
impl AsRef<std::path::Path> for EcoString {
    #[inline]
    fn as_ref(&self) -> &std::path::Path {
        self.as_str().as_ref()
    }
}

impl From<char> for EcoString {
    #[inline]
    fn from(c: char) -> Self {
        Self::inline(c.encode_utf8(&mut [0; 4]))
    }
}

impl From<&str> for EcoString {
    /// Always copies the data (owned string).
    ///
    /// Use [`EcoStr::from`] for a zero-copy borrow instead.
    #[inline]
    fn from(s: &str) -> Self {
        Self::copy_str(s)
    }
}

impl From<String> for EcoString {
    /// For short strings that fit inline, copies. For longer strings,
    /// moves the bytes from the `String`'s `Vec<u8>` into a new `EcoVec`
    /// allocation (one alloc + memcpy instead of two).
    #[inline]
    fn from(s: String) -> Self {
        if s.len() <= LIMIT {
            Self::copy_str(&s)
        } else {
            let bytes = s.into_bytes();
            let eco: crate::EcoVec<u8> = crate::EcoVec::from(bytes);
            let dv = DynamicVec::from_eco(eco);
            EcoString(EcoStr(dv))
        }
    }
}

impl From<&String> for EcoString {
    #[inline]
    fn from(s: &String) -> Self {
        Self::copy_str(s.as_str())
    }
}

impl From<&EcoString> for EcoString {
    #[inline]
    fn from(s: &EcoString) -> Self {
        s.clone()
    }
}

impl From<Cow<'_, str>> for EcoString {
    /// Always copies the data (owned string), matching ecow 0.3.0.
    #[inline]
    fn from(s: Cow<'_, str>) -> Self {
        Self::copy_str(&s)
    }
}

impl From<EcoString> for crate::EcoVec<u8> {
    /// When the string is stored inline (or borrowed), this needs to allocate
    /// to change the layout. Otherwise, it reuses the existing allocation.
    #[inline]
    fn from(s: EcoString) -> Self {
        s.into_raw().into_eco()
    }
}

impl TryFrom<crate::EcoVec<u8>> for EcoString {
    type Error = core::str::Utf8Error;

    /// Zero-copy conversion if the bytes are valid UTF-8. The resulting string
    /// remains spilled even if its contents would fit inline.
    #[inline]
    fn try_from(vec: crate::EcoVec<u8>) -> Result<Self, Self::Error> {
        core::str::from_utf8(vec.as_slice())?;
        Ok(Self::from_raw(DynamicVec::from_eco(vec)))
    }
}

impl FromIterator<char> for EcoString {
    #[inline]
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        let mut s = Self::new();
        for c in iter {
            s.push(c);
        }
        s
    }
}

impl FromIterator<Self> for EcoString {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let mut s = Self::new();
        for piece in iter {
            s.push_str(&piece);
        }
        s
    }
}

impl<'a> FromIterator<&'a str> for EcoString {
    #[inline]
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl Extend<char> for EcoString {
    #[inline]
    fn extend<T: IntoIterator<Item = char>>(&mut self, iter: T) {
        for c in iter {
            self.push(c);
        }
    }
}

impl<'a> Extend<&'a str> for EcoString {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        iter.into_iter().for_each(move |s| self.push_str(s));
    }
}

impl From<EcoString> for String {
    /// This needs to allocate to change the layout.
    #[inline]
    fn from(s: EcoString) -> Self {
        s.as_str().into()
    }
}

impl From<&EcoString> for String {
    #[inline]
    fn from(s: &EcoString) -> Self {
        s.as_str().into()
    }
}

impl FromStr for EcoString {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::copy_str(s))
    }
}

/// A trait for converting a value to an [`EcoString`].
///
/// This trait is automatically implemented for any type which implements the
/// [`core::fmt::Display`] trait. It mirrors the same-named trait in ecow 0.3.0.
pub trait ToEcoString {
    /// Converts the given value to an [`EcoString`].
    fn to_eco_string(&self) -> EcoString;
}

impl<T: core::fmt::Display + ?Sized> ToEcoString for T {
    #[inline]
    fn to_eco_string(&self) -> EcoString {
        crate::eco_format!("{self}")
    }
}

#[cold]
const fn exceeded_inline_capacity() -> ! {
    panic!("exceeded inline capacity");
}

#[cfg(feature = "serde")]
mod serde {
    use crate::EcoString;
    use core::fmt;
    use serde::de::{Deserializer, Error, Unexpected, Visitor};

    impl serde::Serialize for EcoString {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for EcoString {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct EcoStringVisitor;

            impl Visitor<'_> for EcoStringVisitor {
                type Value = EcoString;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a string")
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    Ok(EcoString::from(v))
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    if let Ok(utf8) = core::str::from_utf8(v) {
                        return Ok(EcoString::from(utf8));
                    }
                    Err(Error::invalid_value(Unexpected::Bytes(v), &self))
                }
            }

            deserializer.deserialize_str(EcoStringVisitor)
        }
    }
}
