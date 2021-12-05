use crate::value::*;
use crate::mm::{Heap};
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Symbol {
    inner: string::NString
}

static SYMBOL_TYPEINFO: TypeInfo<Symbol> = TypeInfo::<Symbol> {
    name: "Symbol",
    eq_func: Symbol::eq,
    print_func: Symbol::fmt,
    is_type_func: Symbol::is_type,
};

impl NaviType for Symbol { }

impl Symbol {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Symbol> {
        &SYMBOL_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
    }

    pub fn alloc(heap : &mut Heap, str: &String) -> NBox<Symbol> {
        string::NString::alloc_inner(heap, str, Self::typeinfo())
    }

}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}