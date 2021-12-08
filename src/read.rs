use std::iter::Peekable;
use std::str::Chars;

use crate::value::*;
use crate::object::{Object};

#[derive(Debug)]
pub struct ReadError {
    msg: String,
}

fn readerror(msg: String) -> ReadError {
    ReadError { msg: msg}
}

pub type ReadResult = Result<NBox<Value>, ReadError>;

pub struct ReadContext<'i, 'o> {
    input: Peekable<Chars<'i>>,
    obj: &'o mut Object
}

impl <'i, 'o> ReadContext<'i, 'o> {
    pub fn new(ctx: &'o mut Object, input: Peekable<Chars<'i>>) -> Self {
        ReadContext {
            input: input,
            obj: ctx,
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
                let list = list::List::from_vec(&mut ctx.obj, acc);
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
                let str = string::NString::alloc(&mut ctx.obj, &str);
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
                    let num = number::Integer::alloc(&mut ctx.obj, num);
                    return Ok(num.into_nboxvalue());
                },
                Err(_) => match str.parse::<f64>() {
                    Ok(num) => {
                        //floating number
                        let num = number::Real::alloc(&mut ctx.obj, num);
                        return Ok(num.into_nboxvalue());
                    }
                    Err(_) => {
                        //symbol
                        let symbol = symbol::Symbol::alloc(&mut ctx.obj, &str);
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
            _ => Ok(symbol::Symbol::alloc(&mut ctx.obj, &str).into_nboxvalue()),
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
            Some(ch) if is_delimiter(*ch) => {
                let str: String = acc.into_iter().collect();
                return Ok(str);
            }
            Some(ch) => {
                acc.push(*ch);
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
        if is_whitespace(*ch) {
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
    use crate::read::*;
    use crate::value::*;
    use crate::object::Object;

    fn make_read_context<'a, 'b>(ctx: &'a mut Object, s: &'b str) -> ReadContext<'b, 'a> {
        ReadContext::new(ctx, s.chars().peekable())
    }

    #[test]
    fn read_empty() {
        let mut ctx = Object::new("test");

        let program = r#"
                
        "#;

        let mut ctx = make_read_context(&mut ctx, program);
        let result  = crate::read::read(&mut ctx);
        assert!(result.is_err());
    }

    fn read<T: NaviType>(ctx: &mut Object, program: &str) -> NBox<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut ctx = make_read_context(ctx, program);

        read_with_ctx(&mut ctx)
    }

    fn read_with_ctx<T: NaviType>(ctx: &mut ReadContext) -> NBox<T> {
        let result = crate::read::read(ctx);
        assert!(result.is_ok());

        let result: Option<NBox<T>> = result.unwrap().into_nbox::<T>();
        assert!(result.is_some());

        result.unwrap()
    }

    #[test]
    fn read_string() {
        let mut ctx = Object::new("string");
        let mut ans_ctx = Object::new(" ans");

        {
            let program = r#"
            "aiueo"
            "#;

            let result = read::<string::NString>(&mut ctx, program);
            let ans = string::NString::alloc(&mut ans_ctx, &"aiueo".to_string());
            assert_eq!(result, ans);
        }

        {
            let program = r#"
            "1 + (1 - 3) = -1"
            "3 * (4 / 2) - 12 = -6   "
            "#;

            let mut ctx = make_read_context(&mut ctx, program);

            let result = read_with_ctx::<string::NString>(&mut ctx);
            let ans = string::NString::alloc(&mut ans_ctx, &"1 + (1 - 3) = -1".to_string());
            assert_eq!(result, ans);

            let result = read_with_ctx::<string::NString>(&mut ctx);
            let ans = string::NString::alloc(&mut ans_ctx, &"3 * (4 / 2) - 12 = -6   ".to_string());
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_int() {
        let mut ctx = Object::new("int");
        let mut ans_ctx = Object::new("int ans");

        {
            let program = "1";

            let result = read::<number::Integer>(&mut ctx, program);
            let ans = number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(result, ans);
        }

        {
            let program = "-1";

            let result = read::<number::Integer>(&mut ctx, program);
            let ans = number::Integer::alloc(&mut ans_ctx, -1);
            assert_eq!(result, ans);
        }

        {
            let program = "+1";

            let result = read::<number::Integer>(&mut ctx, program);
            let ans = number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_float() {
        let mut ctx = Object::new("int");
        let mut ans_ctx = Object::new("int ans");

        {
            let program = "1.0";

            let result = read::<number::Real>(&mut ctx, program);
            let ans = number::Real::alloc(&mut ans_ctx, 1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "-1.0";

            let result = read::<number::Real>(&mut ctx, program);
            let ans = number::Real::alloc(&mut ans_ctx, -1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "+1.0";

            let result = read::<number::Real>(&mut ctx, program);
            let ans = number::Real::alloc(&mut ans_ctx, 1.0);
            assert_eq!(result, ans);
        }

        {
            let program = "3.14";

            let result = read::<number::Real>(&mut ctx, program);
            let ans = number::Real::alloc(&mut ans_ctx, 3.14);
            assert_eq!(result, ans);
        }

        {
            let program = "0.5";

            let result = read::<number::Real>(&mut ctx, program);
            let ans = number::Real::alloc(&mut ans_ctx, 0.5);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_symbol() {
        let mut ctx = Object::new("symbol");
        let mut ans_ctx = Object::new("symbol ans");

        {
            let program = "symbol";

            let result = read::<symbol::Symbol>(&mut ctx, program);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"symbol".to_string());
            assert_eq!(result, ans);
        }

        {
            let program = "s1 s2   s3";

            let mut ctx = make_read_context(&mut ctx, program);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"s1".to_string());
            assert_eq!(result, ans);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"s2".to_string());
            assert_eq!(result, ans);


            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"s3".to_string());
            assert_eq!(result, ans);
        }

        {
            let program = "+ - +1-2 -2*3/4";

            let mut ctx = make_read_context(&mut ctx, program);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"+".to_string());
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"-".to_string());
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"+1-2".to_string());
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = symbol::Symbol::alloc(&mut ans_ctx, &"-2*3/4".to_string());
            assert_eq!(result, ans);
        }

        //special symbol
        {
            let program = "true false";

            let mut ctx = make_read_context(&mut ctx, program);

            let result = read_with_ctx(&mut ctx);
            let ans = bool::Bool::true_();
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = bool::Bool::false_();
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_list() {
        let mut ctx = Object::new("list");
        let mut ans_ctx = Object::new("list ans");

        {
            let program = "()";

            let result = read(&mut ctx, program);
            let ans = list::List::nil();
            assert_eq!(result, ans);
        }

        {
            let program = "(1 2 3)";

            let result = read(&mut ctx, program);

            let _1 = number::Integer::alloc(&mut ans_ctx, 1).into_nboxvalue();
            let _2 = number::Integer::alloc(&mut ans_ctx, 2).into_nboxvalue();
            let _3 = number::Integer::alloc(&mut ans_ctx, 3).into_nboxvalue();
            let ans = list::List::nil();
            let ans = list::List::alloc(&mut ans_ctx, &_3, ans);
            let ans = list::List::alloc(&mut ans_ctx, &_2, ans);
            let ans = list::List::alloc(&mut ans_ctx, &_1, ans);

            assert_eq!(result, ans);
        }

        {
            let program = "(1 3.14 \"hohoho\" symbol)";

            let result = read(&mut ctx, program);

            let _1 = number::Integer::alloc(&mut ans_ctx, 1).into_nboxvalue();
            let _3_14 = number::Real::alloc(&mut ans_ctx, 3.14).into_nboxvalue();
            let hohoho = string::NString::alloc(&mut ans_ctx, &"hohoho".to_string()).into_nboxvalue();
            let symbol = symbol::Symbol::alloc(&mut ans_ctx, &"symbol".to_string()).into_nboxvalue();
            let ans = list::List::nil();
            let ans = list::List::alloc(&mut ans_ctx, &symbol, ans);
            let ans = list::List::alloc(&mut ans_ctx, &hohoho, ans);
            let ans = list::List::alloc(&mut ans_ctx, &_3_14, ans);
            let ans = list::List::alloc(&mut ans_ctx, &_1, ans);

            assert_eq!(result, ans);
        }
    }

}