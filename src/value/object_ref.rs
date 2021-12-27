use crate::value::*;
use crate::object::Object;
use crate::object::context::Context;
use std::fmt::{self, Debug, Display};
use std::rc::Rc;


pub struct ObjectRef {
    handle: Rc<Object>,
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

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //コンテキスト自体はクローンせずに同じ実体を持つRcをクローンする。
        let handle = self.handle.clone();
        Self::alloc_inner(handle, obj)
    }
}

impl ObjectRef {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&OBJECT_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(obj: &mut Object) -> FPtr<ObjectRef> {
        Self::alloc_inner(Rc::new(Object::new()), obj)
    }

    fn alloc_inner(handle: Rc<Object>, obj: &mut Object) -> FPtr<ObjectRef> {
        let ptr = obj.alloc::<ObjectRef>();
        let obj = ObjectRef {
            handle: handle,
        };
        unsafe {
            std::ptr::write(ptr.as_ptr(), obj);
        }

        ptr.into_fptr()
    }

    pub unsafe fn get(&self) -> &mut Object {
        //かなり行儀が悪いコードだが現在の実装の都合上、直接ポインタを取得して参照を返す
        //Objectシステムをちゃんと作るときにまともな実装を行う。
        //※そもそもObjectをRcで管理しない。
        //オブジェクトはJobスケジューラが管理されて、オブジェクトの実体(所有権)はすべてJobスケジューラが持つ。
        //オブジェクト間で共有されるのはメッセージ送受信のための郵便ポスト(mailbox)のみ。
        let ptr = Rc::as_ptr(&self.handle);
        &mut *(ptr as *mut Object)
    }

    pub fn recv(&self, msg: &Reachable<Value>) -> FPtr<Value> {
        //let mut obj = (*self.handle).borrow_mut();
        let obj = unsafe { self.get() };

        obj.recv_message(msg)

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

fn func_spawn(_args: &Reachable<array::Array<Value>>, obj: &mut Object) -> FPtr<Value> {
    ObjectRef::alloc(obj).into_value()
}

fn func_send(args: &Reachable<array::Array<Value>>, obj: &mut Object) -> FPtr<Value> {
    let target_obj = args.as_ref().get(0);
    let target_obj = unsafe { target_obj.cast_unchecked::<ObjectRef>() };
    let message = args.as_ref().get(1);

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

pub fn register_global(obj: &mut Context) {
    obj.define_value("spawn", Reachable::new_static(&FUNC_SPAWN.value).cast_value());
    obj.define_value("send", Reachable::new_static(&FUNC_SEND.value).cast_value());
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
            let program = "(def obj (spawn))";
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
        }
    }
}