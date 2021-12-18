use crate::mm::get_typeinfo;
use crate::ptr::*;
use crate::value::*;
use crate::value::func::*;
use crate::context::Context;
use crate::mm::{GCAllocationStruct};
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use once_cell::sync::Lazy;

//TODO OrdとPartialOrdもRealとの比較のために独自実装が必要
#[derive(Ord, PartialOrd, Hash)]
pub struct Integer {
    num : i64,
}

static INTEGER_TYPEINFO : TypeInfo = new_typeinfo!(
    Integer,
    "Integer",
    Integer::eq,
    Display::fmt,
    Integer::is_type,
    Some(Integer::is_comparable),
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

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&INTEGER_TYPEINFO, other_typeinfo)
        || std::ptr::eq(&REAL_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(num: i64, ctx : &mut Context) -> FPtr<Integer> {
        let mut ptr = ctx.alloc::<Integer>();
        let obj = unsafe { ptr.as_mut() };
        obj.num = num;

        ptr.into_fptr()
    }

    pub fn get(&self) -> i64 {
        self.num
    }
}

impl Eq for Integer { }

impl PartialEq for Integer {
    //IntegerはReal型とも比較可能なため、other変数にはRealの参照が来る可能性がある
    fn eq(&self, other: &Self) -> bool {
        //otherはReal型か？
        let other_typeinfo = get_typeinfo(other);
        if std::ptr::eq(Real::typeinfo().as_ptr(), other_typeinfo.as_ptr()) {
            let other = unsafe {
                &*(other as *const Integer as *const Real)
            };
            self.num as f64 == other.num

        } else {
            //Integer 同士なら通常通りnumの比較を行う
            self.num == other.num
        }
    }
}

fn display(this: &Integer, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", this.num)
}

impl Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl Debug for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

//
// Real Number
//
//TODO OrdとPartialOrdもRealとの比較のために独自実装が必要
#[derive(PartialOrd)]
pub struct Real {
    pub num : f64,
}

static REAL_TYPEINFO : TypeInfo = new_typeinfo!(
    Real,
    "Real",
    Real::eq,
    Display::fmt,
    Real::is_type,
    Some(Real::is_comparable),
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

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&REAL_TYPEINFO, other_typeinfo)
        || std::ptr::eq(&INTEGER_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(num: f64, ctx : &mut Context) -> FPtr<Real> {
        let mut ptr = ctx.alloc::<Real>();
        let obj = unsafe { ptr.as_mut() };
        obj.num = num;

        ptr.into_fptr()
    }

}

impl PartialEq for Real {

    //RealはInteger型とも比較可能なため、other変数にはIntegerの参照が来る可能性がある
    fn eq(&self, other: &Self) -> bool {
        //otherはReal型か？
        let other_typeinfo = get_typeinfo(other);
        if std::ptr::eq(Real::typeinfo().as_ptr(), other_typeinfo.as_ptr()) {
            self.num == other.num

        } else {
            //OtherがIntegerなら、Integer型参照に無理やり戻してから比較
            let other = unsafe {
                &*(other as *const Real as *const Integer)
            };

            self.num == other.num as f64
        }
    }
}

fn display_real(this: &Real, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", this.num)
}

impl Display for Real {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_real(self, f)
    }
}

impl Debug for Real {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_real(self, f)
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
    Display::fmt,
    Number::is_type,
    None,
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

impl Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
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

fn func_abs(args: &RPtr<array::Array>, ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);

    match number_to(v.as_ref()) {
        Num::Int(num) => number::Integer::alloc(num.abs(), ctx).into_value(),
        Num::Real(num) => number::Real::alloc(num.abs(), ctx).into_value(),
    }
}

static FUNC_ADD: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("+",
            &[
            Param::new("num", ParamKind::Require, number::Number::typeinfo()),
            Param::new("rest", ParamKind::Rest, number::Number::typeinfo()),
            ],
            func_add)
    )
});

static FUNC_ABS: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("abs",
            &[
            Param::new("num", ParamKind::Require, number::Number::typeinfo()),
            ],
            func_abs)
    )
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("+", &RPtr::new(&FUNC_ADD.value as *const Func as *mut Func).into_value());
    ctx.define_value("abs", &RPtr::new(&FUNC_ABS.value as *const Func as *mut Func).into_value());
}
