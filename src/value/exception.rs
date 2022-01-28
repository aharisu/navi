use crate::value::*;
use crate::ptr::*;
use crate::err;
use std::fmt::{self, Debug, Display};


pub struct Exception {
    err: err::Exception
}

static EXCEPTION_TYPEINFO: TypeInfo = new_typeinfo!(
    Exception,
    "Exception",
    std::mem::size_of::<Exception>(),
    None,
    Exception::eq,
    Exception::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    Some(Exception::child_traversal),
    None,
    None,
);

impl NaviType for Exception {
    fn typeinfo() -> &'static TypeInfo {
        &EXCEPTION_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        let err = unsafe { self.err.value_clone_gcunsafe(allocator) }?;
        Self::alloc(err, allocator)
    }
}

impl Exception {

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        self.err.for_each_alived_value(arg, callback);
    }

    pub fn alloc<A: Allocator>(err: err::Exception, allocator: &mut A) -> NResult<Exception, OutOfMemory> {
        let ptr = allocator.alloc::<Exception>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Exception {
                err: err,
            });
        }

        Ok(ptr.into_ref())
    }

}

impl Eq for Exception {}

impl PartialEq for Exception {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

fn display(this: &Exception, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    std::fmt::Display::fmt(this, f)
}

impl Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}

impl Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}