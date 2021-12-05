use crate::value::*;
use std::fmt::{self, Debug};

pub struct Bool {
}

impl NaviType for Bool {}

static BOOL_TYPEINFO: TypeInfo<Bool> = TypeInfo::<Bool> {
    name: "Bool",
    eq_func: Bool::eq,
    print_func: Bool::fmt,
    is_type_func: Bool::is_type,
};

impl Bool {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Bool> {
        &BOOL_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
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
