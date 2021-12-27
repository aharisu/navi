#![allow(unused_unsafe)]


use crate::value::{self, *};
use crate::ptr::*;
use crate::object::world::World;

pub struct Context {
    world: World,
    frames: Vec<Vec<(FPtr<symbol::Symbol>, FPtr<Value>)>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            world: World::new(),
            frames: Vec::new(),
        }
    }

    pub fn is_toplevel(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn add_to_current_frame(&mut self, symbol: &Reachable<symbol::Symbol>, value: &Reachable<Value>) {
        if let Some(frame) = self.frames.last_mut() {
            frame.push((FPtr::new(symbol.as_ref()), FPtr::new(value.as_ref())));
        } else {
            //ローカルフレームがなければグローバル変数として追加
            self.world.set(symbol.as_ref(), value);
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

    pub fn find_value(&self, symbol: &Reachable<symbol::Symbol>) -> Option<FPtr<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            //後で定義されたものを優先して使用するために逆順で探す
            let result = frame.iter().rev().find(|(sym, _v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(v.clone());
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(v.clone())
        } else {
            None
        }
    }

    pub fn define_value<Key>(&mut self, key: Key, v: &Reachable<Value>)
    where
        Key: AsRef<str>,
    {
        (&mut self.world).set(key, v)
    }

    pub fn register_core_global(&mut self) {
        object_ref::register_global(self);
        number::register_global(self);
        syntax::register_global(self);
        value::register_global(self);
        tuple::register_global(self);
        array::register_global(self);
        list::register_global(self);
    }

    pub(crate) fn for_each_all_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        //ローカルフレーム内で保持している値
        for frame in self.frames.iter() {
            for (sym, v) in frame.iter() {
                callback(sym.cast_value(), arg);
                callback(v, arg);
            }
        }

        //グローバルスペース内で保持している値
        for v in self.world.get_all_values().iter() {
            callback(v, arg);
        }
    }

}