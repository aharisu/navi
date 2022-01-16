use crate::value::*;
use crate::ptr::*;
use core::panic;
use std::fmt::{self, Debug, Display};
use std::io::Read;

type StringRef = std::mem::ManuallyDrop<String>;

#[repr(C)]
pub struct NString {
    len_inbytes: usize,
}

static STRING_TYPEINFO: TypeInfo = new_typeinfo!(
    NString,
    "String",
    0,
    Some(NString::size_of),
    NString::eq,
    NString::clone_inner,
    Display::fmt,
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for NString {
    fn typeinfo() -> &'static TypeInfo {
        &STRING_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        Self::alloc(&self.as_string(), allocator)
    }
}

impl NString {
    pub(crate) fn size_of(&self) -> usize {
        std::mem::size_of::<NString>() + self.len_inbytes
    }

    pub fn alloc<A: Allocator>(str: &String, allocator : &mut A) -> NResult<NString, OutOfMemory> {
        Self::alloc_inner(str, allocator)
    }

    //NStringとSymbol,Keywordクラス共有のアロケーション用関数。TはNSTringもしくはSymbol、Keywordのみ対応。
    pub(crate) fn alloc_inner<T: NaviType, A: Allocator>(str: &String, allocator : &mut A) -> NResult<T, OutOfMemory> {
        let len_inbytes = str.len();
        let ptr = allocator.alloc_with_additional_size::<T>(len_inbytes)?;

        let nstring = unsafe { &mut *(ptr.as_ptr() as *mut NString) };
        nstring.len_inbytes = len_inbytes;
        unsafe {
            let ptr = (nstring as *mut NString).offset(1) as *mut u8;
            std::ptr::copy_nonoverlapping(str.as_bytes().as_ptr(), ptr, len_inbytes);
        }

        Ok(ptr.into_ref())
    }

    #[inline]
    pub(crate) fn as_string(&self) -> StringRef {
        let ptr = self as *const NString;
        let str = unsafe {
            let ptr = ptr.offset(1) as *mut u8;
            String::from_raw_parts(ptr, self.len_inbytes, self.len_inbytes)
        };

        StringRef::new(str)
    }

}

impl Eq for NString { }

impl PartialEq for NString {
    fn eq(&self, other: &Self) -> bool {
        if self.len_inbytes != other.len_inbytes {
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

#[repr(C)]
pub struct StaticString {
    v: NString,
    buf: [u8; 10],
}

pub fn static_string<T: Into<String>>(str: T) -> StaticString {
    let str = str.into();
    let len_inbytes = str.len();
    if str.len() > 10 {
        panic!("static string up to 10 bytes");
    }

    let mut static_str = StaticString {
        v: NString {
            len_inbytes: len_inbytes
        },
        buf: Default::default()
    };

    (&str.as_bytes()[..]).read(&mut static_str.buf).unwrap();

    static_str
}