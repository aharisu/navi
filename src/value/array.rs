use crate::value::{*, self};
use crate::ptr::*;
use crate::vm;
use std::fmt::Display;
use std::fmt::{self, Debug};
use std::marker::PhantomData;

pub struct Array<T: NaviType> {
    len: usize,
    _type: PhantomData<T>,
}

static ARRAY_TYPEINFO : TypeInfo = new_typeinfo!(
    Array<Value>,
    "Array",
    0,
    Some(Array::<Value>::size_of),
    Array::<Value>::eq,
    Array::<Value>::clone_inner,
    Display::fmt,
    Array::<Value>::is_type,
    None,
    None,
    Some(Array::<Value>::child_traversal),
    Some(Array::<Value>::check_reply),
);

impl <T:NaviType> NaviType for Array<T> {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&ARRAY_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self> {
        let size = self.len;
        let mut array = Self::alloc(size, allocator);

        for index in 0..size {
            let child = self.get_inner(index);
            let child = child.cast_value();
            //clone_innerの文脈の中だけ、PtrをキャプチャせずにRPtrとして扱うことが許されている
            let cloned = Value::clone_inner(child.as_ref(), allocator);
            let cloned = unsafe { cloned.cast_unchecked::<T>() };

            array.as_mut().set_uncheck(cloned.raw_ptr(), index);
        }

        array
    }
}

impl <T: NaviType> Array<T> {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Array<T>>()
            + self.len * std::mem::size_of::<Ref<Value>>()
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&ARRAY_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Value>, *mut u8)) {
        for index in 0..self.len {
            callback(self.get_inner(index).cast_mut_value(), arg);
        }
    }

    fn check_reply(cap: &mut Cap<Array<Value>>, obj: &mut Object) -> bool {
        for index in 0.. cap.as_ref().len {
            let child_v = cap.as_ref().get_inner(index);
            //子要素がReply型を持っている場合は
            if child_v.has_replytype() {

                //返信がないか確認する
                let mut child_v = child_v.clone().capture(obj);
                if value::check_reply(&mut child_v, obj) {
                    //返信があった場合は、内部ポインタを返信結果の値に上書きする
                    cap.as_ref().get_inner(index).update_pointer(child_v.raw_ptr());

                } else {
                    //子要素にReplyを含む値が残っている場合は、全体をfalseにする
                    return false;
                }
            }
        }

        //内部にReply型を含まなくなったのでフラグを下す
        value::clear_has_replytype_flag(cap.mut_refer());

        true
    }

    fn alloc<A: Allocator>(size: usize, allocator: &mut A) -> Ref<Array<T>> {
        let ptr = allocator.alloc_with_additional_size::<Array<T>>(size * std::mem::size_of::<Ref<Value>>());
        unsafe {
            std::ptr::write(ptr.as_ptr(), Array { len: size, _type: PhantomData})
        }

        ptr.into_ref()
    }

    fn set_uncheck(&mut self, v: *mut T, index: usize) {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *mut Array<T>;
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

    pub fn get(&self, index: usize) -> Ref<T> {
        self.get_inner(index).clone()
    }

    fn get_inner<'a>(&'a self, index: usize) -> &'a mut Ref<T> {
        if self.len <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

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

impl Array<Value> {
    pub fn from_list(list: &Reachable<list::List>, size: Option<usize>, obj: &mut Object) -> Ref<Array<Value>> {
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        let mut array = Self::alloc(size, obj);
        for (index, v) in list.iter(obj).enumerate() {
            array.as_mut().set_uncheck(v.raw_ptr(), index);
        }

        //listがReplyを持っている場合は、返す値にもReplyを持っているフラグを立てる
        if list.has_replytype() {
            value::set_has_replytype_flag(&mut array)
        }

        array
    }
}

impl <T: NaviType> Ref<Array<T>> {

    fn set<V: ValueHolder<T>>(&mut self, v: &V, index: usize) {
        self.as_mut().set_uncheck(v.raw_ptr(), index);
        if v.has_replytype() {
            value::set_has_replytype_flag(self);
        }
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
    pub fn new(size: usize, obj: &mut Object) -> Self {
        let mut ary = Array::<T>::alloc(size, obj);

        //pushが完了するまでにGCが動作する可能性があるため、あらかじめ全領域をダミーの値で初期化する
        //ヌルポインタを使用しているがGCの動作に問題はない。
        let dummy_value = std::ptr::null_mut();
        for index in 0..size {
            ary.as_mut().set_uncheck(dummy_value, index);
        }

        ArrayBuilder {
            ary: ary.capture(obj),
            index: 0,
        }
    }

    pub fn push<V: ValueHolder<T>>(&mut self, v: &V, _obj: &mut Object) {
        self.ary.mut_refer().set(v, self.index);

        self.index += 1;
    }

    pub fn get(self) -> Ref<Array<T>> {
        self.ary.take()
    }
}

fn func_is_array(obj: &mut Object) -> Ref<Value> {
    let v = vm::refer_arg::<Value>(0, obj);
    if v.as_ref().is_type(array::Array::<Value>::typeinfo()) {
        v.clone()
    } else {
        bool::Bool::false_().into_ref().into_value()
    }
}

fn func_array_len(obj: &mut Object) -> Ref<Value> {
    let v = vm::refer_arg::<Array<Value>>(0, obj);

    number::Integer::alloc(v.as_ref().len() as i64, obj).into_value()
}

fn func_array_ref(obj: &mut Object) -> Ref<Value> {
    let v = vm::refer_arg::<Array<Value>>(0, obj);
    let index = vm::refer_arg::<number::Integer>(1, obj);

    let index = index.as_ref().get() as usize;

    v.as_ref().get(index)
}

static FUNC_IS_ARRAY: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array?",
            &[
            Param::new_no_force("x", ParamKind::Require, Value::typeinfo()),
            ],
            func_is_array)
    )
});

static FUNC_ARRAY_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-len",
            &[
            Param::new_no_force("array", ParamKind::Require, Array::<Value>::typeinfo()),
            ],
            func_array_len)
    )
});

static FUNC_ARRAY_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("array-ref",
            &[
            Param::new_no_force("array", ParamKind::Require, Array::<Value>::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ],
            func_array_ref)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("array?", &Ref::new(&FUNC_IS_ARRAY.value));
    obj.define_global_value("array-len", &Ref::new(&FUNC_ARRAY_LEN.value));
    obj.define_global_value("array-ref", &Ref::new(&FUNC_ARRAY_REF.value));
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
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        let mut ans_obj = Object::new_for_test();
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