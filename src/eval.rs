use crate::value::*;
use crate::object::{Object};

pub fn eval(sexp: &NBox<Value>, ctx: &mut Object) -> NBox<Value> {
    if let Some(sexp) = sexp.duplicate().into_nbox::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.into_nboxvalue()
        } else {
            //TODO GC capture: sexp

            //TODO GC capture: head
            let head = sexp.as_ref().head_ref();
            //TODO GC capture: head
            let head = eval(&head, ctx);

            if let Some(func) = head.try_cast::<func::Func>() {
                //関数適用
                //TODO GC capture: args, iter
                let mut args: Vec<NBox<Value>> = Vec::new();

                //TODO GC Capture: ????
                let args_sexp = sexp.as_ref().tail_ref();

                //TODO GC Capture:
                let iter = args_sexp.as_ref().iter();
                for sexp in iter {
                    //TODO GC Capture: sexp
                    let sexp = NBox::new(sexp.as_mut_ptr());
                    args.push(eval(&sexp, ctx));
                }

                if func.as_ref().process_arguments_descriptor(ctx, &mut args) {
                    func.as_ref().apply(ctx, &args)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", func, args)
                }

            } else if let Some(syntax) = head.try_cast::<syntax::Syntax>() {
                //シンタックス適用
                //TODO GC Capture:
                let args = sexp.as_ref().tail_ref();
                if syntax.as_ref().check_arguments(&args) {
                    syntax.as_ref().apply(ctx, &args)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax, args)
                }
            } else if let Some(closure) = head.try_cast::<closure::Closure>() {
                //クロージャ適用

                //TODO GC capture: args, iter
                let mut args: Vec<NBox<Value>> = Vec::new();

                //TODO GC Capture: ????
                let args_sexp = sexp.as_ref().tail_ref();

                //TODO GC Capture:
                let iter = args_sexp.as_ref().iter();
                for sexp in iter {
                    //TODO GC Capture: sexp
                    let sexp = NBox::new(sexp.as_mut_ptr());
                    args.push(eval(&sexp, ctx));
                }

                if closure.as_ref().process_arguments_descriptor(ctx, &mut args) {
                    closure.as_ref().apply(ctx, &args)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", closure, args)
                }

            } else {
                panic!("Not Applicable: {:?}", head.as_ref())
            }
        }

    } else if let Some(symbol) = sexp.duplicate().into_nbox::<symbol::Symbol>() {
        if let Some(v) = ctx.find_value(&symbol) {
            v
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else {
        sexp.duplicate()
    }

}

#[cfg(test)]
mod tets {
    use crate::read::*;
    use crate::value::*;
    use crate::object::*;

    fn read(ctx: &mut Object, program: &str) -> NBox<Value> {
        let mut ctx = ReadContext::new(ctx, program.chars().peekable());

        let result = crate::read::read(&mut ctx);
        assert!(result.is_ok());

        result.unwrap()
    }

    fn eval<T: NaviType>(ctx: &mut Object, sexp: &NBox<Value>) -> NBox<T> {
        let result = crate::eval::eval(&sexp, ctx);

        let result = result.into_nbox::<T>();
        assert!(result.is_some());

        result.unwrap()
    }

    #[test]
    fn func_test() {
        let mut ctx = Object::new("eval");
        let mut ans_ctx = Object::new(" ans");

        number::register_global(&mut ctx);

        {
            let program = "(abs 1)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(result, ans);

            let program = "(abs -1)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(result, ans);

            let program = "(abs -3.14)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Real::alloc(&mut ans_ctx, 3.14);
            assert_eq!(result, ans);
        }

        {
            let program = "(+ 1)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(result, ans);

            let program = "(+ 3.14)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Real::alloc(&mut ans_ctx, 3.14);
            assert_eq!(result, ans);

            let program = "(+ 1 2 3 -4)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 2);
            assert_eq!(result, ans);

            let program = "(+ 1.5 2 3 -4.5)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Real::alloc(&mut ans_ctx, 2.0);
            assert_eq!(result, ans);
        }

        //TODO Optional引数のテスト

    }

    #[test]
    fn syntax_if_test() {
        let mut ctx = Object::new("eval");
        let mut ans_ctx = Object::new(" ans");

        number::register_global(&mut ctx);
        syntax::register_global(&mut ctx);

        {
            let program = "(if (= 1 1) 10 100)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 10);
            assert_eq!(result, ans);

            let program = "(if (= 1 2) 10 100)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 100);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 1) 10)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 10);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 2) 10)";
            let result = read(&mut ctx, program);
            let result:NBox<Value>  = eval(&mut ctx, &result);
            assert!(result.is::<unit::Unit>())
        }
    }

    #[test]
    fn syntax_fun_test() {
        let mut ctx = Object::new("eval");
        let mut ans_ctx = Object::new(" ans");

        number::register_global(&mut ctx);
        syntax::register_global(&mut ctx);

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 11);
            assert_eq!(result, ans);

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = read(&mut ctx, program);
            let result = eval(&mut ctx, &result);
            let ans = number::Integer::alloc(&mut ans_ctx, 310);
            assert_eq!(result, ans);
        }
    }

}