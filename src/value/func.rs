use crate::value::*;
use crate::ptr::*;
use crate::err::*;
use crate::new_app_typeinfo;
use crate::value::app::{AppTypeInfo, APP_EXTRATYPE_ID};

use std::fmt::{Debug, Display};

pub struct Func {
    name: String,
    body:  fn(num_rest: usize, &mut Object) -> NResult<Any, Exception>,
    parameter: app::Parameter,
}

static FUNC_APP_EXTRATYPEINFO: app::AppTypeInfo = new_app_typeinfo!(
    Func,
    Func::parameter,
    Func::name,
);

static FUNC_TYPEINFO: TypeInfo = new_typeinfo!(
    Func,
    "Func",
    std::mem::size_of::<Func>(),
    None,
    Func::eq,
    Func::clone_inner,
    Display::fmt,
    Some(Func::is_type),
    None,
    None,
    None,
    None,
    Some(&FUNC_APP_EXTRATYPEINFO.base),
);

impl NaviType for Func {
    fn typeinfo() -> &'static TypeInfo {
        &FUNC_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //Funcのインスタンスはヒープ上に作られることがないため、自分自身を返す
        Ok(Ref::new(self))
    }
}

impl Func {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &FUNC_TYPEINFO == other_typeinfo
        || app::App::typeinfo() == other_typeinfo
    }

    pub fn new<T: Into<String>>(name: T, body: fn(num_rest: usize, &mut Object) -> NResult<Any, Exception>, parameter: app::Parameter) -> Func {

        Func {
            name: name.into(),
            body: body,
            parameter,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn parameter(&self) -> &app::Parameter {
        &self.parameter
    }

    pub fn apply(&self, num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
        (self.body)(num_rest, obj)
    }
}

impl Ref<Func> {
    pub fn cast_app(&self) -> &Ref<app::App> {
        unsafe { std::mem::transmute(self) }
    }
}

impl Reachable<Func> {
    pub fn cast_app(&self) -> &Reachable<app::App> {
        unsafe { std::mem::transmute(self) }
    }
}

impl Eq for Func { }

impl PartialEq for Func{
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Display for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
