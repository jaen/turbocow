use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::ops::Deref;
use core::ptr;

use super::types::EcoVec;
use crate::allocator::{AllocatorProvider, Global};

// ── Access ──────────────────────────────────────────────────────────────

impl<T, A> Deref for EcoVec<T, A>
where
    A: AllocatorProvider,
{
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, A> Borrow<[T]> for EcoVec<T, A>
where
    A: AllocatorProvider,
{
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, A> AsRef<[T]> for EcoVec<T, A>
where
    A: AllocatorProvider,
{
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

// ── Comparison ──────────────────────────────────────────────────────────

impl<T, A> Hash for EcoVec<T, A>
where
    T: Hash,
    A: AllocatorProvider,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl<T, A> Eq for EcoVec<T, A>
where
    T: Eq,
    A: AllocatorProvider,
{
}

impl<T, A> PartialEq for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Fast-path: two clones sharing the same backing allocation are
        // trivially equal (same data pointer and same length).
        (self.ptr == other.ptr && self.len == other.len)
            || self.as_slice() == other.as_slice()
    }
}

impl<T, A> PartialEq<[T]> for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

impl<T, A> PartialEq<&[T]> for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &&[T]) -> bool {
        self.as_slice() == *other
    }
}

impl<T, A, const N: usize> PartialEq<[T; N]> for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &[T; N]) -> bool {
        self.as_slice() == other
    }
}

impl<T, A, const N: usize> PartialEq<&[T; N]> for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &&[T; N]) -> bool {
        self.as_slice() == *other
    }
}

impl<T, A> PartialEq<EcoVec<T, A>> for [T]
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &EcoVec<T, A>) -> bool {
        self == other.as_slice()
    }
}

impl<T, A, const N: usize> PartialEq<EcoVec<T, A>> for [T; N]
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &EcoVec<T, A>) -> bool {
        self == other.as_slice()
    }
}

impl<T, A> PartialEq<Vec<T>> for EcoVec<T, A>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T, A> PartialEq<EcoVec<T, A>> for Vec<T>
where
    T: PartialEq,
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &EcoVec<T, A>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T, A> Ord for EcoVec<T, A>
where
    T: Ord,
    A: AllocatorProvider,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T, A> PartialOrd for EcoVec<T, A>
where
    T: PartialOrd,
    A: AllocatorProvider,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

// ── Construction ────────────────────────────────────────────────────────

impl<T> Default for EcoVec<T, Global> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> From<&[T]> for EcoVec<T, Global>
where
    T: Clone,
{
    fn from(slice: &[T]) -> Self {
        let mut vec = Self::with_capacity(slice.len());
        vec.extend_from_slice(slice);
        vec
    }
}

impl<T, const N: usize> From<[T; N]> for EcoVec<T, Global>
where
    T: Clone,
{
    fn from(array: [T; N]) -> Self {
        let mut vec = Self::with_capacity(N);
        unsafe {
            // Safety: Array's IntoIter implements `TrustedLen`.
            vec.extend_from_trusted(array);
        }
        vec
    }
}

impl<T> From<Vec<T>> for EcoVec<T, Global>
where
    T: Clone,
{
    /// Allocates a new EcoVec, and moves other's items into it.
    fn from(mut other: Vec<T>) -> Self {
        let len = other.len();
        let mut vec = Self::with_capacity(len);
        unsafe {
            // Disables dropping of individual `Vec` items that will be moved
            // into the `EcoVec`.
            //
            // Safety: 0 is less than or equal to capacity.
            other.set_len(0);

            // Safety:
            // - The source vector is valid for `len` reads.
            // - The destination is valid for `len` writes due to the
            //   `Self::with_capacity(len)` call.
            // - The source and destination are non-overlapping because we just
            //   allocated the destination.
            core::ptr::copy_nonoverlapping(other.as_ptr(), vec.data_mut(), len);

            // Sets the correct length, and thereby also enables dropping of the
            // individual items that have been moved into the `EcoVec`.
            // There is no possibility of double dropping because we've already
            // set the length of the original `Vec` to 0 before copying.
            vec.len = len;
        }
        vec
    }
}

impl<T, A, const N: usize> TryFrom<EcoVec<T, A>> for [T; N]
where
    T: Clone,
    A: AllocatorProvider,
{
    type Error = EcoVec<T, A>;

    fn try_from(mut vec: EcoVec<T, A>) -> Result<Self, Self::Error> {
        if vec.len() != N {
            return Err(vec);
        }

        Ok(if vec.is_unique() {
            // Set the length to zero to prevent double drop.
            vec.len = 0;

            // Safety: We have unique ownership and len == N.
            unsafe { ptr::read(vec.data() as *const [T; N]) }
        } else {
            // Safety: We know that the length is correct.
            unsafe { core::array::from_fn(|i| vec.get_unchecked(i).clone()) }
        })
    }
}

// ── Iteration ───────────────────────────────────────────────────────────

impl<T, A> Extend<T> for EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider + Clone,
{
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        let iter = iter.into_iter();
        let hint = iter.size_hint().0;
        if hint > 0 {
            self.reserve(hint);
        }
        // After reserve, we're unique and may have spare capacity.
        // Use push_unchecked while capacity allows, then fall back to
        // push (which re-checks unique + grows) only when needed.
        //
        // We hold `&mut self`, so `is_unique()` cannot change during the
        // loop; cache it once to avoid a per-iteration Acquire load.
        let unique = self.is_unique();
        for value in iter {
            if unique && self.len < self.capacity() {
                unsafe { self.push_unchecked(value) };
            } else {
                self.push(value);
            }
        }
    }
}

impl<T> FromIterator<T> for EcoVec<T, Global>
where
    T: Clone,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let hint = iter.size_hint().0;
        let mut vec = Self::with_capacity(hint);
        vec.extend(iter);
        vec
    }
}

impl<'a, T, A> IntoIterator for &'a EcoVec<T, A>
where
    A: AllocatorProvider,
{
    type IntoIter = core::slice::Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<T, A> IntoIterator for EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider,
{
    type IntoIter = IntoIter<T, A>;
    type Item = T;

    #[inline]
    fn into_iter(mut self) -> Self::IntoIter {
        IntoIter {
            unique: self.is_unique(),
            front: 0,
            back: self.len,
            vec: self,
        }
    }
}

/// An owned iterator over an [`EcoVec`](crate::EcoVec).
///
/// If the vector had a reference count of 1, this moves out of the vector,
/// otherwise it lazily clones.
///
/// The second type parameter defaults to `Global` so that
/// `turbocow::vec::IntoIter<T>` resolves without an explicit allocator,
/// matching ecow's single-parameter `ecow::vec::IntoIter<T>`.
pub struct IntoIter<T, A = Global>
where
    A: AllocatorProvider,
{
    vec: EcoVec<T, A>,
    unique: bool,
    front: usize,
    back: usize,
}

impl<T, A> IntoIter<T, A>
where
    A: AllocatorProvider,
{
    /// Returns the remaining items of this iterator as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            core::slice::from_raw_parts(
                self.vec.data().add(self.front),
                self.back - self.front,
            )
        }
    }
}

impl<T, A> Iterator for IntoIter<T, A>
where
    T: Clone,
    A: AllocatorProvider,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.front < self.back).then(|| {
            let prev = self.front;
            self.front += 1;
            if self.unique {
                unsafe { ptr::read(self.vec.data().add(prev)) }
            } else {
                unsafe { self.vec.get_unchecked(prev).clone() }
            }
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }
}

impl<T, A> DoubleEndedIterator for IntoIter<T, A>
where
    T: Clone,
    A: AllocatorProvider,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        (self.back > self.front).then(|| {
            self.back -= 1;
            if self.unique {
                unsafe { ptr::read(self.vec.data().add(self.back)) }
            } else {
                unsafe { self.vec.get_unchecked(self.back).clone() }
            }
        })
    }
}

impl<T, A> ExactSizeIterator for IntoIter<T, A>
where
    T: Clone,
    A: AllocatorProvider,
{
}

impl<T, A> Drop for IntoIter<T, A>
where
    A: AllocatorProvider,
{
    fn drop(&mut self) {
        if !self.unique || !self.vec.is_allocated() {
            return;
        }

        unsafe {
            // Set len to zero before dropping to prevent double dropping in
            // EcoVec's drop impl in case of panic.
            self.vec.len = 0;

            // Drop only the remaining elements in the middle.
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                self.vec.data_mut().add(self.front),
                self.back - self.front,
            ));
        }
    }
}

impl<T, A> core::fmt::Debug for IntoIter<T, A>
where
    T: core::fmt::Debug,
    A: AllocatorProvider,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
