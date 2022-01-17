use crate::err::*;
use crate::value::*;
use crate::vm;
use crate::object::{self, Object};
use crate::object::mailbox::{MailBox, ReplyToken};
use std::fmt::{self, Debug, Display};
use std::sync::{Mutex, Arc};


pub struct ObjectRef {
    object_id: usize,
    mailbox: Arc<Mutex<MailBox>>,
}

static OBJECT_TYPEINFO : TypeInfo = new_typeinfo!(
    ObjectRef,
    "Object",
    std::mem::size_of::<ObjectRef>(),
    None,
    ObjectRef::eq,
    ObjectRef::clone_inner,
    Display::fmt,
    None,
    Some(ObjectRef::finalize),
    None,
    None,
    None,
);

impl NaviType for ObjectRef {
    fn typeinfo() -> &'static TypeInfo {
        &OBJECT_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        let mailbox = self.mailbox.clone();
        Self::alloc(self.object_id, mailbox, allocator)
    }
}

impl ObjectRef {

    pub fn alloc<A: Allocator>(object_id: usize, mailbox: Arc<Mutex<MailBox>>, allocator: &mut A) -> NResult<ObjectRef, OutOfMemory> {
        let ptr = allocator.alloc::<ObjectRef>()?;
        let obj = ObjectRef {
            object_id,
            mailbox,
        };
        unsafe {
            std::ptr::write(ptr.as_ptr(), obj);
        }

        Ok(ptr.into_ref())
    }

    pub fn mailbox(&self) -> Arc<Mutex<MailBox>> {
        Arc::clone(&self.mailbox)
    }

    pub fn recv_message(&self, msg: &Reachable<Any>, reply_to_mailbox: Arc<Mutex<MailBox>>) -> Result<ReplyToken, OutOfMemory> {
        let mut mailbox = self.mailbox.lock().unwrap();
        mailbox.recv_message(msg, reply_to_mailbox)
    }

    fn finalize(&mut self) {
        unsafe {
            std::ptr::drop_in_place(self)
        }
    }

}

impl Eq for ObjectRef {}

impl PartialEq for ObjectRef {
    fn eq(&self, other: &Self) -> bool {
        self.object_id == other.object_id
    }
}

impl Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#Object")
    }
}

impl Debug for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#Object")
    }
}

fn func_spawn(obj: &mut Object) -> NResult<Any, Exception> {
    let standalone = object::new_object();

    let id = standalone.object().id();

    //Objectの所有権と実行権をスケジューラに譲る。
    //Objectとやり取りするためのMailBoxを取得
    let mailbox = object::Object::register_scheduler(standalone);

    let objectref = ObjectRef::alloc(id, mailbox, obj)?;
    Ok(objectref.into_value())
}

fn func_send(obj: &mut Object) -> NResult<Any, Exception> {
    let target_obj = vm::refer_arg::<ObjectRef>(0, obj).reach(obj);
    let message = vm::refer_arg::<Any>(1, obj).reach(obj);

    obj.send_message(&target_obj, &message)
}

static FUNC_SPAWN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("spawn",
            &[],
            func_spawn)
    )
});

static FUNC_SEND: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("send", &[
            Param::new("object", ParamKind::Require, ObjectRef::typeinfo()),
            Param::new("message", ParamKind::Require, Any::typeinfo()),
            ],
            func_send)
    )
});


pub fn register_global(obj: &mut Object) {
    obj.define_global_value("spawn", &Ref::new(&FUNC_SPAWN.value));
    obj.define_global_value("send", &Ref::new(&FUNC_SEND.value));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::*;


    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> Ref<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();
        let sexp = sexp.reach(obj);

        let code = crate::compile::compile(&sexp, obj).unwrap().reach(obj);
        let result = vm::code_execute(&code, vm::WorkTimeLimit::Inf, obj).unwrap();
        //dbg!(result.as_ref());

        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
    }

    fn get_reply_value(reply: &mut Cap<reply::Reply>, obj: &mut Object) -> NResult<Any, Exception> {
        loop {
            match reply::Reply::try_get_reply_value(reply, obj) {
                ResultNone::Ok(result) => {
                    return result;
                }
                ResultNone::Err(_oom) => {
                    panic!("oom")
                }
                ResultNone::None => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
    }

    #[test]
    fn test() {
        let mut standalone = object::new_object();

        {
            let program = "(let obj (spawn))";
            let new_obj_ref = eval::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //操作対象のオブジェクトを新しく作成したオブジェクトに切り替える
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(def-recv 1 10)";
            eval::<Any>(program, standalone.mut_object());

            let program = "(def-recv 2 20)";
            eval::<Any>(program, standalone.mut_object());

            let program = "(def-recv 3 30)";
            eval::<Any>(program, standalone.mut_object());

            let program = "(def-recv (@a @b) (+ a b))";
            eval::<Any>(program, standalone.mut_object());

            //操作対象のオブジェクトを最初のオブジェクトに戻す
            standalone = object::return_object_switch(standalone).unwrap();
        }

        {
            let program = "(send obj 1)";
            let mut ans = eval::<reply::Reply>(program, standalone.mut_object()).capture(standalone.mut_object());
            let ans = get_reply_value(&mut ans, standalone.mut_object()).unwrap();
            assert!(ans.is::<number::Integer>());
            assert_eq!(unsafe { ans.cast_unchecked::<number::Integer>().as_ref().get() }, 10);

            //sendの戻り値はReply型
            let program = "(let a (send obj 2))";
            let ans = eval::<Any>(program, standalone.mut_object());
            assert!( ans.is::<reply::Reply>());

            //forceに通すことでReplyの値を強制的に取得できる
            let program = "(force a)";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);

            //Reply型はforceなしで、そのまま通常の関数に渡すことができる。
            let program = "(+ (send obj 3) 1)";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 31);

            let program = "(force (send obj '(1 2)))";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 3);

            let program = "(force (send obj 4))";
            let ans = eval::<bool::Bool>(program, standalone.mut_object());
            assert!(ans.as_ref().is_false());
        }

        {
            //ListはReply型の値をそのまま受け取る
            let program = "(let l (cons (send obj 1) (cons 1 '())))";
            let ans = eval::<list::List>(program, standalone.mut_object());
            assert!(ans.as_ref().head().is::<reply::Reply>());
            assert!(ans.as_ref().tail().as_ref().head().is::<number::Integer>());

            //値の取得もReplyのまま
            let program = "(list-ref l 0)";
            let ans = eval::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (list-ref l 0))";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 10);

            let program = "(list-ref l 1)";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 1);
        }

        {
            //配列はReply型の値をそのまま受け取る
            let program = "(let a [(send obj 1) (send obj 2)])";
            eval::<Any>(program, standalone.mut_object());

            let program = "(array-ref a 0)";
            let ans = eval::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (array-ref a 1))";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

        {
            //タプルはReply型の値をそのまま受け取る
            let program = "(let t {(send obj 1) (send obj 2)})";
            eval::<Any>(program, standalone.mut_object());

            let program = "(tuple-ref t 0)";
            let ans = eval::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (tuple-ref t 1))";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

    }

    #[test]
    fn test_remove_ref() {
        let mut standalone = object::new_object();

        {
            let program = "(let obj (spawn))";
            let new_obj_ref = eval::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //操作対象のオブジェクトを新しく作成したオブジェクトに切り替える
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(let obj2 (spawn))";
            let new_obj_ref = eval::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            let program = "(def-recv 1 (send obj2 2))";
            eval::<Any>(program, standalone.mut_object());

            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();
            let program = "(def-recv 2 20)";
            eval::<Any>(program, standalone.mut_object());

            //操作対象のオブジェクトを最初のオブジェクトに戻す
            standalone = object::return_object_switch(standalone).unwrap();
            standalone = object::return_object_switch(standalone).unwrap();
        }

        {
            //sendの戻り値はReply型
            let program = "(let a (send obj 1))";
            let ans = eval::<Any>(program, standalone.mut_object());

            let program = "(let obj true)";
            eval::<Any>(program, standalone.mut_object());
            assert!( ans.is::<reply::Reply>());

            //forceに通すことでReplyの値を強制的に取得できる
            let program = "(force a)";
            let ans = eval::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

    }

}