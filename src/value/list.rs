#![allow(unused_unsafe)]

use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use crate::vm;
use std::fmt::{self, Debug, Display};

pub struct List {
    v: FPtr<Value>,
    next: FPtr<List>,
}

static LIST_TYPEINFO : TypeInfo = new_typeinfo!(
    List,
    "List",
    std::mem::size_of::<List>(),
    None,
    List::eq,
    List::clone_inner,
    Display::fmt,
    List::is_type,
    None,
    None,
    Some(List::child_traversal),
);

impl NaviType for List {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&LIST_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        if self.is_nil() {
            //Nilの場合はImmidiate Valueなのでそのまま返す
            FPtr::new(self)
        } else {
            //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
            unsafe {
                let v = crate::value::Value::clone_inner(self.v.as_ref(), obj).into_reachable();
                let next = Self::clone_inner(self.next.as_ref(), obj).into_reachable();

                Self::alloc(&v, &next, obj)
            }
        }
    }
}


impl List {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&LIST_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.v, arg);
        callback(self.next.cast_value(), arg);
    }

    pub fn nil() -> Reachable<List> {
        Reachable::<List>::new_immidiate(IMMIDATE_NIL)
    }

    pub fn is_nil(&self) -> bool {
        std::ptr::eq(self as *const List, IMMIDATE_NIL as *const List)
    }

    pub fn alloc(v: &Reachable<Value>, next: &Reachable<List>, obj: &mut Object) -> FPtr<List> {
        let ptr = obj.alloc::<List>();
        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: FPtr::new(v.as_ref()),
                next: FPtr::new(next.as_ref()),
                })
        }

        ptr.into_fptr()
    }

    pub fn alloc_tail(v: &Reachable<Value>, obj: &mut Object) -> FPtr<List> {
        let ptr = obj.alloc::<List>();

        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: FPtr::new(v.as_ref()),
                next: Self::nil().into_fptr(),
                })
        }

        ptr.into_fptr()
    }

    pub fn head(&self) -> FPtr<Value> {
        self.v.clone()
    }

    pub fn tail(&self) -> FPtr<List> {
        self.next.clone()
    }

    pub fn count(&self) -> usize {
        let mut count = 0;

        let mut l = self;
        loop {
            if l.is_nil() {
                break
            } else {
                count += 1;
                l = l.next.as_ref();
            }
        }

        count
    }

    pub fn len_more_than(&self, count: usize) -> bool {
        let mut count = count;
        let mut l = self;
        loop {
            if l.is_nil() {
                break
            } else {
                count -= 1;
                if count == 0 {
                    break
                }
                l = l.next.as_ref();
            }
        }

        count == 0
    }

    pub fn len_exactly(&self, count: usize) -> bool {
        let mut count = count;
        let mut l = self;
        loop {
            if l.is_nil() {
                break
            } else {
                count -= 1;
                l = l.next.as_ref();
                if count == 0 {
                    break
                }
            }
        }

        count == 0 && l.is_nil()
    }

    pub fn get(&self, mut index: usize) -> FPtr<Value> {
        for v in unsafe { self.iter_gcunsafe() } {
            if index == 0 {
                return v;
            } else {
                index = index - 1;
            }
        }

        panic!("out of bounds {}: {}", index, self)
    }

    pub unsafe fn iter_gcunsafe(&self) -> ListIteratorGCUnsafe {
        ListIteratorGCUnsafe {
            cur: FPtr::new(self),
        }
    }

}

impl Eq for List { }

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self as *const List, other as *const List) {
            true
        } else if self.is_nil() {
            other.is_nil()
        } else if other.is_nil() {
            false
        } else {
            self.v.as_ref().eq(&other.v.as_ref()) && self.next.as_ref().eq(&other.next.as_ref())
        }
    }
}

fn display(this: &List, f: &mut fmt::Formatter<'_>, is_debug: bool) -> fmt::Result {
    let mut first = true;
    write!(f, "(")?;
    //TODO GCが動かない前提のinner iterを作るか？
    for v in unsafe { this.iter_gcunsafe() } {
        if !first {
            write!(f, " ")?
        }

        if is_debug {
            Debug::fmt(v.as_ref(), f)?;
        } else {
            Display::fmt(v.as_ref(), f)?;
        }

        first = false;
    }
    write!(f, ")")
}

impl Display for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, false)
    }
}

impl Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, true)
    }
}

impl Reachable<List> {
    pub fn iter(&self, obj: &mut Object) -> ListIterator {
        ListIterator::new(self, obj)
    }

    pub fn iter_with_info(&self, obj: &mut Object) -> ListIteratorWithInfo {
        ListIteratorWithInfo::new(self, obj)
    }
}

pub struct ListIteratorGCUnsafe {
    cur: FPtr<List>,
}

impl std::iter::Iterator for ListIteratorGCUnsafe {
    type Item = FPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur.as_ref().is_nil() {
            None
        } else {
            let v = self.cur.as_ref().head();
            self.cur = self.cur.as_ref().tail();
            Some(v)
        }
    }
}

pub struct ListIterator {
    cur: Cap<List>,
}

impl ListIterator {
    pub fn new(list: &Reachable<List>, obj: &mut Object) -> Self {
        ListIterator {
            cur: FPtr::new(list.as_ref()).capture(obj)
        }
    }
}

impl std::iter::Iterator for ListIterator {
    type Item = FPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur.as_ref().is_nil() {
            None
        } else {
            let v = self.cur.as_ref().head();
            self.cur.update_pointer(self.cur.as_ref().tail());
            Some(v)
        }
    }
}

pub struct ListIteratorInfo {
    pub is_tail: bool
}

pub struct ListIteratorWithInfo {
    cur: Cap<List>,
}

impl ListIteratorWithInfo {
    pub fn new(list: &Reachable<List>, obj: &mut Object) -> Self {
        ListIteratorWithInfo {
            cur: FPtr::new(list.as_ref()).capture(obj)
        }
    }
}

impl std::iter::Iterator for ListIteratorWithInfo {
    type Item = (FPtr<Value>, ListIteratorInfo);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur.as_ref().is_nil() {
            None
        } else {
            let v = self.cur.as_ref().head();
            let next = self.cur.as_ref().tail();
            let is_tail = next.as_ref().is_nil();

            self.cur.update_pointer(next);
            Some((v,ListIteratorInfo { is_tail: is_tail }))
        }
    }
}

pub struct ListBuilder {
    start: Option<Cap<List>>,
    end: Option<Cap<List>>,
    len: usize,
}

#[macro_export]
macro_rules! cap_append {
    ($builder:expr, $ptr:expr, $obj:expr) => {
        {
            let tmp = ($ptr).reach($obj);
            ($builder).append(&tmp, $obj)
        }
    };
}

impl ListBuilder {
    pub fn new(_obj: &mut Object) -> Self {
        ListBuilder {
            start: None,
            end: None,
            len: 0,
        }
    }

    pub fn append(&mut self, v: &Reachable<Value>, obj: &mut Object) {
        let cell = List::alloc_tail(v, obj);

        if self.start.is_none() {
            self.start = Some(obj.capture(cell.clone()));
            self.end = Some(obj.capture(cell));

        } else {
            let end = self.end.as_mut().unwrap();
            end.as_mut().next = cell.clone();

            end.update_pointer(cell);
        }

        self.len += 1;
    }

    pub fn get(self) -> FPtr<List> {
        let result = if let Some(start) = self.start {
            start.take()
        } else {
            list::List::nil().into_fptr()
        };

        result
    }

    pub fn get_with_size(self) -> (FPtr<List>, usize) {
        (
            if let Some(start) = self.start {
                start.take()
            } else {
                list::List::nil().into_fptr()
            }
            , self.len
        )
    }

}

fn func_is_list(obj: &mut Object) -> FPtr<Value> {
    let v = vm::refer_arg(0, obj);
    if v.is_type(list::List::typeinfo()) {
        v.clone()
    } else {
        bool::Bool::false_().into_fptr().into_value()
    }
}

fn func_list_len(obj: &mut Object) -> FPtr<Value> {
    let v = vm::refer_arg::<List>(0, obj);

    number::Integer::alloc(v.as_ref().count() as i64, obj).into_value()
}

fn func_list_ref(obj: &mut Object) -> FPtr<Value> {
    let v = vm::refer_arg::<List>(0, obj);
    let index = vm::refer_arg::<number::Integer>(1, obj);

    let index = index.as_ref().get() as usize;

    v.as_ref().get(index).clone()
}

static FUNC_IS_LIST: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list?",
            &[
            Param::new("x", ParamKind::Require, Value::typeinfo()),
            ],
            func_is_list)
    )
});

static FUNC_LIST_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list-len",
            &[
            Param::new("list", ParamKind::Require, List::typeinfo()),
            ],
            func_list_len)
    )
});

static FUNC_LIST_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list-ref",
            &[
            Param::new("list", ParamKind::Require, List::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ],
            func_list_ref)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("list?", &FUNC_IS_LIST.value);
    obj.define_global_value("list-len", &FUNC_LIST_LEN.value);
    obj.define_global_value("list-ref", &FUNC_LIST_REF.value);
}

pub mod literal {
    use super::*;

    pub fn is_list() -> Reachable<Func> {
        Reachable::new_static(&FUNC_IS_LIST.value)
    }

    pub fn list_len() -> Reachable<Func> {
        Reachable::new_static(&FUNC_LIST_LEN.value)
    }

    pub fn list_ref() -> Reachable<Func> {
        Reachable::new_static(&FUNC_LIST_REF.value)
    }

}

#[cfg(test)]
mod tests {
    use crate::value::*;
    use crate::value::list::ListBuilder;

    #[test]
    fn test() {
        let mut obj = Object::new();
        let obj = &mut obj;

        let mut ans_obj = Object::new();
        let ans_obj = &mut ans_obj;

        {
            let builder = ListBuilder::new(obj);
            let result = builder.get();

            let ans = list::List::nil();

            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let mut builder = ListBuilder::new(obj);

            cap_append!(builder, number::Integer::alloc(1, obj).into_value(), obj);
            cap_append!(builder, number::Integer::alloc(2, obj).into_value(), obj);
            cap_append!(builder, number::Integer::alloc(3, obj).into_value(), obj);
            let result = builder.get().capture(obj);

            let _1 = number::Integer::alloc(1, ans_obj).into_value().reach(ans_obj);
            let _2 = number::Integer::alloc(2, ans_obj).into_value().reach(ans_obj);
            let _3 = number::Integer::alloc(3, ans_obj).into_value().reach(ans_obj);

            let ans = list::List::nil();
            let ans = list::List::alloc(&_3, &ans, ans_obj).reach(obj);
            let ans = list::List::alloc(&_2, &ans, ans_obj).reach(obj);
            let ans = list::List::alloc(&_1, &ans, ans_obj).reach(obj);

            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }
}