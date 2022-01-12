use crate::value::*;
use crate::ptr::*;
use std::fmt::{self, Debug, Display};

pub struct Bool {
}

static BOOL_TYPEINFO: TypeInfo = new_typeinfo!(
    Bool,
    "Bool",
    0, None, //Immidiate Valueだけなのでサイズは常に0
    Bool::eq,
    Bool::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for Bool {
    fn typeinfo() -> &'static TypeInfo {
        &BOOL_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> Ref<Self> {
        //Bool型の値は常にImmidiate Valueなのでそのまま返す
        Ref::new(self)
    }
}


impl Bool {

    #[inline(always)]
    pub fn true_() -> Reachable<Bool> {
        Reachable::<Bool>::new_immidiate(IMMIDATE_TRUE)
    }

    #[inline(always)]
    pub fn false_() -> Reachable<Bool> {
        Reachable::<Bool>::new_immidiate(IMMIDATE_FALSE)
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
