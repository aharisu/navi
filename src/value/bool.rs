use crate::value::*;
use std::fmt::{self, Debug};

pub struct Bool {
}

static BOOL_TYPEINFO: TypeInfo = new_typeinfo!(
    Bool,
    "Bool",
    Bool::eq,
    Bool::fmt,
    Bool::is_type,
);

impl NaviType for Bool {
    fn typeinfo() -> NonNull<TypeInfo> {
        unsafe { NonNull::new_unchecked(&BOOL_TYPEINFO as *const TypeInfo as *mut TypeInfo) }
    }
}


impl Bool {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&BOOL_TYPEINFO, other_typeinfo)
    }

    #[inline(always)]
    pub fn true_() -> NBox<Bool> {
        NBox::<Bool>::new_immidiate(IMMIDATE_TRUE)
    }

    #[inline(always)]
    pub fn false_() -> NBox<Bool> {
        NBox::<Bool>::new_immidiate(IMMIDATE_FALSE)
    }

    #[inline(always)]
    pub fn is_true(&self) -> bool {
        std::ptr::eq(self as *const Bool, IMMIDATE_TRUE as *const Bool)
    }

    #[inline(always)]
    pub fn is_false(&self) -> bool {
        !self.is_true()
    }

}

impl Eq for Bool {}

impl PartialEq for Bool {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Bool, other as *const Bool)
    }
}

impl Debug for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", if self.is_true() {
                "true"
            } else {
                "false"
            })
    }
}

impl std::hash::Hash for Bool {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.is_true() {
            true.hash(state);
        } else {
            false.hash(state);
        }
    }
}
