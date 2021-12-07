use crate::value::*;
use crate::value::func::*;
use crate::eval::{Context};
use crate::world::{World};
use crate::mm::{Heap, GCAllocationStruct};
use std::fmt::Debug;
use std::hash::Hash;
use once_cell::sync::Lazy;

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
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&INTEGER_TYPEINFO as *const TypeInfo)
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
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&REAL_TYPEINFO as *const TypeInfo)
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
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&NUMBER_TYPEINFO as *const TypeInfo)
    }
}

impl Number {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&NUMBER_TYPEINFO, other_typeinfo)
    }
}

enum Num {
    Int(i64),
    Real(f64),
}

fn number_to(v: &NBox<Value>) -> Num {
    if let Some(integer) = v.as_ref().try_cast::<Integer>() {
        Num::Int(integer.num)
    } else {
        let real = v.as_ref().try_cast::<Real>().unwrap();
        Num::Real(real.num)
    }
}

fn func_add(args: &[NBox<Value>], ctx: &mut Context) -> NBox<Value> {
    let v = &args[0];

    let (mut int,mut real) = match number_to(v) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    let rest = &args[1];
    //TODO GC Capture: rest
    let rest = rest.duplicate().into_nbox::<list::List>().unwrap();

    for v in rest.iter() {
        match (number_to(v), int, real) {
            (Num::Int(num), Some(acc), None) => {
                int = Some(acc + num);
            }
            (Num::Real(num), Some(acc), None) => {
                int = None;
                real = Some(acc as f64 + num);
            }
            (Num::Int(num), None, Some(acc)) => {
                real = Some(acc + num as f64);
            }
            (Num::Real(num), None, Some(acc)) => {
                real = Some(acc + num);
            }
            _ => unreachable!(),
        }
    }

    if int.is_some() {
        number::Integer::alloc(&mut ctx.heap, int.unwrap()).into_nboxvalue()
    } else {
        number::Real::alloc(&mut ctx.heap, real.unwrap()).into_nboxvalue()
    }
}

fn func_abs(args: &[NBox<Value>], ctx: &mut Context) -> NBox<Value> {
    let v = &args[0];

    match number_to(v) {
        Num::Int(num) => number::Integer::alloc(&mut ctx.heap, num.abs()).into_nboxvalue(),
        Num::Real(num) => number::Real::alloc(&mut ctx.heap, num.abs()).into_nboxvalue(),
    }
}

static FUNC_ADD: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new(&[
            Param::new("num", ParamKind::Require, number::Number::typeinfo()),
            Param::new("rest", ParamKind::Rest, number::Number::typeinfo()),
            ],
            func_add)
    )
});

static FUNC_ABS: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new(&[
            Param::new("num", ParamKind::Require, number::Number::typeinfo()),
            ],
            func_abs)
    )
});


pub fn register_global(world: &mut World) {
    world.set("+", NBox::new(&FUNC_ADD.value as *const Func as *mut Func).into_nboxvalue());
    world.set("abs", NBox::new(&FUNC_ABS.value as *const Func as *mut Func).into_nboxvalue());
}
