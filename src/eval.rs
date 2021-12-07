use crate::value::*;
use crate::mm::{Heap};
use crate::world::World;

pub struct Context<'a, 'b> {
    pub heap: &'a mut Heap,
    world: &'b mut World,
}

pub fn eval(sexp: &NBox<Value>, ctx: &mut Context) -> NBox<Value> {
    if let Some(sexp) = sexp.duplicate().into_nbox::<list::List>() {
        if sexp.as_ref().is_nil() {
            sexp.into_nboxvalue()
        } else {
            //TODO GC capture: sexp

            //TODO GC capture: head_evaled
            let head_evaled = eval(sexp.as_ref().head_ref(), ctx);

            if let Some(func) = head_evaled.as_ref().try_cast::<func::Func>() {
                //関数適用
                //TODO GC capture: args
                let mut args: Vec<NBox<Value>> = Vec::new();
                for sexp in sexp.as_ref().tail_ref().iter() {
                    args.push(eval(sexp, ctx));
                }

                if func.process_arguments_descriptor(&mut args, ctx) {
                    func.apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", func, args)
                }

            } else if let Some(syntax) = head_evaled.as_ref().try_cast::<syntax::Syntax>() {
                //シンタックス適用
                let args = sexp.as_ref().tail_ref();
                if syntax.check_arguments(args) {
                    syntax.apply(args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", syntax, args)
                }
            } else {
                panic!("Not Applicable: {:?}", head_evaled.as_ref())
            }
        }

    } else if let Some(symbol) = sexp.duplicate().into_nbox::<symbol::Symbol>() {
        if let Some(v) = ctx.world.get(symbol.as_ref()) {
            v.duplicate()
        } else {
            panic!("{:?} is not found", symbol.as_ref())
        }

    } else {
        sexp.duplicate()
    }

}

#[cfg(test)]
mod tets {
    use crate::mm::{Heap};
    use crate::world::{World};
    use crate::read::*;
    use crate::value::*;

    fn make_read_context<'a, 'b>(heap: &'a mut Heap, s: &'b str) -> ReadContext<'b, 'a> {
        ReadContext::new(Input::new(s), heap)
    }

    fn read(program: &str, heap: &mut Heap) -> NBox<Value> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut ctx = make_read_context(heap, program);

        let result = crate::read::read(&mut ctx);
        assert!(result.is_ok());

        result.unwrap()
    }

    fn eval<T: NaviType>(sexp: &NBox<Value>, heap: &mut Heap, world: &mut World) -> NBox<T> {
        let mut ctx = crate::eval::Context {
            heap: heap,
            world: world,
        };
        let result = crate::eval::eval(&sexp, &mut ctx);

        let result = result.into_nbox::<T>();
        assert!(result.is_some());

        result.unwrap()
    }

    #[test]
    fn test() {
        let mut heap = Heap::new(10240, "eval");
        let mut ans_heap = Heap::new(1024, " ans");

        let mut world = World::new();
        number::register_global(&mut world);

        {
            let program = "(abs 1)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 1);
            assert_eq!(result, ans);

            let program = "(abs -1)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 1);
            assert_eq!(result, ans);

            let program = "(abs -3.14)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Real::alloc(&mut ans_heap, 3.14);
            assert_eq!(result, ans);
        }

        {
            let program = "(+ 1)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 1);
            assert_eq!(result, ans);

            let program = "(+ 3.14)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Real::alloc(&mut ans_heap, 3.14);
            assert_eq!(result, ans);

            let program = "(+ 1 2 3 -4)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 2);
            assert_eq!(result, ans);

            let program = "(+ 1.5 2 3 -4.5)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Real::alloc(&mut ans_heap, 2.0);
            assert_eq!(result, ans);
        }

        //TODO Optional引数のテスト

        heap.free();
        ans_heap.free();
    }
}