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

fn pointer_kind(ptr: *const u8) -> PtrKind {
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

#[derive(Debug, PartialEq)]
struct NaviDummy { }

impl NaviType for NaviDummy {}

#[allow(dead_code)]
pub struct TypeInfo<T: NaviType> {
    pub name : &'static str,
    pub eq_func: fn(&T, &T) -> bool,
    pub print_func: fn(&T, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
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

    pub fn into_navibox(self) -> NaviBox {
        NaviBox { v: self.v.cast() }
    }

    pub fn eq_navibox(&self, other: &NaviBox) -> bool {
        other.eq_nbox(self)
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


//NBox<T>からTの型情報をなくすためのstruct
#[derive(Copy, Clone)]
pub struct NaviBox {
    #[allow(dead_code)]
    v: ptr::NonNull<u8>,
}
//TODO 勉強
unsafe impl Sync for NaviBox {}
//unsafe impl Send for NaviBox {}

impl NaviBox {
    pub fn into_nbox<'ti, T: NaviType>(self, typeinfo: &'ti TypeInfo<T>) -> Option<NBox<T>> {
        let ptr = self.v.as_ptr();

        match pointer_kind(ptr) {
            PtrKind::Nil => {
                let self_typeinfo = crate::value::list::List::typeinfo();
                if ptr::eq(typeinfo, self_typeinfo as *const TypeInfo<list::List> as *const TypeInfo<T>) {
                    Some(NBox::<T>::new_immidiate(IMMIDATE_NIL))

                } else {
                    None
                }
            }
            PtrKind::Ptr => {
                let self_typeinfo = crate::mm::get_typeinfo(ptr);
                if ptr::eq(typeinfo, self_typeinfo) {
                    Some(NBox::<T>::new(ptr as *mut T))
                } else {
                    None
                }
            }
        }
    }

    fn as_nbox_unchecked<'a, T: NaviType>(&self) -> NBox<T> {
        NBox::<T>::new(self.v.as_ptr() as *mut T)
    }

    pub fn eq_nbox<T: NaviType>(&self, other: &NBox<T>) -> bool {
        //互いのTypeInfoオブジェクトを取得し、ポインタが一致するかチェック
        let self_typeinfo = crate::mm::get_typeinfo::<T>(self.v.as_ptr());
        let other_typeinfo = crate::mm::get_typeinfo::<T>(other.as_ptr() as *const u8);
        if ptr::eq(self_typeinfo, other_typeinfo) {
            //同一TypeInfoなら同じ型のオブジェクトなのでNaviBoxをNBox<T>に変換して、
            //型に定義されているeqに処理を委譲する
            let self_nbox = self.as_nbox_unchecked::<T>();
            self_nbox.as_ref().eq(other.as_ref())

        } else {
            false
        }
    }
}

impl Eq for NaviBox {}

impl PartialEq for NaviBox {
    fn eq(&self, other: &Self) -> bool {
        // 完全に型情報が消えてしまっているので、TypeInfo内にあるeq_funcを呼ぶ
        let self_typeinfo = crate::mm::get_typeinfo::<NaviDummy>(self.v.as_ptr());
        let other_typeinfo = crate::mm::get_typeinfo::<NaviDummy>(other.v.as_ptr() as *const u8);
        if ptr::eq(self_typeinfo, other_typeinfo) {
            let self_obj = unsafe { &*(self.v.as_ptr() as *const NaviDummy) };
            let other_obj = unsafe { &*(other.v.as_ptr() as *const NaviDummy) };

            (self_typeinfo.eq_func)(self_obj, other_obj)
        } else {
            false
        }
    }
}

impl std::fmt::Debug for NaviBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = crate::mm::get_typeinfo::<NaviDummy>(self.v.as_ptr());
        let self_obj = unsafe { &*(self.v.as_ptr() as *const NaviDummy) };

        (self_typeinfo.print_func)(self_obj, f)
    }
}