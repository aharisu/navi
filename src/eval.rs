use crate::object::Object;
use crate::{value::*, let_cap, new_cap, with_cap, let_listbuilder};
use crate::ptr::*;

pub fn eval<T>(sexp: &T, obj: &mut Object) -> FPtr<Value>
where
    T: AsReachable<Value>
{
    let sexp = sexp.as_reachable();
    if let Some(sexp) = sexp.try_cast::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.cast_value().clone().into_fptr()
        } else {
            //リスト先頭の式を評価
            let_cap!(head, eval(sexp.as_ref().head_ref(), obj), obj);

            if let Some(func) = head.as_reachable().try_cast::<func::Func>() {
                //関数適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, obj);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    with_cap!(v, eval(sexp, obj), obj, {
                        builder.append(&v, obj);
                    });
                }
                let_cap!(args, builder.get(), obj);

                if let Some(args) = func.as_ref().process_arguments_descriptor(&args, obj) {
                    let ary_ptr = with_cap!(v, args, obj, {
                        array::Array::from_list(&v, None, obj)
                    });

                    with_cap!(args, ary_ptr, obj, {
                        func.as_ref().apply(&args, obj)
                    })
                } else {
                    panic!("Invalid arguments: {:?} {:?}", func.as_ref(), args.as_ref())
                }

            } else if let Some(syntax) = head.as_reachable().try_cast::<syntax::Syntax>() {
                //シンタックス適用
                let args = sexp.as_ref().tail_ref();
                if syntax.as_ref().check_arguments(args) {
                    syntax.as_ref().apply(args, obj)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax.as_ref(), args.as_ref())
                }
            } else if let Some(closure) = head.as_reachable().try_cast::<closure::Closure>() {
                //クロージャ適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, obj);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    with_cap!(v, eval(sexp, obj), obj, {
                        builder.append(&v, obj);
                    });
                }
                let_cap!(args, builder.get(), obj);

                if closure.as_ref().process_arguments_descriptor(&args, obj) {
                    with_cap!(args, array::Array::from_list(&args, None, obj), obj, {
                        closure.as_ref().apply(args.as_reachable().as_ref().iter(), obj)
                    })

                } else {
                    panic!("Invalid arguments: {:?} {:?}", closure.as_ref(), args.as_ref())
                }

            } else {
                panic!("Not Applicable: {:?}", head.as_ref())
            }
        }

    } else if let Some(symbol) = sexp.try_cast::<symbol::Symbol>() {
        if let Some(v) = obj.context().find_value(symbol) {
            v.clone().into_fptr()
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else if let Some(ary) = sexp.try_cast::<array::Array>() {
        let_listbuilder!(builder, obj);
        for sexp in ary.as_ref().iter() {
            with_cap!(v, eval(sexp, obj), obj, {
                builder.append(&v, obj);
            });
        }

        with_cap!(list, builder.get(), obj, {
            array::Array::from_list(&list, Some(ary.as_ref().len()), obj).into_value()
        })
    } else if let Some(tuple) = sexp.try_cast::<tuple::Tuple>() {
        let len = tuple.as_ref().len();

        let_listbuilder!(builder, obj);
        for index in 0..len {
            with_cap!(v, eval(tuple.as_ref().get(index), obj), obj, {
                builder.append(&v, obj);
            });
        }

        with_cap!(list, builder.get(), obj, {
            tuple::Tuple::from_list(&list, Some(len), obj).into_value()
        })
    } else {
        FPtr::new(sexp.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::{let_cap, new_cap};
    use crate::read::*;
    use crate::value::*;
    use crate::ptr::*;

    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> FPtr<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let_cap!(sexp, sexp, obj);
        let result = crate::eval::eval(&sexp, obj);
        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
    }


    #[test]
    fn func_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(abs 1)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let_cap!(result, eval::<number::Real>(program, obj), obj);
            let ans = number::Real::alloc(3.14, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let_cap!(result, eval::<number::Real>(program, obj), obj);
            let ans = number::Real::alloc(3.14, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let_cap!(result, eval::<number::Real>(program, obj), obj);
            let ans = number::Real::alloc(2.0, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //TODO Optional引数のテスト

    }

    #[test]
    fn syntax_if_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(if (= 1 1) 10 100)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(10, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(100, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_false());
        }
    }


    #[test]
    fn syntax_cond_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(cond ((= 1 1) 1) ((= 1 1) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 1) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(3, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2))";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_false());

            let program = "(cond)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_false());
        }
    }

    #[test]
    fn syntax_def_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(def a 1)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(def a 2)";
            eval::<number::Integer>(program, obj);
            let program = "a";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let ((a 3)) a)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(3, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let ((a 3)) (def a 4) a)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(4, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "a";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn syntax_fun_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(11, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(310, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn syntax_let_test() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(let ((a 1)) (+ 10 a))";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(11, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let ((a 100) (b 200)) (+ a b) (+ (let ((a b)) (+ a 10)) a))";
            let_cap!(result, eval::<number::Integer>(program, obj), obj);
            let ans = number::Integer::alloc(310, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn syntax_and_or() {
        let mut obj = Object::new();
        let obj = &mut obj;

        {
            let program = "(and)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_true());

            let program = "(and true true false)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_false());

            let program = "(or)";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_false());

            let program = "(or false (= 1 1))";
            let_cap!(result, eval::<bool::Bool>(program, obj), obj);
            assert!(result.as_ref().is_true());
        }
    }

    #[test]
    fn syntax_match() {
        let mut obj = Object::new();
        let obj = &mut obj;
        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let program = "(match 1 (2 2) (3 3) (4 4) (1 1))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (2 2))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match '((1 2) 3) (((4 5) 6) 1) (((7 8) 9) 2) ((10 (11 12)) 3) (((1 2) 3) 4))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(4, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {{1 2} 3} ({{4 5} 6} 1) ({{1 2} 3} 2) ({10 {11 12}} 3))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match [[1 2] 3] ([4 [5 6]] 1) ([[7 8] 9] 2) ([1 [2 3]] 3))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {{1 2} [3 '(4 5)]} ({{4 5} 6} 1) ((10 (11 12)) 2) ({{1 2} 3 (4 5)} 3) ({{1 2} [3 (4 5)]} 4))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(4, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {1 2 3} ({1 3 2} 1) ({1 2 3} 2))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(2, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match 1 (@x x))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (@x x) (@a (+ a a)))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 2} ({@a @b} (+ a b)))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(3, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {1 '(2 3) [4 '(5)]} ({@a @_ [@b @_]} (+ a b)))";
            let_cap!(result, eval::<Value>(program, obj), obj);
            let ans = number::Integer::alloc(5, ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}