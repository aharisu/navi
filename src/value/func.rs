use crate::eval::{Context, self};
use crate::value::*;
use std::fmt::Debug;


pub struct Func {
    params: Vec<Param>,
    body: fn(&[NBox<Value>], &mut Context) -> NBox<Value>,
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
    Func::eq,
    Func::fmt,
    Func::is_type,
);

impl NaviType for Func {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&FUNC_TYPEINFO as *const TypeInfo)
    }

}

impl Func {
    pub fn new(params: &[Param], body: fn(&[NBox<Value>], &mut Context) -> NBox<Value>) -> Func {
        Func {
            params: params.to_vec(),
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    //TODO 戻り値をboolからResultに変更。Errorには適切なエラー内容を含んだenum
    pub fn process_arguments_descriptor<'a>(&self, args: &mut Vec<NBox<Value>>, ctx: &mut eval::Context) -> bool {
        fn check_type(v: &NBox<Value>, param: &Param) -> bool {
            v.is_type(param.typeinfo)
        }

        for (index, param) in self.params.iter().enumerate() {
            match param.kind {
                ParamKind::Require => {
                    if args.len() <= index {
                        //引数の個数が足らない
                        return false;

                    } else if check_type(&args[index], param) == false {
                        //型チェックエラー
                        return false;
                    }
                }
                ParamKind::Optional => {
                    //Optionalなパラメータに対応する引数がなければ
                    if args.len() <= index {
                        //Unit値をデフォルト値として設定
                        args.push(unit::Unit::unit().into_nboxvalue());

                    } else if check_type(&args[index], param) == false {
                        //型チェックエラー
                        return false;
                    }
                }
                ParamKind::Rest => {
                    if args.len() <= index {
                        args.push(list::List::nil().into_nboxvalue());

                    } else {
                        let rest:Vec<_> = args.drain(index..).collect();
                        //型チェック
                        if rest.iter().all(|v| check_type(v, param)) == false {
                            return false;
                        }

                        let rest = list::List::from_vec(&mut ctx.heap, rest);
                        args.push(rest.into_nboxvalue());
                    }
                }
            }
        }

        true
    }

    pub fn apply(&self, args: &[NBox<Value>], ctx: &mut eval::Context) -> NBox<Value> {
        (self.body)(args, ctx)
    }
}

impl Eq for Func { }

impl PartialEq for Func{
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func")
    }
}
