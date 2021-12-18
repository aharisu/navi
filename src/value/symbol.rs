use crate::value::*;
use crate::context::{Context};
use crate::ptr::*;
use std::fmt::Debug;

#[derive(Eq, PartialEq, PartialOrd, Ord, Hash)]
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
    None,
    None,
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

    pub fn alloc(str: &String, ctx : &mut Context) -> FPtr<Symbol> {
        string::NString::alloc_inner::<Symbol>(str, ctx)
    }

}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

fn display(this: &Symbol, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", &(*this.inner.as_string()))
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

pub mod literal {
}