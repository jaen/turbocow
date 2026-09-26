use core::mem;

use crate::allocator::Global;

/// Compute the per-allocator inline limit from the allocator's size.
///
/// The maximum amount of inline storage. Typically, this is 15 bytes.
///
/// This computes `size_of::<EcoVec<u8, A>>()` manually from the allocator
/// size to avoid circular const evaluation when used in `generic_const_exprs`
/// bounds. `EcoVec<u8, A>` is `repr(C)` with fields:
///   `ptr: NonNull<u8>` + `len: usize` + `alloc: A` + `phantom: PhantomData<u8>`
///
/// The `tagged_len` byte (at offset `N` in `InlineVec`) doubles as the union
/// discriminant. It must be a byte that is reliably *clear* for a real (spilled)
/// `EcoVec`, so the Inline/Referenced variants can mark themselves by setting
/// its top bit(s). Two cases (mirroring `ecow`):
///
/// - **64-bit little-endian:** `tagged_len` (offset `N = ecovec_size − 1` = 15)
///   naturally overlaps the most-significant byte of `EcoVec::len`, which is
///   always clear because `len ≤ isize::MAX`. No explicit tag write is needed
///   (see [`TAG_OVERLAPS_LEN`]).
/// - **big-endian:** `len`'s low (non-clearable) bytes would land on
///   `tagged_len`, so we push it *past* the `EcoVec` by adding a word; the
///   spilled/referenced constructors then write the tag byte explicitly. This
///   grows the union beyond `EcoVec`, but the inline buffer keeps its full
///   capacity — it is **not** reduced. (32-bit LE already has `N` on `len`'s
///   MSB so it overlaps like 64-bit LE.)
/// - For an unusually large `EcoVec` (e.g. 128-bit pointers) the limit grows
///   with `ecovec_size` accordingly.
///
/// This matches `ecow`'s `dynamic::LIMIT`; turbocow's only addition is that the
/// union also carries an allocator `A` (a ZST for the default `Global`).
pub(crate) const fn inline_limit<A>() -> usize {
    // EcoVec<u8, A> is repr(C): NonNull<u8> (ptr-sized) + usize + A + PhantomData (0)
    let ecovec_size =
        mem::size_of::<usize>() + mem::size_of::<usize>() + mem::size_of::<A>();
    let mut limit = ecovec_size - 1;
    if cfg!(target_endian = "big") {
        // Move tagged_len past the EcoVec so it never overlaps `len`'s lower
        // bytes; the constructors write the tag explicitly in this case.
        limit += mem::size_of::<usize>();
    }
    limit
}

/// Backward-compatible LIMIT constant for the default (Global) allocator.
pub(crate) const LIMIT: usize = inline_limit::<Global>();

// ---------------------------------------------------------------------------
// Tag encoding (2-bit scheme in the byte at offset N of the union)
//
// The `tagged_len` byte (offset N in InlineVec) doubles as a variant
// discriminant.  We use the two highest bits:
//
//   0b10xx_xxxx  →  Inline   (bit 7 set, bit 6 clear)
//   0b11xx_xxxx  →  Referenced (both bits set)
//   0b0xxx_xxxx  →  Spilled  (bit 7 clear — guaranteed because EcoVec's
//                              `len` never exceeds `isize::MAX`)
//
// The remaining 6 bits hold the inline / referenced length (max 63).
// ---------------------------------------------------------------------------

/// Bit 7 — set for both Inline and Referenced variants.
pub(crate) const LEN_INLINE_TAG: u8 = 0b1000_0000;

/// Bit 6 — set *only* for the Referenced variant (together with bit 7).
pub(crate) const LEN_REFERENCED_TAG: u8 = 0b0100_0000;

/// Both tag bits combined.
pub(crate) const LEN_TAGS: u8 = LEN_INLINE_TAG | LEN_REFERENCED_TAG;

/// Mask that extracts the length value from a tagged byte.
pub(crate) const LEN_VALUE_MASK: u8 = !LEN_TAGS;

/// `usize`-level mask for embedding the *referenced* tag in
/// `Referenced::len`.  On little-endian 64-bit the high byte of `len`
/// overlaps with `tagged_len`, so the tag is naturally visible.  On other
/// layouts we write the tag byte explicitly as well.
pub(crate) const LEN_TAGS_USIZE: usize =
    (LEN_TAGS as usize) << (usize::BITS as usize - 8);

/// Mask that extracts the actual length from a `Referenced::len` field.
pub(crate) const LEN_VALUE_MASK_USIZE: usize = !LEN_TAGS_USIZE;

/// Whether the high byte of `Referenced::len` naturally overlaps with the
/// `tagged_len` byte at offset `LIMIT` in `InlineVec`.
///
/// On 64-bit little-endian with a ZST allocator this is true:
///   - `Referenced::len` starts at offset `size_of::<usize>()` (after `ptr`)
///   - Its highest byte (LE) is at offset `2 * size_of::<usize>() - 1` = 15 = LIMIT
///
/// When true, writing the tag bits into `Referenced::len` is sufficient and
/// the explicit tag-byte write in `from_referenced` can be skipped.
pub(crate) const TAG_OVERLAPS_LEN: bool = {
    let ptr_size = mem::size_of::<usize>();
    // High byte of `len` field in Referenced (repr(C): ptr then len)
    let len_high_byte_offset = if cfg!(target_endian = "little") {
        // LE: high byte is at the end of the word
        ptr_size + ptr_size - 1
    } else {
        // BE: high byte is at the start of the word
        ptr_size
    };
    len_high_byte_offset == inline_limit::<Global>()
};
