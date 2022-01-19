use crate::err::*;
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
    Some(Integer::is_type),
    None,
    Some(Integer::is_comparable),
    None,
    None,
);

impl NaviType for Integer {
    fn typeinfo() -> &'static TypeInfo {
        &INTEGER_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        Self::alloc(self.num, allocator)
    }
}

impl Integer {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
        || &NUMBER_TYPEINFO == other_typeinfo
    }

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
    }

    pub fn alloc<A: Allocator>(num: i64, allocator : &mut A) -> NResult<Integer, OutOfMemory> {
        let ptr = allocator.alloc::<Integer>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Integer { num: num });
        }

        Ok(ptr.into_ref())
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
        if Real::typeinfo() == other_typeinfo {
            let other = unsafe { std::mem::transmute::<&Integer, &Real>(other) };
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
    Some(Real::is_type),
    None,
    Some(Real::is_comparable),
    None,
    None,
);

impl NaviType for Real {
    fn typeinfo() -> &'static TypeInfo {
        &REAL_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        Self::alloc(self.num, allocator)
    }
}

impl Real {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &REAL_TYPEINFO == other_typeinfo
        || &NUMBER_TYPEINFO == other_typeinfo
    }

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        &REAL_TYPEINFO == other_typeinfo
        || &INTEGER_TYPEINFO == other_typeinfo
    }

    pub fn alloc<A: Allocator>(num: f64, allocator : &mut A) -> NResult<Real, OutOfMemory> {
        let ptr = allocator.alloc::<Real>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Real { num: num });
        }

        Ok(ptr.into_ref())
    }

}

impl PartialEq for Real {

    //RealはInteger型とも比較可能なため、other変数にはIntegerの参照が来る可能性がある
    fn eq(&self, other: &Self) -> bool {
        //otherはReal型か？
        let other_typeinfo = get_typeinfo(other);
        if Real::typeinfo() == other_typeinfo {
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
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for Number {
    fn typeinfo() -> &'static TypeInfo {
        &NUMBER_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //Number型のインスタンスは存在しないため、cloneが呼ばれることはない。
        unreachable!()
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

fn func_add(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Any>(0, obj);

    let (mut int,mut real) = match number_to(&v.as_ref()) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    for index in 0 .. num_rest {
        let v = vm::refer_rest_arg::<Any>(1, index, obj);
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
        let num = number::Integer::alloc(int.unwrap(), obj)?;
        Ok(num.into_value())
    } else {
        let num = number::Real::alloc(real.unwrap(), obj)?;
        Ok(num.into_value())
    }
}

fn func_sub(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Any>(0, obj);

    let (mut int,mut real) = match number_to(&v.as_ref()) {
        Num::Int(num) => (Some(num), None),
        Num::Real(num) => (None, Some(num)),
    };

    if num_rest == 0 {
        if let Some(num) = int {
            int = Some(-num);
        } else {
            real = Some(- real.unwrap());
        }

    } else {
        for index in 0 .. num_rest {
            let v = vm::refer_rest_arg::<Any>(1, index, obj);
            match (number_to(&v.as_ref()), int, real) {
                (Num::Int(num), Some(acc), None) => {
                    int = Some(acc - num);
                }
                (Num::Real(num), Some(acc), None) => {
                    int = None;
                    real = Some(acc as f64 - num);
                }
                (Num::Int(num), None, Some(acc)) => {
                    real = Some(acc - num as f64);
                }
                (Num::Real(num), None, Some(acc)) => {
                    real = Some(acc - num);
                }
                _ => unreachable!(),
            }
        }
    }

    if int.is_some() {
        let num = number::Integer::alloc(int.unwrap(), obj)?;
        Ok(num.into_value())
    } else {
        let num = number::Real::alloc(real.unwrap(), obj)?;
        Ok(num.into_value())
    }
}

fn func_abs(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg(0, obj);

    match number_to(v.as_ref()) {
        Num::Int(num) => {
            let num = number::Integer::alloc(num.abs(), obj)?;
            Ok(num.into_value())
        }
        Num::Real(num) => {
            let num = number::Real::alloc(num.abs(), obj)?;
            Ok(num.into_value())
        }
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

static FUNC_SUB: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("-",
            &[
            Param::new("num", ParamKind::Require, number::Number::typeinfo()),
            Param::new("rest", ParamKind::Rest, number::Number::typeinfo()),
            ],
            func_sub)
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
    obj.define_global_value("-", &Ref::new(&FUNC_SUB.value));
    obj.define_global_value("abs", &Ref::new(&FUNC_ABS.value));
}
