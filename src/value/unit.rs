use crate::value::*;
use crate::ptr::*;
use std::fmt::{self, Debug};

pub struct Unit {
}

static UNIT_TYPEINFO: TypeInfo = new_typeinfo!(
    Unit,
    "Unit",
    Unit::eq,
    Unit::fmt,
    Unit::is_type,
    None,
);

impl NaviType for Unit {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&UNIT_TYPEINFO as *const TypeInfo)
    }
}


impl Unit {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&UNIT_TYPEINFO, other_typeinfo)
    }

    #[inline(always)]
    pub fn unit() -> RPtr<Unit> {
        RPtr::<Unit>::new_immidiate(IMMIDATE_UNIT)
    }

    #[inline(always)]
    pub fn is_unit(&self) -> bool {
        std::ptr::eq(self as *const Self, IMMIDATE_UNIT as *const Self)
    }
}

impl Eq for Unit {}

impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#unit")
    }
}
