use crate::mm::{Heap};
use crate::eval::{Context, self};
use crate::value::*;
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

    pub fn alloc(heap: &mut Heap, params: &NBox<array::Array>, body: &NBox<list::List>) -> NBox<Self> {
        let mut nbox = heap.alloc::<Closure>();
        nbox.as_mut_ref().params = NPtr::new(params.as_mut_ptr());
        nbox.as_mut_ref().body = NPtr::new(body.as_mut_ptr());

        nbox
    }

    pub fn process_arguments_descriptor(&self, args: &mut Vec<NBox<Value>>, ctx: &mut eval::Context) -> bool {
        let count = args.len();
        if count < self.params.as_ref().len() {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args: &[NBox<Value>], ctx: &mut crate::eval::Context) -> NBox<Value> {

        //ローカルフレームを構築
        let mut frame = Vec::<(&NPtr<symbol::Symbol>, &NBox<Value>)>::new();

        let iter1 = self.params.as_ref().iter();
        let iter2 = args.iter();

        let iter = iter1.zip(iter2);
        for (sym, v) in iter {
            let sym = sym.cast::<symbol::Symbol>();
            frame.push((sym, v));
        }

        //ローカルフレームを環境にプッシュ
        ctx.push_local_frame(&frame);

        //Closure本体を実行
        let mut result:Option<NBox<Value>> = None;
        for sexp in self.body.as_ref().iter() {
            //TODO GC Capture:
            let sexp = NBox::new(sexp.as_mut_ptr());
            result = Some(eval::eval(&sexp, ctx));
        }

        //ローカルフレームを環境からポップ
        ctx.pop_local_frame();

        result.unwrap()
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
