use crate::eval::{Context};
use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Syntax {
    inner: func::Func,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    Syntax::eq,
    Syntax::fmt,
    Syntax::is_type,
);

impl NaviType for Syntax {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&SYNTAX_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }

}

impl Syntax {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYNTAX_TYPEINFO, other_typeinfo)
    }
}