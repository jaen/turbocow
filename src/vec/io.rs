use crate::allocator::Global;
use crate::vec::types::EcoVec;

/// # Allocator limitation
///
/// This impl is only available for `EcoVec<u8, Global>`. For an
/// allocator-parameterized `EcoVec<u8, A>`, write to a `Vec<u8>` first
/// and then convert via `EcoVec::from(vec)`.
impl std::io::Write for EcoVec<u8, Global> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.extend_from_byte_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
