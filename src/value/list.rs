#![allow(unused_unsafe)]

use crate::object::Object;
use crate::value::{*, self};
use crate::err::*;
use crate::ptr::*;
use crate::vm;
use crate::value::app::{Parameter, ParamKind, Param};
use std::fmt::{self, Debug, Display};

pub struct List {
    v: Ref<Any>,
    next: Ref<List>,
}

static LIST_TYPEINFO : TypeInfo = new_typeinfo!(
    List,
    "List",
    std::mem::size_of::<List>(),
    None,
    List::eq,
    List::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    Some(List::child_traversal),
    Some(List::check_reply),
    None,
);

impl NaviType for List {
    fn typeinfo() -> &'static TypeInfo {
        &LIST_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        if self.is_nil() {
            //Nilの場合はImmidiate Valueなのでそのまま返す
            Ok(Ref::new(self))
        } else {
            //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
            unsafe {
                let v = crate::value::Any::clone_inner(self.v.as_ref(), allocator)?.into_reachable();
                let next = Self::clone_inner(self.next.as_ref(), allocator)?.into_reachable();

                Self::alloc(&v, &next, allocator)
            }
        }
    }
}


impl List {

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(&mut self.v, arg);
        callback(self.next.cast_mut_value(), arg);
    }

    fn check_reply(cap: &mut Cap<List>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        //headにReplyを含んだ値がないかチェック
        if cap.as_ref().v.has_replytype() {
            let mut head = cap.as_ref().head().capture(obj);
            if value::check_reply(&mut head, obj)? {
                cap.as_mut().v.update_pointer(head.raw_ptr());
            } else {
                //Replyがまだ返信を受け取っていなかったのでfalseを返す
                return Ok(false);
            }
        }

        //nextにReplyを含んだ値がないかチェック
        if cap.as_ref().next.has_replytype() {
            //Replyを含んでいる場合は再帰的に確認していく
            let mut tail = cap.as_ref().tail().capture(obj);
            if  Self::check_reply(&mut tail, obj)? {
                cap.as_mut().next.update_pointer(tail.raw_ptr());

            } else {
                //Replyがまだ返信を受け取っていなかったのでfalseを返す
                return Ok(false);
            }
        }

        //内部にReply型を含まなくなったのでフラグを下す
        value::clear_has_replytype_flag(cap.mut_refer());

        Ok(true)
    }

    pub fn nil() -> Reachable<List> {
        Reachable::<List>::new_immidiate(IMMIDATE_NIL)
    }

    pub fn is_nil(&self) -> bool {
        std::ptr::eq(self as *const List, IMMIDATE_NIL as *const List)
    }

    pub fn alloc<A: Allocator>(v: &Reachable<Any>, next: &Reachable<List>, allocator: &mut A) -> NResult<List, OutOfMemory> {
        let ptr = allocator.alloc::<List>()?;
        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: v.raw_ptr().into(),
                next: next.raw_ptr().into(),
                })
        }

        let mut result = ptr.into_ref();
        if v.has_replytype() || next.has_replytype() {
            value::set_has_replytype_flag(&mut result);
        }

        Ok(result)
    }

    pub fn alloc_tail<A: Allocator>(v: &Reachable<Any>, allocator: &mut A) -> NResult<List, OutOfMemory> {
        let ptr = allocator.alloc::<List>()?;

        unsafe {
            //確保したメモリ内に値を書き込む
            std::ptr::write(ptr.as_ptr(), List {
                v: v.raw_ptr().into(),
                next: Self::nil().into_ref(),
                })
        }

        let mut result = ptr.into_ref();
        if v.has_replytype() {
            value::set_has_replytype_flag(&mut result);
        }

        Ok(result)
    }

    pub fn head(&self) -> Ref<Any> {
        self.v.clone()
    }

    pub fn tail(&self) -> Ref<List> {
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

    pub fn get(&self, mut index: usize) -> Ref<Any> {
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
            cur: Ref::new(self),
        }
    }

}

impl Ref<List> {
    pub fn set_has_reply_flag(&mut self, to_index: usize) {
        value::set_has_replytype_flag(self);
        if 0 < to_index {
            self.as_mut().next.set_has_reply_flag(to_index - 1);
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
    cur: Ref<List>,
}

impl std::iter::Iterator for ListIteratorGCUnsafe {
    type Item = Ref<Any>;

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
            cur: list.make().capture(obj),
        }
    }
}

impl std::iter::Iterator for ListIterator {
    type Item = Ref<Any>;

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
            cur: list.make().capture(obj),
        }
    }
}

impl std::iter::Iterator for ListIteratorWithInfo {
    type Item = (Ref<Any>, ListIteratorInfo);

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

impl ListBuilder {
    pub fn new(_obj: &mut Object) -> Self {
        ListBuilder {
            start: None,
            end: None,
            len: 0,
        }
    }

    pub fn push(&mut self, v: &Reachable<Any>, obj: &mut Object) -> Result<(), OutOfMemory> {
        let cell = List::alloc_tail(v, obj)?;

        if self.start.is_none() {
            self.start = Some(obj.capture(cell.clone()));
            self.end = Some(obj.capture(cell));

        } else {
            let end = self.end.as_mut().unwrap();
            end.as_mut().next.update_pointer(cell.raw_ptr());

            end.update_pointer(cell);
        }

        self.len += 1;

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get(self) -> Ref<List> {
        let result = if let Some(start) = self.start {
            start.take()
        } else {
            list::List::nil().into_ref()
        };

        result
    }

    pub fn append_get(mut self, tail: &Ref<List>) -> Ref<List> {
        match &mut self.end {
            Some(end) => {
                end.as_mut().next.update_pointer(tail.raw_ptr());
                self.start.unwrap().take()
            }
            None => {
                tail.clone()
            }
        }
    }


}

fn func_cons(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<Any>(0, obj);
    let tail = vm::refer_arg::<List>(1, obj);

    let list = list::List::alloc(&v.reach(obj), &tail.reach(obj), obj)?;
    Ok(list.into_value())
}

fn func_list(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    if num_rest == 0 {
        Ok(list::List::nil().make().into_value())

    } else {
        let mut builder = ListBuilder::new(obj);
        let mut last_hasreply_index = 0;

        for index in 0 .. num_rest {
            let v = vm::refer_rest_arg::<Any>(0, index, obj);
            let has_reply = value::has_replytype(&v);

            builder.push(&v.reach(obj), obj)?;

            if  has_reply {
                last_hasreply_index = builder.len();
            }
        }

        let mut list = builder.get();

        //Replyを持つセルの前方のリストに対してフラグを立てたいので、
        //一番最初のセルにだけReplyを持っている場合は何もしない。
        if last_hasreply_index != 0 {
            list.set_has_reply_flag(last_hasreply_index - 1);
        }

        Ok(list.into_value())
    }
}

fn func_is_list(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg(0, obj);
    if v.is_type(list::List::typeinfo()) {
        Ok(v.clone())
    } else {
        Ok(bool::Bool::false_().into_ref().into_value())
    }
}

fn func_list_len(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<List>(0, obj);

    let num = number::make_integer(v.as_ref().count() as i64, obj)?;
    Ok(num)
}

fn func_list_ref(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = vm::refer_arg::<List>(0, obj);
    let index = vm::refer_arg::<number::Integer>(1, obj);

    let index = index.as_ref().get() as usize;

    Ok(v.as_ref().get(index).clone())
}

fn func_append(num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    if num_rest == 0 {
        Ok(list::List::nil().make().into_value())

    } else {
        let mut builder = ListBuilder::new(obj);
        let mut last_hasreply_index = 0;
        for index in 0 .. num_rest -1 {
            let list = vm::refer_rest_arg::<List>(0, index, obj).reach(obj);
            for v in list.iter(obj) {
                let has_reply = value::has_replytype(&v);
                builder.push(&v.reach(obj), obj)?;

                if  has_reply {
                    last_hasreply_index = builder.len();
                }
            }
        }

        let last = vm::refer_rest_arg::<List>(0, num_rest - 1, obj);
        if value::has_replytype(&last) {
            last_hasreply_index = num_rest - 1;
        }
        let mut list = builder.append_get(&last);

        //Replyを持つセルの前方のリストに対してフラグを立てたいので、
        //一番最初のセルにだけReplyを持っている場合は何もしない。
        if last_hasreply_index != 0 {
            list.set_has_reply_flag(last_hasreply_index - 1);
        }

        Ok(list.into_value())
    }
}

static FUNC_CONS: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("cons", func_cons,
        Parameter::new(&[
            Param::new_no_force("head", ParamKind::Require, Any::typeinfo()),
            Param::new_no_force("tail", ParamKind::Require, list::List::typeinfo()),
            ])
        )
    )
});

static FUNC_LIST: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list", func_list,
        Parameter::new(&[
            Param::new_no_force("values", ParamKind::Rest, Any::typeinfo()),
            ])
        )
    )
});

static FUNC_IS_LIST: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list?", func_is_list,
        Parameter::new(&[
            Param::new_no_force("x", ParamKind::Require, Any::typeinfo()),
            ])
        )
    )
});

static FUNC_LIST_LEN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list-len", func_list_len,
        Parameter::new(&[
            Param::new_no_force("list", ParamKind::Require, List::typeinfo()),
            ])
        )
    )
});

static FUNC_LIST_REF: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("list-ref", func_list_ref,
        Parameter::new(&[
            Param::new_no_force("list", ParamKind::Require, List::typeinfo()),
            Param::new("index", ParamKind::Require, number::Integer::typeinfo()),
            ])
        )
    )
});

static FUNC_APPEND: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("append", func_append,
        Parameter::new(&[
            Param::new("list", ParamKind::Rest, list::List::typeinfo()),
            ])
        )
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("cons", &Ref::new(&FUNC_CONS.value));
    obj.define_global_value("list", &Ref::new(&FUNC_LIST.value));
    obj.define_global_value("list?", &Ref::new(&FUNC_IS_LIST.value));
    obj.define_global_value("list-len", &Ref::new(&FUNC_LIST_LEN.value));
    obj.define_global_value("list-ref", &Ref::new(&FUNC_LIST_REF.value));
    obj.define_global_value("append", &Ref::new(&FUNC_APPEND.value));
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
    use crate::eval::exec;
    use crate::value::*;
    use crate::object;
    use super::*;

    #[test]
    fn test_builder() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let builder = ListBuilder::new(obj);
            let result = builder.get();

            let ans = list::List::nil();

            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let mut builder = ListBuilder::new(obj);

            builder.push(&number::make_integer(1, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(2, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(3, obj).unwrap().reach(obj), obj).unwrap();
            let result = builder.get().capture(obj);

            let _1 = number::make_integer(1, ans_obj).unwrap().reach(ans_obj);
            let _2 = number::make_integer(2, ans_obj).unwrap().reach(ans_obj);
            let _3 = number::make_integer(3, ans_obj).unwrap().reach(ans_obj);

            let ans = list::List::nil();
            let ans = list::List::alloc(&_3, &ans, ans_obj).unwrap().reach(obj);
            let ans = list::List::alloc(&_2, &ans, ans_obj).unwrap().reach(obj);
            let ans = list::List::alloc(&_1, &ans, ans_obj).unwrap().reach(obj);

            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }

    #[test]
    fn test_construct() {
        let mut standalone = crate::object::new_object();

        let ans = {
            let obj = standalone.mut_object();
            let mut builder = ListBuilder::new(obj);

            builder.push(&number::make_integer(1, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(2, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(3, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(4, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(5, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(6, obj).unwrap().reach(obj), obj).unwrap();
            builder.push(&number::make_integer(7, obj).unwrap().reach(obj), obj).unwrap();
            builder.get().capture(obj)
        };

        {
            let program = "(let obj (spawn))";
            let new_obj_ref = exec::<value::object_ref::ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //操作対象のオブジェクトを新しく作成したオブジェクトに切り替える
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(def-recv @n n)";
            exec::<Any>(program, standalone.mut_object());

            //操作対象のオブジェクトを最初のオブジェクトに戻す
            standalone = object::return_object_switch(standalone).unwrap();
        }

        {
            let program = "(list)";
            let result = exec::<List>(program, standalone.mut_object());
            assert!(result.as_ref().is_nil());

            let program = "(list 1 2 3 4 5 6 7)";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (list 1 (send obj 2) 3 4 (send obj 5) 6 (send obj 7)))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (list (send obj 1) 2 3 4 5 6 7))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (list 1 2 3 4 5 6 (send obj 7)))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = "(append)";
            let result = exec::<List>(program, standalone.mut_object());
            assert!(result.as_ref().is_nil());

            let program = "(append '(1 2) '(3 4) '(5 6 7))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(append '(1 2) '(3 4 5 6 7))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(append '(1 2 3 4 5 6 7))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (append (list (send obj 1) 2) '(3 4 5 6 7)))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (append (list 1 (send obj 2)) (list 3 (send obj 4)) '(5 6 7)))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (append (list 1 (send obj 2)) (list 3 (send obj 4)) (list 5 6 (send obj 7))))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (append (list (send obj 1) 2) '(3 4) '(5 6 7)))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());

            let program = "(force (append '(1 2) '(3 4) (list 5 6 (send obj 7))))";
            let result = exec::<List>(program, standalone.mut_object());
            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }

}