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