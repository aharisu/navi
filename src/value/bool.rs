use crate::value::*;
use std::fmt::{self, Debug};

pub struct Bool {
}

impl NaviType for Bool {}

static BOOL_TYPEINFO: TypeInfo<Bool> = TypeInfo::<Bool> {
    name: "Bool",
    eq_func: Bool::eq,
    print_func: Bool::fmt,
};

impl Bool {
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<Bool> {
        &BOOL_TYPEINFO
    }

    #[inline(always)]
    pub fn true_() -> NBox<Bool> {
        NBox::<Bool>::new_immidiate(IMMIDATE_TRUE)
    }

    #[inline(always)]
    pub fn false_() -> NBox<Bool> {
        NBox::<Bool>::new_immidiate(IMMIDATE_FALSE)
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
        write!(f, "{}", if std::ptr::eq(self as *const Bool, IMMIDATE_TRUE as *const Bool) {
                "true"
            } else {
                "false"
            })
    }
}
