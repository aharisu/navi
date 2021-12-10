use std::alloc;
use std::mem;
use crate::value::{self, TypeInfo, NaviType, NPtr, Value};
use crate::object::Object;
use crate::util::non_null_const::*;

//const POOL_SIZE : usize = 1024;

pub(crate) struct GCHeader {
    pub(crate) flags: usize,
    pub(crate) typeinfo: NonNullConst<TypeInfo>,
}

unsafe impl Sync for GCHeader {}

#[repr(C)]
pub(crate) struct GCAllocationStruct<T: NaviType> {
    pub(crate) header: GCHeader,
    pub(crate) value: T,
}

unsafe impl<T: NaviType> Sync for GCAllocationStruct<T> {}

impl <T: NaviType> GCAllocationStruct<T> {
    pub fn new(value: T) -> GCAllocationStruct<T> {
        GCAllocationStruct {
            header: GCHeader {
                flags: gc_flags_pack(std::mem::size_of::<T>() as u16, 0, false),
                typeinfo: T::typeinfo(),
            },
            value: value,
        }
    }
}

#[cfg(target_pointer_width="32")]
const SIZE_BIT_SHIFT: usize = 2;
#[cfg(target_pointer_width="64")]
const SIZE_BIT_SHIFT: usize = 3;



#[inline]
const fn gc_flags_pack(alloc_size: u16, forwarding_index: u16, alive: bool) -> usize {
    //Node: アドレスサイズが64bitの場合
    //alloc_sizeとforwading_indexは必ず8の倍数になる
    //容量圧縮のためにそれぞれ8で割った数をフラグ内に持つようにする。
    //8で割ることと同じ結果になる右に3シフトと本来シフトしたいbit幅の差分だけシフトして、フラグを構築する。

    // フラグ内のビット構造
    // sss ssss ssss pppp pppp pppa
    // s:11bit アロケーションしたサイズ / (8 or 4)
    // p:11bit GC時のCopy先アドレスインデックス
    // a 1bit GCで使用する到達可能フラグ
    ((alloc_size as usize) << (12 - SIZE_BIT_SHIFT)) |
    ((forwarding_index as usize) >> (SIZE_BIT_SHIFT - 1)) |
    (alive as usize)
}

#[allow(dead_code)]
#[inline]
const fn gc_flags_unpack(flags: usize) -> (u16, u16, bool) {
    (
        (flags >> (12 - SIZE_BIT_SHIFT) & 0x07FF) as u16, //allocation size 11bit
        (flags << (SIZE_BIT_SHIFT - 1) & 0x07FF) as u16, //forwarding index
        (flags & 1) == 1 //GC到達可能フラグ 1bit
        )
}

pub struct Heap {
    page_layout : alloc::Layout,
    pool_ptr : *mut u8,
    used : usize,
    freed : bool,

    //for debugging
    name: String
}

impl Heap {
    pub fn new<T: Into<String>>(page_size : usize, name: T) -> Self {
        let layout = alloc::Layout::from_size_align(page_size, mem::size_of::<usize>()).unwrap();

        let ptr = unsafe {
             alloc::alloc(layout)
        };

        let heap = Heap {
            page_layout: layout,
            pool_ptr: ptr,
            used: 0,
            freed: false,
            name: name.into(),
        };
        heap
    }

    pub fn alloc<T: NaviType>(&mut self, ctx: &Object) -> NPtr<T> {
        self.alloc_with_additional_size::<T>(0, ctx)
    }

    pub fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize, ctx: &Object) -> NPtr<T> {
        debug_assert!(!self.freed);

        let gc_header_size = mem::size_of::<GCHeader>();
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

        let mut try_count = 0;
        loop {
            if self.used + alloc_size < self.page_layout.size() {
                unsafe {
                    let gc_header_ptr = self.pool_ptr.add(self.used);
                    let gc_header = &mut *(gc_header_ptr as *mut GCHeader);

                    gc_header.flags = gc_flags_pack(alloc_size as u16, 0, false);
                    gc_header.typeinfo = T::typeinfo();

                    let obj_ptr = gc_header_ptr.add(gc_header_size) as *mut T;
                    //println!("[ptr {}] header:{:x} obj:{:x}", self.name, ptr_to_usize(gc_header_ptr), ptr_to_usize(obj_ptr));

                    self.used += alloc_size;

                    return NPtr::new(obj_ptr);
                }
            } else if try_count == 0 {
                self.gc(ctx);
                try_count += 1;

            } else {
                panic!("oom");
            }
        }
    }

    fn gc(&mut self, ctx: &Object) {
        self.mark_phase(ctx);
        self.setup_forwad_ptr(ctx);
        self.update_reference(ctx);
        self.move_object(ctx);
    }

    fn get_gc_header(v: &Value) -> &mut GCHeader {
        let ptr = v as *const Value as *const u8;
        unsafe {
            let ptr = ptr.sub(mem::size_of::<GCHeader>());
            &mut *(ptr as *const GCHeader as *mut GCHeader)
        }
    }

    fn is_marked(v: &Value) -> bool {
        let (_, _, marked) = gc_flags_unpack(Self::get_gc_header(v).flags);
        marked
    }

    fn is_need_mark(v: &Value) -> bool {
        value::value_is_pointer(v)
            && Self::is_marked(v) == false
    }

    fn mark(v: &Value) {
        let header = Self::get_gc_header(v);

        //対象オブジェクトに対して到達フラグを立てる
        let (size, _, _) = gc_flags_unpack(header.flags);
        header.flags = gc_flags_pack(size, 0, true);

        //対象オブジェクトが子オブジェクトを持っているなら、再帰的にマーク処理を行う
        if let Some(func) = unsafe { header.typeinfo.as_ref() }.child_traversal_func {
            func(v, 0, |child, arg| {
                let child = child.as_ref();
                if Self::is_need_mark(child) {
                    Self::mark(child);
                }
            });
        }
    }

    fn mark_phase(&mut self, ctx: &Object) {
        ctx.for_each_all_alived_value(|v| {
            let v = v.as_ref();
            if Self::is_need_mark(v) {
                Self::mark(v);
            }
        });
    }

    fn setup_forwad_ptr(&mut self, ctx: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            let mut forwarding_index:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let (size, _, marked) = gc_flags_unpack(header.flags);
                //生きているオブジェクトなら
                if marked {
                    //再配置される先のアドレス(スタート地点のポインタからのオフセット)をヘッダー内に一時保存する
                    header.flags = gc_flags_pack(size, forwarding_index as u16, true);
                    forwarding_index += size as usize / std::mem::size_of::<usize>();

                } else {
                    //マークがないオブジェクトは開放する
                    //TODO オブジェクトに対するファイナライザを実装する場合ここで実行する
                }


                ptr = ptr.add(size as usize);
            }
        }
    }

    fn update_reference(&mut self, ctx: &Object) {
        //生きているオブジェクトの内部で保持したままのアドレスを、
        //再配置後のアドレスで上書きする

        unsafe {
            let mut ptr = self.pool_ptr;
            let start = ptr_to_usize(ptr);
            let end = ptr.add(self.used);
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let (size, _, marked) = gc_flags_unpack(header.flags);
                //対象オブジェクトがまだ生きていて、
                if marked {
                    //内部で保持しているオブジェクトを持っている場合は
                    if let Some(func) = header.typeinfo.as_ref().child_traversal_func {
                        let v_ptr = ptr.add(std::mem::size_of::<GCHeader>());
                        let v = &*(v_ptr as *const Value);

                        func(v, start,|child, start| {
                            //子オブジェクトへのポインタを移動先の新しいポインタで置き換える
                            if value::value_is_pointer(child.as_ref()) {
                                let header = Self::get_gc_header(child.as_ref());
                                let (_, forwarding_index, _) = gc_flags_unpack(header.flags);
                                let new_ptr = (start as *mut u8).add(forwarding_index as usize) as *mut Value;
                                child.update_pointer(new_ptr);
                            }
                        });
                    }

                }

                ptr = ptr.add(size as usize);
            }
        }
    }

    fn move_object(&mut self, ctx: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let start = ptr;
            let end = ptr.add(self.used);

            let mut used:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let (size, forwarding_index, marked) = gc_flags_unpack(header.flags);
                //対象オブジェクトがまだ生きているなら
                if marked {
                    //GC中に使用したフラグをすべてリセット
                    header.flags = gc_flags_pack(size, 0, false);

                    let new_ptr = start.add(forwarding_index as usize);
                    //現在のポインタと新しい位置のポインタが変わっていたら
                    if ptr != new_ptr {
                        //新しい位置へデータをすべてコピー
                        std::ptr::copy(ptr, new_ptr, size as usize);
                    }

                    used += size as usize;
                }

                ptr = ptr.add(size as usize);
            }

            self.used = used;
        }
    }

    pub fn free(&mut self) {
        if self.freed == false {
            self.freed = true;

            unsafe {
                alloc::dealloc(self.pool_ptr, self.page_layout);
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

pub fn get_typeinfo<T: NaviType>(ptr: *const T) -> NonNullConst<TypeInfo> {
    let ptr = ptr as *const u8;
    let gc_header = unsafe {
        let gc_header_ptr = ptr.sub(mem::size_of::<GCHeader>());
        &*(gc_header_ptr as *const GCHeader)
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