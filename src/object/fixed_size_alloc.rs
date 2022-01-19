use std::{ptr::NonNull, mem::size_of};

pub struct FixedSizeAllocator<T> {
    chunk_ptr: NonNull<Chunk<T>>,
    all_chunks: Vec<NonNull<Chunk<T>>>,
    chunk_layout: std::alloc::Layout,
}

impl <T> FixedSizeAllocator<T> {
    pub fn new() -> Self {
        //指定された型はusize(ポインタ幅)より大きくないといけない
        debug_assert!(size_of::<usize>() <= size_of::<T>());

        //チャンクのサイズは1024固定
        //チャンクが確保されるアドレスも1024にアラインメントさせる。
        //1024のアラインメントはfreeの実装トリックに必須な処理。
        let chunk_layout = std::alloc::Layout::from_size_align(1024, 1024).unwrap();
        let chunk = Self::new_chunk(chunk_layout);

        let ptr = NonNull::new(chunk).unwrap();
        FixedSizeAllocator {
            chunk_ptr: ptr.clone(),
            all_chunks: vec![ptr],
            chunk_layout: chunk_layout,
        }
    }

    pub unsafe fn alloc(&mut self) -> *mut T {
        let chunk = self.chunk_ptr.as_mut();
        let ptr = chunk.alloc();

        if let Some(ptr) = ptr {
            ptr.as_ptr()
        } else {
            //空いているチャンクを探す
            if let Some(chunk) = self.all_chunks.iter_mut().find(|ptr| ptr.as_ref().available()) {
                //空いているチャンクが見つかったら、次回使用時のために保存しておく
                self.chunk_ptr = chunk.clone();

                //見つけたチャンクからアロケート
                chunk.as_mut().alloc().unwrap().as_ptr()

            } else {
                //空きチャンクがない場合は新しいチャンクを作る
                let new_chunk = Self::new_chunk(self.chunk_layout);
                let mut ptr = NonNull::new(new_chunk).unwrap();

                let result = ptr.as_mut().alloc().unwrap();

                self.all_chunks.push(ptr.clone());
                self.chunk_ptr = ptr;

                result.as_ptr()
            }
        }
    }

    pub unsafe fn free(ptr: *mut T) {
        //チャンク内のポインタからチャンクのヘッダを取得する
        let addr = ptr_to_addr(ptr);
        //チャンクのサイズは1024固定で開始アドレスも必ず1024にアラインメントされている。
        //そのため、チャンク内のアドレス下位10bitを0にすることでチャンクヘッダのアドレスに変換できる
        let chunk_addr = addr & !0b11_1111_1111;
        let chunk = &mut *addr_to_ptr::<Chunk<T>>(chunk_addr);

        chunk.free(ptr);
    }

    #[allow(dead_code)]
    pub unsafe fn used_vec(&self) -> Vec<bool> {
        self.chunk_ptr.as_ref().used_vec()
    }

    pub unsafe fn for_each_used_value<F>(&self, callback: F)
    where
        F: Fn(&mut T)
    {
        self.all_chunks.iter().for_each(|chunk| {
            chunk.as_ref().for_each_used_value(&callback);
        });
    }

    fn new_chunk(chunk_layout: std::alloc::Layout) -> *mut Chunk<T> {
        let data = unsafe { std::alloc::alloc_zeroed(chunk_layout) };
        //dataが正しく1024にアラインメントされているかを検証
        assert!(ptr_to_addr(data) % 1024 == 0);

        //allocで使用可能なデータサイズ
        let datasize = chunk_layout.size();
        //まずチャンクヘッダー分の領域を使用可能サイズから引く
        let datasize = datasize - size_of::<Chunk<T>>();
        let chunk = data as *mut Chunk<T>;
        //チャンクヘッダとして使用した領域分ポインタを進める
        let data = unsafe{ data.add(size_of::<Chunk<T>>()) };

        //Chunk内で使用中の領域をBitmapで管理する。
        //Bitmapは確保したヒープ内のChunkの後ろ、データの前に置く。

        //最大でいくつのオブジェクトをallocできるかを計算
        let count = datasize / size_of::<T>();
        //一つのBitmapセルで管理できるオブジェクトの数
        let bitcount = size_of::<usize>() * 8;
        //全てのオブジェクト領域の使用可否に必要なbitmapの長さを計算
        let bitmapsize = ((count + (bitcount - 1)) / bitcount) * size_of::<usize>();

        //bitmapとして使用する領域も使用可能サイズから引く
        let datasize = datasize - bitmapsize;
        //Bitmapとして使用した領域分ポインタを進める
        let data = unsafe { data.add(bitmapsize) };

        //最後の領域に番兵となる1を書き込む
        unsafe {
            //領域内でTの値を保持できる数
            let count = datasize / size_of::<T>();
            //カウントをもとにTの最後の領域にポインタをずらす
            let last_space = (data as *mut T).add(count - 1);
            //最後の領域にusizeとして(アドレスサイズとして)１を書き込む。
            //1は領域終了の番兵
            (last_space as *mut usize).write(1);
        }

        //確保した領域の先頭にチャンクを書き込む
        unsafe {
            //確保したヒープの先頭にはチャンク情報を書き込む
            chunk.write(Chunk
                {
                    data_start: data as *mut T,
                    next: data as *mut T,
                });
        }

        chunk
    }

}

impl <T> Drop for FixedSizeAllocator<T> {
    fn drop(&mut self) {
        let ptr = self.chunk_ptr.as_ptr();
        unsafe {
            std::alloc::dealloc(ptr as *mut u8, self.chunk_layout);
        }
    }
}

struct Chunk<T> {
    data_start: *mut T,
    next: *mut T,
}

impl <T> Chunk<T> {
    pub unsafe fn alloc(&mut self) -> Option<NonNull<T>> {
        let addr = ptr_to_addr(self.next);
        //次に確保するスペースのアドレスが1なら、空き領域なし
        if addr == 1{
            None
        } else {
            let ptr = self.next;
            //今回確保した領域には、次に確保する領域のポインタが入っている
            let next_addr = *(ptr as *mut usize);

            //確保した領域が完全に0なら
            //※一度でも確保した領域なら、freeをしたときに次の空き領域へのポインタが入るため必ず0以外になる
            if next_addr == 0 {
                //アドレスを後ろにずらすと空き領域がある
                self.next = self.next.add(1);
            } else {
                self.next = addr_to_ptr(next_addr);
            }

            //確保した領域に対応するBitmapに対してフラグを立てる
            self.bitmap_set_or_clear(ptr, true);

            Some(NonNull::new_unchecked(ptr))
        }
    }

    pub unsafe fn free(&mut self, ptr: *mut T) {
        //チャンク内で保持されていたnextポインタを解放する領域に書き込む
        let next = self.next;
        let next_ptr = ptr as *mut usize;
        *next_ptr = ptr_to_addr(next);

        //解放した領域のポインタを、次に確保するポインタとしてチャンク内に保存する
        self.next = ptr;

        //解放した領域に対応するBitmapに対してフラグを降ろす
        self.bitmap_set_or_clear(ptr, false);
    }

    unsafe fn bitmap_set_or_clear(&mut self, ptr: *mut T, is_set: bool) {
        //確保した領域に対応するBitmapに対してフラグを立てる
        let offset = ptr.offset_from(self.data_start) as usize;
        let index = offset;

        //Bitmapのセル一つで管理できるオブジェクトの数
        let bitcount = size_of::<usize>() * 8;

        //何番目のBitmapセルの中に自分の情報があるか？
        let cell_index = index / bitcount;

        let cell_ptr = {
            //チャンクヘッダーの後ろを指すポインタ
            let ptr = (self as *mut Chunk<T>).add(1);
            //対象のBitmapセルをサスポインタ
            let ptr = (ptr as *mut usize).add(cell_index);
            ptr
        };

        let bit_index = (offset % bitcount) as u32;
        let bitmask = 1usize.rotate_left(bit_index);

        *cell_ptr = if is_set {
            (*cell_ptr) | bitmask
        } else {
            (*cell_ptr) & (!bitmask)
        };
    }

    #[allow(dead_code)]
    pub unsafe fn used_vec(&self) -> Vec<bool> {
        //for debugging

        let chunk_ptr = self as *const Chunk<T>;
        let mut bitmap_ptr = chunk_ptr.add(1) as *const usize;
        let end = self.data_start as *const usize;

        let mut result: Vec<bool> = Vec::new();
        while bitmap_ptr < end {
            let mut bitmap = *bitmap_ptr;
            for _ in 0..(size_of::<usize>() * 8) {
                result.push(bitmap & 1 == 1);
                bitmap = bitmap >> 1;
            }

            bitmap_ptr = bitmap_ptr.add(1);
        }

        result
    }

    pub unsafe fn for_each_used_value<F>(&self, callback: F)
    where
        F: Fn(&mut T)
    {
        let chunk_ptr = self as *const Chunk<T>;
        let mut bitmap_ptr = chunk_ptr.add(1) as *const usize;
        let end = self.data_start as *const usize;

        let mut index = 0usize;
        while bitmap_ptr < end {
            let mut bitmap = *bitmap_ptr;
            //bitmap全体で0なら使用中の領域は一つもない
            if bitmap == 0 {
                index += size_of::<usize>() * 8;
            } else {
                for _ in 0..(size_of::<usize>() * 8) {
                    //bitが立っていたら対象インデックスの領域は使用中
                    if bitmap & 1 == 1 {
                        let ptr = self.data_start.add(index);
                        let refer = std::mem::transmute::<*mut T, &mut T>(ptr);
                        callback(refer);
                    }

                    bitmap = bitmap >> 1;
                    index += 1;
                }
            }

            bitmap_ptr = bitmap_ptr.add(1);
        }

    }

    pub unsafe fn available(&self) -> bool {
        //次に確保するスペースのアドレスが1以外なら、空き領域がある
        ptr_to_addr(self.next) != 1
    }


}

union PtrAddr {
    ptr: *mut u8,
    addr: usize,
}

#[inline]
fn ptr_to_addr<T>(ptr: *mut T) -> usize {
    unsafe {
        PtrAddr {
            ptr: ptr as *mut u8,
        }.addr
    }
}

#[inline]
fn addr_to_ptr<T>(addr: usize) -> *mut T {
    unsafe {
        PtrAddr {
            addr: addr
        }.ptr as *mut T
    }
}
