use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};


pub struct Func {
    name: String,
    params: Vec<Param>,
    body:  fn(&mut Object) -> FPtr<Value>,
    num_require: u8,
    num_optional: u8,
    has_rest: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParamKind {
    Require,
    Optional,
    Rest,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub typeinfo: NonNullConst<TypeInfo>,
    pub force: bool,
    pub kind: ParamKind,
    //TODO Optionalのデフォルト値
}

impl Param {
    pub fn new<T: Into<String>>(name: T, kind: ParamKind, typeinfo: NonNullConst<TypeInfo>) -> Param {
        Param {
            name: name.into(),
            typeinfo: typeinfo,
            force: true,
            kind: kind,
        }
    }
    pub fn new_no_force<T: Into<String>>(name: T, kind: ParamKind, typeinfo: NonNullConst<TypeInfo>) -> Param {
        Param {
            name: name.into(),
            typeinfo: typeinfo,
            force: false,
            kind: kind,
        }
    }
}

static FUNC_TYPEINFO: TypeInfo = new_typeinfo!(
    Func,
    "Func",
    std::mem::size_of::<Func>(),
    None,
    Func::eq,
    Func::clone_inner,
    Display::fmt,
    Func::is_type,
    None,
    None,
    None,
    None,
);

impl NaviType for Func {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&FUNC_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _allocator: &AnyAllocator) -> FPtr<Self> {
        //Funcのインスタンスはヒープ上に作られることがないため、自分自身を返す
        FPtr::new(self)
    }
}

impl Func {

    pub fn new<T: Into<String>>(name: T, params: &[Param], body: fn(&mut Object) -> FPtr<Value>) -> Func {
        let mut num_require = 0;
        let mut num_optional = 0;
        let mut has_rest = false;
        params.iter().for_each(|p| {
            match p.kind {
                func::ParamKind::Require => num_require += 1,
                func::ParamKind::Optional => num_optional += 1,
                func::ParamKind::Rest => has_rest = true,
            }
        });

        Func {
            name: name.into(),
            params: params.to_vec(),
            body: body,
            num_require,
            num_optional,
            has_rest,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    pub fn get_paramter(&self) -> &[Param] {
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

    pub fn apply(&self, obj: &mut Object) -> FPtr<Value> {
        (self.body)(obj)
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
