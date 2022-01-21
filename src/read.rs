use std::iter::Peekable;

use crate::compile;
use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use crate::err::{self, NResult};
use crate::value::any::Any;
use crate::value::list::ListBuilder;

pub struct StdinChars {
    buf: String,
    index: usize,
}

impl StdinChars {
    pub fn new() -> Self {
        StdinChars {
            buf: String::new(),
            index: 0,
        }
    }
}

impl Iterator for StdinChars {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.is_empty() {
            self.index = 0;

            match std::io::stdin().read_line(&mut self.buf) {
                Ok(_) => {
                    //何もしない
                }
                Err(_err) => {
                    return None;
                }
            }
        }

        let mut iter = unsafe { self.buf.get_unchecked(self.index ..) }.char_indices();
        let (_, ch) = iter.next().unwrap();

        //次回の読み込み開始位置を取得
        if let Some((bytes, _)) = iter.next() {
            self.index += bytes;
        } else {
            //最後まで使用したので、次回に新しい行の読み込みを行う
            self.buf.clear();
        }

        Some(ch)
    }
}

pub struct Reader<I: Iterator<Item=char>> {
    input: Peekable<I>,
}

impl <I: Iterator<Item=char>> Reader<I> {
    pub fn new(input: Peekable<I>) -> Self {
        Reader {
            input: input,
        }
    }
}

#[derive(Debug)]
pub enum ReadException {
    EOF,
    OutOfMemory,
    MalformedFormat(err::MalformedFormat),
}

impl From<err::OutOfMemory> for ReadException {
    fn from(_: err::OutOfMemory) -> Self {
        ReadException::OutOfMemory
    }
}

impl From<err::MalformedFormat> for ReadException {
    fn from(this: err::MalformedFormat) -> Self {
        ReadException::MalformedFormat(this)
    }
}

pub fn read<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    read_internal(reader, obj)
}

fn read_internal<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    skip_whitespace(reader);

    match reader.input.peek() {
        None => Err(ReadException::EOF),
        Some(ch) => match ch {
            '(' => read_list(reader, obj),
            '[' => read_array(reader, obj),
            '{' => read_tuple(reader, obj),

            ')' => return Err(err::MalformedFormat::new(None, "read error )").into()),
            ']' => return Err(err::MalformedFormat::new(None, "read error ]").into()),
            '}' => return Err(err::MalformedFormat::new(None, format!("{}", "read error }")).into()),

            '"' => read_string(reader, obj),
            '\'' => read_quote(reader, obj),
            '@' => read_bind(reader, obj),
            '+' | '-' | '0' ..= '9' => read_number_or_symbol(reader, obj),
            ':' => read_keyword(reader, obj),
            _ => read_symbol(reader, obj),
        }
    }
}


fn read_list<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    let list = read_sequence(')', reader, obj)?;
    Ok(list.into_value())
}

fn read_array<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    let list = read_sequence(']', reader, obj)?;
    let ary = array::Array::from_list(&list.reach(obj), None, obj)?;
    Ok(ary.into_value())
}

fn read_tuple<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    let list = read_sequence('}', reader, obj)?;
    let tuple = tuple::Tuple::from_list(&list.reach(obj), None, obj)?;
    Ok(tuple.into_value())
}

fn read_sequence<I: Iterator<Item=char>>(end_char:char, reader: &mut Reader<I>, obj: &mut Object) -> NResult<list::List, ReadException> {
    //skip first char
    reader.input.next();

    let mut builder = ListBuilder::new(obj);
    loop {
        skip_whitespace(reader);
        match reader.input.peek() {
            None => return Err(err::MalformedFormat::new(None, "シーケンスが完結する前にEOFになった").into()),
            Some(ch) if *ch == end_char => {
                reader.input.next();
                // complete!
                return Ok(builder.get());
            }
            Some(_) => {
                //再帰的にreadを呼び出す
                let v = read_internal(reader, obj)?;
                builder.append(&v.reach(obj), obj)?;
            }
        }
    }
}

fn read_string<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    //skip first char
    reader.input.next();

    //終了文字'"'までのすべての文字を読み込み文字列をぶじぇくとを作成する
    let mut acc: Vec<char> = Vec::new();
    loop {
        match reader.input.next() {
            None => return Err(err::MalformedFormat::new(None, "文字列が完結する前にEOFになった").into()),
            Some('\"') => {
                let str: String = acc.into_iter().collect();
                let str = string::NString::alloc(&str, obj)?;

                return Ok(str.into_value())
            }
            Some(ch) => {
                acc.push(ch);
            }
        }
    }
}

#[allow(dead_code)]
fn read_char<I: Iterator<Item=char>>(_reader: &mut Reader<I>, _ctx: &mut Object) -> NResult<Any, ReadException> {
    //TODO
    unimplemented!()
}

fn read_quote<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    read_with_modifier(compile::literal::quote().cast_value(), reader, obj)
}

fn read_bind<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    read_with_modifier(compile::literal::bind().cast_value(), reader, obj)
}

fn read_with_modifier<I: Iterator<Item=char>>(modifier: &Reachable<Any>, reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    //skip first char
    reader.input.next();

    //再帰的に式を一つ読み込んでquoteで囲む
    let sexp = read_internal(reader, obj)?;
    let sexp = sexp.reach(obj);

    let mut builder = ListBuilder::new(obj);
    builder.append(modifier, obj)?;
    builder.append(&sexp, obj)?;
    Ok(builder.get().into_value())
}

fn read_number_or_symbol<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    let str = read_word(reader, obj)?;
    match str.parse::<i64>() {
        Ok(num) => {
            //integer
            let num = number::make_integer(num, obj)?;
            Ok(num.into_value())
        },
        Err(_) => match str.parse::<f64>() {
            Ok(num) => {
                //floating number
                let num = number::Real::alloc(num, obj)?;
                Ok(num.into_value())
            }
            Err(_) => {
                //symbol
                let symbol = symbol::Symbol::alloc(&str, obj)?;
                Ok(symbol.into_value())
            }
        }
    }
}

fn read_keyword<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    //skip first char
    reader.input.next();

    let str = read_word(reader, obj)?;
    let keyword = keyword::Keyword::alloc(&str, obj)?;
    Ok(keyword.into_value())
}

fn read_symbol<I: Iterator<Item=char>>(reader: &mut Reader<I>, obj: &mut Object) -> NResult<Any, ReadException> {
    let str = read_word(reader, obj)?;
    match str.as_str() {
        "true" => Ok(bool::Bool::true_().into_ref().into_value()),
        "false" => Ok(bool::Bool::false_().into_ref().into_value()),
        _ => {
            let symbol = symbol::Symbol::alloc(&str, obj)?;
            Ok(symbol.into_value())
        }
    }
}

fn read_word<I: Iterator<Item=char>>(reader: &mut Reader<I>, _ctx: &mut Object) -> Result<String, ReadException> {
    let mut acc: Vec<char> = Vec::new();
    loop {
        match reader.input.peek() {
            None => {
                if acc.is_empty() {
                    return Err(err::MalformedFormat::new(None, "ワードが存在しない").into());
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

fn skip_whitespace<I: Iterator<Item=char>>(reader: &mut Reader<I>) {
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
    use std::str::Chars;

    use crate::{read::*, value::array::ArrayBuilder};

    fn make_reader(s: &str) -> Reader<Chars> {
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

    fn read<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
        //let mut heap = navi::mm::Heap::new(1024, name.to_string());
        let mut reader = make_reader(program);

        read_with_ctx(&mut reader, obj)
    }

    fn read_with_ctx<T: NaviType>(reader: &mut Reader<Chars>, obj: &mut Object) -> Ref<T> {
        let result = crate::read::read(reader, obj);
        assert!(result.is_ok());

        let result = result.unwrap();
        let result = result.as_ref().try_cast::<T>();
        assert!(result.is_some());
        Ref::new(result.unwrap())
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
            let ans = string::NString::alloc(&"aiueo".to_string(), ans_obj).unwrap();
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
            let ans = string::NString::alloc(&"1 + (1 - 3) = -1".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<string::NString>(reader, obj);
            let ans = string::NString::alloc(&"3 * (4 / 2) - 12 = -6   ".to_string(), ans_obj).unwrap();
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

            let result = read::<Any>(program, obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1";

            let result = read::<Any>(program, obj);
            let ans = number::make_integer(-1, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1";

            let result = read::<Any>(program, obj);
            let ans = number::make_integer(1, ans_obj).unwrap();
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
            let ans = number::Real::alloc(1.0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "-1.0";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(-1.0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+1.0";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(1.0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "3.14";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(3.14, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "0.5";

            let result = read::<number::Real>(program, obj);
            let ans = number::Real::alloc(0.5, ans_obj).unwrap();
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
            let ans = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "s1 s2   s3";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s1".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s2".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());


            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"s3".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "+ - +1-2 -2*3/4";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<symbol::Symbol>(reader, obj);
            let ans = symbol::Symbol::alloc(&"+".to_string(), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Any>(reader, obj);
            let ans = symbol::Symbol::alloc(&"-".to_string(), ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Any>(reader, obj).into_value();
            let ans = symbol::Symbol::alloc(&"+1-2".to_string(), ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Any>(reader, obj).into_value();
            let ans = symbol::Symbol::alloc(&"-2*3/4".to_string(), ans_obj).unwrap().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        //special symbol
        {
            let program = "true false";

            let mut reader = make_reader(program);
            let reader = &mut reader;

            let result = read_with_ctx::<Any>(reader, obj);
            let ans = bool::Bool::true_().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());

            let result = read_with_ctx::<Any>(reader, obj);
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
            let ans = keyword::Keyword::alloc(&"symbol".to_string(), ans_obj).unwrap();
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

            let result = read::<array::Array<Any>>(program, obj);
            let ans = array::Array::from_list(&list::List::nil(), Some(0), ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "[1 2 3]";

            let result = read::<Any>(program, obj);

            let _1 = number::make_integer(1, ans_obj).unwrap().reach(ans_obj);
            let _2 = number::make_integer(2, ans_obj).unwrap().reach(ans_obj);
            let _3 = number::make_integer(3, ans_obj).unwrap().reach(ans_obj);

            let ans = list::List::nil();
            let ans = list::List::alloc(&_3, &ans, ans_obj).unwrap().reach(ans_obj);
            let ans = list::List::alloc(&_2, &ans, ans_obj).unwrap().reach(ans_obj);
            let ans = list::List::alloc(&_1, &ans, ans_obj).unwrap().reach(ans_obj);
            let ans = array::Array::from_list(&ans, None, ans_obj).unwrap().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "[1 3.14 \"hohoho\" symbol]";

            let result = read::<Any>(program, obj);


            let _1 = number::make_integer(1, ans_obj).unwrap().reach(ans_obj);
            let _3_14 = number::Real::alloc(3.14, ans_obj).unwrap().into_value().reach(ans_obj);
            let hohoho = string::NString::alloc(&"hohoho".to_string(), ans_obj).unwrap().into_value().reach(ans_obj);
            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).unwrap().into_value().reach(ans_obj);

            let mut builder = ArrayBuilder::<Any>::new(4, ans_obj).unwrap();

            builder.push(&_1, ans_obj).unwrap();
            builder.push(&_3_14, ans_obj).unwrap();
            builder.push(&hohoho, ans_obj).unwrap();
            builder.push(&symbol, ans_obj).unwrap();
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

            let result = read::<Any>(program, obj);
            let ans = list::List::nil().into_value();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(1 2 3)";

            let result = read::<Any>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::make_integer(1, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::make_integer(2, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::make_integer(3, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            let ans = builder.get().capture(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "(1 3.14 \"hohoho\" symbol)";

            let result = read::<Any>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::make_integer(1, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::Real::alloc(3.14, ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
            builder.append( &string::NString::alloc(&"hohoho".to_string(), ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
            builder.append( &symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
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

            let result = read::<Any>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::make_integer(1, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::make_integer(2, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::make_integer(3, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            let ans = builder.get().reach(ans_obj);
            let ans = tuple::Tuple::from_list(&ans, None, ans_obj).unwrap().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

        {
            let program = "{1 3.14 \"hohoho\" symbol}";

            let result = read::<Any>(program, obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append( &number::make_integer(1, ans_obj).unwrap().reach(ans_obj), ans_obj).unwrap();
            builder.append( &number::Real::alloc(3.14, ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
            builder.append( &string::NString::alloc(&"hohoho".to_string(), ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
            builder.append( &symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).unwrap().into_value().reach(ans_obj), ans_obj).unwrap();
            let ans = builder.get().reach(ans_obj);
            let ans = tuple::Tuple::from_list(&ans, None, ans_obj).unwrap().reach(ans_obj);

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

            let result = read::<Any>(program, obj);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), ans_obj).unwrap().into_value().reach(ans_obj);

            let mut builder = ListBuilder::new(ans_obj);
            builder.append(compile::literal::quote().cast_value(), ans_obj).unwrap();
            builder.append(&symbol, ans_obj).unwrap();
            let ans = builder.get().reach(ans_obj);

            assert_eq!(result.as_ref(), ans.cast_value().as_ref());
        }

    }

}