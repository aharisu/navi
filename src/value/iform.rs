use core::panic;
use std::fmt::{Display, Debug};

use crate::ptr::*;
use crate::err::*;
use crate::object::{AnyAllocator, Allocator};
use crate::value::symbol;

use super::array::Array;
use super::symbol::Symbol;
use super::{TypeInfo, NaviType, Any};


static IFORM_TYPEINFO : TypeInfo = new_typeinfo!(
    IForm,
    "IForm",
    0, None,
    IForm::_eq,
    IForm::clone_inner,
    IForm::_fmt,
    None,
    None,
    None,
    None,
    None,
    None,
);

pub trait AsIForm : NaviType { }

impl <T: AsIForm + NaviType> Ref<T> {
    pub fn cast_iform(&self) -> &Ref<IForm> {
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_iform(self) -> Ref<IForm> {
        self.cast_iform().raw_ptr().into()
    }
}

pub struct IForm { }

impl NaviType for IForm {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        let value: &Any = self.cast_value();

        //IFormはインスタンス化されることがない型なので、自分自身に対してValue::clone_innerを無限ループにはならない。
        let cloned = Any::clone_inner(value, allocator)?;
        Ok(unsafe { cloned.cast_unchecked::<IForm>() }.clone())
    }
}

impl IForm {
    pub fn is<U: AsIForm>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        let self_typeinfo = super::get_typeinfo(self);
        self_typeinfo == other_typeinfo
    }

    pub fn try_cast<U: AsIForm>(&self) -> Option<&U> {
        if self.is::<U>() {
            Some(unsafe {&* (self as *const IForm as *const U) })
        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: AsIForm>(&self) -> &U {
        std::mem::transmute(self)
    }

    pub fn kind(&self) -> IFormKind {
        let typeinfo = super::get_typeinfo(self);
        let offset = unsafe { (typeinfo as *const TypeInfo).offset_from(IFORM_TYPEINFO_ARY.as_ptr()) };

        if offset < 0 || IFORM_TYPEINFO_ARY.len() <= offset as usize {
            panic!("unknown iform")
        }

        IFORM_KIND_ARY[offset as usize]
    }

    //Value型のインスタンスは存在しないため、これらのメソッドが呼び出されることはない
    fn _eq(&self, _other: &Self) -> bool {
        unreachable!()
    }

    fn _fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }

    fn cast_value(&self) -> &Any {
        //任意のNaviTypeの参照からValueへの参照への変換は安全なので無理やりキャスト
        unsafe { std::mem::transmute(self) }
    }
}

impl PartialEq for IForm {
    fn eq(&self, other: &Self) -> bool {
        let this = self.cast_value();
        let other = other.cast_value();
        this == other
    }
}

impl Display for IForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.cast_value();
        Display::fmt(v, f)
    }
}

impl Debug for IForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.cast_value();
        Debug::fmt(v, f)
    }
}


impl Cap<IForm> {
    pub fn try_cast<U: AsIForm>(&self) -> Option<&Cap<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: AsIForm>(&self) -> &Cap<U> {
        std::mem::transmute(self)
    }
}

impl Reachable<IForm> {
    pub fn try_cast<U: AsIForm>(&self) -> Option<&Reachable<U>> {
        if self.as_ref().is::<U>() {
            Some( unsafe { self.cast_unchecked() } )

        } else {
            None
        }
    }

    pub unsafe fn cast_unchecked<U: AsIForm>(&self) -> &Reachable<U> {
        std::mem::transmute(self)
    }
}

#[derive(Copy, Clone)]
pub enum IFormKind {
    Let = 0,
    If,
    Local,
    LRef,
    GRef,
    Fun,
    Seq,
    Call,
    Const,
    AndOr,
    DefRecv,
    ObjectSwitch,
}

const IFORM_KIND_ARY: [IFormKind; 12] = [
    IFormKind::Let,
    IFormKind::If,
    IFormKind::Local,
    IFormKind::LRef,
    IFormKind::GRef,
    IFormKind::Fun,
    IFormKind::Seq,
    IFormKind::Call,
    IFormKind::Const,
    IFormKind::AndOr,
    IFormKind::DefRecv,
    IFormKind::ObjectSwitch,
];

static IFORM_TYPEINFO_ARY: [TypeInfo; 12] = [
    new_typeinfo!(
        IFormLet,
        "IFormLet",
        std::mem::size_of::<IFormLet>(),
        None,
        IFormLet::eq,
        IFormLet::clone_inner,
        Display::fmt,
        Some(IFormLet::is_type),
        None,
        None,
        Some(IFormLet::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormIf,
        "IFormIf",
        std::mem::size_of::<IFormIf>(),
        None,
        IFormIf::eq,
        IFormIf::clone_inner,
        Display::fmt,
        Some(IFormIf::is_type),
        None,
        None,
        Some(IFormIf::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormLocal,
        "IFormLocal",
        std::mem::size_of::<IFormLocal>(),
        None,
        IFormLocal::eq,
        IFormLocal::clone_inner,
        Display::fmt,
        Some(IFormLocal::is_type),
        None,
        None,
        Some(IFormLocal::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormLRef,
        "IFormLRef",
        std::mem::size_of::<IFormLRef>(),
        None,
        IFormLRef::eq,
        IFormLRef::clone_inner,
        Display::fmt,
        Some(IFormLRef::is_type),
        None,
        None,
        Some(IFormLRef::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormGRef,
        "IFormGRef",
        std::mem::size_of::<IFormGRef>(),
        None,
        IFormGRef::eq,
        IFormGRef::clone_inner,
        Display::fmt,
        Some(IFormGRef::is_type),
        None,
        None,
        Some(IFormGRef::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormFun,
        "IFormFun",
        std::mem::size_of::<IFormFun>(),
        None,
        IFormFun::eq,
        IFormFun::clone_inner,
        Display::fmt,
        Some(IFormFun::is_type),
        None,
        None,
        Some(IFormFun::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormSeq,
        "IFormSeq",
        std::mem::size_of::<IFormSeq>(),
        None,
        IFormSeq::eq,
        IFormSeq::clone_inner,
        Display::fmt,
        Some(IFormSeq::is_type),
        None,
        None,
        Some(IFormSeq::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormCall,
        "IFormCall",
        std::mem::size_of::<IFormCall>(),
        None,
        IFormCall::eq,
        IFormCall::clone_inner,
        Display::fmt,
        Some(IFormCall::is_type),
        None,
        None,
        Some(IFormCall::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormConst,
        "IFormConst",
        std::mem::size_of::<IFormConst>(),
        None,
        IFormConst::eq,
        IFormConst::clone_inner,
        Display::fmt,
        Some(IFormConst::is_type),
        None,
        None,
        Some(IFormConst::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormAndOr,
        "IFormAndOr",
        std::mem::size_of::<IFormAndOr>(),
        None,
        IFormAndOr::eq,
        IFormAndOr::clone_inner,
        Display::fmt,
        Some(IFormAndOr::is_type),
        None,
        None,
        Some(IFormAndOr::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormDefRecv,
        "IFormDefRecv",
        std::mem::size_of::<IFormDefRecv>(),
        None,
        IFormDefRecv::eq,
        IFormDefRecv::clone_inner,
        Display::fmt,
        Some(IFormDefRecv::is_type),
        None,
        None,
        Some(IFormDefRecv::child_traversal),
        None,
        None,
    ),
    new_typeinfo!(
        IFormObjectSwitch,
        "IFormObjectSwitch",
        std::mem::size_of::<IFormObjectSwitch>(),
        None,
        IFormObjectSwitch::eq,
        IFormObjectSwitch::clone_inner,
        Display::fmt,
        Some(IFormObjectSwitch::is_type),
        None,
        None,
        Some(IFormObjectSwitch::child_traversal),
        None,
        None,
    ),
];


pub struct IFormLet {
    symbol: Ref<Symbol>,
    val: Ref<IForm>,
    force_global: bool,
}

impl NaviType for IFormLet {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Let as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = Symbol::clone_inner(self.symbol.as_ref(), allocator)?.into_reachable();
            let val = IForm::clone_inner(self.val.as_ref(), allocator)?.into_reachable();

            Self::alloc(&symbol, &val, self.force_global, allocator)
        }
    }
}

impl IFormLet {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Let as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.symbol.cast_mut_value(), arg);
        callback(self.val.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(symbol: &Reachable<Symbol>, val: &Reachable<IForm>, force_global : bool, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormLet>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormLet {
                    symbol: symbol.raw_ptr().into(),
                    val: val.raw_ptr().into(),
                    force_global: force_global,
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn symbol(&self) -> Ref<Symbol> {
        self.symbol.clone()
    }

    pub fn val(&self) -> Ref<IForm> {
        self.val.clone()
    }

    pub fn force_global(&self) -> bool {
        self.force_global
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.force_global {
            write!(f, "(IFLetGlobal {} {})", self.symbol.as_ref(), self.val.as_ref())
        } else {
            write!(f, "(IFLet {} {})", self.symbol.as_ref(), self.val.as_ref())
        }
    }
}

impl PartialEq for IFormLet {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormLet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormLet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormLet {}

pub struct  IFormIf {
    test: Ref<IForm>,
    then: Ref<IForm>,
    else_: Ref<IForm>,
}

impl NaviType for IFormIf {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::If as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let test = IForm::clone_inner(self.test.as_ref(), allocator)?.into_reachable();
            let then = IForm::clone_inner(self.then.as_ref(), allocator)?.into_reachable();
            let else_ = IForm::clone_inner(self.else_.as_ref(), allocator)?.into_reachable();

            Self::alloc(&test, &then, &else_, allocator)
        }
    }
}

impl IFormIf {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::If as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.test.cast_mut_value(), arg);
        callback(self.then.cast_mut_value(), arg);
        callback(self.else_.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(test: &Reachable<IForm>, true_: &Reachable<IForm>, false_: &Reachable<IForm>, allocator: &mut A) -> NResult<IFormIf, OutOfMemory> {
        let ptr = allocator.alloc::<IFormIf>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormIf {
                    test: test.raw_ptr().into(),
                    then: true_.raw_ptr().into(),
                    else_: false_.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn test(&self) -> Ref<IForm> {
        self.test.clone()
    }

    pub fn then(&self) -> Ref<IForm> {
        self.then.clone()
    }

    pub fn else_(&self) -> Ref<IForm> {
        self.else_.clone()
    }


    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFIf {} {} {})", self.test.as_ref(), self.then.as_ref(), self.else_.as_ref())
    }
}

impl PartialEq for IFormIf {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormIf {}

pub struct IFormLocal {
    body: Ref<IForm>,
}


impl NaviType for IFormLocal {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Local as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let body = IForm::clone_inner(self.body.as_ref(), allocator)?.into_reachable();

            Self::alloc(&body, allocator)
        }
    }
}

impl IFormLocal {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Local as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.body.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(body: &Reachable<IForm>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormLocal>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormLocal {
                    body: body.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn body(&self) -> Ref<IForm> {
        self.body.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFLocal {})", self.body.as_ref())
    }
}

impl PartialEq for IFormLocal {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormLocal {}

pub struct IFormLRef {
    symbol: Ref<Symbol>,
}

impl NaviType for IFormLRef {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::LRef as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = symbol::Symbol::clone_inner(self.symbol.as_ref(), allocator)?.into_reachable();

            Self::alloc(&symbol, allocator)
        }
    }
}

impl IFormLRef {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::LRef as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.symbol.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(v: &Reachable<Symbol>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormLRef>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormLRef {
                    symbol: v.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn symbol(&self) -> Ref<Symbol> {
        self.symbol.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFLRef {})", self.symbol.as_ref())
    }
}

impl PartialEq for IFormLRef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormLRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormLRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormLRef {}

pub struct IFormGRef {
    symbol: Ref<Symbol>,
}

impl NaviType for IFormGRef {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::GRef as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = symbol::Symbol::clone_inner(self.symbol.as_ref(), allocator)?.into_reachable();

            Self::alloc(&symbol, allocator)
        }
    }
}

impl IFormGRef {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::GRef as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.symbol.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(v: &Reachable<Symbol>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormGRef>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormGRef {
                    symbol: v.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn symbol(&self) -> Ref<Symbol> {
        self.symbol.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFGRef {})", self.symbol.as_ref())
    }
}

impl PartialEq for IFormGRef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormGRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormGRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormGRef {}

pub struct IFormFun {
    params: Ref<Array<Symbol>>,
    body: Ref<IForm>
}

impl NaviType for IFormFun {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Fun as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let params = Array::clone_inner(self.params.as_ref(), allocator)?.into_reachable();
            let body = IForm::clone_inner(self.body.as_ref(), allocator)?.into_reachable();

            Self::alloc(&params, &body, allocator)
        }
    }
}

impl IFormFun {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Fun as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.params.cast_mut_value(), arg);
        callback(self.body.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(params: &Reachable<Array<Symbol>>, body: &Reachable<IForm>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormFun>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormFun {
                    params: params.raw_ptr().into(),
                    body: body.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn len_params(&self) -> usize {
        self.params.as_ref().len()
    }

    pub fn get_param(&self, index: usize) -> Ref<Symbol> {
        self.params.as_ref().get(index)
    }

    pub fn body(&self) -> Ref<IForm> {
        self.body.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFFun {} {})", self.params.as_ref(), self.body.as_ref())
    }
}

impl PartialEq for IFormFun {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormFun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormFun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormFun {}

pub struct IFormSeq {
    body: Ref<Array<IForm>>,
}

impl NaviType for IFormSeq {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Seq as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let body = Array::clone_inner(self.body.as_ref(), allocator)?.into_reachable();

            Self::alloc(&body, allocator)
        }
    }
}

impl IFormSeq {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Seq as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.body.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(body: &Reachable<Array<IForm>>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormSeq>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormSeq {
                    body: body.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn body(&self) -> Ref<Array<IForm>> {
        self.body.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFSeq {})", self.body.as_ref())
    }
}

impl PartialEq for IFormSeq {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl AsIForm for IFormSeq {}

pub struct IFormCall {
    app: Ref<IForm>,
    args: Ref<Array<IForm>>,
    is_tail: bool,
}

impl NaviType for IFormCall {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Call as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let app = IForm::clone_inner(self.app.as_ref(), allocator)?.into_reachable();
            let args = Array::clone_inner(self.args.as_ref(), allocator)?.into_reachable();

            Self::alloc(&app, &args, self.is_tail, allocator)
        }
    }
}

impl IFormCall {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Call as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.app.cast_mut_value(), arg);
        callback(self.args.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(app: &Reachable<IForm>, args: &Reachable<Array<IForm>>, is_tail: bool, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormCall>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormCall {
                    app: app.raw_ptr().into(),
                    args: args.raw_ptr().into(),
                    is_tail,
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn app(&self) -> Ref<IForm> {
        self.app.clone()
    }

    pub fn len_args(&self) -> usize {
        self.args.as_ref().len()
    }

    pub fn get_arg(&self, index: usize) -> Ref<IForm> {
        self.args.as_ref().get(index)
    }

    pub fn is_tail(&self) -> bool {
        self.is_tail
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_tail {
            write!(f, "(IFTailCall {} {})", self.app.as_ref(), self.args.as_ref())
        } else {
            write!(f, "(IFCall {} {})", self.app.as_ref(), self.args.as_ref())
        }
    }
}

impl PartialEq for IFormCall {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormCall {}

pub struct IFormConst {
    value: Ref<Any>,
}

impl NaviType for IFormConst {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::Const as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let value = Any::clone_inner(self.value.as_ref(), allocator)?.into_reachable();

            Self::alloc(&value, allocator)
        }
    }
}

impl IFormConst {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::Const as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.value.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(v: &Reachable<Any>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormConst>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormConst {
                    value: v.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn value(&self) -> Ref<Any> {
        self.value.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFConst {})", self.value.as_ref())
    }
}

impl PartialEq for IFormConst {
    fn eq(&self, other: &Self) -> bool {
        self.value.as_ref() == other.value.as_ref()
    }
}

impl Display for IFormConst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormConst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormConst {}

#[derive(Debug, Copy, Clone)]
pub enum AndOrKind {
    And,
    Or,
    MatchSuccess,
}

pub struct IFormAndOr {
    exprs: Ref<Array<IForm>>,
    kind: AndOrKind,
}

impl NaviType for IFormAndOr {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::AndOr as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let exprs = Array::clone_inner(self.exprs.as_ref(), allocator)?.into_reachable();

            Self::alloc(&exprs, self.kind, allocator)
        }
    }
}

impl IFormAndOr {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::AndOr as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.exprs.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(exprs: &Reachable<Array<IForm>>, kind: AndOrKind, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormAndOr>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormAndOr {
                    exprs: exprs.raw_ptr().into(),
                    kind: kind,
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn len_exprs(&self) -> usize {
        self.exprs.as_ref().len()
    }

    pub fn get_expr(&self, index: usize) -> Ref<IForm> {
        self.exprs.as_ref().get(index)
    }

    pub fn kind(&self) -> AndOrKind {
        self.kind
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IF{:?} {})", self.kind, self.exprs.as_ref())
    }
}

impl PartialEq for IFormAndOr {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormAndOr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormAndOr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormAndOr {}

pub struct IFormDefRecv {
    pattern: Ref<Any>,
    body: Ref<super::list::List>,
}

impl NaviType for IFormDefRecv {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::DefRecv as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let pattern = Any::clone_inner(self.pattern.as_ref(), allocator)?.into_reachable();
            let body = super::list::List::clone_inner(self.body.as_ref(), allocator)?.into_reachable();

            Self::alloc(&pattern, &body, allocator)
        }
    }
}

impl IFormDefRecv {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::DefRecv as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(self.pattern.cast_mut_value(), arg);
        callback(self.body.cast_mut_value(), arg);
    }

    pub fn alloc<A: Allocator>(pattern: &Reachable<Any>, body: &Reachable<super::list::List>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormDefRecv>()?;
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormDefRecv {
                    pattern: pattern.raw_ptr().into(),
                    body: body.raw_ptr().into(),
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn pattern(&self) -> Ref<Any> {
        self.pattern.clone()
    }

    pub fn body(&self) -> Ref<super::list::List> {
        self.body.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFDefRecv {} {})", self.pattern.as_ref(), self.body.as_ref())
    }
}

impl PartialEq for IFormDefRecv {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormDefRecv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormDefRecv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormDefRecv {}

pub struct IFormObjectSwitch {
    target_obj: Option<Ref<IForm>>,
}

impl NaviType for IFormObjectSwitch {
    fn typeinfo() -> &'static TypeInfo {
        &IFORM_TYPEINFO_ARY[IFormKind::ObjectSwitch as usize]
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            if let Some(target_obj) = self.target_obj.as_ref() {
                let target_obj = target_obj.clone().into_reachable();
                Self::alloc(Some(&target_obj), allocator)

            } else {
                Self::alloc(None, allocator)
            }
        }
    }
}

impl IFormObjectSwitch {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        &IFORM_TYPEINFO_ARY[IFormKind::ObjectSwitch as usize] == other_typeinfo
        || &IFORM_TYPEINFO == other_typeinfo
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        if let Some(obj) = self.target_obj.as_mut() {
            callback(obj.cast_mut_value(), arg);
        }
    }

    pub fn alloc<A: Allocator>(target_obj: Option<&Reachable<IForm>>, allocator: &mut A) -> NResult<Self, OutOfMemory> {
        let ptr = allocator.alloc::<IFormObjectSwitch>()?;

        let target_obj = target_obj.map(|v| Ref::from(v.raw_ptr()));
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormObjectSwitch {
                    target_obj,
                });
        }

        Ok(ptr.into_ref())
    }

    pub fn target_obj(&self) -> Option<Ref<IForm>> {
        self.target_obj.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(obj) = self.target_obj.as_ref() {
            write!(f, "(IFObjectSwitch {})", obj.as_ref())
        } else {
            write!(f, "(IFReturnObjectSwitch)")
        }
    }
}

impl PartialEq for IFormObjectSwitch {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormObjectSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormObjectSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormObjectSwitch {}