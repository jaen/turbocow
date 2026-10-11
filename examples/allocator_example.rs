//! Example of using turbocow with custom allocators

use turbocow::EcoVec;

fn main() {
    println!("Turbocow allocator example");

    // Standard usage without custom allocators
    standard_usage();

    // With bump allocator
    bump_allocator_usage();
}

fn standard_usage() {
    println!("\n=== Standard Usage (Global Allocator) ===");

    // EcoVec examples
    let mut vec = EcoVec::new();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    println!("EcoVec: {:?}", vec.as_slice());

    let vec2 = EcoVec::from_elem(42, 5);
    println!("EcoVec from_elem: {:?}", vec2.as_slice());
}

fn bump_allocator_usage() {
    use bump_scope::Bump;

    println!("\n=== Bump Allocator Usage ===");

    // Bump allocator with `Global` as its base allocator and the default
    // `BumpSettings` (minimum alignment 1, bumps upward, guaranteed allocated,
    // deallocates and shrinks).
    let bump: Bump = Bump::new();

    // EcoVec with bump allocator
    let mut vec = EcoVec::new_in(&bump);
    vec.push(10);
    vec.push(20);
    vec.push(30);
    println!("EcoVec with bump: {:?}", vec.as_slice());

    // Cloning shares the buffer: all copies of `&bump` refer to the same arena
    let shared = vec.clone();
    println!("Cloned EcoVec with bump: {:?}", shared.as_slice());

    let vec2: EcoVec<i32, _> = EcoVec::with_capacity_in(100, &bump);
    println!("EcoVec capacity with bump: {}", vec2.capacity());

    let vec3 = EcoVec::from_elem_in(7, 3, &bump);
    println!("EcoVec from_elem_in: {:?}", vec3.as_slice());

    let data = vec![1, 2, 3, 4, 5];
    let vec4 = EcoVec::from_slice_in(&data, &bump);
    println!("EcoVec from_slice_in: {:?}", vec4.as_slice());

    println!("\nAll bump allocator tests completed successfully!");
}
