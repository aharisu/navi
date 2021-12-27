use crate::value::*;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::Display;
use std::fmt::{self, Debug};

pub struct Array {
    len: usize,
}

static ARRAY_TYPEINFO : TypeInfo = new_typeinfo!(
    Array,
    "Array",
    0,
    Some(Array::size_of),
    Array::eq,
    Array::clone_inner,
    Display::fmt,
    Array::is_type,
    None,
    None,
    Some(Array::child_traversal),
);

impl NaviType for Array {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&ARRAY_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        let size = self.len;
        let mut array = Self::alloc(size, obj);

        for index in 0..size {
            let child = self.get_inner(index);
            //clone_innerの文脈の中だけ、PtrをキャプチャせずにRPtrとして扱うことが許されている
            let cloned = Value::clone_inner(child.as_ref(), obj);

            array.as_mut().set(cloned.as_ref(), index);
        }

        array
    }
}

impl Array {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Array>()
            + self.len * std::mem::size_of::<FPtr<Value>>()
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&ARRAY_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        for index in 0..self.len {
            callback(self.get_inner(index), arg);
        }
    }

    pub(crate) fn alloc(size: usize, obj: &mut Object) -> FPtr<Array> {
        let ptr = obj.alloc_with_additional_size::<Array>(size * std::mem::size_of::<FPtr<Value>>());
        unsafe {
            std::ptr::write(ptr.as_ptr(), Array { len: size})
        }

        ptr.into_fptr()
    }

    fn set(&mut self, v: &Value, index: usize)
    {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *mut Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut FPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, FPtr::new(v));
        };
    }

    pub fn get(&self, index: usize) -> FPtr<Value> {
        self.get_inner(index).clone()
    }

    fn get_inner<'a>(&'a self, index: usize) -> &'a FPtr<Value> {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Array;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut FPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &*(storage_ptr)
        }
    }


    pub fn len(&self) -> usize {
        self.len
    }

    pub fn from_list(list: &Reachable<list::List>, size: Option<usize>, obj: &mut Object) -> FPtr<Array> {
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        let mut array = Self::alloc(size, obj);
        for (index, v) in list.iter(obj).enumerate() {
            array.as_mut().set(v.as_ref(), index);
        }

        array
    }

}

impl Reachable<Array> {
    pub fn iter(&self) -> ArrayIterator {
        ArrayIterator {
            ary: self,
            index: 0,
        }
    }
}

pub struct ArrayIterator<'a> {
    ary: &'a Reachable<Array>,
    index: usize,
}

impl <'a> std::iter::Iterator for ArrayIterator<'a> {
    type Item = FPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ary.as_ref().len() <= self.index {
            None
        } else {
            let result = self.ary.as_ref().get(self.index);
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

fn display(this: &Array, f: &mut fmt::Formatter<'_>, is_debug: bool) -> fmt::Result {
    write!(f, "[")?;
    let mut first = true;
    for index in 0..this.len() {
        if !first {
            write!(f, " ")?
        }

        if is_debug {
            Debug::fmt(this.get(index).as_ref(), f)?;
        } else {
            Display::fmt(this.get(index).as_ref(), f)?;
        }
        first = false;
    }
    write!(f, "]")
}

impl Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, false)
    }
}

impl Debug for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, true)
    }
}

pub struct ArrayBuilder {
    ary: Cap<Array>,
    index: usize,
}

impl ArrayBuilder {
    pub fn new(size: usize, obj: &mut Object) -> Self {
        ArrayBuilder {
            ary:  Array::alloc(size, obj).capture(obj),
            index: 0,
        }
    }

    pub fn push(&mut self, v: &Reachable<Value>, _obj: &mut Object) {
        self.ary.as_mut().set(v.as_ref(), self.index);
        self.index += 1;
    }

    pub fn get(self) -> FPtr<Array> {
        self.ary.take()
    }
}

fn func_is_array(args: &Reachable<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    if v.as_ref().is_type(array::Array::typeinfo()) {
        v.clone()
    } else {
        bool::Bool::false_().into_fptr().into_value()
    }
}

fn func_array_len(args: &Reachable<array::Array>, obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<Array>() };

    number::Integer::alloc(v.as_ref().len() as i64, obj).into_value()
}

fn func_array_ref(args: &Reachable<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<Array>() };

    let index = args.as_ref().get(1);
    let index = unsafe { index.cast_unchecked::<number::Integer>() };

    let index = index.as_ref().get() as usize;

    v.as_ref().get(index)
}

static FUNC_IS_ARRAY: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array?",
            &[
            Param::new("x", ParamKind::Require, Value::typeinfo()),
            ],
            func_is_array)
    )
});

static FUNC_ARRAY_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-len",
            &[
            Param::new("array", ParamKind::Require, Array::typeinfo()),
            ],
            func_array_len)
    )
});

static FUNC_ARRAY_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-ref",
            &[
            Param::new("array", ParamKind::Require, Array::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ],
            func_array_ref)
    )
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("array?", Reachable::new_static(&FUNC_IS_ARRAY.value).cast_value());
    ctx.define_value("array-len", Reachable::new_static(&FUNC_ARRAY_LEN.value).cast_value());
    ctx.define_value("array-ref", Reachable::new_static(&FUNC_ARRAY_REF.value).cast_value());
}

pub mod literal {
    use super::*;

    pub fn is_array() -> Reachable<Func> {
        Reachable::new_static(&FUNC_IS_ARRAY.value)
    }

    pub fn array_len() -> Reachable<Func> {
        Reachable::new_static(&FUNC_ARRAY_LEN.value)
    }

    pub fn array_ref() -> Reachable<Func> {
        Reachable::new_static(&FUNC_ARRAY_REF.value)
    }

}

#[cfg(test)]
mod tests {
    use crate::{cap_append};
    use crate::value::list::ListBuilder;
    use crate::value::*;

    #[test]
    fn test() {
        let mut obj = Object::new();
        let obj = &mut obj;

        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let mut builder = ListBuilder::new(obj);

            cap_append!(builder, number::Integer::alloc(1, obj).into_value(), obj);
            cap_append!(builder, number::Real::alloc(3.14, obj).into_value(), obj);
            builder.append(list::List::nil().cast_value(), obj);
            builder.append(bool::Bool::true_().cast_value(), obj);

            let (list, size) = builder.get_with_size();
            let list = list.reach(obj);
            let ary = array::Array::from_list(&list, Some(size), obj);

            let ans= number::Integer::alloc(1, ans_obj).into_value();
            assert_eq!(ary.as_ref().get(0).as_ref(), ans.as_ref());

            let ans= number::Real::alloc(3.14, ans_obj).into_value();
            assert_eq!(ary.as_ref().get(1).as_ref(), ans.as_ref());

            let ans= list::List::nil().into_value();
            assert_eq!(ary.as_ref().get(2).as_ref(), ans.as_ref());

            let ans= bool::Bool::true_().into_value();
            assert_eq!(ary.as_ref().get(3).as_ref(), ans.as_ref());
        }
    }
}