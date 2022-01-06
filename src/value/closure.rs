use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};


pub struct Closure {
    params: FPtr<array::Array<symbol::Symbol>>,
    body: FPtr<list::List>,
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

    fn clone_inner(&self, allocator: &AnyAllocator) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        unsafe {
            let params = array::Array::<symbol::Symbol>::clone_inner(self.params.as_ref(), allocator).into_reachable();
            let body = list::List::clone_inner(self.body.as_ref(), allocator).into_reachable();

            let ptr = allocator.alloc::<Closure>();

            std::ptr::write(ptr.as_ptr(), Closure {
                params: FPtr::new(params.as_ref()),
                body: FPtr::new(body.as_ref()),
            });

            ptr.into_fptr()
        }
    }
}

impl Closure {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&CLOSURE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(self.params.cast_value(), arg);
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc<A: Allocator>(params: &Reachable<array::Array<symbol::Symbol>>, body: &Reachable<list::List>, allocator: &A) -> FPtr<Self> {
        let ptr = allocator.alloc::<Closure>();

        unsafe {
            std::ptr::write(ptr.as_ptr(), Closure {
                params: FPtr::new(params.as_ref()),
                body: FPtr::new(body.as_ref()),
            })
        }

        ptr.into_fptr()
    }

    pub fn process_arguments_descriptor(&self, args_iter: impl Iterator<Item = FPtr<Value>>, _obj: &mut Object) -> bool {
        //TODO 各種パラメータ指定の処理(:option, :rest)

        let count = args_iter.count();
        if count < self.params.as_ref().len() {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args_iter: impl Iterator<Item=FPtr<Value>>, obj: &mut Object) -> FPtr<Value> {
        //ローカルフレームを構築
        let mut frame = Vec::<(&symbol::Symbol, &Value)>::new();

        {
            let params_iter = unsafe { self.params.as_ref().iter_gcunsafe() };
            for (sym, v) in params_iter.zip(args_iter) {
                frame.push((sym.as_ref(), v.as_ref()));
            }
        }

        //ローカルフレームを環境にプッシュ
        obj.context().push_local_frame(&frame);

        //Closure本体を実行
        let body = self.body.clone().reach(obj);
        let result = syntax::do_begin(&body, obj);

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
