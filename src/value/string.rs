use crate::value::*;
use crate::context::{Context};
use crate::ptr::*;
use std::fmt::{self, Debug, Display};

type StringRef = std::mem::ManuallyDrop<String>;

pub struct NString {
    len: usize,
    len_inbytes: usize,
}

static STRING_TYPEINFO: TypeInfo = new_typeinfo!(
    NString,
    "String",
    NString::eq,
    Display::fmt,
    NString::is_type,
    None,
    None,
    None,
);

impl NaviType for NString {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&STRING_TYPEINFO as *const TypeInfo)
    }
}

impl NString {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&STRING_TYPEINFO, other_typeinfo)
    }

    pub fn alloc(str: &String, ctx : &mut Context) -> FPtr<NString> {
        Self::alloc_inner(str, ctx)
    }

    //NStringとSymbol,Keywordクラス共有のアロケーション用関数。TはNSTringもしくはSymbol、Keywordのみ対応。
    pub(crate) fn alloc_inner<T: NaviType>(str: &String, ctx : &mut Context) -> FPtr<T> {
        let len_inbytes = str.len();
        let ptr = ctx.alloc_with_additional_size::<T>(len_inbytes);

        let obj = unsafe { &mut *(ptr.as_ptr() as *mut NString) };
        obj.len_inbytes = len_inbytes;
        obj.len = str.chars().count();
        unsafe {
            let ptr = (obj as *mut NString).offset(1) as *mut u8;
            std::ptr::copy_nonoverlapping(str.as_bytes().as_ptr(), ptr, len_inbytes);
        }

        ptr.into_fptr()
    }

    #[inline]
    pub(crate) fn as_string(&self) -> StringRef {
        let ptr = self as *const NString;
        let str = unsafe {
            let ptr = ptr.offset(1) as *mut u8;
            String::from_raw_parts(ptr, self.len, self.len_inbytes)
        };

        StringRef::new(str)
    }

}

impl Eq for NString { }

impl PartialEq for NString {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        } else {
            *self.as_string() == *other.as_string()
        }
    }
}

impl PartialOrd for NString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (*self.as_string()).partial_cmp(&*(other.as_string()))
    }
}

impl Ord for NString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self.as_string()).cmp(&*(other.as_string()))
    }
}

fn display(this: &NString, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    //write!(f, "\"{}\"", &**(this.as_string()))

    Display::fmt( &(*this.as_string()), f)

}

impl Display for NString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}

impl Debug for NString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f)
    }
}

impl std::hash::Hash for NString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (*self.as_string()).hash(state);
    }
}

impl AsRef<[u8]> for NString {
    fn as_ref(&self) -> &[u8] {
        let ptr = self as *const NString;
        unsafe {
            let ptr = ptr.offset(1) as *mut u8;
            std::slice::from_raw_parts(ptr, self.len_inbytes)
        }
    }
}

impl AsRef<str> for NString {
    fn as_ref(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_ref()) }
    }
}