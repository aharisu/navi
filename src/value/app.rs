use crate::ptr::*;
use crate::value::*;

use std::fmt::Display;

pub static APP_EXTRATYPE_ID: ExtraTypeId = make_extratype_id();

#[repr(C)]
pub struct AppTypeInfo {
    pub base: ExtraTypeInfo,
    pub parameter_func: fn(&App) -> &Parameter,
    pub name_func: fn(&App) ->&str,
}

#[macro_export]
macro_rules! new_app_typeinfo {
    ($t:ty, $parameter_func:expr, $name_func:expr, ) => {
        AppTypeInfo {
            base: make_extra_typeinfo(&APP_EXTRATYPE_ID, None),
            parameter_func: unsafe { std::mem::transmute::<fn(&$t)->&app::Parameter, fn(&app::App)->&app::Parameter>($parameter_func) },
            name_func: unsafe { std::mem::transmute::<fn(&$t)->&str, fn(&app::App)->&str>($name_func) },
        }
    };
}

#[derive(Clone)]
pub struct Parameter {
    params: Vec<Param>,
    num_require: u8,
    num_optional: u8,
    has_rest: bool,
}

impl Parameter {
    pub fn new(params: &[Param]) -> Self {
        let mut num_require = 0;
        let mut num_optional = 0;
        let mut has_rest = false;
        params.iter().for_each(|p| {
            match p.kind {
                ParamKind::Require => num_require += 1,
                ParamKind::Optional => num_optional += 1,
                ParamKind::Rest => has_rest = true,
            }
        });

        Self {
            params: params.to_vec(),
            num_require,
            num_optional,
            has_rest,
        }
    }

    #[inline]
    pub fn params(&self) -> &[Param] {
        &self.params
    }

    #[inline]
    pub fn num_require(&self) -> usize {
        self.num_require as usize
    }

    #[inline]
    pub fn num_optional(&self) -> usize {
        self.num_optional as usize
    }

    #[inline]
    pub fn has_rest(&self) -> bool {
        self.has_rest
    }

}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParamKind {
    Require,
    Optional,
    Rest,
}

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub typeinfo: &'static TypeInfo,
    pub force: bool,
    pub kind: ParamKind,
    //TODO Optionalのデフォルト値
}

impl Param {
    pub fn new<T: Into<String>>(name: T, kind: ParamKind, typeinfo: &'static TypeInfo) -> Param {
        Param {
            name: name.into(),
            typeinfo: typeinfo,
            force: true,
            kind: kind,
        }
    }
    pub fn new_no_force<T: Into<String>>(name: T, kind: ParamKind, typeinfo: &'static TypeInfo) -> Param {
        Param {
            name: name.into(),
            typeinfo: typeinfo,
            force: false,
            kind: kind,
        }
    }
}

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

impl App {
    #[inline]
    fn get_app_typeinfo(&self) -> &AppTypeInfo {
        //selfはポインタであることがわかっているので、直接mmからTypeInfoを取得する
        let typeinfo = mm::get_typeinfo(self);
        let extra_typeinfo = typeinfo.extra_typeinfo.unwrap();
        debug_assert_eq!(extra_typeinfo.id, &APP_EXTRATYPE_ID);

        unsafe { std::mem::transmute(extra_typeinfo) }
    }

    pub fn parameter(&self) -> &Parameter {
        let app_typeinfo = self.get_app_typeinfo();
        (app_typeinfo.parameter_func)(self)
    }

    pub fn name(&self) -> &str {
        let app_typeinfo = self.get_app_typeinfo();
        (app_typeinfo.name_func)(self)
    }

}

impl Display for App {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}