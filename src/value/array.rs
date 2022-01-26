use crate::value::{*, self};
use crate::ptr::*;
use crate::err::*;
use crate::vm;
use std::fmt::Display;
use std::fmt::{self, Debug};
use std::marker::PhantomData;

pub struct Array<T: NaviType> {
    len: usize,
    _type: PhantomData<T>,
}

static ARRAY_TYPEINFO : TypeInfo = new_typeinfo!(
    Array<Any>,
    "Array",
    0,
    Some(Array::<Any>::size_of),
    Array::<Any>::eq,
    Array::<Any>::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    Some(Array::<Any>::child_traversal),
    Some(Array::<Any>::check_reply),
);

impl <T:NaviType> NaviType for Array<T> {
    fn typeinfo() -> &'static TypeInfo {
        &ARRAY_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        let size = self.len;
        let mut array = Self::alloc(size, allocator)?;

        for index in 0..size {
            let child = self.get_inner(index);
            let child = child.cast_value();
            //clone_innerの文脈の中だけ、PtrをキャプチャせずにRPtrとして扱うことが許されている
            let cloned = Any::clone_inner(child.as_ref(), allocator)?;
            let cloned = unsafe { cloned.cast_unchecked::<T>() };

            array.set_uncheck(cloned.raw_ptr(), index);
        }

        Ok(array)
    }
}

impl <T: NaviType> Array<T> {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Array<T>>()
            + self.len * std::mem::size_of::<Ref<Any>>()
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        for index in 0..self.len {
            callback(self.get_inner(index).cast_mut_value(), arg);
        }
    }

    fn check_reply(cap: &mut Cap<Array<Any>>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        for index in 0.. cap.as_ref().len {
            let child_v = cap.as_ref().get_inner(index);
            //子要素がReply型を持っている場合は
            if child_v.has_replytype() {

                //返信がないか確認する
                let mut child_v = child_v.clone().capture(obj);
                if value::check_reply(&mut child_v, obj)? {
                    //返信があった場合は、内部ポインタを返信結果の値に上書きする
                    cap.as_ref().get_inner(index).update_pointer(child_v.raw_ptr());
                } else {
                    //子要素にReplyを含む値が残っている場合は、全体をfalseにする
                    return Ok(false);
                }
            }
        }

        //内部にReply型を含まなくなったのでフラグを下す
        value::clear_has_replytype_flag(cap.mut_refer());

        Ok(true)
    }

    fn alloc<A: Allocator>(size: usize, allocator: &mut A) -> NResult<Array<T>, OutOfMemory> {
        let ptr = allocator.alloc_with_additional_size::<Array<T>>(size * std::mem::size_of::<Ref<Any>>())?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), Array { len: size, _type: PhantomData})
        }

        Ok(ptr.into_ref())
    }

    pub fn get(&self, index: usize) -> Ref<T> {
        self.get_inner(index).clone()
    }

    fn get_inner<'a>(&'a self, index: usize) -> &'a mut Ref<T> {
        let ptr = self as *const Array<T>;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<T>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &mut *(storage_ptr)
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub unsafe fn iter_gcunsafe<'a>(&'a self) -> ArrayIteratorGCUnsafe<'a, T> {
        ArrayIteratorGCUnsafe {
            ary: self,
            index: 0,
        }
    }
}

impl Array<Any> {
    pub fn from_list(list: &Reachable<list::List>, size: Option<usize>, obj: &mut Object) -> NResult<Array<Any>, OutOfMemory> {
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        let mut array = Self::alloc(size, obj)?;
        for (index, v) in list.iter(obj).enumerate() {
            array.set_uncheck(v.raw_ptr(), index);
        }

        //listがReplyを持っている場合は、返す値にもReplyを持っているフラグを立てる
        if list.has_replytype() {
            value::set_has_replytype_flag(&mut array)
        }

        Ok(array)
    }
}

impl <T: NaviType> Ref<Array<T>> {

    fn set<V: ValueHolder<T>>(&mut self, v: &V, index: usize) -> Result<(), OutOfBounds> {
        if self.as_ref().len <= index {
            return Err(OutOfBounds::new(self.cast_value().clone(), index));
        }

        self.set_uncheck(v.raw_ptr(), index);

        if v.has_replytype() {
            value::set_has_replytype_flag(self);
        }

        Ok(())
    }

    fn set_uncheck(&mut self, v: *mut T, index: usize) {
        let ptr = self.as_mut() as *mut Array<T>;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Array構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<T>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, v.into());
        };
    }


}

impl <T: NaviType> Eq for Array<T> { }

impl <T: NaviType> PartialEq for Array<T> {
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

fn display<T: NaviType>(this: &Array<T>, f: &mut fmt::Formatter<'_>, is_debug: bool) -> fmt::Result {
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

impl <T: NaviType> Display for Array<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, false)
    }
}

impl <T: NaviType> Debug for Array<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, true)
    }
}

pub struct ArrayIteratorGCUnsafe<'a, T: NaviType> {
    ary: &'a Array<T>,
    index: usize,
}

impl <'a, T: NaviType> std::iter::Iterator for ArrayIteratorGCUnsafe<'a, T> {
    type Item = Ref<T>;

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

impl <T: NaviType> Reachable<Array<T>> {
    pub fn iter(&self) -> ArrayIterator<T> {
        ArrayIterator {
            ary: self,
            index: 0,
        }
    }
}

pub struct ArrayIterator<'a, T: NaviType> {
    ary: &'a Reachable<Array<T>>,
    index: usize,
}

impl <'a, T: NaviType> std::iter::Iterator for ArrayIterator<'a, T> {
    type Item = Ref<T>;

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

pub struct ArrayBuilder<T: NaviType> {
    ary: Cap<Array<T>>,
    index: usize,
}

impl <T: NaviType> ArrayBuilder<T> {
    pub fn new(size: usize, obj: &mut Object) -> Result<Self, OutOfMemory> {
        let mut ary = Array::<T>::alloc(size, obj)?;

        //pushが完了するまでにGCが動作する可能性があるため、あらかじめ全領域をダミーの値で初期化する
        //ヌルポインタを使用しているがGCの動作に問題はない。
        let dummy_value = std::ptr::null_mut();
        for index in 0..size {
            ary.set_uncheck(dummy_value, index);
        }

        Ok(ArrayBuilder {
            ary: ary.capture(obj),
            index: 0,
        })
    }

    pub unsafe fn push_uncheck<V: ValueHolder<T>>(&mut self, v: &V, _obj: &mut Object) {
        self.ary.mut_refer().set_uncheck(v.raw_ptr(), self.index);

        self.index += 1;
    }

    pub fn push<V: ValueHolder<T>>(&mut self, v: &V, _obj: &mut Object) -> Result<(), OutOfBounds> {
        self.ary.mut_refer().set(v, self.index)?;

        self.index += 1;

        Ok(())
    }

    pub fn get(self) -> Ref<Array<T>> {
        self.ary.take()
    }
}

fn func_array(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let mut builder = ArrayBuilder::new(num_rest, obj)?;

    for index in 0 .. num_rest {
        let v = vm::refer_rest_arg::<Any>(0, index, obj);
        builder.push(&v, obj)?;
    }

    Ok(builder.get().into_value())
}

fn func_is_array(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Any>(0, obj);
    if v.as_ref().is_type(array::Array::<Any>::typeinfo()) {
        Ok(v.clone())
    } else {
        Ok(bool::Bool::false_().into_ref().into_value())
    }
}

fn func_array_len(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Array<Any>>(0, obj);

    let list = number::make_integer(v.as_ref().len() as i64, obj)?;
    Ok(list)
}

fn func_array_ref(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Array<Any>>(0, obj);
    let index = vm::refer_arg::<number::Integer>(1, obj);

    let index = index.as_ref().get() as usize;

    if v.as_ref().len() <= index as usize {
        Err(Exception::OutOfBounds(
            OutOfBounds::new(v.into_value(), index)
        ))
    } else {
        Ok(v.as_ref().get(index))
    }
}

static FUNC_ARRAY: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array",
            &[
            Param::new_no_force("values", ParamKind::Rest, Any::typeinfo()),
            ],
            func_array)
    )
});

static FUNC_IS_ARRAY: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array?",
            &[
            Param::new_no_force("x", ParamKind::Require, Any::typeinfo()),
            ],
            func_is_array)
    )
});

static FUNC_ARRAY_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-len",
            &[
            Param::new_no_force("array", ParamKind::Require, Array::<Any>::typeinfo()),
            ],
            func_array_len)
    )
});

static FUNC_ARRAY_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-ref",
            &[
            Param::new_no_force("array", ParamKind::Require, Array::<Any>::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ],
            func_array_ref)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("array", &Ref::new(&FUNC_ARRAY.value));
    obj.define_global_value("array?", &Ref::new(&FUNC_IS_ARRAY.value));
    obj.define_global_value("array-len", &Ref::new(&FUNC_ARRAY_LEN.value));
    obj.define_global_value("array-ref", &Ref::new(&FUNC_ARRAY_REF.value));
}

pub mod literal {
    use super::*;

    pub fn array() -> Reachable<Func> {
        Reachable::new_static(&FUNC_ARRAY.value)
    }

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
    use crate::value::list::ListBuilder;
    use crate::value::*;

    #[test]
    fn test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let mut builder = ListBuilder::new(obj);

            builder.push(&number::make_integer(1, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::Real::alloc(3.14, obj).unwrap().into_value().reach(obj), obj).unwrap();
            builder.push(list::List::nil().cast_value(), obj).unwrap();
            builder.push(bool::Bool::true_().cast_value(), obj).unwrap();

            let size = builder.len();
            let list = builder.get();
            let list = list.reach(obj);
            let ary = array::Array::from_list(&list, Some(size), obj).unwrap();

            let ans= number::make_integer(1, ans_obj).unwrap();
            assert_eq!(ary.as_ref().get(0).as_ref(), ans.as_ref());

            let ans= number::Real::alloc(3.14, ans_obj).unwrap().into_value();
            assert_eq!(ary.as_ref().get(1).as_ref(), ans.as_ref());

            let ans= list::List::nil().into_value();
            assert_eq!(ary.as_ref().get(2).as_ref(), ans.as_ref());

            let ans= bool::Bool::true_().into_value();
            assert_eq!(ary.as_ref().get(3).as_ref(), ans.as_ref());
        }
    }
}