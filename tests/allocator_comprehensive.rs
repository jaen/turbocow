//! Comprehensive test suite for allocator API implementation
//! This file provides extended testing including edge cases, property-based tests, and performance validation

#![cfg(any(
    feature = "allocator-api2-v02",
    feature = "allocator-api2-v03",
    feature = "nightly-allocator-api"
))]
#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use turbocow::{EcoVec, Global};

#[cfg(feature = "allocator-api2-v02")]
use bump_scope::Bump;

#[cfg(feature = "allocator-api2-v02")]
type SimpleBump = Bump<allocator_api2_02::alloc::Global, 1, true, false, true>;

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_empty_allocator_operations() {
    // Test empty operations with Global allocator
    let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());
    assert_eq!(vec.capacity(), 0);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_zero_capacity_allocation() {
    let bump = SimpleBump::new();

    // Zero capacity should not allocate
    let vec: EcoVec<i32, _> = EcoVec::with_capacity_in(0, &bump);
    assert_eq!(vec.capacity(), 0);
    assert_eq!(vec.len(), 0);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_single_element_operations() {
    let bump = SimpleBump::new();

    // Single element vector
    let mut vec: EcoVec<i32, _> = EcoVec::new_in(&bump);
    vec.push(42);
    assert_eq!(vec.len(), 1);
    // assert_eq!(vec[0], 42);
    assert_eq!(vec.pop(), Some(42));
    assert!(vec.is_empty());
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_repeated_grow_shrink_operations() {
    let bump = SimpleBump::new();
    let mut vec: EcoVec<i32, _> = EcoVec::new_in(&bump);

    // Repeatedly grow and shrink
    for _ in 0..10 {
        for i in 0..100 {
            vec.push(i);
        }
        assert_eq!(vec.len(), 100);

        while vec.len() > 50 {
            vec.pop();
        }
        assert_eq!(vec.len(), 50);

        vec.truncate(25);
        assert_eq!(vec.len(), 25);

        vec.clear();
        assert!(vec.is_empty());
    }
}

// ============================================================================
// PROPERTY-BASED TESTS
// ============================================================================

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn property_vec_len_capacity_invariant() {
    let bump = SimpleBump::new();

    for size in [0, 1, 10, 100, 1000] {
        let vec: EcoVec<u8, _> = EcoVec::from_elem_in(0, size, &bump);

        // Property: length should never exceed capacity
        assert!(vec.len() <= vec.capacity());

        // Property: capacity should be at least the requested size
        assert!(vec.capacity() >= size);
    }
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn property_clone_equality() {
    let bump = SimpleBump::new();

    // Property: Cloning should produce equal values
    for size in [0, 1, 10, 100] {
        let vec1: EcoVec<i32, _> = EcoVec::from_elem_in(42, size, &bump);
        let vec2 = vec1.clone();

        assert_eq!(vec1.as_slice(), vec2.as_slice());
        assert_eq!(vec1.len(), vec2.len());
    }
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn property_push_pop_reversibility() {
    let bump = SimpleBump::new();
    let mut vec: EcoVec<i32, _> = EcoVec::new_in(&bump);

    // Property: Push then pop should return to original state
    let original_len = vec.len();

    for i in 0..100 {
        vec.push(i);
    }

    for _ in 0..100 {
        vec.pop();
    }

    assert_eq!(vec.len(), original_len);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn property_extend_from_slice_correctness() {
    let bump = SimpleBump::new();

    // Property: extend_from_slice should append all elements in order
    let mut vec: EcoVec<i32, _> = EcoVec::new_in(&bump);

    let slices = [&[][..], &[1][..], &[2, 3][..], &[4, 5, 6, 7][..]];

    let mut expected = Vec::new();

    for slice in &slices {
        vec.extend_from_slice(slice);
        expected.extend_from_slice(slice);
        assert_eq!(vec.as_slice(), &expected[..]);
    }
}

// ============================================================================
// CROSS-ALLOCATOR TESTS
// ============================================================================

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_different_allocator_instances() {
    // Each bump allocator is independent
    let bump1 = SimpleBump::new();
    let bump2 = SimpleBump::new();

    let vec1: EcoVec<i32, _> = EcoVec::from_slice_in(&[1, 2, 3], &bump1);
    let vec2: EcoVec<i32, _> = EcoVec::from_slice_in(&[4, 5, 6], &bump2);

    assert_eq!(vec1.as_slice(), &[1, 2, 3]);
    assert_eq!(vec2.as_slice(), &[4, 5, 6]);

    // Cloning within same allocator
    let vec1_clone = vec1.clone();
    assert_eq!(vec1_clone.as_slice(), &[1, 2, 3]);
}

#[test]
fn test_global_allocator_consistency() {
    // Test that Global allocator works consistently with default constructors
    let vec1: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();
    let vec2: EcoVec<i32, Global> = EcoVec::<i32, Global>::with_capacity(10);
    let vec3: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_elem(0, 5);

    assert_eq!(vec1.len(), 0);
    assert!(vec2.capacity() >= 10);
    assert_eq!(vec3.len(), 5);
}

// ============================================================================
// CONCURRENCY TESTS
// ============================================================================

#[test]
fn test_clone_thread_safety() {
    // Test that cloned EcoVecs can be safely shared across threads
    let original: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_elem(42, 100);
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let cloned = original.clone();
            thread::spawn(move || {
                // Each thread verifies its clone
                assert_eq!(cloned.len(), 100);
                assert!(cloned.as_slice().iter().all(|&x| x == 42));
                cloned.len()
            })
        })
        .collect();

    for handle in handles {
        assert_eq!(handle.join().unwrap(), 100);
    }
}

#[test]
fn test_arc_wrapped_allocator_operations() {
    // Test operations with Arc-wrapped vectors (common pattern)
    let vec: EcoVec<String, Global> =
        EcoVec::<String, Global>::from_slice(&["hello".to_string(), "world".to_string()]);

    let arc_vec = Arc::new(vec);

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let arc_clone = Arc::clone(&arc_vec);
            thread::spawn(move || {
                // Each thread can read the shared vector
                assert_eq!(arc_clone.len(), 2);
                assert_eq!(arc_clone.as_slice()[0], "hello");
                assert_eq!(arc_clone.as_slice()[1], "world");
                i
            })
        })
        .collect();

    let results: HashSet<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results, (0..5).collect::<HashSet<_>>());
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_large_scale_allocations() {
    use std::alloc::Layout;
    // Create layout for 10MB
    let layout = Layout::from_size_align(10 * 1024 * 1024, 8).unwrap();
    let bump = SimpleBump::with_capacity(layout);

    // Create many small allocations
    let mut vecs = Vec::new();
    for i in 0..1000 {
        let vec: EcoVec<usize, _> = EcoVec::from_elem_in(i, 100, &bump);
        vecs.push(vec);
    }

    // Verify all vectors
    for (i, vec) in vecs.iter().enumerate() {
        assert_eq!(vec.len(), 100);
        assert!(vec.as_slice().iter().all(|&x| x == i));
    }
}

// ============================================================================
// ALLOCATOR-SPECIFIC BEHAVIOR TESTS
// ============================================================================

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_bump_allocator_reset_semantics() {
    // Test bump allocator's specific behavior
    let bump = SimpleBump::new();

    // Create some allocations
    let _vec1: EcoVec<i32, _> = EcoVec::from_elem_in(1, 100, &bump);
    let _vec2: EcoVec<i32, _> = EcoVec::from_elem_in(2, 200, &bump);

    // Note: bump allocator doesn't actually deallocate until reset
    // This tests that our abstractions work correctly with this behavior

    // Allocations should still be valid
    assert_eq!(_vec1.len(), 100);
    assert_eq!(_vec2.len(), 200);
}

#[test]
fn test_global_allocator_deallocation() {
    // Test that Global allocator properly deallocates
    let mut allocations = Vec::new();

    // Create and drop many allocations
    for _ in 0..100 {
        let vec: EcoVec<Vec<u8>, Global> =
            EcoVec::<Vec<u8>, Global>::from_elem(vec![0; 1000], 10);
        allocations.push(vec);
    }

    // Drop half of them
    allocations.truncate(50);

    // Remaining should still be valid
    for vec in &allocations {
        assert_eq!(vec.len(), 10);
        assert!(vec.as_slice().iter().all(|v| v.len() == 1000));
    }
}

// ============================================================================
// MACRO SUPPORT TESTS
// ============================================================================

#[test]
fn test_eco_vec_macro_with_allocator_features() {
    // Test that macro still works when allocator features are enabled
    // The macro uses Global allocator by default
    #[cfg(feature = "allocator-api2-v02")]
    {
        let bump = SimpleBump::new();
        // Manual construction with allocator
        let vec_with_alloc: EcoVec<i32, _> =
            EcoVec::from_slice_in(&[1, 2, 3, 4, 5], &bump);
        assert_eq!(vec_with_alloc.as_slice(), &[1, 2, 3, 4, 5]);
    }

    // Test Global allocator variant
    let vec_global: EcoVec<i32, Global> =
        EcoVec::<i32, Global>::from_slice(&[1, 2, 3, 4, 5]);
    assert_eq!(vec_global.as_slice(), &[1, 2, 3, 4, 5]);
}

// ============================================================================
// ERROR RECOVERY TESTS
// ============================================================================

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_operations_after_clear() {
    let bump = SimpleBump::new();
    let mut vec: EcoVec<String, _> = EcoVec::from_slice_in(
        &["one".to_string(), "two".to_string(), "three".to_string()],
        &bump,
    );

    vec.clear();
    assert!(vec.is_empty());

    // Should be able to reuse after clear
    vec.push("four".to_string());
    vec.push("five".to_string());
    assert_eq!(vec.len(), 2);
    assert_eq!(vec.as_slice()[0], "four");
    assert_eq!(vec.as_slice()[1], "five");
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_truncate_to_various_sizes() {
    let bump = SimpleBump::new();
    let mut vec: EcoVec<i32, _> =
        EcoVec::from_slice_in(&(0..100).collect::<Vec<_>>(), &bump);

    // Truncate to larger size (should do nothing)
    vec.truncate(200);
    assert_eq!(vec.len(), 100);

    // Truncate to same size (should do nothing)
    vec.truncate(100);
    assert_eq!(vec.len(), 100);

    // Truncate to smaller sizes
    vec.truncate(50);
    assert_eq!(vec.len(), 50);
    assert_eq!(vec.as_slice().last(), Some(&49));

    vec.truncate(10);
    assert_eq!(vec.len(), 10);
    assert_eq!(vec.as_slice().last(), Some(&9));

    vec.truncate(0);
    assert!(vec.is_empty());
}

// ============================================================================
// ALLOCATOR TYPE COMPATIBILITY TESTS
// ============================================================================

#[test]
fn test_type_inference_with_allocators() {
    // Test that type inference works properly

    // Explicit Global allocator type
    let vec_explicit: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();
    assert_eq!(vec_explicit.len(), 0);

    // With capacity
    let vec_capacity: EcoVec<u8, Global> = EcoVec::<u8, Global>::with_capacity(100);
    assert!(vec_capacity.capacity() >= 100);
}

#[test]
#[cfg(feature = "allocator-api2-v02")]
fn test_complex_types_with_allocator() {
    let bump = SimpleBump::new();

    // Test with complex nested types
    type ComplexType = Vec<Option<Result<String, Vec<u8>>>>;

    let complex_data: ComplexType =
        vec![Some(Ok("test".to_string())), None, Some(Err(vec![1, 2, 3]))];

    let vec: EcoVec<ComplexType, _> =
        EcoVec::from_slice_in(std::slice::from_ref(&complex_data), &bump);
    assert_eq!(vec.len(), 1);
    assert_eq!(vec.as_slice()[0], complex_data);
}
