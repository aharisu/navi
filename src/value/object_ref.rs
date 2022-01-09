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
    ObjectRef::is_type,
    Some(ObjectRef::finalize),
    None,
    None,
    None,
);

impl NaviType for ObjectRef {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&OBJECT_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &AnyAllocator) -> FPtr<Self> {
        let mailbox = self.mailbox.clone();
        Self::alloc_inner(self.object_id, mailbox, allocator)
    }
}

impl ObjectRef {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&OBJECT_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<A: Allocator>(allocator: &A) -> FPtr<ObjectRef> {
        let (object_id, mailbox) = object::new_object();

        Self::alloc_inner(object_id, mailbox, allocator)
    }

    fn alloc_inner<A: Allocator>(object_id: usize, mailbox: Arc<Mutex<MailBox>>, allocator: &A) -> FPtr<ObjectRef> {
        let ptr = allocator.alloc::<ObjectRef>();
        let obj = ObjectRef {
            object_id,
            mailbox,
        };
        unsafe {
            std::ptr::write(ptr.as_ptr(), obj);
        }

        ptr.into_fptr()
    }

    pub fn recv_message(&self, msg: &Reachable<Value>, reply_to_mailbox: Arc<Mutex<MailBox>>) -> ReplyToken {
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

fn func_spawn(obj: &mut Object) -> FPtr<Value> {
    ObjectRef::alloc(obj).into_value()
}

fn func_send(obj: &mut Object) -> FPtr<Value> {
    let target_obj = vm::refer_arg::<ObjectRef>(0, obj).reach(obj);
    let message = vm::refer_arg::<Value>(1, obj).reach(obj);

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
            Param::new("message", ParamKind::Require, Value::typeinfo()),
            ],
            func_send)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("spawn", &FUNC_SPAWN.value);
    obj.define_global_value("send", &FUNC_SEND.value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::*;


    fn eval<T: NaviType>(program: &str, obj: &mut Object) -> FPtr<T> {
        let mut reader = Reader::new(program.chars().peekable());
        let result = crate::read::read(&mut reader, obj);
        assert!(result.is_ok());
        let sexp = result.unwrap();

        let sexp = sexp.reach(obj);
        let result = crate::eval::eval(&sexp, obj);
        let result = result.try_cast::<T>();
        assert!(result.is_some());

        result.unwrap().clone()
    }

    #[test]
    fn test() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;

        {
            /*
            let program = "(let obj (spawn))";
            let new_obj_ref = eval::<ObjectRef>(program, obj).capture(obj);

            let new_obj = unsafe { new_obj_ref.as_ref().get() };

            let program = "(def-recv 1 10)";
            eval::<Value>(program, new_obj);

            let program = "(def-recv 2 20)";
            eval::<Value>(program, new_obj);

            let program = "(def-recv 3 30)";
            eval::<Value>(program, new_obj);

            let program = "(send obj 1)";
            let ans = eval::<number::Integer>(program, obj);
            assert_eq!(ans.as_ref().get(), 10);

            let program = "(send obj 2)";
            let ans = eval::<number::Integer>(program, obj);
            assert_eq!(ans.as_ref().get(), 20);

            let program = "(send obj 3)";
            let ans = eval::<number::Integer>(program, obj);
            assert_eq!(ans.as_ref().get(), 30);

            let program = "(send obj 4)";
            let ans = eval::<bool::Bool>(program, obj);
            assert!(ans.as_ref().is_false());
            */
        }
    }
}