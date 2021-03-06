use crate::object::AnyAllocator;
use crate::ptr::*;
use crate::value::{*, self};
use crate::value::any::Any;

#[derive(Debug, Clone)]
pub struct OutOfBounds {
    pub related_exp: Ref<Any>,
    pub index: usize,
}

impl OutOfBounds {
    pub fn new(related_exp: Ref<Any>, index: usize) -> Self {
        OutOfBounds {
            related_exp,
            index,
        }
    }

    unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Self, OutOfMemory> {
        let related = self.related_exp.clone().into_reachable();
        let related_cloned = value::value_clone(&related, allocator)?;

        Ok(Self::new(related_cloned, self.index))
    }

    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        callback(&mut self.related_exp, arg);
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "out of bounds {}: {}", self.index, self.related_exp.as_ref())
    }

}

#[derive(Debug, Clone)]
pub struct TypeMismatch {
    pub found_exp: Ref<Any>,
    pub require_type: &'static TypeInfo ,
}

impl TypeMismatch {
    pub fn new(found_exp: Ref<Any>, require_type: &'static TypeInfo) -> Self {
        TypeMismatch {
            found_exp,
            require_type,
        }
    }

    unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Self, OutOfMemory> {
        let related = self.found_exp.clone().into_reachable();
        let related_cloned = value::value_clone(&related, allocator)?;

        Ok(TypeMismatch::new(related_cloned, self.require_type))
    }

    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        callback(&mut self.found_exp, arg);
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mismatched type.\n  expected:{}\n  found:{}", self.require_type.name, self.found_exp.as_ref())
    }

}

#[derive(Clone, Debug)]
pub struct MalformedFormat {
    pub related_exp: Option<Ref<Any>>,
    pub message: String,
}

impl MalformedFormat {
    pub fn new<T: Into<String>>(related_exp: Option<Ref<Any>>, message: T) -> Self {
        MalformedFormat {
            related_exp: related_exp,
            message: message.into(),
        }
    }

    unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Self, OutOfMemory> {
        match self.related_exp.as_ref() {
            Some(related_exp) => {
                let value = related_exp.clone().into_reachable();
                let cloned_related_exp = value::value_clone(&value, allocator)?;
                Ok(MalformedFormat {
                        related_exp: Some(cloned_related_exp),
                        message: self.message.clone()
                    })
            }
            None => {
                Ok(MalformedFormat {
                        related_exp: None,
                        message: self.message.clone()
                    })
            }
        }
    }

    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        if let Some(v) = self.related_exp.as_mut() {
            callback(v, arg);
        }
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed format.\n  {}", self.message)?;
        if let Some(related_exp) = self.related_exp.as_ref() {
            write!(f, "\n  expression:{}", related_exp.as_ref())?;
        }

        Ok(())
    }

}

#[derive(Debug, Clone)]
pub struct ArgTypeMismatch {
    pub name: String,
    pub arg_index: usize,
    pub found_exp: Ref<Any>,
    pub require_type: &'static TypeInfo ,
}

impl ArgTypeMismatch {
    pub fn new(name: String, arg_index: usize, found_exp: Ref<Any>, require_type: &'static TypeInfo) -> Self {
        ArgTypeMismatch {
            name,
            arg_index,
            found_exp,
            require_type,
        }
    }

    unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Self, OutOfMemory> {
        let related = self.found_exp.clone().into_reachable();
        let related_cloned = value::value_clone(&related, allocator)?;

        Ok(ArgTypeMismatch::new(self.name.clone(), self.arg_index, related_cloned, self.require_type))
    }

    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        callback(&mut self.found_exp, arg);
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mismatched type.\nThe {} argument of function {}.\n  expected:{}\n  found:{}",
            self.arg_index, self.name,
            self.require_type.name, self.found_exp.as_ref())
    }

}

#[derive(Clone, Debug)]
pub struct DisallowContext {}


#[derive(Clone, Debug)]
pub struct OutOfMemory {}

#[derive(Clone, Debug)]
pub struct MySelfObjectDeleted {}

#[derive(Clone, Debug)]
pub struct UnboundVariable {
    pub symbol: Ref<value::symbol::Symbol>,
}

impl UnboundVariable {
    pub fn new(symbol: Ref<value::symbol::Symbol>) -> Self {
        UnboundVariable {
            symbol
        }
    }

    unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Self, OutOfMemory> {
        let sym = self.symbol.clone().into_reachable();
        let sym_cloned = value::value_clone(&sym, allocator)?;
        Ok(Self::new(sym_cloned))
    }

    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        callback(self.symbol.cast_mut_value(), arg);
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unbound variable:{}", self.symbol.as_ref())
    }

}

#[derive(Debug, Clone)]
pub enum Exception {
    OutOfBounds(OutOfBounds),
    TypeMismatch(TypeMismatch),
    MalformedFormat(MalformedFormat),
    UnboundVariable(UnboundVariable),
    ArgTypeMismatch(ArgTypeMismatch),
    DisallowContext,
    OutOfMemory,
    TimeLimit,
    WaitReply,
    MySelfObjectDeleted,
    Exit,
    Other(String),
}

impl Exception {
    pub unsafe fn value_clone_gcunsafe(&self, allocator: &mut AnyAllocator) -> Result<Exception, OutOfMemory> {
        match self {
            Self::OutOfBounds(inner) => {
                let inner = inner.value_clone_gcunsafe(allocator)?;
                Ok(Exception::OutOfBounds(inner))
            }
            Self::TypeMismatch(inner) => {
                let inner = inner.value_clone_gcunsafe(allocator)?;
                Ok(Exception::TypeMismatch(inner))
            }
            Self::MalformedFormat(inner) => {
                let inner = inner.value_clone_gcunsafe(allocator)?;
                Ok(Exception::MalformedFormat(inner))
           }
            Self::UnboundVariable(inner) => {
                let inner = inner.value_clone_gcunsafe(allocator)?;
                Ok(Exception::UnboundVariable(inner))
           }
           Self::ArgTypeMismatch(inner) => {
               let inner = inner.value_clone_gcunsafe(allocator)?;
               Ok(Exception::ArgTypeMismatch(inner))
           }
            //enum?????????????????????match????????????????????????????????????????????????????????????????????????
            Exception::DisallowContext => { Ok(Self::DisallowContext) }
            Exception::OutOfMemory => { Ok(Self::OutOfMemory) }
            Exception::MySelfObjectDeleted => { Ok(Self::MySelfObjectDeleted) }
            Exception::TimeLimit => { Ok(Self::TimeLimit) }
            Exception::WaitReply => { Ok(Self::WaitReply) }
            Exception::Exit => { Ok(Self::Exit) }
            Exception::Other(inner) => { Ok(Self::Other(inner.clone())) }
        }
    }

    pub fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        match self {
            Exception::OutOfBounds(inner) => {
                inner.for_each_alived_value(arg, callback);
            }
            Exception::TypeMismatch(inner) => {
                inner.for_each_alived_value(arg, callback);
            }
            Exception::MalformedFormat(inner) => {
                inner.for_each_alived_value(arg, callback);
            }
            Exception::UnboundVariable(inner) => {
                inner.for_each_alived_value(arg, callback);
            }
            Exception::ArgTypeMismatch(inner) => {
                inner.for_each_alived_value(arg, callback);
            }
            //enum?????????????????????match????????????????????????????????????????????????????????????????????????
            Exception::DisallowContext => { }
            Exception::OutOfMemory => { }
            Exception::MySelfObjectDeleted => { }
            Exception::TimeLimit => { }
            Exception::WaitReply => { }
            Exception::Exit => { }
            Exception::Other(_) => { }
        }
    }

}

impl std::fmt::Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*** Error:")?;

        match self {
            Exception::OutOfBounds(inner) => {
                inner.display(f)
            }
            Exception::TypeMismatch(inner) => {
                inner.display(f)
            }
            Exception::MalformedFormat(inner) => {
                inner.display(f)
            }
            Exception::UnboundVariable(inner) => {
                inner.display(f)
            }
            Exception::ArgTypeMismatch(inner) => {
                inner.display(f)
            }
            //enum?????????????????????match????????????????????????????????????????????????????????????????????????
            Exception::DisallowContext => {
                write!(f, "Disallow context")
            }
            Exception::OutOfMemory => {
                write!(f, "Out of memory")
            }
            Exception::MySelfObjectDeleted => {
                //MySelfObjectDeleted???Display??????????????????????????????????????????
                unreachable!()
            }
            Exception::TimeLimit => {
                //TimeLimit???Display??????????????????????????????????????????
                unreachable!()
            }
            Exception::WaitReply => {
                //WaitReplyt???Display??????????????????????????????????????????
                unreachable!()
            }
            Exception::Exit => {
                //Exit???Display??????????????????????????????????????????
                unreachable!()
            }
            Exception::Other(message) => {
                f.write_str(message)
            }
        }
    }
}

impl From<OutOfBounds> for Exception {
    fn from(this: OutOfBounds) -> Self {
        Exception::OutOfBounds(this)
    }
}

impl From<OutOfMemory> for Exception {
    fn from(_: OutOfMemory) -> Self {
        Exception::OutOfMemory
    }
}


pub type NResult<T, Err> = Result<Ref<T>, Err>;

pub fn into_value<T: NaviType, Err>(result: NResult<T, Err>) -> NResult<Any, Err> {
    result.map(|v| v.into_value())
}

pub enum ResultNone<Result, Err> {
    None,
    Ok(Result),
    Err(Err)
}
