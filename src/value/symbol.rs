use crate::value::*;
use crate::context::{Context};
use crate::ptr::*;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};

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

static GENSYM_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl NaviType for Symbol {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYMBOL_TYPEINFO as *const TypeInfo)
    }
}

impl Symbol {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYMBOL_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<T: Into<String>>(str: T, ctx : &mut Context) -> FPtr<Symbol> {
        string::NString::alloc_inner::<Symbol>(&str.into(), ctx)
    }

    pub fn gensym<T :Into<String>>(name: T, ctx: &mut Context) -> FPtr<Symbol> {
        let count = GENSYM_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = name.into() + "_" + &count.to_string();
        Self::alloc(&name, ctx)
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