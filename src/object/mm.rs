use std::alloc;
use std::mem;
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

#[derive(Debug, Copy, Clone)]
enum HeapSize {
    _256,
    _512,
    _1k,
    _2k,
    _8k,
    _16k,
    _32k,
}

pub enum StartHeapSize {
    Default,
    Small,
}

pub trait GCRootValueHolder {
    fn for_each_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8));
}

pub struct Heap {
    pool_ptr : *mut u8,
    used : usize,
    page_layout : alloc::Layout,
    heap_size: HeapSize,
}

struct GCCompactionArg<'a> {
    start_addr: *const u8,
    end_addr: *const u8,
    flags: &'a mut [u16],
}

impl <'a> GCCompactionArg<'a> {
    pub fn new(start_addr: *const u8, end_addr: *const u8, flags: &'a mut [u16]) -> Self {
        GCCompactionArg {
            start_addr: start_addr,
            end_addr: end_addr,
            flags: flags,
        }
    }

    pub fn as_ptr(&mut self) -> *mut u8 {
        self as *mut GCCompactionArg as *mut u8
    }

    pub unsafe fn from_ptr(ptr: *mut u8) -> &'a mut Self {
        &mut *(ptr as *mut GCCompactionArg)
    }
}

struct GCCopyingArg {
    start_addr: *const u8,
    end_addr: *const u8,
    free_ptr: *mut u8,
    used: usize,
}

impl GCCopyingArg {
    pub fn new(start_addr: *const u8, end_addr: *const u8, free_ptr: *mut u8) -> Self {
        GCCopyingArg {
            start_addr: start_addr,
            end_addr: end_addr,
            free_ptr: free_ptr,
            used: 0,
        }
    }

    pub fn as_ptr(&mut self) -> *mut u8 {
        self as *mut Self as *mut u8
    }

    pub unsafe fn from_ptr<'a>(ptr: *mut u8) -> &'a mut Self {
        &mut *(ptr as *mut Self)
    }
}

#[repr(C)]
struct CopiedValue {
    mark: usize,
    forwarding_pointer: *mut u8,
}

impl CopiedValue {
    pub unsafe fn is_copied(this: *mut Self) -> bool {
        (*this).mark == value::IMMIDATE_GC_COPIED
    }

    pub unsafe fn forwarding_pointer(this: *mut Self) -> *mut u8 {
        (*this).forwarding_pointer
    }

    pub unsafe fn mark_copied(this: *mut Self, forwarding_pointer: *mut u8) {
        std::ptr::write(this, CopiedValue {
            mark: value::IMMIDATE_GC_COPIED,
            forwarding_pointer: forwarding_pointer,
        });
    }

}

impl Heap {
    pub fn new(startsize: StartHeapSize) -> Self {
        let heapsize = match startsize {
            StartHeapSize::Default => HeapSize::_2k,
            StartHeapSize::Small => HeapSize::_256,
        };

        let layout = Self::get_alloc_layout(heapsize);
        let ptr = unsafe { alloc::alloc(layout) };

        let heap = Heap {
            pool_ptr: ptr,
            used: 0,
            page_layout: layout,
            heap_size: heapsize,
        };
        heap
    }

    fn get_alloc_layout(heapsize: HeapSize) -> alloc::Layout {
        let size = match heapsize {
            HeapSize::_256 => 256usize,
            HeapSize::_512 => 512usize,
            HeapSize::_1k => 1024usize * 1,
            HeapSize::_2k => 1024usize * 2,
            HeapSize::_8k => 1024usize * 8,
            HeapSize::_16k => 1024usize * 16,
            HeapSize::_32k => 1024usize * 32,
        };

        alloc::Layout::from_size_align(size, VALUE_ALIGN).unwrap()
    }

    pub fn alloc<T: NaviType, R: GCRootValueHolder>(&mut self, root: &R) -> UIPtr<T> {
        self.alloc_with_additional_size::<T, R>(0, root)
    }

    pub fn alloc_with_additional_size<T: NaviType, R: GCRootValueHolder>(&mut self, additional_size: usize, root: &R) -> UIPtr<T> {
        //GCのバグを発見しやすいように、allocのたびにGCを実行する
        //self.debug_gc(obj);

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
                self.gc(root);
                try_count += 1;

            } else {
                self.dump_heap();

                panic!("oom");
            }
        }
    }

    fn get_gc_header(v: &Value) -> &mut GCHeader {
        let ptr = v as *const Value as *const u8;
        unsafe {
            let ptr = ptr.sub(mem::size_of::<GCHeader>());
            &mut *(ptr as *const GCHeader as *mut GCHeader)
        }
    }

    pub fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool {
        let v: &Value = unsafe { std::mem::transmute(v) };

        //ポインタかつ、自分自身のヒープ内に存在するオブジェクトなら、有効な値。
        value::value_is_pointer(v)
            && Self::is_pointer_within_heap(v, self.pool_ptr, unsafe { self.pool_ptr.add(self.used) })
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

    pub fn force_allocation_space<R: GCRootValueHolder>(&mut self, require_size: usize, root: &R) {
        let mut try_count = 0;
        loop {
            if self.used + require_size < self.page_layout.size() {
                return //OK!!
            } else if try_count == 0 {
                self.gc(root);
                try_count += 1;

            } else {
                self.dump_heap();

                panic!("oom");
            }
        }
    }

    pub fn used(&self) -> usize {
        self.used
    }

    #[allow(dead_code)]
    fn debug_gc<R: GCRootValueHolder>(&mut self, root: &R) {
        self.gc(root);

        //ダングリングポインタを発見しやすくするために未使用の領域を全て0埋め
        unsafe {
            let ptr = self.pool_ptr.add(self.used);
            std::ptr::write_bytes(ptr, 0, self.page_layout.size() - self.used);
        }

        //self.dump_heap(obj);
    }

    pub fn value_info(&self, v: &Value) {
        //値を指している参照から、GCHeaderを指しているポインタに変換
        let alloc_ptr = unsafe {
            let ptr = v as *const Value as *const u8;
            ptr.sub(mem::size_of::<GCHeader>())
        };
        let offset = unsafe { alloc_ptr.offset_from(self.pool_ptr) };
        println!("[info] offset:{:>4} {}", offset, v);
    }

    pub fn dump_heap(&self) {
        println!("[dump]------------------------------------");

        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v_ptr = ptr.add(std::mem::size_of::<GCHeader>());
                let v = &*(v_ptr as *const Value);

                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());
                println!("[dump] {:<8}, size:{}, ptr:{:>4}, {:?}",
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

    pub fn dump_gc_heap(&self, flags: &[u16]) {
        println!("[dump]------------------------------------");

        unsafe {
            let mut ptr = self.pool_ptr;
            let end = self.pool_ptr.add(self.used);

            while ptr < end {
                let header = &mut *(ptr as *mut GCHeader);
                let v_ptr = ptr.add(std::mem::size_of::<GCHeader>());
                let v = &*(v_ptr as *const Value);

                let (alive, forwarding) = Self::get_gc_flag(self.pool_ptr, ptr, flags);

                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());
                println!("[dump] {:<8}, size:{}, alive:{}, forwarding:{}, ptr:{:>4}, {:?}",
                    header.typeinfo.as_ref().name,
                    size,
                    alive,
                    forwarding,
                    ptr.offset_from(self.pool_ptr),
                    v
                );

                ptr = ptr.add(size as usize);
            }
        }

        println!("[dump] **** end ****");
    }

    pub(crate) fn gc<R: GCRootValueHolder>(&mut self, root: &R) {
        //self.dump_heap();

        match self.heap_size {
            HeapSize::_256 => self.gc_copying(HeapSize::_512, root),
            HeapSize::_512 => self.gc_copying(HeapSize::_1k, root),
            HeapSize::_1k => self.gc_copying(HeapSize::_2k, root),
            HeapSize::_2k => self.gc_copying(HeapSize::_8k, root),
            HeapSize::_8k => self.gc_copying(HeapSize::_16k, root),
            HeapSize::_16k => self.gc_copying(HeapSize::_32k, root),
            HeapSize::_32k => self.gc_compaction_32k(root),
        };

        //self.dump_heap();
    }

    fn is_valid_value(v: &Value, arg: &mut GCCopyingArg) -> bool {
        //ポインタかつ、自分自身のヒープ内に存在するオブジェクトなら、有効な値。
        value::value_is_pointer(v)
            && Self::is_pointer_within_heap(v, arg.start_addr, arg.end_addr)
    }

    fn gc_copying_copy(v: &Value, arg: &mut GCCopyingArg) -> *mut u8 {
        //値を指している参照から、GCHeaderを指しているポインタに変換
        let alloc_ptr = unsafe {
            let ptr = v as *const Value as *const u8;
            ptr.sub(mem::size_of::<GCHeader>())
        };

        unsafe {
            //オブジェクトがあるはずの場所をCopiedValueとして無理やり解釈する。
            //※有効な値とは絶対にかぶらないような値構造になっているため安全。
            //※まだコピーされていない有効な値の場合、最初のフィールドにはtypeinfoへのポインタが入っている。
            //※コピー済みの場合はポインタではない特別なImmidiate Valueが入っているため区別できる。
            let copied = alloc_ptr as *mut CopiedValue;
            //まだコピーされていないオブジェクトなら
            if CopiedValue::is_copied(copied) == false {
                let header = &mut *(alloc_ptr as *mut GCHeader);

                //新しい領域にオブジェクトをコピー
                let size = Self::get_allocation_size(v, header.typeinfo.as_ref());
                let new_ptr = arg.free_ptr;
                std::ptr::copy_nonoverlapping(alloc_ptr, new_ptr, size);

                //古い領域のコピー済み領域に、マークとコピー先のポインタを保存する
                CopiedValue::mark_copied(copied, new_ptr.add(std::mem::size_of::<GCHeader>()));
                //※注意、これ以降は元の領域にアクセスすると壊れたデータになっている!!

                //使用した分空き領域を指すポインタを進める
                arg.free_ptr = arg.free_ptr.add(size);
                arg.used += size;

                //コピー先の領域にあるデータ参照するように諸々のローカル変数を更新
                let header = &mut *(new_ptr as *mut GCHeader);
                let v = &*(new_ptr.add(mem::size_of::<GCHeader>()) as *const Value);

                //コピーしたオブジェクトが子オブジェクトを持っているなら、再帰的にコピー処理を行う
                if let Some(func) = header.typeinfo.as_ref().child_traversal_func {
                    func(v, arg.as_ptr(), |child, arg_ptr| {
                        let arg = GCCopyingArg::from_ptr(arg_ptr);

                        if Self::is_valid_value(child.as_ref(), arg) {
                            let new_child_ptr = Self::gc_copying_copy(child.as_ref(), arg);

                            //コピー先の新しいポインタで、内部で保持している子オブジェクトへのポインタを上書きする
                            child.update_pointer(new_child_ptr as *mut Value);
                        }
                    });
                }
            }

            //戻り値としてコピーした先のポインタを返す
            CopiedValue::forwarding_pointer(copied)
        }
    }

    fn gc_copying<R: GCRootValueHolder>(&mut self, next_heap_size: HeapSize, root: &R) {
        //self.dump_heap();
        //コピー先の新しいヒープを作成
        let new_layout = Self::get_alloc_layout(next_heap_size);

        println!("copying:{:?} {:?}", next_heap_size, new_layout);
        //self.dump_heap();

        let new_heap_ptr = unsafe { alloc::alloc(new_layout) };

        let mut arg = GCCopyingArg::new(
            self.pool_ptr,
            unsafe { self.pool_ptr.add(self.used) },
            new_heap_ptr,
        );

        //Typeinfoの実装の都合上、クロージャを渡すことができないので、無理やりポインタを経由して値を渡す
        //ルートから辿ることができるオブジェクトをすべて取得して、新しい領域へコピーする
        root.for_each_alived_value(arg.as_ptr(), |v, arg_ptr| {
            let arg = unsafe { GCCopyingArg::from_ptr(arg_ptr) };
            let value = v.as_ref();

            if Self::is_valid_value(value, arg) {
                let new_ptr = Self::gc_copying_copy(value, arg);
                //コピー先の新しいポインタで、保持しているポインタを上書きする
                v.update_pointer(new_ptr as *mut Value);
            }
        });

        //古いヒープを削除
        unsafe {
            alloc::dealloc(self.pool_ptr, self.page_layout);
        }

        self.heap_size = next_heap_size;
        self.page_layout = new_layout;
        self.pool_ptr = new_heap_ptr;
        self.used = arg.used;

        println!("{:?} ... {:?}", new_heap_ptr, unsafe { new_heap_ptr.add(new_layout.size()) });

        //self.dump_heap();
    }

    fn gc_compaction_32k<R: GCRootValueHolder>(&mut self, root: &R) {
        println!("compaction: 32k");

        //32kサイズのヒープ内に存在する可能性がある値すべてのGCフラグを保持できる大きさの配列
        //※最後のシフトは8,または16で割り算することと同じ意味。
        //値の最低サイズは32bitOSなら8、64bitOSなら16なのでそれぞれの数字で割る。
        let mut flags = [0u16; 1024 * 32 >> SIZE_BIT_SHIFT];

        self.gc_compaction_mark_phase(&mut flags, root);
        self.gc_compaction_setup_forwad_ptr(&mut flags);
        //self.dump_gc_heap(&flags, obj);

        self.gc_compaction_update_reference(&mut flags, root);
        self.gc_compaction_move_object(&mut flags);

        self.heap_size = HeapSize::_32k;
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

    fn is_pointer_within_heap(v: &Value, start_addr: *const u8, end_addr: *const u8) -> bool {
        let ptr = v as *const Value as *const u8;
        start_addr <= ptr && ptr < end_addr
    }

    fn is_need_mark(v: &Value, arg: &GCCompactionArg) -> bool {
        //Immidiate Valueの場合があるため正しくポインタであるかを確認
        //かつ、 funcやsyntaxなど、ヒープ外のstaticな領域に確保された値の可能性があるのでチェック
        if value::value_is_pointer(v) && Self::is_pointer_within_heap(v, arg.start_addr, arg.end_addr) {
            //値を指している参照から、GCHeaderを指しているポインタに変換
            let alloc_ptr = unsafe {
                let ptr = v as *const Value as *const u8;
                ptr.sub(mem::size_of::<GCHeader>())
            };
            let (alive, _) = Self::get_gc_flag(arg.start_addr, alloc_ptr, arg.flags);

            //まだ生存フラグを立てていなければ、マークが必要
            alive == false
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

    fn gc_compaction_mark(v: &Value, arg: &mut GCCompactionArg) {
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
                let arg = unsafe { GCCompactionArg::from_ptr(arg_ptr) };

                let child = child.as_ref();
                if Self::is_need_mark(child, arg) {
                    Self::gc_compaction_mark(child, arg);
                }
            });
        }
    }

    fn gc_compaction_mark_phase<R: GCRootValueHolder>(&mut self, flags: &mut [u16], root: &R) {
        let mut arg = unsafe {
            GCCompactionArg::new(self.pool_ptr, self.pool_ptr.add(self.used), flags)
        };

        //Typeinfoの実装の都合上、クロージャを渡すことができないので、無理やりポインタを経由して値を渡す
        root.for_each_alived_value(arg.as_ptr(), |v, arg_ptr| {
            let arg = unsafe { GCCompactionArg::from_ptr(arg_ptr) };

            let v = v.as_ref();
            if Self::is_need_mark(v, arg) {
                Self::gc_compaction_mark(v, arg);
            }
        });
    }

    fn gc_compaction_setup_forwad_ptr(&self, flags: &mut [u16]) {
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

    fn gc_compaction_update_reference<R: GCRootValueHolder>(&mut self, flags: &mut [u16], root: &R) {
        //生きているオブジェクトの内部で保持したままのアドレスを、
        //再配置後のアドレスで上書きする

        fn update_child_pointer(child: &FPtr<Value>, arg_ptr: *mut u8) {
            let arg = unsafe { GCCompactionArg::from_ptr(arg_ptr) };
            let value = child.as_ref();

            //子オブジェクトへのポインタを移動先の新しいポインタで置き換える
            if value::value_is_pointer(value)
                && crate::object::mm::Heap::is_pointer_within_heap(value, arg.start_addr, arg.end_addr) {
                //値を指している参照から、GCHeaderを指しているポインタに変換
                let alloc_ptr = unsafe {
                    let ptr = value as *const Value as *const u8;
                    ptr.sub(mem::size_of::<GCHeader>())
                };

                let (_, forwarding_index) = crate::object::mm::Heap::get_gc_flag(arg.start_addr, alloc_ptr, arg.flags);

                //子オブジェクトが移動しているなら移動先のポインタを参照するように更新する
                let offset = forwarding_index + std::mem::size_of::<GCHeader>();
                let new_ptr = unsafe { arg.start_addr.add(offset) } as *mut Value;

                child.update_pointer(new_ptr);
            }
        }

        unsafe {
            let mut ptr = self.pool_ptr;
            let end = ptr.add(self.used);

            let mut arg = GCCompactionArg::new(ptr, end, flags);

            //ルートオブジェクトとして保持されているオブジェクト内のポインタを更新
            root.for_each_alived_value(arg.as_ptr(), update_child_pointer);

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

    fn gc_compaction_move_object(&mut self, flags: &[u16]) {
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

pub fn usize_to_ptr<T>(data: usize) -> *mut T {
    let u = PtrToUsize {
        v: data,
    };
    unsafe { u.ptr as *const T as *mut T}
}

#[cfg(test)]
mod tests {
    use super::*;
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
                let used = (std::mem::size_of::<GCHeader>() + std::mem::size_of::<number::Integer>()) * 3;
                assert_eq!(obj.heap_used(), used);
            }

            obj.do_gc();
            let used = (std::mem::size_of::<GCHeader>() + std::mem::size_of::<number::Integer>()) * 1;
            assert_eq!(obj.heap_used(), used);
        }

        obj.do_gc();
        assert_eq!(obj.heap_used(), 0);
    }
}
