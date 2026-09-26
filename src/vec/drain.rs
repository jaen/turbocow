//! A draining iterator for [`EcoVec`].
//!
//! Adapted from the Rust standard library's `alloc::vec::Drain` (MIT).
//! Copyright (c) The Rust Project Contributors.
//!
//! The design mirrors std/ecow: when a [`Drain`] is created, the source
//! vector's length is shortened to the start of the drained range up front, so
//! that even if the [`Drain`] is leaked (e.g. via [`mem::forget`]) the vector
//! is left in a sound state — it simply loses the tail rather than exposing
//! moved-from or uninitialized elements. On drop, the [`Drain`] drops any
//! not-yet-yielded drained elements and moves the untouched tail back to close
//! the gap, restoring the correct length.

use core::fmt;
use core::iter::FusedIterator;
use core::mem;
use core::ptr::{self, NonNull};
use core::slice;

use crate::allocator::{AllocatorProvider, Global};
use crate::vec::types::EcoVec;

/// A draining iterator for [`EcoVec<T>`](crate::EcoVec).
///
/// This `struct` is created by `EcoVec::drain`. See its
/// documentation for more.
///
/// The second type parameter defaults to `Global` so that
/// `turbocow::vec::Drain<'_, T>` resolves without an explicit allocator,
/// matching ecow's single-parameter `ecow::vec::Drain<'_, T>`.
pub struct Drain<'a, T: 'a, A: AllocatorProvider = Global> {
    /// Index of the tail to preserve.
    pub(crate) tail_start: usize,
    /// Length of the tail.
    pub(crate) tail_len: usize,
    /// Current remaining range to remove.
    pub(crate) iter: slice::Iter<'a, T>,
    /// Invariant: the ref count of `vec` is `1`, because it has been made
    /// unique in the corresponding [`EcoVec::drain`] call, and is mutably
    /// borrowed by the [`Drain`] struct.
    pub(crate) vec: NonNull<EcoVec<T, A>>,
}

impl<T: fmt::Debug, A: AllocatorProvider> fmt::Debug for Drain<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Drain").field(&self.iter.as_slice()).finish()
    }
}

impl<'a, T, A: AllocatorProvider> Drain<'a, T, A> {
    /// Returns the remaining items of this iterator as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.iter.as_slice()
    }
}

impl<T, A: AllocatorProvider> AsRef<[T]> for Drain<'_, T, A> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

// Safety: `Drain` only ever exposes `&T` / owned `T` to the outside and holds a
// pointer to an `EcoVec` whose backing allocation it uniquely owns for the
// duration of the borrow. The bounds match std/ecow: `T: Sync`/`T: Send` is
// sufficient because the allocator is only used during element drop on the
// owning thread and `EcoVec`'s own `Send`/`Sync` already require the allocator
// to be thread-safe.
unsafe impl<T: Sync, A: AllocatorProvider + Sync> Sync for Drain<'_, T, A> {}
unsafe impl<T: Send, A: AllocatorProvider + Send> Send for Drain<'_, T, A> {}

impl<T, A: AllocatorProvider> Iterator for Drain<'_, T, A> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        // Safety: each element pointer yielded by the slice iterator points to
        // an initialized `T` that we uniquely own and have not yet moved out
        // of. We read it out by value exactly once (the iterator never yields
        // the same element twice), transferring ownership to the caller.
        self.iter.next().map(|elt| unsafe { ptr::read(elt as *const _) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T, A: AllocatorProvider> DoubleEndedIterator for Drain<'_, T, A> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        // Safety: see `next`.
        self.iter.next_back().map(|elt| unsafe { ptr::read(elt as *const _) })
    }
}

impl<T, A: AllocatorProvider> Drop for Drain<'_, T, A> {
    fn drop(&mut self) {
        /// Moves back the un-`Drain`ed elements to restore the original
        /// vector, even if dropping a drained element panics.
        struct DropGuard<'r, 'a, T, A: AllocatorProvider>(&'r mut Drain<'a, T, A>);

        impl<T, A: AllocatorProvider> Drop for DropGuard<'_, '_, T, A> {
            fn drop(&mut self) {
                if self.0.tail_len > 0 {
                    unsafe {
                        let source_vec = self.0.vec.as_mut();
                        // memmove back the untouched tail, update to the new
                        // length.
                        let start = source_vec.len();
                        let tail = self.0.tail_start;
                        if tail != start {
                            let src = source_vec.data().add(tail);
                            let dst = source_vec.data_mut().add(start);
                            ptr::copy(src, dst, self.0.tail_len);
                        }
                        source_vec.len = start + self.0.tail_len;
                    }
                }
            }
        }

        let iter = mem::take(&mut self.iter);
        let drop_len = iter.len();

        let mut vec = self.vec;

        if mem::size_of::<T>() == 0 {
            // ZSTs have no identity, so we don't need to move them around; we
            // only need to drop the correct amount. This is achieved by
            // manipulating the vector length rather than moving values out of
            // `iter`.
            unsafe {
                let vec = vec.as_mut();
                let old_len = vec.len();
                // Temporarily expose all drained + tail ZSTs so the truncate
                // below drops exactly `drop_len` of them, leaving the tail.
                vec.len = old_len + drop_len + self.tail_len;
                // Drop the `drop_len` not-yet-yielded drained ZSTs, restoring
                // the length to `old_len + tail_len`.
                //
                // Safety:
                // - The invariant of `vec` says that the ref count is `1`.
                // - `old_len + self.tail_len` is `<= old_len + drop_len +
                //   self.tail_len`, the value we just set the length to, so the
                //   truncate only drops the `drop_len` drained ZSTs.
                drop_zst_tail(vec, old_len + self.tail_len);
            }

            return;
        }

        // Ensure elements are moved back into their appropriate places, even
        // when `drop_in_place` panics.
        let _guard = DropGuard(self);

        if drop_len == 0 {
            return;
        }

        // Derive the drop-path base pointer from the remaining slice. The
        // `offset_from` below is only valid when that pointer is into the
        // vector's allocation: for ZSTs `slice::Iter`'s backing pointer can be
        // dangling, but the ZST case is handled above (early return), and the
        // `drop_len == 0` check above guarantees the slice is non-empty here, so
        // the pointer is a genuine in-bounds element pointer. (This is a local
        // precondition for the pointer arithmetic, not a restriction on the
        // public `as_slice` / `slice::Iter::as_slice`, which are well-defined
        // for ZSTs and empty iterators.)
        let drop_ptr = iter.as_slice().as_ptr();

        unsafe {
            // `drop_ptr` comes from a `slice::Iter` which only gives us a `&[T]`
            // but `drop_in_place` needs a pointer with mutable provenance.
            // Reconstruct it from the original vec while avoiding creating a
            // `&mut` to the front (which could invalidate raw pointers some
            // unsafe code might rely on).
            let vec_ptr = vec.as_mut().data_mut();
            let drop_offset = drop_ptr.offset_from(vec_ptr) as usize;
            let to_drop =
                ptr::slice_from_raw_parts_mut(vec_ptr.add(drop_offset), drop_len);
            ptr::drop_in_place(to_drop);
        }
    }
}

/// Drops the trailing ZST elements of a uniquely-owned `vec` down to `target`.
///
/// # Safety
/// - The reference count of `vec` must be `1`.
/// - `target <= vec.len()`.
/// - `T` must be a ZST (this is only correct because dropping ZSTs needs no
///   pointer identity).
#[inline]
unsafe fn drop_zst_tail<T, A: AllocatorProvider>(vec: &mut EcoVec<T, A>, target: usize) {
    debug_assert!(mem::size_of::<T>() == 0);
    let len = vec.len();
    debug_assert!(target <= len);
    let rest = len - target;
    // Safety: `target <= len <= capacity` so the new length upholds the
    // `len <= capacity` invariant.
    vec.len = target;
    // Safety: the data pointer is valid for `len` reads; for a ZST any
    // in-bounds offset yields a valid (dangling-but-aligned) pointer, and
    // `drop_in_place` on a ZST slice never dereferences memory.
    unsafe {
        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
            vec.data_mut().add(target),
            rest,
        ));
    }
}

impl<T, A: AllocatorProvider> ExactSizeIterator for Drain<'_, T, A> {}

impl<T, A: AllocatorProvider> FusedIterator for Drain<'_, T, A> {}
