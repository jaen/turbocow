use crate::{allocator::Global, vec::types};

/// An `EcoVec` using the global allocator.
pub type EcoVec<T> = types::EcoVec<T, Global>;

/// Create a new [`EcoVec`] with the given elements.
/// ```
/// # use turbocow::eco_vec;
/// assert_eq!(eco_vec![1; 4], [1; 4]);
/// assert_eq!(eco_vec![1, 2, 3], [1, 2, 3]);
/// ```
#[macro_export]
macro_rules! eco_vec {
    () => {
        $crate::EcoVec::new()
    };

    ($elem:expr; $n:expr) => {
        $crate::EcoVec::from_elem($elem, $n)
    };
    ($($value:expr),+ $(,)?) => {
        $crate::EcoVec::from([$($value),+])
    };
}
