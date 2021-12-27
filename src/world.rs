use crate::value::*;
use crate::ptr::*;

mod map;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: crate::world::map::PatriciaTree<FPtr<Value>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: crate::world::map::PatriciaTree::new(),
        }
    }

    pub fn set<K>(&mut self, key: K, v: &Reachable<Value>)
    where
        K: AsRef<str>,
    {
        self.area.add(key, FPtr::new(v.as_ref()))
    }

    pub fn get<K>(&self, key: K) -> Option<&FPtr<Value>>
    where
        K: AsRef<str>
    {
        self.area.get(key)
    }

    pub(crate) fn get_all_values(&self) -> Vec<&FPtr<Value>> {
        let vec = self.area.to_vec_preorder();
        vec.iter().filter_map(|node| {
            node.value_as_ref()
        }).collect()
    }

}


#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::{value::*};
    use crate::context::{Context};
    use crate::ptr::*;

    fn world_get(symbol: &Reachable<symbol::Symbol>, ctx: &Context) -> FPtr<Value> {
        let result = ctx.find_value(symbol);
        assert!(result.is_some());
        result.unwrap()
    }

    #[test]
    fn set_get() {
        let mut obj = Object::new();
        let obj = &mut obj;

        {
            let v = number::Integer::alloc(1, obj).into_value().reach(obj);
            obj.context().define_value("symbol", &v);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), obj).reach(obj);
            let result = world_get(&symbol, obj.context()).reach(obj);
            let ans = number::Integer::alloc(1, obj).into_value().reach(obj);
            assert_eq!(result.as_ref(), ans.as_ref());


            let v = number::Real::alloc(3.14, obj).into_value().reach(obj);
            obj.context().define_value("symbol", &v);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), obj).reach(obj);
            let result = world_get(&symbol, obj.context()).reach(obj);
            let ans = number::Real::alloc(3.14, obj).into_value().reach(obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let v2 = string::NString::alloc(&"bar".to_string(), obj).into_value().reach(obj);
            obj.context().define_value("hoge", &v2);

            let symbol2 = symbol::Symbol::alloc(&"hoge".to_string(), obj).reach(obj);
            let result = world_get(&symbol2, obj.context()).reach(obj);
            let ans2 = string::NString::alloc(&"bar".to_string(), obj).into_value().reach(obj);
            assert_eq!(result.as_ref(), ans2.as_ref());

            let result = world_get(&symbol, obj.context()).reach(obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}