use crate::{value::*, let_listbuilder, new_cap, with_cap, let_cap};
use crate::object::{Object, Capture};
use std::fmt::Debug;


pub struct Func {
    params: Vec<Param>,
    body:  fn(&Capture<array::Array>, &mut Object) -> NPtr<Value>,
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
    pub fn new(params: &[Param], body: fn(&Capture<array::Array>, &mut Object) -> NPtr<Value>) -> Func {
        Func {
            params: params.to_vec(),
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&FUNC_TYPEINFO, other_typeinfo)
    }

    //TODO 戻り値をboolからResultに変更。Errorには適切なエラー内容を含んだenum
    pub fn process_arguments_descriptor(&self, args: &Capture<list::List>, ctx: &mut Object) -> Option<NPtr<list::List>> {
        fn check_type(v: &NPtr<Value>, param: &Param) -> bool {
            v.is_type(param.typeinfo)
        }

        let_listbuilder!(builder, ctx);

        let mut args_iter = args.as_ref().iter();
        for (index, param) in self.params.iter().enumerate() {
            let arg = args_iter.next();

            match param.kind {
                ParamKind::Require => {
                    if let Some(arg) = arg {
                        if check_type(&arg, param) == false {
                            //型チェックエラー
                            return None;
                        } else {
                            //OK!!
                            with_cap!(v, arg.clone(), ctx, {
                                builder.append(&v, ctx);
                            });
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
                            with_cap!(v, arg.clone(), ctx, {
                                builder.append(&v, ctx);
                            });
                        }
                    } else {
                        //Optionalなパラメータに対応する引数がなければ
                        //Unit値をデフォルト値として設定
                        with_cap!(v, unit::Unit::unit().into_value(), ctx, {
                            builder.append(&v, ctx);
                        });
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
                                with_cap!(v, arg.clone(), ctx, {
                                    rest.append(&v, ctx);
                                });
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
                        with_cap!(v, list::List::nil().into_value(), ctx, {
                            builder.append(&v, ctx);
                        });
                    }
                }
            }
        }

        Some(builder.get())
    }

    pub fn apply(&self, args: &Capture<array::Array>, ctx: &mut Object) -> NPtr<Value> {
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
