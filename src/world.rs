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
mod tets {
    use crate::value::*;
    use crate::object::Object;

    fn world_get(symbol: &NBox<symbol::Symbol>, ctx: &mut Object) -> NBox<Value> {
        let result = ctx.find_value(symbol);
        assert!(result.is_some());
        NBox::new(result.unwrap(), ctx)
    }

    #[test]
    fn set_get() {
        let mut ctx = Object::new("world");
        let ctx = &mut ctx;

        {
            let v = NBox::new(number::Integer::alloc(1, ctx).into_value(), ctx);
            ctx.define_value("symbol", &v);

            let symbol = NBox::new(symbol::Symbol::alloc(&"symbol".to_string(), ctx), ctx);
            let result = world_get(&symbol, ctx);
            let ans = NBox::new(number::Integer::alloc(1, ctx).into_value(), ctx);
            assert_eq!(result, ans);


            let v = NBox::new(number::Real::alloc(3.14, ctx).into_value(), ctx);
            ctx.define_value("symbol", &v);

            let symbol = NBox::new(symbol::Symbol::alloc(&"symbol".to_string(), ctx), ctx);
            let result = world_get(&symbol, ctx);
            let ans = NBox::new(number::Real::alloc(3.14, ctx).into_value(), ctx);
            assert_eq!(result, ans);

            let v2 = NBox::new(string::NString::alloc(&"bar".to_string(), ctx).into_value(), ctx);
            ctx.define_value("hoge", &v2);

            let symbol2 = NBox::new(symbol::Symbol::alloc(&"hoge".to_string(), ctx), ctx);
            let result = world_get(&symbol2, ctx);
            let ans2 = NBox::new(string::NString::alloc(&"bar".to_string(), ctx).into_value(), ctx);
            assert_eq!(result, ans2);

            let result = world_get(&symbol, ctx);
            assert_eq!(result, ans);
        }
    }

}