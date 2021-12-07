use crate::value::*;
use crate::mm::{self, Heap};
use std::fmt::{self, Debug};

pub struct List {
    v: NBox<Value>,
    next: NBox<List>,
}

static LIST_TYPEINFO : TypeInfo = new_typeinfo!(
    List,
    "List",
    List::eq,
    List::fmt,
    List::is_type,
);

impl NaviType for List {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&LIST_TYPEINFO as *const TypeInfo)
    }
}


impl List {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&LIST_TYPEINFO, other_typeinfo)
    }

    pub fn nil() -> NBox<List> {
        NBox::<List>::new_immidiate(IMMIDATE_NIL)
    }

    pub fn is_nil(&self) -> bool {
        std::ptr::eq(self as *const List, IMMIDATE_NIL as *const List)
    }

    pub fn alloc(heap: &mut Heap, v: &NBox<Value>, next: NBox<List>) -> NBox<List> {
        let mut nbox = heap.alloc::<List>();
        //確保したメモリ内に値を書き込む
        mm::copy(List {
            v: NBox::new(v.as_mut_ptr() as *mut Value)
            , next: next
        }, nbox.as_mut_ref());

        nbox
    }

    pub fn head_ref(&self) -> &NBox<Value> {
        &self.v
    }

    pub fn tail_ref(&self) -> &NBox<List> {
        &self.next
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

pub struct ListIterator<'a> {
    cur: &'a NBox<List>,
}

impl <'a> ListIterator<'a> {
    pub fn new(list: &'a NBox<List>) -> Self {
        ListIterator {
            cur: list,
        }
    }
}

impl <'a> std::iter::Iterator for ListIterator<'a> {
    type Item = &'a NBox<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur.as_ref().is_nil() {
            None
        } else {
            let v = &self.cur.as_ref().v;

            self.cur = &self.cur.as_ref().next;

            Some(v)
        }

    }
}