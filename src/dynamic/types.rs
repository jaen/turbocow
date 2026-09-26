use core::mem::ManuallyDrop;

use crate::utils::variance::PhantomCovariantLifetime;

use crate::allocator::{AllocatorProvider, Global};
use crate::vec::types::EcoVec;

use super::common::limit::{LIMIT, TAG_OVERLAPS_LEN};

/// A byte vector that can hold up to 15 bytes inline and then spills to an
/// `EcoVec<u8, A>` with allocator support.
///
/// The const generic `N` controls the inline buffer size. For the default
/// `Global` allocator this is [`LIMIT`] (typically 15 bytes). Custom
/// allocators can use [`inline_limit`](super::common::limit::inline_limit)
/// to compute the correct value.
///
/// The lifetime `'a` tracks borrowed data in the [`Referenced`] variant.
/// Use `'static` (the common case, e.g. in [`EcoString`](crate::EcoString))
/// when the vector always owns its data.
pub(crate) struct DynamicVec<'a, const N: usize = LIMIT, A = Global>(
    pub(super) Repr<'a, N, A>,
)
where
    A: AllocatorProvider;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct InlineVec<const N: usize = LIMIT, A = Global>
where
    A: AllocatorProvider,
{
    /// Inline byte buffer.
    pub(super) buf: [u8; N],
    /// Invariant: After masking off the tag, never exceeds N.
    pub(super) tagged_len: u8,
    /// Allocator used when this buffer spills to the heap.
    pub(super) allocator: A,
}

/// A zero-copy borrowed byte slice stored inside the [`DynamicVec`] union.
///
/// The tag bits (`0b11xx_xxxx`) are embedded in the high byte of `len` so
/// that reading `tagged_len` through the [`InlineVec`] union member detects
/// this variant correctly.  On architectures where the `len` field does
/// **not** overlap with `tagged_len` (e.g. 32-bit), the constructor
/// additionally writes the tag byte explicitly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Referenced<'a, A = Global>
where
    A: AllocatorProvider,
{
    /// Raw pointer to the borrowed data.
    pub(super) ptr: *const u8,
    /// Length with tag bits embedded in the two highest bits.
    pub(super) len: usize,
    /// Ties the borrow lifetime to this struct (covariant in `'a`).
    pub(super) _lifetime: PhantomCovariantLifetime<'a>,
    /// Allocator carried along so spilling can use it.
    pub(super) allocator: A,
}

// Safety: Referenced is just a read-only view + Copy allocator.
unsafe impl<A: AllocatorProvider + Send> Send for Referenced<'_, A> {}
unsafe impl<A: AllocatorProvider + Sync> Sync for Referenced<'_, A> {}

/// The internal representation.
///
/// The discriminant is the `tagged_len` byte at offset `N` in `InlineVec`,
/// which must be reliably clear for a spilled `EcoVec`. The Inline/Referenced
/// variants set its top bit(s). There are two layouts (see
/// [`inline_limit`](super::common::limit::inline_limit)):
/// - On 64-bit little-endian, `tagged_len` overlaps the MSB of `EcoVec::len`
///   (always 0 since `len ≤ isize::MAX`), so spilled needs no explicit tag.
/// - On big-endian (and 32-bit), `N` is pushed *past* the `EcoVec`, so the
///   union is larger than `EcoVec` and the spilled/referenced constructors
///   write the tag byte explicitly (gated on `!TAG_OVERLAPS_LEN`).
#[repr(C)]
pub(super) union Repr<'a, const N: usize, A>
where
    A: AllocatorProvider,
{
    pub(super) referenced: ManuallyDrop<Referenced<'a, A>>,
    pub(super) inline: ManuallyDrop<InlineVec<N, A>>,
    pub(super) spilled: ManuallyDrop<EcoVec<u8, A>>,
}

// ---------------------------------------------------------------------------
// Compile-time layout assertions for the default (Global) allocator.
//
// These verify the invariants that the tag-byte scheme depends on. The union
// is sized by its largest member (`InlineVec`, which is `>= EcoVec` on every
// supported layout), so:
//
// 1. EcoVec and Referenced fit within the InlineVec-sized union.
// 2. `tagged_len` sits at offset LIMIT inside InlineVec (no padding before it).
// 3. The discriminant is reliable: either `tagged_len` overlaps the (clear)
//    MSB of a spilled EcoVec's `len`, OR it sits entirely past the EcoVec so
//    the spilled/referenced constructors can write it explicitly.
// 4. LIMIT fits in the 6-bit value field (<= 63).
// ---------------------------------------------------------------------------
const _: () = {
    use core::mem::{offset_of, size_of};

    let inline_sz = size_of::<InlineVec<LIMIT, Global>>();
    let ecovec_sz = size_of::<EcoVec<u8, Global>>();

    // (1) EcoVec and Referenced must fit within the union (sized by InlineVec).
    assert!(ecovec_sz <= inline_sz, "EcoVec must fit within the InlineVec-sized union");
    assert!(
        size_of::<Referenced<'static, Global>>() <= inline_sz,
        "Referenced must fit within the InlineVec-sized union"
    );

    // (2) tagged_len is at offset LIMIT (buf is [u8; LIMIT] with no padding).
    //     Real check; replaces the old `size_of::<[u8; LIMIT]>() == LIMIT`
    //     tautology (review bug #16).
    assert!(
        offset_of!(InlineVec<LIMIT, Global>, tagged_len) == LIMIT,
        "tagged_len must sit immediately after the inline buffer at offset LIMIT"
    );

    // (3) Discriminant reliability.
    if TAG_OVERLAPS_LEN {
        // tagged_len overlaps EcoVec::len's MSB, which is clear (len <= isize::MAX).
        let word = size_of::<usize>();
        assert!(
            LIMIT == 2 * word - 1,
            "overlapping tagged_len must land on len's most-significant byte"
        );
    } else {
        // tagged_len sits past the EcoVec / Referenced, so the explicit tag
        // write is not clobbered by the variant's own bytes.
        assert!(
            LIMIT >= ecovec_sz && LIMIT >= size_of::<Referenced<'static, Global>>(),
            "explicitly-written tagged_len must sit past the EcoVec and Referenced bytes"
        );
    }

    // (4) LIMIT fits in the 6-bit value field (max 63).
    assert!(LIMIT <= 63, "LIMIT must be at most 63 to fit in the 6-bit value field");

    // (5) Global allocator is ZST (inline path assumes this).
    assert!(size_of::<Global>() == 0, "Global allocator must be zero-sized");
};

/// This is never stored in memory, it's just an abstraction for safe access.
#[derive(Debug)]
pub(super) enum Variant<'a, 'v, const N: usize, A>
where
    A: AllocatorProvider,
{
    Referenced(&'v Referenced<'a, A>),
    Inline(&'v InlineVec<N, A>),
    Spilled(&'v EcoVec<u8, A>),
}

/// This is never stored in memory, it's just an abstraction for safe access.
#[derive(Debug)]
pub(super) enum VariantMut<'a, 'v, const N: usize, A>
where
    A: AllocatorProvider,
{
    Referenced(&'v mut Referenced<'a, A>),
    Inline(&'v mut InlineVec<N, A>),
    Spilled(&'v mut EcoVec<u8, A>),
}
