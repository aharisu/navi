use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Integer {
    num : i64,
}

static INTEGER_TYPEINFO : TypeInfo<Integer> = TypeInfo::<Integer> {
    name: "Integer",
    eq_func: Integer::eq,
    print_func: Integer::fmt,
};

impl Integer {

    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Integer> {
        &INTEGER_TYPEINFO
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
};

impl Float {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Float> {
        &FLOAT_TYPEINFO
    }

    pub fn alloc(heap : &mut Heap, num: f64) -> NBox<Float> {
        let mut nbox = heap.alloc(Self::typeinfo());
        let obj = nbox.as_mut_ref();
        obj.num = num;

        nbox
    }
}

impl NaviType for Float { }