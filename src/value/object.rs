use std::rc::Rc;

use crate::value::*;
use crate::context::Context;
use std::fmt::{self, Debug, Display};


pub struct Object {
    handle: Rc<Context>,
}

static OBJECT_TYPEINFO : TypeInfo = new_typeinfo!(
    Object,
    "Object",
    Object::eq,
    Display::fmt,
    Object::is_type,
    Some(Object::finalize),
    None,
    None,
);

impl NaviType for Object {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&OBJECT_TYPEINFO as *const TypeInfo)
    }
}

impl Object {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&OBJECT_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(ctx: &mut Context) -> FPtr<Object> {
        let ptr = ctx.alloc::<Object>();

        let mut ctx = Context::new();
        ctx.register_core_global();
        let obj = Object {
            handle: Rc::new(ctx)
        };

        unsafe {
            std::ptr::write(ptr.as_ptr(), obj);
        }

        ptr.into_fptr()
    }

    pub fn get(&mut self) -> &mut Context {
        Rc::get_mut(&mut self.handle).unwrap()
    }

    fn finalize(&mut self) {
        unsafe {
            std::ptr::drop_in_place(self)
        }
    }

}

impl Eq for Object {}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#Object")
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#Object")
    }
}

fn func_spawn(args: &RPtr<array::Array>, ctx: &mut Context) -> FPtr<Value> {
    Object::alloc(ctx).into_value()
}

static FUNC_SPAWN: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("spawn",
            &[],
            func_spawn)
    )
});

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("spawn", &RPtr::new(&FUNC_SPAWN.value as *const Func as *mut Func).into_value());
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{let_cap, new_cap, value};
    use crate::value::*;
    use crate::context::*;
    use crate::ptr::*;

    #[test]
    fn hgoe() {
        let mut ctx = Context::new();
        let ctx = &mut ctx;

        ctx.register_core_global();

        {
            let_cap!(obj, Object::alloc(ctx), ctx);
            let ctx = obj.as_mut().get();

        }
    }
}