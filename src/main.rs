use std::io::{stdout, Write};

use navi::err::Exception;
use navi::object;
use navi::ptr::ValueHolder;
use navi::read::{StdinChars, Reader};

fn main() {
    let mut standalone = object::new_object();
    let mut reader = Reader::new(StdinChars::new().peekable());

    loop {
        print!("navi> ");
        stdout().flush().unwrap();
        match navi::read::read(&mut reader, standalone.mut_object()) {
            Ok(v) => {
                let v = v.reach(standalone.mut_object());
                match navi::eval::eval(&v, standalone.mut_object()) {
                    Ok(v) => {
                        println!("{}", v.as_ref());
                    }
                    Err(navi::eval::EvalError::Exit) => {
                        break;
                    }
                    Err(navi::eval::EvalError::ObjectSwitch(new_standaloneobject)) => {
                        standalone = new_standaloneobject;
                    }
                    Err(navi::eval::EvalError::Exception(err)) => {
                        println!("{}", err);
                    }
                }
            }
            Err(navi::read::ReadException::EOF) => {
                break;
            }
            Err(navi::read::ReadException::OutOfMemory) => {
                panic!("OOM");
            }
            Err(navi::read::ReadException::MalformedFormat(err)) => {
                println!("{}", Exception::MalformedFormat(err));
            }
        }
    }
}
