use crate::{eval, with_cap, let_cap, new_cap};
use crate::object::Capture;
use crate::value::*;
use crate::object::{Object};
use std::fmt::Debug;


pub struct Closure {
    params: NPtr<array::Array>,
    body: NPtr<list::List>,
}

static CLOSURE_TYPEINFO: TypeInfo = new_typeinfo!(
    Closure,
    "Closure",
    Closure::eq,
    Closure::fmt,
    Closure::is_type,
    Some(Closure::child_traversal),
);

impl NaviType for Closure {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&CLOSURE_TYPEINFO as *const TypeInfo)
    }
}

impl Closure {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&CLOSURE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: usize, callback: fn(&NPtr<Value>, arg: usize)) {
        callback(self.params.cast_value(), arg);
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc(params: &Capture<array::Array>, body: &Capture<list::List>, ctx: &mut Object) -> NPtr<Self> {
        let mut nbox = ctx.alloc::<Closure>();
        nbox.as_mut().params = NPtr::new(params.as_mut_ptr());
        nbox.as_mut().body = NPtr::new(body.as_mut_ptr());

        nbox
    }

    pub fn process_arguments_descriptor(&self, args: &Capture<list::List>, ctx: &mut Object) -> bool {
        let count = args.as_ref().count();
        if count < self.params.as_ref().len() {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args: &Capture<array::Array>, ctx: &mut Object) -> NPtr<Value> {

        //ローカルフレームを構築
        let mut frame = Vec::<(&NPtr<symbol::Symbol>, &NPtr<Value>)>::new();

        let iter1 = self.params.as_ref().iter();
        let iter2 = args.as_ref().iter();

        let iter = iter1.zip(iter2);
        for (sym, v) in iter {
            let sym = unsafe { sym.cast_unchecked::<symbol::Symbol>() };
            frame.push((sym, v));
        }

        //ローカルフレームを環境にプッシュ
        ctx.push_local_frame(&frame);

        //Closure本体を実行
        let mut result = new_cap!(unit::Unit::unit().into_value(), ctx);
        for sexp in self.body.as_ref().iter() {
            let e = with_cap!(sexp, sexp.clone(), ctx, {
                eval::eval(&sexp, ctx)
            });

            result = new_cap!(e, ctx);
            ctx.add_capture(&mut result);
        }

        //ローカルフレームを環境からポップ
        ctx.pop_local_frame();

        result.nptr().clone()
    }

}


impl Eq for Closure { }

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "closure")
    }
}
