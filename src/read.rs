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

pub type ReadResult = Result<NPtr<Value>, ReadError>;

pub struct ReadContext<'i, 'o> {
    input: Peekable<Chars<'i>>,
    obj: &'o mut Object
}

impl <'i, 'o> ReadContext<'i, 'o> {
    pub fn new(input: Peekable<Chars<'i>>, ctx: &'o mut Object) -> Self {
        ReadContext {
            input: input,
            obj: ctx,
        }
    }
}

pub fn read(ctx: &mut ReadContext) -> ReadResult {
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
                let list = list::List::from_vec(acc, &mut ctx.obj);
                return Ok(list.into_value());
            }
            Some(_) => {
                //再帰的にreadを呼び出す
                match read_internal(ctx) {
                    //内部でエラーが発生した場合は途中停止
                    Err(msg) => return Err(msg),
                    //リストの要素としてvec内に保存してループを継続
                    Ok(v) => acc.push(NBox::new(v, ctx.obj)),
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
                let str = string::NString::alloc(&str, &mut ctx.obj);
                return Ok(str.into_value());
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
                    let num = number::Integer::alloc(num, &mut ctx.obj);
                    return Ok(num.into_value());
                },
                Err(_) => match str.parse::<f64>() {
                    Ok(num) => {
                        //floating number
                        let num = number::Real::alloc(num, &mut ctx.obj);
                        return Ok(num.into_value());
                    }
                    Err(_) => {
                        //symbol
                        let symbol = symbol::Symbol::alloc(&str, &mut ctx.obj);
                        return Ok(symbol.into_value());
                    }
                }
            }
        Err(err) => Err(err),
    }
}

fn read_symbol(ctx: &mut ReadContext) -> ReadResult {
    match read_word(ctx) {
        Ok(str) => match &*str {
            "true" =>Ok(bool::Bool::true_().into_value()),
            "false" =>Ok(bool::Bool::false_().into_value()),
            _ => Ok(symbol::Symbol::alloc(&str, &mut ctx.obj).into_value()),
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

    fn make_read_context<'a, 'b>(s: &'a str, ctx: &'b mut Object) -> ReadContext<'a, 'b> {
        ReadContext::new( s.chars().peekable(), ctx)
    }

    #[test]
    fn read_empty() {
        let mut ctx = Object::new("test");

        let program = r#"
                
        "#;

        let mut ctx = make_read_context( program, &mut ctx);
        let result  = crate::read::read(&mut ctx);
        assert!(result.is_err());
    }

    fn read<T: NaviType>(program: &str, ctx: &mut Object) -> NBox<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut ctx = make_read_context(program, ctx);

        read_with_ctx(&mut ctx)
    }

    fn read_with_ctx<T: NaviType>(ctx: &mut ReadContext) -> NBox<T> {
        let result = {
            let result = crate::read::read(ctx);
            assert!(result.is_ok());

            let result = result.unwrap().try_into::<T>();
            assert!(result.is_some());
            result.unwrap()
        };

        NBox::new(result, &mut ctx.obj)
    }

    #[test]
    fn read_string() {
        let mut ctx = Object::new("string");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = r#"
            "aiueo"
            "#;

            let result = read::<string::NString>(program, ctx);
            let ans = NBox::new(string::NString::alloc(&"aiueo".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = r#"
            "1 + (1 - 3) = -1"
            "3 * (4 / 2) - 12 = -6   "
            "#;

            let mut ctx = make_read_context(program, ctx);

            let result = read_with_ctx::<string::NString>(&mut ctx);
            let ans = NBox::new(string::NString::alloc(&"1 + (1 - 3) = -1".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx::<string::NString>(&mut ctx);
            let ans = NBox::new(string::NString::alloc(&"3 * (4 / 2) - 12 = -6   ".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_int() {
        let mut ctx = Object::new("int");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "1";

            let result = read::<number::Integer>(program, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "-1";

            let result = read::<number::Integer>(program, ctx);
            let ans = NBox::new(number::Integer::alloc(-1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "+1";

            let result = read::<number::Integer>(program, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_float() {
        let mut ctx = Object::new("float");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = NBox::new(number::Real::alloc(1.0, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "-1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = NBox::new(number::Real::alloc(-1.0, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "+1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = NBox::new(number::Real::alloc(1.0, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "3.14";

            let result = read::<number::Real>(program, ctx);
            let ans = NBox::new(number::Real::alloc(3.14, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "0.5";

            let result = read::<number::Real>(program, ctx);
            let ans = NBox::new(number::Real::alloc(0.5, ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_symbol() {
        let mut ctx = Object::new("symbol");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "symbol";

            let result = read::<symbol::Symbol>(program, ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "s1 s2   s3";

            let mut ctx = make_read_context(program, ctx);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"s1".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"s2".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);


            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"s3".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "+ - +1-2 -2*3/4";

            let mut ctx = make_read_context(program, ctx);

            let result = read_with_ctx::<symbol::Symbol>(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"+".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"-".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"+1-2".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = NBox::new(symbol::Symbol::alloc(&"-2*3/4".to_string(), ans_ctx), ans_ctx);
            assert_eq!(result, ans);
        }

        //special symbol
        {
            let program = "true false";

            let mut ctx = make_read_context(program, ctx);

            let result = read_with_ctx(&mut ctx);
            let ans = NBox::new(bool::Bool::true_(), ans_ctx);
            assert_eq!(result, ans);

            let result = read_with_ctx(&mut ctx);
            let ans = NBox::new(bool::Bool::false_(), ans_ctx);
            assert_eq!(result, ans);
        }
    }

    #[test]
    fn read_list() {
        let mut ctx = Object::new("list");
        let ctx = &mut ctx;
        let mut ans_ctx = Object::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "()";

            let result = read(program, ctx);
            let ans = NBox::new(list::List::nil(), ans_ctx);
            assert_eq!(result, ans);
        }

        {
            let program = "(1 2 3)";

            let result = read(program, ctx);

            let _1 = NBox::new(number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let _2 = NBox::new(number::Integer::alloc(2, ans_ctx).into_value(), ans_ctx);
            let _3 = NBox::new(number::Integer::alloc(3, ans_ctx).into_value(), ans_ctx);
            let ans = NBox::new(list::List::nil(), ans_ctx);
            let ans = NBox::new(list::List::alloc(&_3, &ans, ans_ctx), ans_ctx);
            let ans = NBox::new(list::List::alloc(&_2, &ans, ans_ctx), ans_ctx);
            let ans = NBox::new(list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);

            assert_eq!(result, ans);
        }

        {
            let program = "(1 3.14 \"hohoho\" symbol)";

            let result = read(program, ctx);

            let _1 = NBox::new(number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let _3_14 = NBox::new(number::Real::alloc(3.14, ans_ctx).into_value(), ans_ctx);
            let hohoho = NBox::new(string::NString::alloc(&"hohoho".to_string(), ans_ctx).into_value(), ans_ctx);
            let symbol = NBox::new(symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx).into_value(), ans_ctx);
            let ans = NBox::new(list::List::nil(), ans_ctx);
            let ans = NBox::new(list::List::alloc(&symbol, &ans, ans_ctx), ans_ctx);
            let ans = NBox::new(list::List::alloc(&hohoho, &ans, ans_ctx), ans_ctx);
            let ans = NBox::new(list::List::alloc(&_3_14, &ans, ans_ctx), ans_ctx);
            let ans = NBox::new(list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);

            assert_eq!(result, ans);
        }
    }

}