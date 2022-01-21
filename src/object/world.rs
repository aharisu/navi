use crate::value::any::Any;
use crate::ptr::*;

mod map;

//TODO Worldはざっくりいうとグローバル変数空間

pub struct World {
    area: crate::object::world::map::PatriciaTree<Ref<Any>>,
}

impl World {
    pub fn new() -> Self {
        World {
            area: crate::object::world::map::PatriciaTree::new(),
        }
    }

    pub fn set<K>(&mut self, key: K, v: &Ref<Any>)
    where
        K: AsRef<str>,
    {
        self.area.add(key, v.clone())
    }

    pub fn get<K>(&self, key: K) -> Option<&Ref<Any>>
    where
        K: AsRef<str>
    {
        self.area.get(key)
    }

    pub(crate) fn for_each_all_value<F: FnMut(&mut Ref<Any>)>(&mut self, callback: F) {
        self.area.for_each_all_value(callback);
    }
}

impl Clone for World {
    fn clone(&self) -> Self {
        World {
            area: self.area.clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::value::any::Any;
    use crate::{value::*};
    use crate::ptr::*;

    fn world_get(symbol: &symbol::Symbol, obj: &Object) -> Ref<Any> {
        let result = obj.find_global_value(symbol);
        assert!(result.is_some());
        result.unwrap()
    }

    #[test]
    fn set_get() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        {
            let v = number::make_integer(1, obj).unwrap();
            obj.define_global_value("symbol", &v);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), obj).unwrap();
            let result = world_get(symbol.as_ref(), obj).reach(obj);
            let ans = number::make_integer(1, obj).unwrap().reach(obj);
            assert_eq!(result.as_ref(), ans.as_ref());


            let v = number::Real::alloc(3.14, obj).unwrap().into_value();
            obj.define_global_value("symbol", &v);

            let symbol = symbol::Symbol::alloc(&"symbol".to_string(), obj).unwrap();
            let result = world_get(symbol.as_ref(), obj).reach(obj);
            let ans = number::Real::alloc(3.14, obj).unwrap().into_value().reach(obj);
            assert_eq!(result.as_ref(), ans.as_ref());

            let v2 = string::NString::alloc(&"bar".to_string(), obj).unwrap().into_value();
            obj.define_global_value("hoge", &v2);

            let symbol2 = symbol::Symbol::alloc(&"hoge".to_string(), obj).unwrap();
            let result = world_get(symbol2.as_ref(), obj).reach(obj);
            let ans2 = string::NString::alloc(&"bar".to_string(), obj).unwrap().into_value().reach(obj);
            assert_eq!(result.as_ref(), ans2.as_ref());

            let result = world_get(symbol.as_ref(), obj);
            assert_eq!(result.as_ref(), ans.as_ref());
        }
    }

}