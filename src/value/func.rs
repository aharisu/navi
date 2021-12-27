use crate::cap_append;
use crate::value::*;
use crate::ptr::*;
use crate::value::list::ListBuilder;
use std::fmt::{Debug, Display};

use super::array::Array;


pub struct Func {
    name: String,
    params: Vec<Param>,
    body:  fn(&Reachable<array::Array>, &mut Object) -> FPtr<Value>,
}

#[derive(Debug, Copy, Clone)]
pub enum ParamKind {
    Require,
    Optional,
    Rest,
}

#[derive(Debug, Clone)]
pub struct Param {
    name: String,
    typeinfo: NonNullConst<TypeInfo>,
    kind: ParamKind,
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

    pub fn new<T: Into<String>>(name: T, params: &[Param], body: fn(&Reachable<array::Array>, &mut Object) -> FPtr<Value>) -> Func {
        Func {
            name: name.into(),
            params: params.to_vec(),
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    //TODO 戻り値をboolからResultに変更。Errorには適切なエラー内容を含んだenum
    pub fn process_arguments_descriptor(&self, args: &Reachable<list::List>, obj: &mut Object) -> Option<FPtr<list::List>> {
        fn check_type(v: &FPtr<Value>, param: &Param) -> bool {
            v.is_type(param.typeinfo)
        }

        let mut builder = ListBuilder::new(obj);

        let mut args_iter = args.iter(obj);
        for param in self.params.iter() {
            let arg = args_iter.next();

            match param.kind {
                ParamKind::Require => {
                    if let Some(arg) = arg {
                        if check_type(&arg, param) == false {
                            //型チェックエラー
                            return None;
                        } else {
                            //OK!!
                            builder.append(&arg.reach(obj), obj);
                        }
                    } else {
                        //必須の引数が足らないエラー
                        return None;
                    }
                }
                ParamKind::Optional => {
                    if let Some(arg) = arg {
                        if check_type(&arg, param) == false {
                            //型チェックエラー
                            return None;
                        } else {
                            //OK!!
                            builder.append(&arg.reach(obj), obj);
                        }
                    } else {
                        //Optionalなパラメータに対応する引数がなければ
                        //Unit値をデフォルト値として設定
                        builder.append(tuple::Tuple::unit().cast_value(), obj);
                    }
                }
                ParamKind::Rest => {
                    if let Some(arg) = arg {
                        let mut rest = ListBuilder::new(obj);

                        let mut arg = arg;
                        loop {
                            if check_type(&arg, param) == false {
                                //型チェックエラー
                                return None;
                            } else {
                                //OK!!

                                rest.append(&arg.reach(obj), obj);
                            }

                            match args_iter.next() {
                                Some(next) => {
                                    arg = next;
                                }
                                None => break
                            }
                        }

                        cap_append!(builder, rest.get().into_value(), obj);
                    } else {
                        //restパラメータに対応する引数がなければ
                        //nilをデフォルト値として設定
                        builder.append(list::List::nil().cast_value(), obj);
                    }
                }
            }
        }

        Some(builder.get())
    }

    pub fn apply(&self, args: &Reachable<Array>, obj: &mut Object) -> FPtr<Value> {
        (self.body)(args, obj)
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
