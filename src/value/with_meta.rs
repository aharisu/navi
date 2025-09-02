use std::fmt::{self, Debug, Display};

use crate::err::*;
use crate::object::Object;
use crate::ptr::*;
use crate::value::{self, *};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    line: usize,
    column: usize,
    filename: String,
}

impl Meta {
    pub fn new(line: usize, column: usize, filename: String) -> Self {
        Meta {
            line,
            column,
            filename,
        }
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }
}

pub struct WithMeta {
    v: Ref<Any>,
    meta: Meta,
}

static WITH_META_TYPEINFO: TypeInfo = new_typeinfo!(
    WithMeta,
    "WithMeta",
    std::mem::size_of::<WithMeta>(),
    None,
    WithMeta::eq,
    WithMeta::clone_inner,
    Display::fmt,
    None,
    None,
    Some(WithMeta::is_comparable),
    Some(WithMeta::child_traversal),
    Some(WithMeta::check_reply),
    None,
);

impl NaviType for WithMeta {
    fn typeinfo() -> &'static TypeInfo {
        &WITH_META_TYPEINFO
    }

    fn clone_inner(&self, allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //clone_innerの文脈の中だけ、Ptrをキャプチャせずに扱うことが許されている
        unsafe {
            let v = crate::value::Any::clone_inner(self.v.as_ref(), allocator)?.into_reachable();

            Self::alloc(&v, self.meta.clone(), allocator)
        }
    }
}

impl WithMeta {
    pub fn v(&self) -> Ref<Any> {
        self.v.clone()
    }

    pub fn meta(&self) -> Meta {
        self.meta.clone()
    }

    fn child_traversal(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, arg: *mut u8)) {
        callback(&mut self.v, arg);
    }

    fn check_reply(cap: &mut Cap<WithMeta>, obj: &mut Object) -> Result<bool, OutOfMemory> {
        //vReplyを含んだ値がないかチェック
        if cap.as_ref().v.has_reply_type() {
            let mut v = cap.as_ref().v.clone().capture(obj);

            if value::check_reply(&mut v, obj)? {
                cap.as_mut().v.update_pointer(v.raw_ptr());
            } else {
                //Replyがまだ返信を受け取っていなかったのでfalseを返す
                return Ok(false);
            }
        }

        //内部にReply型を含まなくなったのでフラグを下す
        value::clear_has_reply_type_flag(cap.mut_refer());

        Ok(true)
    }

    pub fn alloc<A: Allocator>(
        v: &Reachable<Any>,
        meta: Meta,
        allocator: &mut A,
    ) -> NResult<WithMeta, OutOfMemory> {
        let ptr = allocator.alloc::<WithMeta>()?;
        unsafe {
            // 確保したメモリに値を書き込む
            std::ptr::write(
                ptr.as_ptr(),
                WithMeta {
                    v: v.raw_ptr().into(),
                    meta,
                },
            );
        }

        let mut result = ptr.into_ref();
        if v.has_reply_type() {
            //内部にReply型を含む場合はフラグを立てる
            value::set_has_reply_type_flag(&mut result);
        }

        Ok(result)
    }

    fn is_comparable(_other_typeinfo: &TypeInfo) -> bool {
        true
    }
}

impl PartialEq for WithMeta {
    fn eq(&self, other: &Self) -> bool {
        let other_typeinfo = get_typeinfo(other);
        // WithMetaの同値チェックはメタ情報を無視して、内部オブジェクトの同値チェックのみ行う
        if WithMeta::typeinfo() == other_typeinfo {
            self.v.as_ref() == other.v.as_ref()
        } else {
            self.v.as_ref() == Ref::new(other).cast_value().as_ref()
        }
    }
}

impl Display for WithMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(self.v.as_ref(), f)
    }
}

impl Debug for WithMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#<WithMeta meta: {:?} v: {:?}",
            self.meta,
            self.v.as_ref()
        )
    }
}
