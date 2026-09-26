use core::ptr::NonNull;

use crate::allocator::alloc::{Layout, LayoutError};

#[cfg(feature = "allocator-api")]
use crate::allocator::alloc;

/// Error type for allocation operations.
///
/// This is our internal error type that abstracts over different allocator
/// API error types (allocator-api2 v0.2, v0.3, nightly std).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocError;

impl From<LayoutError> for AllocError {
    fn from(_: LayoutError) -> Self {
        AllocError
    }
}

#[cfg(feature = "allocator-api")]
impl From<alloc::AllocError> for AllocError {
    fn from(_: alloc::AllocError) -> Self {
        AllocError
    }
}

/// Minimal internal allocator trait for turbocow.
///
/// Provides a small interface for memory allocation that can be implemented by
/// different allocator backends.
///
/// # Safety
///
/// Implementations must guarantee that allocated memory blocks are valid
/// and properly aligned per the requested [`Layout`]. Deallocating memory
/// must only be done with pointers and layouts that match a prior allocation.
pub unsafe trait AllocatorProvider {
    /// Allocate memory with the given layout.
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;

    /// Deallocate memory at the given pointer with the specified layout.
    ///
    /// # Safety
    ///
    /// - `ptr` must be currently allocated via this allocator
    /// - `layout` must be the same layout used to allocate the block
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);

    /// Attempt to grow an allocation in-place.
    ///
    /// Returns a new pointer (which may be the same as the old pointer)
    /// and the actual size of the allocation.
    ///
    /// # Safety
    ///
    /// - `ptr` must be currently allocated via this allocator
    /// - `old_layout` must be the layout used to allocate the block
    /// - `new_layout.size()` must be greater than or equal to `old_layout.size()`
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            debug_assert!(
                new_layout.size() >= old_layout.size(),
                "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
            );

            // Default implementation: allocate new, copy, deallocate old
            let new_ptr = self.allocate(new_layout)?;

            // SAFETY: because `new_layout.size()` must be greater than or equal to
            // `old_layout.size()`, both the old and new memory allocation are valid for reads and
            // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
            // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
            // safe. The safety contract for `dealloc` must be upheld by the caller.
            core::ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_ptr.as_ptr().cast(),
                old_layout.size(),
            );

            // Deallocate old memory
            self.deallocate(ptr, old_layout);

            Ok(new_ptr)
        }
    }

    /// Attempt to shrink an allocation in-place.
    ///
    /// # Safety
    ///
    /// - `ptr` must be currently allocated via this allocator
    /// - `old_layout` must be the layout used to allocate the block
    /// - `new_layout.size()` must be less than or equal to `old_layout.size()`
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        _old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // Safe body; `unsafe fn` is required by the trait contract.
        // Default implementation: for shrinking, we can often reuse the same allocation
        // Most allocators don't actually shrink in place, so we just return the same pointer
        Ok(NonNull::slice_from_raw_parts(ptr, new_layout.size()))
    }

    /// Creates a "by reference" adapter for this instance of `Allocator`.
    ///
    /// The returned adapter also implements `Allocator` and will simply borrow this.
    #[inline(always)]
    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }

    /// Handle allocation error.
    fn handle_alloc_error(&self, layout: Layout) -> ! {
        panic!("allocation error: {:?}", layout);
    }
}

#[cfg(feature = "allocator-api")]
unsafe impl<A> AllocatorProvider for A
where
    A: alloc::Allocator,
{
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (*self).allocate(layout).map_err(Into::into)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (*self).deallocate(ptr, layout) }
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (*self).grow(ptr, old_layout, new_layout).map_err(Into::into) }
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (*self).shrink(ptr, old_layout, new_layout).map_err(Into::into) }
    }
}
