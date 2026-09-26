use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop, MaybeUninit};

use crate::allocator::alloc::Layout;
use crate::utils::max;

// ── Tag constants ───────────────────────────────────────────────────────

/// Number of bits used for variant tags in the high bits of `tagged_len`.
const TAG_BITS: u32 = 2;

/// Bit pattern for the Inline variant: `0b00` in the top 2 bits.
pub(super) const TAG_INLINE: usize = 0b00 << (usize::BITS - TAG_BITS);

/// Bit pattern for the Referenced variant: `0b01` in the top 2 bits.
pub(super) const TAG_REFERENCED: usize = 0b01 << (usize::BITS - TAG_BITS);

/// Bit pattern for the Heap variant: `0b10` in the top 2 bits.
pub(super) const TAG_HEAP: usize = 0b10 << (usize::BITS - TAG_BITS);

/// Mask to extract the tag bits.
pub(super) const TAG_MASK: usize = 0b11 << (usize::BITS - TAG_BITS);

/// Mask to extract the length value (lower 62 bits on 64-bit).
pub(super) const LEN_MASK: usize = !TAG_MASK;

// ── Header ──────────────────────────────────────────────────────────────

/// Header stored at the start of a heap allocation, before the data.
///
/// Unlike EcoVec's header, there is no refcount — SmallVec owns its
/// allocation exclusively.
#[repr(C)]
pub(super) struct HeapHeader {
    pub(super) capacity: usize,
}

// ── Variant discriminant ────────────────────────────────────────────────

/// Which variant is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Variant {
    Inline,
    Referenced,
    Heap,
}

// ── Union data ──────────────────────────────────────────────────────────

/// The data payload, overlapping all three variants.
///
/// - **Inline**: `[MaybeUninit<T>; N]` — elements stored on the stack.
/// - **Referenced**: `*const T` — borrowed pointer, len in `tagged_len`.
/// - **Heap**: `*mut T` — owned pointer past a [`HeapHeader`], capacity
///   in the header.
#[repr(C)]
pub(super) union SmallVecData<T, const N: usize> {
    pub(super) inline: ManuallyDrop<[MaybeUninit<T>; N]>,
    pub(super) referenced: *const T,
    pub(super) heap: *mut T,
}

// ── SmallVec ────────────────────────────────────────────────────────────

/// An inline-spillable vector that stores up to `N` elements on the stack.
///
/// When the number of elements exceeds `N`, the vector spills to a
/// heap-allocated buffer with a `HeapHeader` storing the capacity.
/// It can also hold a zero-copy borrow of an existing `&'a [T]` via
/// the Referenced variant.
///
/// # Layout
/// ```text
/// struct SmallVec<'a, T, N> {
///     tagged_len: usize,          // 2 tag bits + 62-bit length
///     data: union {
///         inline: [MaybeUninit<T>; N],   // N * size_of::<T>()
///         referenced: *const T,          // 8 bytes
///         heap: *mut T,                  // 8 bytes
///     },
/// }
/// // Total: 8 + max(N * size_of::<T>(), 8)
/// ```
///
/// # Examples
/// ```
/// use turbocow::SmallVec;
///
/// let mut v = SmallVec::<i32, 4>::new();
/// v.push(1);
/// v.push(2);
/// assert_eq!(v.as_slice(), &[1, 2]);
/// assert!(v.is_inline());
///
/// // After exceeding inline capacity, spills to heap.
/// for i in 3..=10 {
///     v.push(i);
/// }
/// assert!(!v.is_inline());
/// assert_eq!(v.len(), 10);
/// ```
#[repr(C)]
pub struct SmallVec<'a, T, const N: usize = 8> {
    /// Top 2 bits = variant tag, lower 62 bits = element count.
    pub(super) tagged_len: usize,
    /// Union payload.
    pub(super) data: SmallVecData<T, N>,
    /// Covariant lifetime for the Referenced variant.
    /// Uses `&'a ()` instead of `&'a [T]` to avoid requiring `T: 'a`.
    pub(super) _marker: PhantomData<&'a ()>,
}

// ── SmallVec — low-level helpers ────────────────────────────────────────

impl<'a, T, const N: usize> SmallVec<'a, T, N> {
    /// Extract the variant tag.
    #[inline(always)]
    pub(super) fn tag(&self) -> usize {
        self.tagged_len & TAG_MASK
    }

    /// Extract the length (lower 62 bits).
    #[inline(always)]
    pub(super) fn raw_len(&self) -> usize {
        self.tagged_len & LEN_MASK
    }

    /// Which variant is currently active.
    #[inline(always)]
    pub(super) fn variant(&self) -> Variant {
        match self.tag() {
            TAG_INLINE => Variant::Inline,
            TAG_REFERENCED => Variant::Referenced,
            TAG_HEAP => Variant::Heap,
            // SAFETY: the tag bits of `tagged_len` are written only via
            // `encode`, which always ORs in one of TAG_INLINE / TAG_REFERENCED
            // / TAG_HEAP — the 0b11 pattern is never produced — so this arm is
            // genuinely unreachable. Eliding the bounds-style panic branch
            // keeps `variant()` (called on every push/pop/access) branch-lean.
            // The debug_assert preserves a panic on any future regression.
            _ => {
                debug_assert!(false, "invalid SmallVec tag bits");
                unsafe { core::hint::unreachable_unchecked() }
            }
        }
    }

    /// Encode tag + length into `tagged_len`.
    #[inline(always)]
    pub(super) fn encode(tag: usize, len: usize) -> usize {
        debug_assert!(len & TAG_MASK == 0, "length overflows into tag bits");
        tag | len
    }

    // ── Allocation helpers (mirrors EcoVec pattern) ─────────────────

    /// Alignment of the heap allocation.
    #[inline]
    pub(super) const fn heap_align() -> usize {
        max(mem::align_of::<HeapHeader>(), mem::align_of::<T>())
    }

    /// Offset from allocation start to the first element.
    #[inline]
    pub(super) const fn heap_offset() -> usize {
        max(mem::size_of::<HeapHeader>(), Self::heap_align())
    }

    /// Total allocation size for `capacity` elements.
    #[inline]
    pub(super) fn heap_size(capacity: usize) -> usize {
        mem::size_of::<T>()
            .checked_mul(capacity)
            .and_then(|size| Self::heap_offset().checked_add(size))
            .filter(|&size| size < isize::MAX as usize - Self::heap_align())
            .unwrap_or_else(|| capacity_overflow())
    }

    /// Layout for a heap allocation of `capacity` elements.
    #[inline]
    pub(super) fn heap_layout(capacity: usize) -> Layout {
        unsafe {
            Layout::from_size_align_unchecked(
                Self::heap_size(capacity),
                Self::heap_align(),
            )
        }
    }

    /// Given a data pointer (past the header), get a reference to the header.
    #[inline]
    pub(super) unsafe fn header_from_ptr(data_ptr: *mut T) -> &'a HeapHeader {
        unsafe { &*(data_ptr.cast::<u8>().sub(Self::heap_offset()) as *const HeapHeader) }
    }

    /// Given a data pointer, get the allocation start.
    #[inline]
    pub(super) unsafe fn alloc_ptr_from_data(data_ptr: *mut T) -> *mut u8 {
        unsafe { data_ptr.cast::<u8>().sub(Self::heap_offset()) }
    }
}

#[cold]
#[track_caller]
pub(super) fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}

// ── Send / Sync ─────────────────────────────────────────────────────────

unsafe impl<T: Send + Sync, const N: usize> Send for SmallVec<'_, T, N> {}
unsafe impl<T: Send + Sync, const N: usize> Sync for SmallVec<'_, T, N> {}

// ── Compile-time assertions ─────────────────────────────────────────────

const _: () = assert!(
    (0b11usize << (usize::BITS - 2)) & !0b11usize.wrapping_shl(usize::BITS - 2) == 0,
    "tag constants are consistent"
);
const _: () = assert!(TAG_INLINE == 0, "Inline tag must be zero for cheap default");
const _: () = assert!(
    mem::size_of::<HeapHeader>() == mem::size_of::<usize>(),
    "HeapHeader must be one word"
);
