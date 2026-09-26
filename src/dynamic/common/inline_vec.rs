use crate::allocator::{AllocatorProvider, Global};
use crate::dynamic::types::InlineVec;

use super::limit::{LEN_INLINE_TAG, LEN_VALUE_MASK, LIMIT};

impl InlineVec<LIMIT, Global> {
    #[inline]
    pub const fn new() -> Self {
        Self::new_in(Global)
    }

    /// Construct an `InlineVec` from a byte slice.
    ///
    /// Returns `Err(())` if the slice exceeds the inline capacity (`LIMIT`).
    ///
    /// This is a `const fn` so it can be called from const contexts such as
    /// [`EcoString::inline`](crate::EcoString::inline).
    #[inline]
    pub const fn from_slice(bytes: &[u8]) -> Result<Self, ()> {
        let len = bytes.len();
        if len > LIMIT {
            return Err(());
        }
        let mut buf = [0u8; LIMIT];
        let mut i = 0;
        while i < len {
            buf[i] = bytes[i];
            i += 1;
        }
        // Safety: len <= LIMIT was checked above.
        unsafe { Ok(Self::from_buf(buf, len)) }
    }

    /// The given length may not exceed LIMIT.
    #[inline]
    #[allow(dead_code)]
    pub const unsafe fn from_buf(buf: [u8; LIMIT], len: usize) -> Self {
        unsafe { Self::from_buf_in(buf, len, Global) }
    }
}

impl<const N: usize, A> InlineVec<N, A>
where
    A: AllocatorProvider,
{
    #[inline]
    pub const fn new_in(allocator: A) -> Self {
        // Safety: Trivially, 0 <= N
        unsafe { Self::from_buf_in([0; N], 0, allocator) }
    }

    // Allocator-generic counterpart of the `Global` const `from_slice`.
    // Currently unused (the const `from_slice` covers the default path);
    // kept for the in-progress allocator-api feature work.
    #[inline]
    #[allow(dead_code)]
    pub fn from_slice_in(bytes: &[u8], allocator: A) -> Result<Self, ()> {
        let len = bytes.len();
        if len > N {
            return Err(());
        }

        let mut buf = [0; N];
        buf[..len].copy_from_slice(bytes);

        // Safety: If len > N, Err was returned earlier.
        unsafe { Ok(Self::from_buf_in(buf, len, allocator)) }
    }

    /// The given length may not exceed N.
    #[inline]
    pub const unsafe fn from_buf_in(buf: [u8; N], len: usize, allocator: A) -> Self {
        // Safe body; `unsafe fn` because the `len <= N` precondition is a caller contract.
        debug_assert!(len <= N);

        Self {
            buf,
            tagged_len: len as u8 | LEN_INLINE_TAG,
            allocator,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        usize::from(self.tagged_len & LEN_VALUE_MASK)
    }

    /// The given length may not exceed N.
    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        // Safe body; `unsafe fn` because the `len <= N` precondition is a caller contract.
        debug_assert!(len <= N);
        self.tagged_len = len as u8 | LEN_INLINE_TAG;
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // Safety: We have the invariant `len <= N`.
        unsafe { self.buf.get_unchecked(..self.len()) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // Safety: We have the invariant `len <= N`.
        let len = self.len();
        unsafe { self.buf.get_unchecked_mut(..len) }
    }

    #[inline]
    pub fn clear(&mut self) {
        unsafe {
            // Safety: Trivially, `0 <= N`.
            self.set_len(0);
        }
    }

    #[inline]
    pub fn push(&mut self, byte: u8) -> Result<(), ()> {
        let len = self.len();
        if let Some(slot) = self.buf.get_mut(len) {
            *slot = byte;
            unsafe {
                // Safety: The `get_mut` call guarantees that `len < N`.
                self.set_len(len + 1);
            }
            Ok(())
        } else {
            Err(())
        }
    }

    #[inline]
    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let len = self.len();
        let grown = len + bytes.len();
        if let Some(segment) = self.buf.get_mut(len..grown) {
            segment.copy_from_slice(bytes);
            unsafe {
                // Safety: The `get_mut` call guarantees that `grown <= N`.
                self.set_len(grown);
            }
            Ok(())
        } else {
            Err(())
        }
    }

    /// Inserts `bytes` at `index`, shifting the tail right.
    ///
    /// Returns `Err(())` and leaves `self` unchanged if the result would not
    /// fit inline. The caller guarantees `index <= len`.
    #[inline]
    pub fn insert_slice(&mut self, index: usize, bytes: &[u8]) -> Result<(), ()> {
        let len = self.len();
        debug_assert!(index <= len);
        let grown = len + bytes.len();
        if grown > N {
            return Err(());
        }
        self.buf.copy_within(index..len, index + bytes.len());
        self.buf[index..index + bytes.len()].copy_from_slice(bytes);
        unsafe {
            // Safety: Checked that `grown <= N` above.
            self.set_len(grown);
        }
        Ok(())
    }

    /// Removes and returns the byte at `index`, shifting the tail left.
    ///
    /// The caller guarantees `index < len`.
    #[inline]
    pub fn remove(&mut self, index: usize) -> u8 {
        let len = self.len();
        debug_assert!(index < len);
        let byte = self.buf[index];
        self.buf.copy_within(index + 1..len, index);
        unsafe {
            // Safety: `index < len` implies `len - 1 < len <= N`.
            self.set_len(len - 1);
        }
        byte
    }

    #[inline]
    pub fn truncate(&mut self, target: usize) {
        if target < self.len() {
            unsafe {
                // Safety: Checked that it's smaller than the current length,
                // which cannot exceed N itself.
                self.set_len(target);
            }
        }
    }

    #[inline(always)]
    pub(crate) fn get_allocator(&self) -> &A {
        &self.allocator
    }

    /// Construct an inline `DynamicVec` directly into `place`.
    ///
    /// This is the inline writer for the allocator-generic
    /// [`from_slice_in_owned`](crate::dynamic::types::DynamicVec::from_slice_in_owned)
    /// path. The intent is to write straight into the final location, but note
    /// it only pays off when `place` genuinely *is* that location. For a
    /// standalone `Global` construct (the `EcoString::from(&str)` hot path) the
    /// `MaybeUninit` slot does **not** promote to registers, so emplacing here
    /// adds a stack round-trip; that path therefore uses the by-value
    /// `DynamicVec::from_slice` (`InlineVec::from_slice` → `from_inline`)
    /// instead. Emplacing still earns its keep for non-ZST allocators and the
    /// non-`TAG_OVERLAPS_LEN` layouts, where the tag byte must be written into
    /// the slot in place (by-value union construction can't express that).
    ///
    /// Uses word-sized writes to the (aligned) destination buffer and
    /// unaligned reads from the source for optimal codegen on small copies
    /// (the inline limit is typically 15 bytes = at most 1–2 words).
    ///
    /// Returns `true` if the data fit inline (i.e. `bytes.len() + size_hint <= N`),
    /// `false` otherwise (caller must fall back to a heap allocation).
    /// The allocator is borrowed and only cloned when the data actually fits
    /// inline, so the caller can reuse it for the spill fallback without a
    /// redundant clone.
    #[inline(always)]
    pub fn emplace_from_slice_in<'a>(
        place: &mut core::mem::MaybeUninit<crate::dynamic::types::DynamicVec<'a, N, A>>,
        bytes: &[u8],
        allocator: &A,
        size_hint: usize,
    ) -> bool
    where
        A: Clone,
    {
        const WORD: usize = core::mem::size_of::<usize>();

        let len = bytes.len();
        if len + size_hint > N {
            return false;
        }

        unsafe {
            let place_ptr: *mut InlineVec<N, A> = place.as_mut_ptr().cast();
            let buf_ptr: *mut u8 = place_ptr.cast();
            let src_ptr: *const u8 = bytes.as_ptr();

            // Zero the inline buffer so tail bytes [len..N) are deterministically
            // 0. The buffer can exceed two words (N = 23 on 64-bit big-endian),
            // so zero it byte-wise rather than with the word-pair writes the
            // little-endian limits (N = 15 / 7) used to allow. The compiler
            // lowers this to the same word stores for those small N.
            core::ptr::write_bytes(buf_ptr, 0, N);
            let dst = buf_ptr as *mut usize;

            // Fast path: copy the first aligned word when at least a word fits,
            // then the byte loop copies the remainder (WORD-1 bytes for the small
            // limits, more for the larger big-endian limit).
            let offset = if len >= WORD {
                let src = src_ptr as *const usize;
                dst.write(src.read_unaligned());
                WORD
            } else {
                0
            };

            // Copy remaining bytes (at most WORD-1 = 7 on 64-bit).
            let mut j = offset;
            while j < len {
                *buf_ptr.add(j) = *src_ptr.add(j);
                j += 1;
            }

            // Write tagged_len at offset N.
            buf_ptr.add(N).write(len as u8 | LEN_INLINE_TAG);

            // Write the allocator field — clone only on success.
            core::ptr::addr_of_mut!((*place_ptr).allocator).write(allocator.clone());
        }

        true
    }
}
