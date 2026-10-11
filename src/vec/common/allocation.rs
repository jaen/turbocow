use alloc::alloc::handle_alloc_error;
use core::alloc::Allocator;
use core::mem;
use core::ptr::{self, NonNull};

use crate::sync::atomic::AtomicUsize;

use super::capacity_overflow;
use crate::vec::types::{EcoVec, Header};

impl<T, A> EcoVec<T, A>
where
    A: Allocator,
{
    /// Grow the capacity to at least the `target` size.
    ///
    /// May only be called if:
    /// - the reference count is `1`, and
    /// - `target > capacity` (i.e., this methods grows, it doesn't shrink).
    ///
    /// Deliberately left inlinable (matching ecow): on the
    /// `with_capacity`/`from_slice`/`from_elem` paths `grow` runs on *every*
    /// call, so forcing it out-of-line with `#[cold] #[inline(never)]`
    /// pessimizes the always-taken first-allocation fast path. The realloc
    /// branch is large enough that the optimizer keeps it out of line on its
    /// own where it isn't hot (e.g. `push`).
    #[track_caller]
    pub(crate) unsafe fn grow(&mut self, mut target: usize) {
        debug_assert!(self.is_unique());
        debug_assert!(target > self.capacity());

        // Maintain the `capacity <= isize::MAX` invariant.
        if target > isize::MAX as usize {
            capacity_overflow();
        }

        // Directly go to maximum capacity for ZSTs.
        if mem::size_of::<T>() == 0 {
            target = isize::MAX as usize;
        }

        let layout = Self::layout(target);

        let result = if !self.is_allocated() {
            // Allocate new memory using the allocator
            self.alloc.allocate(layout)
        } else {
            // Grow existing allocation
            let old_layout = Self::layout(self.capacity());
            // Safety:
            // `is_allocated()` is true, so `allocation_mut` is valid, and the
            // resulting pointer is non-null (it points to the backing
            // allocation). The block is currently allocated by `self.alloc`,
            // or by an equivalent clone of it if the buffer was shared (only
            // such allocators make `EcoVec` `Clone`; see `clone_shared`),
            // with `old_layout` (the header records the capacity it was
            // requested with, not the possibly larger size the allocator
            // returned, so the layout fits), and `layout` has the same
            // alignment and a larger size, satisfying `Allocator::grow`'s
            // contract.
            let old_ptr = unsafe { self.allocation_mut() };
            unsafe {
                self.alloc.grow(NonNull::new_unchecked(old_ptr), old_layout, layout)
            }
        };
        let allocation = match result {
            Ok(ptr) => ptr.as_ptr().cast::<u8>(),
            // Like `Vec`, defer to the global alloc-error handler (which aborts
            // by default under `std`).
            Err(_) => handle_alloc_error(layout),
        };

        // Construct data pointer by offsetting.
        //
        // Safety:
        // The allocation is non-null and adding only increases the size. Can't
        // overflow because the `allocation` is a valid pointer to
        // `Self::size(target)` bytes and `Self::offset() < Self::size(target)`.
        self.ptr =
            unsafe { NonNull::new_unchecked(allocation.add(Self::offset()).cast()) };
        debug_assert_ne!(self.ptr, Self::dangling());

        // Safety:
        // The freshly allocated pointer is valid for a write of the header.
        unsafe {
            ptr::write(
                allocation.cast::<Header>(),
                Header { refs: AtomicUsize::new(1), capacity: target },
            );
        }
    }
}
