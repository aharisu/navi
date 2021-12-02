use std::alloc;
use std::mem;
use crate::value::{TypeInfo, NaviType, NBox};

//const POOL_SIZE : usize = 1024;

pub(crate) struct GCHeader<'ti, T: NaviType> {
    pub(crate) flags: usize,
    pub(crate) typeinfo: &'ti TypeInfo<T>,
}

#[repr(C)]
pub(crate) struct GCAllocationStruct<T: 'static + NaviType> {
    pub(crate) header: GCHeader<'static, T>,
    pub(crate) value: T,
}

#[inline]
const fn gc_flags_pack(alloc_size: u16, marked: bool) -> usize {
    // フラグ内のビット構造
    // s ssss ssss ssss sssm
    // s:16bit アロケーションしたサイズ
    // m 1bit GCで使用するマーク済みフラグ
    ((alloc_size as usize) << 16) |
    (marked as usize)
}

#[allow(dead_code)]
#[inline]
const fn gc_flags_unpack(flags: usize) -> (u16, bool) {
    (
        (flags >> 16 & 0xFFFF) as u16, //allocation size 16bit
        (flags & 1) == 1 //marked marker 1bit
        )
}

pub struct Heap {
    page_layout : alloc::Layout,
    pool_ptr : usize,
    used : usize,
    freed : bool,

    //for debugging
    name: String
}

impl Heap {
    pub fn new(page_size : usize, name: String) -> Self {
        let layout = alloc::Layout::from_size_align(page_size, mem::size_of::<usize>()).unwrap();

        let ptr = unsafe {
             alloc::alloc(layout) as usize
        };

        let heap = Heap {
            page_layout: layout,
            pool_ptr: ptr,
            used: 0,
            freed: false,
            name: name,
        };
        heap
    }

    pub fn alloc<'ti, T: NaviType>(&'ti mut self, typeinfo : &'ti TypeInfo<T>) -> NBox<T> {
        self.alloc_with_additional_size::<T>(typeinfo, 0)
    }

    pub fn alloc_with_additional_size<'ti, T: NaviType>(&'ti mut self, typeinfo: &'ti TypeInfo<T>, additional_size: usize) -> NBox<T> {
        debug_assert!(!self.freed);

        let gc_header_size = mem::size_of::<GCHeader<T>>();
        let obj_size = std::mem::size_of::<T>();

        //TODO 確保するオブジェクトのサイズが16bit範囲内かをチェック
        //16bit以上のオブジェクト(64KB)は別に確保する。
        //別に確保しタオブジェクトはポインタ内のイミディエイト判別フラグで判断する。
        let need_size = gc_header_size + obj_size + additional_size;

        //確保するバイト数をアラインメントに沿うように切り上げる
        let aligh = self.page_layout.align();
        let aligned_size = (need_size + (aligh - 1)) / aligh * aligh;

        let alloc_size = aligned_size;

        println!("[alloc {}]struct:{}, add:{}, aligned:{}, alloc:{}, cur_used={}", self.name, obj_size, additional_size, aligned_size, alloc_size, self.used);

        if self.used + alloc_size < self.page_layout.size() {
            unsafe {
                let gc_header_ptr = (self.pool_ptr as *mut u8).add(self.used);
                let gc_header = &mut *(gc_header_ptr as *mut GCHeader<T>);

                gc_header.flags = gc_flags_pack(alloc_size as u16, false);
                gc_header.typeinfo = typeinfo;

                let obj_ptr = gc_header_ptr.add(gc_header_size) as *mut T;
                //println!("[ptr {}] header:{:x} obj:{:x}", self.name, ptr_to_usize(gc_header_ptr), ptr_to_usize(obj_ptr));

                self.used += alloc_size;

                return NBox::new(obj_ptr);
            }
        } else {
            //TODO GC

            panic!("oom");
        }
    }

    pub fn free(&mut self) {
        if self.freed == false {
            self.freed = true;

            unsafe {
                alloc::dealloc(self.pool_ptr as *mut u8, self.page_layout);
            }
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        if self.freed == false {
            panic!("[{}]heep leaked", self.name);
        }
    }
}

pub fn copy<T>(src: T, dest: &mut T) {
    debug_assert_eq!(std::mem::size_of_val(&src), std::mem::size_of_val(dest));

    unsafe {
        std::ptr::copy_nonoverlapping(&src as *const T, dest, std::mem::size_of::<T>());
    }

    //コピー先で値が生きているのでsrcは破棄されるが、dropはさせないようにする
    std::mem::forget(src);
}

pub fn get_typeinfo<'ti, T: NaviType>(ptr: *const T) -> &'ti TypeInfo<T> {
    let ptr = ptr as *const u8;
    let gc_header = unsafe {
        let gc_header_ptr = ptr.sub(mem::size_of::<GCHeader<T>>());
        &*(gc_header_ptr as *const GCHeader<T>)
    };
    gc_header.typeinfo
}

union PtrToUsize {
    ptr: *const u8,
    v: usize,
}

pub fn ptr_to_usize<T>(ptr: *const T) -> usize {
    let u = PtrToUsize {
        ptr: ptr as *const u8,
    };
    unsafe { u.v }
}


#[repr(transparent)]
pub struct RawRef<T: ?Sized> {
    value: T,
}

impl<T: ?Sized> RawRef<T> {
    #[inline(always)]
    pub const fn get_ptr(&self) -> *mut T {
        self as *const RawRef<T> as *mut T
    }

    #[inline(always)]
    pub fn get(&mut self) -> &mut T {
        &mut self.value
    }

    #[inline(always)]
    pub fn from_value<'a>(v: &'a T) -> &'a RawRef<T> {
        unsafe {
            &(*(v as *const T as *const RawRef<T>))
        }
    }

    #[inline(always)]
    pub fn from_value_mut<'a>(v: &'a mut T) -> &'a mut RawRef<T> {
        unsafe {
            &mut (*(v as *mut T as *mut RawRef<T>))
        }
    }
}