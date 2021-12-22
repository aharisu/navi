#![allow(unused_unsafe)]


use crate::value::{self, *};
use crate::ptr::*;
use crate::world::World;

use std::cell::{Cell};
use std::ptr::NonNull;

pub struct Context {
    world: World,
    frames: Vec<Vec<(RPtr<symbol::Symbol>, RPtr<Value>)>>,
    nbox_root: Cell<Option<NonNull<Capture<Value>>>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            world: World::new(),
            frames: Vec::new(),
            nbox_root: Cell::new(None),
        }
    }

    pub fn is_toplevel(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn add_to_current_frame<Key, V>(&mut self, symbol: &Key, value: &V)
    where
        Key: AsReachable<symbol::Symbol>,
        V: AsReachable<Value>,
    {
        if let Some(frame) = self.frames.last_mut() {
            frame.push((symbol.as_reachable().clone(), value.as_reachable().clone()));
        } else {
            //ローカルフレームがなければグローバル変数として追加
            self.world.set(symbol.as_reachable().as_ref(), value);
        }
    }

    pub fn push_local_frame<Key, V>(&mut self, frame: &[(&Key, &V)])
    where
        Key: AsReachable<symbol::Symbol>,
        V: AsReachable<Value>,
    {
        let mut vec = Vec::<(RPtr<symbol::Symbol>, RPtr<Value>)>::new();
        for (symbol, v) in frame {
            vec.push((symbol.as_reachable().clone(), v.as_reachable().clone()));
        }

        self.frames.push(vec);
    }

    pub fn pop_local_frame(&mut self) {
        self.frames.pop();
    }

    pub fn find_value<'a, Key>(&'a self, symbol: &Key) -> Option<&'a RPtr<Value>>
    where
        Key: AsReachable<symbol::Symbol>,
    {
        let symbol = symbol.as_reachable();
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            //後で定義されたものを優先して使用するために逆順で探す
            let result = frame.iter().rev().find(|(sym, _v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(v);
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(v)
        } else {
            None
        }
    }

    pub fn define_value<Key, V>(&mut self, key: Key, v: &V)
    where
        Key: AsRef<str>,
        V: AsReachable<Value>,
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

    pub fn add_capture(&self, capture: &mut Capture<Value>) {
        //ポインタ以外の値はキャプチャの必要がないので何もしない
        if value::value_is_pointer(capture.v.as_ref()) == false {
            return
        }

        //println!("capture, {:?}", capture.as_ref());

        unsafe {
            let nbox_ptr= NonNull::new_unchecked(capture as *mut Capture<Value>);

            match &mut self.nbox_root.get() {
                Some(root) => {
                    capture.next = Some(*root);
                    root.as_mut().prev = Some(nbox_ptr);
                }
                None => { }
            }

            capture.prev = None;

            self.nbox_root.set(Some(nbox_ptr));
        }
    }

    pub fn drop_capture(&self, capture: &mut Capture<Value>) {
        //ポインタ以外の値はキャプチャの必要がないので何もしない
        if value::value_is_pointer(capture.v.as_ref()) == false {
            return
        }

        //println!("drop, {:?}", capture.as_ref());

        match capture.prev {
            Some(prev) => {
                unsafe {
                    (*prev.as_ptr()).next = capture.next;
                }
            }
            None => {
                self.nbox_root.set(capture.next);
            }
        };

        match capture.next {
            Some(next) => {
                unsafe { (*next.as_ptr()).prev = capture.prev }
            }
            None => { }
        };
    }

    pub(crate) fn for_each_all_alived_value(&self, arg: &usize, callback: fn(&RPtr<Value>, &usize)) {
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

        //ローカル変数として捕捉している値
        let mut node = self.nbox_root.get();
        loop {
            match node {
                Some(capture_ptr) => {
                    let capture = unsafe { capture_ptr.as_ref() };
                    callback(capture.as_reachable(), arg);
                    node = capture.next;
                }
                None => break,
            }
        }
    }

}