use core::ptr;

use crate::allocator::AllocatorProvider;
use crate::dynamic::common::limit::LEN_VALUE_MASK_USIZE;
use crate::dynamic::types::DynamicVec;

impl<'a, const N: usize, A> Clone for DynamicVec<'a, N, A>
where
    A: AllocatorProvider + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        if self.is_referenced() {
            // Clone Referenced separately to preserve pointer provenance.
            // Copying the raw pointer through the InlineVec byte-array
            // view strips Miri provenance, causing UB when dereferenced.
            let r = unsafe { &*self.0.referenced };
            let len = r.len & LEN_VALUE_MASK_USIZE;
            // Safety: r.ptr was created from a &'a [u8] in from_referenced,
            // so reconstituting the slice with the original lifetime is sound.
            let slice = unsafe { core::slice::from_raw_parts(r.ptr, len) };
            Self::from_referenced(slice, r.allocator.clone())
        } else if self.is_inline() {
            Self::from_inline(unsafe { (*self.0.inline).clone() })
        } else {
            Self::from_eco(unsafe { (*self.0.spilled).clone() })
        }
    }
}

impl<const N: usize, A> Drop for DynamicVec<'_, N, A>
where
    A: AllocatorProvider,
{
    #[inline]
    fn drop(&mut self) {
        // `is_inline()` returns true for both Inline and Referenced variants
        // (both have bit 7 set in tagged_len), so `!is_inline()` identifies
        // exactly the Spilled variant. This avoids the full 3-way variant
        // dispatch that `variant_mut()` performs (saving one branch).
        if !self.is_inline() {
            // Spilled: EcoVec's drop runs the allocator's Drop via
            // field-drop glue (EcoVec owns its `alloc: A` field directly).
            unsafe {
                ptr::drop_in_place(&mut *self.0.spilled);
            }
        } else if core::mem::needs_drop::<A>() {
            // Inline or Referenced: the allocator field is inside
            // ManuallyDrop inside the Repr union, so it is never auto-dropped.
            // For ZST A (e.g. Global), needs_drop is false at compile time
            // and this branch is eliminated — zero-cost defensive fix.
            //
            // Safety: is_inline() is true, so the union is either Inline or
            // Referenced. is_referenced() disambiguates. We only touch the
            // allocator field, which has the same type in both variants.
            // buf/ptr/len/tagged_len are all trivially droppable (u8/usize).
            unsafe {
                let allocator_ptr = if self.is_referenced() {
                    ptr::addr_of_mut!((*self.0.referenced).allocator)
                } else {
                    ptr::addr_of_mut!((*self.0.inline).allocator)
                };
                ptr::drop_in_place(allocator_ptr);
            }
        }
    }
}

impl<const N: usize, A> PartialEq for DynamicVec<'_, N, A>
where
    A: AllocatorProvider,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // is_inline() is true for both Inline and Referenced (bit 7 set).
        // !is_inline() identifies exactly the Spilled variant.
        let si = self.is_inline();
        let oi = other.is_inline();
        if !si & !oi {
            // Both Spilled: EcoVec::PartialEq has a pointer-identity
            // fast-path for clones that share the same backing allocation.
            unsafe { *self.0.spilled == *other.0.spilled }
        } else {
            // At least one operand is Inline or Referenced: compare by
            // content. A raw-byte comparison is unsound here because
            // (1) inline tail bytes `[len..N)` are not guaranteed zero after
            //     shrinking ops (`truncate`, `clear`), and
            // (2) Inline and Referenced encode the same content with different
            //     bytes (an inline buffer vs. a pointer + length),
            // so equal-content values can have differing raw bytes.
            self.as_slice() == other.as_slice()
        }
    }
}

impl<const N: usize, A> Eq for DynamicVec<'_, N, A> where A: AllocatorProvider {}

impl<const N: usize, A> core::fmt::Debug for DynamicVec<'_, N, A>
where
    A: AllocatorProvider,
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("DynamicVec")
            .field("referenced", &self.is_referenced())
            .field("inline", &self.is_inline())
            .field("len", &self.len())
            .field("data", &self.as_slice())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::alloc::Layout;
    use crate::allocator::{AllocError, AllocatorProvider, Global};
    use crate::dynamic::common::limit::LIMIT;
    use crate::dynamic::types::InlineVec;
    use core::ptr::NonNull;
    use core::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Non-ZST allocator with a Drop impl that counts. Functionally
    /// identical to Global (delegates all AllocatorProvider methods) except
    /// it occupies 8 bytes and runs Drop — so we can verify DynamicVec::drop
    /// actually drops the allocator field for Inline/Referenced variants.
    #[derive(Clone)]
    struct CountingAlloc {
        _marker: [u8; 8], // forces non-ZST
    }

    unsafe impl AllocatorProvider for CountingAlloc {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            Global.allocate(layout)
        }
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { Global.deallocate(ptr, layout) }
        }
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old: Layout,
            new: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            unsafe { Global.grow(ptr, old, new) }
        }
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old: Layout,
            new: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            unsafe { Global.shrink(ptr, old, new) }
        }
    }

    impl Drop for CountingAlloc {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dynamicvec_inline_drops_allocator() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            // Construct an Inline DynamicVec with a non-ZST allocator.
            // `new_in` is the safe InlineVec constructor (sets the inline
            // tag bit correctly); `from_inline` wraps it in the union.
            let inline = InlineVec::<LIMIT, CountingAlloc>::new_in(CountingAlloc {
                _marker: [0u8; 8],
            });
            let _dv = DynamicVec::from_inline(inline);
        }
        assert_eq!(
            DROP_COUNT.load(Ordering::SeqCst),
            1,
            "non-ZST allocator must be dropped when DynamicVec (Inline) drops"
        );
    }

    #[test]
    fn dynamicvec_referenced_drops_allocator() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            // Construct a Referenced DynamicVec with a non-ZST allocator.
            // `from_referenced(slice, alloc)` handles tag encoding internally.
            // Turbofish on the constructor pins `N = LIMIT` (the Referenced
            // variant doesn't carry an `N`, so inference alone can't pin it).
            let data = b"hello";
            let _dv = DynamicVec::<LIMIT, CountingAlloc>::from_referenced(
                data,
                CountingAlloc { _marker: [0u8; 8] },
            );
        }
        assert_eq!(
            DROP_COUNT.load(Ordering::SeqCst),
            1,
            "non-ZST allocator must be dropped when DynamicVec (Referenced) drops"
        );
    }

    // Note: a `dynamicvec_spilled_drops_allocator_once_not_twice` test would
    // require constructing an `EcoVec<u8, CountingAlloc>` via the internal
    // API, which is non-trivial without the allocator-api feature gate. The
    // two tests above cover the new code path; the Spilled branch already
    // delegates to `EcoVec`'s field-drop glue, so it is exercised by the
    // existing `EcoString`/`EcoVec` test suites that use the Spilled path.
}
