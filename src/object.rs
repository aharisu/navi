use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::context::Context;
use crate::{value::*, let_arraybuilder};
use crate::{ptr::*, let_cap, new_cap, let_listbuilder, with_cap};

use crate::mm::{GCAllocationStruct, Heap};

pub struct Object {
    ctx: Context,
    heap: RefCell<Heap>,

    receiver_vec: Vec<(RPtr<Value>, RPtr<list::List>)>,
    receiver_closure: Option<RPtr<closure::Closure>>,
}

impl Object {
    pub fn new() -> Self {
        let mut ctx = Context::new();
        ctx.register_core_global();

        let heap = Heap::new();

        Object {
            ctx: ctx,
            heap: RefCell::new(heap),
            receiver_vec: Vec::new(),
            receiver_closure: None,
        }
    }

    pub fn add_receiver<T, U>(&mut self, pattern: &T, body: &U)
    where
        T: AsReachable<Value>,
        U: AsReachable<list::List>,
    {
        //コンテキストが持つレシーバーリストに追加する
        self.receiver_vec.push((pattern.as_reachable().clone(), body.as_reachable().clone()));
        //レシーブを実際に行う処理は遅延して構築する。
        //レシーバーのパターンはmatch構文のpatternに置き換えられる。
        //構築にはレシーバーパターンすべて必要になり、一つのレシーバーを追加するたびに再構築するのではコストが大きい。
        self.receiver_closure = None;
    }

    pub fn recv_message(&mut self, msg: &RPtr<Value>) -> FPtr<Value> {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let msg = crate::value::value_clone(msg, self);
        let_cap!(msg, msg, self);

        //メッセージをオブジェクトに対して適用
        self.apply_message(&msg)
    }

    pub fn apply_message<T>(&mut self, message: &T) -> FPtr<Value>
    where
        T: AsReachable<Value>
    {
        //レシーバーのパターンマッチ式がまだ構築されていなければ
        if self.receiver_closure.is_none() {

            //パターン部分を構築
            let match_args = {
                let_listbuilder!(builder, self);
                builder.append(&literal::msg_symbol().into_value(), self);
                for (pattern, body) in self.receiver_vec.clone().into_iter() {
                    let_cap!(clause, list::List::alloc(&pattern, &body, self).into_value(), self);
                    builder.append(&clause, self);
                }
                builder.get()
            };

            //match構文を適用して式を展開する
            let match_exp = with_cap!(match_args, match_args, self, {
                crate::value::syntax::r#match::translate(match_args.as_reachable(), self).into_value()
            });
            let closure_body = with_cap!(match_exp, match_exp, self, {
                list::List::alloc_tail(&match_exp, self)
            });
            let_cap!(closure_body, closure_body, self);

            //Closure生成のためのパラメータ部分を構築
            let_arraybuilder!(builder_params, 1, self);
            builder_params.push(&literal::msg_symbol().into_value(), self);
            let_cap!(params, builder_params.get(), self);

            //matchを含んだClosureを作成する
            let closure = closure::Closure::alloc(&params, &closure_body, self);
            self.receiver_closure = Some(closure.into_rptr());
        }

        //メッセージをクロージャに適用してパターンマッチを実行する
        let msg = message.as_reachable();
        let closure = self.receiver_closure.as_ref().unwrap().clone();
        closure.as_ref().apply(std::iter::once(msg), self)
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

    pub(crate) fn for_each_all_alived_value(&self, arg: *mut u8, callback: fn(&RPtr<Value>, *mut u8)) {
        self.ctx.for_each_all_alived_value(arg, callback);

        self.receiver_vec.iter().for_each(|(pat, body)| {
            callback(pat, arg);
            callback(body.cast_value(), arg);
        });

        if let Some(closure) = self.receiver_closure.as_ref() {
            callback(closure.cast_value(), arg);
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
    use crate::ptr::RPtr;
    use super::*;

    pub fn msg_symbol() -> RPtr<symbol::Symbol> {
        RPtr::new(SYMBOL_MSG.value.as_ref() as *const symbol::Symbol as *mut symbol::Symbol)
    }
}
