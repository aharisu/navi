use crate::err::*;
use crate::value::*;
use crate::value::app::{Parameter, ParamKind, Param};
use crate::vm;
use crate::object::{self, Object};
use crate::object::mailbox::{MailBox, ReplyToken, MessageKind};
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

    pub fn recv_message(&self, msg: MessageKind, reply_to_mailbox: Arc<Mutex<MailBox>>) -> Result<ReplyToken, OutOfMemory> {
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
        write!(f, "#Object:{}", self.object_id)
    }
}

impl Debug for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#Object:{}", self.object_id)
    }
}

fn func_spawn(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    if let Some(target_obj) = vm::refer_arg::<Any>(0, obj).try_cast::<ObjectRef>() {
        let target_obj = target_obj.clone().reach(obj);
        let message = MessageKind::Duplicate;

        obj.send_message(&target_obj, message)

    } else {
        let standalone = object::new_object();

        let id = standalone.object().id();

        //Object?????????????????????????????????????????????????????????
        //Object??????????????????????????????MailBox?????????
        let mailbox = object::Object::register_scheduler(standalone);

        let objectref = ObjectRef::alloc(id, mailbox, obj)?;
        Ok(objectref.into_value())
    }
}

fn func_send(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let target_obj = vm::refer_arg::<ObjectRef>(0, obj).reach(obj);

    //?????????Reachable??????????????????????????????message???alloc?????????????????????????????????Ref?????????????????????
    let message = vm::refer_arg::<Any>(1, obj);
    let message = MessageKind::Message(message);

    obj.send_message(&target_obj, message)
}

static FUNC_SPAWN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("spawn", func_spawn,
            Parameter::new(&[
            Param::new("object", ParamKind::Optional, ObjectRef::typeinfo()),
            ])
        )
    )
});

static FUNC_SEND: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("send", func_send,
            Parameter::new(&[
            Param::new("object", ParamKind::Require, ObjectRef::typeinfo()),
            Param::new("message", ParamKind::Require, Any::typeinfo()),
            ])
        )
    )
});


pub fn register_global(obj: &mut Object) {
    obj.define_global_value("spawn", &Ref::new(&FUNC_SPAWN.value));
    obj.define_global_value("send", &Ref::new(&FUNC_SEND.value));
}

#[cfg(test)]
mod tests {
    use crate::eval::exec;

    use super::*;

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
            let new_obj_ref = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //?????????????????????????????????????????????????????????????????????????????????????????????
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(def-recv 1 10)";
            exec::<Any>(program, standalone.mut_object());

            let program = "(def-recv 2 20)";
            exec::<Any>(program, standalone.mut_object());

            let program = "(def-recv 3 30)";
            exec::<Any>(program, standalone.mut_object());

            let program = "(def-recv (@a @b) (+ a b))";
            exec::<Any>(program, standalone.mut_object());

            //????????????????????????????????????????????????????????????????????????
            standalone = object::return_object_switch(standalone).unwrap();
        }

        {
            let program = "(send obj 1)";
            let mut ans = exec::<reply::Reply>(program, standalone.mut_object()).capture(standalone.mut_object());
            let ans = get_reply_value(&mut ans, standalone.mut_object()).unwrap();
            assert!(ans.is::<number::Integer>());
            assert_eq!(number::get_integer(&ans), 10);

            //send???????????????Reply???
            let program = "(let a (send obj 2))";
            let ans = exec::<Any>(program, standalone.mut_object());
            assert!( ans.is::<reply::Reply>());

            //force??????????????????Reply????????????????????????????????????
            let program = "(force a)";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);

            //Reply??????force?????????????????????????????????????????????????????????????????????
            let program = "(+ (send obj 3) 1)";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 31);

            let program = "(force (send obj '(1 2)))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 3);

            let program = "(force (send obj 4))";
            let ans = exec::<bool::Bool>(program, standalone.mut_object());
            assert!(ans.as_ref().is_false());
        }

        {
            //List???Reply????????????????????????????????????
            let program = "(let l (cons (send obj 1) (cons 1 '())))";
            let ans = exec::<list::List>(program, standalone.mut_object());
            assert!(ans.as_ref().head().is::<reply::Reply>());
            assert!(ans.as_ref().tail().as_ref().head().is::<number::Integer>());

            //???????????????Reply?????????
            let program = "(list-ref l 0)";
            let ans = exec::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (list-ref l 0))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 10);

            let program = "(list-ref l 1)";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 1);
        }

        {
            //?????????Reply????????????????????????????????????
            let program = "(let a [(send obj 1) (send obj 2)])";
            exec::<Any>(program, standalone.mut_object());

            let program = "(array-ref a 0)";
            let ans = exec::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (array-ref a 1))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

        {
            //????????????Reply????????????????????????????????????
            let program = "(let t {(send obj 1) (send obj 2)})";
            exec::<Any>(program, standalone.mut_object());

            let program = "(tuple-ref t 0)";
            let ans = exec::<Any>(program, standalone.mut_object());
            assert!(ans.as_ref().is::<reply::Reply>());

            let program = "(force (tuple-ref t 1))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

    }

    #[test]
    fn test_remove_ref() {
        let mut standalone = object::new_object();

        {
            let program = "(let obj (spawn))";
            let new_obj_ref = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //?????????????????????????????????????????????????????????????????????????????????????????????
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(let obj2 (spawn))";
            let new_obj_ref = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            let program = "(def-recv 1 (send obj2 2))";
            exec::<Any>(program, standalone.mut_object());

            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();
            let program = "(def-recv 2 20)";
            exec::<Any>(program, standalone.mut_object());

            //????????????????????????????????????????????????????????????????????????
            standalone = object::return_object_switch(standalone).unwrap();
            standalone = object::return_object_switch(standalone).unwrap();
        }

        {
            //send???????????????Reply???
            let program = "(let a (send obj 1))";
            let ans = exec::<Any>(program, standalone.mut_object());

            let program = "(let obj true)";
            exec::<Any>(program, standalone.mut_object());
            assert!( ans.is::<reply::Reply>());

            //force??????????????????Reply????????????????????????????????????
            let program = "(force a)";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 20);
        }

    }

    #[test]
    fn test_remove_reply() {
        let mut standalone = object::new_object();

        {
            let program = "(let obj (spawn))";
            let new_obj_ref = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //?????????????????????????????????????????????????????????????????????????????????????????????
            standalone = object::object_switch(standalone, new_obj_ref.as_ref()).unwrap();

            let program = "(def-recv {:add-one @n} (+ n 1))";
            exec::<Any>(program, standalone.mut_object());

            standalone = object::return_object_switch(standalone).unwrap();

            //send?????????????????????????????????
            let program = "(send obj {:add-one 5})";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());
            //GC???????????????????????????????????????Reply??????????????????
            standalone.mut_object().do_gc();

            //obj??????????????????????????????????????????
            std::thread::sleep(std::time::Duration::from_millis(1000));

            //???????????????????????????????????????????????????????????????
            let count_resultbox = {
                let mailbox = standalone.mailbox().lock().unwrap();
                mailbox.count_resultbox()
            };
            assert_eq!(count_resultbox, 0);

        }
    }

    #[test]
    fn test_dup() {
        let mut standalone = object::new_object();

        {
            let program = "(let obj1 (spawn))";
            let obj1 = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            //?????????????????????????????????????????????????????????????????????????????????????????????
            standalone = object::object_switch(standalone, obj1.as_ref()).unwrap();

            let program = "(let global 1)";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());

            let program = "(def-recv :global global)";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());

            //????????????????????????????????????????????????????????????????????????
            standalone = object::return_object_switch(standalone).unwrap();

            let program = "(force (send obj1 :global))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 1);
        }

        {
            //obj1??????????????????
            let program = "(let obj2 (force (spawn obj1)))";
            let obj2 = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            let program = "(force (send obj2 :global))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 1);

            //?????????????????????????????????????????????????????????????????????????????????????????????
            standalone = object::object_switch(standalone, obj2.as_ref()).unwrap();

            let program = "(let global 2)";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());

            //????????????????????????????????????????????????????????????????????????
            standalone = object::return_object_switch(standalone).unwrap();

            let program = "(force (send obj1 :global))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 1);

            let program = "(force (send obj2 :global))";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 2);
        }

        {
            let program = "obj1";
            let obj1 = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            standalone = object::object_switch(standalone, obj1.as_ref()).unwrap();

            let program = "(let obj3 (spawn))";
            let obj3 = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            standalone = object::object_switch(standalone, obj3.as_ref()).unwrap();
            let program = "(def-recv :hoge 100)";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());
            standalone = object::return_object_switch(standalone).unwrap();

            let program = "(let hoge (send obj3 :hoge))";
            exec::<Any>(program, standalone.mut_object()).capture(standalone.mut_object());
            standalone = object::return_object_switch(standalone).unwrap();

            let program = "(let obj4 (force (spawn obj1)))";
            let obj4 = exec::<ObjectRef>(program, standalone.mut_object()).capture(standalone.mut_object());

            standalone = object::object_switch(standalone, obj4.as_ref()).unwrap();

            let program = "hoge";
            let ans = exec::<number::Integer>(program, standalone.mut_object());
            assert_eq!(ans.as_ref().get(), 100);
        }
    }

}