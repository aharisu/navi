mod fixed_size_alloc;
mod world;
pub mod context;
pub mod mm;


use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::value::*;
use crate::ptr::*;

use crate::value::list::ListBuilder;

use self::context::Context;
use self::fixed_size_alloc::FixedSizeAllocator;
use self::mm::{GCAllocationStruct, Heap};

pub struct Object {
    ctx: Context,
    heap: RefCell<Heap>,

    captures: FixedSizeAllocator<FPtr<Value>>,
    receiver_vec: Vec<(FPtr<Value>, FPtr<list::List>)>,
    receiver_closure: Option<FPtr<closure::Closure>>,
}

impl Object {
    pub fn new() -> Self {
        let mut ctx = Context::new();
        ctx.register_core_global();

        let heap = Heap::new();

        Object {
            ctx: ctx,
            heap: RefCell::new(heap),
            captures: FixedSizeAllocator::new(),

            receiver_vec: Vec::new(),
            receiver_closure: None,
        }
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
        let msg = crate::value::value_clone(msg, self).reach(self);

        //メッセージをオブジェクトに対して適用
        self.apply_message(&msg)
    }

    pub fn apply_message(&mut self, message: &Reachable<Value>) -> FPtr<Value> {
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
            let mut params_vec = Vec::new();
            params_vec.push(literal::msg_symbol());

            //matchを含んだClosureを作成する
            let closure = closure::Closure::alloc(params_vec, &closure_body, obj);
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

    pub fn alloc<T: NaviType>(&mut self) -> UIPtr<T> {
        self.heap.borrow_mut().alloc::<T>(self)
    }

    pub fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> UIPtr<T> {
        self.heap.borrow_mut().alloc_with_additional_size::<T>(additional_size, self)
    }

    pub fn force_allocation_space(&mut self, size: usize) {
        self.heap.borrow_mut().force_allocation_space(size, self);
    }

    #[allow(dead_code)]
    pub(crate) fn heap_used(&self) -> usize {
        self.heap.borrow().used()
    }

    #[allow(dead_code)]
    pub(crate) fn do_gc(&mut self) {
        self.heap.borrow_mut().gc(self);
    }

    #[allow(dead_code)]
    pub(crate) fn dump_gc(&self) {
        self.heap.borrow().dump_heap(self);
    }

    #[allow(dead_code)]
    pub(crate) fn value_info(&self, v: &Value) {
        self.heap.borrow().value_info(v);
    }

    pub(crate) fn for_each_all_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        self.ctx.for_each_all_alived_value(arg, callback);

        unsafe {
            self.captures.for_each_used_value(|refer| {
                callback(refer, arg);
            });
        }

        self.receiver_vec.iter().for_each(|(pat, body)| {
            callback(pat, arg);
            callback(body.cast_value(), arg);
        });

        if let Some(closure) = self.receiver_closure.as_ref() {
            callback(closure.cast_value(), arg);
        }
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
