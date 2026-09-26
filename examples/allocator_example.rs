//! Example of using turbocow with custom allocators

#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]

// use turbocow::{EcoVec, EcoString};
use turbocow::EcoVec;

#[cfg(any(
    feature = "allocator-api2-v02",
    feature = "allocator-api2-v03",
    feature = "nightly-allocator-api"
))]
use turbocow::Global;

fn main() {
    println!("Turbocow allocator example");

    // Standard usage without custom allocators
    standard_usage();

    // With bump allocator
    #[cfg(feature = "allocator-api2-v02")]
    bump_allocator_usage();
}

fn standard_usage() {
    println!("\n=== Standard Usage (Global Allocator) ===");

    // EcoVec examples
    #[cfg(not(any(
        feature = "allocator-api2-v02",
        feature = "allocator-api2-v03",
        feature = "nightly-allocator-api"
    )))]
    let mut vec = EcoVec::new();

    #[cfg(any(
        feature = "allocator-api2-v02",
        feature = "allocator-api2-v03",
        feature = "nightly-allocator-api"
    ))]
    let mut vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();

    vec.push(1);
    vec.push(2);
    vec.push(3);
    println!("EcoVec: {:?}", vec.as_slice());

    #[cfg(not(any(
        feature = "allocator-api2-v02",
        feature = "allocator-api2-v03",
        feature = "nightly-allocator-api"
    )))]
    let vec2 = EcoVec::from_elem(42, 5);

    #[cfg(any(
        feature = "allocator-api2-v02",
        feature = "allocator-api2-v03",
        feature = "nightly-allocator-api"
    ))]
    let vec2: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_elem(42, 5);

    println!("EcoVec from_elem: {:?}", vec2.as_slice());

    // // EcoString examples
    // #[cfg(not(any(
    //     feature = "allocator-api2-v02",
    //     feature = "allocator-api2-v03",
    //     feature = "nightly-allocator-api"
    // )))]
    // let mut string = EcoString::new();

    // #[cfg(any(
    //     feature = "allocator-api2-v02",
    //     feature = "allocator-api2-v03",
    //     feature = "nightly-allocator-api"
    // ))]
    // let mut string: EcoString<Global> = EcoString::<Global>::new();

    // string.push_str("Hello, ");
    // string.push_str("World!");
    // println!("EcoString: {}", string.as_str());
}

#[cfg(feature = "allocator-api2-v02")]
fn bump_allocator_usage() {
    use bump_scope::Bump;

    println!("\n=== Bump Allocator Usage ===");

    // Bump allocator with default parameters:
    // - Global allocator as base
    // - Minimum alignment of 1
    // - Grow upward (true)
    // - Guaranteed allocated (true)
    // - Deallocates (true)
    let bump: Bump = Bump::new();

    // EcoVec with bump allocator
    let mut vec = EcoVec::new_in(&bump);
    vec.push(10);
    vec.push(20);
    vec.push(30);
    println!("EcoVec with bump: {:?}", vec.as_slice());

    let vec2: EcoVec<i32, _> = EcoVec::with_capacity_in(100, &bump);
    println!("EcoVec capacity with bump: {}", vec2.capacity());

    let vec3 = EcoVec::from_elem_in(7, 3, &bump);
    println!("EcoVec from_elem_in: {:?}", vec3.as_slice());

    let data = vec![1, 2, 3, 4, 5];
    let vec4 = EcoVec::from_slice_in(&data, &bump);
    println!("EcoVec from_slice_in: {:?}", vec4.as_slice());

    // // EcoString with bump allocator
    // let mut string = EcoString::new_in(&bump);
    // string.push_str("Bump ");
    // string.push_str("allocated!");
    // println!("EcoString with bump: {}", string.as_str());

    // let string2 = EcoString::with_capacity_in(100, &bump);
    // println!("EcoString capacity with bump: {}", string2.len());

    // let string3 = EcoString::from_str_in("Hello from bump!", &bump);
    // println!("EcoString from_str_in: {}", string3.as_str());

    // // Test cloning with allocator
    // let cloned_vec = vec.clone();
    // println!("Cloned vec: {:?}", cloned_vec.as_slice());

    // let cloned_string = string.clone();
    // println!("Cloned string: {}", cloned_string.as_str());

    // // Test spilling from inline
    // let mut small = EcoString::from_str_in("x", &bump);
    // println!("Small string (inline): {}", small.as_str());

    // // This should cause spilling to heap
    // for _ in 0..20 {
    //     small.push('y');
    // }
    // println!("Spilled string: {}", small.as_str());

    println!("\nAll bump allocator tests completed successfully!");
}
