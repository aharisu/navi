use crate::object::Object;
use crate::value::*;
use crate::ptr::*;
use std::fmt::{Debug, Display};

pub struct Code {
    program: Vec<u8>,
    constants: Vec<FPtr<Value>>,
}

static CODE_TYPEINFO: TypeInfo = new_typeinfo!(
    Code,
    "Code",
    std::mem::size_of::<Code>(),
    None,
    Code::eq,
    Code::clone_inner,
    Display::fmt,
    Code::is_type,
    None,
    None,
    Some(Code::child_traversal),
);

impl NaviType for Code {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&CODE_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        unsafe {
            let program = self.program.clone();
            let constants = self.constants.iter()
                .map(|c| Value::clone_inner(c.as_ref(), obj))
                .collect()
                ;

            let ptr = obj.alloc::<Code>();
            std::ptr::write(ptr.as_ptr(), Code {
                program: program,
                constants: constants,
            });

            ptr.into_fptr()
        }
    }
}

impl Code {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&CODE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        self.constants.iter().for_each(|v| callback(v, arg));
    }

    pub fn alloc(program: Vec<u8>, constants: Vec<Cap<Value>>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<Code>();

        unsafe {
            std::ptr::write(ptr.as_ptr(), Self::new(program, constants))
        }

        ptr.into_fptr()
    }

    pub fn new(program: Vec<u8>, constants: Vec<Cap<Value>>) -> Self {
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

    pub fn get_constant(&self, index: usize) -> FPtr<Value> {
        self.constants[index].clone()
    }

    pub fn get_constant_slice(&self, start: usize, end: usize) -> &[FPtr<Value>] {
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
    Closure::is_type,
    None,
    None,
    Some(Closure::child_traversal),
);

impl NaviType for Closure {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&CLOSURE_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、FPtrをキャプチャせずに扱うことが許されている
        let program = self.code.program.clone();
        let constants:Vec<FPtr<Value>> = self.code.constants.iter()
            .map(|c| Value::clone_inner(c.as_ref(), obj))
            .collect()
            ;

        Self::alloc(program, &constants, self.num_args, obj)
    }
}

impl Closure {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&CLOSURE_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        self.code.child_traversal(arg, callback);
    }

    pub fn alloc(program: Vec<u8>, constants: &[FPtr<Value>], num_args: usize, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<Closure>();

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

        ptr.into_fptr()
    }

    pub fn arg_descriptor(&self) -> usize {
        self.num_args
    }

    pub fn code(&self) -> FPtr<Code> {
        //structの先頭にあるCodeフィールドの参照とselfの参照は(ポインタレベルでは)同一視できるはず。
        //Codeへの参照をReachableで確保することでClosure自体もGCされないようにする
        FPtr::new(&self.code)
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
