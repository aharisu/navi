mod fixed_size_alloc;
mod world;
pub mod context;
pub mod mm;


use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::value::*;
use crate::ptr::*;

use crate::value::array::ArrayBuilder;
use crate::value::list::ListBuilder;
use crate::vm::VMState;

use self::context::Context;
use self::fixed_size_alloc::FixedSizeAllocator;
use self::mm::{GCAllocationStruct, Heap};

pub trait Allocator {
    fn alloc<T: NaviType>(&self) -> UIPtr<T>;
    fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> UIPtr<T>;
    fn force_allocation_space(&self, size: usize);
}

//型パラメータがどうしても使えない場所で使用するAllocatorを実装する値を内包したenum
//TypeInfo内で保持されるclone_innerで使用している。
pub enum AnyAllocator<'a> {
    Object(&'a Object),
    MailBox(&'a MailBox),
}

impl <'a> Allocator for AnyAllocator<'a> {
    fn alloc<T: NaviType>(&self) -> UIPtr<T> {
        match self {
            AnyAllocator::Object(obj) => obj.alloc(),
            AnyAllocator::MailBox(mailbox) => mailbox.alloc(),
        }
    }

    fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> UIPtr<T> {
        match self {
            AnyAllocator::Object(obj) => obj.alloc_with_additional_size(additional_size),
            AnyAllocator::MailBox(mailbox) => mailbox.alloc_with_additional_size(additional_size),
        }
    }

    fn force_allocation_space(&self, size: usize) {
        match self {
            AnyAllocator::Object(obj) => obj.force_allocation_space(size),
            AnyAllocator::MailBox(mailbox) => mailbox.force_allocation_space(size),
        }
    }
}

pub struct Object {
    ctx: Context,
    vm_state: VMState,

    world: world::World,

    heap: RefCell<Heap>,
    captures: FixedSizeAllocator<FPtr<Value>>,

    receiver_vec: Vec<(FPtr<Value>, FPtr<list::List>)>,
    receiver_closure: Option<FPtr<closure::Closure>>,
}

impl Object {
    pub fn new() -> Self {
        let mut obj = Object {
            ctx: Context::new(),
            vm_state: VMState::new(),

            world: world::World::new(),

            heap: RefCell::new(Heap::new(mm::StartHeapSize::Default)),
            captures: FixedSizeAllocator::new(),

            receiver_vec: Vec::new(),
            receiver_closure: None,
        };
        obj.register_core_global();

        obj
    }

    pub fn add_receiver(&mut self, pattern: &Reachable<Value>, body: &Reachable<list::List>) {
        //コンテキストが持つレシーバーリストに追加する
        self.receiver_vec.push((FPtr::new(pattern.as_ref()), FPtr::new(body.as_ref())));
        //レシーブを実際に行う処理は遅延して構築する。
        //レシーバーのパターンはmatch構文のpatternに置き換えられる。
        //構築にはレシーバーパターンすべて必要になり、一つのレシーバーを追加するたびに再構築するのではコストが大きい。
        self.receiver_closure = None;
    }

    pub fn recv_message(&mut self, msg: &Reachable<Value>) -> FPtr<Value> {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let allocator = AnyAllocator::Object(self);
        let msg = crate::value::value_clone(msg, &allocator).reach(self);

        //メッセージをオブジェクトに対して適用
        self.apply_message(&msg)
    }

    fn apply_message(&mut self, message: &Reachable<Value>) -> FPtr<Value> {
        let obj = self;

        //レシーバーのパターンマッチ式がまだ構築されていなければ
        if obj.receiver_closure.is_none() {

            //パターン部分を構築
            let match_args = {
                let mut builder = ListBuilder::new(obj);
                builder.append(literal::msg_symbol().cast_value(), obj);

                for (pattern, body) in obj.receiver_vec.clone().into_iter() {
                    let pattern = pattern.reach(obj);
                    let body = body.reach(obj);

                    let clause = list::List::alloc(&pattern, &body, obj).into_value().reach(obj);
                    builder.append(&clause, obj);
                }
                builder.get()
            };

            //match構文を適用して式を展開する
            let match_exp = {
                let match_args = match_args.reach(obj);
                crate::value::syntax::r#match::translate(&match_args, obj).into_value()
            };
            let closure_body = {
                let match_exp = match_exp.reach(obj);
                list::List::alloc_tail(&match_exp, obj)
            };
            let closure_body = closure_body.reach(obj);

            //Closure生成のためのパラメータ部分を構築
            let mut builder_params = ArrayBuilder::<symbol::Symbol>::new(1, obj);
            builder_params.push(literal::msg_symbol().as_ref(), obj);

            let params = builder_params.get().reach(obj);
            //matchを含んだClosureを作成する
            let closure = closure::Closure::alloc(&params, &closure_body, obj);
            obj.receiver_closure = Some(closure);
        }

        //メッセージをクロージャに適用してパターンマッチを実行する
        let msg = FPtr::new(message.as_ref());
        let closure = obj.receiver_closure.as_ref().unwrap().clone();
        closure.as_ref().apply(std::iter::once(msg), obj)
    }

    pub fn context(&mut self) -> &mut Context {
        &mut self.ctx
    }

    #[inline(always)]
    pub fn vm_state(&mut self) -> &mut VMState {
        &mut self.vm_state
    }

    pub fn find_global_value(&self, symbol: &symbol::Symbol) -> Option<FPtr<Value>> {
        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol) {
            Some(v.clone())
        } else {
            None
        }
    }

    pub fn define_global_value<Key: AsRef<str>, V: NaviType>(&mut self, key: Key, v: &V) {
        self.world.set(key, cast_value(v))
    }

    fn register_core_global(&mut self) {
        object_ref::register_global(self);
        number::register_global(self);
        syntax::register_global(self);
        crate::value::register_global(self);
        tuple::register_global(self);
        array::register_global(self);
        list::register_global(self);
    }

    pub fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool {
        self.heap.borrow().is_in_heap_object(v)
    }

    #[allow(dead_code)]
    pub(crate) fn heap_used(&self) -> usize {
        self.heap.borrow().used()
    }

    #[allow(dead_code)]
    pub(crate) fn do_gc(&self) {
        self.heap.borrow_mut().gc(self);
    }

    #[allow(dead_code)]
    pub(crate) fn dump_gc(&self) {
        self.heap.borrow().dump_heap();
    }

    #[allow(dead_code)]
    pub(crate) fn value_info(&self, v: &Value) {
        self.heap.borrow().value_info(v);
    }

    pub fn capture<T: NaviType>(&mut self, v: FPtr<T>) -> Cap<T> {
        unsafe {
            let ptr = self.captures.alloc();
            let ptr = ptr as *mut FPtr<T>;

            let mut cap = Cap::new(ptr);
            cap.update_pointer(v);

            cap
        }
    }

    pub(crate) fn release_capture<T: NaviType>(cap: &Cap<T>) {
        unsafe {
            let ptr = cap.cast_value().ptr();
            FixedSizeAllocator::<FPtr<Value>>::free(ptr);
        }
    }

}

impl Eq for Object {}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        //同じヒープを持っている場合は同じコンテキストと判断する
        std::ptr::eq(&self.heap, &other.heap)
    }
}

impl mm::GCRootValueHolder for Object {
    fn for_each_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        self.ctx.for_each_all_alived_value(arg, callback);
        self.vm_state.for_each_all_alived_value(arg, callback);

        //グローバルスペース内で保持している値
        for v in self.world.get_all_values().iter() {
            callback(v, arg);
        }

        //キャプチャーしているローカル変数
        unsafe {
            self.captures.for_each_used_value(|refer| {
                callback(refer, arg);
            });
        }

        //オブジェクトが持つメッセージレシーバーオブジェクト
        self.receiver_vec.iter().for_each(|(pat, body)| {
            callback(pat, arg);
            callback(body.cast_value(), arg);
        });

        //レシーバークロージャ
        if let Some(closure) = self.receiver_closure.as_ref() {
            callback(closure.cast_value(), arg);
        }
    }
}

impl Allocator for Object {
    fn alloc<T: NaviType>(&self) -> UIPtr<T> {
        self.heap.borrow_mut().alloc(self)
    }

    fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> UIPtr<T> {
        self.heap.borrow_mut().alloc_with_additional_size(additional_size, self)
    }

    fn force_allocation_space(&self, size: usize) {
        self.heap.borrow_mut().force_allocation_space(size, self);
    }
}

static SYMBOL_MSG: Lazy<GCAllocationStruct<symbol::StaticSymbol>> = Lazy::new(|| {
    symbol::gensym_static("msg")
});

mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn msg_symbol() -> Reachable<symbol::Symbol> {
        Reachable::new_static(SYMBOL_MSG.value.as_ref())
    }
}

pub struct MailBox {
    obj: Object,
    heap: RefCell<Heap>,
}

impl MailBox {
    pub fn new(obj: Object) -> Self {
        MailBox {
            obj,
            heap: RefCell::new(Heap::new(mm::StartHeapSize::Small)),
        }
    }

    pub fn recv(&mut self, msg: &Reachable<Value>) -> FPtr<Value> {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let allocator = AnyAllocator::MailBox(self);
        let msg = crate::value::value_clone(msg, &allocator);
        //TODO 受け取ったメッセージを内部バッファに保存する

        //Objectへ渡す場合は絶対にGCが発生しないため無理やりReachableに変換する
        let msg = unsafe { msg.into_reachable() };
        self.obj.recv_message(&msg)
    }

}

impl Eq for MailBox {}

impl PartialEq for MailBox {
    fn eq(&self, other: &Self) -> bool {
        //同じオブジェクトを参照しているならイコール
        self.obj == other.obj
    }
}

impl mm::GCRootValueHolder for MailBox {
    fn for_each_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
    }
}

impl Allocator for MailBox {
    fn alloc<T: NaviType>(&self) -> UIPtr<T> {
        self.heap.borrow_mut().alloc(self)
    }

    fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> UIPtr<T> {
        self.heap.borrow_mut().alloc_with_additional_size(additional_size, self)
    }

    fn force_allocation_space(&self, size: usize) {
        self.heap.borrow_mut().force_allocation_space(size, self);
    }
}