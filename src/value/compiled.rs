use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};

pub struct Code {
    program: Vec<u8>,
    constants: Vec<Ref<Any>>,
}

static CODE_TYPEINFO: TypeInfo = new_typeinfo!(
    Code,
    "Code",
    std::mem::size_of::<Code>(),
    None,
    Code::eq,
    Code::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    Some(Code::child_traversal),
    None,
);

impl NaviType for Code {
    fn typeinfo() -> &'static TypeInfo {
        &CODE_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        unsafe {
            let program = self.program.clone();
            let constants: Result<Vec<_>, _> = self.constants.iter()
                .map(|c| Any::clone_inner(c.as_ref(), allocator))
                .collect()
                ;
            let constants = constants?;

            let ptr = allocator.alloc::<Code>()?;
            std::ptr::write(ptr.as_ptr(), Code {
                program: program,
                constants: constants,
            });

            Ok(ptr.into_ref())
        }
    }
}

impl Code {

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        self.constants.iter_mut().for_each(|v| callback(v, arg));
    }

    pub fn alloc<A: Allocator>(program: Vec<u8>, constants: Vec<Cap<Any>>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<Code>()?;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Self::new(program, constants))
        }

        Ok(ptr.into_ref())
    }

    pub fn new(program: Vec<u8>, constants: Vec<Cap<Any>>) -> Self {
        let constants = constants.into_iter()
            .map(|c| c.take())
            .collect()
            ;
        Code {
            program: program,
            constants: constants,
        }
    }

    pub fn program(&self) -> &[u8] {
        &self.program[..]
    }

    pub fn get_constant(&self, index: usize) -> Ref<Any> {
        self.constants[index].clone()
    }

    pub fn get_constant_slice(&self, start: usize, end: usize) -> &[Ref<Any>] {
        &self.constants[start..end]
    }

}

impl Eq for Code { }

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#code")
    }
}

impl Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#code")
    }
}


#[repr(C)]
pub struct Closure {
    code: Code,
    num_args: usize,
}

static CLOSURE_TYPEINFO: TypeInfo = new_typeinfo!(
    Closure,
    "Closure",
    std::mem::size_of::<Closure>(),
    None,
    Closure::eq,
    Closure::clone_inner,
    Display::fmt,
    Some(Closure::is_type),
    None,
    None,
    Some(Closure::child_traversal),
    None,
);

impl NaviType for Closure {
    fn typeinfo() -> &'static TypeInfo {
        &CLOSURE_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        let program = self.code.program.clone();
        let constants: Result<Vec<_>, _> = self.code.constants.iter()
            .map(|c| Any::clone_inner(c.as_ref(), allocator))
            .collect()
            ;
        let constants = constants?;

        Self::alloc(program, &constants, self.num_args, allocator)
    }
}

impl Closure {

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        self.code.child_traversal(arg, callback);
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &CLOSURE_TYPEINFO == other_typeinfo
        || app::App::typeinfo() == other_typeinfo
    }

    pub fn alloc<A: Allocator>(program: Vec<u8>, constants: &[Ref<Any>], num_args: usize, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<Closure>()?;

        let constants = constants.into_iter()
            .map(|c| c.clone())
            .collect()
            ;

        unsafe {
            std::ptr::write(ptr.as_ptr(), Closure {
                code: Code {
                    program: program,
                    constants: constants,
                },
                num_args: num_args,
            })
        }

        Ok(ptr.into_ref())
    }

    pub fn arg_descriptor(&self) -> usize {
        self.num_args
    }

    pub fn code(&self) -> Ref<Code> {
        //structの先頭にあるCodeフィールドの参照とselfの参照は(ポインタレベルでは)同一視できるはず。
        //Codeへの参照をReachableで確保することでClosure自体もGCされないようにする
        Ref::new(&self.code)
    }
}

impl Eq for Closure { }

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#compiled_closure")
    }
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#compiled_closure")
    }
}
