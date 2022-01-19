use crate::object::Object;
use crate::object::StandaloneObject;
use crate::value::*;
use crate::ptr::*;
use crate::err::*;
use crate::value::any::Any;
use crate::vm;

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
    Exit,
    Exception(Exception),
}

pub fn eval(sexp: &Reachable<Any>, obj: &mut Object) -> Result<Ref<Any>, EvalError> {

    fn inner(code: &Reachable<compiled::Code>, obj: &mut Object) -> Result<Ref<Any>, EvalError> {
        match vm::code_execute(code, vm::WorkTimeLimit::Inf, obj) {
            Err(vm::ExecException::TimeLimit) => unreachable!(),
            Err(vm::ExecException::WaitReply) => unreachable!(),
            Err(vm::ExecException::MySelfObjectDeleted) => unreachable!(),
            Err(vm::ExecException::Exit) => Err(EvalError::Exit),
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

#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::read::*;
    use crate::value::*;
    use crate::value::any::Any;
    use crate::ptr::*;

    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let sexp = sexp.reach(obj);
        let result = crate::eval::eval(&sexp, obj).unwrap();
        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
    }


    #[test]
    fn func_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(abs 1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(2.0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(- 1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(-1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(- 3.14)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(-3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(- 1 2 3 -4)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let fib (fun (n) (if (or (= n 0) (= n 1)) n (+ (fib (- n 2)) (fib (- n 1))))))";
            eval::<Any>(program, obj);
            let program = "(fib 10)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(55, ans_obj).unwrap();
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
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(10, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(100, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
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
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 1) 2) (else 3))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2) (else 3))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2))";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(cond)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
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
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let a 2)";
            eval::<number::Integer>(program, obj);
            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) a)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) (let a 4) a)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap();
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
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(11, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(310, ans_obj).unwrap();
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
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(11, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 100) (let b 200) (+ a b) (+ (local (let a b) (+ a 10)) a))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(310, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn syntax_and_or() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        {
            let program = "(and)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true false)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(or)";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
            assert!(result.as_ref().is_false());

            let program = "(or false (= 1 1))";
            let result = eval::<bool::Bool>(program, obj).capture(obj);
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
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (2 2))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match '((1 2) 3) (((4 5) 6) 1) (((7 8) 9) 2) ((10 (11 12)) 3) (((1 2) 3) 4))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {{1 2} 3} ({{4 5} 6} 1) ({{1 2} 3} 2) ({10 {11 12}} 3))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match [[1 2] 3] ([4 [5 6]] 1) ([[7 8] 9] 2) ([1 [2 3]] 3))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {{1 2} [3 '(4 5)]} ({{4 5} 6} 1) ((10 (11 12)) 2) ({{1 2} 3 (4 5)} 3) ({{1 2} [3 (4 5)]} 4))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {1 2 3} ({1 3 2} 1) ({1 2 3} 2))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match 1 (@x x))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (@x x) (@a (+ a a)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 2} ({@a @b} (+ a b)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 '(2 3) [4 '(5)]} ({@a @_ [@b @_]} (+ a b)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(5, ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}