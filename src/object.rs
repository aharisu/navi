mod fixed_size_alloc;
mod world;
pub mod context;
pub mod mm;
mod balance;
mod schedule;
pub mod mailbox;


use std::fmt::Debug;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Weak, Mutex};
use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::value::*;
use crate::ptr::*;

use crate::value::list::ListBuilder;
use crate::value::object_ref::ObjectRef;
use crate::vm::{self, VMState};

use self::context::Context;
use self::fixed_size_alloc::FixedSizeAllocator;
use self::mm::{GCAllocationStruct, Heap};
use self::mailbox::*;


pub trait Allocator {
    fn alloc<T: NaviType>(&self) -> UIPtr<T>;
    fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> UIPtr<T>;
    fn force_allocation_space(&self, size: usize);
    fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool;
    fn do_gc(&self);
    fn heap_used(&self) -> usize;
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

    fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool {
        match self {
            AnyAllocator::Object(obj) => obj.is_in_heap_object(v),
            AnyAllocator::MailBox(mailbox) => mailbox.is_in_heap_object(v),
        }
    }

    fn do_gc(&self) {
        match self {
            AnyAllocator::Object(obj) => obj.do_gc(),
            AnyAllocator::MailBox(mailbox) => mailbox.do_gc(),
        }
    }

    fn heap_used(&self) -> usize {
        match self {
            AnyAllocator::Object(obj) => obj.heap_used(),
            AnyAllocator::MailBox(mailbox) => mailbox.heap_used(),
        }
    }
}

pub struct Object {
    id: usize,

    //MailBoxへの弱参照。
    //ObjectはMailBoxから強参照されている、相互参照の関係。
    //MailBoxがDropされるとき、Objectも同時にDropされる。
    mailbox: Weak<Mutex<MailBox>>,
    suspend_state: Option<(Arc<Mutex<MailBox>>, ReplyToken)>,

    //object-switchで切り替えた時の、切り替え前オブジェクト。
    prev_object:Option<FPtr<ObjectRef>>,

    ctx: Context,
    vm_state: VMState,

    world: world::World,

    heap: RefCell<Heap>,
    captures: FixedSizeAllocator<FPtr<Value>>,

    receiver_vec: Vec<(FPtr<Value>, FPtr<list::List>)>,
    receiver_closure: Option<FPtr<compiled::Closure>>,
}

impl Object {
    fn new(id: usize, mailbox: Weak<Mutex<MailBox>>) -> Self {
        let mut obj = Object {
            id,
            mailbox: mailbox,
            suspend_state: None,

            prev_object: None,

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

    #[cfg(test)]
    pub(crate) fn new_for_test() -> Self {
        new_object().object
    }

    #[inline]
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn make_object_ref<A: Allocator>(&self, allocator: &A) -> Option<FPtr<ObjectRef>> {
        self.mailbox.upgrade()
            .map(|mailbox| {
                object_ref::ObjectRef::alloc(self.id,  mailbox, allocator)
            })
    }

    pub fn set_prev_object(&mut self, prev_object: &FPtr<ObjectRef>) {
        self.prev_object = Some(prev_object.clone());
    }

    pub fn take_prev_object(&mut self) -> Option<FPtr<ObjectRef>> {
        self.prev_object.take()
    }

    pub fn register_scheduler(standalone: StandaloneObject) -> Arc<Mutex<MailBox>> {
        //MailBoxとスケジューラ間でObjectを共有するためのArcを作成
        //MutexではなくRefCellで内部状態を持つ理由は、MailBox構造体定義上のコメントを参照してください。
        let obj = Arc::new(RefCell::new(standalone.object));

        //MailBoxへObjectの所有権を渡す
        let mailbox = standalone.mailbox;
        {
            let mut mailbox = mailbox.lock().unwrap();
            mailbox.give_object_ownership(Arc::clone(&obj));
        }

        //objをバランサーに渡して、スケジューラに割り当ててもらう。
        //スケジューラは渡したオブジェクトの弱参照を内部で保持する。
        crate::object::balance::add_object(obj);

        mailbox
    }

    pub fn unregister_scheduler(mailbox: Arc<Mutex<MailBox>>) -> StandaloneObject {
        //MailBoxからObjectの所有権を奪う
        let mut obj = {
            let mut mailbox = mailbox.lock().unwrap();
            mailbox.take_object_ownership()
        };

        //スケジューラーと共有しているArcからObjectの所有権を取得する
        loop {
            //スケジューラがObjectを実行中の場合、強参照が二つになっているので取得に失敗する。
            match Arc::try_unwrap(obj) {
                Ok(inner) => {
                    //Objectの所有権を得た場合は、StandaloneObjectとして返す。
                    return StandaloneObject {
                        object: inner.into_inner(),
                        mailbox: mailbox,
                    };
                }
                Err(ret) => {
                    //取得に失敗した場合は、適当な時間待ってからもう一度取得を試みる
                    obj = ret;
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
    }

    pub fn send_message(&mut self, target_obj: &Reachable<ObjectRef>, message: &Reachable<Value>) -> FPtr<Value> {
        if let Some(mailbox) = self.mailbox.upgrade() {
            //戻り値を受け取るために自分自身のメールボックスをメッセージ送信相手に渡す
            let reply_token = target_obj.as_ref().recv_message(message, mailbox);

            //返信を受け取るための特別な値を生成して返す
            crate::value::reply::Reply::alloc(reply_token, self).into_value()

        } else {
            //mailboxが取得できない場合は、自分自身のオブジェクトが削除されようとしているとき。
            //処理を継続できないため無効な値を返して処理を終了させる
            //TODO エラー値を返したい
            bool::Bool::false_().into_value().into_fptr()
        }
    }

    pub fn check_reply(&mut self, reply_token: ReplyToken) -> Option<FPtr<Value>> {
        if let Some(mailbox) = self.mailbox.upgrade() {
            let mut mailbox = mailbox.lock().unwrap();
            mailbox.check_reply(reply_token)
                .map(|result| {
                    //MailBoxのロックを保持している間は、MailBox内ヒープのGCは発生しないため強制的にReachableに変換する。
                    let result = unsafe { result.into_reachable() };
                    //valはMailBox内のヒープに確保された値なので、Object内ヒープに値をクローンする
                    let allocator = AnyAllocator::Object(self);
                    crate::value::value_clone(&result, &allocator)
                })

        } else {
            //mailboxが取得できない場合は、自分自身のオブジェクトが削除されようとしているとき。
            //処理を継続できないため無効な値を返して処理を終了させる
            //TODO エラー値を返したい
            Some(bool::Bool::false_().into_value().into_fptr())
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

    pub fn do_work(&mut self, reduction_count: usize) {

        match self.suspend_state.take() {
            Some((reply_to_mailbox, reply_token)) => {
                let result = vm::resume(reduction_count, self);
                self.apply_message_finish(result, reply_to_mailbox, reply_token);
            }
            None => {
                if let Some(mailbox) = self.mailbox.upgrade() {
                    let data =  {
                        //MailBoxは複数スレッド(複数オブジェクト)間で共有されているのでロックを取得してから操作を行う
                        let mut mailbox = mailbox.lock().unwrap();
                        mailbox.pop_inbox().map(|mut data| {
                            //MailBoxのロックをとっている間はGCが発生しないので、直接Reachableに変換して扱う
                            let msg = unsafe { data.message.into_reachable() };

                            //messageの値はMailBox内のヒープに割り当てられいる。
                            //メッセージの値を自分自身のヒープ内にコピーする
                            let allocator = AnyAllocator::Object(self);
                            data.message = crate::value::value_clone(&msg, &allocator);

                            data
                        })
                    };

                    //メッセージを受信していたら
                    if let Some(data) = data {
                        //受信処理を実行
                        self.apply_message(data, reduction_count);
                    }
                }
            }
        }
    }

    fn apply_message(&mut self, data: MessageData, mut reduction_count: usize) {
        let obj = self;
        let message = data.message.reach(obj);

        //レシーバーのパターンマッチ式がまだ構築されていなければ
        if obj.receiver_closure.is_none() {
            let mut builder_fun = ListBuilder::new(obj);
            //(fun)
            builder_fun.append(crate::value::syntax::literal::fun().cast_value(), obj);
            //(msg_var)
            let paramter = list::List::alloc_tail(literal::msg_symbol().cast_value(), obj).into_value();
            //(fun (msg_var))
            builder_fun.append(&paramter.reach(obj), obj);

            //パターンマッチ部分を構築
            //(match msg_var (pattern body) (pattern2 body2) ...)
            let match_ = {
                let mut builder_match = ListBuilder::new(obj);
                //(match)
                builder_match.append(crate::value::syntax::literal::match_().cast_value(), obj);
                //(match msg_var)
                builder_match.append(literal::msg_symbol().cast_value(), obj);

                for (pattern, body) in obj.receiver_vec.clone().into_iter() {
                    //(pattern body)
                    let pattern = pattern.reach(obj);
                    let body = body.reach(obj);

                    let clause = list::List::alloc(&pattern, &body, obj).into_value().reach(obj);
                    builder_match.append(&clause, obj);
                }
                builder_match.get()
            };
            //(fun (msg_var) (match msg_var (pattern body) (pattern2 body2) ...))
            builder_fun.append(&match_.into_value().reach(obj), obj);

            //メッセージレシーバ用のクロージャをコンパイル
            let receiver = builder_fun.get();
            let message_receiver_gen = crate::compile::compile(&receiver.into_value().reach(obj), obj);

            //クロージャを生成するコードを実行
            let message_receiver = crate::eval::eval(&message_receiver_gen.into_value().reach(obj), obj);
            //実行結果は必ずコンパイル済みクロージャなのでuncheckedでキャスト
            let message_receiver = unsafe { message_receiver.cast_unchecked::<compiled::Closure>() }.clone();

            obj.receiver_closure = Some(message_receiver);

            //TODO パターンマッチ生成部分でもreduction_countを一定減少させる
            reduction_count = reduction_count.saturating_sub(100);
        }

        //メッセージに対してパターンマッチングと対応する処理を実行するクロージャ
        let closure = obj.receiver_closure.as_ref().unwrap().clone().reach(obj);
        //受信したメッセージを引数の形に変換
        let args_iter = std::iter::once(FPtr::new(message.as_ref()));
        //VMの時間制限
        let limit = vm::WorkTimeLimit::Reductions(reduction_count);

        //メッセージをクロージャに適用してパターンマッチを実行する
        let result = vm::closure_call(&closure, args_iter, limit, obj);
        obj.apply_message_finish(result, data.reply_to_mailbox, data.reply_token);
    }

    fn apply_message_finish(&mut self, result: Result<FPtr<Value>, vm::ExecError>
        , reply_to_mailbox: Arc<Mutex<MailBox>>, reply_token: ReplyToken) {

        match result {
            Ok(result) => {
                //結果を送信元のオブジェクト(MailBox)に返す
                {
                    //resultの値はMailBox上のヒープにコピーされるだけ。
                    //自分自身のGCは絶対に発生しないためinto_reachableを行う。
                    let result = unsafe { result.into_reachable() };

                    //返信先メールボックスのロックを取得
                    let mut reply_to_mailbox = reply_to_mailbox.lock().unwrap();
                    //返信を送信
                    reply_to_mailbox.recv_reply(&result, reply_token);
                }

                //残ったreductions分もう一度do_workを実行する
                let remain = self.vm_state().remain_reductions();
                self.do_work(remain);
            },
            Err(vm::ExecError::TimeLimit) => {
                //VMの状態をsuspendにして、次回のdo_work時に処理を継続する
                self.suspend_state = Some((reply_to_mailbox, reply_token));
            },
            Err(vm::ExecError::WaitReply) => {
                //VMの状態をsuspendにして、次回のdo_work時に処理を継続する
                self.suspend_state = Some((reply_to_mailbox, reply_token));
            }
            Err(vm::ExecError::ObjectSwitch(_)) => {
                //Objectの切り替えはグローバル環境のトップレベルでのみ許可されているため、ここでは絶対に発生しない。
                unreachable!()
            }
            Err(vm::ExecError::Exception) => {
                panic!("TODO");
            },
        }
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
        reply::register_global(self);
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

        if let Some(prev_object) = self.prev_object.as_ref() {
            callback(prev_object.cast_value(), arg);
        }

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

pub struct StandaloneObject {
    object: Object,
    mailbox: Arc<Mutex<MailBox>>,
}

impl StandaloneObject {
    #[inline]
    pub fn object(&self) -> &Object {
        &self.object
    }

    #[inline]
    pub fn mut_object(&mut self) -> &mut Object {
        &mut self.object
    }
}

impl Debug for StandaloneObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StandaloneObject:{}", self.object.id())
    }
}

static OBJECT_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn new_object() -> StandaloneObject {
    let mailbox = Arc::new(Mutex::new(MailBox::new()));

    //オブジェクトを識別するためのIDを生成
    let object_id = OBJECT_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    //ObjectはMailBoxを常に弱参照で保持する
    let obj = Object::new(object_id, Arc::downgrade(&mailbox));

    StandaloneObject {
        object: obj,
        mailbox: mailbox
    }
}

pub fn object_switch(cur_object: StandaloneObject, target_object: &ObjectRef) -> StandaloneObject {
    object_switch_inner(cur_object, target_object, true)
}

pub fn return_object_switch(mut cur_object: StandaloneObject) -> Option<StandaloneObject> {
    cur_object.object.take_prev_object()
        .map(|target_object| {
            object_switch_inner(cur_object, target_object.as_ref(), false)
        })
}

fn object_switch_inner(cur_object: StandaloneObject, target_object: &ObjectRef, is_register_prev_object: bool) -> StandaloneObject {
    //TODO VM内のコードと重複が多いのでどうにかしたい。最後のObject::register_schedulerをVMの中では呼べないところだけ異なる。
    let mailbox = target_object.mailbox();
    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
    let mut standalone = Object::unregister_scheduler(mailbox);

    if is_register_prev_object {
        //現在のオブジェクトに対応するObjectRefを作成
        //※このObjectRefはSwitch先のオブジェクトのヒープに作成される
        let prev_object = cur_object.object().make_object_ref(&standalone.object).unwrap();
        //return-object-switchでもどれるようにするために移行元のオブジェクトを保存
        standalone.object.set_prev_object(&prev_object);
    }

    //現在のオブジェクトをスケジューラに登録
    Object::register_scheduler(cur_object);

    standalone
}