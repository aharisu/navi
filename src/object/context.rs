#![allow(unused_unsafe)]


use crate::value::*;
use crate::ptr::*;

pub struct Context {
    frames: Vec<Vec<(FPtr<symbol::Symbol>, FPtr<Value>)>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            frames: Vec::new(),
        }
    }

    pub fn is_toplevel(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn add_to_current_frame(&mut self, symbol: &Reachable<symbol::Symbol>, value: &Reachable<Value>) -> bool{
        if let Some(frame) = self.frames.last_mut() {
            frame.push((FPtr::new(symbol.as_ref()), FPtr::new(value.as_ref())));
            true
        } else {
            //ローカルフレームがなければfalseを返す
            false
        }
    }

    pub fn push_local_frame(&mut self, frame: &[(&symbol::Symbol, &Value)]) {
        let mut vec = Vec::<(FPtr<symbol::Symbol>, FPtr<Value>)>::new();
        for (symbol, v) in frame {
            vec.push((FPtr::new(symbol), FPtr::new(v)));
        }

        self.frames.push(vec);
    }

    pub fn pop_local_frame(&mut self) {
        self.frames.pop();
    }

    pub fn find_local_value(&self, symbol: &symbol::Symbol) -> Option<FPtr<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            //後で定義されたものを優先して使用するために逆順で探す
            let result = frame.iter().rev().find(|(sym, _v)| {
                symbol.eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(v.clone());
            }
        }

        None
    }

    pub(crate) fn for_each_all_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        //ローカルフレーム内で保持している値
        for frame in self.frames.iter() {
            for (sym, v) in frame.iter() {
                callback(sym.cast_value(), arg);
                callback(v, arg);
            }
        }
    }

}