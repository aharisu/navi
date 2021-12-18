#![allow(unused_unsafe)]

use std::ptr::NonNull;

use crate::util::non_null_const::NonNullConst;
use crate::value::{NaviType, Value, TypeInfo};
use crate::context::Context;

pub trait AsReachable<T: NaviType> {
    fn as_reachable(&self) -> &RPtr<T>;
}

pub trait AsPtr<T: ?Sized> {
    fn as_ptr(&self) -> *mut T;
}

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
    pub unsafe fn as_ref(&self) -> &T {
        & *(self.pointer)
    }

    pub unsafe fn as_mut(&mut self) -> &mut T {
        &mut *(self.pointer)
    }

    pub fn into_fptr(self) -> FPtr<T> {
        FPtr::new(self.pointer)
    }

    pub fn into_rptr(self) -> RPtr<T> {
        RPtr::new(self.pointer)
    }
}

impl <T: NaviType> UIPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        UIPtr::<T> {
            pointer: ptr
        }
    }
}

impl <T: NaviType> AsPtr<T> for UIPtr<T> {
    fn as_ptr(&self) -> *mut T {
        self.pointer
    }
}

impl <T: NaviType> Clone for UIPtr<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ptr())
    }
}

//
//
//
//Reachable Pointer
//
#[repr(transparent)]
pub struct RPtr<T: NaviType> {
    pointer: *mut T,
}

impl <T: NaviType> RPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        RPtr { pointer: ptr }
    }

    pub fn new_immidiate(value: usize) -> Self {
        let ptr = value as *mut usize;
        let ptr = ptr as *mut T;
        RPtr::new(ptr)
    }

    pub fn cast_value(&self) -> &RPtr<Value> {
        unsafe { &*(self as *const RPtr<T> as *const RPtr<Value>) }
    }

    pub fn into_value(self) -> RPtr<Value> {
        self.cast_value().clone()
    }

    pub fn into_fptr(self) -> FPtr<T> {
        FPtr::new(self.pointer)
    }

    pub fn update_pointer(&self, new_ptr: *mut T) {
        unsafe {
            std::ptr::write(&self.pointer as *const *mut T as *mut *mut T, new_ptr);
        }
    }
}

impl RPtr<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&RPtr<U>> {
        if self.as_ref().is::<U>() {
            Some(unsafe { &*(self as *const RPtr<Value> as *const RPtr<U>) })

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &RPtr<U> {
        unsafe { &*(self as *const RPtr<Value> as *const RPtr<U>) }
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(other_typeinfo)
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }
}

impl <T: NaviType> AsPtr<T> for RPtr<T> {
    fn as_ptr(&self) -> *mut T {
        self.pointer
    }
}

impl <T: NaviType> Clone for RPtr<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ptr())
    }
}

impl <T: NaviType> AsReachable<T> for RPtr<T> {
    fn as_reachable(&self) -> &RPtr<T> {
        self
    }
}

impl <T: NaviType> AsRef<T> for RPtr<T> {
    fn as_ref(&self) -> &T {
        unsafe { & *(self.pointer) }
    }
}

impl <T: NaviType> AsMut<T> for RPtr<T> {

    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.pointer) }
    }
}

impl <T: NaviType> std::fmt::Debug for RPtr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_ref(), f)
    }
}

//
//
//
//Floating Pointer
//
#[derive(Debug)]
#[repr(transparent)]
pub struct FPtr<T: NaviType> {
    pointer: *mut T,
}

impl <T: NaviType> FPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        FPtr { pointer: ptr }
    }

    pub fn cast_value(&self) -> &FPtr<Value> {
        unsafe { &*(self as *const FPtr<T> as *const FPtr<Value>) }
    }

    pub fn into_value(self) -> FPtr<Value> {
        self.cast_value().clone()
    }

    pub fn into_rptr(self) -> RPtr<T> {
        RPtr::new(self.pointer)
    }
}

impl FPtr<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&FPtr<U>> {
        if self.as_ref().is::<U>() {
            Some(unsafe { &*(self as *const FPtr<Value> as *const FPtr<U>) })

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: NaviType>(&self) -> &FPtr<U> {
        unsafe { &*(self as *const FPtr<Value> as *const FPtr<U>) }
    }
}

impl <T: NaviType> AsPtr<T> for FPtr<T> {
    fn as_ptr(&self) -> *mut T {
        self.pointer
    }
}

impl <T: NaviType> Clone for FPtr<T> {
    fn clone(&self) -> Self {
        Self::new(self.as_ptr())
    }
}

impl <T: NaviType> AsRef<T> for FPtr<T> {
    fn as_ref(&self) -> &T {
        unsafe { & *(self.pointer) }
    }
}

impl <T: NaviType> AsMut<T> for FPtr<T> {
    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.pointer) }
    }
}


#[macro_export]
macro_rules! new_cap {
    ($ptr:expr, $ctx:expr) => {
        crate::ptr::Capture {
            v: $ptr,
            ctx: unsafe { std::ptr::NonNull::new_unchecked( $ctx as *const Context as *mut Context) },
            next: None,
            prev: None,
            _pinned: std::marker::PhantomPinned,
        }
    };
}

#[macro_export]
macro_rules! let_cap {
    ($name:ident, $ptr:expr, $ctx:expr) => {
        #[allow(dead_code)]
        let mut $name = new_cap!($ptr, $ctx);
        //Captureはmove禁止オブジェクトなので本来はPinを使用すべき。
        //書き方が冗長になるため一時的に廃止。
        //冗長な書き方の回避方法がわかるか、
        //問題が大きいようならPinを使用するようにする。
        //pin_utils::pin_mut!($name);
        ($ctx).add_capture(&mut ($name).cast_value_mut());
    };
}

#[macro_export]
macro_rules! with_cap {
    ($name:ident, $ptr:expr, $ctx:expr, $block:expr) => {
        {
            let_cap!($name, $ptr, $ctx);
            $block
        }
    };
}

pub struct Capture<T: NaviType> {
    pub v: FPtr<T>,
    pub ctx: NonNull<Context>,
    pub next: Option<NonNull<Capture<Value>>>,
    pub prev: Option<NonNull<Capture<Value>>>,
    pub _pinned: std::marker::PhantomPinned,
}

impl <T:NaviType> Capture<T> {
    pub fn cast_value_mut(&mut self) -> &mut Capture<Value> {
        unsafe { &mut *(self as *const Capture<T> as *const Capture<Value> as *mut Capture<Value>) }
    }
}

impl <T: NaviType> AsReachable<T> for Capture<T> {
    fn as_reachable(&self) -> &RPtr<T> {
        unsafe {
            &*(&self.v as *const FPtr<T> as *const RPtr<T>)
        }
    }
}

impl <T: NaviType> AsRef<T> for Capture<T> {
    fn as_ref(&self) -> &T {
        self.v.as_ref()
    }
}

impl <T: NaviType> AsMut<T> for Capture<T> {
    fn as_mut(&mut self) -> &mut T {
        self.v.as_mut()
    }
}

impl <T: NaviType> AsPtr<T> for Capture<T> {
    fn as_ptr(&self) -> *mut T {
        self.v.as_ptr()
    }
}

impl <T: NaviType> Drop for Capture<T> {
    fn drop(&mut self) {
        unsafe { self.ctx.as_ref() }.drop_capture(self.cast_value_mut())
    }
}

/*
impl <T: NaviType> Ptr<T> for std::pin::Pin<&mut Capture<T>> {
    fn try_cast<U: NaviType>(&self) -> Option<&RPtr<U>> {
        (**self).try_cast()
    }

    fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        (**self).is_type(other_typeinfo)
    }
}
*/