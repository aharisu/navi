use crate::value::*;
use crate::ptr::*;

mod map;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: crate::world::map::PatriciaTree<RPtr<Value>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: crate::world::map::PatriciaTree::new(),
        }
    }

    pub fn set<K, V>(&mut self, key: K, v: &V)
    where
        K: AsRef<str>,
        V: AsReachable<Value>,
    {
        self.area.add(key, v.as_reachable().clone())
    }

    pub fn get<K>(&self, key: K) -> Option<&RPtr<Value>>
    where
        K: AsRef<str>
    {
        self.area.get(key)
    }

    pub(crate) fn get_all_values(&self) -> Vec<&RPtr<Value>> {
        let vec = self.area.to_vec_preorder();
        vec.iter().filter_map(|node| {
            node.value_as_ref()
        }).collect()
    }

}


#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::{value::*, let_cap, new_cap};
    use crate::context::{Context};
    use crate::ptr::*;

    fn world_get<T>(symbol: &T, ctx: &Context) -> FPtr<Value>
    where
        T: AsReachable<symbol::Symbol>
    {
        let result = ctx.find_value(symbol);
        assert!(result.is_some());
        result.unwrap().clone().into_fptr()
    }

    #[test]
    fn set_get() {
        let mut obj = Object::new();
        let obj = &mut obj;

        {
            let_cap!(v, number::Integer::alloc(1, obj).into_value(), obj);
            obj.context().define_value("symbol", &v);

            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), obj), obj);
            let_cap!(result, world_get(&symbol, obj.context()), obj);
            let_cap!(ans, number::Integer::alloc(1, obj).into_value(), obj);
            assert_eq!(result.as_ref(), ans.as_ref());


            let_cap!(v, number::Real::alloc(3.14, obj).into_value(), obj);
            obj.context().define_value("symbol", &v);

            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), obj), obj);
            let_cap!(result, world_get(&symbol, obj.context()), obj);
            let_cap!(ans, number::Real::alloc(3.14, obj).into_value(), obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let_cap!(v2, string::NString::alloc(&"bar".to_string(), obj).into_value(), obj);
            obj.context().define_value("hoge", &v2);

            let_cap!(symbol2, symbol::Symbol::alloc(&"hoge".to_string(), obj), obj);
            let_cap!(result, world_get(&symbol2, obj.context()), obj);
            let_cap!(ans2, string::NString::alloc(&"bar".to_string(), obj).into_value(), obj);
            assert_eq!(result.as_ref(), ans2.as_ref());

            let_cap!(result, world_get(&symbol, obj.context()), obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}