use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull};

use crate::allocator::{AllocatorProvider, alloc::Layout};
use crate::sync::atomic::Ordering::*;

use crate::utils::max;
use crate::vec::types::{EcoVec, Header};

#[cold]
#[track_caller]
pub(in crate::vec) fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}

#[cold]
#[track_caller]
pub(in crate::vec) fn ref_count_overflow<T, A: AllocatorProvider>(
    ptr: NonNull<T>,
    len: usize,
    alloc: A,
) -> ! {
    // Roll back the `fetch_add(1)` from `Clone::clone` (cloneable.rs:502) by
    // dropping a temporary EcoVec that shares the allocation. `Drop` does
    // `fetch_sub(1, Release)` and, because the count is still far above 1
    // after overflow, returns early without deallocating — so `alloc` is never
    // used, but is passed to keep the temporary well-formed. Matches ecow 0.3.0:
    // reference/ecow/src/vec.rs:1231.
    drop(EcoVec { ptr, len, alloc, phantom: PhantomData });
    panic!("reference count overflow");
}

#[cold]
#[track_caller]
pub(in crate::vec) fn out_of_bounds(index: usize, len: usize) -> ! {
    panic!("index is out bounds (index: {index}, len: {len})");
}

// Debug implementation for allocator-aware EcoVec
impl<T, A> core::fmt::Debug for EcoVec<T, A>
where
    T: core::fmt::Debug,
    A: AllocatorProvider,
{
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T, A> EcoVec<T, A>
where
    A: AllocatorProvider,
{
    /// Returns a reference to the allocator.
    #[inline]
    pub const fn allocator(&self) -> &A {
        &self.alloc
    }

    /// Returns `true` if the vector contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of elements in the vector.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// How many elements the vector's backing allocation can hold.
    ///
    /// Even if `len < capacity`, pushing into the vector may still
    /// allocate if the reference count is larger than one.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.header().map_or(0, |header| header.capacity)
    }

    /// Extracts a slice containing the entire vector.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // Safety:
        // - The pointer returned by `data()` is non-null, well-aligned, and
        //   valid for `len` reads of `T`.
        // - We have the invariant `len <= capacity <= isize::MAX`.
        // - The memory referenced by the slice isn't mutated for the returned
        //   slice's lifetime, because `self` becomes borrowed and even if there
        //   are other vectors referencing the same backing allocation, they are
        //   now allowed to mutate the slice since then the ref-count is larger
        //   than one.
        unsafe { core::slice::from_raw_parts(self.data(), self.len) }
    }

    /// Removes all values from the vector.
    pub fn clear(&mut self)
    where
        A: Clone,
    {
        // Nothing to do if it's empty.
        if self.is_empty() {
            return;
        }

        // If there are other vectors that reference the same backing
        // allocation, we just create a new, empty vector.
        if !self.is_unique() {
            // If another vector was dropped in the meantime, this vector could
            // have become unique, but we don't care, creating a new one
            // is safe nonetheless. Note that this runs the vector's drop
            // impl and reduces the ref-count.
            *self = Self::new_in(self.alloc.clone());
            return;
        }

        unsafe {
            let prev = self.len;
            self.len = 0;

            // Safety:
            // - We set the length to zero first in case a drop panics, so we
            //   leak rather than double dropping.
            // - We have unique ownership of the backing allocation, so we can
            //   keep it and clear it. In particular, no other vector can have
            //   gained shared ownership in the meantime since `is_unique()`,
            //   as this is the only live vector available for cloning and we
            //   hold a mutable reference to it.
            // - The pointer returned by `data_mut()` is valid for `capacity`
            //   writes, we have the invariant `prev <= capacity` and thus,
            //   `data_mut()` is valid for `prev` writes.
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.data_mut(), prev));
        }
    }

    /// Whether this vector has a backing allocation.
    #[inline]
    pub(crate) fn is_allocated(&self) -> bool {
        !ptr::eq(self.ptr.as_ptr(), Self::dangling().as_ptr())
    }

    /// An immutable pointer to the backing allocation.
    ///
    /// May only be called if `is_allocated` returns `true`.
    #[inline]
    unsafe fn allocation(&self) -> *const u8 {
        unsafe {
            debug_assert!(self.is_allocated());
            self.ptr.as_ptr().cast::<u8>().sub(Self::offset())
        }
    }

    /// A mutable pointer to the backing allocation.
    ///
    /// May only be called if `is_allocated` returns `true`.
    #[inline]
    pub(in crate::vec) unsafe fn allocation_mut(&mut self) -> *mut u8 {
        unsafe {
            debug_assert!(self.is_allocated());
            self.ptr.as_ptr().cast::<u8>().sub(Self::offset())
        }
    }

    /// A reference to the header.
    #[inline]
    pub(in crate::vec) fn header(&self) -> Option<&Header> {
        // Safety:
        // If the vector is allocated, there is always a valid header.
        self.is_allocated()
            .then(|| unsafe { &*self.allocation().cast::<Header>() })
    }

    /// The data pointer.
    ///
    /// Returns a pointer that is non-null, well-aligned, and valid for `len`
    /// reads of `T`.
    #[inline]
    pub(in crate::vec) fn data(&self) -> *const T {
        self.ptr.as_ptr()
    }

    /// The data pointer, mutably.
    ///
    /// Returns a pointer that is non-null, well-aligned, and valid for
    /// `capacity` writes of `T`.
    ///
    /// May only be called if the reference count is 1.
    #[inline]
    pub(in crate::vec) unsafe fn data_mut(&mut self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// The layout of a backing allocation for the given capacity.
    #[inline]
    #[track_caller]
    pub(in crate::vec) fn layout(capacity: usize) -> Layout {
        // Safety:
        // - `Self::size(capacity)` guarantees that it rounded up the alignment
        //   does not overflow `isize::MAX`.
        // - Since `Self::align()` is the header's alignment or T's alignment,
        //   it fulfills the requirements of a valid alignment.
        unsafe { Layout::from_size_align_unchecked(Self::size(capacity), Self::align()) }
    }

    /// The size of a backing allocation for the given capacity.
    ///
    /// Always `> 0`. When rounded up to the next multiple of `Self::align()` is
    /// guaranteed to be `<= isize::MAX`.
    #[inline]
    #[track_caller]
    pub(in crate::vec) fn size(capacity: usize) -> usize {
        mem::size_of::<T>()
            .checked_mul(capacity)
            .and_then(|size| Self::offset().checked_add(size))
            .filter(|&size| {
                // See `Layout::max_size_for_align` for details.
                size < isize::MAX as usize - Self::align()
            })
            .unwrap_or_else(|| capacity_overflow())
    }

    /// The alignment of the backing allocation.
    #[inline]
    const fn align() -> usize {
        max(mem::align_of::<Header>(), mem::align_of::<T>())
    }

    /// The offset of the data in the backing allocation.
    ///
    /// Always `> 0`. `self.ptr` points to the data and `self.ptr - offset` to
    /// the header.
    #[inline]
    pub(in crate::vec) const fn offset() -> usize {
        max(mem::size_of::<Header>(), Self::align())
    }

    /// The sentinel value of `self.ptr`, used to indicate an uninitialized,
    /// unallocated vector. It is dangling (does not point to valid memory) and
    /// has no provenance. As such, it must not be used to read/write/offset.
    /// However, it is well-aligned, so it can be used to create 0-length
    /// slices.
    ///
    /// All pointers to allocated vector elements will be distinct from this
    /// value, because allocated vector elements start `Self::offset()` bytes
    /// into a heap allocation and heap allocations cannot start at 0 (null).
    #[inline]
    pub(in crate::vec) const fn dangling() -> NonNull<T> {
        // The pointer we create has no provenance and may not be
        // read/write/offset. `without_provenance_mut` is the stable replacement
        // for the old `mem::transmute::<usize, *mut T>` (stable since 1.84).
        let ptr = core::ptr::without_provenance_mut::<T>(Self::offset());

        // Safety: `Self::offset()` is never 0.
        unsafe { NonNull::new_unchecked(ptr) }
    }

    /// The minimum non-zero capacity.
    #[inline]
    pub(in crate::vec) const fn min_cap() -> usize {
        // In the spirit of the `EcoVec`, we choose the cutoff size of T from
        // which 1 is the minimum capacity a bit lower than a standard `Vec`.
        if mem::size_of::<T>() == 1 {
            8
        } else if mem::size_of::<T>() <= 32 {
            4
        } else {
            1
        }
    }

    /// Whether no other vector is pointing to the same backing allocation.
    ///
    /// This takes a mutable reference because only callers with ownership or a
    /// mutable reference can ensure that the result stays relevant. Potential
    /// callers with a shared reference could read `true` while another shared
    /// reference is cloned on a different thread, bumping the ref-count. By
    /// restricting this callers with mutable access, we ensure that no
    /// uncontrolled cloning is happening in the time between the `is_unique`
    /// call and any subsequent mutation.
    #[inline]
    pub fn is_unique(&mut self) -> bool {
        // See Arc's is_unique() method.
        self.header().is_none_or(|header| header.refs.load(Acquire) == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eco_vec;
    use crate::sync::atomic::Ordering::Relaxed;

    // Regression test for ref_count_overflow rollback (#15).
    //
    // Clone::clone does `fetch_add(1)` before checking for overflow. On
    // overflow, `ref_count_overflow` must undo that increment by dropping a
    // temporary EcoVec (whose Drop does `fetch_sub(1)`). The genuine overflow
    // path requires `isize::MAX` clones (practically unreachable), so we
    // simulate the post-`fetch_add` state directly. The `fetch_add` below
    // keeps the refcount balanced: after the rollback the count returns to its
    // original value, so `v` and `_clone` drop cleanly without a leak (which
    // Miri would flag).
    //
    // Matches ecow 0.3.0: reference/ecow/src/vec.rs:1231.
    #[test]
    #[should_panic(expected = "reference count overflow")]
    fn ref_count_overflow_panics_and_rolls_back() {
        let v: crate::EcoVec<u32> = eco_vec![1, 2, 3];
        let _clone = v.clone(); // refs = 2

        // Simulate the `fetch_add(1)` that `Clone::clone` performs before the
        // overflow guard. This inflates refs to 3; the temp-drop inside
        // `ref_count_overflow` will `fetch_sub` it back to 2 before panicking.
        let header = v.header().expect("allocated");
        header.refs.fetch_add(1, Relaxed);

        ref_count_overflow(v.ptr, v.len, v.alloc);
    }
}
