use std::iter::Peekable;
use std::str::Chars;

use crate::{let_listbuilder, new_cap, with_cap, let_cap};
use crate::value::*;
use crate::context::{Context};
use crate::ptr::*;

#[derive(Debug)]
pub struct ReadError {
    msg: String,
}

fn readerror(msg: String) -> ReadError {
    ReadError { msg: msg}
}

pub type ReadResult = Result<FPtr<Value>, ReadError>;

pub struct Reader<'i> {
    input: Peekable<Chars<'i>>,
}

impl <'i> Reader<'i> {
    pub fn new(input: Peekable<Chars<'i>>) -> Self {
        Reader {
            input: input,
        }
    }
}

pub fn read(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    read_internal(reader, ctx)
}

fn read_internal(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    skip_whitespace(reader);

    match reader.input.peek() {
        None => Err(readerror("読み込む内容がない".to_string())),
        Some(ch) => match ch {
            '(' => read_list(reader, ctx),
            '[' => read_array(reader, ctx),
            '"' => read_string(reader, ctx),
            '\'' => read_quote(reader, ctx),
            '+' | '-' | '0' ..= '9' => read_number_or_symbol(reader, ctx),
            ':' => read_keyword(reader, ctx),
            _ => read_symbol(reader, ctx),
        }
    }
}


fn read_list(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    let list = read_sequence(')', reader, ctx)?;
    Ok(list.into_value())
}

fn read_array(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    let list = read_sequence(']', reader, ctx)?;
    let_cap!(list, list, ctx);
    Ok(array::Array::from_list(&list, None, ctx).into_value())
}

fn read_sequence(end_char:char, reader: &mut Reader, ctx: &mut Context) -> Result<FPtr<list::List>, ReadError> {
    //skip first char
    reader.input.next();

    let_listbuilder!(builder, ctx);

    loop {
        skip_whitespace(reader);
        match reader.input.peek() {
            None => return Err(readerror("シーケンスが完結する前にEOFになった".to_string())),
            Some(ch) if *ch == end_char => {
                reader.input.next();
                // complete!
                return Ok(builder.get());
            }
            Some(_) => {
                //再帰的にreadを呼び出す
                match read_internal(reader, ctx) {
                    //内部でエラーが発生した場合は途中停止
                    Err(msg) => return Err(msg),
                    Ok(v) => {
                        with_cap!(v, v, ctx, {
                            builder.append(&v, ctx)
                        })
                    }
                }
            }
        }
    }
}

fn read_string(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    //skip first char
    reader.input.next();

    //終了文字'"'までのすべての文字を読み込み文字列をぶじぇくとを作成する
    let mut acc: Vec<char> = Vec::new();
    loop {
        match reader.input.next() {
            None => return Err(readerror("文字列が完結する前にEOFになった".to_string())),
            Some('\"') => {
                let str: String = acc.into_iter().collect();
                let str = string::NString::alloc(&str, ctx);
                return Ok(str.into_value());
            }
            Some(ch) => {
                acc.push(ch);
            }
        }
    }
}

#[allow(dead_code)]
fn read_char(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    //TODO
    unimplemented!()
}

fn read_quote(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    //skip first char
    reader.input.next();

    //再帰的に式を一つ読み込んでquoteで囲む
    let sexp = match read_internal(reader, ctx) {
        //内部でエラーが発生した場合は途中停止
        Err(msg) => return Err(msg),
        Ok(v) => v,
    };
    let_cap!(sexp, sexp, ctx);

    let_listbuilder!(builder, ctx);
    builder.append(syntax::Syntax::quote().cast_value(), ctx);
    builder.append(&sexp, ctx);
    Ok(builder.get().into_value())
}

fn read_number_or_symbol(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    match read_word(reader, ctx) {
        Ok(str) => match str.parse::<i64>() {
                Ok(num) => {
                    //integer
                    let num = number::Integer::alloc(num, ctx);
                    return Ok(num.into_value());
                },
                Err(_) => match str.parse::<f64>() {
                    Ok(num) => {
                        //floating number
                        let num = number::Real::alloc(num, ctx);
                        return Ok(num.into_value());
                    }
                    Err(_) => {
                        //symbol
                        let symbol = symbol::Symbol::alloc(&str, ctx);
                        return Ok(symbol.into_value());
                    }
                }
            }
        Err(err) => Err(err),
    }
}
fn read_keyword(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    //skip first char
    reader.input.next();

    match read_word(reader, ctx) {
        Ok(str) => Ok(keyword::Keyword::alloc(&str, ctx).into_value()),
        Err(err) => Err(err),
    }
}

fn read_symbol(reader: &mut Reader, ctx: &mut Context) -> ReadResult {
    match read_word(reader, ctx) {
        Ok(str) => match &*str {
            "true" =>Ok(bool::Bool::true_().into_fptr().into_value()),
            "false" =>Ok(bool::Bool::false_().into_fptr().into_value()),
            _ => Ok(symbol::Symbol::alloc(&str, ctx).into_value()),
        }
        Err(err) => Err(err),
    }
}

fn read_word(reader: &mut Reader, ctx: &mut Context) -> Result<String, ReadError> {
    let mut acc: Vec<char> = Vec::new();
    loop {
        match reader.input.peek() {
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
                reader.input.next();
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

fn skip_whitespace(reader: &mut Reader) {
    let mut next = reader.input.peek();
    while let Some(ch) = next {
        if is_whitespace(*ch) {
            //Skip!!
            reader.input.next();
            next = reader.input.peek();
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
mod tests {
    use crate::read::*;
    use crate::context::Context;
    use crate::ptr::*;

    fn make_reader<'a>(s: &'a str) -> Reader<'a> {
        Reader::new( s.chars().peekable())
    }

    #[test]
    fn read_empty() {
        let mut ctx = Context::new("test");
        let ctx = &mut ctx;

        let program = r#"
                
        "#;

        let mut reader = make_reader(program);
        let reader = &mut reader;
        let result  = crate::read::read(reader, ctx);
        assert!(result.is_err());
    }

    fn read<T: NaviType>(program: &str, ctx: &mut Context) -> FPtr<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut reader = make_reader(program);

        read_with_ctx(&mut reader, ctx)
    }

    fn read_with_ctx<T: NaviType>(reader: &mut Reader, ctx: &mut Context) -> FPtr<T> {
        let result = crate::read::read(reader, ctx);
        assert!(result.is_ok());

        let result = result.unwrap();
        let result = result.as_ref().try_cast::<T>();
        assert!(result.is_some());
        FPtr::new(result.unwrap() as *const T as *mut T)
    }

    #[test]
    fn read_string() {
        let mut ctx = Context::new("string");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = r#"
            "aiueo"
            "#;

            let result = read::<string::NString>(program, ctx);
            let ans = string::NString::alloc(&"aiueo".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = r#"
            "1 + (1 - 3) = -1"
            "3 * (4 / 2) - 12 = -6   "
            "#;

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<string::NString>(reader, ctx);
            let ans = string::NString::alloc(&"1 + (1 - 3) = -1".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<string::NString>(reader, ctx);
            let ans = string::NString::alloc(&"3 * (4 / 2) - 12 = -6   ".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_int() {
        let mut ctx = Context::new("int");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "1";

            let result = read::<number::Integer>(program, ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1";

            let result = read::<number::Integer>(program, ctx);
            let ans = number::Integer::alloc(-1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1";

            let result = read::<number::Integer>(program, ctx);
            let ans = number::Integer::alloc(1, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_float() {
        let mut ctx = Context::new("float");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = number::Real::alloc(1.0, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = number::Real::alloc(-1.0, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1.0";

            let result = read::<number::Real>(program, ctx);
            let ans = number::Real::alloc(1.0, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "3.14";

            let result = read::<number::Real>(program, ctx);
            let ans = number::Real::alloc(3.14, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "0.5";

            let result = read::<number::Real>(program, ctx);
            let ans = number::Real::alloc(0.5, ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_symbol() {
        let mut ctx = Context::new("symbol");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "symbol";

            let result = read::<symbol::Symbol>(program, ctx);
            let ans = symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "s1 s2   s3";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, ctx);
            let ans = symbol::Symbol::alloc(&"s1".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<symbol::Symbol>(reader, ctx);
            let ans = symbol::Symbol::alloc(&"s2".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());


            let result = read_with_ctx::<symbol::Symbol>(reader, ctx);
            let ans = symbol::Symbol::alloc(&"s3".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+ - +1-2 -2*3/4";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, ctx);
            let ans = symbol::Symbol::alloc(&"+".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, ctx);
            let ans = symbol::Symbol::alloc(&"-".to_string(), ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, ctx).into_value();
            let ans = symbol::Symbol::alloc(&"+1-2".to_string(), ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, ctx).into_value();
            let ans = symbol::Symbol::alloc(&"-2*3/4".to_string(), ans_ctx).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //special symbol
        {
            let program = "true false";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<Value>(reader, ctx);
            let ans = bool::Bool::true_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, ctx);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn read_keyword() {
        let mut ctx = Context::new("symbol");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = ":symbol";

            let result = read::<keyword::Keyword>(program, ctx);
            let ans = keyword::Keyword::alloc(&"symbol".to_string(), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_array() {
        let mut ctx = Context::new("array");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "[]";

            let result = read::<array::Array>(program, ctx);
            let ans = array::Array::from_list(&list::List::nil(), Some(0), ans_ctx);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "[1 2 3]";

            let result = read::<Value>(program, ctx);

            let_cap!(_1, number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let_cap!(_2, number::Integer::alloc(2, ans_ctx).into_value(), ans_ctx);
            let_cap!(_3, number::Integer::alloc(3, ans_ctx).into_value(), ans_ctx);
            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&_3, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_2, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, array::Array::from_list(&ans, None, ans_ctx), ans_ctx);

            assert_eq!(result.as_ref(), ans.as_reachable().cast_value().as_ref());
        }

        {
            let program = "[1 3.14 \"hohoho\" symbol]";

            let result = read::<Value>(program, ctx);

            let_cap!(_1, number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let_cap!(_3_14, number::Real::alloc(3.14, ans_ctx).into_value(), ans_ctx);
            let_cap!(hohoho, string::NString::alloc(&"hohoho".to_string(), ans_ctx).into_value(), ans_ctx);
            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx).into_value(), ans_ctx);

            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&symbol, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&hohoho, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_3_14, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, array::Array::from_list(&ans, None, ans_ctx), ans_ctx);

            assert_eq!(result.as_ref(), ans.as_reachable().cast_value().as_ref());
        }
    }

    #[test]
    fn read_list() {
        let mut ctx = Context::new("list");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "()";

            let result = read::<Value>(program, ctx);
            let ans = list::List::nil().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(1 2 3)";

            let result = read::<Value>(program, ctx);

            let_cap!(_1, number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let_cap!(_2, number::Integer::alloc(2, ans_ctx).into_value(), ans_ctx);
            let_cap!(_3, number::Integer::alloc(3, ans_ctx).into_value(), ans_ctx);
            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&_3, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_2, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);

            assert_eq!(result.as_ref(), ans.as_reachable().cast_value().as_ref());
        }

        {
            let program = "(1 3.14 \"hohoho\" symbol)";

            let result = read::<Value>(program, ctx);

            let_cap!(_1, number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let_cap!(_3_14, number::Real::alloc(3.14, ans_ctx).into_value(), ans_ctx);
            let_cap!(hohoho, string::NString::alloc(&"hohoho".to_string(), ans_ctx).into_value(), ans_ctx);
            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx).into_value(), ans_ctx);

            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&symbol, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&hohoho, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_3_14, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);

            assert_eq!(result.as_ref(), ans.as_reachable().cast_value().as_ref());
        }
    }

    #[test]
    fn read_quote() {
        let mut ctx = Context::new("list");
        let ctx = &mut ctx;
        let mut ans_ctx = Context::new(" ans");
        let ans_ctx = &mut ans_ctx;

        {
            let program = "'symbol";

            let result = read::<Value>(program, ctx);

            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), ans_ctx).into_value(), ans_ctx);
            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&symbol, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&syntax::Syntax::quote().into_value(), &ans, ans_ctx), ans_ctx);

            assert_eq!(result.as_ref(), ans.as_reachable().cast_value().as_ref());
        }

    }

}