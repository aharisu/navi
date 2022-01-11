use crate::value::{*};
use crate::ptr::*;

pub struct Any { }

static ANY_TYPEINFO : TypeInfo = new_typeinfo!(
    Any,
    "Any",
    0, None,
    Any::_eq,
    Any::clone_inner,
    Any::_fmt,
    Any::_is_type,
    None,
    None,
    None,
    None,
);

impl NaviType for Any {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&ANY_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self> {
        if value_is_pointer(self) {
            let typeinfo = get_typeinfo(self);
           (unsafe { typeinfo.as_ref() }.clone_func)(self, allocator)

        } else {
            //Immidiate Valueの場合はそのまま返す
            Ref::new(self)
        }
    }
}

impl Eq for Any {}

impl PartialEq for Any {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = unsafe { get_typeinfo(self).as_ref() };
        let other_typeinfo = unsafe { get_typeinfo(other).as_ref() };

        //比較可能な型同士かを確認する関数を持っている場合は、処理を委譲する。
        //持っていない場合は同じ型同士の時だけ比較可能にする。
        let comparable = match self_typeinfo.is_comparable_func {
            Some(func) => func(other_typeinfo),
            None => std::ptr::eq(self_typeinfo, other_typeinfo),
        };

        if comparable {
            (self_typeinfo.eq_func)(self, other)
        } else {
            false
        }
    }
}

impl std::fmt::Display for Any {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = get_typeinfo(self);

        (unsafe { self_typeinfo.as_ref() }.print_func)(self, f)
    }
}

impl std::fmt::Debug for Any {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = get_typeinfo(self);

        (unsafe { self_typeinfo.as_ref() }.print_func)(self, f)
    }
}

impl Any {
    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        if std::ptr::eq(&ANY_TYPEINFO, other_typeinfo.as_ptr()) {
            //is::<Any>()の場合、常に結果はtrue
            true

        } else {
            let self_typeinfo = get_typeinfo(self);

            (unsafe { self_typeinfo.as_ref() }.is_type_func)(unsafe { other_typeinfo.as_ref() })
        }
    }

    pub fn try_cast<U: NaviType>(&self) -> Option<&U> {
        if self.is::<U>() {
            Some(unsafe { &*(self as *const Any as *const U) })
        } else {
            None
        }
    }

    //Value型のインスタンスは存在しないため、これらのメソッドが呼び出されることはない
    fn _eq(&self, _other: &Self) -> bool {
        unreachable!()
    }

    fn _fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }

    fn _is_type(_other_typeinfo: &TypeInfo) -> bool {
        unreachable!()
    }
}

fn func_equal(obj: &mut Object) -> Ref<Any> {
    let left = vm::refer_arg::<Any>(0, obj);
    let right = vm::refer_arg::<Any>(1, obj);

    let result = left.as_ref().eq(right.as_ref());

    if result {
        bool::Bool::true_().into_ref().into_value()
    } else {
        bool::Bool::false_().into_ref().into_value()
    }
}

static FUNC_EQUAL: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("=",
            &[
            Param::new("left", ParamKind::Require, Any::typeinfo()),
            Param::new("right", ParamKind::Require, Any::typeinfo()),
            ],
            func_equal)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("=", &Ref::new(&FUNC_EQUAL.value));
}

pub mod literal {
    use super::*;

    pub fn equal() -> Reachable<Func> {
        Reachable::new_static(&FUNC_EQUAL.value)
    }
}

#[cfg(test)]
mod tests {
    use crate::read::Reader;
    use crate::value::*;

    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let sexp = sexp.reach(obj);

        let result = {
            let result = crate::eval::eval(&sexp, obj);
            let result = result.try_cast::<T>();
            assert!(result.is_some());

            result.unwrap().clone()
        };
        let result = result.reach(obj);

        let result2 = {
            let compiled = crate::compile::compile(&sexp, obj).into_value().reach(obj);

            let result = crate::eval::eval(&compiled, obj);
            let result = result.try_cast::<T>();
            assert!(result.is_some());

            result.unwrap().clone()
        };
        assert_eq!(result.as_ref(), result2.as_ref());

        result.into_ref()
    }


    #[test]
    fn equal() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        {
            let program = "(= 1 1)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 1 1.0)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 1.0 1)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 3.14 3.14)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 1 1.001)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= \"hoge\" \"hoge\")";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= \"hoge\" \"hogehoge\")";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= \"hoge\" \"huga\")";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= 'symbol 'symbol)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 'symbol 'other)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= :keyword :keyword)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= 'symbol 'other)";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= '(1 \"2\" :3) '(1 \"2\" :3))";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= '(1 \"2\" :3) '(1 \"2\" '3))";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= [1 \"2\" :3] [1 \"2\" :3])";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= [1 \"2\" :3] [1 \"2\" '3])";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= {1 \"2\" :3} {1 \"2\" :3})";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_true());

            let program = "(= {1 \"2\" :3} {1 \"2\" '3})";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= 1 \"1\")";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= '(1 2 3) [1 2 3])";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());

            let program = "(= {} [])";
            let result = eval::<bool::Bool>(program, obj).reach(obj);
            assert!(result.as_ref().is_false());
        }

    }

}