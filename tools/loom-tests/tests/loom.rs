#[cfg(not(loom))]
compile_error!(
    "Loom tests require `--cfg loom`. \
     Run with: RUSTFLAGS=\"--cfg loom\" cargo test --release"
);

#[cfg(loom)]
#[cfg(debug_assertions)]
compile_error!("Loom tests are typically slow in debug mode. Run them with `--release`");

#[cfg(loom)]
mod tests {
    use turbocow::eco_vec;

    #[test]
    fn smoke() {
        loom::model(|| {
            let mut one = eco_vec![1, 2, 3];
            let two = one.clone();

            loom::thread::spawn(move || {
                let mut three = two.clone();
                three.push(4);

                assert!(three.len() > two.len());
            });

            one.push(4);
            assert_eq!(one, [1, 2, 3, 4]);
        });
    }

    #[test]
    fn clone_drop_concurrent() {
        loom::model(|| {
            let original = eco_vec![10, 20, 30];
            let c1 = original.clone();
            let c2 = original.clone();

            let t1 = loom::thread::spawn(move || {
                drop(c1);
            });

            let t2 = loom::thread::spawn(move || {
                let _ = c2.as_slice();
                drop(c2);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            assert_eq!(original, [10, 20, 30]);
        });
    }

    // Exercises the unique-owner drop fast path: a refs==2 allocation with NO
    // surviving owner is dropped concurrently by two threads. Whichever thread
    // decrements first returns via the slow path (fetch_sub 2->1 returns 2);
    // the other then observes refs==1 via the Acquire-load fast path and must
    // deallocate exactly once. Loom explores all interleavings, catching any
    // double-free or missing happens-before in the new branch.
    #[test]
    fn concurrent_drop_no_survivor() {
        loom::model(|| {
            let v = eco_vec![1u64, 2, 3];
            let c = v.clone(); // refs == 2; both owners dropped below, none survive

            let t1 = loom::thread::spawn(move || {
                drop(v);
            });
            let t2 = loom::thread::spawn(move || {
                let _ = c.as_slice();
                drop(c);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    // Same fast-path race for a heap-backed EcoString (EcoVec<u8>).
    #[test]
    fn concurrent_string_drop_no_survivor() {
        loom::model(|| {
            let s = turbocow::EcoString::from("hello world, this is a heap string!!");
            let c = s.clone(); // refs == 2

            let t1 = loom::thread::spawn(move || {
                drop(s);
            });
            let t2 = loom::thread::spawn(move || {
                let _ = c.as_str();
                drop(c);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn clone_mutate_concurrent() {
        loom::model(|| {
            let shared = eco_vec![1u8, 2, 3];
            let c1 = shared.clone();
            let c2 = shared.clone();

            let t1 = loom::thread::spawn(move || {
                let mut v = c1;
                v.push(4);
                assert!(v.len() == 4);
            });

            let t2 = loom::thread::spawn(move || {
                let mut v = c2;
                v.push(5);
                assert!(v.len() == 4);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            assert_eq!(shared, [1, 2, 3]);
        });
    }

    #[test]
    fn string_clone_mutate_concurrent() {
        loom::model(|| {
            let s = turbocow::EcoString::from("hello world, this is a heap string!!");
            let c1 = s.clone();
            let c2 = s.clone();

            let t1 = loom::thread::spawn(move || {
                let mut v = c1;
                v.push('!');
                assert!(v.len() > 35);
            });

            let t2 = loom::thread::spawn(move || {
                let mut v = c2;
                v.push_str(" extra");
                assert!(v.len() > 35);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            assert_eq!(s.as_str(), "hello world, this is a heap string!!");
        });
    }
}
