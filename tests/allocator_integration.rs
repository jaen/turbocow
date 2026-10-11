//! Integration tests for turbocow with custom allocators

use turbocow::{EcoVec, Global};

use bump_scope::{Bump, settings::BumpSettings};

// Type alias for simpler bump allocator usage: minimum alignment 1, bumps
// upward, not guaranteed to be allocated.
type SimpleBump = Bump<Global, BumpSettings<1, true, false>>;

#[test]
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
fn test_clone_with_allocator() {
    let bump = SimpleBump::new();

    let vec1: EcoVec<i32, _> = EcoVec::from_slice_in(&[1, 2, 3, 4], &bump);
    let vec2 = vec1.clone();

    assert_eq!(vec1.as_slice(), vec2.as_slice());
}

#[test]
fn test_make_mut_with_bump() {
    let bump = SimpleBump::new();

    let mut vec: EcoVec<i32, _> = EcoVec::from_slice_in(&[1, 2, 3], &bump);
    let slice = vec.make_mut();
    slice[0] = 10;
    assert_eq!(vec.as_slice(), &[10, 2, 3]);
}

#[test]
fn test_large_allocation_with_bump() {
    let bump = SimpleBump::new();

    // Large vector
    let vec: EcoVec<u8, _> = EcoVec::from_elem_in(0xAB, 10000, &bump);
    assert_eq!(vec.len(), 10000);
    assert!(vec.as_slice().iter().all(|&b| b == 0xAB));
}

// Unit tests for basic functionality with the Global allocator
mod basic_tests {
    use turbocow::{EcoVec, Global};

    #[test]
    fn test_ecovec_constructors() {
        // The standard constructors use the Global allocator, spelled out...
        {
            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::new();
            assert_eq!(vec.len(), 0);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::with_capacity(10);
            assert!(vec.capacity() >= 10);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_elem(42, 5);
            assert_eq!(vec.as_slice(), &[42; 5]);

            let vec: EcoVec<i32, Global> = EcoVec::<i32, Global>::from_slice(&[1, 2, 3]);
            assert_eq!(vec.as_slice(), &[1, 2, 3]);
        }

        // ...or left to the `EcoVec<T, A = Global>` default
        {
            // `turbocow::Global` is the standard library's `Global`.
            let _: EcoVec<i32, std::alloc::Global> = EcoVec::<i32>::new();

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

// Allocators beyond bump-scope: every free is checked against the block it
// frees, and only allocators with interchangeable clones make `EcoVec: Clone`.
mod tracking_tests {
    #[cfg(feature = "std")]
    use std::alloc::System;
    use std::alloc::{AllocError, Allocator, Layout};
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::ptr::NonNull;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use turbocow::{EcoVec, Global, eco_vec};

    /// Forwards to `Global` and records every live block, so a test can check
    /// that each one is freed exactly once, with the layout it was allocated
    /// with, by an allocator that can free it.
    #[derive(Default)]
    struct Tracking {
        live: Mutex<HashMap<usize, Layout>>,
        errors: Mutex<Vec<String>>,
    }

    impl Tracking {
        fn live_blocks(&self) -> usize {
            self.live.lock().unwrap().len()
        }

        fn assert_balanced(&self) {
            assert_eq!(*self.errors.lock().unwrap(), Vec::<String>::new());
            assert_eq!(self.live_blocks(), 0, "leaked blocks");
        }
    }

    unsafe impl Allocator for Tracking {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            let block = Global.allocate(layout)?;
            self.live
                .lock()
                .unwrap()
                .insert(block.cast::<u8>().addr().get(), layout);
            Ok(block)
        }

        // `grow` and `shrink` use the trait's defaults (allocate, copy,
        // deallocate), so they are tracked through the two methods here.
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            // Never unwind out of an allocator: record mismatches instead, and
            // only free blocks this allocator handed out (with their real
            // layout).
            let allocated = self.live.lock().unwrap().remove(&ptr.addr().get());
            match allocated {
                Some(allocated) => {
                    if allocated != layout {
                        self.errors.lock().unwrap().push(format!(
                            "freed {ptr:?} as {layout:?}, allocated as {allocated:?}"
                        ));
                    }
                    unsafe { Global.deallocate(ptr, allocated) }
                }
                None => self.errors.lock().unwrap().push(format!(
                    "freed {ptr:?}, which this allocator did not allocate"
                )),
            }
        }
    }

    #[test]
    fn test_clone_on_write_frees_every_block() {
        let tracking = Tracking::default();
        {
            let mut a = EcoVec::new_in(&tracking);
            for i in 0..100 {
                a.push(i); // grows repeatedly
            }
            let b = a.clone(); // shares the buffer
            let mut c = b.clone();
            c.push(100); // clone-on-write: fresh buffer
            assert_eq!(c.len(), 101);
            assert_eq!(tracking.live_blocks(), 2);

            a.truncate(10); // shared: copies into a fresh buffer
            a.shrink_to_fit();
            a.insert(0, -1);
            assert_eq!(a.remove(0), -1);
            a.retain(|x| *x % 2 == 0);
            a.extend([1000, 1001]);
            assert_eq!(a.as_slice(), &[0, 2, 4, 6, 8, 1000, 1001]);
            drop(b);

            // A shared `into_iter` clones lazily instead of moving out.
            let collected: Vec<_> = c.clone().into_iter().collect();
            assert_eq!(collected.len(), 101);
            let drained: Vec<_> = c.drain(..50).collect();
            assert_eq!(drained.len(), 50);
            let removed: Vec<_> = c.splice(..10, [7; 20]).collect();
            assert_eq!(removed.len(), 10);
            assert_eq!(c.len(), 61);

            let d = eco_vec![in &tracking; 1, 2, 3];
            let e = EcoVec::from_elem_in(String::from("x"), 5, &tracking);
            assert_eq!((d.len(), e.len()), (3, 5));
            c.clear();
        }
        tracking.assert_balanced();
    }

    #[test]
    fn test_last_clone_frees_with_its_own_allocator() {
        // Each vector holds its own `Arc`/`Rc` clone; whichever is dropped
        // last frees the shared buffer through it.
        let tracking = Arc::new(Tracking::default());
        let a = EcoVec::from_slice_in(&[1, 2, 3], Arc::clone(&tracking));
        let b = a.clone();
        drop(a);
        assert_eq!(tracking.live_blocks(), 1);
        // `EcoVec<T, Arc<_>>` is `Send`, so the last owner may be elsewhere.
        thread::spawn(move || assert_eq!(b, [1, 2, 3])).join().unwrap();
        tracking.assert_balanced();

        let tracking = Rc::new(Tracking::default());
        let a = EcoVec::from_slice_in(&[1, 2, 3], Rc::clone(&tracking));
        let mut b = a.clone();
        drop(a);
        b.push(4); // unique again: grows in place
        drop(b);
        tracking.assert_balanced();
    }

    // turbocow can only name `System` (to make it `Clone`) with its `std`
    // feature.
    #[test]
    #[cfg(feature = "std")]
    fn test_system_allocator() {
        let a = eco_vec![in System; 1, 2, 3];
        let mut b = a.clone();
        b.push(4);
        assert_eq!((a.as_slice(), b.as_slice()), (&[1, 2, 3][..], &[1, 2, 3, 4][..]));
    }

    /// An allocator turbocow knows nothing about: its clones may or may not be
    /// interchangeable, so vectors using it by value are not `Clone`.
    #[derive(Clone, Copy)]
    struct ByValue;

    unsafe impl Allocator for ByValue {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            Global.allocate(layout)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { Global.deallocate(ptr, layout) }
        }
    }

    // Whether `$ty: Clone`: an inherent method whose bound holds is picked over
    // the trait method of the same name.
    struct Probe<T>(PhantomData<T>);
    trait NotClone {
        fn is_clone(&self) -> bool {
            false
        }
    }
    impl<T> NotClone for Probe<T> {}
    impl<T: Clone> Probe<T> {
        fn is_clone(&self) -> bool {
            true
        }
    }
    macro_rules! is_clone {
        ($ty:ty) => {
            Probe::<$ty>(PhantomData).is_clone()
        };
    }

    #[test]
    fn test_clone_needs_interchangeable_allocator_clones() {
        assert!(is_clone!(EcoVec<i32>));
        #[cfg(feature = "std")]
        assert!(is_clone!(EcoVec<i32, System>));
        assert!(is_clone!(EcoVec<i32, &ByValue>));
        assert!(is_clone!(EcoVec<i32, &dyn Allocator>));
        assert!(is_clone!(EcoVec<i32, Arc<ByValue>>));
        assert!(is_clone!(EcoVec<i32, Rc<ByValue>>));

        assert!(!is_clone!(EcoVec<i32, ByValue>));
        assert!(!is_clone!(EcoVec<i32, Box<ByValue>>));

        // By value, `ByValue` still backs every non-sharing operation.
        let mut vec = EcoVec::from_slice_in(&[1, 2, 3], ByValue);
        vec.push(4);
        assert_eq!(vec, [1, 2, 3, 4]);
    }
}
