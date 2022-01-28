use crate::value::*;
use crate::value::list::{self, List};
use crate::ptr::*;
use crate::err::*;
use crate::value::iform::IForm;
use crate::compile::SyntaxException;
use crate::compile;

use std::fmt::{Debug, Display};


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