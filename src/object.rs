use crate::value::*;
use crate::mm::{Heap};
use crate::world::World;

pub struct Object {
    heap: Heap,
    world: World,
    frames: Vec<Vec<(NPtr<symbol::Symbol>, NPtr<Value>)>>,
}

impl Object {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Object {
            heap: Heap::new(1024 * 8, name),
            world: World::new(),
            frames: Vec::new(),
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

    pub fn find_value(&self, symbol: &NBox<symbol::Symbol>) -> Option<NBox<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            let result = frame.iter().find(|(sym, v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(NBox::new(v.as_mut_ptr()));
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(v.duplicate())
        } else {
            None
        }
    }

    pub fn alloc<T: NaviType>(&mut self) -> NBox<T> {
        self.heap.alloc::<T>()
    }

    pub fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> NBox<T> {
        self.heap.alloc_with_additional_size::<T>(additional_size)
    }

    pub fn define_value<K: AsRef<str>>(&mut self, key: K, v: NBox<Value>) {
        self.world.set(key, v)
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        self.heap.free();
    }
}