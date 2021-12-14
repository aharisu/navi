use crate::{eval, new_cap};
use crate::value::*;
use crate::ptr::*;
use crate::context::{Context};
use std::fmt::Debug;


pub struct Closure {
    params: RPtr<array::Array>,
    body: RPtr<list::List>,
}

static CLOSURE_TYPEINFO: TypeInfo = new_typeinfo!(
    Closure,
    "Closure",
    Closure::eq,
    Closure::fmt,
    Closure::is_type,
    None,
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

    fn child_traversal(&self, arg: usize, callback: fn(&RPtr<Value>, arg: usize)) {
        callback(self.params.cast_value(), arg);
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc<T, U>(params: &T, body: &U, ctx: &mut Context) -> FPtr<Self>
    where
        T: AsReachable<array::Array>,
        U: AsReachable<list::List>,
    {
        let mut ptr = ctx.alloc::<Closure>();
        let closure = unsafe { ptr.as_mut() };
        closure.params = params.as_reachable().clone();
        closure.body = body.as_reachable().clone();

        ptr.into_fptr()
    }

    pub fn process_arguments_descriptor<T>(&self, args: &T, _ctx: &mut Context) -> bool
    where
        T: AsReachable<list::List>
    {
        let count = args.as_reachable().as_ref().count();
        if count < self.params.as_ref().len() {
            false
        } else {
            true
        }
    }

    pub fn apply<T>(&self, args: &T, ctx: &mut Context) -> FPtr<Value>
    where
        T: AsReachable<array::Array>,
    {
        //ローカルフレームを構築
        let mut frame = Vec::<(&RPtr<symbol::Symbol>, &RPtr<Value>)>::new();

        let iter1 = self.params.as_ref().iter();
        let iter2 = args.as_reachable().as_ref().iter();

        let iter = iter1.zip(iter2);
        for (sym, v) in iter {
            let sym = unsafe { sym.cast_unchecked::<symbol::Symbol>() };
            frame.push((sym, v));
        }

        //ローカルフレームを環境にプッシュ
        ctx.push_local_frame(&frame);

        //Closure本体を実行
        let result = syntax::do_begin(&self.body, ctx);

        //ローカルフレームを環境からポップ
        ctx.pop_local_frame();

        result
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
