use crate::value::*;
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
    0,
    Some(Symbol::size_of),
    Symbol::eq,
    Symbol::clone_inner,
    Symbol::fmt,
    Symbol::is_type,
    None,
    None,
    None,
);

static GENSYM_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl NaviType for Symbol {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYMBOL_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(this: &RPtr<Self>, obj: &mut Object) -> FPtr<Self> {
        Self::alloc(this.as_ref().as_ref(), obj)
    }
}

impl Symbol {
    fn size_of(&self) -> usize {
        string::NString::size_of(&self.inner)
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYMBOL_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<T: Into<String>>(str: T, obj : &mut Object) -> FPtr<Symbol> {
        string::NString::alloc_inner::<Symbol>(&str.into(), obj)
    }

    pub fn gensym<T :Into<String>>(name: T, obj: &mut Object) -> FPtr<Symbol> {
        let count = GENSYM_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = name.into() + "_" + &count.to_string();
        Self::alloc(&name, obj)
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

#[allow(dead_code)]
#[repr(transparent)]
pub(crate) struct StaticSymbol {
    inner: string::StaticString,
}

impl AsRef<Symbol> for StaticSymbol {
    fn as_ref(&self) -> &Symbol {
        //StaticSymbolとSymbolは同じメモリレイアウトなので
        //無理やりキャストしてSymbolとして扱えるようにする。
        unsafe {
            std::mem::transmute(self)
        }
    }
}

pub(crate) fn gensym_static<T: Into<String>>(name: T) -> GCAllocationStruct<StaticSymbol> {
    let count = GENSYM_COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = name.into() + "_" + &count.to_string();

    let symbol = StaticSymbol {
        inner: string::static_string(name)
    };

    GCAllocationStruct::new_with_typeinfo(symbol, Symbol::typeinfo())
}
