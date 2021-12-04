use crate::value::*;

mod map;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: crate::world::map::PatriciaTree<NBox<Value>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: crate::world::map::PatriciaTree::new(),
        }
    }

    pub fn set(&mut self, symbol: NBox<symbol::Symbol>, v: NBox<Value>) {
        self.area.add(symbol.as_ref(), v)
    }

    pub fn get(&mut self, symbol: &NBox<symbol::Symbol>) -> Option<&NBox<Value>> {
        self.area.get(symbol.as_ref())
    }

}