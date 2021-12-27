#![allow(unused_unsafe)]

use std::ptr::NonNull;

use crate::object::Object;
use crate::util::non_null_const::NonNullConst;
use crate::value::{NaviType, Value, TypeInfo};

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

    pub fn into_fptr(self) -> FPtr<T> {
        FPtr::from_ptr(self.pointer)
    }
}

impl <T: NaviType> Clone for UIPtr<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ptr())
    }
}

pub trait ValueHolder<T: NaviType> {
    fn as_ptr(&self) -> *mut T;
    fn as_ref<'a, 'b>(&'a self) -> &'b T;
    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T;
}

//
//
//
// Floating Pointer
//
#[repr(transparent)]
pub struct FPtr<T: NaviType + ?Sized> {
    pointer: *mut T,
}

impl <T: NaviType> FPtr<T> {
    pub fn new(value: &T) -> Self {
        Self::from_ptr(value as *const T as *mut T)
    }

    pub fn from_ptr(ptr: *mut T) -> Self {
        FPtr { pointer: ptr }
    }

    pub fn capture(self, obj: &mut Object) -> Cap<T> {
        obj.capture(self)
    }

    pub fn reach(self, obj: &mut Object) -> Reachable<T> {
        self.capture(obj).into_reachable()
    }

    pub fn cast_value(&self) -> &FPtr<Value> {
        unsafe { &*(self as *const FPtr<T> as *const FPtr<Value>) }
    }

    pub fn into_value(self) -> FPtr<Value> {
        self.cast_value().clone()
    }

    pub unsafe fn into_reachable(self) -> Reachable<T> {
        //このメソッドはGCが動作しないことが保証された期間のみ呼び出し可能。
        Reachable::new_static(&*self.pointer)
    }

    pub fn update_pointer(&self, new_ptr: *mut T) {
        if self.pointer != new_ptr {
            unsafe {
                std::ptr::write(&self.pointer as *const *mut T as *mut *mut T, new_ptr);
            }
        }
    }
}

impl FPtr<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&FPtr<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &FPtr<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> ValueHolder<T> for FPtr<T>{
    fn as_ptr(&self) -> *mut T {
        self.pointer
    }

    fn as_ref<'a, 'b>(&'a self) -> &'b T {
        unsafe { & *(self.pointer) }
    }

    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T {
        unsafe { &mut *(self.pointer) }
    }
}

impl <T: NaviType> Clone for FPtr<T> {
    fn clone(&self) -> Self {
        Self::from_ptr(self.as_ptr())
    }
}

impl <T: NaviType> std::fmt::Debug for FPtr<T> {
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
    pointer: NonNull<FPtr<T>>,
}

impl <T:NaviType> Cap<T> {
    pub fn new(ptr: *mut FPtr<T>) -> Cap<T> {
        Cap {
             pointer: NonNull::new(ptr).unwrap(),
        }
    }

    pub fn clone(&self, obj: &mut Object) -> Self {
        FPtr::new(self.as_ref()).capture(obj)
    }

    pub fn cast_value(&self) -> &Cap<Value> {
        unsafe {
            std::mem::transmute(self)
        }
    }

    pub fn take(self) -> FPtr<T> {
        unsafe { std::ptr::read(self.pointer.as_ptr()) }
    }

    pub fn into_reachable(self) -> Reachable<T> {
        Reachable::new_capture(self)
    }

    pub fn ptr(&self) -> *mut FPtr<T> {
        unsafe { self.pointer.clone().as_ptr() }
    }

    pub(crate) fn update_pointer(&mut self, ptr: FPtr<T>) {
        unsafe {
            self.pointer.as_ref().update_pointer(ptr.as_ptr());
        }
    }
}

impl Cap<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Cap<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &Cap<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> ValueHolder<T> for Cap<T>{
    fn as_ptr(&self) -> *mut T {
        unsafe {
            self.pointer.as_ref().as_ptr()
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
        let ptr = value as *mut usize;
        let ptr = ptr as *mut T;

        Reachable::Static(ptr)
    }

    pub fn new_capture(cap: Cap<T>) -> Self {
        Reachable::Capture(cap)
    }

    pub fn into_fptr(self) -> FPtr<T> {
        match self {
            Self::Static(ptr) => {
                FPtr::from_ptr(ptr)
            },
            Self::Capture(cap) => {
                cap.take()
            },
        }
    }

    pub fn into_value(self) -> Reachable<Value> {
        match self {
            Self::Static(ptr) => {
                Reachable::Static(ptr as *mut Value)
            },
            Self::Capture(cap) => {
                unsafe {
                    //Capが内部で持っているのはTやValueへのポインタのポインタで、
                    //T から Valueへの変換は動作に影響がないことが保証されているので無理やりtransmuteする。
                    //CapはクローンするとObjectから新しい領域をアロケートするので、オーバーヘッドを回避するためにtransmuteしている。
                    let cap_value = std::mem::transmute::<Cap<T>, Cap<Value>>(cap);
                    Reachable::Capture(cap_value)
                }
            },
        }
    }

    pub fn cast_value(&self) -> &Reachable<Value> {
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

impl Reachable<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Reachable<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &Reachable<U> {
        std::mem::transmute(self)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> ValueHolder<T> for Reachable<T> {
    fn as_ptr(&self) -> *mut T {
        match self {
            Self::Static(ptr) => {
                *ptr
            },
            Self::Capture(cap) => {
                cap.as_ptr()
            },
        }
    }

    fn as_ref<'a, 'b>(&'a self) -> &'b T {
        match self {
            Self::Static(ptr) => {
                unsafe { & **ptr }
            },
            Self::Capture(cap) => {
                cap.as_ref()
            },
        }

    }

    fn as_mut<'a, 'b>(&'a mut self) -> &'b mut T {

        match self {
            Self::Static(ptr) => {
                unsafe { &mut **ptr }
            },
            Self::Capture(cap) => {
                cap.as_mut()
            },
        }
    }
}
