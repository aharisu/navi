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
    Keyword::is_type,
    None,
    None,
    None,
);

impl NaviType for Keyword {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&KEYWORD_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        Self::alloc(self.as_ref(), obj)
    }
}

impl Keyword {
    fn size_of(&self) -> usize {
        string::NString::size_of(&self.inner)
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&KEYWORD_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<T: Into<String>>(str: T, obj : &mut Object) -> FPtr<Keyword> {
        string::NString::alloc_inner::<Keyword>(&str.into(), obj)
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