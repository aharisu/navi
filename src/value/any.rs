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
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for Any {
    fn typeinfo() -> &'static TypeInfo {
        &ANY_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self> {
        if value_is_pointer(self) {
            let typeinfo = get_typeinfo(self);
           (typeinfo.clone_func)(self, allocator)

        } else {
            //Immidiate Valueの場合はそのまま返す
            Ref::new(self)
        }
    }
}

impl Eq for Any {}

impl PartialEq for Any {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = get_typeinfo(self);
        let other_typeinfo =get_typeinfo(other);

        //比較可能な型同士かを確認する関数を持っている場合は、処理を委譲する。
        //持っていない場合は同じ型同士の時だけ比較可能にする。
        let comparable = match self_typeinfo.is_comparable_func {
            Some(func) => func(other_typeinfo),
            None => self_typeinfo == other_typeinfo,
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

        (self_typeinfo.print_func)(self, f)
    }
}

impl std::fmt::Debug for Any {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = get_typeinfo(self);

        (self_typeinfo.print_func)(self, f)
    }
}

impl Any {
    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }

    pub fn is_type(&self, other_typeinfo: &TypeInfo) -> bool {
        if &ANY_TYPEINFO == other_typeinfo {
            //is::<Any>()の場合、常に結果はtrue
            true

        } else {
            let self_typeinfo = get_typeinfo(self);
            if let Some(func) = self_typeinfo.is_type_func {
                func(other_typeinfo)
            } else {
                std::ptr::eq(self_typeinfo, other_typeinfo)
            }
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

    #[test]
    fn is_type() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        //int
        let v = number::Integer::alloc(10, obj).into_value();
        assert!(v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //real
        let v = number::Real::alloc(3.14, obj).into_value();
        assert!(!v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //nil
        let v = list::List::nil().into_value();
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());

        //list
        let item = number::Integer::alloc(10, obj).into_value().reach(obj);
        let v = list::List::alloc(&item, v.try_cast::<list::List>().unwrap(), obj).into_value().reach(obj);
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());
    }

    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let sexp = sexp.reach(obj);
        let result = crate::eval::eval(&sexp, obj);
        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
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