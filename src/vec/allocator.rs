use super::types;

use crate::allocator::Global;

/// An `EcoVec` that defaults to the [`Global`] allocator.
///
/// The allocator type parameter defaults to [`Global`] so that the common
/// `EcoVec<T>` spelling keeps working in type position (e.g. `EcoVec<u8>`),
/// while method calls without an explicit allocator (`EcoVec::with_capacity`,
/// `eco_vec![..]`) infer `A = Global` through the alias — mirroring the
/// non-allocator-api build's `EcoVec<T>` alias.
pub type EcoVec<T, A = Global> = types::EcoVec<T, A>;

/// Create a new [`EcoVec`] with the given elements.
/// ```
/// # use turbocow::eco_vec;
/// assert_eq!(eco_vec![1; 4], [1; 4]);
/// assert_eq!(eco_vec![1, 2, 3], [1, 2, 3]);
/// ```
///
/// Optionally, you can specify an allocator to use. (This example uses
/// `bump_scope::Bump`, whose `Allocator` impl is wired up under the
/// `allocator-api2-v02` feature.)
/// ```
/// # use turbocow::eco_vec;
/// # use bump_scope::Bump;
/// let allocator: Bump = Bump::new();
/// assert_eq!(eco_vec![in &allocator; 1; 4], [1; 4]);
/// assert_eq!(eco_vec![in &allocator; 1, 2, 3], [1, 2, 3]);
/// ```
#[macro_export]
macro_rules! eco_vec {
    () => {
        $crate::EcoVec::new()
    };

    (in $alloc:expr) => {
        $crate::EcoVec::new_in($alloc)
    };

    ($elem:expr; $n:expr) => {
        $crate::EcoVec::from_elem($elem, $n)
    };

    (in $alloc:expr; $elem:expr; $n:expr) => {
        $crate::EcoVec::from_elem_in($elem, $n, $alloc)
    };

    ($($value:expr),+ $(,)?) => {
        $crate::EcoVec::from([$($value),+])
    };

    (in $alloc:expr; $($value:expr),+ $(,)?) => {
        $crate::EcoVec::from_slice_in(&[$($value),+], $alloc)
    };
}
