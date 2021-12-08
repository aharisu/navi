use crate::value::*;
use crate::mm::{Heap};
use crate::world::World;

pub struct Context<'a, 'b> {
    pub heap: &'a mut Heap,
    world: &'b mut World,
    frames: Vec<Vec<(NPtr<symbol::Symbol>, NPtr<Value>)>>,
}

impl <'a, 'b> Context<'a, 'b> {
    pub fn new(heap: &'a mut Heap, world: &'b mut World) -> Self {
        Context {
            heap: heap,
            world: world,
            frames: Vec::new(),
        }
    }

    pub fn push_local_frame<T, U>(&mut self, frame: &[(&T, &U)])
    where
        T: crate::value::AsPtr<symbol::Symbol>,
        U: crate::value::AsPtr<Value>
    {
        let mut vec = Vec::<(NPtr<symbol::Symbol>, NPtr<Value>)>::new();
        for (symbol, v) in frame {
            vec.push((NPtr::new(symbol.as_mut_ptr()), NPtr::new(v.as_mut_ptr())));
        }

        self.frames.push(vec);
    }

    pub fn pop_local_frame(&mut self) {
        self.frames.pop();
    }

    pub fn find_value(&self, symbol: &NBox<symbol::Symbol>) -> Option<NBox<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            let result = frame.iter().find(|(sym, v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(NBox::new(v.as_mut_ptr()));
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(v.duplicate())
        } else {
            None
        }
    }

}

pub fn eval(sexp: &NBox<Value>, ctx: &mut Context) -> NBox<Value> {
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

                if func.as_ref().process_arguments_descriptor(&mut args, ctx) {
                    func.as_ref().apply(&args, ctx)

                } else {
                    panic!("Invalid arguments: {:?} {:?}", func, args)
                }

            } else if let Some(syntax) = head.try_cast::<syntax::Syntax>() {
                //シンタックス適用
                //TODO GC Capture:
                let args = sexp.as_ref().tail_ref();
                if syntax.as_ref().check_arguments(&args) {
                    syntax.as_ref().apply(&args, ctx)

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

                if closure.as_ref().process_arguments_descriptor(&mut args, ctx) {
                    closure.as_ref().apply(&args, ctx)

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
        let mut ctx = crate::eval::Context::new(heap, world);
        let result = crate::eval::eval(&sexp, &mut ctx);

        let result = result.into_nbox::<T>();
        assert!(result.is_some());

        result.unwrap()
    }

    #[test]
    fn func_test() {
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

    #[test]
    fn syntax_if_test() {
        let mut heap = Heap::new(10240, "eval");
        let mut ans_heap = Heap::new(1024, " ans");

        let mut world = World::new();
        number::register_global(&mut world);
        syntax::register_global(&mut world);

        {
            let program = "(if (= 1 1) 10 100)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 10);
            assert_eq!(result, ans);

            let program = "(if (= 1 2) 10 100)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 100);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 1) 10)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 10);
            assert_eq!(result, ans);

            let program = "(if (= 1 1 2) 10)";
            let result = read(program, &mut heap);
            let result:NBox<Value>  = eval(&result, &mut heap, &mut world);
            assert!(result.is::<unit::Unit>())
        }

        heap.free();
        ans_heap.free();
    }

    #[test]
    fn syntax_fun_test() {
        let mut heap = Heap::new(10240, "eval");
        let mut ans_heap = Heap::new(1024, " ans");

        let mut world = World::new();
        number::register_global(&mut world);
        syntax::register_global(&mut world);

        {
            let program = "((fun (a) (+ 10 a)) 1)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 11);
            assert_eq!(result, ans);

            let program = "((fun (a b) (+ a b) (+ ((fun (a) (+ a 10)) b) a)) 100 200)";
            let result = read(program, &mut heap);
            let result = eval(&result, &mut heap, &mut world);
            let ans = number::Integer::alloc(&mut ans_heap, 310);
            assert_eq!(result, ans);
        }

        heap.free();
        ans_heap.free();
    }

}