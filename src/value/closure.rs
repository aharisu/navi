use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};


pub struct Closure {
    params: Vec::<FPtr<symbol::Symbol>>,
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

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        unsafe {
            let params = self.params.iter()
                .map(|param| symbol::Symbol::clone_inner(param.as_ref(), obj))
                .collect()
                ;
            //array::Array::clone_inner(self.params.as_ref(), obj).into_rptr();
            let body = list::List::clone_inner(self.body.as_ref(), obj).into_reachable();

            let ptr = obj.alloc::<Closure>();

            std::ptr::write(ptr.as_ptr(), Closure {
                params: params,
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
        self.params.iter().for_each(|param| callback(param.cast_value(), arg));
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc(params: Vec::<Reachable<symbol::Symbol>>, body: &Reachable<list::List>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<Closure>();

        let params: Vec::<FPtr<symbol::Symbol>> = params.into_iter()
            .map(|param| FPtr::new(param.as_ref()))
            .collect()
            ;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Closure {
                params: params,
                body: FPtr::new(body.as_ref()),
            })
        }

        ptr.into_fptr()
    }

    pub fn process_arguments_descriptor(&self, args: &Reachable<list::List>, _obj: &mut Object) -> bool {
        //TODO 各種パラメータ指定の処理(:option, :rest)

        let count = args.as_ref().count();
        if count < self.params.len() {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args_iter: impl Iterator<Item=FPtr<Value>>, obj: &mut Object) -> FPtr<Value> {
        //ローカルフレームを構築
        let mut frame = Vec::<(&symbol::Symbol, &Value)>::new();

        for (sym, v) in self.params.iter().zip(args_iter) {
            frame.push((sym.as_ref(), v.as_ref()));
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
