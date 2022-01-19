use crate::value::*;
use crate::value::list::{self, List};
use crate::ptr::*;
use crate::err::*;
use crate::value::iform::IForm;
use crate::compile::SyntaxException;
use crate::object::mm::GCAllocationStruct;
use crate::compile;

use std::fmt::{Debug, Display};
use once_cell::sync::Lazy;


pub mod r#match;

pub struct Syntax {
    name: String,
    require: usize,
    optional: usize,
    has_rest: bool,
    transform_body: fn(&Reachable<List>, &mut compile::CCtx, &mut Object) -> NResult<IForm, SyntaxException>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    std::mem::size_of::<Syntax>(),
    None,
    Syntax::eq,
    Syntax::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for Syntax {
    fn typeinfo() -> &'static TypeInfo {
        &SYNTAX_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //Syntaxのインスタンスはヒープ上に作られることがないため、自分自身を返す
        Ok(Ref::new(self))
    }
}

impl Syntax {

    pub fn new<T: Into<String>>(name: T, require: usize, optional: usize, has_rest: bool
        , translate_body: fn(&Reachable<list::List>, &mut crate::compile::CCtx, &mut Object) -> NResult<IForm, SyntaxException>,
    ) -> Self {
        Syntax {
            name: name.into(),
            require: require,
            optional: optional,
            has_rest: has_rest,
            transform_body: translate_body,
        }
    }

    pub fn check_arguments(&self, args: &Reachable<list::List>) -> bool {
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn transform(&self, args: &Reachable<List>, ctx: &mut compile::CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
        (self.transform_body)(args, ctx, obj)
    }

}

impl Eq for Syntax { }

impl PartialEq for Syntax {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Display for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("if", 2, 1, false, compile::syntax_if))
});

static SYNTAX_BEGIN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("begin", 0, 0, true, compile::syntax_begin))
});

static SYNTAX_COND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("cond", 0, 0, true, compile::syntax_cond))
});

static SYNTAX_DEF_RECV: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("def-recv", 1, 0, true, compile::syntax_def_recv))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("fun", 1, 0, true, compile::syntax_fun))
});

static SYNTAX_LOCAL: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("local", 1, 0, true, compile::syntax_local))
});

static SYNTAX_LET: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let", 2, 0, false, compile::syntax_let))
});

static SYNTAX_LET_GLOBAL: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let-global", 2, 0, false, compile::syntax_let_global))
});

static SYNTAX_QUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("quote", 1, 0, false, compile::syntax_quote))
});

static SYNTAX_UNQUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("unquote", 1, 0, false, compile::syntax_unquote))
});

static SYNTAX_BIND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("bind", 1, 0, false, compile::syntax_bind))
});

static SYNTAX_MATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("match", 1, 0, true, compile::syntax_match))
});

static SYNTAX_AND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("and", 0, 0, true, compile::syntax_and))
});

static SYNTAX_OR: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("or", 0, 0, true, compile::syntax_or))
});

static SYNTAX_OBJECT_SWITCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("object-switch", 1, 0, false, compile::syntax_object_switch))
});

static SYNTAX_RETURN_OBJECT_SWITCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("return-object-switch", 0, 0, false, compile::syntax_return_object_switch))
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("if", &Ref::new(&SYNTAX_IF.value));
    obj.define_global_value("begin", &Ref::new(&SYNTAX_BEGIN.value));
    obj.define_global_value("cond", &Ref::new(&SYNTAX_COND.value));
    obj.define_global_value("def-recv", &Ref::new(&SYNTAX_DEF_RECV.value));
    obj.define_global_value("fun", &Ref::new(&SYNTAX_FUN.value));
    obj.define_global_value("local", &Ref::new(&SYNTAX_LOCAL.value));
    obj.define_global_value("let", &Ref::new(&SYNTAX_LET.value));
    obj.define_global_value("let-global", &Ref::new(&SYNTAX_LET_GLOBAL.value));
    obj.define_global_value("quote", &Ref::new(&SYNTAX_QUOTE.value));
    obj.define_global_value("unquote", &Ref::new(&SYNTAX_UNQUOTE.value));
    obj.define_global_value("bind", &Ref::new(&SYNTAX_BIND.value));
    obj.define_global_value("match", &Ref::new(&SYNTAX_MATCH.value));
    obj.define_global_value("and", &Ref::new(&SYNTAX_AND.value));
    obj.define_global_value("or", &Ref::new(&SYNTAX_OR.value));
    obj.define_global_value("object-switch", &Ref::new(&SYNTAX_OBJECT_SWITCH.value));
    obj.define_global_value("return-object-switch", &Ref::new(&SYNTAX_RETURN_OBJECT_SWITCH.value));
}

pub mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn quote() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_QUOTE.value)
    }

    pub fn unquote() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_UNQUOTE.value)
    }

    pub fn bind() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_BIND.value)
    }

    pub fn fun() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_FUN.value)
    }

    pub fn local() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_LOCAL.value)
    }

    pub fn let_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_LET.value)
    }

    pub fn match_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_MATCH.value)
    }

    pub fn if_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_IF.value)
    }

    pub fn begin() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_BEGIN.value)
    }

    pub fn cond() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_COND.value)
    }
}