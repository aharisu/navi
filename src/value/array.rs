use crate::value::*;
use crate::object::Object;
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

    fn alloc(ctx: &mut Object, size: usize) -> NBox<Array> {
        let mut ary = ctx.alloc_with_additional_size::<Array>(size * std::mem::size_of::<NPtr<Value>>());
        ary.as_mut_ref().len = size;

        ary
    }

    fn set<T>(&mut self, v: &T, index: usize)
    where
        T: crate::value::AsPtr<Value>
    {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *mut Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut NPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, NPtr::new(v.as_mut_ptr()));
        };
    }

    pub(crate) fn get_internal<'a>(&'a self, index: usize) -> &'a NPtr<Value> {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut NPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &*(storage_ptr)
        }
    }

    pub fn get(&self, index: usize) -> NBox<Value> {
        let refer = self.get_internal(index);

        NBox::new(refer.as_mut_ptr())
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

    pub fn from_slice<T>(ctx: &mut Object, ary: &[&T]) -> NBox<Array>
    where
        T: crate::value::AsPtr<Value>
    {
        let size = ary.len();
        let mut obj = Self::alloc(ctx, size);

        for (index, v) in ary.iter().enumerate() {
            obj.as_mut_ref().set(*v, index);
        }

        obj
    }
}

pub struct ArrayIterator<'a> {
    ary: &'a Array,
    index: usize,
}

impl <'a> std::iter::Iterator for ArrayIterator<'a> {
    type Item = &'a NPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ary.len() <= self.index {
            None
        } else {
            let result = self.ary.get_internal(self.index);
            self.index += 1;
            Some(result)
        }
    }
}

impl Eq for Array { }

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Array, other as *const Array)
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
    use crate::value::*;
    use crate::object::{Object};

    #[test]
    fn test() {
        let mut ctx = Object::new("array");
        let mut ans_ctx = Object::new("ans");

        {
            let item1= number::Integer::alloc(&mut ctx, 1);
            let item2= number::Real::alloc(&mut ctx, 3.14);

            let ary = array::Array::from_slice(&mut ctx, &vec![
                &item1.into_nboxvalue(),
                &item2.into_nboxvalue(),
                &list::List::nil().into_nboxvalue(),
                &bool::Bool::true_().into_nboxvalue(),
            ]);

            let ans= number::Integer::alloc(&mut ans_ctx, 1);
            assert_eq!(ary.as_ref().get(0), ans.into_nboxvalue());

            let ans= number::Real::alloc(&mut ans_ctx, 3.14);
            assert_eq!(ary.as_ref().get(1), ans.into_nboxvalue());

            let ans= list::List::nil();
            assert_eq!(ary.as_ref().get(2), ans.into_nboxvalue());

            let ans= bool::Bool::true_();
            assert_eq!(ary.as_ref().get(3), ans.into_nboxvalue());
        }
    }
}