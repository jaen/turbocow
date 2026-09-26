//! Allocator-aware implementation of EcoVec
//!
//! This module contains the implementation of EcoVec when allocator features are enabled.
//! It's kept in a separate file to maintain clarity and avoid excessive conditional compilation.

use core::ptr::NonNull;
use core::{marker::PhantomData, ptr, sync::atomic::Ordering};

use crate::allocator::AllocatorProvider;
use crate::vec::drain::Drain;
use crate::vec::types::EcoVec;

use super::{capacity_overflow, out_of_bounds, ref_count_overflow};

impl<T, A> EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider + Clone,
{
    /// Produce a mutable slice containing the entire vector.
    ///
    /// Clones the vector if its reference count is larger than 1.
    #[inline]
    pub fn make_mut(&mut self) -> &mut [T] {
        // To provide mutable access, we must have unique ownership over the
        // backing allocation.
        self.make_unique();

        // Safety:
        // The reference count is `1` because of `make_unique`.
        // For more details, see `Self::as_slice()`.
        unsafe { core::slice::from_raw_parts_mut(self.data_mut(), self.len) }
    }

    /// Add a value at the end of the vector.
    ///
    /// Clones the vector if its reference count is larger than 1.
    #[inline]
    #[track_caller]
    pub fn push(&mut self, value: T) {
        // Ensure unique ownership and grow the vector if necessary.
        self.reserve((self.len == self.capacity()) as usize);

        // Safety: we just called `EcoVec::reserve()`
        unsafe {
            self.push_unchecked(value);
        }
    }

    /// Add a value at the end of the vector, without reallocating.
    ///
    /// You must ensure that `self.is_unique()` and `self.len < self.capacity()`
    /// hold, by calling `EcoVec::with_capacity()` or `EcoVec::reserve()`.
    #[inline]
    pub(crate) unsafe fn push_unchecked(&mut self, value: T) {
        debug_assert!(self.is_unique());
        debug_assert!(self.len < self.capacity());

        unsafe {
            // Safety:
            // - The caller must ensure that the reference count is `1`.
            // - The pointer returned by `data_mut()` is valid for `capacity`
            //   writes.
            // - The caller must ensure that `len < capacity`.
            // - Thus, `data_mut() + len` is valid for one write.
            ptr::write(self.data_mut().add(self.len), value);

            // Safety:
            // Since we reserved space, we maintain `len <= capacity`.
            self.len += 1;
        }
    }

    /// Removes the last element from a vector and returns it, or `None` if the
    /// vector is empty.
    ///
    /// Clones the vector if its reference count is larger than 1.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        self.make_unique();

        unsafe {
            // Safety:
            // Cannot underflow because `is_empty` returned `false`.
            self.len -= 1;

            // Safety:
            // - The reference count is `1` because of `make_unique`.
            // - The pointer returned by `data()` is valid for `len` reads and
            //   thus `data() + new_len` is valid for one read.
            Some(ptr::read(self.data().add(self.len)))
        }
    }

    /// Ensure that this vector has a unique backing allocation.
    ///
    /// May change the capacity.
    #[inline]
    pub(crate) fn make_unique(&mut self) {
        if !self.is_unique() {
            // An empty EcoVec has no header allocation, so `is_unique`
            // returns true for it — this branch is unreachable for
            // `len == 0`. The `min_cap()` allocation below only runs
            // for non-empty shared vecs.
            //
            // Create a new vec from slice.  Allocate at least min_cap()
            // so that the very next push doesn't immediately realloc.
            let slice = self.as_slice();
            let cap = slice.len().max(Self::min_cap());
            let mut vec = Self::with_capacity_in(cap, self.alloc.clone());
            vec.extend_from_slice(slice);
            *self = vec;
        }
    }

    /// Reserve space for at least `additional` more elements.
    ///
    /// Guarantees that the resulting vector has reference count `1` and space
    /// for `additional` more elements.
    #[inline]
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        let capacity = self.capacity();
        let mut target = capacity;

        if additional > capacity - self.len {
            // Reserve at least the `additional` capacity, but also at least
            // double the capacity to ensure exponential growth and finally
            // jump directly to a minimum capacity to prevent frequent
            // reallocation for small vectors.
            target = self
                .len
                .checked_add(additional)
                .unwrap_or_else(|| capacity_overflow())
                .max(2 * capacity)
                .max(Self::min_cap());
        }

        if !self.is_unique() {
            self.reserve_clone(target);
        } else if target > capacity {
            unsafe {
                // Safety:
                // - The reference count is `1` because of `is_unique`.
                // - The `target` capacity is greater than the current capacity
                //   because `additional > 0`.
                self.grow(target);
            }
        }
    }

    /// Cold path of [`reserve`](Self::reserve): the vector is shared, so its
    /// backing allocation can't be mutated in place — clone the elements into a
    /// fresh, uniquely-owned allocation of `target` capacity.
    ///
    /// Split out and marked `#[cold] #[inline(never)]` so that the common
    /// uniquely-owned `reserve` path stays small enough to inline into
    /// `extend_from_slice`/`From<&[T]>`, where the clone loop then auto-
    /// vectorizes into a `memcpy` for trivially-copyable elements.
    #[cold]
    #[inline(never)]
    fn reserve_clone(&mut self, target: usize) {
        let mut vec = Self::with_capacity_in(target, self.alloc.clone());
        vec.extend_from_slice(self.as_slice());
        *self = vec;
    }

    /// Shrinks the capacity as close to the length as possible.
    ///
    /// If the vector is shared (reference count > 1), this is a no-op —
    /// the backing allocation cannot be shrunk without disturbing the
    /// other owners.
    ///
    /// If the vector is uniquely owned and `capacity > len`, a new
    /// allocation of exactly `len` elements is made and the data moved
    /// into it, releasing the excess capacity.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        // Only shrink if we are the sole owner of the backing allocation.
        if !self.is_unique() {
            return;
        }
        let len = self.len;
        if self.capacity() <= len {
            return;
        }
        // Reallocate to capacity == len (min 1 to avoid the dangling sentinel).
        let new_cap = len.max(1);
        let mut shrunk = Self::with_capacity_in(new_cap, self.alloc.clone());
        shrunk.extend_from_slice(self.as_slice());
        *self = shrunk;
    }

    /// Clones and pushes all elements in a slice to the vector.
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[T]) {
        if slice.is_empty() {
            return;
        }

        self.reserve(slice.len());

        for value in slice {
            // Safety:
            // - The reference count is `1` because of `reserve`.
            // - `self.len < self.capacity()` because we reserved space for
            //   `slice.len()` more elements.
            unsafe {
                self.push_unchecked(value.clone());
            }
        }
    }

    /// Copies all elements from a slice of `Copy` types using `memcpy`.
    ///
    /// This is significantly faster than `extend_from_slice` for `Copy` types
    /// because it avoids per-element clone + write overhead.
    #[inline]
    pub fn extend_from_copy_slice(&mut self, slice: &[T])
    where
        T: Copy,
    {
        if slice.is_empty() {
            return;
        }

        self.reserve(slice.len());

        // Safety: reserve guarantees unique ownership and sufficient capacity.
        unsafe {
            ptr::copy_nonoverlapping(
                slice.as_ptr(),
                self.data_mut().add(self.len),
                slice.len(),
            );
            self.len += slice.len();
        }
    }

    /// Inserts an element at position `index` within the vector, shifting all
    /// elements after it to the right.
    ///
    /// Clones the vector if its reference count is larger than 1.
    ///
    /// Panics if `index > len`.
    #[track_caller]
    pub fn insert(&mut self, index: usize, value: T) {
        if index > self.len {
            out_of_bounds(index, self.len);
        }

        // Ensure unique ownership and grow the vector if necessary.
        self.reserve((self.len == self.capacity()) as usize);

        unsafe {
            let at = self.data_mut().add(index);
            ptr::copy(at, at.add(1), self.len - index);
            ptr::write(at, value);
            self.len += 1;
        }
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// Clones the vector if its reference count is larger than 1.
    ///
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len {
            out_of_bounds(index, self.len);
        }

        self.make_unique();

        unsafe {
            let at = self.data_mut().add(index);
            let value = ptr::read(at);
            ptr::copy(at.add(1), at, self.len - index - 1);
            self.len -= 1;
            value
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// Clones the vector if its reference count is larger than 1.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let len = self.len;
        let values = self.make_mut();

        let mut del = 0;
        for i in 0..len {
            if !f(&mut values[i]) {
                del += 1;
            } else if del > 0 {
                values.swap(i - del, i);
            }
        }

        if del > 0 {
            self.truncate(len - del);
        }
    }

    /// Replaces the element at position `index` with `value`, dropping the old
    /// element.
    ///
    /// Clones the vector if its reference count is larger than 1.
    ///
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn replace(&mut self, index: usize, value: T) {
        if index >= self.len {
            out_of_bounds(index, self.len);
        }

        self.make_unique();

        unsafe {
            let at = self.data_mut().add(index);
            // Panic-safety: ptr::read moves the old value out, ptr::write puts the
            // new value in (single intrinsic — cannot unwind for any T). drop(old)
            // runs T::drop *after* the slot holds the new value, so if T::drop
            // panics, EcoVec::drop will still find a valid value at this slot.
            // The old code did drop_in_place first, which left the slot
            // uninitialized if Drop panicked → double-drop during EcoVec::drop.
            let old = ptr::read(at);
            ptr::write(at, value);
            drop(old);
        }
    }

    /// Truncate the vector to the target length.
    #[inline]
    pub fn truncate(&mut self, target: usize) {
        if target >= self.len {
            return;
        }

        if !self.is_unique() {
            // Safety: Just checked bounds.
            // Create a new truncated vector
            let slice = unsafe { core::slice::from_raw_parts(self.data(), target) };
            let mut vec = Self::with_capacity_in(target, self.alloc.clone());
            vec.extend_from_slice(slice);
            *self = vec;
            return;
        }

        let rest = self.len - target;
        unsafe {
            // Safety:
            // - Since `target < len`, we maintain `len <= capacity`.
            self.len = target;

            // Safety:
            // The reference count is `1` because of `is_unique`.
            // - The pointer returned by `data_mut()` is valid for `capacity`
            //   writes.
            // - We have the invariant `len <= capacity`.
            // - Thus, `data_mut() + target` is valid for `len - target` writes.
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                self.data_mut().add(target),
                rest,
            ));
        }
    }

    /// Removes the subslice indicated by the given range from the vector,
    /// returning a double-ended iterator over the removed subslice.
    ///
    /// If the iterator is dropped before being fully consumed, it drops the
    /// remaining removed elements.
    ///
    /// Clones the vector if its reference count is larger than 1, so draining a
    /// clone never affects the other owners (clone-on-write).
    ///
    /// # Panics
    /// Panics if the starting point is greater than the end point or if the end
    /// point is greater than the length of the vector.
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T, A>
    where
        R: core::ops::RangeBounds<usize>,
    {
        // Adapted from Rust's `Vec::drain` function (MIT).
        // Copyright (c) The Rust Project Contributors.
        // See NOTICE for full attribution.
        //
        // Memory safety
        //
        // When the `Drain` is first created, it shortens the length of the
        // source vector to make sure no uninitialized or moved-from elements
        // are accessible at all if the `Drain`'s destructor never gets to run.
        //
        // `Drain` will `ptr::read` out the values to remove. When finished, the
        // remaining tail of the vec is copied back to cover the hole, and the
        // vector length is restored to the new length.
        use core::ops::Bound;

        let len = self.len;
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            // Match std: an exclusive start of `usize::MAX` overflows and
            // panics rather than silently wrapping to `0`.
            Bound::Excluded(&n) => {
                n.checked_add(1).unwrap_or_else(|| out_of_bounds(n, len))
            }
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            // Match std: an inclusive end of `usize::MAX` overflows and panics
            // rather than silently wrapping to `0`.
            Bound::Included(&n) => {
                n.checked_add(1).unwrap_or_else(|| out_of_bounds(n, len))
            }
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };
        assert!(start <= end, "drain start must not be greater than end");
        assert!(end <= len, "drain end out of bounds");

        // Force unique ownership *before* removing anything, so draining a
        // clone does not affect the other owners. After this, the reference
        // count is `1` and the backing allocation is solely owned, which is the
        // safety invariant `Drain` relies on (no move-from-shared).
        self.make_unique();

        unsafe {
            // Set the length to `start`, so that the vector is left in a sound
            // state (just the prefix `[..start]`) if the `Drain` is leaked.
            self.len = start;

            // Safety:
            // - The reference count is `1` because of `make_unique`.
            // - `start <= end <= len <= capacity`, so `data() + start` is valid
            //   for `end - start` reads of `T`.
            let range_slice =
                core::slice::from_raw_parts(self.data().add(start), end - start);
            Drain {
                tail_start: end,
                tail_len: len - end,
                iter: range_slice.iter(),
                vec: NonNull::from(self),
            }
        }
    }

    /// Pushes all elements in a trusted-len iterator to the vector.
    ///
    /// # Safety
    /// We can't use `TrustedLen` because it is unstable. Still, the
    /// `ExactSizeIterator::len` must return the exact length of the iterator
    /// for this to be safe.
    pub unsafe fn extend_from_trusted<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let iter = iter.into_iter();
        let count = iter.len();

        if count == 0 {
            return;
        }

        self.reserve(count);

        for value in iter {
            // Safety:
            // - The reference count is `1` because of `reserve`.
            // - `self.len < self.capacity()` because we reserved space for
            //   `iter.len()` more elements.
            unsafe {
                self.push_unchecked(value);
            }
        }
    }
}

// Clone for allocator-aware EcoVec
impl<T: Clone, A> Clone for EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider + Clone,
{
    #[inline]
    #[track_caller]
    fn clone(&self) -> Self {
        // If the vector has a backing allocation, bump the ref-count.
        if let Some(header) = self.header() {
            // See Arc's clone impl for details about memory ordering.
            let prev = header.refs.fetch_add(1, Ordering::Relaxed);

            // See Arc's clone impl details about guarding against incredibly degenerate programs
            if prev > isize::MAX as usize {
                ref_count_overflow(self.ptr, self.len, self.alloc.clone());
            }
        }

        Self {
            ptr: self.ptr,
            len: self.len,
            alloc: self.alloc.clone(),
            phantom: PhantomData,
        }
    }
}
