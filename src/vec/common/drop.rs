//! Allocator-aware implementation of EcoVec
//!
//! This module contains the implementation of EcoVec when allocator features are enabled.
//! It's kept in a separate file to maintain clarity and avoid excessive conditional compilation.

use core::alloc::Layout;
use core::ptr::{self, NonNull};

use crate::allocator::AllocatorProvider;
use crate::sync::atomic::{self, Ordering::*};
use crate::vec::types::EcoVec;

impl<T, A> Drop for EcoVec<T, A>
where
    A: AllocatorProvider,
{
    fn drop(&mut self) {
        // Decide whether this is the last reference to the backing allocation.
        //
        // Fast path: an `Acquire` load of `1` proves we are the *unique* owner.
        // No other thread can hold, clone, or drop this allocation (turbocow has
        // no `Weak` references that could resurrect the count from 1), so we can
        // skip the expensive lock-prefixed read-modify-write entirely. The
        // `Acquire` load synchronizes-with the release sequence of every prior
        // decrement, so all previous owners' writes are visible before we drop
        // the elements — exactly the guarantee the `fence(Acquire)` below
        // provides on the shared path.
        //
        // Slow path: fall back to the standard `fetch_sub(Release)` and bail
        // unless we observe that we were the last owner, then `fence(Acquire)`.
        // See Arc's `Drop` for the canonical version of this reasoning.
        match self.header() {
            // No backing allocation (inline/empty/dangling) — nothing to do.
            None => return,
            Some(header) => {
                if header.refs.load(Acquire) == 1 {
                    // Unique owner: the Acquire load above is sufficient
                    // synchronization; skip the redundant atomic decrement.
                } else if header.refs.fetch_sub(1, Release) != 1 {
                    // Other owners remain; we are not the last reference.
                    return;
                } else {
                    // We were the last owner. See Arc's drop impl for details.
                    atomic::fence(Acquire);
                }
            }
        }

        // Safety:
        // The vector has a header, so `self.allocation()` points to an
        // allocation with the layout of current capacity.
        let layout = Self::layout(self.capacity());
        let alloc_ptr = unsafe { NonNull::new_unchecked(self.allocation_mut()) };
        let data_ptr = unsafe { self.data_mut() };

        // Ensures that the backing storage is deallocated even if one of the
        // element drops panics.
        struct DeallocGuard<'a, A: AllocatorProvider> {
            alloc: &'a A,
            ptr: NonNull<u8>,
            layout: Layout,
        }

        impl<A: AllocatorProvider> Drop for DeallocGuard<'_, A> {
            fn drop(&mut self) {
                unsafe { self.alloc.deallocate(self.ptr, self.layout) };
            }
        }

        let _dealloc = DeallocGuard { alloc: &self.alloc, ptr: alloc_ptr, layout };

        unsafe {
            // Safety:
            // No other vector references the backing allocation (just checked).
            // For more details, see `Self::as_slice()`.
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(data_ptr, self.len));
        }
    }
}
