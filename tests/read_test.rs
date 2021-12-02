extern crate navi;

use navi::mm::{Heap};
use navi::read::*;
use navi::value::*;

fn make_read_context<'a, 'b>(heap: &'a mut Heap, s: &'b str) -> ReadContext<'b, 'a> {
    ReadContext::new(Input::new(s), heap)
}

#[test]
fn read_empty() {
    let mut heap = navi::mm::Heap::new(1024, "test".to_string());
    let program = r#"
               
    "#;

    let mut ctx = make_read_context(&mut heap, program);
    let result  = navi::read::read(&mut ctx);
    assert!(result.is_err());

    heap.free();
}

fn read<'ti, T: NaviType>(program: &str, typeinfo: &'ti TypeInfo<T>, heap: &mut navi::mm::Heap) -> NBox<T> {
    //let mut heap = navi::mm::Heap::new(1024, name.to_string());
    let mut ctx = make_read_context(heap, program);

    read_with_ctx(&mut ctx, typeinfo)
}

fn read_with_ctx<'ti, T: NaviType>(ctx: &mut ReadContext, typeinfo: &'ti TypeInfo<T>) -> NBox<T> {
    let result = navi::read::read(ctx);
    assert!(result.is_ok());

    let result: Option<NBox<T>> = result.unwrap().into_nbox(typeinfo);
    assert!(result.is_some());

    result.unwrap()
}

#[test]
fn read_string() {
    let mut heap = navi::mm::Heap::new(1024, "string".to_string());
    let mut ans_heap = navi::mm::Heap::new(1024, " ans".to_string());

    {
        let program = r#"
        "aiueo"
        "#;

        let result = read(program, string::NString::typeinfo(), &mut heap);
        let ans = string::NString::alloc(&mut ans_heap, &"aiueo".to_string());
        assert_eq!(result, ans);
    }

    {
        let program = r#"
        "1 + (1 - 3) = -1"
        "3 * (4 / 2) - 12 = -6   "
        "#;

        let mut ctx = make_read_context(&mut heap, program);

        let result = read_with_ctx(&mut ctx, string::NString::typeinfo());
        let ans = string::NString::alloc(&mut ans_heap, &"1 + (1 - 3) = -1".to_string());
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, string::NString::typeinfo());
        let ans = string::NString::alloc(&mut ans_heap, &"3 * (4 / 2) - 12 = -6   ".to_string());
        assert_eq!(result, ans);
    }

    heap.free();
    ans_heap.free();
}

#[test]
fn read_int() {
    let mut heap = navi::mm::Heap::new(1024, "int".to_string());
    let mut ans_heap = navi::mm::Heap::new(1024, "int ans".to_string());

    {
        let program = "1";

        let result = read(program, number::Integer::typeinfo(), &mut heap);
        let ans = number::Integer::alloc(&mut ans_heap, 1);
        assert_eq!(result, ans);
    }

    {
        let program = "-1";

        let result = read(program, number::Integer::typeinfo(), &mut heap);
        let ans = number::Integer::alloc(&mut ans_heap, -1);
        assert_eq!(result, ans);
    }

    {
        let program = "+1";

        let result = read(program, number::Integer::typeinfo(), &mut heap);
        let ans = number::Integer::alloc(&mut ans_heap, 1);
        assert_eq!(result, ans);
    }

    heap.free();
    ans_heap.free();
}

#[test]
fn read_float() {
    let mut heap = navi::mm::Heap::new(1024, "float".to_string());
    let mut ans_heap = navi::mm::Heap::new(1024, " ans".to_string());

    {
        let program = "1.0";

        let result = read(program, number::Float::typeinfo(), &mut heap);
        let ans = number::Float::alloc(&mut ans_heap, 1.0);
        assert_eq!(result, ans);
    }

    {
        let program = "-1.0";

        let result = read(program, number::Float::typeinfo(), &mut heap);
        let ans = number::Float::alloc(&mut ans_heap, -1.0);
        assert_eq!(result, ans);
    }

    {
        let program = "+1.0";

        let result = read(program, number::Float::typeinfo(), &mut heap);
        let ans = number::Float::alloc(&mut ans_heap, 1.0);
        assert_eq!(result, ans);
    }

    {
        let program = "3.14";

        let result = read(program, number::Float::typeinfo(), &mut heap);
        let ans = number::Float::alloc(&mut ans_heap, 3.14);
        assert_eq!(result, ans);
    }

    {
        let program = "0.5";

        let result = read(program, number::Float::typeinfo(), &mut heap);
        let ans = number::Float::alloc(&mut ans_heap, 0.5);
        assert_eq!(result, ans);
    }

    heap.free();
    ans_heap.free();
}

#[test]
fn read_symbol() {
    let mut heap = navi::mm::Heap::new(1024, "symbol".to_string());
    let mut ans_heap = navi::mm::Heap::new(1024, " ans".to_string());

    {
        let program = "symbol";

        let result = read(program, symbol::Symbol::typeinfo(), &mut heap);
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"symbol".to_string());
        assert_eq!(result, ans);
    }

    {
        let program = "s1 s2   s3";

        let mut ctx = make_read_context(&mut heap, program);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"s1".to_string());
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"s2".to_string());
        assert_eq!(result, ans);


        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"s3".to_string());
        assert_eq!(result, ans);
    }

    {
        let program = "+ - +1-2 -2*3/4";

        let mut ctx = make_read_context(&mut heap, program);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"+".to_string());
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"-".to_string());
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"+1-2".to_string());
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, symbol::Symbol::typeinfo());
        let ans = symbol::Symbol::alloc(&mut ans_heap, &"-2*3/4".to_string());
        assert_eq!(result, ans);
    }

    //special symbol
    {
        let program = "true false";

        let mut ctx = make_read_context(&mut heap, program);

        let result = read_with_ctx(&mut ctx, bool::Bool::typeinfo());
        let ans = bool::Bool::true_();
        assert_eq!(result, ans);

        let result = read_with_ctx(&mut ctx, bool::Bool::typeinfo());
        let ans = bool::Bool::false_();
        assert_eq!(result, ans);
    }

    heap.free();
    ans_heap.free();
}

#[test]
fn read_list() {
    let mut heap = navi::mm::Heap::new(1024, "list".to_string());
    let mut ans_heap = navi::mm::Heap::new(1024, " ans".to_string());

    {
        let program = "()";

        let result = read(program, list::List::typeinfo(), &mut heap);
        let ans = list::List::nil();
        assert_eq!(result, ans);
    }

    {
        let program = "(1 2 3)";

        let result = read(program, list::List::typeinfo(), &mut heap);

        let _1 = number::Integer::alloc(&mut ans_heap, 1).into_nboxvalue();
        let _2 = number::Integer::alloc(&mut ans_heap, 2).into_nboxvalue();
        let _3 = number::Integer::alloc(&mut ans_heap, 3).into_nboxvalue();
        let ans = list::List::nil();
        let ans = list::List::alloc(&mut ans_heap, &_3, ans);
        let ans = list::List::alloc(&mut ans_heap, &_2, ans);
        let ans = list::List::alloc(&mut ans_heap, &_1, ans);

        assert_eq!(result, ans);
    }

    {
        let program = "(1 3.14 \"hohoho\" symbol)";

        let result = read(program, list::List::typeinfo(), &mut heap);

        let _1 = number::Integer::alloc(&mut ans_heap, 1).into_nboxvalue();
        let _3_14 = number::Float::alloc(&mut ans_heap, 3.14).into_nboxvalue();
        let hohoho = string::NString::alloc(&mut ans_heap, &"hohoho".to_string()).into_nboxvalue();
        let symbol = symbol::Symbol::alloc(&mut ans_heap, &"symbol".to_string()).into_nboxvalue();
        let ans = list::List::nil();
        let ans = list::List::alloc(&mut ans_heap, &symbol, ans);
        let ans = list::List::alloc(&mut ans_heap, &hohoho, ans);
        let ans = list::List::alloc(&mut ans_heap, &_3_14, ans);
        let ans = list::List::alloc(&mut ans_heap, &_1, ans);

        assert_eq!(result, ans);
    }

    heap.free();
    ans_heap.free();
}