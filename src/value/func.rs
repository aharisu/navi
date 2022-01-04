use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};


pub struct Func {
    name: String,
    params: Vec<Param>,
    body:  fn(&mut Object) -> FPtr<Value>,
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
    pub kind: ParamKind,
    //TODO Optionalのデフォルト値
}

impl Param {
    pub fn new<T: Into<String>>(name: T, kind: ParamKind, typeinfo: NonNullConst<TypeInfo>) -> Param {
        Param {
            name: name.into(),
            typeinfo: typeinfo,
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
);

impl NaviType for Func {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&FUNC_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _obj: &mut Object) -> FPtr<Self> {
        //Funcのインスタンスはヒープ上に作られることがないため、自分自身を返す
        FPtr::new(self)
    }
}

impl Func {

    pub fn new<T: Into<String>>(name: T, params: &[Param], body: fn(&mut Object) -> FPtr<Value>) -> Func {
        Func {
            name: name.into(),
            params: params.to_vec(),
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    pub fn get_paramter(&self) -> &[Param] {
        &self.params
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
