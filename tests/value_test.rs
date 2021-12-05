extern crate navi;
use navi::value::*;

#[test]
fn is_type() {
    let mut heap = navi::mm::Heap::new(1024, "test");

    //int
    let v = number::Integer::alloc(&mut heap, 10).into_nboxvalue();
    assert!(v.as_ref().is_type(number::Integer::typeinfo()));
    assert!(v.as_ref().is_type(number::Float::typeinfo()));
    assert!(v.as_ref().is_type(number::Number::typeinfo()));

    //real
    let v = number::Float::alloc(&mut heap, 3.14).into_nboxvalue();
    assert!(!v.as_ref().is_type(number::Integer::typeinfo()));
    assert!(v.as_ref().is_type(number::Float::typeinfo()));
    assert!(v.as_ref().is_type(number::Number::typeinfo()));

    //nil
    let v = list::List::nil().into_nboxvalue();
    assert!(v.as_ref().is_type(list::List::typeinfo()));
    assert!(!v.as_ref().is_type(string::NString::typeinfo()));

    //list
    let item = number::Integer::alloc(&mut heap, 10).into_nboxvalue();
    let v = list::List::alloc(&mut heap, &item, v.into_nbox(list::List::typeinfo()).unwrap()).into_nboxvalue();
    assert!(v.as_ref().is_type(list::List::typeinfo()));
    assert!(!v.as_ref().is_type(string::NString::typeinfo()));

    heap.free();
}