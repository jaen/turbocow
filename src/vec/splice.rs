//! A splicing iterator for [`EcoVec`].
//!
//! Adapted from the Rust standard library's `alloc::vec::Splice` (MIT) and
//! ecow 0.3.0's vendored `Splice`.
//! Copyright (c) The Rust Project Contributors.
//!
//! [`Splice`] is built on top of [`Drain`]: it reuses the drain's
//! up-front length-shortening (so a leaked/forgotten splice leaves the vector
//! sound, exposing only the prefix `[..start]`) and its tail-move-back
//! machinery. Iterating a [`Splice`] yields the *removed* elements of the
//! drained range. On drop it (a) drops any not-yet-yielded removed elements,
//! and (b) fills the hole left behind with the `replace_with` elements,
//! growing the buffer and shifting the tail right when the replacement is
//! longer than the drained range, or letting `Drain::drop` move the tail left
//! when it is shorter.

extern crate alloc;

use core::fmt;
use core::ptr;
use core::slice;

use crate::allocator::{AllocatorProvider, Global};
use crate::vec::drain::Drain;
use crate::vec::types::EcoVec;

/// A splicing iterator for [`EcoVec<T>`](crate::EcoVec).
///
/// This `struct` is created by `EcoVec::splice`. See
/// its documentation for more.
///
/// The final type parameter defaults to `Global` so that
/// `turbocow::vec::Splice<'_, I>` resolves without an explicit allocator,
/// matching ecow's two-parameter `ecow::vec::Splice<'_, I>`.
pub struct Splice<'a, I: Iterator + 'a, A: AllocatorProvider + Clone = Global>
where
    // The `Clone` bound on `I::Item` is required here because the `Drop`
    // implementation calls `EcoVec` methods (`reserve`/`grow`) that require it,
    // mirroring std and ecow. The `A: Clone` bound is likewise needed by those
    // methods; because Rust requires a `Drop` impl's bounds to match the
    // struct's, it is declared on the struct itself rather than only on `Drop`.
    // `Global` is `Clone`, so the default-allocator surface is unaffected.
    I::Item: Clone,
{
    pub(crate) drain: Drain<'a, I::Item, A>,
    pub(crate) replace_with: I,
}

impl<I, A> fmt::Debug for Splice<'_, I, A>
where
    I: Iterator + fmt::Debug,
    I::Item: Clone + fmt::Debug,
    A: AllocatorProvider + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Splice")
            .field("drain", &self.drain)
            .field("replace_with", &self.replace_with)
            .finish()
    }
}

impl<I: Iterator, A: AllocatorProvider + Clone> Iterator for Splice<'_, I, A>
where
    I::Item: Clone,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.drain.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.drain.size_hint()
    }
}

impl<I: Iterator, A: AllocatorProvider + Clone> DoubleEndedIterator for Splice<'_, I, A>
where
    I::Item: Clone,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.drain.next_back()
    }
}

impl<I: Iterator, A: AllocatorProvider + Clone> ExactSizeIterator for Splice<'_, I, A> where
    I::Item: Clone
{
}

impl<I: Iterator, A> Drop for Splice<'_, I, A>
where
    I::Item: Clone,
    A: AllocatorProvider + Clone,
{
    fn drop(&mut self) {
        // Drop (or finish yielding) any not-yet-yielded removed elements first.
        self.drain.by_ref().for_each(drop);
        // At this point draining is done and the only remaining tasks are
        // splicing and moving things into the final place. Reset the inner
        // slice iterator to an empty one so that `Drain::drop` can still call
        // `iter.len()` (`== 0`) without ever computing `offset_from` over a
        // pointer into the (possibly reallocated) buffer, which would otherwise
        // be undefined behaviour.
        self.drain.iter = [].iter();

        unsafe {
            if self.drain.tail_len == 0 {
                // No tail to preserve: just append the replacement at the end.
                // The vector is unique (drain invariant) and its length is
                // `tail_start` (== prefix end), so `extend` appends in place.
                self.drain.vec.as_mut().extend(self.replace_with.by_ref());
                return;
            }

            // First fill the range left by `drain()`.
            if !self.drain.fill(&mut self.replace_with) {
                return;
            }

            // There may be more elements. Use the lower bound as an estimate.
            let (lower_bound, _upper_bound) = self.replace_with.size_hint();
            if lower_bound > 0 {
                self.drain.move_tail(lower_bound);
                if !self.drain.fill(&mut self.replace_with) {
                    return;
                }
            }

            // Collect any remaining elements.
            // This is a zero-length vector which does not allocate if
            // `lower_bound` was exact.
            let mut collected = self
                .replace_with
                .by_ref()
                .collect::<alloc::vec::Vec<I::Item>>()
                .into_iter();
            // Now we have an exact count.
            if collected.len() > 0 {
                self.drain.move_tail(collected.len());
                let filled = self.drain.fill(&mut collected);
                debug_assert!(filled);
                debug_assert_eq!(collected.len(), 0);
            }
        }
        // Let `Drain::drop` move the tail back if necessary and restore
        // `vec.len`.
    }
}

/// Private helper methods for `Splice::drop`.
impl<T: Clone, A: AllocatorProvider + Clone> Drain<'_, T, A> {
    /// The range from `self.vec.len` to `self.tail_start` contains elements
    /// that have been moved out (the hole).
    ///
    /// Fill that range as much as possible with new elements from the
    /// `replace_with` iterator. Returns `true` if we filled the entire range
    /// (i.e. `replace_with.next()` didn't return `None`).
    ///
    /// # Safety
    /// The reference count of `self.vec` must be `1`, and the range
    /// `[vec.len(), self.tail_start)` must be the in-bounds hole owned by the
    /// vector: slots that are *logically* uninitialized (owned but not live —
    /// the elements that previously lived there were `ptr::read` out by
    /// `Drain`), so overwriting them with `ptr::write` does not leak a live `T`.
    unsafe fn fill<I: Iterator<Item = T>>(&mut self, replace_with: &mut I) -> bool {
        let vec = unsafe { self.vec.as_mut() };
        let range_start = vec.len();
        let range_end = self.tail_start;
        // Safety: `range_start <= range_end <= capacity` (the hole lies before
        // the tail, which is itself within capacity), so `data_mut() +
        // range_start` is valid for `range_end - range_start` writes.
        let range_slice = unsafe {
            slice::from_raw_parts_mut(
                vec.data_mut().add(range_start),
                range_end - range_start,
            )
        };

        for place in range_slice {
            let Some(new_item) = replace_with.next() else {
                return false;
            };
            // Safety: `place` points into the hole — a slot that is logically
            // uninitialized (its former element was `ptr::read` out by
            // `Drain`), so writing does not overwrite (and thus leak) a live
            // element.
            unsafe { ptr::write(place, new_item) };
            // Bump the length one element at a time so that, should a future
            // `replace_with.next()` panic, `Drain::drop` (run by the unwinding
            // guard) only ever sees initialized elements in `[..vec.len()]`.
            vec.len += 1;
        }
        true
    }

    /// Makes room for inserting `additional` more elements before the tail,
    /// growing the buffer if necessary and shifting the preserved tail to the
    /// right.
    ///
    /// # Safety
    /// The reference count of `self.vec` must be `1`.
    unsafe fn move_tail(&mut self, additional: usize) {
        let vec = unsafe { self.vec.as_mut() };
        let len = self.tail_start + self.tail_len;
        let capacity = vec.capacity();
        if additional > capacity - len {
            // `reserve` keeps the vector unique (it already is) and grows the
            // backing allocation in place. `vec.len()` is currently the prefix
            // length, so we reserve relative to the *logical* end-of-data
            // (`len`); reserve enough so that `len + additional <= capacity`.
            //
            // We temporarily expose the full logical length so `reserve`
            // preserves the tail bytes on reallocation, then restore the
            // prefix-only length. The tail occupies `[tail_start, len)`; bytes
            // in `[vec.len(), tail_start)` are the (currently empty) hole.
            let saved_len = vec.len;
            // Safety: `len <= capacity`, so the temporary length upholds the
            // `len <= capacity` invariant. Every element in `[..len]` is
            // initialized: `[..saved_len]` is the prefix and `[tail_start,
            // len)` is the tail; the hole `[saved_len, tail_start)` is only
            // entered when `saved_len < tail_start`, but in that branch the
            // hole was already fully filled by `fill` (which returned `true`),
            // so `saved_len == tail_start` whenever we reach `move_tail`.
            debug_assert_eq!(saved_len, self.tail_start);
            vec.len = len;
            vec.reserve(additional);
            vec.len = saved_len;
        }

        let new_tail_start = self.tail_start + additional;
        // Safety: after the (possible) reservation, `new_tail_start +
        // tail_len <= capacity`, so both `src` and `dst` ranges are in-bounds;
        // `ptr::copy` (memmove) tolerates overlap.
        unsafe {
            let src = vec.data().add(self.tail_start);
            let dst = vec.data_mut().add(new_tail_start);
            ptr::copy(src, dst, self.tail_len);
        }
        self.tail_start = new_tail_start;
    }
}

impl<T, A> EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider + Clone,
{
    /// Creates a splicing iterator that replaces the specified range in the
    /// vector with the given `replace_with` iterator and yields the removed
    /// items.
    ///
    /// The `range` is removed even if the returned iterator is not consumed
    /// until the end. It is unspecified how many elements are removed from the
    /// vector if the `Splice` value is leaked.
    ///
    /// Clones the vector if its reference count is larger than 1, so splicing a
    /// clone never affects the other owners (clone-on-write).
    ///
    /// # Panics
    /// Panics if the starting point is greater than the end point or if the end
    /// point is greater than the length of the vector.
    ///
    /// # Example
    /// ```
    /// use turbocow::EcoVec;
    ///
    /// let mut v: EcoVec<i32> = (0..5).collect();
    /// let removed: Vec<i32> = v.splice(1..3, [10, 11, 12]).collect();
    /// assert_eq!(removed, vec![1, 2]);
    /// assert_eq!(v, [0, 10, 11, 12, 3, 4]);
    /// ```
    #[inline]
    pub fn splice<R, I>(
        &mut self,
        range: R,
        replace_with: I,
    ) -> Splice<'_, I::IntoIter, A>
    where
        R: core::ops::RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        // Adapted from Rust's `Vec::splice` function (MIT).
        // Copyright (c) The Rust Project Contributors.
        // See NOTICE for full attribution.
        //
        // `drain` forces unique ownership (CoW) up front and shortens the
        // length to the start of the range, so the splice is built on a sound,
        // uniquely-owned buffer.
        Splice {
            drain: self.drain(range),
            replace_with: replace_with.into_iter(),
        }
    }
}
