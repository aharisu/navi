use crate::value::*;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::Display;
use std::fmt::{self, Debug};
use std::pin::Pin;

pub struct Array {
    len: usize,
}

static ARRAY_TYPEINFO : TypeInfo = new_typeinfo!(
    Array,
    "Array",
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

    fn clone_inner(this: &RPtr<Self>, obj: &mut Object) -> FPtr<Self> {
        let size = this.as_ref().len;
        let mut array = Self::alloc(size, obj);

        for index in 0..size {
            let child = this.as_ref().get(index);
            //clone_innerの文脈の中だけ、FPtrをキャプチャせずにRPtrとして扱うことが許されている
            let cloned = Value::clone_inner(child, obj).into_rptr();

            array.as_mut().set(&cloned, index);
        }

        array
    }
}

impl Array {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&ARRAY_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: &usize, callback: fn(&RPtr<Value>, &usize)) {
        for index in 0..self.len {
            callback(self.get(index), arg);
        }
    }

    pub(crate) fn alloc(size: usize, obj: &mut Object) -> FPtr<Array> {
        let ptr = obj.alloc_with_additional_size::<Array>(size * std::mem::size_of::<RPtr<Value>>());
        unsafe {
            std::ptr::write(ptr.as_ptr(), Array { len: size})
        }

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

    pub fn from_list<T>(list: &T, size: Option<usize>, obj: &mut Object) -> FPtr<Array>
    where
        T: AsReachable<list::List>,
    {
        let list = list.as_reachable();
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        let mut array = Self::alloc(size, obj);
        for (index, v) in list.as_ref().iter().enumerate() {
            array.as_mut().set(v, index);
        }

        array
    }

    pub fn is_array_func() -> RPtr<Func> {
        RPtr::new(&FUNC_IS_ARRAY.value as *const Func as *mut Func)
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
    pub ary: Capture<Array>,
    pub index: usize,
    pub _pinned: std::marker::PhantomPinned,
}

#[macro_export]
macro_rules! let_arraybuilder {
    ($name:ident, $size:expr, $obj:expr) => {
        let $name = crate::value::array::ArrayBuilder {
            ary: new_cap!(crate::value::array::Array::alloc($size, $obj), $obj),
            index: 0,
            _pinned: std::marker::PhantomPinned,
        };
        pin_utils::pin_mut!($name);
        unsafe {
            ($obj).context().add_capture($name.as_mut().get_unchecked_mut().ary.cast_value_mut());
        }
    };
}

impl ArrayBuilder {
    pub fn push<T>(self: &mut Pin<&mut Self>, v: &T, _obj: &mut Object)
    where
        T: AsReachable<Value>
    {
        let this = unsafe {
            self.as_mut().get_unchecked_mut()
        };
        this.ary.as_mut().set(v, this.index);
        this.index += 1;
    }

    pub fn get(self: Pin<&mut Self>) -> FPtr<Array> {
        self.ary.as_reachable().clone().into_fptr()
    }
}

fn func_is_array(args: &RPtr<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    if v.is_type(array::Array::typeinfo()) {
        v.clone().into_fptr()
    } else {
        bool::Bool::false_().into_value().into_fptr()
    }
}

fn func_array_len(args: &RPtr<array::Array>, obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<Array>() };

    number::Integer::alloc(v.as_ref().len() as i64, obj).into_value()
}

fn func_array_ref(args: &RPtr<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<Array>() };

    let index = args.as_ref().get(1);
    let index = unsafe { index.cast_unchecked::<number::Integer>() };

    let index = index.as_ref().get() as usize;

    v.as_ref().get(index).clone().into_fptr()
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
    ctx.define_value("array?", &RPtr::new(&FUNC_IS_ARRAY.value as *const Func as *mut Func).into_value());
    ctx.define_value("array-len", &RPtr::new(&FUNC_ARRAY_LEN.value as *const Func as *mut Func).into_value());
    ctx.define_value("array-ref", &RPtr::new(&FUNC_ARRAY_REF.value as *const Func as *mut Func).into_value());
}

pub mod literal {
    use super::*;

    pub fn is_array() -> RPtr<Func> {
        RPtr::new(&FUNC_IS_ARRAY.value as *const Func as *mut Func)
    }

    pub fn array_len() -> RPtr<Func> {
        RPtr::new(&FUNC_ARRAY_LEN.value as *const Func as *mut Func)
    }

    pub fn array_ref() -> RPtr<Func> {
        RPtr::new(&FUNC_ARRAY_REF.value as *const Func as *mut Func)
    }

}

#[cfg(test)]
mod tests {
    use crate::{value::*, let_listbuilder, new_cap, with_cap, let_cap};

    #[test]
    fn test() {
        let mut obj = Object::new();
        let obj = &mut obj;

        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let_listbuilder!(builder, obj);

            with_cap!(v, number::Integer::alloc(1, obj).into_value(), obj, {
                builder.append(&v, obj);
            });
            with_cap!(v, number::Real::alloc(3.14, obj).into_value(), obj, {
                builder.append(&v, obj);
            });
            builder.append(&list::List::nil().into_value(), obj);
            builder.append(&bool::Bool::true_().into_value(), obj);

            let (list, size) = builder.get_with_size();
            let_cap!(list, list, obj);
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