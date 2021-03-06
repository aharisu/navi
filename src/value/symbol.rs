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
    None,
    None,
    None,
    None,
    None,
    None,
);

static GENSYM_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl NaviType for Symbol {
    fn typeinfo() -> &'static TypeInfo {
        &SYMBOL_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        Self::alloc(self.as_ref(), allocator)
    }
}

impl Symbol {
    fn size_of(&self) -> usize {
        string::NString::size_of(&self.inner)
    }

    pub fn alloc<T: Into<String>, A: Allocator>(str: T, allocator : &mut A) -> NResult<Symbol, OutOfMemory> {
        string::NString::alloc_inner::<Symbol, A>(&str.into(), allocator)
    }

    pub fn gensym<T :Into<String>, A: Allocator>(name: T, allocator: &mut A) -> NResult<Symbol, OutOfMemory> {
        let count = GENSYM_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = name.into() + "_" + &count.to_string();
        Self::alloc(&name, allocator)
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
        //StaticSymbol???Symbol??????????????????????????????????????????
        //??????????????????????????????Symbol????????????????????????????????????
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

pub(crate) fn symbol_static<T: Into<String>>(name: T) -> GCAllocationStruct<StaticSymbol> {
    let symbol = StaticSymbol {
        inner: string::static_string(name)
    };

    GCAllocationStruct::new_with_typeinfo(symbol, Symbol::typeinfo())
}