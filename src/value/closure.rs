use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};


pub struct Closure {
    params: RPtr<array::Array>,
    body: RPtr<list::List>,
}

static CLOSURE_TYPEINFO: TypeInfo = new_typeinfo!(
    Closure,
    "Closure",
    std::mem::size_of::<Closure>(),
    None,
    Closure::eq,
    Closure::clone_inner,
    Display::fmt,
    Closure::is_type,
    None,
    None,
    Some(Closure::child_traversal),
);

impl NaviType for Closure {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&CLOSURE_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(this: &RPtr<Self>, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずにRPtrとして扱うことが許されている
        let params = array::Array::clone_inner(&this.as_ref().params, obj).into_rptr();
        let body = list::List::clone_inner(&this.as_ref().body, obj).into_rptr();

        Self::alloc(&params, &body, obj)
    }
}

impl Closure {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&CLOSURE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: &usize, callback: fn(&RPtr<Value>, arg: &usize)) {
        callback(self.params.cast_value(), arg);
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc<T, U>(params: &T, body: &U, obj: &mut Object) -> FPtr<Self>
    where
        T: AsReachable<array::Array>,
        U: AsReachable<list::List>,
    {
        let ptr = obj.alloc::<Closure>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), Closure {
                params: params.as_reachable().clone(),
                body: body.as_reachable().clone(),
            })
        }


        ptr.into_fptr()
    }

    pub fn process_arguments_descriptor<T>(&self, args: &T, _obj: &mut Object) -> bool
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

    pub fn apply<'a>(&self, args_iter: impl Iterator<Item=&'a RPtr<Value>>, obj: &mut Object) -> FPtr<Value>
    {
        //ローカルフレームを構築
        let mut frame = Vec::<(&RPtr<symbol::Symbol>, &RPtr<Value>)>::new();

        let iter1 = self.params.as_ref().iter();
        let iter2 = args_iter;

        let iter = iter1.zip(iter2);
        for (sym, v) in iter {
            let sym = unsafe { sym.cast_unchecked::<symbol::Symbol>() };
            frame.push((sym, v));
        }

        //ローカルフレームを環境にプッシュ
        obj.context().push_local_frame(&frame);

        //Closure本体を実行
        let result = syntax::do_begin(&self.body, obj);

        //ローカルフレームを環境からポップ
        obj.context().pop_local_frame();

        result
    }

}

impl Eq for Closure { }

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

fn display(_this: &Closure, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //TODO
        write!(f, "#closure")
}

impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}
