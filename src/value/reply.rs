use crate::object::mailbox::ReplyToken;
use crate::ptr::*;
use crate::err::*;
use crate::value::*;

use std::fmt::{Debug, Display};
use std::sync::{Arc, Mutex};

//TODO 返信をメールボックスから受け取る前にReplyの値が削除されてしまうと
//一生メールボックス内に返信が残り続けてしまう。どうにかして解決しないと。

pub struct Reply {
    reply_token: ReplyToken,

    reply_value: Option<NResult<Any, Exception>>,

    myself_mailbox: Option<Arc<Mutex<crate::object::mailbox::MailBox>>>,
    dest_mailbox: Option<Arc<Mutex<crate::object::mailbox::MailBox>>>,
}

static REPLY_TYPEINFO : TypeInfo = new_typeinfo!(
    Reply,
    "Reply",
    std::mem::size_of::<Reply>(),
    None,
    Reply::eq,
    Reply::clone_inner,
    Display::fmt,
    None,
    Some(Reply::finalize),
    None,
    Some(Reply::child_traversal),
    Some(Reply::_check_reply_dummy),
);

impl NaviType for Reply {
    fn typeinfo() -> &'static TypeInfo {
        &REPLY_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        unreachable!()
    }
}

impl Reply {
    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        match self.reply_value.as_mut() {
            Some(value) => {
                match value {
                    Ok(v) => {
                        callback(v, arg);
                    }
                    Err(err) => {
                        err.for_each_alived_value(arg, callback);
                    }
                }

            },
            None => { }
        }
    }

    pub fn try_get_reply_value(cap: &mut Cap<Reply>, obj: &mut Object) -> ResultNone<NResult<Any, Exception>, OutOfMemory> {
        match Self::check_reply(cap, obj) {
            Ok(has_reply) => {
                if has_reply {
                    ResultNone::Ok(cap.as_ref().reply_value.as_ref().unwrap().clone())

                } else {
                    ResultNone::None
                }

            }
            Err(oom) => {
                ResultNone::Err(oom)
            }
        }
    }

    fn check_reply(cap: &mut Cap<Reply>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        if cap.as_ref().reply_value.is_some() {
            Ok(true)
        } else {
            //自分自身のメールボックスのロックを取得
            //※メールボックスから取得した値をObject内のヒープにcloneするまでロックを保持しておく必要があります。
            let mut mailbox = cap.as_ref().myself_mailbox.as_ref().unwrap().lock().unwrap();
            match mailbox.try_take_reply(cap.as_ref().reply_token) {
                Some(reply) => {
                    match reply {
                        Ok(result) => {
                            let result = unsafe { result.into_reachable() };
                            //valはMailBox内のヒープに確保された値なので、Object内ヒープに値をクローンする
                            let mut allocator = AnyAllocator::Object(obj);
                            match crate::value::value_clone(&result, &mut allocator) {
                                Ok(cloned) => {
                                    cap.as_mut().reply_value = Some(Ok(cloned));
                                    //値を受け取ったので、MailBoxへの参照を削除する
                                    cap.as_mut().myself_mailbox = None;
                                    cap.as_mut().dest_mailbox = None;
                                    return Ok(true);
                                }
                                Err(oom) => {
                                    return Err(oom);
                                }
                            }
                        }
                        Err(err) => {
                            //errはMailBox内のヒープに確保された値なので、Object内ヒープにクローンする
                            let mut allocator = AnyAllocator::Object(obj);
                            match unsafe { err.value_clone_gcunsafe(&mut allocator) } {
                                Ok(cloned) => {
                                    cap.as_mut().reply_value = Some(Err(cloned));
                                    //値を受け取ったので、MailBoxへの参照を削除する
                                    cap.as_mut().myself_mailbox = None;
                                    cap.as_mut().dest_mailbox = None;
                                    return Ok(true);
                                }
                                Err(oom) => {
                                    return Err(oom);
                                }
                            }

                        }
                    }
                }
                None => {
                    return Ok(false);
                }
            }
        }
    }

    fn _check_reply_dummy(_cap: &mut Cap<Reply>, _obj: &mut Object) -> Result<bool, OutOfMemory> {
        //本来この関数が呼ばれることはない。
        //不具合を検出できるようにダミーでパニックするだけの関数を登録できるようにする
        unreachable!()
    }

    pub fn alloc<A: Allocator>(token: ReplyToken
        , myself_mailbox: Arc<Mutex<crate::object::mailbox::MailBox>>
        , dest_mailbox: Arc<Mutex<crate::object::mailbox::MailBox>>
        , allocator: &mut A) -> NResult<Reply, OutOfMemory> {
        let ptr = allocator.alloc::<Reply>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Reply {
                reply_token: token,
                reply_value: None,
                myself_mailbox: Some(myself_mailbox),
                dest_mailbox: Some(dest_mailbox),
            });
        }

        let mut result = ptr.into_ref();
        //Reply型を持つポインタとして目印のフラグを立てる。
        crate::value::set_has_replytype_flag(&mut result);

        Ok(result)
    }

    fn finalize(&mut self) {
        //まだ返信を受け取っていなければ
        if let Some(mailbox) = self.myself_mailbox.as_ref() {
            match mailbox.lock() {
                Ok(mut mailbox) => {
                    //メールボックスに対して、受け取った返信を破棄するように指定しておく
                    mailbox.discard_reply(self.reply_token);
                }
                Err(_) => {
                    //do nothing
                }
            }
        }

        //内部で保持しているArcをデクリメントしないといけないのでDrop処理を実行する
        unsafe {
            std::ptr::drop_in_place(self)
        }
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

fn func_force(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    //関数にわたってきている時点でReplyから実際の値になっているので引数の値をそのまま返す
    Ok(vm::refer_arg::<Any>(0, obj))
}

static FUNC_FORCE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("force",
            &[
            Param::new("v", ParamKind::Require, Any::typeinfo()),
            ],
            func_force)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("force", &Ref::new(&FUNC_FORCE.value));
}
