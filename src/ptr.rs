#![allow(unused_unsafe)]

use std::ptr::NonNull;

use crate::object::Object;
use crate::object::mm::usize_to_ptr;
use crate::value::{NaviType, TypeInfo};
use crate::value::any::Any;

//
//
//
//UnInitilized Pointer
//
#[derive(Debug)]
#[repr(transparent)]
pub struct UIPtr<T: NaviType> {
    pointer: *mut T,
}

impl <T: NaviType> UIPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        UIPtr::<T> {
            pointer: ptr
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.pointer
    }

    pub fn into_ref(self) -> Ref<T> {
        self.pointer.into()
    }
}

impl <T: NaviType> Clone for UIPtr<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ptr())
    }
}

pub trait ValueHolder<T: NaviType> {
    fn has_replytype(&self) -> bool;
    fn raw_ptr(&self) -> *mut T;
    fn as_ref<'a, 'b>(&'a self) -> &'b T;
    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T;
}

//
//
//
// Reference
//
#[repr(transparent)]
pub struct Ref<T: NaviType + ?Sized> {
    pointer: *mut T,
}

impl <T: NaviType> Ref<T> {
    pub fn new(value: &T) -> Self {
        Ref::from(value as *const T as *mut T)
    }

    pub fn capture(self, obj: &mut Object) -> Cap<T> {
        obj.capture(self)
    }

    pub fn reach(self, obj: &mut Object) -> Reachable<T> {
        self.capture(obj).into_reachable()
    }

    pub fn cast_value(&self) -> &Ref<Any> {
        unsafe { std::mem::transmute(self) }
    }

    pub fn cast_mut_value(&mut self) -> &mut Ref<Any> {
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_value(self) -> Ref<Any> {
        self.cast_value().clone()
    }

    pub unsafe fn into_reachable(self) -> Reachable<T> {
        //このメソッドはGCが動作しないことが保証された期間のみ呼び出し可能。
        Reachable::new_static(&*self.pointer)
    }

    pub fn update_pointer(&mut self, new_ptr: *mut T) {
        if self.pointer != new_ptr {
            unsafe {
                std::ptr::write(&mut self.pointer, new_ptr);
            }
        }
    }

    pub(crate) fn gc_update_pointer(&mut self, new_ptr: *mut T) {
        //GC時専用のポインタ更新関数
        //GC時に呼ばれる場合は、もともとのReplyフラグを保持して更新する
        if self.pointer != new_ptr {
            let has_reply = self.has_replytype();

            unsafe {
                std::ptr::write(&mut self.pointer, new_ptr);
            }

            if has_reply {
                crate::value::set_has_replytype_flag(self);
            }
        }
    }

}

impl Ref<Any> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Ref<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub fn try_cast_mut<U: NaviType>(&mut self) -> Option<&mut Ref<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_mut_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &Ref<U> {
        std::mem::transmute(self)
    }

    pub unsafe fn cast_mut_unchecked<U: NaviType>(&mut self) -> &mut Ref<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: &'static TypeInfo) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> From<*mut T> for Ref<T> {
    fn from(item: *mut T) -> Self {
       Ref { pointer: item }
    }
}

impl <T: NaviType> ValueHolder<T> for Ref<T>{
    fn has_replytype(&self) -> bool {
        crate::value::has_replytype(self)
    }

    fn raw_ptr(&self) -> *mut T {
        self.pointer
    }

    fn as_ref<'a, 'b>(&'a self) -> &'b T {
        crate::value::refer_value(self)
    }

    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T {
        crate::value::mut_refer_value(self)
    }
}

impl <T: NaviType> Clone for Ref<T> {
    fn clone(&self) -> Self {
        self.raw_ptr().into()
    }
}

impl <T: NaviType> std::fmt::Debug for Ref<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_ref(), f)
    }
}

//
//
//
// Capture
//
#[repr(transparent)]
pub struct Cap<T: NaviType> {
    pointer: NonNull<Ref<T>>,
}

impl <T:NaviType> Cap<T> {
    pub fn new(ptr: *mut Ref<T>) -> Cap<T> {
        Cap {
             pointer: NonNull::new(ptr).unwrap(),
        }
    }

    pub fn clone(&self, obj: &mut Object) -> Self {
        Ref::from(self.raw_ptr()).capture(obj)
    }

    pub fn cast_value(&self) -> &Cap<Any> {
        unsafe {
            std::mem::transmute(self)
        }
    }

    pub fn make(&self) -> Ref<T> {
        (unsafe { self.pointer.as_ref() }).clone()
    }

    pub fn take(self) -> Ref<T> {
        unsafe { std::ptr::read(self.pointer.as_ptr()) }
    }

    pub fn into_reachable(self) -> Reachable<T> {
        Reachable::new_capture(self)
    }

    pub fn ptr(&self) -> *mut Ref<T> {
        unsafe { self.pointer.clone().as_ptr() }
    }

    pub fn refer(&self) -> &Ref<T> {
        unsafe { &*(self.ptr()) }
    }

    pub fn mut_refer(&mut self) -> &mut Ref<T> {
        unsafe { &mut *(self.ptr()) }
    }

    pub(crate) fn update_pointer(&mut self, ptr: Ref<T>) {
        unsafe {
            self.pointer.as_mut().update_pointer(ptr.raw_ptr());
        }
    }
}

impl Cap<Any> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Cap<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub fn try_cast_mut<U: NaviType>(&mut self) -> Option<&mut Cap<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked_mut() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &Cap<U> {
        std::mem::transmute(self)
    }

    pub unsafe fn cast_unchecked_mut<U: NaviType>(&mut self) -> &mut Cap<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: &'static TypeInfo) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> ValueHolder<T> for Cap<T>{
    fn has_replytype(&self) -> bool {
        unsafe {
            self.pointer.as_ref().has_replytype()
        }
    }

    fn raw_ptr(&self) -> *mut T {
        unsafe {
            self.pointer.as_ref().raw_ptr()
        }
    }

    fn as_ref<'a, 'b>(&'a self) -> &'b T {
        unsafe {
            self.pointer.as_ref().as_ref()
        }
    }

    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T {
        unsafe {
            self.pointer.as_mut().as_mut()
        }
    }
}

impl <T: NaviType> Drop for Cap<T> {
    fn drop(&mut self) {
        crate::object::Object::release_capture(self)
    }
}

//
//
//
// Reachable(Static / Immidiate / Capture)
//
pub enum Reachable<T: NaviType> {
    Static(*mut T),
    Capture(Cap<T>),
}

impl <T: NaviType> Reachable<T> {
    pub fn new_static(value: &T) -> Self {
        let ptr = value as *const T as *mut T;
        Reachable::Static(ptr)
    }

    pub fn new_immidiate(value: usize) -> Self {
        let ptr = usize_to_ptr::<T>(value);

        Reachable::Static(ptr)
    }

    pub fn new_capture(cap: Cap<T>) -> Self {
        Reachable::Capture(cap)
    }

    pub fn make(&self) -> Ref<T> {
        match self {
            Self::Static(ptr) => {
                Ref::from(*ptr)
            },
            Self::Capture(cap) => {
                cap.make()
            },
        }
    }

    pub fn into_ref(self) -> Ref<T> {
        match self {
            Self::Static(ptr) => {
                ptr.into()
            },
            Self::Capture(cap) => {
                cap.take()
            },
        }
    }

    pub fn into_value(self) -> Reachable<Any> {
        match self {
            Self::Static(ptr) => {
                Reachable::Static(ptr as *mut Any)
            },
            Self::Capture(cap) => {
                unsafe {
                    //Capが内部で持っているのはTやValueへのポインタのポインタで、
                    //T から Valueへの変換は動作に影響がないことが保証されているので無理やりtransmuteする。
                    //CapはクローンするとObjectから新しい領域をアロケートするので、オーバーヘッドを回避するためにtransmuteしている。
                    let cap_value = std::mem::transmute::<Cap<T>, Cap<Any>>(cap);
                    Reachable::Capture(cap_value)
                }
            },
        }
    }

    pub fn cast_value(&self) -> &Reachable<Any> {
        unsafe {
            std::mem::transmute(self)
        }
    }

    pub fn clone(&self, obj: &mut Object) -> Self {
        match self {
            Self::Static(ptr) => {
                Reachable::Static(*ptr)
            },
            Self::Capture(cap) => {
                Reachable::Capture(cap.clone(obj))
            },
        }
    }
}

impl Reachable<Any> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Reachable<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub fn try_cast_mut<U: NaviType>(&mut self) -> Option<&mut Reachable<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked_mut() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &Reachable<U> {
        std::mem::transmute(self)
    }

    pub unsafe fn cast_unchecked_mut<U: NaviType>(&mut self) -> &mut Reachable<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: &'static TypeInfo) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> ValueHolder<T> for Reachable<T> {
    fn has_replytype(&self) -> bool {
        match self {
            Self::Static(_) => false,
            Self::Capture(cap) => cap.has_replytype(),
        }
    }

    fn raw_ptr(&self) -> *mut T {
        match self {
            Self::Static(ptr) => {
                unsafe {
                    let refer: &Ref<T> = std::mem::transmute(ptr);
                    refer.raw_ptr()
                }
            },
            Self::Capture(cap) => {
                cap.raw_ptr()
            },
        }
    }

    fn as_ref<'a, 'b>(&'a self) -> &'b T {
        match self {
            Self::Static(ptr) => {
                unsafe {
                    let refer: &Ref<T> = std::mem::transmute(ptr);
                    refer.as_ref()
                }
            },
            Self::Capture(cap) => {
                cap.as_ref()
            },
        }

    }

    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T {

        match self {
            Self::Static(ptr) => {
                unsafe {
                    let refer: &mut Ref<T> = std::mem::transmute(ptr);
                    refer.as_mut()
                }
            },
            Self::Capture(cap) => {
                cap.as_mut()
            },
        }
    }
}
