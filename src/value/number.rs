use crate::ptr::*;
use crate::value::*;
use crate::value::func::*;
use crate::context::Context;
use crate::mm::{GCAllocationStruct};
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
    None,
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

    pub fn alloc(num: i64, ctx : &mut Context) -> FPtr<Integer> {
        let mut ptr = ctx.alloc::<Integer>();
        let obj = unsafe { ptr.as_mut() };
        obj.num = num;

        ptr.into_fptr()
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
    None,
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

    pub fn alloc(num: f64, ctx : &mut Context) -> FPtr<Real> {
        let mut ptr = ctx.alloc::<Real>();
        let obj = unsafe { ptr.as_mut() };
        obj.num = num;

        ptr.into_fptr()
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
    None,
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

fn number_to(v: &Value) -> Num {
    if let Some(integer) = v.try_cast::<Integer>() {
        Num::Int(integer.num)
    } else {
        let real = v.try_cast::<Real>().unwrap();
        Num::Real(real.num)
    }
}

fn func_add(args: &RPtr<array::Array>, ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);

    let (mut int,mut real) = match number_to(&v.as_ref()) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    let rest = args.as_ref().get(1);
    let rest = rest.try_cast::<list::List>().unwrap();

    let iter =  rest.as_ref().iter();
    for v in iter {
        match (number_to(&v.as_ref()), int, real) {
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
        number::Integer::alloc(int.unwrap(), ctx).into_value()
    } else {
        number::Real::alloc(real.unwrap(), ctx).into_value()
    }
}

fn func_eqv(args: &RPtr<array::Array>, _ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);

    let (int,real) = match number_to(&v.as_ref()) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    fn check(int: Option<i64>, real: Option<f64>, v: &Value) -> (Option<i64>, Option<f64>, bool) {
        match (number_to(v), int, real) {
            (Num::Int(num), Some(pred), None) => {
                (Some(num), None, num == pred)
            }
            (Num::Real(num), Some(pred), None) => {
                (None, Some(num), num == pred as f64)
            }
            (Num::Int(num), None, Some(pred)) => {
                (None, Some(num as f64), num as f64 == pred)
            }
            (Num::Real(num), None, Some(pred)) => {
                (None, Some(num), num == pred)
            }
            _ => unreachable!(),
        }
    }

    let v = args.as_ref().get(1);
    let (mut int, mut real, mut result) = check(int, real, v.as_ref());

    if result {
        let rest = args.as_ref().get(2);
        let rest = rest.try_cast::<list::List>().unwrap();

        let mut iter = rest.as_ref().iter();
        result = iter.all(|v| {
            let (i, r, result) = check(int, real, v.as_ref());
            int = i;
            real = r;
            result
        });
    }

    if result {
        bool::Bool::true_().into_fptr().into_value()
    } else {
        bool::Bool::false_().into_fptr().into_value()
    }
}

fn func_abs(args: &RPtr<array::Array>, ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);

    match number_to(v.as_ref()) {
        Num::Int(num) => number::Integer::alloc(num.abs(), ctx).into_value(),
        Num::Real(num) => number::Real::alloc(num.abs(), ctx).into_value(),
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

static FUNC_EQV: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new(&[
            Param::new("num1", ParamKind::Require, number::Number::typeinfo()),
            Param::new("num2", ParamKind::Require, number::Number::typeinfo()),
            Param::new("rest", ParamKind::Rest, number::Number::typeinfo()),
            ],
            func_eqv)
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


pub fn register_global(ctx: &mut Context) {
    ctx.define_value("+", &RPtr::new(&FUNC_ADD.value as *const Func as *mut Func).into_value());
    ctx.define_value("=", &RPtr::new(&FUNC_EQV.value as *const Func as *mut Func).into_value());
    ctx.define_value("abs", &RPtr::new(&FUNC_ABS.value as *const Func as *mut Func).into_value());
}
