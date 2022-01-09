use std::iter::Peekable;
use std::str::Chars;

use crate::cap_append;
use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use crate::value::list::ListBuilder;

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

pub fn read(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    read_internal(reader, obj)
}

fn read_internal(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    skip_whitespace(reader);

    match reader.input.peek() {
        None => Err(readerror("読み込む内容がない".to_string())),
        Some(ch) => match ch {
            '(' => read_list(reader, obj),
            '[' => read_array(reader, obj),
            '{' => read_tuple(reader, obj),

            ')' => panic!("read error )"),
            ']' => panic!("read error ]"),
            '}' => panic!("{}", "read error }"),

            '"' => read_string(reader, obj),
            '\'' => read_quote(reader, obj),
            '@' => read_bind(reader, obj),
            '+' | '-' | '0' ..= '9' => read_number_or_symbol(reader, obj),
            ':' => read_keyword(reader, obj),
            _ => read_symbol(reader, obj),
        }
    }
}


fn read_list(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    let list = read_sequence(')', reader, obj)?;
    Ok(list.into_value())
}

fn read_array(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    let list = read_sequence(']', reader, obj)?;
    Ok(array::Array::from_list(&list.reach(obj), None, obj).into_value())
}

fn read_tuple(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    let list = read_sequence('}', reader, obj)?;
    Ok(tuple::Tuple::from_list(&list.reach(obj), None, obj).into_value())
}

fn read_sequence(end_char:char, reader: &mut Reader, obj: &mut Object) -> Result<FPtr<list::List>, ReadError> {
    //skip first char
    reader.input.next();

    let mut builder = ListBuilder::new(obj);
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
                match read_internal(reader, obj) {
                    //内部でエラーが発生した場合は途中停止
                    Err(msg) => return Err(msg),
                    Ok(v) => {
                        cap_append!(builder, v, obj);
                    }
                }
            }
        }
    }
}

fn read_string(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    //skip first char
    reader.input.next();

    //終了文字'"'までのすべての文字を読み込み文字列をぶじぇくとを作成する
    let mut acc: Vec<char> = Vec::new();
    loop {
        match reader.input.next() {
            None => return Err(readerror("文字列が完結する前にEOFになった".to_string())),
            Some('\"') => {
                let str: String = acc.into_iter().collect();
                let str = string::NString::alloc(&str, obj);
                return Ok(str.into_value());
            }
            Some(ch) => {
                acc.push(ch);
            }
        }
    }
}

#[allow(dead_code)]
fn read_char(_reader: &mut Reader, _ctx: &mut Object) -> ReadResult {
    //TODO
    unimplemented!()
}

fn read_quote(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    read_with_modifier(syntax::literal::quote().cast_value(), reader, obj)
}

fn read_bind(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    read_with_modifier(syntax::literal::bind().cast_value(), reader, obj)
}

fn read_with_modifier(modifier: &Reachable<Value>, reader: &mut Reader, obj: &mut Object) -> ReadResult {
    //skip first char
    reader.input.next();

    //再帰的に式を一つ読み込んでquoteで囲む
    let sexp = match read_internal(reader, obj) {
        //内部でエラーが発生した場合は途中停止
        Err(msg) => return Err(msg),
        Ok(v) => v,
    };
    let sexp = sexp.reach(obj);

    let mut builder = ListBuilder::new(obj);
    builder.append(modifier, obj);
    builder.append(&sexp, obj);
    Ok(builder.get().into_value())
}

fn read_number_or_symbol(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    match read_word(reader, obj) {
        Ok(str) => match str.parse::<i64>() {
                Ok(num) => {
                    //integer
                    let num = number::Integer::alloc(num, obj);
                    return Ok(num.into_value());
                },
                Err(_) => match str.parse::<f64>() {
                    Ok(num) => {
                        //floating number
                        let num = number::Real::alloc(num, obj);
                        return Ok(num.into_value());
                    }
                    Err(_) => {
                        //symbol
                        let symbol = symbol::Symbol::alloc(&str, obj);
                        return Ok(symbol.into_value());
                    }
                }
            }
        Err(err) => Err(err),
    }
}
fn read_keyword(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    //skip first char
    reader.input.next();

    match read_word(reader, obj) {
        Ok(str) => Ok(keyword::Keyword::alloc(&str, obj).into_value()),
        Err(err) => Err(err),
    }
}

fn read_symbol(reader: &mut Reader, obj: &mut Object) -> ReadResult {
    match read_word(reader, obj) {
        Ok(str) => match &*str {
            "true" =>Ok(bool::Bool::true_().into_fptr().into_value()),
            "false" =>Ok(bool::Bool::false_().into_fptr().into_value()),
            _ => Ok(symbol::Symbol::alloc(&str, obj).into_value()),
        }
        Err(err) => Err(err),
    }
}

fn read_word(reader: &mut Reader, _ctx: &mut Object) -> Result<String, ReadError> {
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
            ']' |
            '{' |
            '}'
              => true,
            _ => false,
        }

}

#[cfg(test)]
mod tests {
    use crate::{read::*, value::array::ArrayBuilder};

    fn make_reader<'a>(s: &'a str) -> Reader<'a> {
        Reader::new( s.chars().peekable())
    }

    #[test]
    fn read_empty() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        let program = r#"
                
        "#;

        let mut reader = make_reader(program);
        let reader = &mut reader;
        let result  = crate::read::read(reader, obj);
        assert!(result.is_err());
    }

    fn read<T: NaviType>(program: &str, obj: &mut Object) -> FPtr<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut reader = make_reader(program);

        read_with_ctx(&mut reader, obj)
    }

    fn read_with_ctx<T: NaviType>(reader: &mut Reader, obj: &mut Object) -> FPtr<T> {
        let result = crate::read::read(reader, obj);
        assert!(result.is_ok());

        let result = result.unwrap();
        let result = result.as_ref().try_cast::<T>();
        assert!(result.is_some());
        FPtr::new(result.unwrap())
    }

    #[test]
    fn read_string() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = r#"
            "aiueo"
            "#;

            let result = read::<string::NString>(program, obj);
            let ans = string::NString::alloc(&"aiueo".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = r#"
            "1 + (1 - 3) = -1"
            "3 * (4 / 2) - 12 = -6   "
            "#;

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<string::NString>(reader, obj);
            let ans = string::NString::alloc(&"1 + (1 - 3) = -1".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<string::NString>(reader, obj);
            let ans = string::NString::alloc(&"3 * (4 / 2) - 12 = -6   ".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_int() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "1";

            let result = read::<number::Integer>(program, obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1";

            let result = read::<number::Integer>(program, obj);
            let ans = number::Integer::alloc(-1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1";

            let result = read::<number::Integer>(program, obj);
            let ans = number::Integer::alloc(1, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_float() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "1.0";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(1.0, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1.0";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(-1.0, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1.0";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(1.0, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "3.14";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(3.14, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "0.5";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(0.5, ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_symbol() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "symbol";

            let result = read::<symbol::Symbol>(program, obj);
            let ans = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "s1 s2   s3";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s1".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s2".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());


            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s3".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+ - +1-2 -2*3/4";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"+".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, obj);
            let ans = symbol::Symbol::alloc(&"-".to_string(), ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, obj).into_value();
            let ans = symbol::Symbol::alloc(&"+1-2".to_string(), ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, obj).into_value();
            let ans = symbol::Symbol::alloc(&"-2*3/4".to_string(), ans_obj).into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //special symbol
        {
            let program = "true false";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<Value>(reader, obj);
            let ans = bool::Bool::true_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Value>(reader, obj);
            let ans = bool::Bool::false_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }


    #[test]
    fn read_keyword() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = ":symbol";

            let result = read::<keyword::Keyword>(program, obj);
            let ans = keyword::Keyword::alloc(&"symbol".to_string(), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

    #[test]
    fn read_array() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "[]";

            let result = read::<array::Array<Value>>(program, obj);
            let ans = array::Array::from_list(&list::List::nil(), Some(0), ans_obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "[1 2 3]";

            let result = read::<Value>(program, obj);

            let _1 = number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj);
            let _2 = number::Integer::alloc(2, ans_obj).into_value().reach(ans_obj);
            let _3 = number::Integer::alloc(3, ans_obj).into_value().reach(ans_obj);

            let ans = list::List::nil();
            let ans = list::List::alloc(&_3, &ans, ans_obj).reach(ans_obj);
            let ans = list::List::alloc(&_2, &ans, ans_obj).reach(ans_obj);
            let ans = list::List::alloc(&_1, &ans, ans_obj).reach(ans_obj);
            let ans = array::Array::from_list(&ans, None, ans_obj).reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "[1 3.14 \"hohoho\" symbol]";

            let result = read::<Value>(program, obj);


            let _1 = number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj);
            let _3_14 = number::Real::alloc(3.14, ans_obj).into_value().reach(ans_obj);
            let hohoho = string::NString::alloc(&"hohoho".to_string(), ans_obj).into_value().reach(ans_obj);
            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).into_value().reach(ans_obj);

            let mut builder = ArrayBuilder::<Value>::new(4, ans_obj);

            builder.push(_1.as_ref(), ans_obj);
            builder.push(_3_14.as_ref(), ans_obj);
            builder.push(hohoho.as_ref(), ans_obj);
            builder.push(symbol.as_ref(), ans_obj);
            let ans = builder.get().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }
    }

    #[test]
    fn read_list() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "()";

            let result = read::<Value>(program, obj);
            let ans = list::List::nil().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(1 2 3)";

            let result = read::<Value>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Integer::alloc(2, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Integer::alloc(3, ans_obj).into_value().reach(ans_obj), ans_obj);
            let ans = builder.get().capture(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "(1 3.14 \"hohoho\" symbol)";

            let result = read::<Value>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Real::alloc(3.14, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &string::NString::alloc(&"hohoho".to_string(), ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).into_value().reach(ans_obj), ans_obj);
            let ans = builder.get().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }
    }

    #[test]
    fn read_tuple() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "{}";

            let result = read::<tuple::Tuple>(program, obj);
            let ans = tuple::Tuple::unit();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "{1 2 3}";

            let result = read::<Value>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Integer::alloc(2, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Integer::alloc(3, ans_obj).into_value().reach(ans_obj), ans_obj);
            let ans = builder.get().reach(ans_obj);
            let ans = tuple::Tuple::from_list(&ans, None, ans_obj).reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "{1 3.14 \"hohoho\" symbol}";

            let result = read::<Value>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &number::Real::alloc(3.14, ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &string::NString::alloc(&"hohoho".to_string(), ans_obj).into_value().reach(ans_obj), ans_obj);
            builder.append( &symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).into_value().reach(ans_obj), ans_obj);
            let ans = builder.get().reach(ans_obj);
            let ans = tuple::Tuple::from_list(&ans, None, ans_obj).reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

    }

    #[test]
    fn read_quote() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "'symbol";

            let result = read::<Value>(program, obj);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).into_value().reach(ans_obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append(syntax::literal::quote().cast_value(), ans_obj);
            builder.append(&symbol, ans_obj);
            let ans = builder.get().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

    }

}