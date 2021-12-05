use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Integer {
    num : i64,
}

static INTEGER_TYPEINFO : TypeInfo<Integer> = TypeInfo::<Integer> {
    name: "Integer",
    eq_func: Integer::eq,
    print_func: Integer::fmt,
    is_type_func: Integer::is_type,
};

impl Integer {

    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Integer> {
        &INTEGER_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
        || std::ptr::eq(Real::typeinfo().cast(), other_typeinfo)
        || std::ptr::eq(Number::typeinfo().cast(), other_typeinfo)
    }

    pub fn alloc<'ti>(heap : &'ti mut Heap, num: i64) -> NBox<Integer> {
        let mut nbox = heap.alloc(Self::typeinfo());
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

impl NaviType for Integer { }

//
// Real Number
//
#[derive(Debug, PartialEq, PartialOrd)]
pub struct Real {
    pub num : f64,
}

static REAL_TYPEINFO : TypeInfo<Real> = TypeInfo::<Real> {
    name: "Real",
    eq_func: Real::eq,
    print_func: Real::fmt,
    is_type_func: Real::is_type,
};

impl Real {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Real> {
        &REAL_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
        || std::ptr::eq(Number::typeinfo().cast(), other_typeinfo)
    }

    pub fn alloc(heap : &mut Heap, num: f64) -> NBox<Real> {
        let mut nbox = heap.alloc(Self::typeinfo());
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

impl NaviType for Real { }

#[derive(Debug, PartialEq)]
pub struct Number {
}

impl NaviType for Number { }

//Number型は抽象型なので実際にアロケーションされることはない。
//NUMBER_TYPEINFOは型チェックのためだけに使用される。
static NUMBER_TYPEINFO : TypeInfo<Number> = TypeInfo::<Number> {
    name: "Number",
    eq_func: Number::eq,
    print_func: Number::fmt,
    is_type_func: Number::is_type,
};

impl Number {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Self> {
        &NUMBER_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
    }
}