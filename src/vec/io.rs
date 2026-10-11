use core::alloc::Allocator;

use crate::vec::types::EcoVec;

impl<A> std::io::Write for EcoVec<u8, A>
where
    A: Allocator + Clone,
{
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
