#[macro_export]
macro_rules! new_typeinfo {
    ($t:ty, $name:expr, $eq_func:expr, $print_func:expr, $is_type_func:expr, $child_traversal_func:expr, ) => {
        TypeInfo {
            name: $name,
            eq_func: unsafe { std::mem::transmute::<fn(&$t, &$t) -> bool, fn(&Value, &Value) -> bool>($eq_func) },
            print_func: unsafe { std::mem::transmute::<fn(&$t, &mut std::fmt::Formatter<'_>) -> std::fmt::Result, fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result>($print_func) },
            is_type_func: $is_type_func,
            child_traversal_func: match $child_traversal_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&$t, usize, fn(&NPtr<Value>, usize)), fn(&Value, usize, fn(&NPtr<Value>, usize))>(func) }),
                None => None
             },
        }
    };
}

pub mod array;
pub mod bool;
pub mod closure;
pub mod list;
pub mod number;
pub mod string;
pub mod symbol;
pub mod func;
pub mod syntax;
pub mod unit;

use std::ptr::{self};
use std::cell::Cell;
use crate::util::non_null_const::*;

// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b0000_1111;


const fn tagged_value(tag: usize) -> usize {
    (tag << 16) | IMMIDATE_TAGGED_VALUE
}

pub(crate) const IMMIDATE_NIL: usize = tagged_value(0);
pub(crate) const IMMIDATE_TRUE: usize = tagged_value(1);
pub(crate) const IMMIDATE_FALSE: usize = tagged_value(2);
pub(crate) const IMMIDATE_UNIT: usize = tagged_value(3);

#[derive(PartialEq)]
enum PtrKind {
    Ptr,
    Nil,
    True,
    False,
    Unit,
}

fn pointer_kind<T>(ptr: *const T) -> PtrKind {
    let value = crate::mm::ptr_to_usize(ptr);

    //下位2bitが00なら生ポインタ
    if value & 0b11 == 0 {
        PtrKind::Ptr
    } else {
        //残りは下位16bitで判断する
        match value &0xFFFF {
            IMMIDATE_TAGGED_VALUE => {
                match value {
                    IMMIDATE_NIL => PtrKind::Nil,
                    IMMIDATE_TRUE => PtrKind::True,
                    IMMIDATE_FALSE => PtrKind::False,
                    IMMIDATE_UNIT => PtrKind::Unit,
                    _ => panic!("invalid tagged value"),
                }
            }
            _ => panic!("invalid pointer"),
        }
    }
}

pub fn value_is_pointer(v: &Value) -> bool {
    pointer_kind(v as *const Value) == PtrKind::Ptr
}

pub trait NaviType: PartialEq + std::fmt::Debug {
    fn typeinfo() -> NonNullConst<TypeInfo>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub eq_func: fn(&Value, &Value) -> bool,
    pub print_func: fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: fn(&TypeInfo) -> bool,
    pub child_traversal_func: Option<fn(&Value, usize, fn(&NPtr<Value>, usize))>,
}

pub struct Value { }

static VALUE_TYPEINFO : TypeInfo = new_typeinfo!(
    Value,
    "Value",
    Value::_eq,
    Value::_fmt,
    Value::_is_type,
    None,
);

impl NaviType for Value {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&VALUE_TYPEINFO as *const TypeInfo)
    }
}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = self.get_typeinfo();
        let other_typeinfo = other.get_typeinfo();
        if ptr::eq(self_typeinfo.as_ptr(), other_typeinfo.as_ptr()) {
            (unsafe { self_typeinfo.as_ref() }.eq_func)(self, other)

        } else {
            false
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = crate::mm::get_typeinfo(self as *const Self);

        (unsafe { self_typeinfo.as_ref() }.print_func)(self, f)
    }
}

impl Value {
    pub fn get_typeinfo(&self) -> NonNullConst<TypeInfo> {
        let ptr = self as *const Value;
        match pointer_kind(ptr) {
            PtrKind::Nil => {
                crate::value::list::List::typeinfo()
            }
            PtrKind::True | PtrKind::False => {
                crate::value::bool::Bool::typeinfo()
            }
            PtrKind::Unit => {
                crate::value::unit::Unit::typeinfo()
            }
            PtrKind::Ptr => {
                crate::mm::get_typeinfo(ptr)
            }
        }
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        if std::ptr::eq(&VALUE_TYPEINFO, other_typeinfo.as_ptr()) {
            //is::<Value>()の場合、常に結果はtrue
            true

        } else {
            let self_typeinfo = self.get_typeinfo();

            (unsafe { self_typeinfo.as_ref() }.is_type_func)(unsafe { other_typeinfo.as_ref() })
        }
    }

    pub fn try_cast<U: NaviType>(&self) -> Option<&U> {
        if self.is::<U>() {
            Some(unsafe { &*(self as *const Value as *const U) })
        } else {
            None
        }
    }

    //Value型のインスタンスは存在しないため、これらのメソッドが呼び出されることはない
    fn _eq(&self, _other: &Self) -> bool {
        unreachable!()
    }

    fn _fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }

    fn _is_type(_other_typeinfo: &TypeInfo) -> bool {
        unreachable!()
    }

    fn _child_traversal(&self, _callback: fn(&Value)) {
        unreachable!()
    }

}

pub trait AsPtr<T: ?Sized> {
    fn as_ptr(&self) -> *const T;
    fn as_mut_ptr(&self) -> *mut T;
}


#[derive(Debug)]
#[repr(transparent)]
pub struct NPtr<T: NaviType> {
    pointer: Cell<*mut T>,
}

impl <T: NaviType> NPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        NPtr { pointer: Cell::new(ptr) }
    }

    pub fn new_immidiate(value: usize) -> Self {
        let ptr = value as *mut usize;
        let ptr = ptr as *mut T;
        NPtr::new(ptr)
    }

    pub fn into_value(self) -> NPtr<Value> {
        self.cast_value().clone()
    }

    pub fn cast_value(&self) -> &NPtr<Value> {
        unsafe { &*(self as *const NPtr<T> as *const NPtr<Value>) }
    }

    pub fn update_pointer(&self, ptr: *mut T) {
        self.pointer.set(ptr);
    }
}

impl NPtr<Value> {
    pub fn try_into<'b, U: NaviType>(self) -> Option<NPtr<U>> {
        if let Some(v) = self.try_cast::<U>() {
            Some(v.clone())
        } else {
            None
        }
    }

    pub fn try_cast<U: NaviType>(&self) -> Option<&NPtr<U>> {
        if self.as_ref().is::<U>() {
            Some(unsafe { &*(self as *const NPtr<Value> as *const NPtr<U>) })

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &NPtr<U> {
        &*(self as *const NPtr<Value> as *const NPtr<U>)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        self.as_ref().is::<U>()
    }

    pub fn is_type(&self, typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(typeinfo)
    }
}


impl <T: NaviType> AsRef<T> for NPtr<T> {

    fn as_ref(&self) -> &T {
        unsafe { & *(self.pointer.get()) }
    }
}

impl <T: NaviType> AsMut<T> for NPtr<T> {

    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.pointer.get()) }
    }
}

impl <T: NaviType> crate::value::AsPtr<T> for NPtr<T> {
    fn as_ptr(&self) -> *const T {
        self.pointer.get()
    }

    fn as_mut_ptr(&self) -> *mut T {
        self.pointer.get()
    }
}

impl <T: NaviType> Clone for NPtr<T> {
    fn clone(&self) -> Self {
        NPtr::new(self.pointer.get())
    }
}


#[cfg(test)]
mod tests {
    use crate::{value::*, let_cap, new_cap};
    use crate::object::Object;

    #[test]
    fn is_type() {
        let mut ctx = Object::new("test");
        let ctx = &mut ctx;

        //int
        let v = number::Integer::alloc(10, ctx).into_value();
        assert!(v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //real
        let v = number::Real::alloc(3.14, ctx).into_value();
        assert!(!v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //nil
        let_cap!(v, list::List::nil().into_value(), ctx);
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());

        //list
        let_cap!(item, number::Integer::alloc(10, ctx).into_value(), ctx);
        let_cap!(v, list::List::alloc(&item, v.try_cast::<list::List>().unwrap(), ctx).into_value(), ctx);
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());
    }
}