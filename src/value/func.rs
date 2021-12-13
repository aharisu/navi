use crate::{value::*, let_listbuilder, new_cap, with_cap, let_cap};
use crate::context::Context;
use crate::ptr::*;
use std::fmt::Debug;


pub struct Func {
    params: Vec<Param>,
    body:  fn(&RPtr<array::Array>, &mut Context) -> FPtr<Value>,
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
    None,
);

impl NaviType for Func {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&FUNC_TYPEINFO as *const TypeInfo)
    }

}

impl Func {
    pub fn new(params: &[Param], body: fn(&RPtr<array::Array>, &mut Context) -> FPtr<Value>) -> Func {
        Func {
            params: params.to_vec(),
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    //TODO 戻り値をboolからResultに変更。Errorには適切なエラー内容を含んだenum
    pub fn process_arguments_descriptor<T>(&self, args: &T, ctx: &mut Context) -> Option<FPtr<list::List>>
    where
        T: AsReachable<list::List>
    {
        let args = args.as_reachable();
        fn check_type(v: &RPtr<Value>, param: &Param) -> bool {
            v.is_type(param.typeinfo)
        }

        let_listbuilder!(builder, ctx);

        let mut args_iter = args.as_ref().iter();
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
                            builder.append(arg, ctx);
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
                            builder.append(arg, ctx);
                        }
                    } else {
                        //Optionalなパラメータに対応する引数がなければ
                        //Unit値をデフォルト値として設定
                        builder.append(tuple::Tuple::unit().cast_value(), ctx);
                    }
                }
                ParamKind::Rest => {
                    if let Some(arg) = arg {
                        let_listbuilder!(rest, ctx);
                        let mut arg = arg;
                        loop {
                            if check_type(&arg, param) == false {
                                //型チェックエラー
                                return None;
                            } else {
                                //OK!!
                                rest.append(arg, ctx);
                            }

                            match args_iter.next() {
                                Some(a) => arg = a,
                                None => break
                            }
                        }

                        with_cap!(v, rest.get().into_value(), ctx, {
                            builder.append(&v, ctx);
                        });
                    } else {
                        //restパラメータに対応する引数がなければ
                        //nilをデフォルト値として設定
                        builder.append(list::List::nil().cast_value(), ctx);
                    }
                }
            }
        }

        Some(builder.get())
    }

    pub fn apply<T>(&self, args: &T, ctx: &mut Context) -> FPtr<Value>
    where
        T: AsReachable<array::Array>,
    {
        (self.body)(args.as_reachable(), ctx)
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
