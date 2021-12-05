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


#[cfg(test)]
mod tets {
    use crate::mm::{Heap};
    use crate::value::*;
    use crate::world::*;

    fn world_get<'a>(world: &'a mut World, symbol: &NBox<symbol::Symbol>) -> &'a NBox<Value> {
        let result = world.get(symbol);
        assert!(result.is_some());
        result.unwrap()
    }

    #[test]
    fn set_get() {
        let mut heap = Heap::new(1024, "string");
        let mut ans_heap = Heap::new(1024, " ans");

        {
            let mut world = World::new();

            let symbol = symbol::Symbol::alloc(&mut heap, &"symbol".to_string());
            let v = number::Integer::alloc(&mut heap, 1).into_nboxvalue();
            world.set(symbol, v);

            let symbol = symbol::Symbol::alloc(&mut heap, &"symbol".to_string());
            let result = world_get(&mut world, &symbol);
            let ans = number::Integer::alloc(&mut ans_heap, 1).into_nboxvalue();
            assert_eq!(result, &ans);


            let symbol = symbol::Symbol::alloc(&mut heap, &"symbol".to_string());
            let v = number::Real::alloc(&mut heap, 3.14).into_nboxvalue();
            world.set(symbol, v);

            let symbol = symbol::Symbol::alloc(&mut heap, &"symbol".to_string());
            let result = world_get(&mut world, &symbol);
            let ans = number::Real::alloc(&mut ans_heap, 3.14).into_nboxvalue();
            assert_eq!(result, &ans);

            let symbol2 = symbol::Symbol::alloc(&mut heap, &"hoge".to_string());
            let v2 = string::NString::alloc(&mut heap, &"bar".to_string()).into_nboxvalue();
            world.set(symbol2, v2);

            let symbol2 = symbol::Symbol::alloc(&mut heap, &"hoge".to_string());
            let result = world_get(&mut world, &symbol2);
            let ans2 = string::NString::alloc(&mut ans_heap, &"bar".to_string()).into_nboxvalue();
            assert_eq!(result, &ans2);

            let result = world_get(&mut world, &symbol);
            assert_eq!(result, &ans);
        }

        heap.free();
        ans_heap.free();
    }

}