use crate::value::*;
use crate::mm::{self, Heap};
use std::fmt::{self, Debug};
use std::ptr::NonNull;

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

    fn alloc(heap: &mut Heap, size: usize) -> NBox<Array> {
        let mut ary = heap.alloc_with_additional_size::<Array>(size * std::mem::size_of::<NonNull<Value>>());
        ary.as_mut_ref().len = size;

        ary
    }

    fn set(self: &mut Array, v: &NBox<Value>, index: usize) {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *mut Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut NonNull<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, NonNull::new_unchecked(v.as_mut_ptr()));
        };
    }

    fn get(self: &Array, index: usize) -> NBox<Value> {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Array;
        let v_ptr = unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut NonNull<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::read(storage_ptr)
        };

        NBox::new(v_ptr.as_ptr())
    }

    pub fn from_slice(heap: &mut Heap, ary: &[NBox<Value>]) -> NBox<Array> {
        let size = ary.len();
        let mut obj = Self::alloc(heap, size);

        for (index, v) in ary.iter().enumerate() {
            obj.as_mut_ref().set(v, index);
        }

        obj
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
    use crate::mm::{Heap};
    use crate::value::*;

    #[test]
    fn test() {
        let mut heap = Heap::new(1024, "array");
        let mut ans = Heap::new(1024, "ans");

        {
            let item1= number::Integer::alloc(&mut heap, 1);
            let item2= number::Real::alloc(&mut heap, 3.14);

            let ary = array::Array::from_slice(&mut heap, &vec![
                item1.into_nboxvalue(),
                item2.into_nboxvalue(),
                list::List::nil().into_nboxvalue(),
                bool::Bool::true_().into_nboxvalue(),
            ]);


            let ans= number::Integer::alloc(&mut heap, 1);
            assert_eq!(ary.as_ref().get(0), ans.into_nboxvalue());

            let ans= number::Real::alloc(&mut heap, 3.14);
            assert_eq!(ary.as_ref().get(1), ans.into_nboxvalue());

            let ans= list::List::nil();
            assert_eq!(ary.as_ref().get(2), ans.into_nboxvalue());

            let ans= bool::Bool::true_();
            assert_eq!(ary.as_ref().get(3), ans.into_nboxvalue());
        }

        heap.free();
        ans.free();
    }
}