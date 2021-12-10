use crate::value::*;
use crate::object::{Object};

pub fn eval(sexp: &NBox<Value>, ctx: &mut Object) -> NPtr<Value> {
    if let Some(sexp) = sexp.try_cast::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.get().clone().into_value()
        } else {
            let head = NBox::new(sexp.as_ref().head_ref(), ctx);
            //リスト先頭の式を評価
            let head = NBox::new(eval(&head, ctx), ctx);

            if let Some(func) = head.try_cast::<func::Func>() {
                //関数適用

                //引数を順に評価してvec内に保存
                let mut args: Vec<NBox<Value>> = Vec::new();
                let args_sexp = NBox::new(sexp.as_ref().tail_ref(), ctx);
                for sexp in args_sexp.as_ref().iter() {
                    let sexp = NBox::new(sexp.clone(), ctx);
                    args.push(NBox::new(eval(&sexp, ctx), ctx));
                }

                if func.as_ref().process_arguments_descriptor(&mut args, ctx) {
                    func.as_ref().apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", func, args)
                }

            } else if let Some(syntax) = head.try_cast::<syntax::Syntax>() {
                //シンタックス適用
                //TODO GC Capture:
                let args = NBox::new(sexp.as_ref().tail_ref(), ctx);
                if syntax.as_ref().check_arguments(&args) {
                    syntax.as_ref().apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax, args)
                }
            } else if let Some(closure) = head.try_cast::<closure::Closure>() {
                //クロージャ適用

                //TODO GC capture: args, iter
                let mut args: Vec<NBox<Value>> = Vec::new();

                let args_sexp = NBox::new(sexp.as_ref().tail_ref(), ctx);

                for sexp in args_sexp.as_ref().iter() {
                    let sexp = NBox::new(sexp.clone(), ctx);
                    args.push(NBox::new(eval(&sexp, ctx), ctx));
                }

                if closure.as_ref().process_arguments_descriptor(&mut args, ctx) {
                    closure.as_ref().apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", closure, args)
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
        sexp.get().clone()
    }
}

#[cfg(test)]
mod tets {
    use crate::read::*;
    use crate::value::*;
    use crate::object::*;

    fn read(program: &str, ctx: &mut Object) -> NBox<Value> {
        let exp = {
            let mut ctx = ReadContext::new(program.chars().peekable(), ctx);

            let result = crate::read::read(&mut ctx);
            assert!(result.is_ok());
            result.unwrap()
        };

        NBox::new(exp, ctx)
    }

    fn eval<T: NaviType>(sexp: &NBox<Value>, ctx: &mut Object) -> NBox<T> {
        let result = crate::eval::eval(&sexp, ctx);

        let result = result.try_cast::<T>();
        assert!(result.is_some());

        NBox::new(result.unwrap().clone(), ctx)
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
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(abs -1)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(abs -3.14)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Real::alloc(3.14, ans_ctx), ans_ctx);

            assert_eq!(result, ans);
        }

        {
            let program = "(+ 1)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(+ 3.14)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Real::alloc(3.14, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(+ 1 2 3 -4)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(2, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(+ 1.5 2 3 -4.5)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Real::alloc(2.0, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
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
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(10, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(if (= 1 2) 10 100)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(100, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 1) 10)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(10, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 2) 10)";
            let result = read(program, ctx);
            let result:NBox<Value>  = eval(&result, ctx);
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
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(11, ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = read(program, ctx);
            let result = eval(&result, ctx);
            let ans = NBox::new(number::Integer::alloc(310, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }
    }

}