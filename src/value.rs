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
            clone_func: unsafe { std::mem::transmute::<fn(&$t, &mut crate::object::AnyAllocator) -> crate::err::NResult<$t, crate::err::OutOfMemory>, fn(&Any, &mut crate::object::AnyAllocator) -> crate::err::NResult<Any, crate::err::OutOfMemory>>($clone_func) },
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
                Some(func) => Some(unsafe { std::mem::transmute::<fn(&mut Cap<$t>, &mut crate::object::Object) -> Result<bool, crate::err::OutOfMemory>, fn(&mut Cap<Any>, &mut crate::object::Object) -> Result<bool, crate::err::OutOfMemory>>(func) }),
                None => None
             },
        }
    };
}

pub mod any;
pub mod app;
pub mod array;
pub mod bool;
pub mod compiled;
pub mod exception;
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


use crate::err::OutOfMemory;
use crate::value::any::Any;
use crate::object::{Object, Allocator, AnyAllocator};
use crate::object::mm::{self, GCAllocationStruct, ptr_to_usize, usize_to_ptr};
use crate::{ptr::*, vm};
use crate::err::*;

use crate::value::func::*;
use once_cell::sync::Lazy;


//xxxx xxxx xxxx xx0r pointer value(r = 1: has Reply type. r = 0: do not have Reply type.)
//xxxx xxxx xxxx x110 fixnum
//xxxx xxxx xxx1 0010 tagged value


pub const IMMIDATE_FIXNUM: usize = 0b110;
pub const FIXNUM_MASK_BITS:usize = 3;

// [tagged value]
// Nil, true, false, ...
const IMMIDATE_TAGGED_VALUE: usize = 0b1_0010;


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
    MatchFail,
    Fixnum,
}

fn pointer_kind<T>(ptr: *const T) -> PtrKind {
    let value = mm::ptr_to_usize(ptr);

    //下位2bitが00なら生ポインタ
    if value & 0b11 == 0 {
        PtrKind::Ptr
    } else if value & 0b111 ==  IMMIDATE_FIXNUM {
        PtrKind::Fixnum
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

pub fn value_clone<T: NaviType>(v: &Reachable<T>, allocator: &mut AnyAllocator) -> NResult<T, OutOfMemory> {
    //クローンを行う値のトータルのサイズを計測
    //リストや配列など内部に値を保持している場合は再帰的にすべての値のサイズも計測されている
    let total_size = mm::Heap::calc_total_size(v.cast_value().as_ref());
    //事前にクローンを行うために必要なメモリスペースを確保する
    allocator.force_allocation_space(total_size)?;

    NaviType::clone_inner(v.as_ref(), allocator)
}

pub fn get_typeinfo<T: NaviType>(this: &T) -> &'static TypeInfo{
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
        PtrKind::Fixnum => {
            crate::value::number::Fixnum::typeinfo()
        }
        PtrKind::Ptr => {
            mm::get_typeinfo(ptr)
        }
    }
}

pub fn get_typename<T: NaviType>(this: &T) -> &'static str {
    let typeinfo = get_typeinfo(this);
    typeinfo.name
}

pub fn check_reply(cap: &mut Cap<Any>, obj: &mut Object) -> Result<bool, OutOfMemory> {
    if let Some(reply) = cap.try_cast_mut::<reply::Reply>() {
        match reply::Reply::try_get_reply_value(reply, obj) {
            ResultNone::Ok(reply) => {
                match reply {
                    Ok(result) => {
                        //Replyオブジェクトを指していたポインタを、返信の結果を指すように上書きする
                        cap.update_pointer(result);
                        //返信があったのでtrueを返す
                        Ok(true)
                    }
                    Err(err) => {
                        //返信があったが、返信の内容がエラーだった。
                        //Exceptionオブジェクトを作成してReplyを上書きする。
                        let exception = exception::Exception::alloc(err, obj)?;
                        cap.update_pointer(exception.into_value());
                        Ok(true)
                    }
                }
            }
            ResultNone::Err(oom) => {
                //返信を受け取る前に、自分自身のOOMが発生した。
                Err(oom)
            }
            ResultNone::None => {
                //まだ返信がないのでfalseを返す
                Ok(false)
            }
        }
    } else {
        let typeinfo = get_typeinfo(cap.as_ref());
        match typeinfo.check_reply_func {
            Some(func) => func(cap, obj),
            //返信を含まない値なので無条件でtrueを返す
            None => Ok(true),
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
    fn typeinfo() -> &'static TypeInfo;
    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory>;
}

#[allow(dead_code)]
pub struct TypeInfo {
    pub name : &'static str,
    pub fixed_size: usize,
    pub variable_size_func: Option<fn(&Any) -> usize>,
    pub eq_func: fn(&Any, &Any) -> bool,
    pub clone_func: fn(&Any, &mut AnyAllocator) -> NResult<Any, OutOfMemory>,
    pub print_func: fn(&Any, &mut std::fmt::Formatter<'_>) -> std::fmt::Result,
    pub is_type_func: Option<fn(&TypeInfo) -> bool>,
    pub finalize: Option<fn(&mut Any)>,
    pub is_comparable_func: Option<fn(&TypeInfo) -> bool>,
    pub child_traversal_func: Option<fn(&mut Any, *mut u8, fn(&mut Ref<Any>, *mut u8))>,
    pub check_reply_func: Option<fn(&mut Cap<Any>, &mut Object) -> Result<bool, OutOfMemory>>,
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool {
        //TypeInfoのインスタンスは常にstaticなライフタイムを持つため、参照の同一性だけで値の同一性を測る
        std::ptr::eq(self, other)
    }
}

impl std::fmt::Debug for TypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}