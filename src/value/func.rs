use crate::eval::{Context};
use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;


pub struct Func {
    params: Vec<Param>,
    body: fn(&[NBox<Value>], &mut Context) -> NBox<Value>,
}

#[derive(Debug)]
pub enum ParamKind {
    Require,
    Optional,
    Rest,
}

#[derive(Debug)]
pub struct Param {
    name: Option<Box<str>>,
    t: NonNull<TypeInfo>,
    kind: ParamKind,
}

static FUNC_TYPEINFO: TypeInfo = new_typeinfo!(
    Func,
    "Func",
    Func::eq,
    Func::fmt,
    Func::is_type,
);

impl NaviType for Func {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&FUNC_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }

}

impl Func {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }
}

impl Eq for Func { }

impl PartialEq for Func{
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func")
    }
}
