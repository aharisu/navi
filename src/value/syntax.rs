use crate::mm::{GCAllocationStruct};
use crate::eval::{eval};
use crate::value::*;
use crate::value::list;
use crate::object::{Object};
use std::fmt::Debug;
use std::panic;
use once_cell::sync::Lazy;

pub struct Syntax {
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&NBox<list::List>, &mut Object) -> NPtr<Value>,
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
    pub fn new(require: usize, optional: usize, has_rest: bool, body:  fn(&NBox<list::List>, &mut Object) -> NPtr<Value>) -> Self {
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

    pub fn check_arguments(&self, args: &NBox<list::List>) -> bool {
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args: &NBox<list::List>, ctx: &mut Object) -> NPtr<Value> {
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

fn syntax_if(args: &NBox<list::List>, ctx: &mut Object) -> NPtr<Value> {
    let pred = NBox::new(args.as_ref().head_ref(), ctx);
    let pred = NBox::new(eval(&pred, ctx), ctx);

    let pred = if let Some(pred) = pred.as_ref().try_cast::<bool::Bool>() {
        pred.is_true()
    } else {
        panic!("boolean required. but got {:?}", pred)
    };

    let args = args.as_ref().tail_ref();
    if pred {
        //TODO GC Capture:
        let true_sexp = NBox::new(args.as_ref().head_ref(), ctx);
        eval(&true_sexp, ctx)

    } else {
        let args = args.as_ref().tail_ref();
        if args.as_ref().is_nil() {
            unit::Unit::unit().into_value()
        } else {
            //TODO GC Capture:
            let false_sexp = NBox::new(args.as_ref().head_ref(), ctx);
            eval(&false_sexp, ctx)
        }
    }
}

fn syntax_fun(args: &NBox<list::List>, ctx: &mut Object) -> NPtr<Value> {
    //TODO GC Capture: params_vec
    let mut params_vec: Vec<NBox<Value>> = Vec::new();

    //引数指定の内容を解析
    let params = args.as_ref().head_ref();
    if let Some(params) = params.try_cast::<list::List>() {
        //TODO :optionalと:rest引数の対応
        for p in params.as_ref().iter() {
            match p.try_cast::<symbol::Symbol>() {
                Some(sym) => {
                    params_vec.push(NBox::new(sym.cast_value().clone(), ctx));
                }
                None => {
                    panic!("parameter require symbol. But got {:?}", p.as_ref())
                }
            }
        }
    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }

    //GC Capture:
    let params = NBox::new( array::Array::from_slice(&params_vec, ctx), ctx);
    let body = NBox::new(args.as_ref().tail_ref(), ctx);

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