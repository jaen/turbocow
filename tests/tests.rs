// Test with `cargo +nightly miri test` to check sanity!

#![allow(clippy::redundant_clone)]
#![allow(clippy::disallowed_names)]

use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering::*};

use turbocow::{EcoVec, eco_vec};

fn v<T>(value: T) -> Box<T> {
    Box::new(value)
}

#[test]
fn test_mem_size() {
    let word = mem::size_of::<usize>();
    assert_eq!(mem::size_of::<EcoVec<u8>>(), 2 * word);
    assert_eq!(mem::size_of::<Option<EcoVec<u8>>>(), 2 * word);
}

#[test]
fn test_vec_macro() {
    assert_eq!(eco_vec![Box::new(1); 3], vec![v(1); 3]);
}

#[test]
fn test_vec_construction() {
    assert_eq!(EcoVec::<()>::default(), &[]);
    assert_eq!(EcoVec::from(vec![(); 100]), vec![(); 100]);
}

#[test]
fn test_from_vec_rc() {
    use std::rc::Rc;

    let x = Rc::new(());
    let v = vec![x.clone()];
    assert_eq!(Rc::strong_count(&x), 2);

    let vec = EcoVec::from(v);
    assert_eq!(Rc::strong_count(&x), 2, "Rc count should not change");
    assert_eq!(vec.len(), 1);
    assert!(Rc::ptr_eq(&x, &vec[0]));

    std::mem::drop(vec);
    assert_eq!(Rc::strong_count(&x), 1);
}

#[test]
fn test_vec_with_capacity() {
    let mut vec = EcoVec::with_capacity(3);
    assert_eq!(vec.capacity(), 3);
    let ptr = vec.as_ptr();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    assert_eq!(ptr, vec.as_ptr());
    vec.push(4);
    assert_eq!(vec, [1, 2, 3, 4]);
}

#[test]
#[should_panic(expected = "capacity overflow")]
fn test_vec_with_capacity_fail() {
    EcoVec::<u8>::with_capacity(usize::MAX);
}

#[test]
fn test_vec_empty() {
    let mut first = EcoVec::with_capacity(3);
    assert!(first.is_empty());
    assert_eq!(first.len(), 0);
    first.push("hi".to_string());
    assert!(!first.is_empty());
    assert_eq!(first.len(), 1);
    let second = first.clone();
    first.clear();
    assert!(first.is_empty());
    first.clear();
    assert!(first.is_empty());
    assert_eq!(second.len(), 1);
    assert_eq!(second, ["hi".to_string()]);
}

#[test]
fn test_vec_make_mut() {
    let mut first = eco_vec![4, -3, 11, 6, 10];
    let ptr = first.as_ptr();
    first.make_mut()[1] -= 1;
    assert_eq!(ptr, first.as_ptr());
    let second = first.clone();
    first.make_mut().sort();
    assert_eq!(first, [-4, 4, 6, 10, 11]);
    assert_eq!(second, [4, -4, 11, 6, 10]);
}

#[test]
fn test_vec_push() {
    let mut first = EcoVec::new();
    first.push(1);
    first.push(2);
    first.push(3);
    assert_eq!(first.len(), 3);
    let mut second = first.clone();
    let third = second.clone();
    let _ = third.clone();
    second.push(4);
    assert_eq!(second.len(), 4);
    assert_eq!(first, [1, 2, 3]);
    assert_eq!(second, [1, 2, 3, 4]);
    assert_eq!(third, [1, 2, 3]);
    assert_ne!(first.as_ptr(), second.as_ptr());
    assert_eq!(first.as_ptr(), third.as_ptr());
}

#[test]
fn test_vec_pop() {
    let mut first = EcoVec::new();
    assert_eq!(first.pop(), None);
    first.push(v("a"));
    let ptr = first.as_ptr();
    assert_eq!(first.pop(), Some(v("a")));
    first.push(v("b"));
    assert_eq!(ptr, first.as_ptr());
    let second = first.clone();
    assert_eq!(first[0], v("b"));
    assert_eq!(ptr, first.as_ptr());
    assert_eq!(first.pop(), Some(v("b")));
    assert_eq!(first, []);
    assert_eq!(second, [v("b")]);
}

#[test]
fn test_vec_insert() {
    let mut first = EcoVec::new();
    first.insert(0, "okay");
    let ptr = first.as_ptr();
    first.insert(0, "reverse");
    let mut second = first.clone();
    first.insert(2, "a");
    first.insert(1, "b");
    second.insert(2, "last");
    assert_eq!(first, ["reverse", "b", "okay", "a"]);
    assert_eq!(second, ["reverse", "okay", "last"]);
    assert_ne!(ptr, first.as_ptr());
    assert_eq!(ptr, second.as_ptr());
}

#[test]
#[should_panic(expected = "index is out bounds (index: 4, len: 3)")]
fn test_vec_insert_fail() {
    EcoVec::from([1, 2, 3]).insert(4, 0);
}

#[test]
fn test_vec_remove() {
    let mut first = EcoVec::with_capacity(4);
    let ptr = first.as_ptr();
    first.extend_from_slice(&[v(2), v(4), v(1)]);
    let second = first.clone();
    assert_eq!(first.remove(1), v(4));
    assert_eq!(first, [v(2), v(1)]);
    assert_eq!(second, [v(2), v(4), v(1)]);
    assert_ne!(ptr, first.as_ptr());
    assert_eq!(ptr, second.as_ptr());
}

#[test]
#[should_panic(expected = "index is out bounds (index: 4, len: 3)")]
fn test_vec_remove_fail() {
    EcoVec::from([1, 2, 3]).remove(4);
}

#[test]
fn test_vec_truncate() {
    let mut vec = eco_vec!["ok"; 10];
    vec.truncate(13);
    vec.truncate(3);
    assert_eq!(vec, ["ok"; 3]);

    let mut cloned = vec.clone();
    cloned.truncate(2);
    assert_eq!(cloned, ["ok"; 2]);
}

#[test]
fn test_vec_extend() {
    let mut vec = EcoVec::new();
    vec.extend_from_slice(&[]);
    vec.extend_from_slice(&[2, 3, 4]);
    assert_eq!(vec, [2, 3, 4]);
}

// Ported from ecow 0.3.0's `tests/tests.rs::test_vec_splice` (imports
// retargeted to turbocow).
#[test]
fn test_vec_splice() {
    let mut vec = eco_vec!["a"; 6];
    // Inserted iterator is smaller.
    vec.splice(2..4, ["b"; 1]);
    assert_eq!(vec, ["a", "a", "b", "a", "a"]);

    // Inserted iterator is larger.
    let mut cloned = vec.clone();
    cloned.splice(3..4, ["c"; 3]);
    assert_eq!(cloned, ["a", "a", "b", "c", "c", "c", "a"]);

    // Inserted iterator is the same size.
    cloned.splice(2..6, ["d"; 4]);
    assert_eq!(cloned, ["a", "a", "d", "d", "d", "d", "a"]);

    // Insert an iterator that doesn't have exact size hints.
    cloned.splice(1..6, ["e"; 3].into_iter().filter(|_| true));
    assert_eq!(cloned, ["a", "e", "e", "e", "a"]);
}

#[test]
fn test_vec_into_iter() {
    let first = eco_vec![v(2), v(4), v(5)];
    let mut second = first.clone();
    assert_eq!(first.clone().into_iter().count(), 3);
    assert_eq!(second.clone().into_iter().rev().collect::<Vec<_>>(), [v(5), v(4), v(2)]);
    second.clear();
    assert_eq!(second.into_iter().collect::<Vec<_>>(), []);
    assert_eq!(first.clone().into_iter().collect::<Vec<_>>(), [v(2), v(4), v(5)]);
    let mut iter = first.into_iter();
    assert_eq!(iter.len(), 3);
    assert_eq!(iter.next(), Some(v(2)));
    assert_eq!(iter.next_back(), Some(v(5)));
    assert_eq!(iter.as_slice(), [v(4)]);
    drop(iter);
}

#[test]
fn test_vec_zst() {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone)]
    struct Potato;
    impl Drop for Potato {
        fn drop(&mut self) {
            COUNTER.fetch_add(1, SeqCst);
        }
    }

    let mut vec = EcoVec::new();
    for _ in 0..1000 {
        vec.push(Potato);
    }
    assert_eq!(vec.len(), 1000);
    drop(vec);

    assert_eq!(COUNTER.load(SeqCst), 1000);
}

#[test]
fn test_vec_huge_alignment() {
    #[derive(Debug, PartialEq, Clone)]
    #[repr(align(128))]
    struct B(&'static str);
    let mut vec: EcoVec<B> =
        "hello, world! what's going on?".split_whitespace().map(B).collect();

    assert_eq!(vec.len(), 5);
    assert_eq!(vec.capacity(), 8);
    assert_eq!(vec, [B("hello,"), B("world!"), B("what's"), B("going"), B("on?")]);
    assert_eq!(vec.pop(), Some(B("on?")));
    assert_eq!(vec.len(), 4);
    assert_eq!(vec.last(), Some(&B("going")));
    assert_eq!(vec.remove(1), B("world!"));
    assert_eq!(vec.len(), 3);
    assert_eq!(vec, [B("hello,"), B("what's"), B("going")]);
    assert_eq!(vec[1], B("what's"));
    vec.push(B("where?"));
    vec.insert(1, B("wonder!"));
    assert_eq!(vec, [B("hello,"), B("wonder!"), B("what's"), B("going"), B("where?")]);
    vec.retain(|b| b.0.starts_with('w'));
    assert_eq!(vec, [B("wonder!"), B("what's"), B("where?")]);
    vec.truncate(1);
    assert_eq!(vec.last(), vec.first());

    let empty: EcoVec<B> = EcoVec::new();
    assert_eq!(empty, &[]);
}

#[test]
#[should_panic(expected = "dropped the hot potato!")]
#[allow(unused_must_use)]
fn test_vec_drop_panic() {
    #[derive(Clone)]
    struct Potato;
    impl Drop for Potato {
        fn drop(&mut self) {
            panic!("dropped the hot potato!");
        }
    }

    eco_vec![Potato];
}

#[test]
#[should_panic(expected = "dropped the hot potato!")]
fn test_vec_clear_drop_panic() {
    #[derive(Clone)]
    struct Potato;
    impl Drop for Potato {
        fn drop(&mut self) {
            panic!("dropped the hot potato!");
        }
    }

    let mut vec = eco_vec![Potato];
    vec.clear();
}

#[test]
fn test_array_from_vec() {
    let array = [String::from("foo"), String::from("bar")];
    let a = EcoVec::from(array.clone());
    let b = a.clone();
    let c: [String; 2] = a.try_into().unwrap();
    assert_eq!(b, c);
    let d = b.clone();
    assert_eq!(c, array);
    assert_eq!(d, array);
    drop(b);
    assert_eq!(c, array);
    drop(d);
    assert_eq!(c, array);

    assert_eq!(<[String; 0]>::try_from(EcoVec::new()).unwrap(), <[String; 0]>::default());
}

#[test]
fn eco_vec_shrink_to_fit() {
    use turbocow::EcoVec;
    let mut v: EcoVec<i32> = EcoVec::with_capacity(100);
    v.push(1);
    v.push(2);
    v.push(3);
    assert!(v.capacity() >= 100);
    v.shrink_to_fit();
    assert_eq!(v.len(), 3);
    assert!(v.capacity() >= 3);
    // After shrink, capacity should be substantially reduced.
    assert!(v.capacity() < 50);
    assert_eq!(&v[..], &[1, 2, 3]);
}

#[test]
fn eco_vec_shrink_to_fit_shared_is_noop() {
    use turbocow::EcoVec;
    let mut v: EcoVec<i32> = EcoVec::with_capacity(100);
    v.push(1);
    let _snapshot = v.clone(); // now shared
    let cap_before = v.capacity();
    v.shrink_to_fit(); // shared: no-op
    assert_eq!(v.capacity(), cap_before); // unchanged
    assert_eq!(v.len(), 1);
}

#[test]
fn ecovec_extend_capacity_scenarios() {
    // Extend into empty vec (no reserve hint path: hint is the iterator's
    // lower-bound size hint, which for Vec::into_iter is exact).
    let mut v: EcoVec<u32> = EcoVec::new();
    v.extend(vec![1, 2, 3]);
    assert_eq!(v.as_slice(), &[1, 2, 3]);

    // Extend into vec with spare capacity (push_unchecked path).
    let mut v: EcoVec<u32> = EcoVec::with_capacity(10);
    v.push(0);
    v.extend(vec![1, 2, 3]);
    assert_eq!(v.as_slice(), &[0, 1, 2, 3]);

    // Extend beyond capacity (triggers growth + fallback push path).
    let mut v: EcoVec<u32> = eco_vec![1, 2, 3];
    v.extend(vec![4, 5, 6, 7, 8]);
    assert_eq!(v.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);

    // Extend with empty iterator (no-op; also exercises the hint == 0 path
    // where reserve is skipped entirely).
    let mut v: EcoVec<u32> = eco_vec![1, 2, 3];
    v.extend(std::iter::empty());
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}
