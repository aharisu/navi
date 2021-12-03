use std::collections::HashMap;
use crate::value::*;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: HashMap<NBox<symbol::Symbol>, NBox<Value>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: HashMap::new(),
        }
    }

    pub fn set(&mut self, symbol: NBox<symbol::Symbol>, v: NBox<Value>) {
        self.area.insert(symbol, v);
    }

    pub fn get(&mut self, symbol: &NBox<symbol::Symbol>) -> Option<&NBox<Value>> {
        self.area.get(symbol)
    }

}