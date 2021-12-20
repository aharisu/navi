#![allow(unused_unsafe)]

use crate::{value::*, new_cap};
use crate::ptr::*;
use crate::context::{Context};
use std::fmt::{self, Debug, Display};

pub struct List {
    v: RPtr<Value>,
    next: RPtr<List>,
}

static LIST_TYPEINFO : TypeInfo = new_typeinfo!(
    List,
    "List",
    List::eq,
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

    pub fn alloc<V, N>(v: &V, next: &N, ctx: &mut Context) -> FPtr<List>
    where
        V: AsReachable<Value>,
        N: AsReachable<List>,
    {
        let v = v.as_reachable();
        let next = next.as_reachable();

        let ptr = ctx.alloc::<List>();
        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: v.clone(),
                next: next.clone(),
                })
        }

        ptr.into_fptr()
    }

    pub fn alloc_tail<V>(v: &V, ctx: &mut Context) -> FPtr<List>
    where
        V: AsReachable<Value>,
    {
        let v = v.as_reachable();

        let ptr = ctx.alloc::<List>();

        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: v.clone(),
                next: Self::nil(),
                })
        }

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

    pub fn get(&self, mut index: usize) -> &RPtr<Value> {
        for v in self.iter() {
            if index == 0 {
                return v;
            } else {
                index = index - 1;
            }
        }

        panic!("out of bounds {}: {}", index, self)
    }

    pub fn iter(&self) -> ListIterator {
        ListIterator::new(self)
    }

    pub fn iter_with_info(&self) -> ListIteratorWithInfo {
        ListIteratorWithInfo::new(self)
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
    for v in this.iter() {
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

pub struct ListIteratorInfo {
    pub is_tail: bool
}

pub struct ListIteratorWithInfo<'a> {
    list: &'a List,
    cur: Option<&'a RPtr<List>>,
}

impl <'a> ListIteratorWithInfo<'a> {
    pub fn new(list: &'a List) -> Self {
        ListIteratorWithInfo {
            list: list,
            cur: None,
        }
    }
}

impl <'a> std::iter::Iterator for ListIteratorWithInfo<'a> {
    type Item = (&'a RPtr<Value>, ListIteratorInfo);

    fn next(&mut self) -> Option<Self::Item> {
        let cur = match self.cur {
            Some(cur) => cur.as_ref(),
            None => self.list
        };

        if cur.is_nil() {
            None
        } else {
            let v = cur.head_ref();

            let next = cur.tail_ref();
            let is_tail = next.as_ref().is_nil();
            self.cur = Some(next);

            Some((v, ListIteratorInfo { is_tail: is_tail }))
        }
    }
}

use std::pin::Pin;

pub struct ListBuilder {
    pub start: Capture<List>,
    //同一リストの先頭cellがstartとしてキャプチャされているため
    //endはCaptureしなくてもGCの対象にならないことが保証されるためFPtrのまま保持する。
    pub end: FPtr<List>,
    pub len: usize,
    pub _pinned: std::marker::PhantomPinned,
}

#[macro_export]
macro_rules! let_listbuilder {
    ($name:ident, $ctx:expr) => {
        let $name = crate::value::list::ListBuilder {
            start: new_cap!(crate::value::list::List::nil().into_fptr(), $ctx), //nilはimmidiate valueのためadd_captureしなくてもOK
            end: crate::value::list::List::nil().into_fptr(),
            len: 0,
            _pinned: std::marker::PhantomPinned,
        };
        pin_utils::pin_mut!($name);
    };
}


impl ListBuilder {
    pub fn append<T>(self: &mut Pin<&mut Self>, v: &T, ctx: &mut Context)
    where
        T: AsReachable<Value>
    {

        if self.start.as_ref().is_nil() {
            unsafe {
                let this = self.as_mut().get_unchecked_mut();
                let cell = List::alloc_tail(v, ctx);

                this.start = new_cap!(FPtr::new(cell.as_ptr()), ctx);
                ctx.add_capture(this.start.cast_value_mut());

                this.end = cell;

                this.len += 1;
            }
        } else {

            unsafe {
                let this = self.as_mut().get_unchecked_mut();
                let cell = List::alloc_tail(v, ctx);
                this.end.as_mut().next = RPtr::new(cell.as_ptr());

                this.end = cell;

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

    pub fn debug_print(self: &Pin<&mut Self>) {
        println!("{:?}", self.start.as_reachable().as_ref());
    }

}

fn func_is_list(args: &RPtr<array::Array>, _ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    if v.is_type(list::List::typeinfo()) {
        v.clone().into_fptr()
    } else {
        bool::Bool::false_().into_value().into_fptr()
    }
}

fn func_list_len(args: &RPtr<array::Array>, ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<List>() };

    number::Integer::alloc(v.as_ref().count() as i64, ctx).into_value()
}

fn func_list_ref(args: &RPtr<array::Array>, _ctx: &mut Context) -> FPtr<Value> {
    let v = args.as_ref().get(0);
    let v = unsafe { v.cast_unchecked::<List>() };

    let index = args.as_ref().get(1);
    let index = unsafe { index.cast_unchecked::<number::Integer>() };

    let index = index.as_ref().get() as usize;

    v.as_ref().get(index).clone().into_fptr()
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

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("list?", &RPtr::new(&FUNC_IS_LIST.value as *const Func as *mut Func).into_value());
    ctx.define_value("list-len", &RPtr::new(&FUNC_LIST_LEN.value as *const Func as *mut Func).into_value());
    ctx.define_value("list-ref", &RPtr::new(&FUNC_LIST_REF.value as *const Func as *mut Func).into_value());
}

pub mod literal {
    use super::*;

    pub fn is_list() -> RPtr<Func> {
        RPtr::new(&FUNC_IS_LIST.value as *const Func as *mut Func)
    }

    pub fn list_len() -> RPtr<Func> {
        RPtr::new(&FUNC_LIST_LEN.value as *const Func as *mut Func)
    }

    pub fn list_ref() -> RPtr<Func> {
        RPtr::new(&FUNC_LIST_REF.value as *const Func as *mut Func)
    }

}

#[cfg(test)]
mod tests {
    use crate::{value::*, let_cap, new_cap};
    use crate::context::{Context};

    #[test]
    fn test() {
        let mut ctx = Context::new();
        let ctx = &mut ctx;

        let mut ans_ctx = Context::new();
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