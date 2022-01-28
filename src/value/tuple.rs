use crate::value::{*, self};
use crate::value::app::{Parameter, ParamKind, Param};
use crate::ptr::*;
use crate::err::*;
use crate::vm;
use std::fmt::{self, Debug};

pub struct Tuple {
    len: usize,
}

static TUPLE_TYPEINFO : TypeInfo = new_typeinfo!(
    Tuple,
    "Tuple",
    0,
    Some(Tuple::size_of),
    Tuple::eq,
    Tuple::clone_inner,
    Tuple::fmt,
    None,
    None,
    None,
    Some(Tuple::child_traversal),
    Some(Tuple::check_reply),
    None,
);

impl NaviType for Tuple {
    fn typeinfo() -> &'static TypeInfo {
        &TUPLE_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        if self.is_unit() {
            //UnitはImmidiate Valueなのでそのまま返す
            Ok(Ref::new(self))
        } else {
            let size = self.len();
            let mut tuple = Self::alloc(size, allocator)?;

            for index in 0..size {
                let child = self.get_inner(index);
                //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
                let cloned = Any::clone_inner(child.as_ref(), allocator)?;

                tuple.set_uncheck(cloned.raw_ptr(), index);
            }

            Ok(tuple)
        }
    }

}

impl Tuple {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Tuple>()
            + self.len * std::mem::size_of::<Ref<Any>>()
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        for index in 0..self.len {
            callback(self.get_inner(index), arg);
        }
    }

    fn check_reply(cap: &mut Cap<Tuple>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        for index in 0.. cap.as_ref().len {
            let child_v = cap.as_ref().get(index);
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


    #[inline(always)]
    pub fn unit() -> Reachable<Tuple> {
        Reachable::<Tuple>::new_immidiate(IMMIDATE_UNIT)
    }

    #[inline(always)]
    pub fn is_unit(&self) -> bool {
        std::ptr::eq(self as *const Self, IMMIDATE_UNIT as *const Self)
    }

    fn alloc<A: Allocator>(size: usize, allocator: &mut A) -> NResult<Tuple, OutOfMemory> {
        let ptr = allocator.alloc_with_additional_size::<Tuple>(size * std::mem::size_of::<Ref<Any>>())?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Tuple {len: size});
        }

        Ok(ptr.into_ref())
    }

    pub fn get(&self, index: usize) -> Ref<Any> {
        self.get_inner(index).clone()
    }

    fn get_inner<'a>(&'a self, index: usize) -> &'a mut Ref<Any> {
        let ptr = self as *const Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<Any>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &mut *(storage_ptr)
        }
    }

    pub fn len(&self) -> usize {
        if self.is_unit() {
            0
        } else {
            self.len
        }
    }

    pub fn from_array(ary: &Reachable<array::Array<Any>>, obj: &mut Object) -> NResult<Tuple, OutOfMemory> {
        let len = ary.as_ref().len();

        if len == 0 {
            Ok(Self::unit().into_ref())

        } else {
            let mut tuple = Self::alloc(len, obj)?;
            for index in 0..len {
                tuple.set_uncheck(ary.as_ref().get(index).raw_ptr(), index);
            }

            //listがReplyを持っている場合は、返す値にもReplyを持っているフラグを立てる
            if ary.has_replytype() {
                value::set_has_replytype_flag(&mut tuple)
            }

            Ok(tuple)
        }
    }

    pub fn from_list(list: &Reachable<list::List>, size: Option<usize>, obj: &mut Object) -> NResult<Tuple, OutOfMemory> {
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        if size == 0 {
            Ok(Self::unit().into_ref())

        } else {
            let mut tuple = Self::alloc(size, obj)?;
            for (index, v) in list.iter(obj).enumerate() {
                tuple.set_uncheck(v.raw_ptr(), index);
            }

            //listがReplyを持っている場合は、返す値にもReplyを持っているフラグを立てる
            if list.has_replytype() {
                value::set_has_replytype_flag(&mut tuple)
            }

            Ok(tuple)
        }
    }
}

impl Ref<Tuple> {

    fn set<V: ValueHolder<Any>>(&mut self, v: &V, index: usize)  -> Result<(), OutOfBounds> {
        if self.as_ref().len() <= index {
            return Err(OutOfBounds::new(self.cast_value().clone(), index))
        }

        self.set_uncheck(v.raw_ptr(), index);

        if v.has_replytype() {
            value::set_has_replytype_flag(self);
        }

        Ok(())
    }

    fn set_uncheck(&mut self, v: *mut Any, index: usize) {
        let ptr = self.as_mut() as *mut Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<Any>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, v.into());
        };
    }


}


impl Eq for Tuple { }

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        if self.len() == other.len() {
            for index in 0..self.len() {
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

fn display(this: &Tuple, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{{")?;
    let mut first = true;
    for index in 0..this.len() {
        if !first {
            write!(f, " ")?
        }

        this.get(index).as_ref().fmt(f)?;
        first = false;
    }
    write!(f, "}}")
}

impl std::fmt::Display for Tuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl Debug for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}

pub struct TupleBuilder {
    tuple: Cap<Tuple>,
    index: usize,
}

impl TupleBuilder {
    pub fn new(size: usize, obj: &mut Object) -> Result<Self, OutOfMemory> {
        let mut tuple = Tuple::alloc(size, obj)?;

        //pushが完了するまでにGCが動作する可能性があるため、あらかじめ全領域をダミーの値で初期化する
        let dummy_value = bool::Bool::false_().into_value().raw_ptr();
        for index in 0..size {
            tuple.set_uncheck(dummy_value, index);
        }

        Ok(TupleBuilder {
            tuple: tuple.capture(obj),
            index: 0,
        })
    }

    pub fn push<V: ValueHolder<Any>>(&mut self, v: &V, _obj: &mut Object) -> Result<(), OutOfBounds> {
        self.tuple.mut_refer().set(v, self.index)?;
        self.index += 1;

        Ok(())
    }

    pub fn get(self) -> Ref<Tuple> {
        self.tuple.take()
    }
}

fn func_tuple(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let mut builder = TupleBuilder::new(num_rest, obj)?;

    for index in 0 .. num_rest {
        let v = vm::refer_rest_arg::<Any>(0, index, obj);
        builder.push(&v, obj)?;
    }

    Ok(builder.get().into_value())
}

fn func_is_tuple(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Any>(0, obj);
    if v.is_type(tuple::Tuple::typeinfo()) {
        Ok(v.clone())
    } else {
        Ok(bool::Bool::false_().into_ref().into_value())
    }
}

fn func_tuple_len(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<tuple::Tuple>(0, obj);

    let num = number::make_integer(v.as_ref().len as i64, obj)?;
    Ok(num)
}

fn func_tuple_ref(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let tuple = vm::refer_arg::<tuple::Tuple>(0, obj);
    let index = vm::refer_arg::<number::Integer>(1, obj).as_ref().get() as usize;

    if tuple.as_ref().len() <= index as usize {
        Err(Exception::OutOfBounds(
            OutOfBounds::new(tuple.into_value(), index)
        ))
    } else {
        Ok(tuple.as_ref().get(index))
    }
}

static FUNC_TUPLE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple", func_tuple,
            Parameter::new(&[
            Param::new_no_force("values", ParamKind::Rest, Any::typeinfo()),
            ])
        )
    )
});

static FUNC_IS_TUPLE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple?", func_is_tuple,
        Parameter::new(&[
            Param::new_no_force("x", ParamKind::Require, Any::typeinfo()),
            ])
        )
    )
});

static FUNC_TUPLE_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple-len", func_tuple_len,
        Parameter::new(&[
            Param::new_no_force("tuple", ParamKind::Require, Tuple::typeinfo()),
            ])
        )
    )
});

static FUNC_TUPLE_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple-ref", func_tuple_ref,
        Parameter::new(&[
            Param::new_no_force("tuple", ParamKind::Require, Tuple::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ])
        )
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("tuple", &Ref::new(&FUNC_TUPLE.value));
    obj.define_global_value("tuple?", &Ref::new(&FUNC_IS_TUPLE.value));
    obj.define_global_value("tuple-len", &Ref::new(&FUNC_TUPLE_LEN.value));
    obj.define_global_value("tuple-ref", &Ref::new(&FUNC_TUPLE_REF.value));
}

pub mod literal {
    use crate::ptr::*;
    use crate::value::func::Func;
    use super::*;

    pub fn tuple() -> Reachable<Func> {
        Reachable::new_static(&FUNC_TUPLE.value)
    }

    pub fn is_tuple() -> Reachable<Func> {
        Reachable::new_static(&FUNC_IS_TUPLE.value)
    }

    pub fn tuple_len() -> Reachable<Func> {
        Reachable::new_static(&FUNC_TUPLE_LEN.value)
    }

    pub fn tuple_ref() -> Reachable<Func> {
        Reachable::new_static(&FUNC_TUPLE_REF.value)
    }

}
