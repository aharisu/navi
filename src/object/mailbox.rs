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

struct MailBoxGCRootValues {
    pub inbox: Vec<MessageData>,
    pub result_box: Vec<(ReplyToken, FPtr<Value>)>,
}

impl mm::GCRootValueHolder for MailBoxGCRootValues {
    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut FPtr<Value>, *mut u8)) {
        self.inbox.iter_mut().for_each(|data| callback(&mut data.message, arg));
        self.result_box.iter_mut().for_each(|(_, result)| callback(result, arg));
    }
}


// 実装メモ
// ※ Objectがスケジューラに割り当てられている時の参照図
//
//        Strong Refer
//MailBox ============> Object
//         Weak Refer
//MailBox <------------ Object
//         Weak Refer
//Scheduler ----------> Object
//
//Objectの所有者はMailBox(唯一の強参照)。
//他にObjectを参照しているのはSchedulerの弱参照の一つのみ。
//MailBoxの強参照は、MailBoxのDropと同時にObjectもDropさせるためのもので、
//Objectへ操作を行うのはSchedulerのみ。
//Objectへの参照が競合してしまうことはないため、Arc<Mutex<Object>>ではなく、Arc<RefCell<Object>>で持つ。

pub struct MailBox {
    //関連しているObjectがスケジューラに紐づけられている時に値が設定される。
    //Objectの初期化時やスケジューラから切り離されている時はNoneになる。
    obj: Option<Arc<RefCell<Object>>>,
    heap: Heap,

    reply_token: ReplyToken,
    values: MailBoxGCRootValues,
}

impl MailBox {
    pub(super) fn new() -> Self {
        MailBox {
            obj:None,
            heap: Heap::new(mm::StartHeapSize::Small),

            reply_token: ReplyToken::new(),
            values: MailBoxGCRootValues {
                inbox: Vec::new(),
                result_box: Vec::new(),
            }
        }
    }

    pub fn recv_message(&mut self, msg: &Reachable<Value>, reply_to_mailbox: Arc<Mutex<MailBox>>) -> ReplyToken {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let mut allocator = AnyAllocator::MailBox(self);
        let msg = crate::value::value_clone(msg, &mut allocator);

        //返信を送受信するためのtx/rxを作成
        let reply_token = self.reply_token;
        self.reply_token = self.reply_token.next();

        //受け取ったメッセージを内部バッファに保存する
        self.values.inbox.push(MessageData {
            message: msg,
            reply_to_mailbox: reply_to_mailbox,
            reply_token: reply_token,
        });

        //処理終了後の値を受け取るための受信チャンネルを返す
        reply_token
    }

    pub fn pop_inbox(&mut self) -> Option<MessageData> {
        self.values.inbox.pop()
    }

    pub fn recv_reply(&mut self, result: &Reachable<Value>, reply_token: ReplyToken) {
        //受け取ったメッセージをすべて自分自身のヒープ内にコピーする
        let mut allocator = AnyAllocator::MailBox(self);
        let result = crate::value::value_clone(result, &mut allocator);

        self.values.result_box.push((reply_token, result));
    }

    pub fn check_reply(&mut self, reply_token: ReplyToken) -> Option<FPtr<Value>> {
        self.values.result_box.iter().position(|(token, _)| reply_token == *token)
            .map(|index| {
                let (_, result) = self.values.result_box.swap_remove(index);
                result
            })
    }

    pub(super) fn give_object_ownership(&mut self, obj: Arc<RefCell<Object>>) {
        self.obj = Some(obj);
    }

    pub(super) fn take_object_ownership(&mut self) -> Arc<RefCell<Object>> {
        self.obj.take().unwrap()
    }

}

impl Eq for MailBox {}

impl PartialEq for MailBox {
    fn eq(&self, other: &Self) -> bool {
        //同じオブジェクトを参照しているならイコール
        self.obj == other.obj
    }
}

impl Allocator for MailBox {
    fn alloc<T: NaviType>(&mut self) -> UIPtr<T> {
        self.heap.alloc(&mut self.values)
    }

    fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> UIPtr<T> {
        self.heap.alloc_with_additional_size(additional_size, &mut self.values)
    }

    fn force_allocation_space(&mut self, size: usize) {
        self.heap.force_allocation_space(size, &mut self.values);
    }

    fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool {
        self.heap.is_in_heap_object(v)
    }

    fn do_gc(&mut self) {
        self.heap.gc(&mut self.values)
    }

    fn heap_used(&self) -> usize {
        self.heap.used()
    }
}
