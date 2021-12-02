pub mod boolean;
pub mod list;
pub mod number;
pub mod string;
pub mod symbol;

use std::ptr;

// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b0000_1111;


const fn tagged_value(tag: usize) -> usize {
    (tag << 8) | IMMIDATE_TAGGED_VALUE
}

pub(crate) const IMMIDATE_NIL: usize = tagged_value(0);

enum PtrKind {
    Ptr,
    Nil,
}

fn pointer_kind<T>(ptr: *const T) -> PtrKind {
    let value = crate::mm::ptr_to_usize(ptr);

    //下位2bitが00なら生ポインタ
    if value & 0b11 == 0 {
        PtrKind::Ptr
    } else {
        //残りは下位16bitで判断する
        match value &0xFFFF {
            IMMIDATE_TAGGED_VALUE => PtrKind::Nil,
            _ => panic!("invalid pointer"),
        }
    }
}

pub trait NaviType: PartialEq + std::fmt::Debug { }

#[allow(dead_code)]
pub struct TypeInfo<T: NaviType> {
    pub name : &'static str,
    pub eq_func: fn(&T, &T) -> bool,
    pub print_func: fn(&T, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
}

pub struct Value { }

impl NaviType for Value {}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = crate::mm::get_typeinfo(self as *const Self);
        let other_typeinfo = crate::mm::get_typeinfo(other as *const Self);
        if ptr::eq(self_typeinfo, other_typeinfo) {
            (self_typeinfo.eq_func)(self, other)

        } else {
            false
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = crate::mm::get_typeinfo(self as *const Self);

        (self_typeinfo.print_func)(self, f)
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

    pub fn into_nbox<'ti, U: NaviType>(self, typeinfo: &'ti TypeInfo<U>) -> Option<NBox<U>> {
        let ptr = self.v.as_ptr();

        match pointer_kind(ptr) {
            PtrKind::Nil => {
                let self_typeinfo = crate::value::list::List::typeinfo();
                if ptr::eq(typeinfo, self_typeinfo as *const TypeInfo<list::List> as *const TypeInfo<U>) {
                    Some(NBox::<U>::new_immidiate(IMMIDATE_NIL))

                } else {
                    None
                }
            }
            PtrKind::Ptr => {
                let self_typeinfo = crate::mm::get_typeinfo(ptr);
                if ptr::eq(typeinfo, self_typeinfo as *const TypeInfo<T> as *const TypeInfo<U>) {
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

//TODO 勉強
unsafe impl<T: NaviType> Sync for NBox<T> {}
//unsafe impl<T> Send for NBox<T> {}
