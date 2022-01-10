use crate::object::mailbox::ReplyToken;
use crate::ptr::*;
use crate::value::*;

use std::fmt::{Debug, Display};

pub struct Reply {
    reply_token: ReplyToken,
    reply_value: Option<FPtr<Value>>,
}

static REPLY_TYPEINFO : TypeInfo = new_typeinfo!(
    Reply,
    "Reply",
    std::mem::size_of::<Reply>(),
    None,
    Reply::eq,
    Reply::clone_inner,
    Display::fmt,
    Reply::is_type,
    None,
    None,
    Some(Reply::child_traversal),
    Some(Reply::_check_reply_dummy),
);

impl NaviType for Reply {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&REPLY_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _allocator: &AnyAllocator) -> FPtr<Self> {
        unreachable!()
    }
}

impl Reply {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&REPLY_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        match self.reply_value.as_ref() {
            Some(value) => {
                callback(value, arg);
            },
            None => { }
        }
    }

    pub fn try_get_reply_value(cap: &mut Cap<Reply>, obj: &mut Object) -> Option<FPtr<Value>> {
        if Self::check_reply(cap, obj) {
            cap.as_ref().reply_value.clone()
        } else {
            None
        }
    }

    fn check_reply(cap: &mut Cap<Reply>, obj: &mut Object) -> bool {
        if cap.as_ref().reply_value.is_some() {
            true
        } else {
            match obj.check_reply(cap.as_ref().reply_token) {
                Some(reply_value) => {
                    cap.as_mut().reply_value = Some(reply_value);
                    true
                },
                None => false
            }
        }
    }

    fn _check_reply_dummy(_cap: &mut Cap<Reply>, _obj: &mut Object) -> bool {
        //本来この関数が呼ばれることはない。
        //不具合を検出できるようにダミーでパニックするだけの関数を登録できるようにする
        unreachable!()
    }

    pub fn alloc<A: Allocator>(token: ReplyToken, allocator: &A) -> FPtr<Reply> {
        let ptr = allocator.alloc::<Reply>();

        unsafe {
            std::ptr::write(ptr.as_ptr(), Reply {
                reply_token: token,
                reply_value: None,
            });
        }

        ptr.into_fptr()
    }
}

impl Eq for Reply { }

impl PartialEq for Reply {
    fn eq(&self, other: &Self) -> bool {
        self.reply_token == other.reply_token
    }
}

fn display(this: &Reply, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "#reply:{:?}", this.reply_token)
}

impl Display for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

impl Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display(self, f)
    }
}

fn func_force(obj: &mut Object) -> FPtr<Value> {
    //関数にわたってきている時点でReplyから実際の値になっているので引数の値をそのまま返す
    vm::refer_arg::<Value>(0, obj)
}

static FUNC_FORCE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("force",
            &[
            Param::new("v", ParamKind::Require, Value::typeinfo()),
            ],
            func_force)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("force", &FUNC_FORCE.value);
}
