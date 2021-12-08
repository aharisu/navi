#[macro_export]
macro_rules! new_typeinfo {
    ($t:ty, $name:expr, $eq_func:expr, $print_func:expr, $is_type_func:expr, ) => {
        TypeInfo {
            name: $name,
            eq_func: unsafe { std::mem::transmute::<fn(&$t, &$t) -> bool, fn(&Value, &Value) -> bool>($eq_func) },
            print_func: unsafe { std::mem::transmute::<fn(&$t, &mut std::fmt::Formatter<'_>) -> std::fmt::Result, fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result>($print_func) },
            is_type_func: $is_type_func,
        }
    }
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

use std::ptr;
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

pub trait NaviType: PartialEq + std::fmt::Debug {
    fn typeinfo() -> NonNullConst<TypeInfo>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub eq_func: fn(&Value, &Value) -> bool,
    pub print_func: fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: fn(&TypeInfo) -> bool,
}

pub struct Value { }

static VALUE_TYPEINFO : TypeInfo = new_typeinfo!(
    Value,
    "Value",
    Value::_eq,
    Value::_fmt,
    Value::_is_type,
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
    fn _eq(&self, other: &Self) -> bool {
        unreachable!()
    }

    fn _fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }

    fn _is_type(other_typeinfo: &TypeInfo) -> bool {
        unreachable!()
    }
}

pub trait AsPtr<T: ?Sized> {
    fn as_ptr(&self) -> *const T;
    fn as_mut_ptr(&self) -> *mut T;
}

pub struct NBox<T: NaviType> {
    v: NPtr<T>,
}

impl <T: NaviType> NBox<T> {
    pub fn new(v: *mut T) -> Self {
        NBox {
            v: NPtr::new(v),
        }
    }

    pub fn new_immidiate(value: usize) -> Self {
        let ptr = value as *mut usize;
        let ptr = ptr as *mut T;
        Self::new(ptr)
    }

    pub fn as_ref(&self) -> &T {
        self.v.as_ref()
    }

    pub fn as_mut_ref(&mut self) -> &mut T {
        self.v.as_mut_ref()
    }

    pub fn duplicate(&self) -> NBox<T> {
        NBox::new(self.v.as_mut_ptr())
    }

    pub fn into_nboxvalue(self) -> NBox<Value> {
        NBox::new(self.as_mut_ptr() as *mut Value)
    }

    pub fn cast_nboxvalue(&self) -> &NBox<Value> {
        unsafe { &*(self as *const NBox<T> as *const NBox<Value>) }
    }

    pub fn into_nptr(self) -> NPtr<T> {
        self.v
    }
}

impl NBox<Value> {
    pub fn into_nbox<U: NaviType>(self) -> Option<NBox<U>> {
        if let Some(v) = self.try_cast::<U>() {
            Some(v.duplicate())
        } else {
            None
        }
    }

    pub fn try_cast<'a, U: NaviType>(&'a self) -> Option<&'a NBox<U>> {
        if self.as_ref().is::<U>() {
            Some(unsafe { &*(self as *const NBox<Value> as *const NBox<U>) })

        } else {
            None
        }
    }

    pub fn is<U: NaviType>(&self) -> bool {
        self.as_ref().is::<U>()
    }

    pub fn is_type(&self, typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(typeinfo)
    }
}

impl <T: NaviType> AsPtr<T> for NBox<T> {
    fn as_ptr(&self) -> *const T {
        self.v.as_ptr()
    }

    fn as_mut_ptr(&self) -> *mut T {
        self.v.as_mut_ptr()
    }
}

impl <T: NaviType> Eq for NBox<T> {}

impl <T: NaviType> PartialEq for NBox<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl <T: NaviType> std::fmt::Debug for NBox<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl std::hash::Hash for NBox<symbol::Symbol> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

//TODO 勉強
unsafe impl<T: NaviType> Sync for NBox<T> {}
//unsafe impl<T> Send for NBox<T> {}

#[derive(Debug)]
#[repr(transparent)]
pub struct NPtr<T: NaviType> {
    pointer: *mut T,
}

impl <T: NaviType> NPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        NPtr { pointer: ptr as _ }
    }

    pub fn cast<'a, U: NaviType>(&'a self) -> &'a NPtr<U> {
        unsafe { &*(self as *const NPtr<T> as *const NPtr<U>) }
    }

    pub fn as_ref<'a>(&'a self) -> &'a T {
        self.as_mut_ref()
    }

    pub fn as_mut_ref<'a>(&'a self) -> &'a mut T {
        unsafe { &mut *(self.pointer) }
    }
}

impl <T: NaviType> crate::value::AsPtr<T> for NPtr<T> {
    fn as_ptr(&self) -> *const T {
        self.pointer
    }

    fn as_mut_ptr(&self) -> *mut T {
        self.pointer
    }
}

impl <T: NaviType> Copy for NPtr<T> { }

impl <T: NaviType> Clone for NPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}


#[cfg(test)]
mod tets {
    use crate::value::*;
    use crate::object::Object;

    #[test]
    fn is_type() {
        let mut ctx = Object::new("test");

        //int
        let v = number::Integer::alloc(&mut ctx, 10).into_nboxvalue();
        assert!(v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //real
        let v = number::Real::alloc(&mut ctx, 3.14).into_nboxvalue();
        assert!(!v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //nil
        let v = list::List::nil().into_nboxvalue();
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());

        //list
        let item = number::Integer::alloc(&mut ctx, 10).into_nboxvalue();
        let v = list::List::alloc(&mut ctx, &item, v.into_nbox::<list::List>().unwrap()).into_nboxvalue();
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());
    }
}