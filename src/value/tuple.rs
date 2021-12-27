use crate::value::*;
use crate::ptr::*;
use crate::object::context::Context;
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
    Tuple::is_type,
    None,
    None,
    Some(Tuple::child_traversal),
);

impl NaviType for Tuple {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&TUPLE_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        if self.is_unit() {
            //UnitはImmidiate Valueなのでそのまま返す
            FPtr::new(self)
        } else {
            let size = self.len();
            let mut tuple = Self::alloc(size, obj);

            for index in 0..size {
                let child = self.get_inner(index);
                //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
                let cloned = Value::clone_inner(child.as_ref(), obj);

                tuple.as_mut().set(cloned.as_ref(), index);
            }

            tuple
        }
    }

}

impl Tuple {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Tuple>()
            + self.len * std::mem::size_of::<FPtr<Value>>()
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&TUPLE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        for index in 0..self.len {
            callback(self.get_inner(index), arg);
        }
    }

    #[inline(always)]
    pub fn unit() -> Reachable<Tuple> {
        Reachable::<Tuple>::new_immidiate(IMMIDATE_UNIT)
    }

    #[inline(always)]
    pub fn is_unit(&self) -> bool {
        std::ptr::eq(self as *const Self, IMMIDATE_UNIT as *const Self)
    }

    fn alloc(size: usize, obj: &mut Object) -> FPtr<Tuple> {
        let ptr = obj.alloc_with_additional_size::<Tuple>(size * std::mem::size_of::<FPtr<Value>>());

        unsafe {
            std::ptr::write(ptr.as_ptr(), Tuple {len: size});
        }

        ptr.into_fptr()
    }

    fn set(&mut self, v: &Value, index: usize) {
        if self.len() <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *mut Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
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
        if self.len() <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut FPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &*(storage_ptr)
        }
    }

    pub fn len(&self) -> usize {
        if self.is_unit() {
            0
        } else {
            self.len
        }
    }

    pub fn from_array(ary: &Reachable<array::Array>, obj: &mut Object) -> FPtr<Tuple> {
        let len = ary.as_ref().len();

        if len == 0 {
            Self::unit().into_fptr()

        } else {
            let mut tuple = Self::alloc(len, obj);
            for index in 0..len {
                tuple.as_mut().set(ary.as_ref().get(index).as_ref(), index);
            }

            tuple
        }

    }

    pub fn from_list(list: &Reachable<list::List>, size: Option<usize>, obj: &mut Object) -> FPtr<Tuple> {
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        if size == 0 {
            Self::unit().into_fptr()

        } else {
            let mut tuple = Self::alloc(size, obj);
            for (index, v) in list.iter(obj).enumerate() {
                tuple.as_mut().set(v.as_ref(), index);
            }

            tuple
        }
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

fn func_is_tuple(args: &Reachable<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    if v.is_type(tuple::Tuple::typeinfo()) {
        v.clone()
    } else {
        bool::Bool::false_().into_fptr().into_value()
    }
}

fn func_tuple_len(args: &Reachable<array::Array>, obj: &mut Object) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<tuple::Tuple>() };

    number::Integer::alloc(v.as_ref().len as i64, obj).into_value()
}

fn func_tuple_ref(args: &Reachable<array::Array>, _obj: &mut Object) -> FPtr<Value> {
    let tuple = args.as_ref().get(0);
    let tuple = unsafe { tuple.cast_unchecked::<tuple::Tuple>() };

    let index = args.as_ref().get(1);
    let index = unsafe { index.cast_unchecked::<number::Integer>() };

    tuple.as_ref().get(index.as_ref().get() as usize)
        .clone()
}

static FUNC_IS_TUPLE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple?",
            &[
            Param::new("x", ParamKind::Require, Value::typeinfo()),
            ],
            func_is_tuple)
    )
});

static FUNC_TUPLE_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple-len",
            &[
            Param::new("tuple", ParamKind::Require, Tuple::typeinfo()),
            ],
            func_tuple_len)
    )
});

static FUNC_TUPLE_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("tuple-ref",
            &[
            Param::new("tuple", ParamKind::Require, Tuple::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ],
            func_tuple_ref)
    )
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("tuple?", Reachable::new_static(&FUNC_IS_TUPLE.value).cast_value());
    ctx.define_value("tuple-len", Reachable::new_static(&FUNC_TUPLE_LEN.value).cast_value());
    ctx.define_value("tuple-ref", Reachable::new_static(&FUNC_TUPLE_REF.value).cast_value());
}

pub mod literal {
    use crate::ptr::*;
    use crate::value::func::Func;
    use super::*;

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
