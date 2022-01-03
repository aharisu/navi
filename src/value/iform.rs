use core::panic;
use std::fmt::{Display, Debug};

use crate::ptr::*;
use crate::object::Object;
use crate::value::symbol;
use crate::{util::non_null_const::NonNullConst};

use super::array::Array;
use super::symbol::Symbol;
use super::{TypeInfo, NaviType, Value, cast_value};


static IFORM_TYPEINFO : TypeInfo = new_typeinfo!(
    IForm,
    "IForm",
    0, None,
    IForm::_eq,
    IForm::clone_inner,
    IForm::_fmt,
    IForm::_is_type,
    None,
    None,
    None,
);

pub trait AsIForm : NaviType { }

impl <T: AsIForm + NaviType> FPtr<T> {
    pub fn cast_iform(&self) -> &FPtr<IForm> {
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_iform(self) -> FPtr<IForm> {
        FPtr::new(self.cast_iform().as_ref())
    }
}

pub struct IForm { }

impl NaviType for IForm {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        let value: &Value = cast_value(self);

        //IFormはインスタンス化されることがない型なので、自分自身に対してValue::clone_innerを無限ループにはならない。
        let cloned = Value::clone_inner(value, obj);
        unsafe { cloned.cast_unchecked::<IForm>() }.clone()
    }
}

impl IForm {
    pub fn is<U: AsIForm>(&self) -> bool {
        let other_typeinfo = U::typeinfo();
        let self_typeinfo = super::get_typeinfo(self);
        std::ptr::eq(self_typeinfo.as_ptr(), other_typeinfo.as_ptr())
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
        let offset = unsafe { typeinfo.as_ptr().offset_from(IFORM_TYPEINFO_ARY.as_ptr()) };

        if offset < 0 || IFORM_TYPEINFO_ARY.len() <= offset as usize {
            panic!("unknown iform")
        }

        IFORM_KIND_ARY[offset as usize]
    }

    //Value型のインスタンスは存在しないため、これらのメソッドが呼び出されることはない
    fn _is_type(_other_typeinfo: &TypeInfo) -> bool {
        unreachable!()
    }

    fn _eq(&self, _other: &Self) -> bool {
        unreachable!()
    }

    fn _fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}

impl PartialEq for IForm {
    fn eq(&self, other: &Self) -> bool {
        let this = cast_value(self);
        let other = cast_value(other);
        this == other
    }
}

impl Display for IForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = cast_value(self);
        Display::fmt(v, f)
    }
}

impl Debug for IForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = cast_value(self);
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
    Def = 0,
    If,
    Let,
    LRef,
    GRef,
    Fun,
    Seq,
    Call,
    Const,
    AndOr,
    Container,
    DefRecv,
}

const IFORM_KIND_ARY: [IFormKind; 12] = [
    IFormKind::Def,
    IFormKind::If,
    IFormKind::Let,
    IFormKind::LRef,
    IFormKind::GRef,
    IFormKind::Fun,
    IFormKind::Seq,
    IFormKind::Call,
    IFormKind::Const,
    IFormKind::AndOr,
    IFormKind::Container,
    IFormKind::DefRecv,
];

static IFORM_TYPEINFO_ARY: [TypeInfo; 12] = [
    new_typeinfo!(
        IFormDef,
        "IFormDef",
        std::mem::size_of::<IFormDef>(),
        None,
        IFormDef::eq,
        IFormDef::clone_inner,
        Display::fmt,
        IFormDef::is_type,
        None,
        None,
        Some(IFormDef::child_traversal),
    ),
    new_typeinfo!(
        IFormIf,
        "IFormIf",
        std::mem::size_of::<IFormIf>(),
        None,
        IFormIf::eq,
        IFormIf::clone_inner,
        Display::fmt,
        IFormIf::is_type,
        None,
        None,
        Some(IFormIf::child_traversal),
    ),
    new_typeinfo!(
        IFormLet,
        "IFormLet",
        std::mem::size_of::<IFormLet>(),
        None,
        IFormLet::eq,
        IFormLet::clone_inner,
        Display::fmt,
        IFormLet::is_type,
        None,
        None,
        Some(IFormLet::child_traversal),
    ),
    new_typeinfo!(
        IFormLRef,
        "IFormLRef",
        std::mem::size_of::<IFormLRef>(),
        None,
        IFormLRef::eq,
        IFormLRef::clone_inner,
        Display::fmt,
        IFormLRef::is_type,
        None,
        None,
        Some(IFormLRef::child_traversal),
    ),
    new_typeinfo!(
        IFormGRef,
        "IFormGRef",
        std::mem::size_of::<IFormGRef>(),
        None,
        IFormGRef::eq,
        IFormGRef::clone_inner,
        Display::fmt,
        IFormGRef::is_type,
        None,
        None,
        Some(IFormGRef::child_traversal),
    ),
    new_typeinfo!(
        IFormFun,
        "IFormFun",
        std::mem::size_of::<IFormFun>(),
        None,
        IFormFun::eq,
        IFormFun::clone_inner,
        Display::fmt,
        IFormFun::is_type,
        None,
        None,
        Some(IFormFun::child_traversal),
    ),
    new_typeinfo!(
        IFormSeq,
        "IFormSeq",
        std::mem::size_of::<IFormSeq>(),
        None,
        IFormSeq::eq,
        IFormSeq::clone_inner,
        Display::fmt,
        IFormSeq::is_type,
        None,
        None,
        Some(IFormSeq::child_traversal),
    ),
    new_typeinfo!(
        IFormCall,
        "IFormCall",
        std::mem::size_of::<IFormCall>(),
        None,
        IFormCall::eq,
        IFormCall::clone_inner,
        Display::fmt,
        IFormCall::is_type,
        None,
        None,
        Some(IFormCall::child_traversal),
    ),
    new_typeinfo!(
        IFormConst,
        "IFormConst",
        std::mem::size_of::<IFormConst>(),
        None,
        IFormConst::eq,
        IFormConst::clone_inner,
        Display::fmt,
        IFormConst::is_type,
        None,
        None,
        Some(IFormConst::child_traversal),
    ),
    new_typeinfo!(
        IFormAndOr,
        "IFormAndOr",
        std::mem::size_of::<IFormAndOr>(),
        None,
        IFormAndOr::eq,
        IFormAndOr::clone_inner,
        Display::fmt,
        IFormAndOr::is_type,
        None,
        None,
        Some(IFormAndOr::child_traversal),
    ),
    new_typeinfo!(
        IFormContainer,
        "IFormContainer",
        std::mem::size_of::<IFormContainer>(),
        None,
        IFormContainer::eq,
        IFormContainer::clone_inner,
        Display::fmt,
        IFormContainer::is_type,
        None,
        None,
        Some(IFormContainer::child_traversal),
    ),
    new_typeinfo!(
        IFormDefRecv,
        "IFormDefRecv",
        std::mem::size_of::<IFormDefRecv>(),
        None,
        IFormDefRecv::eq,
        IFormDefRecv::clone_inner,
        Display::fmt,
        IFormDefRecv::is_type,
        None,
        None,
        Some(IFormDefRecv::child_traversal),
    ),
];


pub struct IFormDef {
    symbol: FPtr<Symbol>,
    val: FPtr<IForm>,
}

impl NaviType for IFormDef {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Def as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = Symbol::clone_inner(self.symbol.as_ref(), obj).into_reachable();
            let val = IForm::clone_inner(self.val.as_ref(), obj).into_reachable();

            Self::alloc(&symbol, &val, obj)
        }
    }
}

impl IFormDef {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Def as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.symbol.cast_value(), arg);
        callback(self.val.cast_value(), arg);
    }

    pub fn alloc(symbol: &Reachable<Symbol>, val: &Reachable<IForm>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormDef>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormDef {
                    symbol: FPtr::new(symbol.as_ref()),
                    val: FPtr::new(val.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn symbol(&self) -> FPtr<Symbol> {
        self.symbol.clone()
    }

    pub fn val(&self) -> FPtr<IForm> {
        self.val.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFDef {} {})", self.symbol.as_ref(), self.val.as_ref())
    }
}

impl PartialEq for IFormDef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormDef {}

pub struct  IFormIf {
    test: FPtr<IForm>,
    then: FPtr<IForm>,
    else_: FPtr<IForm>,
}

impl NaviType for IFormIf {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::If as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let test = IForm::clone_inner(self.test.as_ref(), obj).into_reachable();
            let then = IForm::clone_inner(self.then.as_ref(), obj).into_reachable();
            let else_ = IForm::clone_inner(self.else_.as_ref(), obj).into_reachable();

            Self::alloc(&test, &then, &else_, obj)
        }
    }
}

impl IFormIf {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::If as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.test.cast_value(), arg);
        callback(self.then.cast_value(), arg);
        callback(self.else_.cast_value(), arg);
    }

    pub fn alloc(test: &Reachable<IForm>, true_: &Reachable<IForm>, false_: &Reachable<IForm>, obj: &mut Object) -> FPtr<IFormIf> {
        let ptr = obj.alloc::<IFormIf>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormIf {
                    test: FPtr::new(test.as_ref()),
                    then: FPtr::new(true_.as_ref()),
                    else_: FPtr::new(false_.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn test(&self) -> FPtr<IForm> {
        self.test.clone()
    }

    pub fn then(&self) -> FPtr<IForm> {
        self.then.clone()
    }

    pub fn else_(&self) -> FPtr<IForm> {
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

pub struct IFormLet {
    symbols: FPtr<Array<Symbol>>,
    vals: FPtr<Array<IForm>>,
    body: FPtr<IForm>,
}


impl NaviType for IFormLet {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Let as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbols = Array::clone_inner(self.symbols.as_ref(), obj).into_reachable();
            let vals = Array::clone_inner(self.vals.as_ref(), obj).into_reachable();
            let body = IForm::clone_inner(self.body.as_ref(), obj).into_reachable();

            Self::alloc(&symbols, &vals, &body, obj)
        }
    }
}

impl IFormLet {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Let as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.symbols.cast_value(), arg);
        callback(self.vals.cast_value(), arg);
        callback(self.body.cast_value(), arg);
    }

    pub fn alloc(symbols: &Reachable<Array<Symbol>>, vals: &Reachable<Array<IForm>>, body: &Reachable<IForm>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormLet>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormLet {
                    symbols: FPtr::new(symbols.as_ref()),
                    vals: FPtr::new(vals.as_ref()),
                    body: FPtr::new(body.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn len_binders(&self) -> usize {
        self.symbols.as_ref().len()
    }

    pub fn get_symbol(&self, index: usize) -> FPtr<Symbol> {
        self.symbols.as_ref().get(index)
    }

    pub fn get_val(&self, index: usize) -> FPtr<IForm> {
        self.vals.as_ref().get(index)
    }

    pub fn body(&self) -> FPtr<IForm> {
        self.body.clone()
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFLet {} {} {})", self.symbols.as_ref(), self.vals.as_ref(), self.body.as_ref())
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

pub struct IFormLRef {
    symbol: FPtr<Symbol>,
}

impl NaviType for IFormLRef {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::LRef as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = symbol::Symbol::clone_inner(self.symbol.as_ref(), obj).into_reachable();

            Self::alloc(&symbol, obj)
        }
    }
}

impl IFormLRef {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::LRef as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.symbol.cast_value(), arg);
    }

    pub fn alloc(v: &Reachable<Symbol>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormLRef>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormLRef {
                    symbol: FPtr::new(v.as_ref())
                });
        }

        ptr.into_fptr()
    }

    pub fn symbol(&self) -> FPtr<Symbol> {
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
    symbol: FPtr<Symbol>,
}

impl NaviType for IFormGRef {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::GRef as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let symbol = symbol::Symbol::clone_inner(self.symbol.as_ref(), obj).into_reachable();

            Self::alloc(&symbol, obj)
        }
    }
}

impl IFormGRef {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::GRef as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.symbol.cast_value(), arg);
    }

    pub fn alloc(v: &Reachable<Symbol>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormGRef>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormGRef {
                    symbol: FPtr::new(v.as_ref())
                });
        }

        ptr.into_fptr()
    }

    pub fn symbol(&self) -> FPtr<Symbol> {
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
    params: FPtr<Array<Symbol>>,
    body: FPtr<IForm>
}

impl NaviType for IFormFun {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Fun as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let params = Array::clone_inner(self.params.as_ref(), obj).into_reachable();
            let body = IForm::clone_inner(self.body.as_ref(), obj).into_reachable();

            Self::alloc(&params, &body, obj)
        }
    }
}

impl IFormFun {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Fun as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.params.cast_value(), arg);
        callback(&self.body.cast_value(), arg);
    }

    pub fn alloc(params: &Reachable<Array<Symbol>>, body: &Reachable<IForm>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormFun>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormFun {
                    params: FPtr::new(params.as_ref()),
                    body: FPtr::new(body.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn len_params(&self) -> usize {
        self.params.as_ref().len()
    }

    pub fn get_param(&self, index: usize) -> FPtr<Symbol> {
        self.params.as_ref().get(index)
    }

    pub fn body(&self) -> FPtr<IForm> {
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
    body: FPtr<Array<IForm>>,
}

impl NaviType for IFormSeq {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Seq as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let body = Array::clone_inner(self.body.as_ref(), obj).into_reachable();

            Self::alloc(&body, obj)
        }
    }
}

impl IFormSeq {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Seq as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.body.cast_value(), arg);
    }

    pub fn alloc(body: &Reachable<Array<IForm>>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormSeq>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormSeq {
                    body: FPtr::new(body.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn body(&self) -> FPtr<Array<IForm>> {
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
    app: FPtr<IForm>,
    args: FPtr<Array<IForm>>,
}

impl NaviType for IFormCall {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Call as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let app = IForm::clone_inner(self.app.as_ref(), obj).into_reachable();
            let args = Array::clone_inner(self.args.as_ref(), obj).into_reachable();

            Self::alloc(&app, &args, obj)
        }
    }
}

impl IFormCall {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Call as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.app.cast_value(), arg);
        callback(&self.args.cast_value(), arg);
    }

    pub fn alloc(app: &Reachable<IForm>, args: &Reachable<Array<IForm>>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormCall>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormCall {
                    app: FPtr::new(app.as_ref()),
                    args: FPtr::new(args.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn app(&self) -> FPtr<IForm> {
        self.app.clone()
    }

    pub fn len_args(&self) -> usize {
        self.args.as_ref().len()
    }

    pub fn get_arg(&self, index: usize) -> FPtr<IForm> {
        self.args.as_ref().get(index)
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IFCall {} {})", self.app.as_ref(), self.args.as_ref())
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
    value: FPtr<Value>,
}

impl NaviType for IFormConst {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Const as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let value = Value::clone_inner(self.value.as_ref(), obj).into_reachable();

            Self::alloc(&value, obj)
        }
    }
}

impl IFormConst {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Const as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.value.cast_value(), arg);
    }

    pub fn alloc(v: &Reachable<Value>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormConst>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormConst {
                    value: FPtr::new(v.as_ref())
                });
        }

        ptr.into_fptr()
    }

    pub fn value(&self) -> FPtr<Value> {
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
    exprs: FPtr<Array<IForm>>,
    kind: AndOrKind,
}

impl NaviType for IFormAndOr {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::AndOr as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let exprs = Array::clone_inner(self.exprs.as_ref(), obj).into_reachable();

            Self::alloc(&exprs, self.kind, obj)
        }
    }
}

impl IFormAndOr {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::AndOr as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.exprs.cast_value(), arg);
    }

    pub fn alloc(exprs: &Reachable<Array<IForm>>, kind: AndOrKind, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormAndOr>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormAndOr {
                    exprs: FPtr::new(exprs.as_ref()),
                    kind: kind,
                });
        }

        ptr.into_fptr()
    }

    pub fn len_exprs(&self) -> usize {
        self.exprs.as_ref().len()
    }

    pub fn get_expr(&self, index: usize) -> FPtr<IForm> {
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

#[derive(Debug, Copy, Clone)]
pub enum ContainerKind {
    //TODO
    //List,
    Array,
    Tuple,
}

pub struct IFormContainer {
    exprs: FPtr<Array<IForm>>,
    kind: ContainerKind,
}

impl NaviType for IFormContainer {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::Container as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let exprs = Array::clone_inner(self.exprs.as_ref(), obj).into_reachable();

            Self::alloc(&exprs, self.kind, obj)
        }
    }
}

impl IFormContainer {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::Container as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.exprs.cast_value(), arg);
    }

    pub fn alloc(exprs: &Reachable<Array<IForm>>, kind: ContainerKind, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormContainer>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormContainer {
                    exprs: FPtr::new(exprs.as_ref()),
                    kind: kind,
                });
        }

        ptr.into_fptr()
    }

    pub fn len_exprs(&self) -> usize {
        self.exprs.as_ref().len()
    }

    pub fn get_expr(&self, index: usize) -> FPtr<IForm> {
        self.exprs.as_ref().get(index)
    }

    pub fn kind(&self) -> ContainerKind {
        self.kind
    }

    fn fmt(&self, _is_debug: bool, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(IF{:?} {})", self.kind, self.exprs.as_ref())
    }
}

impl PartialEq for IFormContainer {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Display for IFormContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(false, f)
    }
}

impl Debug for IFormContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(true, f)
    }
}

impl AsIForm for IFormContainer {}

pub struct IFormDefRecv {
    pattern: FPtr<Value>,
    body: FPtr<super::list::List>,
}

impl NaviType for IFormDefRecv {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&IFORM_TYPEINFO_ARY[IFormKind::DefRecv as usize] as *const TypeInfo)
    }

    fn clone_inner(&self, obj: &mut Object) -> FPtr<Self> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let pattern = Value::clone_inner(self.pattern.as_ref(), obj).into_reachable();
            let body = super::list::List::clone_inner(self.body.as_ref(), obj).into_reachable();

            Self::alloc(&pattern, &body, obj)
        }
    }
}

impl IFormDefRecv {
    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&IFORM_TYPEINFO_ARY[IFormKind::DefRecv as usize], other_typeinfo)
        || std::ptr::eq(&IFORM_TYPEINFO, other_typeinfo)
    }

    fn child_traversal(&self, arg: *mut u8, callback: fn(&FPtr<Value>, arg: *mut u8)) {
        callback(&self.pattern.cast_value(), arg);
        callback(&self.body.cast_value(), arg);
    }

    pub fn alloc(pattern: &Reachable<Value>, body: &Reachable<super::list::List>, obj: &mut Object) -> FPtr<Self> {
        let ptr = obj.alloc::<IFormDefRecv>();
        unsafe {
            std::ptr::write(ptr.as_ptr(), IFormDefRecv {
                    pattern: FPtr::new(pattern.as_ref()),
                    body: FPtr::new(body.as_ref()),
                });
        }

        ptr.into_fptr()
    }

    pub fn pattern(&self) -> FPtr<Value> {
        self.pattern.clone()
    }

    pub fn body(&self) -> FPtr<super::list::List> {
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