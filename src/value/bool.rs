use crate::value::*;
use crate::ptr::RPtr;
use std::fmt::{self, Debug, Display};

pub struct Bool {
}

static BOOL_TYPEINFO: TypeInfo = new_typeinfo!(
    Bool,
    "Bool",
    Bool::eq,
    Display::fmt,
    Bool::is_type,
    None,
    None,
    None,
);

impl NaviType for Bool {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&BOOL_TYPEINFO as *const TypeInfo)
    }
}


impl Bool {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&BOOL_TYPEINFO, other_typeinfo)
    }

    #[inline(always)]
    pub fn true_() -> RPtr<Bool> {
        RPtr::<Bool>::new_immidiate(IMMIDATE_TRUE)
    }

    #[inline(always)]
    pub fn false_() -> RPtr<Bool> {
        RPtr::<Bool>::new_immidiate(IMMIDATE_FALSE)
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

fn display(this: &Bool, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", if this.is_true() {
            "true"
        } else {
            "false"
        })
}

impl Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}

impl Debug for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
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
