pub(crate) mod internal {
    extern crate alloc as alloc_lib;

    pub use alloc_lib::alloc;
    use core::ptr::NonNull;

    use crate::allocator::{AllocatorProvider, common::AllocError};

    use alloc::Layout;

    /// Global allocator backed by the standard Rust global allocator.
    ///
    /// This is the default allocator when no allocator-API feature is enabled.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Global;

    unsafe impl AllocatorProvider for Global {
        #[inline]
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            match layout.size() {
                0 => Ok(NonNull::slice_from_raw_parts(NonNull::dangling(), 0)),
                _ => {
                    let ptr = unsafe { alloc::alloc(layout) };

                    if ptr.is_null() {
                        Err(AllocError)
                    } else {
                        Ok(NonNull::slice_from_raw_parts(
                            unsafe { NonNull::new_unchecked(ptr) },
                            layout.size(),
                        ))
                    }
                }
            }
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe {
                if layout.size() != 0 {
                    alloc::dealloc(ptr.as_ptr(), layout);
                }
            }
        }

        #[inline]
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            unsafe {
                debug_assert!(
                    new_layout.size() >= old_layout.size(),
                    "new layout size must be greater than or equal to old layout size"
                );

                if old_layout.size() == 0 {
                    self.allocate(new_layout)
                } else {
                    let new_ptr =
                        alloc::realloc(ptr.as_ptr(), old_layout, new_layout.size());

                    if new_ptr.is_null() {
                        Err(AllocError)
                    } else {
                        Ok(NonNull::slice_from_raw_parts(
                            NonNull::new_unchecked(new_ptr),
                            new_layout.size(),
                        ))
                    }
                }
            }
        }

        #[inline]
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            unsafe {
                debug_assert!(
                    new_layout.size() <= old_layout.size(),
                    "new layout size must be less than or equal to old layout size"
                );

                if new_layout.size() == 0 {
                    self.deallocate(ptr, old_layout);
                    Ok(NonNull::slice_from_raw_parts(NonNull::dangling(), 0))
                } else if old_layout.size() == new_layout.size() {
                    Ok(NonNull::slice_from_raw_parts(ptr, new_layout.size()))
                } else {
                    let new_ptr =
                        alloc::realloc(ptr.as_ptr(), old_layout, new_layout.size());

                    if new_ptr.is_null() {
                        Err(AllocError)
                    } else {
                        Ok(NonNull::slice_from_raw_parts(
                            NonNull::new_unchecked(new_ptr),
                            new_layout.size(),
                        ))
                    }
                }
            }
        }

        #[inline]
        fn handle_alloc_error(&self, layout: Layout) -> ! {
            alloc::handle_alloc_error(layout);
        }
    }
}
