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
        || std::ptr::eq(Float::typeinfo().cast(), other_typeinfo)
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
// Floating Number
//
#[derive(Debug, PartialEq, PartialOrd)]
pub struct Float {
    pub num : f64,
}

static FLOAT_TYPEINFO : TypeInfo<Float> = TypeInfo::<Float> {
    name: "Float",
    eq_func: Float::eq,
    print_func: Float::fmt,
    is_type_func: Float::is_type,
};

impl Float {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Float> {
        &FLOAT_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
    }

    pub fn alloc(heap : &mut Heap, num: f64) -> NBox<Float> {
        let mut nbox = heap.alloc(Self::typeinfo());
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

impl NaviType for Float { }