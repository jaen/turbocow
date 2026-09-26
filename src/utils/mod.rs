pub(crate) mod variance;

#[inline(always)]
pub(crate) const fn max(x: usize, y: usize) -> usize {
    if x > y { x } else { y }
}
