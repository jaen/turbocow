use core::ptr;

use crate::allocator::AllocatorProvider;
use crate::vec::types::EcoVec;

impl<A> EcoVec<u8, A>
where
    A: AllocatorProvider + Clone,
{
    /// Copies from a byte slice.
    #[allow(unused)]
    #[inline]
    pub(crate) fn extend_from_byte_slice(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        self.reserve(bytes.len());

        // Safety: reserve guarantees unique ownership and sufficient capacity.
        unsafe { self.extend_from_byte_slice_unchecked(bytes) }
    }

    /// Ensure unique ownership, using memcpy instead of element-by-element
    /// clone.  This is the `u8`-specialised counterpart of
    /// [`EcoVec::make_unique`](super::cloneable).
    #[inline]
    pub(crate) fn make_unique_bytes(&mut self) {
        if !self.is_unique() {
            let mut vec = Self::with_capacity_in(self.len, self.alloc.clone());
            // Safety: with_capacity_in guarantees capacity ≥ self.len and
            // the fresh vec is unique (refcount = 1).
            unsafe { vec.extend_from_byte_slice_unchecked(self.as_slice()) };
            *self = vec;
        }
    }

    /// Copies from a byte slice without reserving capacity.
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `self.is_unique()` (reference count is 1)
    /// - `self.len + bytes.len() <= self.capacity()`
    #[inline(always)]
    pub(crate) unsafe fn extend_from_byte_slice_unchecked(&mut self, bytes: &[u8]) {
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.data_mut().add(self.len),
                bytes.len(),
            );
        }
        self.len += bytes.len();
    }
}
