use std::fmt::{self, Debug, Display};

use crate::err::*;
use crate::object::Object;
use crate::ptr::*;
use crate::value::{self, *};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Meta {
    line: usize,
    column: usize,
    filename: String,
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
    None,
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

            Self::alloc(&v, allocator)
        }
    }
}

impl WithMeta {
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
        allocator: &mut A,
    ) -> NResult<WithMeta, OutOfMemory> {
        let ptr = allocator.alloc::<WithMeta>()?;
        unsafe {
            // 確保したメモリに値を書き込む
            std::ptr::write(
                ptr.as_ptr(),
                WithMeta {
                    v: v.raw_ptr().into(),
                    meta: Meta {
                        line: 0,
                        column: 0,
                        filename: "".to_string(),
                    },
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
}

impl Eq for WithMeta {}

impl PartialEq for WithMeta {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self as *const WithMeta, other as *const WithMeta) {
            true
        } else {
            // メタ情報が同一であれば同一とみなす
            self.meta == other.meta
        }
    }
}

fn display(this: &WithMeta, f: &mut fmt::Formatter<'_>, is_debug: bool) -> fmt::Result {
    if is_debug {
        write!(
            f,
            "#<WithMeta meta: {:?} v: {:?}",
            this.meta,
            this.v.as_ref()
        )
    } else {
        write!(f, "#<WithMeta {}> ", this.v.as_ref())
    }
}

impl Display for WithMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, false)
    }
}

impl Debug for WithMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self, f, true)
    }
}
