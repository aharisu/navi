use crate::value::*;
use crate::mm::{Heap};
use std::fmt::{self, Debug};

type StringRef = std::mem::ManuallyDrop<String>;

pub struct NString {
    len: usize,
    len_inbytes: usize,
}


impl NaviType for NString { }

static STRING_TYPEINFO: TypeInfo<NString> = TypeInfo::<NString> {
    name: "String",
    eq_func: NString::eq,
    print_func: NString::fmt,
    is_type_func: NString::is_type,
};

impl NString {

    #[inline(always)]
    pub fn typeinfo<'ti>() -> &'ti TypeInfo<NString> {
        &STRING_TYPEINFO
    }

    fn is_type(other_typeinfo: &TypeInfo<Value>) -> bool {
        std::ptr::eq(Self::typeinfo().cast(), other_typeinfo)
    }

    pub fn alloc<'ti>(heap : &'ti mut Heap, str: &String) -> NBox<NString> {
        Self::alloc_inner(heap, str, Self::typeinfo())
    }

    //NStringとSymbolクラス共有のアロケーション用関数。TはNSTringもしくはSymbolのみ対応。
    pub(crate) fn alloc_inner<'ti, T: NaviType>(heap : &'ti mut Heap, str: &String, typeinfo: &'ti TypeInfo<T>) -> NBox<T> {
        let len_inbytes = str.len();
        let nbox = heap.alloc_with_additional_size(typeinfo, len_inbytes);

        let obj = unsafe { &mut *(nbox.as_mut_ptr() as *mut NString) };
        obj.len_inbytes = len_inbytes;
        obj.len = str.chars().count();
        unsafe {
            let ptr = (obj as *mut NString).offset(1) as *mut u8;
            std::ptr::copy_nonoverlapping(str.as_bytes().as_ptr(), ptr, len_inbytes);
        }

        nbox
    }

    #[inline]
    fn as_string(&self) -> StringRef {
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

impl Debug for NString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (*self.as_string()).fmt(f)
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