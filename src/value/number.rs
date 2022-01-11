use crate::ptr::*;
use crate::value::*;
use crate::value::func::*;
use crate::object::Object;
use crate::object::mm::{GCAllocationStruct, get_typeinfo};
use crate::vm;
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
    std::mem::size_of::<Integer>(),
    None,
    Integer::eq,
    Integer::clone_inner,
    Display::fmt,
    Integer::is_type,
    None,
    Some(Integer::is_comparable),
    None,
    None,
);

impl NaviType for Integer {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&INTEGER_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self> {
        Self::alloc(self.num, allocator)
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

    pub fn alloc<A: Allocator>(num: i64, allocator : &mut A) -> Ref<Integer> {
        let ptr = allocator.alloc::<Integer>();

        unsafe {
            std::ptr::write(ptr.as_ptr(), Integer { num: num });
        }

        ptr.into_ref()
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
    std::mem::size_of::<Real>(),
    None,
    Real::eq,
    Real::clone_inner,
    Display::fmt,
    Real::is_type,
    None,
    Some(Real::is_comparable),
    None,
    None,
);

impl NaviType for Real {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&REAL_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self> {
        Self::alloc(self.num, allocator)
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

    pub fn alloc<A: Allocator>(num: f64, allocator : &mut A) -> Ref<Real> {
        let ptr = allocator.alloc::<Real>();

        unsafe {
            std::ptr::write(ptr.as_ptr(), Real { num: num });
        }

        ptr.into_ref()
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
    0, None,
    Number::eq,
    Number::clone_inner,
    Display::fmt,
    Number::is_type,
    None,
    None,
    None,
    None,
);

impl NaviType for Number {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&NUMBER_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> Ref<Self> {
        //Number型のインスタンスは存在しないため、cloneが呼ばれることはない。
        unreachable!()
    }
}

impl Number {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&NUMBER_TYPEINFO, other_typeinfo)
    }
}

impl Display for Number {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}

enum Num {
    Int(i64),
    Real(f64),
}

fn number_to(v: &Any) -> Num {
    if let Some(integer) = v.try_cast::<Integer>() {
        Num::Int(integer.num)
    } else {
        let real = v.try_cast::<Real>().unwrap();
        Num::Real(real.num)
    }
}

fn func_add(obj: &mut Object) -> Ref<Any> {
    let v = vm::refer_arg::<Any>(0, obj);

    let (mut int,mut real) = match number_to(&v.as_ref()) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    let rest = vm::refer_arg::<list::List>(1, obj);

    let iter =  unsafe { rest.as_ref().iter_gcunsafe() };
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
        number::Integer::alloc(int.unwrap(), obj).into_value()
    } else {
        number::Real::alloc(real.unwrap(), obj).into_value()
    }
}

fn func_abs(obj: &mut Object) -> Ref<Any> {
    let v = vm::refer_arg(0, obj);

    match number_to(v.as_ref()) {
        Num::Int(num) => number::Integer::alloc(num.abs(), obj).into_value(),
        Num::Real(num) => number::Real::alloc(num.abs(), obj).into_value(),
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

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("+", &Ref::new(&FUNC_ADD.value));
    obj.define_global_value("abs", &Ref::new(&FUNC_ABS.value));
}
