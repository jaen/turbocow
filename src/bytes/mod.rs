//! A clone-on-write byte buffer with inline storage.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::{self, Debug, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::Deref;

use crate::allocator::Global;
use crate::dynamic::{DynamicVec, InlineVec, LIMIT};
use crate::string::EcoString;
use crate::vec::EcoVec;

#[cfg(feature = "rkyv")]
mod rkyv_impl;

/// An economical byte buffer with inline storage and clone-on-write semantics.
///
/// This is the byte-oriented counterpart of [`EcoString`]. It stores arbitrary
/// `[u8]` data (not necessarily valid UTF-8) with the same 16-byte footprint,
/// 15-byte inline storage, and clone-on-write heap fallback to an
/// [`EcoVec<u8>`]. The internal reference counter of the heap variant is
/// atomic, making this type [`Sync`] and [`Send`].
///
/// Like [`EcoString`], it can also borrow `'static` data without copying — see
/// [`EcoBytes::from_static`].
///
/// # Example
/// ```
/// use turbocow::EcoBytes;
///
/// // This is stored inline.
/// let small = EcoBytes::from(b"Welcome");
///
/// // This spills to the heap. The clone shares its allocation until mutation.
/// let mut big = small.repeat(3);
/// let clone = big.clone();
/// big.push(b'!');
/// assert_ne!(big, clone);
/// ```
///
/// # Note
/// The above holds true for normal 32-bit or 64-bit little-endian systems. On
/// 64-bit big-endian systems, the type's size increases to 24 bytes and the
/// amount of inline storage to 23 bytes.
#[derive(Clone)]
pub struct EcoBytes(DynamicVec<'static>);

impl EcoBytes {
    /// Maximum number of bytes for an inline `EcoBytes` before spilling to the
    /// heap.
    ///
    /// The exact value for this is architecture dependent.
    ///
    /// # Note
    /// This value is semver exempt and can be changed with any update.
    pub const INLINE_LIMIT: usize = LIMIT;

    /// Create a new, empty byte buffer.
    ///
    /// This does not allocate.
    #[inline]
    pub const fn new() -> Self {
        Self(DynamicVec::new())
    }

    /// Create a new, inline byte buffer.
    ///
    /// Panics if the slice's length exceeds the capacity of the inline storage.
    #[inline]
    #[track_caller]
    pub const fn inline(bytes: &[u8]) -> Self {
        let Ok(inline) = InlineVec::from_slice(bytes) else {
            exceeded_inline_capacity();
        };
        Self(DynamicVec::from_inline(inline))
    }

    /// Try to create a new, inline byte buffer.
    ///
    /// Returns `None` if the slice's length exceeds the capacity of the inline
    /// storage.
    #[inline]
    pub const fn try_inline(bytes: &[u8]) -> Option<Self> {
        match InlineVec::from_slice(bytes) {
            Ok(inline) => Some(Self(DynamicVec::from_inline(inline))),
            Err(()) => None,
        }
    }

    /// Create a byte buffer that **borrows** a `'static` byte slice without
    /// copying (the Referenced variant).
    ///
    /// Construction is essentially free — no allocation or copy occurs. The
    /// bytes are copied on the first mutation.
    #[inline]
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self(DynamicVec::from_slice_in(bytes, Global))
    }

    /// Creates a new, empty byte buffer with at least the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(DynamicVec::with_capacity(capacity))
    }

    /// Returns `true` if the buffer contains no bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of bytes in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// How many bytes the buffer can hold without (re-)allocating.
    ///
    /// If the buffer's heap allocation is shared, mutation can still allocate
    /// even when the requested length fits within this capacity. A buffer
    /// created with [`from_static`](Self::from_static) owns no storage, so its
    /// capacity is its length.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Whether this byte buffer is stored inline.
    ///
    /// A buffer created with [`from_static`](Self::from_static) is borrowed,
    /// not inline.
    #[inline]
    pub fn is_inline(&self) -> bool {
        self.0.is_pure_inline()
    }

    /// Extracts a slice containing the entire buffer.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Produces a mutable slice containing the entire buffer.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    #[inline]
    pub fn make_mut(&mut self) -> &mut [u8] {
        self.0.make_mut(0)
    }

    /// Adds a byte at the end of the buffer.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    #[inline]
    pub fn push(&mut self, byte: u8) {
        self.0.push(byte);
    }

    /// Removes and returns the last byte, or returns `None` if the buffer is
    /// empty.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    #[inline]
    pub fn pop(&mut self) -> Option<u8> {
        let last = *self.as_slice().last()?;
        self.0.truncate(self.len() - 1);
        Some(last)
    }

    /// Inserts a byte at an `index` within the buffer, shifting all bytes after
    /// it to the right.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    ///
    /// Panics if `index > len`.
    #[inline]
    #[track_caller]
    pub fn insert(&mut self, index: usize, byte: u8) {
        self.0.insert_slice(index, &[byte]);
    }

    /// Removes and returns the byte at position index within the buffer,
    /// shifting all bytes after it to the left.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    ///
    /// Panics if `index >= len`.
    #[inline]
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> u8 {
        self.0.remove(index)
    }

    /// Copies and pushes all bytes in a slice to the buffer.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    #[inline]
    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    /// Inserts the given byte slice at the `index`.
    ///
    /// Clones the buffer if it's spilled and its reference count is larger than
    /// 1.
    ///
    /// Panics if `index > len`.
    #[inline]
    #[track_caller]
    pub fn insert_slice(&mut self, index: usize, bytes: &[u8]) {
        self.0.insert_slice(index, bytes);
    }

    /// Removes all bytes from the buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Shortens the buffer, keeping the first `target` bytes and dropping the
    /// rest.
    ///
    /// If `target` is greater than or equal to the current length, this has no
    /// effect.
    #[inline]
    pub fn truncate(&mut self, target: usize) {
        self.0.truncate(target);
    }

    /// Reserves space for at least `additional` more bytes.
    ///
    /// Guarantees that the resulting buffer has space for `additional` more
    /// bytes and owns its storage: if spilled, it uniquely owns its backing
    /// allocation, and a buffer borrowed via [`from_static`](Self::from_static)
    /// is copied.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Repeats this byte buffer `n` times.
    pub fn repeat(&self, n: usize) -> Self {
        let slice = self.as_slice();
        let capacity = slice.len().saturating_mul(n);
        let mut vec = DynamicVec::with_capacity(capacity);
        for _ in 0..n {
            vec.extend_from_slice(slice);
        }
        Self(vec)
    }

    /// Convert this byte buffer into an [`EcoString`], replacing invalid UTF-8
    /// sequences with `U+FFFD REPLACEMENT CHARACTER`.
    ///
    /// If the bytes are already valid UTF-8, this is a zero-copy conversion.
    /// Use [`EcoString::try_from`] to reject invalid UTF-8 instead.
    pub fn into_eco_string_lossy(self) -> EcoString {
        if core::str::from_utf8(self.as_slice()).is_ok() {
            // Just validated as UTF-8.
            EcoString::from_raw(self.0)
        } else {
            EcoString::from(String::from_utf8_lossy(self.as_slice()))
        }
    }
}

impl Deref for EcoBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Default for EcoBytes {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for EcoBytes {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}

impl Eq for EcoBytes {}

impl PartialEq for EcoBytes {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<[u8]> for EcoBytes {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice() == other
    }
}

impl PartialEq<&[u8]> for EcoBytes {
    #[inline]
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_slice() == *other
    }
}

impl<const N: usize> PartialEq<[u8; N]> for EcoBytes {
    #[inline]
    fn eq(&self, other: &[u8; N]) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<const N: usize> PartialEq<&[u8; N]> for EcoBytes {
    #[inline]
    fn eq(&self, other: &&[u8; N]) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<Vec<u8>> for EcoBytes {
    #[inline]
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<EcoVec<u8>> for EcoBytes {
    #[inline]
    fn eq(&self, other: &EcoVec<u8>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<EcoBytes> for [u8] {
    #[inline]
    fn eq(&self, other: &EcoBytes) -> bool {
        self == other.as_slice()
    }
}

impl PartialEq<EcoBytes> for &[u8] {
    #[inline]
    fn eq(&self, other: &EcoBytes) -> bool {
        *self == other.as_slice()
    }
}

impl<const N: usize> PartialEq<EcoBytes> for [u8; N] {
    #[inline]
    fn eq(&self, other: &EcoBytes) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<EcoBytes> for Vec<u8> {
    #[inline]
    fn eq(&self, other: &EcoBytes) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<EcoBytes> for EcoVec<u8> {
    #[inline]
    fn eq(&self, other: &EcoBytes) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Ord for EcoBytes {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl PartialOrd for EcoBytes {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for EcoBytes {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl AsRef<[u8]> for EcoBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Borrow<[u8]> for EcoBytes {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<&[u8]> for EcoBytes {
    #[inline]
    fn from(bytes: &[u8]) -> Self {
        Self(DynamicVec::from_slice(bytes))
    }
}

impl<const N: usize> From<&[u8; N]> for EcoBytes {
    #[inline]
    fn from(bytes: &[u8; N]) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl<const N: usize> From<[u8; N]> for EcoBytes {
    #[inline]
    fn from(bytes: [u8; N]) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl From<Vec<u8>> for EcoBytes {
    /// For short data that fits inline, copies. For longer data, moves the
    /// bytes from the `Vec<u8>` into a new `EcoVec` allocation (mirrors
    /// `From<String>` for `EcoString`).
    #[inline]
    fn from(bytes: Vec<u8>) -> Self {
        if bytes.len() <= LIMIT {
            Self::from(bytes.as_slice())
        } else {
            Self(DynamicVec::from_eco(EcoVec::from(bytes)))
        }
    }
}

impl From<&Vec<u8>> for EcoBytes {
    #[inline]
    fn from(bytes: &Vec<u8>) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl From<EcoVec<u8>> for EcoBytes {
    /// This does not allocate. The resulting byte buffer remains spilled even
    /// if its contents would fit inline.
    #[inline]
    fn from(bytes: EcoVec<u8>) -> Self {
        Self(DynamicVec::from_eco(bytes))
    }
}

impl From<&EcoBytes> for EcoBytes {
    #[inline]
    fn from(bytes: &EcoBytes) -> Self {
        bytes.clone()
    }
}

impl From<Cow<'_, [u8]>> for EcoBytes {
    #[inline]
    fn from(bytes: Cow<'_, [u8]>) -> Self {
        match bytes {
            Cow::Borrowed(bytes) => Self::from(bytes),
            Cow::Owned(bytes) => Self::from(bytes),
        }
    }
}

impl From<EcoString> for EcoBytes {
    /// Zero-cost conversion: moves the underlying storage directly.
    #[inline]
    fn from(string: EcoString) -> Self {
        Self(string.into_raw())
    }
}

impl From<&EcoString> for EcoBytes {
    #[inline]
    fn from(string: &EcoString) -> Self {
        Self::from(string.clone())
    }
}

impl TryFrom<EcoBytes> for EcoString {
    type Error = core::str::Utf8Error;

    /// Zero-copy conversion if the bytes are valid UTF-8.
    #[inline]
    fn try_from(bytes: EcoBytes) -> Result<Self, Self::Error> {
        core::str::from_utf8(bytes.as_slice())?;
        Ok(EcoString::from_raw(bytes.0))
    }
}

impl From<EcoBytes> for Vec<u8> {
    /// This needs to allocate to change the layout.
    #[inline]
    fn from(bytes: EcoBytes) -> Self {
        bytes.as_slice().to_vec()
    }
}

impl From<&EcoBytes> for Vec<u8> {
    #[inline]
    fn from(bytes: &EcoBytes) -> Self {
        bytes.as_slice().to_vec()
    }
}

impl From<EcoBytes> for EcoVec<u8> {
    /// When the byte buffer is stored inline (or borrowed), this needs to
    /// allocate to change the layout. Otherwise, it reuses the existing
    /// allocation.
    #[inline]
    fn from(bytes: EcoBytes) -> Self {
        bytes.0.into_eco()
    }
}

impl From<&EcoBytes> for EcoVec<u8> {
    #[inline]
    fn from(bytes: &EcoBytes) -> Self {
        bytes.clone().0.into_eco()
    }
}

impl FromIterator<u8> for EcoBytes {
    #[inline]
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        let mut bytes = Self::new();
        bytes.extend(iter);
        bytes
    }
}

impl FromIterator<Self> for EcoBytes {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let mut bytes = Self::new();
        for piece in iter {
            bytes.extend_from_slice(&piece);
        }
        bytes
    }
}

impl<'a> FromIterator<&'a [u8]> for EcoBytes {
    #[inline]
    fn from_iter<T: IntoIterator<Item = &'a [u8]>>(iter: T) -> Self {
        let mut bytes = Self::new();
        bytes.extend(iter);
        bytes
    }
}

impl Extend<u8> for EcoBytes {
    #[inline]
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        for byte in iter {
            self.push(byte);
        }
    }
}

impl<'a> Extend<&'a u8> for EcoBytes {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.extend(iter.into_iter().copied());
    }
}

impl<'a> Extend<&'a [u8]> for EcoBytes {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a [u8]>>(&mut self, iter: T) {
        iter.into_iter().for_each(move |bytes| self.extend_from_slice(bytes));
    }
}

impl<'a> IntoIterator for &'a EcoBytes {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

#[cfg(feature = "std")]
impl std::io::Write for EcoBytes {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cold]
#[track_caller]
const fn exceeded_inline_capacity() -> ! {
    panic!("exceeded inline capacity");
}

#[cfg(feature = "serde")]
mod serde {
    use super::EcoBytes;

    use core::fmt;
    use serde::de::{Deserializer, SeqAccess, Visitor};

    impl serde::Serialize for EcoBytes {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_bytes(self.as_slice())
        }
    }

    impl<'de> serde::Deserialize<'de> for EcoBytes {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(EcoBytesVisitor)
        }
    }

    struct EcoBytesVisitor;

    impl<'de> Visitor<'de> for EcoBytesVisitor {
        type Value = EcoBytes;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte buffer")
        }

        // Self-describing formats without a native byte type (e.g. JSON)
        // serialize bytes as a sequence of numbers.
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            // Cap the pre-allocation: the size hint comes from untrusted input.
            let len = seq.size_hint().unwrap_or(0).min(4096);
            let mut bytes = EcoBytes::with_capacity(len);
            while let Some(byte) = seq.next_element()? {
                bytes.push(byte);
            }
            Ok(bytes)
        }

        fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(EcoBytes::from(bytes))
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_bytes(string.as_bytes())
        }
    }
}
