use crate::value::*;

mod map;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: crate::world::map::PatriciaTree<NPtr<Value>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: crate::world::map::PatriciaTree::new(),
        }
    }

    pub fn set<K>(&mut self, key: K, v: NPtr<Value>)
    where
        K: AsRef<str>
    {
        self.area.add(key, v)
    }

    pub fn get<K>(&self, key: K) -> Option<&NPtr<Value>>
    where
        K: AsRef<str>
    {
        self.area.get(key)
    }

    pub(crate) fn get_all_values(&self) -> Vec<&NPtr<Value>> {
        let vec = self.area.to_vec_preorder();
        vec.iter().filter_map(|node| {
            node.value_as_ref()
        }).collect()
    }

}


#[cfg(test)]
mod tests {
    use crate::{value::*, let_cap, new_cap};
    use crate::object::{Object, Capture};

    fn world_get(symbol: &Capture<symbol::Symbol>, ctx: &mut Object) -> NPtr<Value> {
        let result = ctx.find_value(symbol);
        assert!(result.is_some());
        result.unwrap()
    }

    #[test]
    fn set_get() {
        let mut ctx = Object::new("world");
        let ctx = &mut ctx;

        {
            let_cap!(v, number::Integer::alloc(1, ctx).into_value(), ctx);
            ctx.define_value("symbol", &v);

            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), ctx), ctx);
            let_cap!(result, world_get(&symbol, ctx), ctx);
            let_cap!(ans, number::Integer::alloc(1, ctx).into_value(), ctx);
            assert_eq!((*result).as_ref(), ans.nptr().as_ref());


            let_cap!(v, number::Real::alloc(3.14, ctx).into_value(), ctx);
            ctx.define_value("symbol", &v);

            let_cap!(symbol, symbol::Symbol::alloc(&"symbol".to_string(), ctx), ctx);
            let_cap!(result, world_get(&symbol, ctx), ctx);
            let_cap!(ans, number::Real::alloc(3.14, ctx).into_value(), ctx);
            assert_eq!(result.nptr().as_ref(), ans.nptr().as_ref());

            let_cap!(v2, string::NString::alloc(&"bar".to_string(), ctx).into_value(), ctx);
            ctx.define_value("hoge", &v2);

            let_cap!(symbol2, symbol::Symbol::alloc(&"hoge".to_string(), ctx), ctx);
            let_cap!(result, world_get(&symbol2, ctx), ctx);
            let_cap!(ans2, string::NString::alloc(&"bar".to_string(), ctx).into_value(), ctx);
            assert_eq!(result.nptr().as_ref(), ans2.nptr().as_ref());

            let_cap!(result, world_get(&symbol, ctx), ctx);
            assert_eq!(result.nptr().as_ref(), ans.nptr().as_ref());
        }
    }

}