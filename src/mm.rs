use std::alloc;
use std::mem;
use crate::object::Object;
use crate::ptr::*;
use crate::value::{self, TypeInfo, NaviType, Value};
use crate::util::non_null_const::*;

//const POOL_SIZE : usize = 1024;

pub(crate) struct GCHeader {
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

#[derive(Copy, Clone)]
enum HeapSize {
    _32k
}

pub struct Heap {
    pool_ptr : *mut u8,
    used : usize,
    page_layout : alloc::Layout,
    heap_size: HeapSize,
}

struct GCTempArg<'a> {
    start_addr: *const u8,
    end_addr: *const u8,
    flags: &'a mut [u16],
}

impl <'a> GCTempArg<'a> {
    pub fn new(start_addr: *const u8, end_addr: *const u8, flags: &'a mut [u16]) -> Self {
        GCTempArg {
            start_addr: start_addr,
            end_addr: end_addr,
            flags: flags,
        }
    }

    pub fn as_ptr(&mut self) -> *mut u8 {
        self as *mut GCTempArg as *mut u8
    }

    pub unsafe fn from_ptr(ptr: *mut u8) -> &'a mut Self {
        &mut *(ptr as *mut GCTempArg)
    }
}


impl Heap {
    pub fn new() -> Self {
        let heapsize = HeapSize::_32k;
        let layout = Self::alloc_layout(heapsize);

        let ptr = unsafe {
             alloc::alloc(layout)
        };

        let heap = Heap {
            pool_ptr: ptr,
            used: 0,
            page_layout: layout,
            heap_size: heapsize,
        };
        heap
    }

    fn alloc_layout(heapsize: HeapSize) -> alloc::Layout {
        let size = match heapsize {
            _32k => 1024usize * 32,
        };

        alloc::Layout::from_size_align(size, VALUE_ALIGN).unwrap()
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

                let size_ptr = (&size) as *const usize as *mut u8;
                func(v, size_ptr, |child, size_ptr| {
                    let size_ref = unsafe { &mut *(size_ptr as *mut usize) };
                    let child_size = Self::calc_total_size(child.as_ref());
                    //Typeinfoの実装上の制限でClosureを渡すことができない。
                    //全てのオブジェクトのサイズを合算するために、参照で渡した変数の領域に無理やり書き込む。
                    unsafe {
                        std::ptr::write(size_ref as *mut usize, child_size + *size_ref);
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
                let v_ptr = ptr.add(std::mem::size_of::<GCHeader>());
                let v = &*(v_ptr as *const Value);

                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());
                println!("[dump] {:<8}, size:{}, ptr:{:x}, {:?}",
                    header.typeinfo.as_ref().name,
                    size,
                    ptr.offset_from(self.pool_ptr),
                    v
                );

                ptr = ptr.add(size as usize);
            }
        }

        println!("[dump] **** end ****");
    }

    pub(crate) fn gc(&mut self, obj: &Object) {
        match self.heap_size {
            HeapSize::_32k => self.gc_32k(obj)
        };
    }

    fn gc_32k(&mut self, obj: &Object) {
        //32kサイズのヒープ内に存在する可能性がある値すべてのGCフラグを保持できる大きさの配列
        //※最後のシフトは8,または16で割り算することと同じ意味。
        //値の最低サイズは32bitOSなら8、64bitOSなら16なのでそれぞれの数字で割る。
        let mut flags = [0u16; 1024 * 32 >> SIZE_BIT_SHIFT];

        self.mark_phase(&mut flags, obj);
        self.setup_forwad_ptr(&mut flags, obj);
        self.update_reference(&mut flags, obj);
        self.move_object(&mut flags, obj);
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

    fn is_need_mark(v: &Value, arg: &GCTempArg) -> bool {
        //Immidiate Valueの場合があるため正しくポインタであるかを確認
        if value::value_is_pointer(v) {
            //値を指している参照から、GCHeaderを指しているポインタに変換
            let alloc_ptr = unsafe {
                let ptr = v as *const Value as *const u8;
                ptr.sub(mem::size_of::<GCHeader>())
            };

            //funcやsyntaxなど、ヒープ外のstaticな領域に確保された値の可能性があるのでチェック
            if arg.start_addr <= alloc_ptr && alloc_ptr < arg.end_addr {
                let (alive, _) = Self::get_gc_flag(arg.start_addr, alloc_ptr, arg.flags);

                //まだ生存フラグを立てていなければ、マークが必要
                alive == false
            } else {
                false
            }
        } else {
            false
        }
    }

    fn set_gc_flag(alive: bool, forwarding_index: usize, start_ptr: *const u8, alloc_ptr: *const u8, flags: &mut [u16]) {
        //ヒープの開始位置からのオフセットを取得
        let offset = unsafe { alloc_ptr.offset_from(start_ptr) };
        //値は最低でも8、もしくは16Byteある。
        //offsetを値ごとのインデックスにするために8か16で割り算するためにシフトする。
        let offset = offset >> SIZE_BIT_SHIFT;

        //15bit forwarding index
        //1bit  alive?
        //※forwarding indexは必ず8の倍数になっている(32bitOSの値の最低サイズが8)。
        //8の倍数なら下位3bitは0のため、aliveフラグのための最下位1bitを除く2bit分詰めることで容量を稼ぐ。
        //forwarding inddexとして扱える値の幅は実質17bit分になる。
        let flag = (forwarding_index >> 2) | (alive as usize);
        flags[offset as usize] = flag as u16;
    }

    fn get_gc_flag(start_ptr: *const u8, alloc_ptr: *const u8, flags: &[u16]) -> (bool, usize) {
        //ヒープの開始位置からのオフセットを取得
        let offset = unsafe { alloc_ptr.offset_from(start_ptr) };
        //値は最低でも8、もしくは16Byteある。
        //offsetを値ごとのインデックスにするために8か16で割り算するためにシフトする。
        let offset = offset >> SIZE_BIT_SHIFT;

        let flag = flags[offset as usize];
        (
            (flag & 1) == 1, //GC到達可能フラグ 1bit
            ((flag & 0xFFFE) << 2) as usize, // forwarding index 15bit
        )
    }

    fn mark(v: &Value, arg: &mut GCTempArg) {
        //値を指している参照から、GCHeaderを指しているポインタに変換
        let alloc_ptr = unsafe {
            let ptr = v as *const Value as *const u8;
            ptr.sub(mem::size_of::<GCHeader>())
        };
        //対象オブジェクトに対して生存フラグを立てる
        Self::set_gc_flag(true, 0, arg.start_addr, alloc_ptr, arg.flags);

        //対象オブジェクトが子オブジェクトを持っているなら、再帰的にマーク処理を行う
        let header = unsafe { & *(alloc_ptr as *const GCHeader as *mut GCHeader) };
        let typeinfo = unsafe { header.typeinfo.as_ref() };
        if let Some(func) = typeinfo.child_traversal_func {
            //Typeinfoの実装の都合上、クロージャを渡すことができないので、無理やりポインタを経由して値を渡す
            func(v, arg.as_ptr(), |child, arg_ptr| {
                let arg = unsafe { GCTempArg::from_ptr(arg_ptr) };

                let child = child.as_ref();
                if Self::is_need_mark(child, arg) {
                    Self::mark(child, arg);
                }
            });
        }
    }

    fn mark_phase(&mut self, flags: &mut [u16], obj: &Object) {
        let mut arg = unsafe {
            GCTempArg::new(self.pool_ptr, self.pool_ptr.add(self.used), flags)
        };

        //Typeinfoの実装の都合上、クロージャを渡すことができないので、無理やりポインタを経由して値を渡す
        obj.for_each_all_alived_value(arg.as_ptr(), |v, arg_ptr| {
            let arg = unsafe { GCTempArg::from_ptr(arg_ptr) };

            let v = v.as_ref();
            if Self::is_need_mark(v, arg) {
                Self::mark(v, arg);
            }
        });
    }

    fn setup_forwad_ptr(&self, flags: &mut [u16], _obj: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            let mut forwarding_index:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &mut *(ptr.add(std::mem::size_of::<GCHeader>()) as *mut Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (alive, _) = Self::get_gc_flag(self.pool_ptr, ptr, flags);

                //生きているオブジェクトなら
                if alive {
                    //再配置される先のアドレス(スタート地点のポインタからのオフセット)をフラグ内に保存する
                    Self::set_gc_flag(true, forwarding_index, self.pool_ptr, ptr, flags);
                    forwarding_index += size as usize;

                } else {
                    //マークがないオブジェクトは開放する
                    if let Some(finalize) = header.typeinfo.as_ref().finalize {
                        finalize(v);
                    }
                }


                ptr = ptr.add(size as usize);
            }
        }
    }

    fn update_reference(&mut self, flags: &mut [u16], obj: &Object) {
        //生きているオブジェクトの内部で保持したままのアドレスを、
        //再配置後のアドレスで上書きする

        fn update_child_pointer(child: &RPtr<Value>, arg_ptr: *mut u8) {
            let arg = unsafe { GCTempArg::from_ptr(arg_ptr) };

            //子オブジェクトへのポインタを移動先の新しいポインタで置き換える
            if value::value_is_pointer(child.as_ref()) {
                //値を指している参照から、GCHeaderを指しているポインタに変換
                let alloc_ptr = unsafe {
                    let ptr = child.as_ref() as *const Value as *const u8;
                    ptr.sub(mem::size_of::<GCHeader>())
                };

                //funcやsyntaxなど、ヒープ外のstaticな領域に確保された値の可能性があるのでチェック
                if arg.start_addr <= alloc_ptr && alloc_ptr < arg.end_addr {
                    let (_, forwarding_index) = crate::mm::Heap::get_gc_flag(arg.start_addr, alloc_ptr, arg.flags);

                    //子オブジェクトが移動しているなら移動先のポインタを参照するように更新する
                    let offset = forwarding_index + std::mem::size_of::<GCHeader>();
                    let new_ptr = unsafe { arg.start_addr.add(offset) } as *mut Value;

                    child.update_pointer(new_ptr);
                }
            }
        }

        unsafe {
            let mut ptr = self.pool_ptr;
            let end = ptr.add(self.used);

            let mut arg = GCTempArg::new(ptr, end, flags);

            //ルートオブジェクトとして保持されているオブジェクト内のポインタを更新
            obj.for_each_all_alived_value(arg.as_ptr(), update_child_pointer);

            //ヒープ内のオブジェクト内のポインタを更新
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &*(ptr.add(std::mem::size_of::<GCHeader>()) as *const Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (alive, _) = Self::get_gc_flag(arg.start_addr, ptr, arg.flags);
                //対象オブジェクトがまだ生きていて、
                if alive {
                    //内部で保持しているオブジェクトを持っている場合は
                    if let Some(func) = header.typeinfo.as_ref().child_traversal_func {
                        func(v, arg.as_ptr(), update_child_pointer);
                    }
                }

                ptr = ptr.add(size as usize);
            }
        }
    }

    fn move_object(&mut self, flags: &[u16], _obj: &Object) {
        unsafe {
            let mut ptr = self.pool_ptr;
            let start = ptr;
            let end = ptr.add(self.used);

            let mut used:usize = 0;
            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v = &*(ptr.add(std::mem::size_of::<GCHeader>()) as *const Value);
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());

                let (alive, forwarding_index) = Self::get_gc_flag(start, ptr, flags);

                //対象オブジェクトがまだ生きているなら
                if alive {
                    //現在のポインタと新しい位置のポインタが変わっていたら
                    let new_ptr = start.add(forwarding_index as usize);
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
    use crate::value::*;

    #[test]
    fn gc_test() {
        let mut obj = Object::new();
        let obj = &mut obj;

        {
            let _1 = number::Integer::alloc(1, obj).into_value().capture(obj);
            {
                let _2 = number::Integer::alloc(2, obj).into_value().capture(obj);
                let _3 = number::Integer::alloc(3, obj).into_value().capture(obj);

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
