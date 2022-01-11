use crate::cap_append;
use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use crate::value::any::Any;
use crate::value::array::ArrayBuilder;
use crate::value::list::ListBuilder;
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

pub fn eval(sexp: &Reachable<Any>, obj: &mut Object) -> Ref<Any> {
    if let Some(code) = sexp.try_cast::<compiled::Code>() {
        vm::code_execute(code, vm::WorkTimeLimit::Inf, obj).unwrap()

    } else if let Some(sexp) = sexp.try_cast::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.make().into_value()

        } else {
            //リスト先頭の式を評価
            let head = cap_eval!(sexp.as_ref().head(), obj).reach(obj);

            if let Some(func) = head.try_cast::<func::Func>() {
                //関数適用

                //引数を順に評価してリスト内に保存
                let mut builder_args = ListBuilder::new(obj);
                for sexp in sexp.as_ref().tail().reach(obj).iter(obj) {
                    cap_append!(builder_args, cap_eval!(sexp, obj), obj);
                }
                let args = builder_args.get().reach(obj);

                //関数の呼び出し処理を実行
                vm::func_call(func, args.iter(obj), vm::WorkTimeLimit::Inf, obj).unwrap()

            } else if let Some(syntax) = head.try_cast::<syntax::Syntax>() {
                //シンタックス適用
                let args = sexp.as_ref().tail().reach(obj);
                if syntax.as_ref().check_arguments(&args) {
                    syntax.as_ref().apply(&args, obj)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax.as_ref(), args.as_ref())
                }
            } else if let Some(closure) = head.try_cast::<closure::Closure>() {
                //クロージャ適用

                //引数を順に評価してリスト内に保存
                let mut builder_args = ListBuilder::new(obj);
                for sexp in sexp.as_ref().tail().reach(obj).iter(obj) {
                    cap_append!(builder_args, cap_eval!(sexp, obj), obj);
                }

                let args = builder_args.get().reach(obj);
                if closure.as_ref().process_arguments_descriptor(args.iter(obj), obj) {
                    let args = array::Array::from_list(&args, None, obj).reach(obj);
                    closure.as_ref().apply(args.iter(), obj)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", closure.as_ref(), args.as_ref())
                }
            } else if let Some(closure) = head.try_cast::<compiled::Closure>() {

                //TODO vm::closure_call
                unimplemented!()

            } else {
                panic!("Not Applicable: {:?}", head.as_ref())
            }
        }

    } else if let Some(symbol) = sexp.try_cast::<symbol::Symbol>() {
        if let Some(v) = obj.context().find_local_value(symbol.as_ref()) {
            v.clone()
        } else if let Some(v) = obj.find_global_value(symbol.as_ref()) {
            v.clone()
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else if let Some(ary) = sexp.try_cast::<array::Array<Any>>() {
        let mut builder = ArrayBuilder::<Any>::new(ary.as_ref().len(), obj);
        for sexp in ary.iter() {
            let v = cap_eval!(sexp, obj);
            builder.push(&v, obj)
        }
        builder.get().into_value()

    } else if let Some(tuple) = sexp.try_cast::<tuple::Tuple>() {
        let len = tuple.as_ref().len();
        if len == 0 {
            tuple::Tuple::unit().into_ref().into_value()

        } else {
            let mut builder = ArrayBuilder::<Any>::new(len, obj);
            for index in 0..len {
                let sexp = tuple.as_ref().get(index).reach(obj);
                let v = eval(&sexp, obj);
                builder.push(&v, obj)
            }
            tuple::Tuple::from_array(&builder.get().reach(obj), obj).into_value()
        }

    } else {
        sexp.make()
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
    fn func_test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(abs 1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(3.14, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let result = eval::<number::Real>(program, obj).capture(obj);
            let ans = number::Real::alloc(2.0, ans_obj);
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
            let ans = number::Integer::alloc(10, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(100, ans_obj);
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
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 1) 2) (else 3))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2) (else 3))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj);
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
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let a 2)";
            eval::<number::Integer>(program, obj);
            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) a)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 3) (let a 4) a)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj);
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
            let ans = number::Integer::alloc(11, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(310, ans_obj);
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
            let ans = number::Integer::alloc(11, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(local (let a 100) (let b 200) (+ a b) (+ (local (let a b) (+ a 10)) a))";
            let result = eval::<number::Integer>(program, obj).capture(obj);
            let ans = number::Integer::alloc(310, ans_obj);
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
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (2 2))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match '((1 2) 3) (((4 5) 6) 1) (((7 8) 9) 2) ((10 (11 12)) 3) (((1 2) 3) 4))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {{1 2} 3} ({{4 5} 6} 1) ({{1 2} 3} 2) ({10 {11 12}} 3))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match [[1 2] 3] ([4 [5 6]] 1) ([[7 8] 9] 2) ([1 [2 3]] 3))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {{1 2} [3 '(4 5)]} ({{4 5} 6} 1) ((10 (11 12)) 2) ({{1 2} 3 (4 5)} 3) ({{1 2} [3 (4 5)]} 4))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(4, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {1 2 3} ({1 3 2} 1) ({1 2 3} 2))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(2, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match 1 (@x x))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (@x x) (@a (+ a a)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 2} ({@a @b} (+ a b)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(3, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 '(2 3) [4 '(5)]} ({@a @_ [@b @_]} (+ a b)))";
            let result = eval::<Any>(program, obj).capture(obj);
            let ans = number::Integer::alloc(5, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}