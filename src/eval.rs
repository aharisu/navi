use crate::{value::*, let_cap, new_cap, with_cap, let_listbuilder};
use crate::context::Context;
use crate::ptr::*;

pub fn eval<T>(sexp: &T, ctx: &mut Context) -> FPtr<Value>
where
    T: AsReachable<Value>
{
    let sexp = sexp.as_reachable();
    if let Some(sexp) = sexp.try_cast::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.cast_value().clone().into_fptr()
        } else {
            //リスト先頭の式を評価
            let_cap!(head, eval(sexp.as_ref().head_ref(), ctx), ctx);

            if let Some(func) = head.as_reachable().try_cast::<func::Func>() {
                //関数適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, ctx);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    with_cap!(v, eval(sexp, ctx), ctx, {
                        builder.append(&v, ctx);
                    });
                }
                let_cap!(args, builder.get(), ctx);

                if let Some(args) = func.as_ref().process_arguments_descriptor(&args, ctx) {
                    let ary_ptr = with_cap!(v, args, ctx, {
                        array::Array::from_list(&v, None, ctx)
                    });

                    with_cap!(args, ary_ptr, ctx, {
                        func.as_ref().apply(&args, ctx)
                    })
                } else {
                    panic!("Invalid arguments: {:?} {:?}", func.as_ref(), args.as_ref())
                }

            } else if let Some(syntax) = head.as_reachable().try_cast::<syntax::Syntax>() {
                //シンタックス適用
                let args = sexp.as_ref().tail_ref();
                if syntax.as_ref().check_arguments(args) {
                    syntax.as_ref().apply(args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax.as_ref(), args.as_ref())
                }
            } else if let Some(closure) = head.as_reachable().try_cast::<closure::Closure>() {
                //クロージャ適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, ctx);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    with_cap!(v, eval(sexp, ctx), ctx, {
                        builder.append(&v, ctx);
                    });
                }
                let_cap!(args, builder.get(), ctx);

                if closure.as_ref().process_arguments_descriptor(&args, ctx) {
                    with_cap!(args, array::Array::from_list(&args, None, ctx), ctx, {
                        closure.as_ref().apply(&args, ctx)
                    })

                } else {
                    panic!("Invalid arguments: {:?} {:?}", closure.as_ref(), args.as_ref())
                }

            } else {
                panic!("Not Applicable: {:?}", head.as_ref())
            }
        }

    } else if let Some(symbol) = sexp.try_cast::<symbol::Symbol>() {
        if let Some(v) = ctx.find_value(symbol) {
            v.clone().into_fptr()
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else if let Some(ary) = sexp.try_cast::<array::Array>() {
        let_listbuilder!(builder, ctx);
        for sexp in ary.as_ref().iter() {
            with_cap!(v, eval(sexp, ctx), ctx, {
                builder.append(&v, ctx);
            });
        }

        with_cap!(list, builder.get(), ctx, {
            array::Array::from_list(&list, Some(ary.as_ref().len()), ctx).into_value()
        })
    } else if let Some(tuple) = sexp.try_cast::<tuple::Tuple>() {
        let len = tuple.as_ref().len();

        let_listbuilder!(builder, ctx);
        for index in 0..len {
            with_cap!(v, eval(tuple.as_ref().get(index), ctx), ctx, {
                builder.append(&v, ctx);
            });
        }

        with_cap!(list, builder.get(), ctx, {
            tuple::Tuple::from_list(&list, Some(len), ctx).into_value()
        })
    } else {
        FPtr::new(sexp.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use crate::{let_cap, new_cap, value};
    use crate::read::*;
    use crate::value::*;
    use crate::context::*;
    use crate::ptr::*;

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
    fn func_test() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);

        {
            let program = "(abs 1)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let_cap!(result, eval::<number::Real>(program, ctx), ctx);
            let ans = number::Real::alloc(3.14, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let_cap!(result, eval::<number::Real>(program, ctx), ctx);
            let ans = number::Real::alloc(3.14, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(2, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let_cap!(result, eval::<number::Real>(program, ctx), ctx);
            let ans = number::Real::alloc(2.0, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //TODO Optional引数のテスト

    }

    #[test]
    fn syntax_if_test() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        value::register_global(ctx);
        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "(if (= 1 1) 10 100)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(10, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(100, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }
    }


    #[test]
    fn syntax_cond_test() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        value::register_global(ctx);
        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "(cond ((= 1 1) 1) ((= 1 1) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 1) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(2, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2) (else 3))";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(3, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(cond ((= 1 2) 1) ((= 1 3) 2))";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(cond)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());
        }
    }

    #[test]
    fn syntax_fun_test() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(11, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(310, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn syntax_let_test() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "(let ((a 1)) (+ 10 a))";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(11, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(let ((a 100) (b 200)) (+ a b) (+ (let ((a b)) (+ a 10)) a))";
            let_cap!(result, eval::<number::Integer>(program, ctx), ctx);
            let ans = number::Integer::alloc(310, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn syntax_and_or() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);
        value::register_global(ctx);

        {
            let program = "(and)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(and true true)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());

            let program = "(and true true false)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(or)";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_false());

            let program = "(or false (= 1 1))";
            let_cap!(result, eval::<bool::Bool>(program, ctx), ctx);
            assert!(result.as_ref().is_true());
        }
    }

    #[test]
    fn syntax_match() {
        let mut ctx = Context::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);
        value::register_global(ctx);
        tuple::register_global(ctx);
        array::register_global(ctx);
        list::register_global(ctx);

        {
            let program = "(match 1 (2 2) (3 3) (4 4) (1 1))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match 1 (2 2))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match '((1 2) 3) (((4 5) 6) 1) (((7 8) 9) 2) ((10 (11 12)) 3) (((1 2) 3) 4))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = number::Integer::alloc(4, ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match {{1 2} 3} ({{4 5} 6} 1) ({{1 2} 3} 2) ({10 {11 12}} 3))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = number::Integer::alloc(2, ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(match [[1 2] 3] ([4 [5 6]] 1) ([[7 8] 9] 2) ([1 [2 3]] 3))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(match {{1 2} [3 '(4 5)]} ({{4 5} 6} 1) ((10 (11 12)) 2) ({{1 2} 3 (4 5)} 3) ({{1 2} [3 (4 5)]} 4))";
            let_cap!(result, eval::<Value>(program, ctx), ctx);
            let ans = number::Integer::alloc(4, ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}