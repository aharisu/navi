use crate::value::*;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::{self, Debug};

pub struct Array {
    len: usize,
}

static ARRAY_TYPEINFO : TypeInfo = new_typeinfo!(
    Array,
    "Array",
    Array::eq,
    Array::fmt,
    Array::is_type,
    Some(Array::child_traversal),
);

impl NaviType for Array {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&ARRAY_TYPEINFO as *const TypeInfo)
    }

}

impl Array {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&ARRAY_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: usize, callback: fn(&RPtr<Value>, usize)) {
        for index in 0..self.len {
            callback(self.get(index), arg);
        }
    }

    fn alloc(size: usize, ctx: &mut Context) -> FPtr<Array> {
        let mut ptr = ctx.alloc_with_additional_size::<Array>(size * std::mem::size_of::<RPtr<Value>>());
        let ary = unsafe { ptr.as_mut() };
        ary.len = size;

        ptr.into_fptr()
    }

    fn set<T>(&mut self, v: &T, index: usize)
    where
        T: AsReachable<Value>
    {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let v = v.as_reachable();
        let ptr = self as *mut Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut RPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, v.clone());
        };
    }

    pub fn get<'a>(&'a self, index: usize) -> &'a RPtr<Value> {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut RPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &*(storage_ptr)
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> ArrayIterator {
        ArrayIterator {
            ary: self,
            index: 0,
        }
    }

    pub fn from_list<T>(list: &T, size: Option<usize>, ctx: &mut Context) -> FPtr<Array>
    where
        T: AsReachable<list::List>,
    {
        let list = list.as_reachable();
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        let mut obj = Self::alloc(size, ctx);
        for (index, v) in list.as_ref().iter().enumerate() {
            obj.as_mut().set(v, index);
        }

        obj
    }
}

pub struct ArrayIterator<'a> {
    ary: &'a Array,
    index: usize,
}

impl <'a> std::iter::Iterator for ArrayIterator<'a> {
    type Item = &'a RPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ary.len() <= self.index {
            None
        } else {
            let result = self.ary.get(self.index);
            self.index += 1;
            Some(result)
        }
    }
}

impl Eq for Array { }

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        if self.len == other.len {
            for index in 0..self.len {
                if self.get(index).as_ref() != other.get(index).as_ref() {
                    return false;
                }
            }

            true
        } else {
            false
        }
    }
}

impl Debug for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //TODO
        write!(f, "Array")
    }
}

#[cfg(test)]
mod tests {
    use crate::{value::*, let_listbuilder, new_cap, with_cap, let_cap};
    use crate::context::{Context};

    #[test]
    fn test() {
        let mut ctx = Context::new("array");
        let ctx = &mut ctx;

        let mut ans_ctx = Context::new("ans");
        let ans_ctx = &mut ans_ctx;

        {
            let_listbuilder!(builder, ctx);

            with_cap!(v, number::Integer::alloc(1, ctx).into_value(), ctx, {
                builder.append(&v, ctx);
            });
            with_cap!(v, number::Real::alloc(3.14, ctx).into_value(), ctx, {
                builder.append(&v, ctx);
            });
            builder.append(&list::List::nil().into_value(), ctx);
            builder.append(&bool::Bool::true_().into_value(), ctx);

            let (list, size) = builder.get_with_size();
            let_cap!(list, list, ctx);
            let ary = array::Array::from_list(&list, Some(size), ctx);

            let ans= number::Integer::alloc(1, ans_ctx).into_value();
            assert_eq!(ary.as_ref().get(0).as_ref(), ans.as_ref());

            let ans= number::Real::alloc(3.14, ans_ctx).into_value();
            assert_eq!(ary.as_ref().get(1).as_ref(), ans.as_ref());

            let ans= list::List::nil().into_value();
            assert_eq!(ary.as_ref().get(2).as_ref(), ans.as_ref());

            let ans= bool::Bool::true_().into_value();
            assert_eq!(ary.as_ref().get(3).as_ref(), ans.as_ref());
        }
    }
}