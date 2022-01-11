#[macro_export]
macro_rules! new_typeinfo {
    ($t:ty, $name:expr, $fixed_size:expr, $variable_size_func:expr, $eq_func:expr, $clone_func:expr, $print_func:expr, $is_type_func:expr, $finalize_func:expr, $is_comparable_func:expr, $child_traversal_func:expr, $check_reply_func:expr, ) => {
        TypeInfo {
            name: $name,
            fixed_size: $fixed_size,
            variable_size_func: match $variable_size_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&$t) -> usize, fn(&Any) -> usize>(func) }),
                None => None
            },
            eq_func: unsafe { std::mem::transmute::<fn(&$t, &$t) -> bool, fn(&Any, &Any) -> bool>($eq_func) },
            clone_func: unsafe { std::mem::transmute::<fn(&$t, &mut crate::object::AnyAllocator) -> Ref<$t>, fn(&Any, &mut crate::object::AnyAllocator) -> Ref<Any>>($clone_func) },
            print_func: unsafe { std::mem::transmute::<fn(&$t, &mut std::fmt::Formatter<'_>) -> std::fmt::Result, fn(&Any, &mut std::fmt::Formatter<'_>) -> std::fmt::Result>($print_func) },
            is_type_func: $is_type_func,
            finalize: match $finalize_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&mut $t), fn(&mut Any)>(func) }),
                None => None
            },
            is_comparable_func: match $is_comparable_func {
                Some(func) => Some(func),
                None => None
             },
            child_traversal_func: match $child_traversal_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&mut $t, *mut u8, fn(&mut Ref<Any>, *mut u8)), fn(&mut Any, *mut u8, fn(&mut Ref<Any>, *mut u8))>(func) }),
                None => None
             },
            check_reply_func: match $check_reply_func {
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&mut Cap<$t>, &mut crate::object::Object) -> bool, fn(&mut Cap<Any>, &mut crate::object::Object) -> bool>(func) }),
                None => None
             },
        }
    };
}

pub mod any;
pub mod array;
pub mod bool;
pub mod compiled;
pub mod list;
pub mod number;
pub mod string;
pub mod symbol;
pub mod keyword;
pub mod func;
pub mod syntax;
pub mod tuple;
pub mod object_ref;
pub mod iform;
pub mod reply;


use crate::value::any::Any;
use crate::object::{Object, Allocator, AnyAllocator};
use crate::object::mm::{self, GCAllocationStruct, ptr_to_usize, usize_to_ptr};
use crate::util::non_null_const::*;
use crate::{ptr::*, vm};

use crate::value::func::*;
use once_cell::sync::Lazy;


//xxxx xxxx xxxx xx0r pointer value(r = 1: has Reply type. r = 0: do not have Reply type.)
//xxxx xxxx xxxx x110 fixnum
//xxxx xxxx xxx1 0010 tagged value


// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b0001_0010;


const fn tagged_value(tag: usize) -> usize {
    (tag << 16) | IMMIDATE_TAGGED_VALUE
}

pub(crate) const IMMIDATE_GC_COPIED: usize = tagged_value(0); //GC内でだけ使用する特別な値。
pub(crate) const IMMIDATE_NIL: usize = tagged_value(1);
pub(crate) const IMMIDATE_TRUE: usize = tagged_value(2);
pub(crate) const IMMIDATE_FALSE: usize = tagged_value(3);
pub(crate) const IMMIDATE_UNIT: usize = tagged_value(4);
pub(crate) const IMMIDATE_MATCHFAIL: usize = tagged_value(5);

#[derive(PartialEq)]
enum PtrKind {
    Ptr,
    Nil,
    True,
    False,
    Unit,
    MatchFail
}

fn pointer_kind<T>(ptr: *const T) -> PtrKind {
    let value = mm::ptr_to_usize(ptr);

    //下位2bitが00なら生ポインタ
    if value & 0b11 == 0 {
        PtrKind::Ptr
    } else {
        //残りは下位16bitで判断する
        match value &0xFFFF {
            IMMIDATE_TAGGED_VALUE => {
                match value {
                    IMMIDATE_NIL => PtrKind::Nil,
                    IMMIDATE_TRUE => PtrKind::True,
                    IMMIDATE_FALSE => PtrKind::False,
                    IMMIDATE_UNIT => PtrKind::Unit,
                    IMMIDATE_MATCHFAIL => PtrKind::MatchFail,
                    _ => panic!("invalid tagged value"),
                }
            }
            _ => panic!("invalid pointer: {}", value),
        }
    }
}

pub fn value_is_pointer(v: &Any) -> bool {
    pointer_kind(v as *const Any) == PtrKind::Ptr
}

pub fn value_clone<T: NaviType>(v: &Reachable<T>, allocator: &mut AnyAllocator) -> Ref<T> {
    //クローンを行う値のトータルのサイズを計測
    //リストや配列など内部に値を保持している場合は再帰的にすべての値のサイズも計測されている
    let total_size = mm::Heap::calc_total_size(v.cast_value().as_ref());
    //事前にクローンを行うために必要なメモリスペースを確保する
    allocator.force_allocation_space(total_size);

    NaviType::clone_inner(v.as_ref(), allocator)
}

pub fn get_typeinfo<T: NaviType>(this: &T) -> NonNullConst<TypeInfo>{
    let ptr = this as *const T;
    match pointer_kind(ptr) {
        PtrKind::Nil => {
            crate::value::list::List::typeinfo()
        }
        PtrKind::True | PtrKind::False => {
            crate::value::bool::Bool::typeinfo()
        }
        PtrKind::Unit => {
            crate::value::tuple::Tuple::typeinfo()
        }
        PtrKind::MatchFail => {
            crate::value::syntax::r#match::MatchFail::typeinfo()
        }
        PtrKind::Ptr => {
            mm::get_typeinfo(ptr)
        }
    }
}

pub fn check_reply(cap: &mut Cap<Any>, obj: &mut Object) -> bool {

    if let Some(reply) = cap.try_cast_mut::<reply::Reply>() {
        if let Some(result) = reply::Reply::try_get_reply_value(reply, obj) {
            //Replyオブジェクトを指していたポインタを、返信の結果を指すように上書きする
            cap.update_pointer(result);

            // OK!!
            true

        } else {
            //Replyがまだ返信を受け取っていなかったのでfalseを返す
            false
        }
    } else {
        let typeinfo = get_typeinfo(cap.as_ref());
        let typeinfo = unsafe { typeinfo.as_ref() };
        match typeinfo.check_reply_func {
            Some(func) => func(cap, obj),
            None => true,
        }
    }
}

pub fn has_replytype<T: NaviType>(ptr: &Ref<T>) -> bool {
    let value = ptr_to_usize(ptr.raw_ptr());
    //最下位bitが1ならReply値、もしくは内部でReplyを持つ値。
    value & 1 == 1
}

pub fn set_has_replytype_flag<T: NaviType>(ptr: &mut Ref<T>) {
    //最下位bitに1を立てる
    ptr.update_pointer(usize_to_ptr(ptr_to_usize(ptr.raw_ptr()) | 1));
}

pub fn clear_has_replytype_flag<T: NaviType>(ptr: &mut Ref<T>) {
    //最下位bitの1を降ろす
    ptr.update_pointer(usize_to_ptr(ptr_to_usize(ptr.raw_ptr()) & !1));
}

#[inline]
pub fn ptr_value<T: NaviType>(ptr: &Ref<T>) -> *mut T {
    //最下位bitをマスクしてからポインタを参照する
    usize_to_ptr::<T>(ptr_to_usize(ptr.raw_ptr()) & !1)
}

#[inline]
pub fn refer_value<'a, 'b, T: NaviType>(ptr: &'a Ref<T>) -> &'b T {
    unsafe { &*ptr_value(ptr) }
}

#[inline]
pub fn mut_refer_value<'a, 'b, T: NaviType>(ptr: &'a mut Ref<T>) -> &'b mut T {
    //最下位bitをマスクしてからポインタを参照する
    unsafe { &mut *ptr_value(ptr) }
}

pub trait NaviType: PartialEq + std::fmt::Debug + std::fmt::Display {
    fn typeinfo() -> NonNullConst<TypeInfo>;
    fn clone_inner(&self, allocator: &mut AnyAllocator) -> Ref<Self>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub fixed_size: usize,
    pub variable_size_func: Option<fn(&Any) -> usize>,
    pub eq_func: fn(&Any, &Any) -> bool,
    pub clone_func: fn(&Any, &mut AnyAllocator) -> Ref<Any>,
    pub print_func: fn(&Any, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: fn(&TypeInfo) -> bool,
    pub finalize: Option<fn(&mut Any)>,
    pub is_comparable_func: Option<fn(&TypeInfo) -> bool>,
    pub child_traversal_func: Option<fn(&mut Any, *mut u8, fn(&mut Ref<Any>, *mut u8))>,
    pub check_reply_func: Option<fn(&mut Cap<Any>, &mut Object) -> bool>,
}

#[cfg(test)]
mod tests {
    use crate::value::*;

    #[test]
    fn is_type() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        //int
        let v = number::Integer::alloc(10, obj).into_value();
        assert!(v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //real
        let v = number::Real::alloc(3.14, obj).into_value();
        assert!(!v.as_ref().is::<number::Integer>());
        assert!(v.as_ref().is::<number::Real>());
        assert!(v.as_ref().is::<number::Number>());

        //nil
        let v = list::List::nil().into_value();
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());

        //list
        let item = number::Integer::alloc(10, obj).into_value().reach(obj);
        let v = list::List::alloc(&item, v.try_cast::<list::List>().unwrap(), obj).into_value().reach(obj);
        assert!(v.as_ref().is::<list::List>());
        assert!(!v.as_ref().is::<string::NString>());
    }

}