use crate::value::*;
use crate::mm::{Heap};
use crate::world::World;

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

pub struct Object {
    heap: RefCell<Heap>,
    world: World,
    frames: Vec<Vec<(NPtr<symbol::Symbol>, NPtr<Value>)>>,
    nbox_root: Cell<Option<NonNull<NBox<Value>>>>,
}

impl Object {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Object {
            heap: RefCell::new(Heap::new(1024 * 8, name)),
            world: World::new(),
            frames: Vec::new(),
            nbox_root: Cell::new(None),
        }
    }

    pub fn push_local_frame<T, U>(&mut self, frame: &[(&T, &U)])
    where
        T: crate::value::AsPtr<symbol::Symbol>,
        U: crate::value::AsPtr<Value>
    {
        let mut vec = Vec::<(NPtr<symbol::Symbol>, NPtr<Value>)>::new();
        for (symbol, v) in frame {
            vec.push((NPtr::new(symbol.as_mut_ptr()), NPtr::new(v.as_mut_ptr())));
        }

        self.frames.push(vec);
    }

    pub fn pop_local_frame(&mut self) {
        self.frames.pop();
    }

    pub fn find_value(&self, symbol: &NBox<symbol::Symbol>) -> Option<NPtr<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            let result = frame.iter().find(|(sym, v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(NPtr::new(v.as_mut_ptr()));
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(NPtr::new(v.as_mut_ptr()))
        } else {
            None
        }
    }

    pub fn alloc<T: NaviType>(&self) -> NPtr<T> {
        self.heap.borrow_mut().alloc::<T>(self)
    }

    pub fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> NPtr<T> {
        self.heap.borrow_mut().alloc_with_additional_size::<T>(additional_size, self)
    }

    pub fn define_value<K, T>(&mut self, key: K, v: &T)
    where
        K: AsRef<str>,
        T: crate::value::AsPtr<Value>,
    {
        (&mut self.world).set(key, NPtr::new(v.as_mut_ptr()))
    }

    pub fn add_capture(&self, nbox: &mut NBox<Value>) {
        unsafe {
            let nbox_ptr= NonNull::new_unchecked(nbox as *mut NBox<Value>);

            match &mut self.nbox_root.get() {
                Some(root) => {
                    nbox.next = Some(*root);
                    root.as_mut().prev = Some(nbox_ptr);
                }
                None => { }
            }

            nbox.prev = None;

            self.nbox_root.set(Some(nbox_ptr));
        }
    }

    pub fn drop_capture(&self, nbox: &mut NBox<Value>) {
        match nbox.prev {
            Some(prev) => {
                unsafe {
                    (*prev.as_ptr()).next = nbox.next;
                }
            }
            None => {
                self.nbox_root.set(nbox.next);
            }
        };

        match nbox.next {
            Some(next) => {
                unsafe { (*next.as_ptr()).prev = nbox.prev }
            }
            None => { }
        };
    }

    pub(crate) fn for_each_all_alived_value(&self, callback: fn(&NPtr<Value>)) {
        //ローカルフレーム内で保持している値
        for frame in self.frames.iter() {
            for (sym, v) in frame.iter() {
                callback(sym.cast_value());
                callback(v);
            }
        }

        //グローバルスペース内で保持している値
        for v in self.world.get_all_values().iter() {
            callback(v);
        }
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        self.heap.borrow_mut().free();
    }
}