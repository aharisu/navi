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

pub mod bool;
pub mod list;
pub mod number;
pub mod string;
pub mod symbol;
pub mod func;
pub mod syntax;

use std::ptr::{self, NonNull};

// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b0000_1111;


const fn tagged_value(tag: usize) -> usize {
    (tag << 16) | IMMIDATE_TAGGED_VALUE
}

pub(crate) const IMMIDATE_NIL: usize = tagged_value(0);
pub(crate) const IMMIDATE_TRUE: usize = tagged_value(1);
pub(crate) const IMMIDATE_FALSE: usize = tagged_value(2);

enum PtrKind {
    Ptr,
    Nil,
    True,
    False,
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
                    _ => panic!("invalid tagged value"),
                }
            }
            _ => panic!("invalid pointer"),
        }
    }
}

pub trait NaviType: PartialEq + std::fmt::Debug {
    fn typeinfo() -> NonNull<TypeInfo>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub eq_func: fn(&Value, &Value) -> bool,
    pub print_func: fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: fn(&TypeInfo) -> bool,
}

pub struct Value { }

impl NaviType for Value {
    fn typeinfo() -> NonNull<TypeInfo> {
        unreachable!()
    }
}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = crate::mm::get_typeinfo(self as *const Self);
        let other_typeinfo = crate::mm::get_typeinfo(other as *const Self);
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
    pub fn is_type<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();

        let ptr = self as *const Value;
        let self_typeinfo = match pointer_kind(ptr) {
            PtrKind::Nil => {
                crate::value::list::List::typeinfo()
            }
            PtrKind::True | PtrKind::False => {
                crate::value::bool::Bool::typeinfo()
            }
            _ => {
                crate::mm::get_typeinfo(ptr)
            }
        };

        (unsafe { self_typeinfo.as_ref() }.is_type_func)(unsafe { other_typeinfo.as_ref() })
    }
}


pub struct NBox<T: NaviType> {
    v: ptr::NonNull<T>,
}

impl <T: NaviType> NBox<T> {
    pub fn new(v: *mut T) -> Self {
        NBox {
            v: unsafe { std::ptr::NonNull::new_unchecked(v) },
        }
    }

    pub fn new_immidiate(value: usize) -> Self {
        let ptr = value as *mut usize;
        let ptr = ptr as *mut T;
        Self::new(ptr)
    }

    pub fn as_ptr(&self) -> *const T {
        self.v.as_ptr()
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.v.as_ptr()
    }

    pub fn as_ref(&self) -> &T {
        unsafe {
            self.v.as_ref()
        }
    }

    pub fn as_mut_ref(&mut self) -> &mut T {
        unsafe {
            self.v.as_mut()
        }
    }

    pub fn into_nboxvalue(self) -> NBox<Value> {
        NBox::new(self.as_mut_ptr() as *mut Value)
    }

    pub fn into_nbox<U: NaviType>(self) -> Option<NBox<U>> {
        let typeinfo = U::typeinfo();
        let ptr = self.v.as_ptr();

        match pointer_kind(ptr) {
            PtrKind::Nil => {
                let self_typeinfo = crate::value::list::List::typeinfo();
                if ptr::eq(typeinfo.as_ptr(), self_typeinfo.as_ptr()) {
                    Some(NBox::<U>::new_immidiate(IMMIDATE_NIL))

                } else {
                    None
                }
            }
            PtrKind::True | PtrKind::False => {
                let self_typeinfo = crate::value::bool::Bool::typeinfo();
                if ptr::eq(typeinfo.as_ptr(), self_typeinfo.as_ptr()) {
                    Some(NBox::<U>::new_immidiate(crate::mm::ptr_to_usize(ptr)))
                } else {
                    None
                }
            }
            PtrKind::Ptr => {
                let self_typeinfo = crate::mm::get_typeinfo(ptr);
                if ptr::eq(typeinfo.as_ptr(), self_typeinfo.as_ptr()) {
                    Some(NBox::<U>::new(ptr as *mut U))
                } else {
                    None
                }
            }
        }
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


#[cfg(test)]
mod tets {
    use crate::mm::{Heap};
    use crate::value::*;

    #[test]
    fn is_type() {
        let mut heap = Heap::new(1024, "test");

        //int
        let v = number::Integer::alloc(&mut heap, 10).into_nboxvalue();
        assert!(v.as_ref().is_type::<number::Integer>());
        assert!(v.as_ref().is_type::<number::Real>());
        assert!(v.as_ref().is_type::<number::Number>());

        //real
        let v = number::Real::alloc(&mut heap, 3.14).into_nboxvalue();
        assert!(!v.as_ref().is_type::<number::Integer>());
        assert!(v.as_ref().is_type::<number::Real>());
        assert!(v.as_ref().is_type::<number::Number>());

        //nil
        let v = list::List::nil().into_nboxvalue();
        assert!(v.as_ref().is_type::<list::List>());
        assert!(!v.as_ref().is_type::<string::NString>());

        //list
        let item = number::Integer::alloc(&mut heap, 10).into_nboxvalue();
        let v = list::List::alloc(&mut heap, &item, v.into_nbox::<list::List>().unwrap()).into_nboxvalue();
        assert!(v.as_ref().is_type::<list::List>());
        assert!(!v.as_ref().is_type::<string::NString>());

        heap.free();
    }
}