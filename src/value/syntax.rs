use crate::mm::{GCAllocationStruct};
use crate::eval::{eval};
use crate::{value::*, let_listbuilder, with_cap, let_cap, new_cap};
use crate::value::list;
use crate::object::{Object, Capture};
use std::fmt::Debug;
use std::panic;
use once_cell::sync::Lazy;

pub struct Syntax {
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&Capture<list::List>, &mut Object) -> NPtr<Value>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    Syntax::eq,
    Syntax::fmt,
    Syntax::is_type,
    None,
);

impl NaviType for Syntax {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYNTAX_TYPEINFO as *const TypeInfo)
    }

}

impl Syntax {
    pub fn new(require: usize, optional: usize, has_rest: bool, body:  fn(&Capture<list::List>, &mut Object) -> NPtr<Value>) -> Self {
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

    pub fn check_arguments(&self, args: &Capture<list::List>) -> bool {
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args: &Capture<list::List>, ctx: &mut Object) -> NPtr<Value> {
        (self.body)(args, ctx)
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

fn syntax_if(args: &Capture<list::List>, ctx: &mut Object) -> NPtr<Value> {
    let pred_ptr = with_cap!(pred, args.as_ref().head_ref(), ctx, {
        eval(&pred, ctx)
    });
    let_cap!(pred, pred_ptr, ctx);

    let pred = if let Some(pred) = pred.as_ref().try_cast::<bool::Bool>() {
        pred.as_ref().is_true()
    } else {
        panic!("boolean required. but got {:?}", pred)
    };

    let args = args.as_ref().tail_ref();
    if pred {
        with_cap!(true_clause, args.as_ref().head_ref(), ctx, {
            eval(&true_clause, ctx)
        })

    } else {
        let args = args.as_ref().tail_ref();
        if args.as_ref().is_nil() {
            unit::Unit::unit().into_value()
        } else {
            with_cap!(false_clause, args.as_ref().head_ref(), ctx, {
                eval(&false_clause, ctx)
            })
        }
    }
}

fn syntax_fun(args: &Capture<list::List>, ctx: &mut Object) -> NPtr<Value> {
    let_listbuilder!(builder, ctx);

    //引数指定の内容を解析
    let params = args.as_ref().head_ref();
    if let Some(params) = params.try_cast::<list::List>() {
        //TODO :optionalと:rest引数の対応
        for p in params.as_ref().iter() {
            match p.try_cast::<symbol::Symbol>() {
                Some(sym) => {
                    with_cap!(sym, sym.cast_value().clone(), ctx, {
                        builder.append(&sym, ctx);
                    });
                }
                None => {
                    panic!("parameter require symbol. But got {:?}", p.as_ref())
                }
            }
        }
    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }

    let (list, size) = builder.get_with_size();
    let params_ptr = with_cap!(list, list, ctx, {
        array::Array::from_list(&list, Some(size), ctx)
    });
    let_cap!(params, params_ptr, ctx);
    let_cap!(body, args.as_ref().tail_ref(), ctx);

    closure::Closure::alloc(&params, &body, ctx).into_value()
}

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(2, 1, false, syntax_if))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(1, 0, true, syntax_fun))
});

pub fn register_global(ctx: &mut Object) {
    ctx.define_value("if", &NPtr::new(&SYNTAX_IF.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("fun", &NPtr::new(&SYNTAX_FUN.value as *const Syntax as *mut Syntax).into_value());
}