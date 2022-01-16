use crate::ptr::*;
use crate::value::*;

use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub struct App { }

//App型は抽象型なので実際にアロケーションされることはない。
//APP_TYPEINFOは型チェックのためだけに使用される。
static APP_TYPEINFO : TypeInfo = new_typeinfo!(
    App,
    "App",
    0, None,
    App::eq,
    App::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for App {
    fn typeinfo() -> &'static TypeInfo {
        &APP_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //App型のインスタンスは存在しないため、cloneが呼ばれることはない。
        unreachable!()
    }
}

impl Display for App {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}