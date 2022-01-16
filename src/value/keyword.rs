use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};

#[derive(Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Keyword {
    inner: string::NString
}


static KEYWORD_TYPEINFO: TypeInfo = new_typeinfo!(
    Keyword,
    "Keyword",
    0,
    Some(Keyword::size_of),
    Keyword::eq,
    Keyword::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for Keyword {
    fn typeinfo() -> &'static TypeInfo {
        &KEYWORD_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        Self::alloc(self.as_ref(), allocator)
    }
}

impl Keyword {
    fn size_of(&self) -> usize {
        string::NString::size_of(&self.inner)
    }

    pub fn alloc<T: Into<String>, A: Allocator>(str: T, allocator : &mut A) -> NResult<Keyword, OutOfMemory> {
        string::NString::alloc_inner::<Keyword, A>(&str.into(), allocator)
    }

}

impl AsRef<str> for Keyword {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

fn display(this: &Keyword, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    this.inner.as_string().fmt(f)
}

impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl Debug for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}