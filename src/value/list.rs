use crate::value::*;
use crate::mm::{self, Heap};
use std::fmt::{self, Debug};

pub struct List {
    v: NBox<Value>,
    next: NBox<List>,
}

impl NaviType for List { }

static LIST_TYPEINFO : TypeInfo<List> = TypeInfo::<List> {
    name: "List",
    eq_func: List::eq,
    print_func: List::fmt,
};

impl List {
    #[inline(always)]
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<List> {
        &LIST_TYPEINFO
    }

    pub fn nil() -> NBox<List> {
        NBox::<List>::new_immidiate(IMMIDATE_NIL)
    }

    pub fn alloc(heap: &mut Heap, v: &NBox<Value>, next: NBox<List>) -> NBox<List> {
        let mut nbox = heap.alloc(Self::typeinfo());
        //確保したメモリ内に値を書き込む
        mm::copy(List {
            v: NBox::new(v.as_mut_ptr() as *mut Value)
            , next: next
        }, nbox.as_mut_ref());

        nbox
    }

    pub fn from_vec(heap: &mut Heap, vec: Vec<NBox<Value>>) -> NBox<List> {
        //TODO gc guard
        let mut acc = Self::nil();
        for v in vec.iter().rev() {
            acc = Self::alloc(heap, v, acc);
        }

        acc
    }
}

impl Eq for List { }

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const List, other as *const List)
        ||(self.v.eq(&other.v)
            && self.next.eq(&other.next))
    }
}

impl Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.v.fmt(f) {
            Ok(_) => self.next.as_ref().fmt(f),
            x => x,
        }
    }
}