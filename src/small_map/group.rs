//! SIMD-accelerated h2 byte matching for inline map lookups.
//!
//! The h2 value is the top 7 bits of a hash, used as a quick pre-filter
//! before full key comparison. [`match_h2`] scans an h2 sidecar array in
//! 16-byte chunks and yields the matching positions.
//!
//! Platform support:
//! - **x86_64**: SSE2 `_mm_cmpeq_epi8` + `_mm_movemask_epi8` per in-bounds chunk
//! - **All platforms**: scalar fallback (auto-vectorised by LLVM) for the
//!   final partial chunk and for non-x86_64 targets
//!
//! There is no upper bound on the inline capacity `N`: chunks are walked
//! lazily, so a 16-bit lane mask per chunk is sufficient regardless of `N`.

use core::hash::{BuildHasher, Hash};

/// Extract h2 (top 7 bits) from a 64-bit hash.
#[inline]
pub(super) fn h2(hash: u64) -> u8 {
    (hash >> 57) as u8
}

/// Sentinel for empty slots (never a valid h2 since h2 ∈ 0..128).
pub(super) const H2_EMPTY: u8 = 0xFF;

/// Compute the full u64 hash of a value.
#[inline]
pub(super) fn make_hash<Q: Hash + ?Sized, S: BuildHasher>(
    hash_builder: &S,
    val: &Q,
) -> u64 {
    hash_builder.hash_one(val)
}

/// Iterator over matching positions in an h2 sidecar.
///
/// Scans `h2_bytes[0..len]` in 16-byte chunks, yielding each index `i` where
/// `h2_bytes[i] == needle`. Chunks that lie fully inside the `[0, N)` array use
/// a single SSE2 compare on x86_64; the final partial chunk (and every chunk on
/// other architectures) uses a scalar loop that LLVM auto-vectorises. Chunks
/// are loaded lazily, so iteration stops as soon as the caller stops pulling —
/// a lookup that hits in the first chunk never touches the rest.
pub(super) struct MatchH2 {
    ptr: *const u8,
    len: usize,
    /// Array capacity `N`; bounds the in-place 16-byte SIMD loads.
    cap: usize,
    needle: u8,
    /// Base index of the next chunk to load.
    next_base: usize,
    /// Remaining match bits for the current chunk, relative to `cur_base`.
    cur: u32,
    cur_base: usize,
}

impl Iterator for MatchH2 {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        loop {
            if self.cur != 0 {
                let j = self.cur.trailing_zeros() as usize;
                self.cur &= self.cur - 1;
                return Some(self.cur_base + j);
            }
            if self.next_base >= self.len {
                return None;
            }
            let base = self.next_base;
            self.next_base = base + 16;
            self.cur_base = base;
            // SAFETY: `ptr` points to `cap >= len` readable bytes (contract of
            // `match_h2`); `base < len <= cap` here, and `chunk_mask` only reads
            // in-bounds (a full SIMD load only when `base + 16 <= cap`).
            self.cur =
                unsafe { chunk_mask(self.ptr, base, self.len, self.cap, self.needle) };
        }
    }
}

/// Match bitmask (relative to `base`) for the 16-byte chunk at `base`.
///
/// Only positions in `[base, len)` are considered; the returned bits are in
/// `0..min(16, len - base)`.
///
/// # Safety
/// `ptr` must point to at least `cap` readable bytes, with `base < len <= cap`.
#[inline]
unsafe fn chunk_mask(
    ptr: *const u8,
    base: usize,
    len: usize,
    cap: usize,
    needle: u8,
) -> u32 {
    unsafe {
        // `base < len` (caller-guaranteed) so `valid` ∈ 1..=16, and
        // `(1 << valid) - 1` never overflows a u32.
        let valid = (len - base).min(16);

        #[cfg(target_arch = "x86_64")]
        if base + 16 <= cap {
            // A full 16-byte SIMD load is in bounds. It may read into
            // `[len, cap)` (whose h2 bytes can be stale after a swap-remove),
            // but `lane_mask` discards anything at or past `len`.
            let lane_mask = (1u32 << valid) - 1;
            use core::arch::x86_64::*;
            let needle_v = _mm_set1_epi8(needle as i8);
            let g = _mm_loadu_si128(ptr.add(base) as *const __m128i);
            let m = _mm_movemask_epi8(_mm_cmpeq_epi8(g, needle_v)) as u32;
            return m & lane_mask;
        }

        // Only the x86_64 SIMD path above consults `cap` (to decide whether a
        // full 16-byte load stays in bounds); off x86_64 every chunk takes the
        // scalar tail, so mark `cap` used to keep those builds warning-clean.
        #[cfg(not(target_arch = "x86_64"))]
        let _ = cap;

        // Scalar tail (final partial chunk, or every chunk off x86_64). All
        // reads are within `[base, base + valid) ⊆ [0, len) ⊆ [0, cap)`.
        let mut m = 0u32;
        let mut j = 0;
        while j < valid {
            if *ptr.add(base + j) == needle {
                m |= 1 << j;
            }
            j += 1;
        }
        m
    }
}

/// Scan `h2_bytes[0..len]` for positions equal to `needle`, returning an
/// iterator over the matching indices.
///
/// The const generic `N` is the array capacity (`>= len`); it bounds the
/// in-place SIMD loads. Any `N` is supported.
///
/// # Safety
/// `h2_bytes` must point to at least `N` readable bytes, with `len <= N`.
#[inline]
pub(super) unsafe fn match_h2<const N: usize>(
    h2_bytes: *const u8,
    len: usize,
    needle: u8,
) -> MatchH2 {
    debug_assert!(len <= N);
    MatchH2 {
        ptr: h2_bytes,
        len,
        cap: N,
        needle,
        next_base: 0,
        cur: 0,
        cur_base: 0,
    }
}
