use std::sync::{Arc, Mutex};
use std::cell::RefCell;

use crate::value::*;
use crate::ptr::*;

use super::{Object, Allocator, AnyAllocator};
use super::mm::{self, Heap};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ReplyToken(usize);

impl ReplyToken {
    fn new() -> Self {
        ReplyToken(0)
    }

    fn next(&self) -> Self {
        //オーバーフローを無視してインクリメント
        ReplyToken(self.0.wrapping_add(1))
    }
}

pub struct MessageData {
    pub message: FPtr<Value>,
    pub reply_to_mailbox: Arc<Mutex<MailBox>>,
    pub reply_token: ReplyToken,
}

// 実装メモ
//
//        Strong Refer
//MailBox ============> Object
//         Weak Refer
//MailBox <------------ Object
// ***     Weak Refer
//Scheduler ----------> Object
//
//Objectの所有者はMailBox(唯一の強参照)。
//他にObjectを参照しているのはSchedulerの弱参照の一つのみ。
//MailBoxの強参照は、MailBoxのDropと同時にObjectもDropさせるためのもので、
//Objectへ操作を行うのはSchedulerのみ。
//Objectへの参照が競合してしまうことはないため、Arc<Mutex<Object>>ではなく、Arc<RefCell<Object>>で持つ。

pub struct MailBox {
    //TODO 検証。RefCellではなくUnsafeCellでもいい？
    obj: Arc<RefCell<Object>>,
    heap: RefCell<Heap>,

    inbox: Vec<MessageData>,
    reply_token: ReplyToken,
    result_box: Vec<(ReplyToken, FPtr<Value>)>,
}

impl MailBox {
    pub(super) fn new(obj: Arc<RefCell<Object>>) -> Self {
        MailBox {
            obj,
            heap: RefCell::new(Heap::new(mm::StartHeapSize::Small)),

            reply_token: ReplyToken::new(),
            inbox: Vec::new(),
            result_box: Vec::new(),
        }
    }

    pub fn recv_message(&mut self, msg: &Reachable<Value>, reply_to_mailbox: Arc<Mutex<MailBox>>) -> ReplyToken {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let allocator = AnyAllocator::MailBox(self);
        let msg = crate::value::value_clone(msg, &allocator);

        //返信を送受信するためのtx/rxを作成
        let reply_token = self.reply_token;
        self.reply_token = self.reply_token.next();

        //受け取ったメッセージを内部バッファに保存する
        self.inbox.push(MessageData {
            message: msg,
            reply_to_mailbox: reply_to_mailbox,
            reply_token: reply_token,
        });

        //処理終了後の値を受け取るための受信チャンネルを返す
        reply_token
    }

    pub fn pop_inbox(&mut self) -> Option<MessageData> {
        self.inbox.pop()
    }

    pub fn recv_reply(&mut self, result: &Reachable<Value>, reply_token: ReplyToken) {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let allocator = AnyAllocator::MailBox(self);
        let result = crate::value::value_clone(result, &allocator);

        self.result_box.push((reply_token, result));
    }

    pub fn check_reply(&mut self, reply_token: ReplyToken) -> Option<FPtr<Value>> {
        self.result_box.iter().position(|(token, _)| reply_token == *token)
            .map(|index| {
                let (_, result) = self.result_box.swap_remove(index);
                result
            })
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
        self.inbox.iter().for_each(|data| callback(&data.message, arg));
        self.result_box.iter().for_each(|(_, result)| callback(result, arg));
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

    fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool {
        self.heap.borrow().is_in_heap_object(v)
    }

    fn do_gc(&self) {
        self.heap.borrow_mut().gc(self)
    }

    fn heap_used(&self) -> usize {
        self.heap.borrow().used()
    }
}
