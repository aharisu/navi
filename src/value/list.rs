#![allow(unused_unsafe)]

use crate::{value::*, new_cap};
use crate::ptr::*;
use crate::object::{Object};
use std::fmt::{self, Debug};

pub struct List {
    v: RPtr<Value>,
    next: RPtr<List>,
}

static LIST_TYPEINFO : TypeInfo = new_typeinfo!(
    List,
    "List",
    List::eq,
    List::fmt,
    List::is_type,
    Some(List::child_traversal),
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

    fn child_traversal(&self, arg: usize, callback: fn(&RPtr<Value>, arg: usize)) {
        callback(&self.v, arg);
        callback(self.next.cast_value(), arg);
    }

    pub fn nil() -> RPtr<List> {
        RPtr::<List>::new_immidiate(IMMIDATE_NIL)
    }

    pub fn is_nil(&self) -> bool {
        std::ptr::eq(self as *const List, IMMIDATE_NIL as *const List)
    }

    pub fn alloc<V, N>(v: &V, next: &N, ctx: &mut Object) -> FPtr<List>
    where
        V: AsReachable<Value>,
        N: AsReachable<List>,
    {
        let v = v.as_reachable();
        let next = next.as_reachable();
        let mut ptr = ctx.alloc::<List>();
        let list = unsafe { ptr.as_mut() };
        //確保したメモリ内に値を書き込む
        list.v = v.clone();
        list.next = next.clone();

        ptr.into_fptr()
    }

    fn alloc_tail<V>(v: &V, ctx: &mut Object) -> FPtr<List>
    where
        V: AsReachable<Value>,
    {
        let v = v.as_reachable();
        let mut ptr = ctx.alloc::<List>();
        let list = unsafe { ptr.as_mut() };
        //確保したメモリ内に値を書き込む
        list.v = v.clone();
        list.next = Self::nil();

        ptr.into_fptr()
    }

    pub fn head_ref(&self) -> &RPtr<Value> {
        &self.v
    }

    pub fn tail_ref(&self) -> &RPtr<List> {
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

    pub fn iter(&self) -> ListIterator {
        ListIterator::new(self)
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

impl Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.v.as_ref().fmt(f) {
            Ok(_) => self.next.as_ref().fmt(f),
            x => x,
        }
    }
}

//TODO この構造体の値をローカルで保持する場合はGC Captureが必要
pub struct ListIterator<'a> {
    list: &'a List,
    cur: Option<&'a RPtr<List>>,
}

impl <'a> ListIterator<'a> {
    pub fn new(list: &'a List) -> Self {
        ListIterator {
            list: list,
            cur: None,
        }
    }
}

impl <'a> std::iter::Iterator for ListIterator<'a> {
    type Item = &'a RPtr<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.cur {
            if cur.as_ref().is_nil() {
                None
            } else {
                let v = &cur.as_ref().v;
                self.cur = Some(&cur.as_ref().next);

                Some(v)
            }
        } else {
            if self.list.is_nil() {
                None
            } else {
                let v = &self.list.v;
                self.cur = Some(&self.list.next);

                Some(v)
            }
        }
    }
}

use std::pin::Pin;

pub struct ListBuilder {
    pub start: Capture<List>,
    pub end: Capture<List>,
    pub len: usize,
    pub _pinned: std::marker::PhantomPinned,
}

#[macro_export]
macro_rules! let_listbuilder {
    ($name:ident, $ctx:expr) => {
        let $name = crate::value::list::ListBuilder {
            start: new_cap!(crate::value::list::List::nil().into_fptr(), $ctx), //nilはimmidiate valueのためadd_captureしなくてもOK
            end: new_cap!(crate::value::list::List::nil().into_fptr(), $ctx),
            len: 0,
            _pinned: std::marker::PhantomPinned,
        };
        pin_utils::pin_mut!($name);
    };
}


impl ListBuilder {
    pub fn append<T>(self: &mut Pin<&mut Self>, v: &T, ctx: &mut Object)
    where
        T: AsReachable<Value>
    {

        if self.start.as_ref().is_nil() {
            unsafe {
                let this = self.as_mut().get_unchecked_mut();
                let cell = List::alloc_tail(v, ctx);

                this.start = new_cap!(FPtr::new(cell.as_ptr()), ctx);
                ctx.add_capture(this.start.cast_value_mut());

                this.end = new_cap!(cell, ctx);
                ctx.add_capture(this.end.cast_value_mut());

                this.len += 1;
            }
        } else {

            unsafe {
                let this = self.as_mut().get_unchecked_mut();
                let cell = List::alloc_tail(v, ctx);
                this.end.as_mut().next = RPtr::new(cell.as_ptr());

                this.end = new_cap!(cell, ctx);
                ctx.add_capture(this.end.cast_value_mut());

                this.len += 1;
            }
        }
    }

    pub fn get(self: Pin<&mut Self>) -> FPtr<List> {
        self.start.as_reachable().clone().into_fptr()
    }

    pub fn get_with_size(self: Pin<&mut Self>) -> (FPtr<List>, usize) {
        (self.start.as_reachable().clone().into_fptr(), self.len)
    }
}

#[cfg(test)]
mod tests {
    use crate::{value::*, let_cap, new_cap};
    use crate::object::{Object};

    #[test]
    fn test() {
        let mut ctx = Object::new("list");
        let ctx = &mut ctx;

        let mut ans_ctx = Object::new("ans");
        let ans_ctx = &mut ans_ctx;

        {
            let_listbuilder!(builder, ctx);

            let_cap!(result, builder.get(), ctx);
            let ans = list::List::nil();

            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let_listbuilder!(builder, ctx);

            let_cap!(item1, number::Integer::alloc(1, ctx).into_value(), ctx);
            let_cap!(item2, number::Integer::alloc(2, ctx).into_value(), ctx);
            let_cap!(item3, number::Integer::alloc(3, ctx).into_value(), ctx);

            builder.append(&item1, ctx);
            builder.append(&item2, ctx);
            builder.append(&item3, ctx);
            let_cap!(result, builder.get(), ctx);

            let_cap!(_1, number::Integer::alloc(1, ans_ctx).into_value(), ans_ctx);
            let_cap!(_2, number::Integer::alloc(2, ans_ctx).into_value(), ans_ctx);
            let_cap!(_3, number::Integer::alloc(3, ans_ctx).into_value(), ans_ctx);


            let ans = list::List::nil();
            let_cap!(ans, list::List::alloc(&_3, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_2, &ans, ans_ctx), ans_ctx);
            let_cap!(ans, list::List::alloc(&_1, &ans, ans_ctx), ans_ctx);


            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }
}