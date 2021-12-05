use crate::value::*;
use crate::mm::{Heap};

#[derive(Debug)]
pub struct ReadError {
    msg: String,
}

fn readerror(msg: String) -> ReadError {
    ReadError { msg: msg}
}

pub type ReadResult = Result<NBox<Value>, ReadError>;

pub struct Input<'a> {
    chars: std::str::Chars<'a>,
    prev_char: Option<char>,
}

impl <'a> Input<'a> {
   pub fn new(text: &'a str) -> Input {
       Input {
           chars: text.chars(),
           prev_char: None,
       }
   }

   pub fn peek(&mut self) -> Option<char> {
       if self.prev_char.is_none() {
           self.prev_char = self.chars.next();
       }

       self.prev_char
    }

    pub fn next(&mut self) -> Option<char> {
        match self.prev_char {
            None => {
                self.chars.next()
            },
            x => {
                self.prev_char = None;
                x
            }
        }
    }
}

pub struct ReadContext<'input, 'heap> {
    input: Input<'input>,
    heap: &'heap mut Heap
}

impl <'input, 'heap> ReadContext<'input, 'heap> {
    pub fn new(input: Input<'input>, heap: &'heap mut Heap) -> Self {
        ReadContext {
            input: input,
            heap: heap,
        }
    }
}

pub fn read<'a>(ctx: &'a mut ReadContext) -> ReadResult {
    read_internal(ctx)
}

fn read_internal(ctx: &mut ReadContext) -> ReadResult {
    skip_whitespace(ctx);

    match ctx.input.peek() {
        None => Err(readerror("読み込む内容がない".to_string())),
        Some(ch) => match ch {
            '(' => read_list(ctx),
            '"' => read_string(ctx),
            '\'' => read_char(ctx),
            '+' | '-' | '0' ..= '9' => read_number_or_symbol(ctx),
            _ => read_symbol(ctx),
        }
    }
}


fn read_list(ctx: &mut ReadContext) -> ReadResult {
    //skip first char
    ctx.input.next();

    let mut acc: Vec<NBox<Value>> = Vec::new();
    loop {
        skip_whitespace(ctx);
        match ctx.input.peek() {
            None => return Err(readerror("リストが完結する前にEOFになった".to_string())),
            Some(')') => {
                ctx.input.next();
                // complete!
                let list = list::List::from_vec(ctx.heap, acc);
                return Ok(list.into_nboxvalue());
            }
            Some(_) => {
                //再帰的にreadを呼び出す
                match read_internal(ctx) {
                    //内部でエラーが発生した場合は途中停止
                    Err(msg) => return Err(msg),
                    //リストの要素としてvec内に保存してループを継続
                    Ok(v) => acc.push(v),
                }
            }
        }
    }
}

fn read_string(ctx: &mut ReadContext) -> ReadResult {
    //skip first char
    ctx.input.next();

    //終了文字'"'までのすべての文字を読み込み文字列をぶじぇくとを作成する
    let mut acc: Vec<char> = Vec::new();
    loop {
        match ctx.input.next() {
            None => return Err(readerror("文字列が完結する前にEOFになった".to_string())),
            Some('\"') => {
                let str: String = acc.into_iter().collect();
                let str = string::NString::alloc(ctx.heap, &str);
                return Ok(str.into_nboxvalue());
            }
            Some(ch) => {
                acc.push(ch);
            }
        }
    }
}

fn read_char(ctx: &mut ReadContext) -> ReadResult {
    Err(readerror("TODO".to_string()))
}

fn read_number_or_symbol(ctx: &mut ReadContext) -> ReadResult {
    match read_word(ctx) {
        Ok(str) => match str.parse::<i64>() {
                Ok(num) => {
                    //integer
                    let num = number::Integer::alloc(ctx.heap, num);
                    return Ok(num.into_nboxvalue());
                },
                Err(_) => match str.parse::<f64>() {
                    Ok(num) => {
                        //floating number
                        let num = number::Real::alloc(ctx.heap, num);
                        return Ok(num.into_nboxvalue());
                    }
                    Err(_) => {
                        //symbol
                        let symbol = symbol::Symbol::alloc(ctx.heap, &str);
                        return Ok(symbol.into_nboxvalue());
                    }
                }
            }
        Err(err) => Err(err),
    }
}

fn read_symbol(ctx: &mut ReadContext) -> ReadResult {
    match read_word(ctx) {
        Ok(str) => match &*str {
            "true" =>Ok(bool::Bool::true_().into_nboxvalue()),
            "false" =>Ok(bool::Bool::false_().into_nboxvalue()),
            _ => Ok(symbol::Symbol::alloc(ctx.heap, &str).into_nboxvalue()),
        }
        Err(err) => Err(err),
    }
}

fn read_word(ctx: &mut ReadContext) -> Result<String, ReadError> {
    let mut acc: Vec<char> = Vec::new();
    loop {
        match ctx.input.peek() {
            None => {
                if acc.is_empty() {
                    return Err(readerror("ワードが存在しない".to_string()));
                } else {
                    let str: String = acc.into_iter().collect();
                    return Ok(str);
                }
            }
            Some(ch) if is_delimiter(ch) => {
                let str: String = acc.into_iter().collect();
                return Ok(str);
            }
            Some(ch) => {
                acc.push(ch);
                ctx.input.next();
            }
        }
    }
}

#[inline(always)]
const fn is_whitespace(ch: char) -> bool {
    match ch {
        '\u{0020}' | // space ' '
        '\u{0009}' | // tab '\t'
        '\u{000A}' | // line feed '\n'
        '\u{000D}' | // carrige return '\r'
        '\u{000B}' | // vertical tab
        '\u{000C}' | // form feed
        '\u{0085}' | // nextline
        '\u{200E}' | // left-to-right mark
        '\u{200F}' | // right-to-left mark
        '\u{2028}' | // line separator
        '\u{2029}'   // paragraph separator
          => true,
        _ => false,
    }
}

fn skip_whitespace(ctx: &mut ReadContext) {
    let mut next = ctx.input.peek();
    while let Some(ch) = next {
        if is_whitespace(ch) {
            //Skip!!
            ctx.input.next();
            next = ctx.input.peek();
        } else {
            next = None;
        }
    }
}

#[inline(always)]
const fn is_delimiter(ch: char) -> bool {
    is_whitespace(ch)
        || match ch {
            '"' |
            '\''|
            '(' |
            ')' |
            '[' |
            ']'
              => true,
            _ => false,
        }

}

#[cfg(test)]
mod tets {
    use crate::mm::{Heap};
    use crate::read::*;
    use crate::value::*;

    fn make_read_context<'a, 'b>(heap: &'a mut Heap, s: &'b str) -> ReadContext<'b, 'a> {
        ReadContext::new(Input::new(s), heap)
    }

    #[test]
    fn read_empty() {
        let mut heap = Heap::new(1024, "test");
        let program = r#"
                
        "#;

        let mut ctx = make_read_context(&mut heap, program);
        let result  = crate::read::read(&mut ctx);
        assert!(result.is_err());

        heap.free();
    }

    fn read<'ti, T: NaviType>(program: &str, typeinfo: &'ti TypeInfo<T>, heap: &mut Heap) -> NBox<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut ctx = make_read_context(heap, program);

        read_with_ctx(&mut ctx, typeinfo)
    }

    fn read_with_ctx<'ti, T: NaviType>(ctx: &mut ReadContext, typeinfo: &'ti TypeInfo<T>) -> NBox<T> {
        let result = crate::read::read(ctx);
        assert!(result.is_ok());

        let result: Option<NBox<T>> = result.unwrap().into_nbox(typeinfo);
        assert!(result.is_some());

        result.unwrap()
    }

    #[test]
    fn read_string() {
        let mut heap = Heap::new(1024, "string");
        let mut ans_heap = Heap::new(1024, " ans");

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
        let mut heap = Heap::new(1024, "int");
        let mut ans_heap = Heap::new(1024, "int ans");

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
        let mut heap = Heap::new(1024, "float");
        let mut ans_heap = Heap::new(1024, " ans");

        {
            let program = "1.0";

            let result = read(program, number::Real::typeinfo(), &mut heap);
            let ans = number::Real::alloc(&mut ans_heap, 1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "-1.0";

            let result = read(program, number::Real::typeinfo(), &mut heap);
            let ans = number::Real::alloc(&mut ans_heap, -1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "+1.0";

            let result = read(program, number::Real::typeinfo(), &mut heap);
            let ans = number::Real::alloc(&mut ans_heap, 1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "3.14";

            let result = read(program, number::Real::typeinfo(), &mut heap);
            let ans = number::Real::alloc(&mut ans_heap, 3.14);
            assert_eq!(result, ans);
        }

        {
            let program = "0.5";

            let result = read(program, number::Real::typeinfo(), &mut heap);
            let ans = number::Real::alloc(&mut ans_heap, 0.5);
            assert_eq!(result, ans);
        }

        heap.free();
        ans_heap.free();
    }

    #[test]
    fn read_symbol() {
        let mut heap = Heap::new(1024, "symbol");
        let mut ans_heap = Heap::new(1024, " ans");

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
        let mut heap = Heap::new(1024, "list");
        let mut ans_heap = Heap::new(1024, " ans");

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
            let _3_14 = number::Real::alloc(&mut ans_heap, 3.14).into_nboxvalue();
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

}