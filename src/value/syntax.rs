use crate::mm::{GCAllocationStruct};
use crate::eval::{eval, Context, self};
use crate::value::*;
use crate::value::list;
use crate::world::{World};
use std::fmt::Debug;
use std::panic;
use once_cell::sync::Lazy;

pub struct Syntax {
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&NBox<list::List>, &mut Context) -> NBox<Value>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    Syntax::eq,
    Syntax::fmt,
    Syntax::is_type,
);

impl NaviType for Syntax {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYNTAX_TYPEINFO as *const TypeInfo)
    }

}

impl Syntax {
    pub fn new(require: usize, optional: usize, has_rest: bool, body:  fn(&NBox<list::List>, &mut Context) -> NBox<Value>) -> Self {
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

    pub fn apply(&self, args: &NBox<list::List>, ctx: &mut crate::eval::Context) -> NBox<Value> {
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

fn syntax_if(args: &NBox<list::List>, ctx: &mut Context) -> NBox<Value> {
    let pred = args.as_ref().head_ref();
    let pred = eval(pred, ctx);

    let pred = if let Some(pred) = pred.as_ref().try_cast::<bool::Bool>() {
        pred.is_true()
    } else {
        panic!("boolean required. but got {:?}", pred)
    };

    let args = args.as_ref().tail_ref();
    if pred {
        let true_sexp = args.as_ref().head_ref();
        eval(true_sexp, ctx)

    } else {
        let args = args.as_ref().tail_ref();
        if args.as_ref().is_nil() {
            unit::Unit::unit().into_nboxvalue()
        } else {
            let false_sexp = args.as_ref().head_ref();
            eval(false_sexp, ctx)
        }
    }
}

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new(2, 1, false, syntax_if))
});

pub fn register_global(world: &mut World) {
    world.set("if", NBox::new(&SYNTAX_IF.value as *const Syntax as *mut Syntax).into_nboxvalue());
}