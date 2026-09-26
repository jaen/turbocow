use core::mem::{ManuallyDrop, MaybeUninit};
use core::ptr::addr_of_mut;

use crate::allocator::{AllocatorProvider, Global};
use crate::dynamic::common::limit::LEN_VALUE_MASK_USIZE;
use crate::dynamic::types::{DynamicVec, InlineVec, Referenced, Repr};
use crate::utils::variance::PhantomCovariantLifetime;
use crate::vec::types::EcoVec;

use super::limit::{LEN_TAGS, LEN_TAGS_USIZE, LIMIT, TAG_OVERLAPS_LEN};

impl DynamicVec<'static, LIMIT, Global> {
    #[inline]
    pub const fn new() -> Self {
        Self::from_inline(InlineVec::new())
    }

    /// Owned copy of `bytes` for the default `Global` allocator (the
    /// `EcoString::from(&str)` hot path).
    ///
    /// Builds the inline value **by value** — `InlineVec::from_slice` →
    /// [`from_inline`](Self::from_inline) — exactly like
    /// [`EcoString::inline`](crate::EcoString::inline). This deliberately does
    /// NOT route through [`from_slice_in_owned`](Self::from_slice_in_owned)'s
    /// `MaybeUninit` + `emplace_*` machinery: on this hot path LLVM can't
    /// promote the emplaced stack slot back into registers, so emplacing costs
    /// an extra inline-area zero plus a granular stack→return copy-out (verified
    /// by asm-diffing against `ecow`, which builds by value). The by-value form
    /// stays in registers. When the data doesn't fit inline we fall back to the
    /// owned heap path (which legitimately emplaces — see `from_slice_in_owned`).
    #[inline]
    pub fn from_slice(bytes: &[u8]) -> Self {
        match InlineVec::<LIMIT, Global>::from_slice(bytes) {
            // Fits inline: construct the union by value, no MaybeUninit slot.
            Ok(inline) => Self::from_inline(inline),
            // Spills past the inline limit: reuse the owned heap path.
            Err(()) => Self::from_slice_in_owned(bytes, &Global, 0),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= LIMIT {
            Self::new()
        } else {
            Self::from_eco(EcoVec::<u8, Global>::with_capacity(capacity))
        }
    }
}

impl<'a, const N: usize, A> DynamicVec<'a, N, A>
where
    A: AllocatorProvider,
{
    #[inline]
    #[allow(dead_code)]
    pub const fn new_in(alloc: A) -> Self {
        Self::from_inline(InlineVec::new_in(alloc))
    }

    #[inline]
    pub(crate) const fn from_inline(inline: InlineVec<N, A>) -> Self {
        DynamicVec(Repr { inline: ManuallyDrop::new(inline) })
    }

    #[inline]
    pub fn from_eco(vec: EcoVec<u8, A>) -> Self {
        let mut place = MaybeUninit::<Self>::uninit();
        Self::emplace_from_eco(&mut place, vec);
        unsafe { place.assume_init() }
    }

    /// Emplace an EcoVec into a `MaybeUninit`, producing a Spilled variant.
    ///
    /// On 64-bit LE the high byte of `EcoVec::len` (always 0 since
    /// `len ≤ isize::MAX`) sits at the `tagged_len` position, so no
    /// explicit tag clear is needed.  On other layouts the tag byte
    /// is zeroed before the write.
    #[inline(always)]
    pub(crate) fn emplace_from_eco(place: &mut MaybeUninit<Self>, vec: EcoVec<u8, A>) {
        unsafe {
            if !TAG_OVERLAPS_LEN {
                let tag_ptr =
                    (addr_of_mut!((*place.as_mut_ptr()).0.inline) as *mut u8).add(N);
                tag_ptr.write(0);
            }
            let spilled_ptr = addr_of_mut!((*place.as_mut_ptr()).0.spilled);
            spilled_ptr.write(ManuallyDrop::new(vec));
        }
    }

    /// Emplace a spilled EcoVec built from `bytes` into a `MaybeUninit`.
    /// Counterpart to [`InlineVec::emplace_from_slice_in`] for the heap path.
    #[inline(always)]
    fn emplace_spilled_from_slice(
        place: &mut MaybeUninit<Self>,
        bytes: &[u8],
        alloc: A,
        size_hint: usize,
    ) where
        A: Clone,
    {
        let mut vec = EcoVec::with_capacity_in(bytes.len() + size_hint, alloc);
        // Safety: with_capacity_in just allocated bytes.len() + size_hint
        // capacity and the fresh vec is unique (refcount = 1).
        unsafe { vec.extend_from_byte_slice_unchecked(bytes) };
        Self::emplace_from_eco(place, vec);
    }

    /// Create a `DynamicVec` that **borrows** `bytes` without copying
    /// (the [`Referenced`] variant).
    ///
    /// The tag bits are embedded in `Referenced::len` **and** written
    /// explicitly to the `tagged_len` position so the variant is detected
    /// correctly on every architecture (the two overlap only on 64-bit LE).
    #[inline]
    #[allow(dead_code)]
    pub(super) fn from_referenced(bytes: &'a [u8], alloc: A) -> Self {
        let ptr = bytes.as_ptr();
        let actual_len = bytes.len();
        let tagged_len_usize = actual_len | LEN_TAGS_USIZE;

        let mut place = MaybeUninit::<Self>::uninit();
        unsafe {
            let repr_ptr = place.as_mut_ptr();
            // Write the Referenced struct into the union.
            let ref_ptr = addr_of_mut!((*repr_ptr).0.referenced);
            ref_ptr.write(ManuallyDrop::new(Referenced {
                ptr,
                len: tagged_len_usize,
                _lifetime: PhantomCovariantLifetime::new(),
                allocator: alloc,
            }));
            // On 64-bit LE, the high byte of Referenced::len naturally
            // overlaps with tagged_len — the tag bits in len are already
            // visible.  On other layouts write the tag byte explicitly.
            if !TAG_OVERLAPS_LEN {
                let inline_base = addr_of_mut!((*repr_ptr).0.inline) as *mut u8;
                let tag_ptr = inline_base.add(N);
                tag_ptr.write(LEN_TAGS);
            }
            place.assume_init()
        }
    }

    /// Zero-copy borrow: stores a pointer to `bytes` in the [`Referenced`]
    /// variant.  The returned `DynamicVec` borrows `bytes` for `'a`.
    ///
    /// This is the primary constructor used by `EcoString<'a>` (future) and
    /// any code that wants to avoid copying short-lived data.
    #[inline]
    pub fn from_slice_in(bytes: &'a [u8], alloc: A) -> Self {
        Self::from_referenced(bytes, alloc)
    }

    /// Owned copy: always copies `bytes` into inline storage or a heap
    /// allocation.  Uses [`emplace_from_slice_in`](InlineVec::emplace_from_slice_in)
    /// for the inline fast-path.
    ///
    /// This is the **allocator-generic, `size_hint`-aware** constructor, and it
    /// builds into a `MaybeUninit<Self>` via the `emplace_*` helpers on purpose:
    /// emplacing lets it write the active union variant *and* the tag byte
    /// directly into the slot, which is what's needed for (a) arbitrary
    /// (non-ZST) allocators and (b) the layouts where the tag byte does **not**
    /// overlap `EcoVec::len` (32-bit / big-endian, see `emplace_from_eco`).
    /// By-value union construction can't express that in-place tag write.
    ///
    /// The trade-off is that the emplaced stack slot doesn't promote back into
    /// registers, so the common `Global` + `size_hint == 0` construction routes
    /// through [`from_slice`](Self::from_slice) instead — it builds by value and
    /// skips this round-trip on the hot `EcoString::from(&str)` path.
    ///
    /// `size_hint` is the number of *extra* bytes the caller expects to
    /// append shortly after construction.  It influences both the inline
    /// fit check and the heap pre-allocation size, avoiding an immediate
    /// reallocation after materialisation.
    ///
    /// Returns `DynamicVec<'static>` because the data is fully owned.
    ///
    /// Takes `&A` so callers can borrow the allocator (e.g. from a
    /// Referenced variant) without cloning upfront.  The inline emplace
    /// clones only on success; the spill path clones once for the heap
    /// allocation.
    #[inline]
    pub fn from_slice_in_owned(
        bytes: &[u8],
        alloc: &A,
        size_hint: usize,
    ) -> DynamicVec<'static, N, A>
    where
        A: Clone,
    {
        let mut place = MaybeUninit::<DynamicVec<'static, N, A>>::uninit();

        // Non-ZST allocators break the InlineVec union layout assumptions,
        // so skip the inline path entirely and always spill to heap.
        // Each emplace function writes a complete valid variant (including
        // the tag byte), so no pre-zeroing is needed.
        //
        // emplace_from_slice_in borrows &A and clones only on success.
        let inlined = core::mem::size_of::<A>() == 0
            && InlineVec::emplace_from_slice_in(&mut place, bytes, alloc, size_hint);

        if !inlined {
            DynamicVec::<'static, N, A>::emplace_spilled_from_slice(
                &mut place,
                bytes,
                alloc.clone(),
                size_hint,
            );
        }
        unsafe { place.assume_init() }
    }

    /// Convert this `DynamicVec` into a fully owned `DynamicVec<'static>`.
    ///
    /// If the current variant is [`Referenced`], the data is copied into
    /// inline or heap storage.  Inline and Spilled variants are already
    /// owned, so the lifetime is simply widened via transmute.
    #[inline]
    // Deliberately consumes `self` to widen the lifetime to 'static; the
    // `to_*`-takes-`&self` convention doesn't apply to this conversion.
    #[allow(dead_code, clippy::wrong_self_convention)]
    pub fn to_owned(self) -> DynamicVec<'static, N, A>
    where
        A: Clone,
    {
        if self.is_referenced() {
            // Read directly from the union — we already know the variant.
            // Avoids the 3-way dispatch that as_slice() + get_allocator() do.
            let r = unsafe { &*self.0.referenced };
            let len = r.len & LEN_VALUE_MASK_USIZE;
            let slice = unsafe { core::slice::from_raw_parts(r.ptr, len) };
            let owned = Self::from_slice_in_owned(slice, &r.allocator, 0);
            // Prevent drop of the old Referenced (it doesn't own heap memory,
            // but we must not run the Spilled branch of Drop by accident).
            core::mem::forget(self);
            return owned;
        }
        // Inline and Spilled are already owned — transmute is safe because
        // the only difference between DynamicVec<'a> and DynamicVec<'static>
        // is a PhantomData lifetime in the Referenced union arm, which is not
        // active here.
        unsafe { core::mem::transmute(self) }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        if capacity <= N {
            Self::new_in(alloc)
        } else {
            Self::from_eco(EcoVec::with_capacity_in(capacity, alloc))
        }
    }
}
