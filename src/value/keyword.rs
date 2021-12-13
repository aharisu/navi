use crate::value::*;
use crate::context::{Context};
use crate::ptr::*;
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Keyword {
    inner: string::NString
}


static KEYWORD_TYPEINFO: TypeInfo = new_typeinfo!(
    Keyword,
    "Keyword",
    Keyword::eq,
    Keyword::fmt,
    Keyword::is_type,
    None,
);

impl NaviType for Keyword {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&KEYWORD_TYPEINFO as *const TypeInfo)
    }
}

impl Keyword {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&KEYWORD_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(str: &String, ctx : &mut Context) -> FPtr<Keyword> {
        string::NString::alloc_inner::<Keyword>(str, ctx)
    }

}

impl AsRef<str> for Keyword {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}