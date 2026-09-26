//! Integration tests for turbocow with custom allocators

#![cfg(feature = "allocator-api")]
#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]

// File-level import is only needed by the v02 tests below; the `basic_tests`
// module has its own `use turbocow::EcoVec;`.
#[cfg(feature = "allocator-api2-v02")]
use turbocow::EcoVec;

#[cfg(feature = "allocator-api2-v02")]
use bump_scope::Bump;

// Type alias for simpler bump allocator usage
#[cfg(feature = "allocator-api2-v02")]
type SimpleBump = Bump<allocator_api2_02::alloc::Global, 1, true, false, true>;

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_ecovec_with_bump_allocator() {
    let bump = SimpleBump::new();

    // Test new_in
    let vec: EcoVec<i32, _> = EcoVec::new_in(&bump);
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());

    // Test with_capacity_in
    let vec: EcoVec<u8, _> = EcoVec::with_capacity_in(100, &bump);
    assert_eq!(vec.len(), 0);
    assert!(vec.capacity() >= 100);

    // Test from_elem_in
    let vec: EcoVec<i32, _> = EcoVec::from_elem_in(42, 10, &bump);
    assert_eq!(vec.len(), 10);
    assert_eq!(vec.as_slice(), &[42; 10]);

    // Test from_slice_in
    let data = vec![1, 2, 3, 4, 5];
    let vec: EcoVec<i32, _> = EcoVec::from_slice_in(&data, &bump);
    assert_eq!(vec.as_slice(), &data[..]);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_ecovec_mutations_with_bump() {
    let bump = SimpleBump::new();

    let mut vec: EcoVec<i32, _> = EcoVec::new_in(&bump);

    // Push operations
    vec.push(1);
    vec.push(2);
    vec.push(3);
    assert_eq!(vec.as_slice(), &[1, 2, 3]);

    // Pop operation
    assert_eq!(vec.pop(), Some(3));
    assert_eq!(vec.as_slice(), &[1, 2]);

    // Extend from slice
    vec.extend_from_slice(&[4, 5, 6]);
    assert_eq!(vec.as_slice(), &[1, 2, 4, 5, 6]);

    // Truncate
    vec.truncate(3);
    assert_eq!(vec.as_slice(), &[1, 2, 4]);

    // Clear
    vec.clear();
    assert!(vec.is_empty());
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_clone_with_allocator() {
    let bump = SimpleBump::new();

    let vec1: EcoVec<i32, _> = EcoVec::from_slice_in(&[1, 2, 3, 4], &bump);
    let vec2 = vec1.clone();

    assert_eq!(vec1.as_slice(), vec2.as_slice());
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_make_mut_with_bump() {
    let bump = SimpleBump::new();

    let mut vec: EcoVec<i32, _> = EcoVec::from_slice_in(&[1, 2, 3], &bump);
    let slice = vec.make_mut();
    slice[0] = 10;
    assert_eq!(vec.as_slice(), &[10, 2, 3]);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_large_allocation_with_bump() {
    let bump = SimpleBump::new();

    // Large vector
    let vec: EcoVec<u8, _> = EcoVec::from_elem_in(0xAB, 10000, &bump);
    assert_eq!(vec.len(), 10000);
    assert!(vec.as_slice().iter().all(|&b| b == 0xAB));
}

// Unit tests for basic functionality with Global allocator (when allocator features are enabled)
mod basic_tests {
    use turbocow::EcoVec;

    #[test]
    fn test_ecovec_constructors() {
        // Test standard constructors still work
        // When allocator features are enabled, these use the Global allocator
        #[cfg(any(
            feature = "allocator-api2-v02",
            feature = "allocator-api2-v03",
            feature = "nightly-allocator-api"
        ))]
        {
            use turbocow::Global;
            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();
            assert_eq!(vec.len(), 0);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::with_capacity(10);
            assert!(vec.capacity() >= 10);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_elem(42, 5);
            assert_eq!(vec.as_slice(), &[42; 5]);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_slice(&[1, 2, 3]);
            assert_eq!(vec.as_slice(), &[1, 2, 3]);
        }

        #[cfg(not(any(
            feature = "allocator-api2-v02",
            feature = "allocator-api2-v03",
            feature = "nightly-allocator-api"
        )))]
        {
            let vec: EcoVec<i32> = EcoVec::new();
            assert_eq!(vec.len(), 0);

            let vec: EcoVec<i32> = EcoVec::with_capacity(10);
            assert!(vec.capacity() >= 10);

            let vec: EcoVec<i32> = EcoVec::from_elem(42, 5);
            assert_eq!(vec.as_slice(), &[42; 5]);

            let vec: EcoVec<i32> = EcoVec::from_slice(&[1, 2, 3]);
            assert_eq!(vec.as_slice(), &[1, 2, 3]);
        }
    }
}
