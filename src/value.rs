#[macro_export]
macro_rules! new_typeinfo {
    ($t:ty, $name:expr, $eq_func:expr, $print_func:expr, $is_type_func:expr, $finalize_func:expr, $is_comparable_func:expr, $child_traversal_func:expr, ) => {
        TypeInfo {
            name: $name,
            eq_func: unsafe { std::mem::transmute::<fn(&$t, &$t) -> bool, fn(&Value, &Value) -> bool>($eq_func) },
            print_func: unsafe { std::mem::transmute::<fn(&$t, &mut std::fmt::Formatter<'_>) -> std::fmt::Result, fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result>($print_func) },
            is_type_func: $is_type_func,
            finalize: match $finalize_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&mut $t), fn(&mut Value)>(func) }),
                None => None
            },
            is_comparable_func: match $is_comparable_func {
                Some(func) => Some(func),
                None => None
             },
            child_traversal_func: match $child_traversal_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&$t, usize, fn(&RPtr<Value>, usize)), fn(&Value, usize, fn(&RPtr<Value>, usize))>(func) }),
                None => None
             },
        }
    };
}

pub mod array;
pub mod bool;
pub mod closure;
pub mod list;
pub mod number;
pub mod string;
pub mod symbol;
pub mod keyword;
pub mod func;
pub mod syntax;
pub mod tuple;
pub mod object;


use std::ptr::{self};
use crate::context::Context;
use crate::util::non_null_const::*;
use crate::ptr::*;

use crate::mm::{GCAllocationStruct};
use crate::value::func::*;
use once_cell::sync::Lazy;

// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b0000_1111;


const fn tagged_value(tag: usize) -> usize {
    (tag << 16) | IMMIDATE_TAGGED_VALUE
}

pub(crate) const IMMIDATE_NIL: usize = tagged_value(0);
pub(crate) const IMMIDATE_TRUE: usize = tagged_value(1);
pub(crate) const IMMIDATE_FALSE: usize = tagged_value(2);
pub(crate) const IMMIDATE_UNIT: usize = tagged_value(3);
pub(crate) const IMMIDATE_MATCHFAIL: usize = tagged_value(4);

#[derive(PartialEq)]
enum PtrKind {
    Ptr,
    Nil,
    True,
    False,
    Unit,
    MatchFail
}

fn pointer_kind<T>(ptr: *const T) -> PtrKind {
    let value = crate::mm::ptr_to_usize(ptr);

    //下位2bitが00なら生ポインタ
    if value & 0b11 == 0 {
        PtrKind::Ptr
    } else {
        //残りは下位16bitで判断する
        match value &0xFFFF {
            IMMIDATE_TAGGED_VALUE => {
                match value {
                    IMMIDATE_NIL => PtrKind::Nil,
                    IMMIDATE_TRUE => PtrKind::True,
                    IMMIDATE_FALSE => PtrKind::False,
                    IMMIDATE_UNIT => PtrKind::Unit,
                    IMMIDATE_MATCHFAIL => PtrKind::MatchFail,
                    _ => panic!("invalid tagged value"),
                }
            }
            _ => panic!("invalid pointer"),
        }
    }
}

pub fn value_is_pointer(v: &Value) -> bool {
    pointer_kind(v as *const Value) == PtrKind::Ptr
}

pub trait NaviType: PartialEq + std::fmt::Debug + std::fmt::Display {
    fn typeinfo() -> NonNullConst<TypeInfo>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub eq_func: fn(&Value, &Value) -> bool,
    pub print_func: fn(&Value, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: fn(&TypeInfo) -> bool,
    pub finalize: Option<fn(&mut Value)>,
    pub is_comparable_func: Option<fn(&TypeInfo) -> bool>,
    pub child_traversal_func: Option<fn(&Value, usize, fn(&RPtr<Value>, usize))>,
}

pub struct Value { }

static VALUE_TYPEINFO : TypeInfo = new_typeinfo!(
    Value,
    "Value",
    Value::_eq,
    Value::_fmt,
    Value::_is_type,
    None,
    None,
    None,
);

impl NaviType for Value {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&VALUE_TYPEINFO as *const TypeInfo)
    }
}

impl Eq for Value {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let self_typeinfo = unsafe { self.get_typeinfo().as_ref() };
        let other_typeinfo = unsafe { other.get_typeinfo().as_ref() };

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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = self.get_typeinfo();

        (unsafe { self_typeinfo.as_ref() }.print_func)(self, f)
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_typeinfo = self.get_typeinfo();

        (unsafe { self_typeinfo.as_ref() }.print_func)(self, f)
    }
}

impl Value {
    pub fn get_typeinfo(&self) -> NonNullConst<TypeInfo> {
        let ptr = self as *const Value;
        match pointer_kind(ptr) {
            PtrKind::Nil => {
                crate::value::list::List::typeinfo()
            }
            PtrKind::True | PtrKind::False => {
                crate::value::bool::Bool::typeinfo()
            }
            PtrKind::Unit => {
                crate::value::tuple::Tuple::typeinfo()
            }
            PtrKind::MatchFail => {
                crate::value::syntax::r#match::MatchFail::typeinfo()
            }
            PtrKind::Ptr => {
                crate::mm::get_typeinfo(ptr)
            }
        }
    }

    pub fn is<U: NaviType>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        self.is_type(other_typeinfo)
    }

    pub fn is_type(&self, other_typeinfo: NonNullConst<TypeInfo>) -> bool {
        if std::ptr::eq(&VALUE_TYPEINFO, other_typeinfo.as_ptr()) {
            //is::<Value>()の場合、常に結果はtrue
            true

        } else {
            let self_typeinfo = self.get_typeinfo();

            (unsafe { self_typeinfo.as_ref() }.is_type_func)(unsafe { other_typeinfo.as_ref() })
        }
    }

    pub fn try_cast<U: NaviType>(&self) -> Option<&U> {
        if self.is::<U>() {
            Some(unsafe { &*(self as *const Value as *const U) })
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

    fn _child_traversal(&self, _callback: fn(&Value)) {
        unreachable!()
    }

}

fn func_equal(args: &RPtr<array::Array>, _ctx: &mut Context) -> FPtr<Value> {
    let left = args.as_ref().get(0);
    let right = args.as_ref().get(1);

    let result = left.as_ref().eq(right.as_ref());

    if result {
        bool::Bool::true_().into_fptr().into_value()
    } else {
        bool::Bool::false_().into_fptr().into_value()
    }
}

static FUNC_EQUAL: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("=",
            &[
            Param::new("left", ParamKind::Require, Value::typeinfo()),
            Param::new("right", ParamKind::Require, Value::typeinfo()),
            ],
            func_equal)
    )
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("=", &RPtr::new(&FUNC_EQUAL.value as *const Func as *mut Func).into_value());
}

pub mod literal {
    use super::*;

    pub fn equal() -> RPtr<Func> {
        RPtr::new(&FUNC_EQUAL.value as *const Func as *mut Func)
    }
}

#[cfg(test)]
mod tests {
    use crate::read::Reader;
    use crate::{value::*, let_cap, new_cap};
    use crate::context::Context;

    #[test]
    fn is_type() {
        let mut ctx = Context::new();
        let ctx = &mut ctx;

        //int
        let v = number::Integer::alloc(10, ctx).into_value();
        assert!(v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //real
        let v = number::Real::alloc(3.14, ctx).into_value();
        assert!(!v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //nil
        let v = list::List::nil().into_value();
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());

        //list
        let_cap!(item, number::Integer::alloc(10, ctx).into_value(), ctx);
        let_cap!(v, list::List::alloc(&item, v.try_cast::<list::List>().unwrap(), ctx).into_value(), ctx);
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());
    }

    fn eval<T: NaviType>(program: &str, ctx: &mut Context) -> FPtr<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, ctx);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let_cap!(sexp, sexp, ctx);
        let result = crate::eval::eval(&sexp, ctx);
        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
    }


    #[test]
    fn equal() {
        let mut ctx = Context::new();
        let ctx = &mut ctx;

        register_global(ctx);
        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "(= 1 1)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 1 1.0)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 1.0 1)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 3.14 3.14)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 1 1.001)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= \"hoge\" \"hoge\")";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= \"hoge\" \"hogehoge\")";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= \"hoge\" \"huga\")";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= 'symbol 'symbol)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 'symbol 'other)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= :keyword :keyword)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= 'symbol 'other)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= '(1 \"2\" :3) '(1 \"2\" :3))";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= '(1 \"2\" :3) '(1 \"2\" '3))";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= [1 \"2\" :3] [1 \"2\" :3])";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= [1 \"2\" :3] [1 \"2\" '3])";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= {1 \"2\" :3} {1 \"2\" :3})";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(= {1 \"2\" :3} {1 \"2\" '3})";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }

        {
            let program = "(= 1 \"1\")";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= '(1 2 3) [1 2 3])";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(= {} [])";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }

    }

}