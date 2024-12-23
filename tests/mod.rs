extern crate alloc;
use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString},
};

use smallstr::SmallString;

#[test]
fn test_drain() {
    let mut s: SmallString<2> = SmallString::new();

    s.push('a');
    assert_eq!(s.drain().collect::<String>(), "a");
    assert!(s.is_empty());

    // spilling the vec
    s.push('x');
    s.push('y');
    s.push('z');

    assert_eq!(s.drain().collect::<String>(), "xyz");
    assert!(s.is_empty());
}

#[test]
fn test_drain_rev() {
    let mut s: SmallString<2> = SmallString::new();

    s.push('a');
    assert_eq!(s.drain().rev().collect::<String>(), "a");
    assert!(s.is_empty());

    // spilling the vec
    s.push('x');
    s.push('y');
    s.push('z');

    assert_eq!(s.drain().rev().collect::<String>(), "zyx");
    assert!(s.is_empty());
}

#[test]
fn test_eq() {
    let s: SmallString<4> = SmallString::from("foo");

    assert_eq!(s, *"foo");
    assert_eq!(s, "foo");
    assert_eq!(s, "foo".to_owned());
    assert_eq!(s, Cow::Borrowed("foo"));
}

#[cfg(feature = "ffi")]
#[test]
fn test_eq_os_str() {
    use std::ffi::OsStr;

    let s: SmallString<4> = SmallString::from("foo");
    let os_s: &OsStr = "foo".as_ref();

    assert_eq!(s, os_s);
    assert_eq!(s, *os_s);
    assert_eq!(s, os_s.to_owned());
    assert_eq!(s, Cow::Borrowed(os_s));
}

#[test]
fn test_from_buf() {
    let s: SmallString<2> = SmallString::from_buf([206, 177]).unwrap();
    assert_eq!(s, "α");

    assert!(SmallString::<2>::from_buf([206, 0]).is_err());
}

#[test]
fn test_insert() {
    let mut s: SmallString<8> = SmallString::from("abc");

    s.insert(1, 'x');
    assert_eq!(s, "axbc");

    s.insert(3, 'α');
    assert_eq!(s, "axbαc");

    s.insert_str(0, "foo");
    assert_eq!(s, "fooaxbαc");
}

#[test]
#[should_panic]
fn test_insert_panic() {
    let mut s: SmallString<8> = SmallString::from("αβγ");

    s.insert(1, 'x');
}

#[test]
fn test_into_string() {
    let s: SmallString<2> = SmallString::from("foo");
    assert_eq!(s.into_string(), "foo");

    let s: SmallString<8> = SmallString::from("foo");
    assert_eq!(s.into_string(), "foo");
}

#[test]
fn test_to_string() {
    let s: SmallString<2> = SmallString::from("foo");
    assert_eq!(s.to_string(), "foo");

    let s: SmallString<8> = SmallString::from("foo");
    assert_eq!(s.to_string(), "foo");
}

#[test]
fn test_pop() {
    let mut s: SmallString<8> = SmallString::from("αβγ");

    assert_eq!(s.pop(), Some('γ'));
    assert_eq!(s.pop(), Some('β'));
    assert_eq!(s.pop(), Some('α'));
    assert_eq!(s.pop(), None);
}

#[test]
fn test_remove() {
    let mut s: SmallString<8> = SmallString::from("αβγ");

    assert_eq!(s.remove(2), 'β');
    assert_eq!(s, "αγ");

    assert_eq!(s.remove(0), 'α');
    assert_eq!(s, "γ");

    assert_eq!(s.remove(0), 'γ');
    assert_eq!(s, "");
}

#[test]
#[should_panic]
fn test_remove_panic_0() {
    let mut s: SmallString<8> = SmallString::from("foo");

    // Attempt to remove at the end
    s.remove(3);
}

#[test]
#[should_panic]
fn test_remove_panic_1() {
    let mut s: SmallString<8> = SmallString::from("αβγ");

    // Attempt to remove mid-character
    s.remove(1);
}

#[test]
fn test_retain() {
    let mut s: SmallString<8> = SmallString::from("α_β_γ");

    s.retain(|_| true);
    assert_eq!(s, "α_β_γ");

    s.retain(|c| c != '_');
    assert_eq!(s, "αβγ");

    s.retain(|c| c != 'β');
    assert_eq!(s, "αγ");

    s.retain(|c| c == 'α');
    assert_eq!(s, "α");

    s.retain(|_| false);
    assert_eq!(s, "");
}

#[test]
fn test_truncate() {
    let mut s: SmallString<2> = SmallString::from("foobar");

    s.truncate(6);
    assert_eq!(s, "foobar");

    s.truncate(3);
    assert_eq!(s, "foo");
}

#[test]
#[should_panic]
fn test_truncate_panic() {
    let mut s: SmallString<2> = SmallString::from("α");

    s.truncate(1);
}

#[test]
fn test_write() {
    use core::fmt::Write;

    let mut s: SmallString<8> = SmallString::from("foo");

    write!(s, "bar").unwrap();

    assert_eq!(s, "foobar");
}

// #[cfg(feature = "serde")]
// #[test]
// fn test_serde() {
//     use bincode::{config, decode_from_slice, encode_to_vec};
//
//     let mut small_str: SmallString<4> = SmallString::from("foo");
//     let config = config::standard();
//     let encoded = encode_to_vec(&small_str, config).unwrap();
//     let decoded = decode_from_slice(&encoded, config).unwrap();
//
//     assert_eq!(small_str, decoded);
//
//     // Spill the vec
//     small_str.push_str("bar");
//
//     // Check again after spilling.
//     let encoded = encode_to_vec(&small_str,config).unwrap();
//     let decoded = decode_from_slice(&encoded,config).unwrap();
//
//     assert_eq!(small_str, decoded);
// }
