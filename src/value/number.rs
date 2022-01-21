use crate::err::*;
use crate::ptr::*;
use crate::value::*;
use crate::value::func::*;
use crate::object::Object;
use crate::object::mm::GCAllocationStruct;
use crate::vm;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use once_cell::sync::Lazy;

static FIXNUM_TYPEINFO : TypeInfo = new_typeinfo!(
    Fixnum,
    "Fixnum",
    std::mem::size_of::<Fixnum>(),
    None,
    Fixnum::eq,
    Fixnum::clone_inner,
    Display::fmt,
    Some(Fixnum::is_type),
    None,
    Some(Fixnum::is_comparable),
    None,
    None,
);

pub struct Fixnum { }

impl NaviType for Fixnum {
    fn typeinfo() -> &'static TypeInfo {
        &FIXNUM_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //Fixnuml型の値は常にImmidiate Valueなのでそのまま返す
        Ok(Ref::new(self))
    }
}

impl Fixnum {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &FIXNUM_TYPEINFO == other_typeinfo
        || &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
        || &NUMBER_TYPEINFO == other_typeinfo
    }

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        &FIXNUM_TYPEINFO == other_typeinfo
        || &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
    }

    #[inline]
    pub fn get(&self) -> i64 {
        let v = ptr_to_usize(self);
        let num = v as i64;
        num >> FIXNUM_MASK_BITS
    }

}

impl Eq for Fixnum { }

impl PartialEq for Fixnum {
    fn eq(&self, other: &Self) -> bool {
        let other_typeinfo = get_typeinfo(other);
        if Fixnum::typeinfo() == other_typeinfo {
            std::ptr::eq(self, other)

        } else if Integer::typeinfo() == other_typeinfo {
            let other = unsafe { std::mem::transmute::<&Fixnum, &Integer>(other) };
            self.get() == other.num

        } else {
            //otherはReal型か？
            let other = unsafe { std::mem::transmute::<&Fixnum, &Real>(other) };
            self.get() as f64 == other.num
        }
    }
}

impl Display for Fixnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl Debug for Fixnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

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
        &FIXNUM_TYPEINFO == other_typeinfo
        || &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
        || &NUMBER_TYPEINFO == other_typeinfo
    }

    fn is_comparable(other_typeinfo: &TypeInfo) -> bool {
        &FIXNUM_TYPEINFO == other_typeinfo
        || &INTEGER_TYPEINFO == other_typeinfo
        || &REAL_TYPEINFO == other_typeinfo
    }

    fn alloc<A: Allocator>(num: i64, allocator : &mut A) -> NResult<Integer, OutOfMemory> {
        let ptr = allocator.alloc::<Integer>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Integer { num: num });
        }

        Ok(ptr.into_ref())
    }

    #[inline]
    pub fn get(&self) -> i64 {
        if Fixnum::typeinfo() == get_typeinfo(self) {
            let this = unsafe { std::mem::transmute::<&Integer, &Fixnum>(self) };
            this.get()
        } else {
            self.num
        }
    }
}

impl Eq for Integer { }

impl PartialEq for Integer {
    //IntegerはReal型とも比較可能なため、other変数にはRealの参照が来る可能性がある
    fn eq(&self, other: &Self) -> bool {
        let other_typeinfo = get_typeinfo(other);
        if Fixnum::typeinfo() == other_typeinfo {
            let other = unsafe { std::mem::transmute::<&Integer, &Fixnum>(other) };
            self.num == other.get()

        } else if Integer::typeinfo() == other_typeinfo {
            //Integer 同士なら通常通りnumの比較を行う
            self.num == other.num

        } else {
            //otherはReal型か？
            let other = unsafe { std::mem::transmute::<&Integer, &Real>(other) };
            self.num as f64 == other.num
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

#[inline]
fn is_fixnum_range(number: i64) -> bool {
    ((isize::MIN >> FIXNUM_MASK_BITS) as i64) < number && number <  (isize::MAX >> FIXNUM_MASK_BITS) as i64
}

pub fn make_integer<A: Allocator>(number: i64, allocator: &mut A) -> NResult<Any, OutOfMemory> {
    if is_fixnum_range(number) {
        let v = (number << FIXNUM_MASK_BITS) as usize;
        let v = v | IMMIDATE_FIXNUM;

        Ok(Ref::from(usize_to_ptr::<Any>(v)))

    } else {
        into_value(Integer::alloc(number, allocator))
    }
}

pub fn get_integer<V: ValueHolder<Any>>(v: &V) -> i64 {
    let typeinfo = get_typeinfo(v.as_ref());

    if Fixnum::typeinfo() == typeinfo {
        let fixnum: &Fixnum = v.as_ref().cast_unchecked();
        fixnum.get()

    } else if Integer::typeinfo() == typeinfo {
        let integer: &Integer = v.as_ref().cast_unchecked();
        integer.num
    } else {
        panic!("invalid number")
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
        || &FIXNUM_TYPEINFO == other_typeinfo
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

        } else if Fixnum::typeinfo() == other_typeinfo {
            let other = unsafe { std::mem::transmute::<&Real, &Fixnum>(other) };
            self.num == other.get() as f64

        } else {
            //OtherがIntegerなら、Integer型参照に無理やり戻してから比較
            let other = unsafe { std::mem::transmute::<&Real, &Integer>(other) };

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
    let typeinfo = get_typeinfo(v);

    if Fixnum::typeinfo() == typeinfo {
        let fixnum: &Fixnum = v.cast_unchecked();
        Num::Int(fixnum.get())

    } else if Integer::typeinfo() == typeinfo {
        let integer: &Integer = v.cast_unchecked();
        Num::Int(integer.num)

    } else {
        let real: &Real = v.cast_unchecked();
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
        let num = number::make_integer(int.unwrap(), obj)?;
        Ok(num)
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
        let num = number::make_integer(int.unwrap(), obj)?;
        Ok(num)
    } else {
        let num = number::Real::alloc(real.unwrap(), obj)?;
        Ok(num.into_value())
    }
}

fn func_abs(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg(0, obj);

    match number_to(v.as_ref()) {
        Num::Int(num) => {
            let num = number::make_integer(num.abs(), obj)?;
            Ok(num)
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
