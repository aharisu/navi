use once_cell::sync::Lazy;

use crate::object::Object;
use crate::object::StandaloneObject;
use crate::object::mm::GCAllocationStruct;
use crate::value::*;
use crate::ptr::*;
use crate::err::*;
use crate::value::any::Any;
use crate::value::func::*;
use crate::value::list::ListBuilder;
use crate::vm;
use crate::vm::ExecException;

#[macro_export]
macro_rules! cap_eval {
    ($local_ptr:expr, $obj:expr) => {
        {
            let tmp = ($local_ptr).reach($obj);
            crate::eval::eval(&tmp, $obj)
        }
    };
}

#[derive(Debug)]
pub enum EvalError {
    ObjectSwitch(StandaloneObject),
    Exception(Exception),
}

pub fn eval(sexp: &Reachable<Any>, obj: &mut Object) -> Result<Ref<Any>, EvalError> {

    fn inner(code: &Reachable<compiled::Code>, obj: &mut Object) -> Result<Ref<Any>, EvalError> {
        match vm::code_execute(code, vm::WorkTimeLimit::Inf, obj) {
            Err(vm::ExecException::Exception(e)) => Err(EvalError::Exception(e)),
            Err(vm::ExecException::ObjectSwitch(o)) => Err(EvalError::ObjectSwitch(o)),
            Ok(v) => Ok(v),
        }

    }

    if let Some(code) = sexp.try_cast::<compiled::Code>() {
        inner(code, obj)

    } else {
        match crate::compile::compile(&sexp, obj) {
            Ok(code) => {
                let code = code.reach(obj);
                inner(&code, obj)
            }
            Err(e) => {
                Err(EvalError::Exception(e.into()))
            }
        }
    }
}

fn func_apply(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let app = vm::refer_arg::<app::App>(0, obj).reach(obj);

    let mut last_arg = vm::refer_arg::<Any>(1, obj).reach(obj);
    let mut builder = ListBuilder::new(obj);
    for index in 0 .. num_rest {
        let arg = vm::refer_rest_arg::<Any>(2, index, obj).reach(obj);

        builder.push(&last_arg, obj)?;
        last_arg = arg;
    }

    match last_arg.try_cast::<list::List>() {
        Some(last_arg) => {
            let args = builder.append_get(&last_arg.make());
            let iter = args.reach(obj).iter(obj);
            func_apply_result(vm::app_call(&app, iter, vm::WorkTimeLimit::TakeOver, obj), obj)
        }
        None => {
            Err(Exception::ArgTypeMismatch(ArgTypeMismatch::new(
                String::from("apply"), 2 + num_rest, last_arg.into_ref(), list::List::typeinfo()
            )))
        }
    }
}

fn func_apply_resume(obj: &mut Object) -> NResult<Any, Exception> {
    func_apply_result(vm::resume(vm::WorkTimeLimit::TakeOver, obj), obj)
}

#[inline]
fn func_apply_result(result: NResult<Any, ExecException>, obj: &mut Object) -> NResult<Any, Exception> {
    match result {
        Ok(result) => {
            Ok(result)
        }
        Err(vm::ExecException::ObjectSwitch(_)) => {
            //ObjectSwitchは特殊な構文のみ発生させる例外なのでapplyでは発生しない。
            unreachable!()
        }
        Err(vm::ExecException::Exception(err)) => {
            match err {
                Exception::WaitReply |
                Exception::TimeLimit => {
                    vm::save_func_suspend_info(func_apply_resume, obj);
                }
                _ => { }
            }
            Err(err)
        }
    }
}

static FUNC_APPLY: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("apply",
            &[
            Param::new("app", ParamKind::Require, app::App::typeinfo()),
            Param::new("arg1", ParamKind::Require, Any::typeinfo()),
            Param::new("args", ParamKind::Rest, Any::typeinfo()),
            ],
            func_apply)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("apply", &Ref::new(&FUNC_APPLY.value));
}

#[cfg(test)]
pub fn exec<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
    let mut reader = crate::read::Reader::new(program.chars().peekable());
    let result = crate::read::read(&mut reader, obj);
    assert!(result.is_ok());
    let sexp = result.unwrap();

    let sexp = sexp.reach(obj);
    let result = crate::eval::eval(&sexp, obj).unwrap();
    let result = result.try_cast::<T>();
    assert!(result.is_some());

    result.unwrap().clone()
}

#[cfg(test)]
mod tests {
    use crate::eval::exec;
    use crate::object::Object;
    use crate::value::*;
    use crate::value::any::Any;
    use crate::ptr::*;


    #[test]
    fn func_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(abs 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let result = exec::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let result = exec::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1 + 2 + 3 + -4, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let result = exec::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(2.0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(- 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(-1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(- 3.14)";
            let result = exec::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(-3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(- 1 2 3 -4)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1 - 2 - 3 - -4, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let fib (fun (n) (if (or (= n 0) (= n 1)) n (+ (fib (- n 2)) (fib (- n 1))))))";
            exec::<Any>(program, obj);
            let program = "(fib 10)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(55, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //TODO Optional引数のテスト

    }

    #[test]
    fn syntax_if_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(if (= 1 1) 10 100)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(10, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(100, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());
        }
    }


    #[test]
    fn syntax_cond_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(cond ((= 1 1) 1) ((= 1 1) 2) (else 3))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 1) 2) (else 3))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2) (else 3))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(3, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2))";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(cond)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());
        }
    }

    #[test]
    fn syntax_let_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(let a 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let a 2)";
            exec::<Any>(program, obj);
            let program = "a";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) a)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(3, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) (let a 4) a)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(4, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let-global a 3))";
            exec::<Any>(program, obj);
            let program = "a";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(3, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn syntax_fun_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(11, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(310, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn syntax_local_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(local (let a 1) (+ 10 a))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(11, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 100) (let b 200) (+ a b) (+ (local (let a b) (+ a 10)) a))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(310, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn syntax_and_or() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        {
            let program = "(and)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true false)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(or)";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(or false (= 1 1))";
            let result = exec::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_true());
        }
    }

    #[test]
    fn syntax_match() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(match 1 (2 2) (3 3) (4 4) (1 1))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (2 2))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match '((1 2) 3) (((4 5) 6) 1) (((7 8) 9) 2) ((10 (11 12)) 3) (((1 2) 3) 4))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(4, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {{1 2} 3} ({{4 5} 6} 1) ({{1 2} 3} 2) ({10 {11 12}} 3))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(2, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match [[1 2] 3] ([4 [5 6]] 1) ([[7 8] 9] 2) ([1 [2 3]] 3))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {{1 2} [3 '(4 5)]} ({{4 5} 6} 1) ((10 (11 12)) 2) ({{1 2} 3 (4 5)} 3) ({{1 2} [3 (4 5)]} 4))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(4, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {1 2 3} ({1 3 2} 1) ({1 2 3} 2))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(2, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match 1 (@x x))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (@x x) (@a (+ a a)))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 2} ({@a @b} (+ a b)))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(3, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 '(2 3) [4 '(5)]} ({@a @_ [@b @_]} (+ a b)))";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(5, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn test_apply() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        let program = "(apply + '(1 2 3 4))";
        let result = exec::<Any>(program, obj).capture(obj);
        let ans = number::make_integer(10, obj).unwrap().into_value();
        assert_eq!(result.as_ref(), ans.as_ref());

        let program = "(apply + 1 '(2 3 4))";
        let result = exec::<Any>(program, obj).capture(obj);
        let ans = number::make_integer(10, obj).unwrap().into_value();
        assert_eq!(result.as_ref(), ans.as_ref());

        let program = "(apply + 1 2 '(3 4))";
        let result = exec::<Any>(program, obj).capture(obj);
        let ans = number::make_integer(10, obj).unwrap().into_value();
        assert_eq!(result.as_ref(), ans.as_ref());

        let program = "(apply (fun (n) (sleep 1000) (+ n 1)) '(1))";
        let result = exec::<Any>(program, obj).capture(obj);
        let ans = number::make_integer(2, obj).unwrap().into_value();
        assert_eq!(result.as_ref(), ans.as_ref());
    }

}