//! Tests for `std::io::Write` implementation on `EcoVec<u8>`.

#[test]
fn write_all_appends_bytes() {
    let mut v = turbocow::EcoVec::<u8>::new();
    std::io::Write::write_all(&mut v, b"hi").unwrap();
    assert_eq!(v.as_slice(), b"hi");
}

#[test]
fn write_empty_slice() {
    let mut v = turbocow::EcoVec::<u8>::new();
    std::io::Write::write_all(&mut v, b"").unwrap();
    assert!(v.is_empty());
}

#[test]
fn write_multiple_times() {
    let mut v = turbocow::EcoVec::<u8>::new();
    std::io::Write::write_all(&mut v, b"foo").unwrap();
    std::io::Write::write_all(&mut v, b"bar").unwrap();
    assert_eq!(v.as_slice(), b"foobar");
}

#[test]
fn flush_is_noop() {
    let mut v = turbocow::EcoVec::<u8>::new();
    std::io::Write::flush(&mut v).unwrap();
    assert!(v.is_empty());
}
