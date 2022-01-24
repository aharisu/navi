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
    num_free_vars: usize,
}

static CLOSURE_TYPEINFO: TypeInfo = new_typeinfo!(
    Closure,
    "Closure",
    0,
    Some(Closure::size_of),
    Closure::eq,
    Closure::clone_inner,
    Display::fmt,
    Some(Closure::is_type),
    None,
    None,
    Some(Closure::child_traversal),
    Some(Closure::check_reply),
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

        let mut closure = Self::alloc(program, &constants, self.num_args, self.num_free_vars, allocator)?;

        for index in 0 .. self.num_free_vars {
            let child = self.get_inner(index);
            let child = child.cast_value();
            let cloned = Any::clone_inner(child.as_ref(), allocator)?;

            closure.set_uncheck(cloned.raw_ptr(), index);
        }

        Ok(closure)
    }
}

impl Closure {
    fn size_of(&self) -> usize {
        std::mem::size_of::<Closure>()
            + self.num_free_vars * std::mem::size_of::<Ref<Any>>()
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        self.code.child_traversal(arg, callback);

        for index in 0 .. self.num_free_vars {
            callback(self.get_inner(index), arg);
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &CLOSURE_TYPEINFO == other_typeinfo
        || app::App::typeinfo() == other_typeinfo
    }

    fn check_reply(cap: &mut Cap<Closure>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        for index in 0.. cap.as_ref().num_free_vars {
            let child_v = cap.as_ref().get_inner(index);
            //子要素がReply型を持っている場合は
            if child_v.has_replytype() {

                //返信がないか確認する
                let mut child_v = child_v.clone().capture(obj);
                if crate::value::check_reply(&mut child_v, obj)? {
                    //返信があった場合は、内部ポインタを返信結果の値に上書きする
                    cap.as_ref().get_inner(index).update_pointer(child_v.raw_ptr());
                } else {
                    //子要素にReplyを含む値が残っている場合は、全体をfalseにする
                    return Ok(false);
                }
            }
        }

        //内部にReply型を含まなくなったのでフラグを下す
        crate::value::clear_has_replytype_flag(cap.mut_refer());

        Ok(true)
    }

    pub fn alloc<A: Allocator>(program: Vec<u8>, constants: &[Ref<Any>], num_args: usize, num_free_vars: usize, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc_with_additional_size::<Closure>(num_free_vars * std::mem::size_of::<Ref<Any>>())?;

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
                num_free_vars,
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

    pub fn get(&self, index: usize) -> Ref<Any> {
        self.get_inner(index).clone()
    }

    fn get_inner<'a>(&'a self, index: usize) -> &'a mut Ref<Any> {
        let ptr = self as *const Closure;
        unsafe {
            //ポインタをClosure構造体の後ろに移す
            let ptr = ptr.add(1);
            //Closure構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<Any>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);

            &mut *(storage_ptr)
        }
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

impl Ref<Closure> {
    fn set_uncheck(&mut self, v: *mut Any, index: usize) {
        let ptr = self.as_mut() as *mut Closure;
        unsafe {
            //ポインタをArray構造体の後ろに移す
            let ptr = ptr.add(1);
            //Closure構造体の後ろにはallocで確保した保存領域がある
            let storage_ptr = ptr as *mut Ref<Any>;
            //保存領域内の指定indexに移動
            let storage_ptr = storage_ptr.add(index);
            //指定indexにポインタを書き込む
            std::ptr::write(storage_ptr, v.into());
        };

    }

    pub fn set<V: ValueHolder<Any>>(&mut self, v: &V, index: usize) {
        debug_assert!(index < self.as_ref().num_free_vars);

        self.set_uncheck(v.raw_ptr(), index);

        if v.has_replytype() {
            crate::value::set_has_replytype_flag(self);
        }
    }
}