use crate::{value::*, let_cap, new_cap, with_cap, let_listbuilder};
use crate::object::{Object, Capture};

pub fn eval(sexp: &Capture<Value>, ctx: &mut Object) -> NPtr<Value> {
    if let Some(sexp) = sexp.try_cast::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.nptr().clone().into_value()
        } else {
            //リスト先頭の式を評価
            let head_ptr = with_cap!(head, sexp.as_ref().head_ref(), ctx, {
                eval(&head, ctx)
            });
            let_cap!(head, head_ptr, ctx);

            if let Some(func) = head.try_cast::<func::Func>() {
                //関数適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, ctx);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    let ptr = with_cap!(sexp, sexp.clone(), ctx, {
                        eval(&sexp, ctx)
                    });
                    with_cap!(v, ptr, ctx, {
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

            } else if let Some(syntax) = head.try_cast::<syntax::Syntax>() {
                //シンタックス適用
                let_cap!(args, sexp.as_ref().tail_ref(), ctx);
                if syntax.as_ref().check_arguments(&args) {
                    syntax.as_ref().apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax.as_ref(), args.as_ref())
                }
            } else if let Some(closure) = head.try_cast::<closure::Closure>() {
                //クロージャ適用

                //引数を順に評価してリスト内に保存
                let_listbuilder!(builder, ctx);
                let args_sexp = sexp.as_ref().tail_ref();
                for sexp in args_sexp.as_ref().iter() {
                    let ptr = with_cap!(sexp, sexp.clone(), ctx, {
                        eval(&sexp, ctx)
                    });
                    with_cap!(v, ptr, ctx, {
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
            v
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else {
        sexp.nptr().clone()
    }
}

#[cfg(test)]
mod tets {
    use crate::{let_cap, new_cap};
    use crate::read::*;
    use crate::value::*;
    use crate::object::*;

    fn read(program: &str, ctx: &mut Object) -> NPtr<Value> {
        let mut ctx = ReadContext::new(program.chars().peekable(), ctx);

        let result = crate::read::read(&mut ctx);
        assert!(result.is_ok());
        result.unwrap()
    }

    fn eval(sexp: &Capture<Value>, ctx: &mut Object) -> NPtr<Value> {
        crate::eval::eval(&sexp, ctx)
    }

    #[test]
    fn func_test() {
        let mut ctx = Object::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);

        {
            let program = "(abs 1)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(abs -1)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx).into_value(), ctx);
            let ans = number::Integer::alloc(1, ans_ctx).into_value();
            assert_eq!((*result).as_ref(), ans.as_ref());

            let program = "(abs -3.14)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Real::alloc(3.14, ans_ctx).into_value();
            assert_eq!((*result).as_ref(), ans.as_ref());
        }

        {
            let program = "(+ 1)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(1, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(+ 3.14)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Real::alloc(3.14, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(+ 1 2 3 -4)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(2, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(+ 1.5 2 3 -4.5)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Real::alloc(2.0, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());
        }

        //TODO Optional引数のテスト

    }

    #[test]
    fn syntax_if_test() {
        let mut ctx = Object::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "(if (= 1 1) 10 100)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(10, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(if (= 1 2) 10 100)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(100, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(if (= 1 1 1) 10)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(10, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "(if (= 1 1 2) 10)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            assert!(result.is::<unit::Unit>())
        }
    }

    #[test]
    fn syntax_fun_test() {
        let mut ctx = Object::new("eval");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        number::register_global(ctx);
        syntax::register_global(ctx);

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(11, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let_cap!(result, read(program, ctx), ctx);
            let_cap!(result, eval(&result, ctx), ctx);
            let ans = number::Integer::alloc(310, ans_ctx).into_value();
            assert_eq!(result.nptr().as_ref(), ans.as_ref());
        }
    }

}