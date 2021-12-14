use crate::mm::{GCAllocationStruct};
use crate::eval::{eval, self};
use crate::{value::*, let_listbuilder, with_cap, let_cap, new_cap};
use crate::value::list;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::Debug;
use std::panic;
use once_cell::sync::Lazy;

pub struct Syntax {
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&RPtr<list::List>, &mut Context) -> FPtr<Value>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    Syntax::eq,
    Syntax::fmt,
    Syntax::is_type,
    None,
    None,
);

impl NaviType for Syntax {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYNTAX_TYPEINFO as *const TypeInfo)
    }

}

impl Syntax {
    pub fn new(require: usize, optional: usize, has_rest: bool, body:  fn(&RPtr<list::List>, &mut Context) -> FPtr<Value>) -> Self {
        Syntax {
            require: require,
            optional: optional,
            has_rest: has_rest,
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYNTAX_TYPEINFO, other_typeinfo)
    }

    pub fn check_arguments<T>(&self, args: &T) -> bool
    where
        T: AsReachable<list::List>
    {
        let args = args.as_reachable();
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn apply<T>(&self, args: &T, ctx: &mut Context) -> FPtr<Value>
    where
        T: AsReachable<list::List>
    {
        (self.body)(args.as_reachable(), ctx)
    }

    pub fn quote() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_QUOTE.value as *const Syntax as *mut Syntax)
    }
}

impl Eq for Syntax { }

impl PartialEq for Syntax {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Debug for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "syntax")
    }
}

fn syntax_if(args: &RPtr<list::List>, ctx: &mut Context) -> FPtr<Value> {
    let_cap!(pred, eval(args.as_ref().head_ref(), ctx), ctx);

    let pred = if let Some(pred) = pred.as_reachable().try_cast::<bool::Bool>() {
        pred.as_ref().is_true()
    } else {
        panic!("boolean required. but got {:?}", pred.as_ref())
    };

    let args = args.as_ref().tail_ref();
    if pred {
        eval(args.as_ref().head_ref(), ctx)

    } else {
        let args = args.as_ref().tail_ref();
        if args.as_ref().is_nil() {
            tuple::Tuple::unit().into_value().into_fptr()
        } else {
            eval(args.as_ref().head_ref(), ctx)
        }
    }
}

pub(crate) fn do_begin<T>(body: &T, ctx: &mut Context) -> FPtr<Value>
where
    T: AsReachable<list::List>,
{
    let body = body.as_reachable();

    let mut result = new_cap!(tuple::Tuple::unit().into_value().into_fptr(), ctx);
    for sexp in body.as_ref().iter() {
        let e = eval::eval(sexp, ctx);

        result = new_cap!(e, ctx);
        ctx.add_capture(result.cast_value_mut());
    }

    result.as_reachable().clone().into_fptr()
}

fn syntax_cond(args: &RPtr<list::List>, ctx: &mut Context) -> FPtr<Value> {
    for (sexp, info) in args.as_ref().iter_with_info() {
        if let Some(clause) = sexp.try_cast::<list::List>() {
            let test = clause.as_ref().head_ref();

            //最後の節のTESTがシンボルのelseの場合、無条件でbody部分を評価します
            if let Some(else_) = test.try_cast::<symbol::Symbol>() {
                if else_.as_ref().as_ref() == "else" && info.is_tail {
                    return do_begin(clause.as_ref().tail_ref(), ctx);
                }
            }

            //TEST式を評価
            let result = eval::eval(test, ctx);
            if let Some(result) = result.try_cast::<bool::Bool>() {
                //TESTの結果がtrueなら続く式を実行して結果を返す
                if result.as_ref().is_true() {
                    return do_begin(clause.as_ref().tail_ref(), ctx);
                }
            } else {
                panic!("boolean required. but got {:?}", result.as_ref());
            }

        } else {
            panic!("cond clause require list. but got {:?}", sexp.as_ref());
        }
    }

    tuple::Tuple::unit().into_value().into_fptr()
}

fn syntax_fun(args: &RPtr<list::List>, ctx: &mut Context) -> FPtr<Value> {
    let_listbuilder!(builder, ctx);

    //引数指定の内容を解析
    let params = args.as_ref().head_ref();
    if let Some(params) = params.try_cast::<list::List>() {
        //TODO :optionalと:rest引数の対応
        for p in params.as_ref().iter() {
            match p.try_cast::<symbol::Symbol>() {
                Some(sym) => {
                    builder.append(sym.cast_value(), ctx);
                }
                None => {
                    panic!("parameter require symbol. But got {:?}", p.as_ref())
                }
            }
        }
    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }

    let (list_ptr, size) = builder.get_with_size();
    let params_ptr = with_cap!(list, list_ptr, ctx, {
        array::Array::from_list(&list, Some(size), ctx)
    });
    let_cap!(params, params_ptr, ctx);
    let body = args.as_ref().tail_ref();

    closure::Closure::alloc(&params, body, ctx).into_value()
}

fn syntax_quote(args: &RPtr<list::List>, _ctx: &mut Context) -> FPtr<Value> {
    let sexp = args.as_ref().head_ref();
    sexp.clone().into_fptr()
}

fn syntax_and(args: &RPtr<list::List>, ctx: &mut Context) -> FPtr<Value> {
    for sexp in args.as_ref().iter() {
        let result = eval::eval(sexp, ctx);
        if let Some(result) = result.try_cast::<bool::Bool>() {
            if result.as_ref().is_false() {
                return bool::Bool::false_().into_fptr().into_value();
            }

        } else {
            panic!("boolean required. but got {:?}", result.as_ref());
        }
    }

    bool::Bool::true_().into_fptr().into_value()
}

fn syntax_or(args: &RPtr<list::List>, ctx: &mut Context) -> FPtr<Value> {
    for sexp in args.as_ref().iter() {
        let result = eval::eval(sexp, ctx);
        if let Some(result) = result.try_cast::<bool::Bool>() {
            if result.as_ref().is_true() {
                return bool::Bool::true_().into_fptr().into_value();
            }

        } else {
            panic!("boolean required. but got {:?}", result.as_ref());
        }
    }

    bool::Bool::false_().into_fptr().into_value()
}

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(2, 1, false, syntax_if))
});

static SYNTAX_COND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(0, 0, true, syntax_cond))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(1, 0, true, syntax_fun))
});

static SYNTAX_QUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(1, 0, false, syntax_quote))
});

static SYNTAX_AND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(0, 0, true, syntax_and))
});

static SYNTAX_OR: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(0, 0, true, syntax_or))
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("if", &RPtr::new(&SYNTAX_IF.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("cond", &RPtr::new(&SYNTAX_COND.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("fun", &RPtr::new(&SYNTAX_FUN.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("quote", &RPtr::new(&SYNTAX_QUOTE.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("and", &RPtr::new(&SYNTAX_AND.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("or", &RPtr::new(&SYNTAX_OR.value as *const Syntax as *mut Syntax).into_value());
}