use crate::{value::*, vm};
use crate::object::{Object, MailBox};
use std::cell::{RefCell, RefMut};
use std::fmt::{self, Debug, Display};
use std::rc::Rc;


pub struct ObjectRef {
    handle: Rc<RefCell<MailBox>>,
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
);

impl NaviType for ObjectRef {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&OBJECT_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, allocator: &AnyAllocator) -> FPtr<Self> {
        //コンテキスト自体はクローンせずに同じ実体を持つRcをクローンする。
        let handle = self.handle.clone();
        Self::alloc_inner(handle, allocator)
    }
}

impl ObjectRef {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&OBJECT_TYPEINFO, other_typeinfo)
    }

    pub fn alloc<A: Allocator>(allocator: &A) -> FPtr<ObjectRef> {
        let new_obj =  Object::new();
        let mailbox = MailBox::new(new_obj);

        Self::alloc_inner(Rc::new(RefCell::new(mailbox)), allocator)
    }

    fn alloc_inner<A: Allocator>(handle: Rc<RefCell<MailBox>>, allocator: &A) -> FPtr<ObjectRef> {
        let ptr = allocator.alloc::<ObjectRef>();
        let obj = ObjectRef {
            handle: handle,
        };
        unsafe {
            std::ptr::write(ptr.as_ptr(), obj);
        }

        ptr.into_fptr()
    }

    /*
    pub unsafe fn get<'a>(&'a self) -> RefMut<'a, MailBox> {


        //かなり行儀が悪いコードだが現在の実装の都合上、直接ポインタを取得して参照を返す
        //Objectシステムをちゃんと作るときにまともな実装を行う。
        //※そもそもObjectをRcで管理しない。
        //オブジェクトはJobスケジューラが管理されて、オブジェクトの実体(所有権)はすべてJobスケジューラが持つ。
        //オブジェクト間で共有されるのはメッセージ送受信のための郵便ポスト(mailbox)のみ。
        let ptr = Rc::as_ptr(&self.handle);
        &mut *(ptr as *mut MailBox)
    }
    */

    pub fn recv(&self, msg: &Reachable<Value>) -> FPtr<Value> {
        //let mut obj = (*self.handle).borrow_mut();
        let mut mailbox = (*self.handle).borrow_mut();
        mailbox.recv(msg)

        /*
        //TODO 自分から自分へのsendの場合はクローンする必要がないので場合分けしたい

        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let msg = crate::value::value_clone(msg, obj);
        let_cap!(msg, msg, obj);

        //メッセージをオブジェクトに対して適用
        obj.apply_message(&msg)
        */
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
        self.handle == other.handle
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
    let target_obj = vm::refer_arg::<ObjectRef>(0, obj);
    let message = vm::refer_arg::<Value>(1, obj);

    target_obj.as_ref().recv(&message.reach(obj))
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
        let mut obj = Object::new();
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