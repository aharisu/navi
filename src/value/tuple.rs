use crate::value::*;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::{self, Debug};

pub struct Tuple {
    len: usize,
}

static TUPLE_TYPEINFO : TypeInfo = new_typeinfo!(
    Tuple,
    "Tuple",
    Tuple::eq,
    Tuple::fmt,
    Tuple::is_type,
    None,
    Some(Tuple::child_traversal),
);

impl NaviType for Tuple {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&TUPLE_TYPEINFO as *const TypeInfo)
    }

}

impl Tuple {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&TUPLE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: usize, callback: fn(&RPtr<Value>, usize)) {
        for index in 0..self.len {
            callback(self.get(index), arg);
        }
    }

    #[inline(always)]
    pub fn unit() -> RPtr<Tuple> {
        RPtr::<Tuple>::new_immidiate(IMMIDATE_UNIT)
    }

    #[inline(always)]
    pub fn is_unit(&self) -> bool {
        std::ptr::eq(self as *const Self, IMMIDATE_UNIT as *const Self)
    }

    fn alloc(size: usize, ctx: &mut Context) -> FPtr<Tuple> {
        let mut ptr = ctx.alloc_with_additional_size::<Tuple>(size * std::mem::size_of::<RPtr<Value>>());
        let tuple = unsafe { ptr.as_mut() };
        tuple.len = size;

        ptr.into_fptr()
    }

    fn set<T>(&mut self, v: &T, index: usize)
    where
        T: AsReachable<Value>
    {
        if self.len() <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let v = v.as_reachable();
        let ptr = self as *mut Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut RPtr<Value>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, v.clone());
        };
    }

    pub fn get<'a>(&'a self, index: usize) -> &'a RPtr<Value> {
        if self.len() <= index {
            panic!("out of bounds {}: {:?}", index, self)
        }

        let ptr = self as *const Tuple;
        unsafe {
            //ポインタをTuple構造体の後ろに移す
            let ptr = ptr.add(1);
            //Tuple構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut RPtr<Value>;
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

    pub fn from_list<T>(list: &T, size: Option<usize>, ctx: &mut Context) -> FPtr<Tuple>
    where
        T: AsReachable<list::List>,
    {
        let list = list.as_reachable();
        let size = match size {
            Some(s) => s,
            None => list.as_ref().count(),
        };

        if size == 0 {
            Self::unit().into_fptr()

        } else {
            let mut obj = Self::alloc(size, ctx);
            for (index, v) in list.as_ref().iter().enumerate() {
                obj.as_mut().set(v, index);
            }

            obj
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

impl Debug for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for index in 0..self.len() {
            if !first {
                write!(f, " ")?
            }

            self.get(index).as_ref().fmt(f)?;
            first = false;
        }
        write!(f, "}}")
    }
}