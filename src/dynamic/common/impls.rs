use core::alloc::Allocator;
use core::ptr;

use crate::dynamic::common::limit::LEN_VALUE_MASK_USIZE;
use crate::dynamic::types::DynamicVec;
use crate::vec::types::EcoVec;

impl<'a, const N: usize, A> Clone for DynamicVec<'a, N, A>
where
    A: Allocator + Clone,
    // The Spilled variant shares its `EcoVec`, which is only `Clone` for
    // allocators whose clones are interchangeable.
    EcoVec<u8, A>: Clone,
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
    A: Allocator,
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
    A: Allocator,
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

impl<const N: usize, A> Eq for DynamicVec<'_, N, A> where A: Allocator {}

impl<const N: usize, A> core::fmt::Debug for DynamicVec<'_, N, A>
where
    A: Allocator,
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
    use crate::Global;
    use crate::dynamic::common::limit::LIMIT;
    use crate::dynamic::types::InlineVec;
    use core::alloc::{AllocError, Layout};
    use core::ptr::NonNull;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// One drop counter per test (indexed by `CountingAlloc`'s first byte),
    /// so the tests stay independent when they run in parallel.
    static DROP_COUNTS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];

    /// Non-ZST allocator with a Drop impl that counts. Functionally
    /// identical to Global (delegates to it; `grow`/`shrink` use the trait's
    /// default allocate-copy-deallocate) except it occupies 8 bytes and runs
    /// Drop — so we can verify DynamicVec::drop drops the allocator field
    /// exactly once for every variant.
    #[derive(Clone)]
    struct CountingAlloc {
        // Forces non-ZST. Byte 0 selects the counter; the rest stays zero
        // (on big-endian the union's tag byte can overlap the last one).
        marker: [u8; 8],
    }

    impl CountingAlloc {
        fn new(counter: usize) -> Self {
            let mut marker = [0; 8];
            marker[0] = counter as u8;
            Self { marker }
        }

        fn drops(counter: usize) -> usize {
            DROP_COUNTS[counter].load(Ordering::SeqCst)
        }
    }

    unsafe impl Allocator for CountingAlloc {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            Global.allocate(layout)
        }
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { Global.deallocate(ptr, layout) }
        }
    }

    impl Drop for CountingAlloc {
        fn drop(&mut self) {
            DROP_COUNTS[usize::from(self.marker[0])].fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dynamicvec_inline_drops_allocator() {
        {
            // Construct an Inline DynamicVec with a non-ZST allocator.
            // `new_in` is the safe InlineVec constructor (sets the inline
            // tag bit correctly); `from_inline` wraps it in the union.
            let inline = InlineVec::<LIMIT, CountingAlloc>::new_in(CountingAlloc::new(0));
            let _dv = DynamicVec::from_inline(inline);
        }
        assert_eq!(
            CountingAlloc::drops(0),
            1,
            "non-ZST allocator must be dropped when DynamicVec (Inline) drops"
        );
    }

    #[test]
    fn dynamicvec_referenced_drops_allocator() {
        {
            // Construct a Referenced DynamicVec with a non-ZST allocator.
            // `from_referenced(slice, alloc)` handles tag encoding internally.
            // Turbofish on the constructor pins `N = LIMIT` (the Referenced
            // variant doesn't carry an `N`, so inference alone can't pin it).
            let data = b"hello";
            let _dv = DynamicVec::<LIMIT, CountingAlloc>::from_referenced(
                data,
                CountingAlloc::new(1),
            );
        }
        assert_eq!(
            CountingAlloc::drops(1),
            1,
            "non-ZST allocator must be dropped when DynamicVec (Referenced) drops"
        );
    }

    #[test]
    fn dynamicvec_spilled_drops_allocator_once_not_twice() {
        {
            // Construct a Spilled DynamicVec around an `EcoVec<u8, _>` that
            // owns a non-ZST allocator. The Spilled branch of
            // `DynamicVec::drop` must leave the allocator to `EcoVec`'s
            // field-drop glue instead of dropping it a second time.
            let bytes = [7u8; LIMIT + 1];
            let mut eco = EcoVec::with_capacity_in(bytes.len(), CountingAlloc::new(2));
            eco.extend_from_byte_slice(&bytes);
            let dv = DynamicVec::<LIMIT, CountingAlloc>::from_eco(eco);
            assert!(!dv.is_inline());
            assert_eq!(dv.as_slice(), &bytes);
        }
        assert_eq!(
            CountingAlloc::drops(2),
            1,
            "non-ZST allocator must be dropped exactly once when DynamicVec (Spilled) drops"
        );
    }
}
