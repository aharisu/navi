use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Symbol {
    inner: string::NString
}

static SYMBOL_TYPEINFO: TypeInfo = new_typeinfo!(
    Symbol,
    "Symbol",
    Symbol::eq,
    Symbol::fmt,
    Symbol::is_type,
);

impl NaviType for Symbol {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYMBOL_TYPEINFO as *const TypeInfo)
    }
}

impl Symbol {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYMBOL_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(heap : &mut Heap, str: &String) -> NBox<Symbol> {
        string::NString::alloc_inner::<Symbol>(heap, str)
    }

}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}