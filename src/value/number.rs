use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Integer {
    num : i64,
}

static INTEGER_TYPEINFO : TypeInfo = new_typeinfo!(
    Integer,
    "Integer",
    Integer::eq,
    Integer::fmt,
    Integer::is_type,
);

impl NaviType for Integer {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&INTEGER_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }
}

impl Integer {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&INTEGER_TYPEINFO, other_typeinfo)
        || std::ptr::eq(&REAL_TYPEINFO, other_typeinfo)
        || std::ptr::eq(&NUMBER_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<'ti>(heap : &'ti mut Heap, num: i64) -> NBox<Integer> {
        let mut nbox = heap.alloc::<Integer>();
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

//
// Real Number
//
#[derive(Debug, PartialEq, PartialOrd)]
pub struct Real {
    pub num : f64,
}

static REAL_TYPEINFO : TypeInfo = new_typeinfo!(
    Real,
    "Real",
    Real::eq,
    Real::fmt,
    Real::is_type,
);

impl NaviType for Real {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&REAL_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }
}

impl Real {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&REAL_TYPEINFO, other_typeinfo)
        || std::ptr::eq(&NUMBER_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(heap : &mut Heap, num: f64) -> NBox<Real> {
        let mut nbox = heap.alloc::<Real>();
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

#[derive(Debug, PartialEq)]
pub struct Number {
}

//Number型は抽象型なので実際にアロケーションされることはない。
//NUMBER_TYPEINFOは型チェックのためだけに使用される。
static NUMBER_TYPEINFO : TypeInfo = new_typeinfo!(
    Number,
    "Number",
    Number::eq,
    Number::fmt,
    Number::is_type,
);

impl NaviType for Number {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&NUMBER_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }
}

impl Number {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&NUMBER_TYPEINFO, other_typeinfo)
    }
}