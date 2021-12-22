use std::alloc;
use std::mem;
use crate::object::Object;
use crate::ptr::*;
use crate::value::{self, TypeInfo, NaviType, Value};
use crate::util::non_null_const::*;

//const POOL_SIZE : usize = 1024;

pub(crate) struct GCHeader {
    pub(crate) flags: usize,
    pub(crate) typeinfo: NonNullConst<TypeInfo>,
}

unsafe impl Sync for GCHeader {}

#[repr(C)]
pub(crate) struct GCAllocationStruct<T> {
    pub(crate) header: GCHeader,
    pub(crate) value: T,
}

unsafe impl<T> Sync for GCAllocationStruct<T> {}

impl <T: NaviType> GCAllocationStruct<T> {
    pub fn new(value: T) -> GCAllocationStruct<T> {
        GCAllocationStruct {
            header: GCHeader {
                flags: gc_flags_pack(0, false, false),
                typeinfo: T::typeinfo(),
            },
            value: value,
        }
    }
}

impl <T> GCAllocationStruct<T> {
    pub fn new_with_typeinfo(value: T, typeinfo: NonNullConst<TypeInfo>) -> GCAllocationStruct<T> {
        GCAllocationStruct {
            header: GCHeader {
                flags: gc_flags_pack(0, false, false),
                typeinfo: typeinfo,
            },
            value: value,
        }
    }
}

#[cfg(target_pointer_width="32")]
const SIZE_BIT_SHIFT: usize = 2;
#[cfg(target_pointer_width="64")]
const SIZE_BIT_SHIFT: usize = 3;

const VALUE_ALIGN:usize = mem::size_of::<usize>();

#[inline]
const fn gc_flags_pack(forwarding_index: u16, need_move: bool, alive: bool) -> usize {
    //Node: アドレスサイズが64bitの場合
    //alloc_sizeとforwading_indexは必ず8の倍数になる
    //容量圧縮のためにそれぞれ8で割った数をフラグ内に持つようにする。
    //8で割ることと同じ結果になる右に3シフトと本来シフトしたいbit幅の差分だけシフトして、フラグを構築する。

    // p:13bit GC時のCopy先アドレスインデックス
    // m 1bit GC時に使用する移動が必要かどうかのフラグ
    // a 1bit GCで使用する到達可能フラグ
    ((forwarding_index as usize) << (16 - SIZE_BIT_SHIFT)) |
    (need_move as usize) << 1|
    (alive as usize)
}

#[inline]
const fn gc_flags_unpack(flags: usize) -> (u16, bool, bool) {
    (
        ((flags & 0x1FFF_0000) >> (16 - SIZE_BIT_SHIFT)) as u16, //forwarding index
        (flags & 2) == 2, //GC時に使用する移動が必要かどうかのフラグ
        (flags & 1) == 1 //GC到達可能フラグ 1bit
        )
}

pub struct Heap {
    page_layout : alloc::Layout,
    pool_ptr : *mut u8,
    used : usize,
}

impl Heap {
    pub fn new(page_size : usize) -> Self {
        let layout = alloc::Layout::from_size_align(page_size, VALUE_ALIGN).unwrap();

        let ptr = unsafe {
             alloc::alloc(layout)
        };

        let heap = Heap {
            page_layout: layout,
            pool_ptr: ptr,
            used: 0,
        };
        heap
    }

    pub fn alloc<T: NaviType>(&mut self, obj: &Object) -> UIPtr<T> {
        self.alloc_with_additional_size::<T>(0, obj)
    }

    pub fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize, obj: &Object) -> UIPtr<T> {
        //GCのバグを発見しやすいように、allocのたびにGCを実行する
        self.debug_gc(obj);

        let gc_header_size = mem::size_of::<GCHeader>();
        let obj_size = std::mem::size_of::<T>();

        //TODO 確保するオブジェクトのサイズが16bit範囲内かをチェック
        //16bit以上のオブジェクト(64KB)は別に確保する。
        //別に確保しタオブジェクトはポインタ内のイミディエイト判別フラグで判断する。
        let need_size = gc_header_size + obj_size + additional_size;

        //確保するバイト数をアラインメントに沿うように切り上げる
        let aligned_size = (need_size + (VALUE_ALIGN - 1)) / VALUE_ALIGN * VALUE_ALIGN;

        let alloc_size = aligned_size;

        let mut try_count = 0;
        loop {
            if self.used + alloc_size < self.page_layout.size() {
                unsafe {
                    let gc_header_ptr = self.pool_ptr.add(self.used);
                    let gc_header = &mut *(gc_header_ptr as *mut GCHeader);

                    gc_header.flags = gc_flags_pack(0, false, false);
                    gc_header.typeinfo = T::typeinfo();

                    let obj_ptr = gc_header_ptr.add(gc_header_size) as *mut T;

                    self.used += alloc_size;

                    return UIPtr::new(obj_ptr);
                }
            } else if try_count == 0 {
                self.gc(obj);
                try_count += 1;

            } else {
                self.dump_heap(obj);

                panic!("oom");
            }
        }
    }

    pub fn calc_total_size(v: &Value) -> usize {
        if value::value_is_pointer(v) {
            let header = Self::get_gc_header(v);
            let typeinfo = unsafe { header.typeinfo.as_ref() };

            let size = Self::get_allocation_size(v, typeinfo);
            if let Some(func) = typeinfo.child_traversal_func {
                func(v, &size, |child, total_size| {
                    let size = Self::calc_total_size(child.as_ref());
                    //Typeinfoの実装上の制限でClosureを渡すことができない。
                    //全てのオブジェクトのサイズを合算するために、参照で渡した変数の領域に無理やり書き込む。
                    unsafe {
                        std::ptr::write(total_size as *const usize as *mut usize, size + total_size);
                    }
                });
            }

            size
        } else {
            0
        }
    }

    pub fn force_allocation_space(&mut self, require_size: usize, obj: &Object) {
        let mut try_count = 0;
        loop {
            if self.used + require_size < self.page_layout.size() {
                return //OK!!
            } else if try_count == 0 {
                self.gc(obj);
                try_count += 1;

            } else {
                self.dump_heap(obj);

                panic!("oom");
            }
        }
    }

    pub fn used(&self) -> usize {
        self.used
    }

    fn debug_gc(&mut self, obj: &Object) {
        self.gc(obj);

        //ダングリングポインタを発見しやすくするために未使用の領域を全て0埋め
        unsafe {
            let ptr = self.pool_ptr.add(self.used);
            std::ptr::write_bytes(ptr, 0, self.page_layout.size() - self.used);
        }

        //self.dump_heap(obj);
    }

    pub fn dump_heap(&self, _obj: &Object) {
        println!("[dump]------------------------------------");

        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let (forwarding_index, need_move, marked) = gc_flags_unpack(header.flags);
                let v_ptr = ptr.add(std::mem::size_of::<GCHeader>());
                let v = &*(v_ptr as *const Value);

                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());
                println!("[dump] {:<8}, size:{}, mark:{}, forwarding:{:x}, need_move:{}, ptr:{:x}, {:?}",
                    header.typeinfo.as_ref().name,
                    size,
                    marked,
                    forwarding_index,
                    need_move,
                    ptr.offset_from(self.pool_ptr),
                    v
                );

                ptr = ptr.add(size as usize);
            }
        }

        println!("[dump] **** end ****");
    }

    pub(crate) fn gc(&mut self, obj: &Object) {
        self.mark_phase(obj);
        self.setup_forwad_ptr(obj);
        self.update_reference(obj);
        self.move_object(obj);
    }

    fn get_allocation_size(v: &Value, typeinfo: &TypeInfo) -> usize {
        let val_size = if let Some(size_of_func) = typeinfo.variable_size_func {
            let size = size_of_func(v);
            (size + (VALUE_ALIGN - 1)) / VALUE_ALIGN * VALUE_ALIGN
        } else {
            typeinfo.fixed_size
        };
        val_size + std::mem::size_of::<GCHeader>()
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
        let typeinfo = unsafe { header.typeinfo.as_ref() };

        //対象オブジェクトに対して到達フラグを立てる
        header.flags = gc_flags_pack(0,false, true);

        //対象オブジェクトが子オブジェクトを持っているなら、再帰的にマーク処理を行う
        if let Some(func) = typeinfo.child_traversal_func {
            let dummy:usize = 0;
            func(v, &dummy, |child, _| {
                let child = child.as_ref();
                if Self::is_need_mark(child) {
                    Self::mark(child);
                }
            });
        }
    }

    fn mark_phase(&mut self, obj: &Object) {
        let dummy:usize = 0;
        obj.for_each_all_alived_value(&dummy, |v, _| {
            let v = v.as_ref();
            if Self::is_need_mark(v) {
                Self::mark(v);
            }
        });
    }

    fn setup_forwad_ptr(&mut self, _obj: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            let mut is_moving = false;
            let mut forwarding_index:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &mut *(ptr.add(std::mem::size_of::<GCHeader>()) as *mut Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (_, _, marked) = gc_flags_unpack(header.flags);
                //生きているオブジェクトなら
                if marked {
                    //再配置される先のアドレス(スタート地点のポインタからのオフセット)をヘッダー内に一時保存する
                    if is_moving {
                        header.flags = gc_flags_pack(forwarding_index as u16, true, true);
                    }
                    forwarding_index += size as usize;

                } else {
                    //マークがないオブジェクトは開放する
                    if let Some(finalize) = header.typeinfo.as_ref().finalize {
                        finalize(v);
                    }

                    //解放するオブジェクトが一つでも見つかったら、それ以降のオブジェクトは移動される
                    is_moving = true;
                }


                ptr = ptr.add(size as usize);
            }
        }
    }

    fn update_reference(&mut self, obj: &Object) {
        //生きているオブジェクトの内部で保持したままのアドレスを、
        //再配置後のアドレスで上書きする

        fn update_child_pointer(child: &RPtr<Value>, start_addr: &usize) {
            //子オブジェクトへのポインタを移動先の新しいポインタで置き換える
            if value::value_is_pointer(child.as_ref()) {
                let header = crate::mm::Heap::get_gc_header(child.as_ref());
                let (forwarding_index, need_move, _) = gc_flags_unpack(header.flags);

                //子オブジェクトが移動しているなら移動先のポインタを参照するように更新する
                if need_move {
                    let start_addr = (*start_addr) as *mut u8;
                    let offset = forwarding_index as usize + std::mem::size_of::<GCHeader>();
                    let new_ptr = unsafe { start_addr.add(offset) } as *mut Value;


                    child.update_pointer(new_ptr);
                }
            }
        }

        unsafe {
            let mut ptr = self.pool_ptr;
            let start = ptr_to_usize(ptr);

            obj.for_each_all_alived_value(&start, update_child_pointer);

            let end = ptr.add(self.used);
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &*(ptr.add(std::mem::size_of::<GCHeader>()) as *const Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (_, _, marked) = gc_flags_unpack(header.flags);
                //対象オブジェクトがまだ生きていて、
                if marked {
                    //内部で保持しているオブジェクトを持っている場合は
                    if let Some(func) = header.typeinfo.as_ref().child_traversal_func {
                        func(v, &start, update_child_pointer);
                    }

                }

                ptr = ptr.add(size as usize);
            }
        }
    }

    fn move_object(&mut self, _obj: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let start = ptr;
            let end = ptr.add(self.used);

            let mut used:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &*(ptr.add(std::mem::size_of::<GCHeader>()) as *const Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (forwarding_index, need_move, marked) = gc_flags_unpack(header.flags);
                //対象オブジェクトがまだ生きているなら
                if marked {
                    //GC中に使用したフラグをすべてリセット
                    header.flags = gc_flags_pack(0, false, false);

                    //現在のポインタと新しい位置のポインタが変わっていたら
                    if need_move {
                        let new_ptr = start.add(forwarding_index as usize);

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
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let typeinfo = header.typeinfo.as_ref();

                let v = &mut *(ptr.add(std::mem::size_of::<GCHeader>()) as *mut Value);
                let size = Self::get_allocation_size(v, typeinfo);

                if let Some(finalize) = typeinfo.finalize {
                    finalize(v);
                }

                ptr = ptr.add(size as usize);
            }

            alloc::dealloc(self.pool_ptr, self.page_layout);
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

#[cfg(test)]
mod tests {
    use crate::object::Object;
    use crate::{let_cap, new_cap};
    use crate::value::*;

    #[test]
    fn gc_test() {
        let mut obj = Object::new();
        let obj = &mut obj;

        {
            let_cap!(_1, number::Integer::alloc(1, obj).into_value(), obj);
            {
                let_cap!(_2, number::Integer::alloc(2, obj).into_value(), obj);
                let_cap!(_3, number::Integer::alloc(3, obj).into_value(), obj);

                obj.do_gc();
                let used = (std::mem::size_of::<crate::mm::GCHeader>() + std::mem::size_of::<number::Integer>()) * 3;
                assert_eq!(obj.heap_used(), used);
            }

            obj.do_gc();
            let used = (std::mem::size_of::<crate::mm::GCHeader>() + std::mem::size_of::<number::Integer>()) * 1;
            assert_eq!(obj.heap_used(), used);
        }

        obj.do_gc();
        assert_eq!(obj.heap_used(), 0);
    }
}
