use crate::allocator::AllocatorProvider;
use crate::dynamic::types::{DynamicVec, Variant, VariantMut};

use super::limit::{LEN_INLINE_TAG, LEN_TAGS};

impl<'a, const N: usize, A> DynamicVec<'a, N, A>
where
    A: AllocatorProvider,
{
    // If this returns true, guarantees that `self.0.inline` is initialized.
    // (Also true for the Referenced variant — check `is_referenced` first.)
    #[inline]
    pub(super) fn is_inline(&self) -> bool {
        // Safety:
        // We always initialize tagged_len, even for the `EcoVec` variant. For
        // the inline variant the highest-order bit is always `1`. For the
        // spilled variant, it is initialized with `0` and cannot deviate from
        // that because the EcoVec's `len` field is bounded by `isize::MAX`. (At
        // least on 64-bit little endian; on 32-bit or big-endian the EcoVec
        // and tagged_len fields don't even overlap, meaning tagged_len stays at
        // its initial value.)
        //
        // Note: this returns `true` for Referenced too (both bits set).
        unsafe { self.0.inline.tagged_len & LEN_INLINE_TAG != 0 }
    }

    /// Returns `true` only for the Inline variant (`0b10xx_xxxx`).
    /// Unlike `is_inline()`, this excludes the Referenced variant.
    #[inline]
    pub(crate) fn is_pure_inline(&self) -> bool {
        unsafe { (self.0.inline.tagged_len & LEN_TAGS) == LEN_INLINE_TAG }
    }

    /// Returns `true` when both tag bits are set (`0b11xx_xxxx`).
    #[inline]
    pub(super) fn is_referenced(&self) -> bool {
        unsafe { (self.0.inline.tagged_len & LEN_TAGS) == LEN_TAGS }
    }

    #[inline]
    pub(super) fn variant(&self) -> Variant<'a, '_, N, A> {
        unsafe {
            // Safety:
            // We access the respective variant only if the check passes.
            // Check !is_inline() first so the Spilled hot path needs one branch.
            if !self.is_inline() {
                Variant::Spilled(&self.0.spilled)
            } else if self.is_referenced() {
                Variant::Referenced(&self.0.referenced)
            } else {
                Variant::Inline(&self.0.inline)
            }
        }
    }

    #[inline]
    pub(super) fn variant_mut(&mut self) -> VariantMut<'a, '_, N, A> {
        unsafe {
            // Safety:
            // We access the respective variant only if the check passes.
            if !self.is_inline() {
                VariantMut::Spilled(&mut self.0.spilled)
            } else if self.is_referenced() {
                VariantMut::Referenced(&mut self.0.referenced)
            } else {
                VariantMut::Inline(&mut self.0.inline)
            }
        }
    }
}
