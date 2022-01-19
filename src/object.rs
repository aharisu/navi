mod fixed_size_alloc;
mod world;
pub mod mm;
mod balance;
mod schedule;
pub mod mailbox;


use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Weak, Mutex};
use std::cell::RefCell;

use once_cell::sync::Lazy;

use crate::value::*;
use crate::ptr::*;
use crate::err::{*, OutOfMemory};

use crate::value::any::Any;
use crate::value::func::Func;
use crate::value::list::ListBuilder;
use crate::value::object_ref::ObjectRef;
use crate::vm::{self, VMState, ExecException};

use self::fixed_size_alloc::FixedSizeAllocator;
use self::mm::{GCAllocationStruct, Heap};
use self::mailbox::*;

pub trait Allocator {
    fn alloc<T: NaviType>(&mut self) -> Result<UIPtr<T>, OutOfMemory>;
    fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> Result<UIPtr<T>, OutOfMemory>;
    fn force_allocation_space(&mut self, size: usize) -> Result<(), OutOfMemory>;
    fn is_in_heap_object<T: NaviType>(&self, v: &T) -> bool;
    fn do_gc(&mut self);
    fn heap_used(&self) -> usize;
}

enum SuspendState {
    Sleep,
    VMSuspend(Arc<Mutex<MailBox>>, ReplyToken),
    WaitReply(Ref<reply::Reply>, Arc<Mutex<MailBox>>, ReplyToken),
}

impl SuspendState {
    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Sleep)
    }
}

struct ObjectGCRootValues {
    suspend_state: SuspendState,

    //object-switchで切り替えた時の、切り替え前オブジェクト。
    prev_object:Option<Ref<ObjectRef>>,

    world: world::World,

    vm_state: VMState,

    captures: FixedSizeAllocator<Ref<Any>>,

    receiver_vec: Vec<(Ref<Any>, Ref<list::List>)>,
    receiver_closure: Option<Ref<compiled::Closure>>,
}

impl mm::GCRootValueHolder for ObjectGCRootValues {
    fn for_each_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        self.vm_state.for_each_all_alived_value(arg, callback);

        if let Some(prev_object) = self.prev_object.as_mut() {
            callback(prev_object.cast_mut_value(), arg);
        }

        //グローバルスペース内で保持している値
        self.world.for_each_all_value(|v :&mut Ref<Any>| {
            callback(v, arg);
        });

        //キャプチャーしているローカル変数
        unsafe {
            self.captures.for_each_used_value(|refer| {
                callback(refer, arg);
            });
        }

        //オブジェクトが持つメッセージレシーバーオブジェクト
        self.receiver_vec.iter_mut().for_each(|(pat, body)| {
            callback(pat, arg);
            callback(body.cast_mut_value(), arg);
        });

        //レシーバークロージャ
        if let Some(closure) = self.receiver_closure.as_mut() {
            callback(closure.cast_mut_value(), arg);
        }
    }
}

pub struct Object {
    id: usize,

    //MailBoxへの弱参照。
    //ObjectはMailBoxから強参照されている、相互参照の関係。
    //MailBoxがDropされるとき、Objectも同時にDropされる。
    mailbox: Weak<Mutex<MailBox>>,

    heap: Heap,

    values: ObjectGCRootValues,
}

//基本的な生成関数やアクセサ
impl Object {
    fn new(id: usize, mailbox: Weak<Mutex<MailBox>>) -> Self {
        let mut obj = Object {
            id,
            mailbox: mailbox,

            heap: Heap::new(mm::StartHeapSize::Default),

            values: ObjectGCRootValues {
                suspend_state: SuspendState::Sleep,

                prev_object: None,

                world: world::World::new(),

                vm_state: VMState::new(),

                captures: FixedSizeAllocator::new(),

                receiver_vec: Vec::new(),
                receiver_closure: None,
            }
        };
        obj.register_core_global();

        obj
    }

    #[allow(invalid_value)]
    fn dup(object: &Object, id: usize, mailbox: Weak<Mutex<MailBox>>) -> Self {
        //TODO 複製元オブジェクトのグローバル変数としてReplyが存在するとエラー!!!!

        let mut obj_cloned = Object {
            id,
            mailbox,

            //複製元のヒープ内オブジェクトがすべて収まる範囲の新しいヒープを作成
            heap: Heap::new_capacity(object.heap.used()),
            //valuesは新しいヒープにコピーしないといけないので、現時点ではダミーの値を入れておく
            values: unsafe { MaybeUninit::uninit().assume_init() }
        };

        let values = {
            //各値はまだ複製元のObjectのヒープ内にある
            let mut values = ObjectGCRootValues {
                    suspend_state: SuspendState::Sleep,
                    prev_object: None,
                    world: object.values.world.clone(),
                    vm_state: VMState::new(),
                    captures: FixedSizeAllocator::new(),

                    receiver_vec: object.values.receiver_vec.clone(),
                    receiver_closure: object.values.receiver_closure.clone(),
                };

            let mut allocator = AnyAllocator::Object(&mut obj_cloned);
            //グローバルスペース内で保持している値をすべて新しいオブジェクト内のヒープに移す
            values.world.for_each_all_value(|v :&mut Ref<Any>| {
                let cloned = NaviType::clone_inner(v.as_ref(), &mut allocator).unwrap();
                v.update_pointer(cloned.raw_ptr());
            });

            //オブジェクトが持つメッセージレシーバーオブジェクト
            values.receiver_vec.iter_mut().for_each(|(pat, body)| {
                let pat_cloned = NaviType::clone_inner(pat.as_ref(), &mut allocator).unwrap();
                pat.update_pointer(pat_cloned.raw_ptr());

                let body_cloned = NaviType::clone_inner(body.as_ref(), &mut allocator).unwrap();
                body.update_pointer(body_cloned.raw_ptr());
            });

            //レシーバークロージャ
            if let Some(closure) = values.receiver_closure.as_mut() {
                let closure_cloned = NaviType::clone_inner(closure.as_ref(), &mut allocator).unwrap();
                closure.update_pointer(closure_cloned.raw_ptr());
            }

            values
        };

        //valuesはダミー用の不正な値になっているので、クローン済みの正しい値を書き込む
        unsafe {
            std::ptr::write(&mut obj_cloned.values, values);
        }

        obj_cloned
    }

    #[cfg(test)]
    pub(crate) fn new_for_test() -> Self {
        new_object().object
    }

    #[inline]
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn make_object_ref<A: Allocator>(&self, allocator: &mut A) -> Option<NResult<ObjectRef, OutOfMemory>> {
        self.mailbox.upgrade()
            .map(|mailbox| {
                object_ref::ObjectRef::alloc(self.id,  mailbox, allocator)
            })
    }

    pub fn set_prev_object(&mut self, prev_object: &Ref<ObjectRef>) {
        self.values.prev_object = Some(prev_object.clone());
    }

    pub fn take_prev_object(&mut self) -> Option<Ref<ObjectRef>> {
        self.values.prev_object.take()
    }

    #[inline(always)]
    pub fn vm_state(&mut self) -> &mut VMState {
        &mut self.values.vm_state
    }

    pub fn find_global_value(&self, symbol: &symbol::Symbol) -> Option<Ref<Any>> {
        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.values.world.get(symbol) {
            Some(v.clone())
        } else {
            None
        }
    }

    pub fn define_global_value<Key: AsRef<str>, V: NaviType>(&mut self, key: Key, v: &Ref<V>) {
        self.values.world.set(key, v.cast_value())
    }

    fn register_core_global(&mut self) {
        register_global(self);
        object_ref::register_global(self);
        syntax::register_global(self);
        any::register_global(self);
        tuple::register_global(self);
        array::register_global(self);
        list::register_global(self);
        reply::register_global(self);
    }

    pub fn capture<T: NaviType>(&mut self, v: Ref<T>) -> Cap<T> {
        unsafe {
            let ptr = self.values.captures.alloc();
            let ptr = ptr as *mut Ref<T>;

            let mut cap = Cap::new(ptr);
            cap.update_pointer(v);

            cap
        }
    }

    pub(crate) fn release_capture<T: NaviType>(cap: &Cap<T>) {
        unsafe {
            let ptr = cap.cast_value().ptr();
            FixedSizeAllocator::<Ref<Any>>::free(ptr);
        }
    }

}

impl Object {

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

    ///
    /// # Returns
    /// * `Exception` is one of the following
    /// OutOfMemory
    /// MySelFObjectDeleted
    pub fn send_message(&mut self, target_obj: &Reachable<ObjectRef>, message: MessageKind) -> NResult<Any, Exception> {
        if let Some(mailbox) = self.mailbox.upgrade() {
            //戻り値を受け取るために自分自身のメールボックスをメッセージ送信相手に渡す
            let reply_token = target_obj.as_ref().recv_message(message, mailbox)?;
            //MailBoxを保持しているObjectRef値がなくなってしまうと、メッセージを送信した先のオブジェクトが削除される可能性がある。
            //そうなると一生Replyを受け取ることができなくなるため、ArcをReply内にも保持させる。
            let refer_mailbox = target_obj.as_ref().mailbox();

            //返信を受け取るための特別な値を生成して返す
            let reply = crate::value::reply::Reply::alloc(reply_token, refer_mailbox, self)?;
            Ok(reply.into_value())

        } else {
            //mailboxが取得できない場合は、自分自身のオブジェクトが削除されようとしているとき。
            //処理を継続できないため無効な値を返して処理を終了させる
            Err(Exception::MySelfObjectDeleted)
        }
    }

    pub fn try_take_reply(&mut self, reply_token: ReplyToken) -> ResultNone<NResult<Any, Exception>, OutOfMemory> {
        if let Some(mailbox) = self.mailbox.upgrade() {
            //自分のオブジェクトに対応するメールボックスのロックを取得
            let mut mailbox = mailbox.lock().unwrap();

            //メールボックスに返信がないか確認
            match mailbox.try_take_reply(reply_token) {
                Some(reply) => {
                    //MailBoxのロックを保持している間は、MailBox内ヒープのGCは発生しないため強制的にReachableに変換する。
                    match reply {
                        Ok(result) => {
                            let result = unsafe { result.into_reachable() };
                            //valはMailBox内のヒープに確保された値なので、Object内ヒープに値をクローンする
                            let mut allocator = AnyAllocator::Object(self);
                            match crate::value::value_clone(&result, &mut allocator) {
                                Ok(cloned) => {
                                    ResultNone::Ok(Ok(cloned))
                                }
                                Err(oom) => {
                                    ResultNone::Err(oom)
                                }
                            }
                        }
                        Err(err) => {
                            //errはMailBox内のヒープに確保された値なので、Object内ヒープにクローンする
                            let mut allocator = AnyAllocator::Object(self);
                            match unsafe { err.value_clone_gcunsafe(&mut allocator) } {
                                Ok(cloned) => {
                                    ResultNone::Ok(Err(cloned))
                                }
                                Err(oom) => {
                                    ResultNone::Err(oom)
                                }
                            }
                        }
                    }
                }
                None => {
                    ResultNone::None
                }
            }

        } else {
            //MailBoxが先に削除されているケースは、MailBoxがどのオブジェクトからも参照されなくなり、ARcによりすでにDropされてしまっているケース。※Standalone時を除く。
            //MailBoxとObjectは一心同体ですぐに今実行中のオブジェクトも削除される。
            //処理を継続する必要はないためNoneを返して実行を中断させる
            ResultNone::None
        }
    }

    pub fn add_receiver(&mut self, pattern: &Reachable<Any>, body: &Reachable<list::List>) {
        //コンテキストが持つレシーバーリストに追加する
        self.values.receiver_vec.push((pattern.make(), body.make()));
        //レシーブを実際に行う処理は遅延して構築する。
        //レシーバーのパターンはmatch構文のpatternに置き換えられる。
        //構築にはレシーバーパターンすべて必要になり、一つのレシーバーを追加するたびに再構築するのではコストが大きい。
        self.values.receiver_closure = None;
    }

    pub fn do_work(&mut self, reduction_count: usize) -> Result<(), OutOfMemory> {

        match self.values.suspend_state.take() {
            SuspendState::VMSuspend(reply_to_mailbox, reply_token) => {
                let result = vm::resume(reduction_count, self);
                self.apply_message_finish(result, reply_to_mailbox, reply_token)
            }
            SuspendState::WaitReply(reply, reply_to_mailbox, reply_token) => {
                self.wait_reply(reply, reply_to_mailbox, reply_token)
            }
            SuspendState::Sleep => {
                if let Some(mailbox) = self.mailbox.upgrade() {
                    let data =  {
                        //MailBoxは複数スレッド(複数オブジェクト)間で共有されているのでロックを取得してから操作を行う
                        let mut mailbox = mailbox.lock().unwrap();

                        mailbox.pop_inbox().map(|mut data| {
                            //messageの値はMailBox内のヒープに割り当てられいる。
                            //メッセージの値を自分自身のヒープ内にコピーする
                            let mut allocator = AnyAllocator::Object(self);
                            match unsafe { data.kind.value_clone_gcunsafe(&mut allocator) } {
                                Ok(kind) => {
                                    data.kind = kind;
                                    Ok(data)
                                }
                                Err(oom) => {
                                    Err((data.reply_to_mailbox, data.reply_token, oom))
                                }
                            }
                        })
                    };

                    //メッセージを受信していたら
                    if let Some(data) = data {
                        match data {
                            Ok(data) => {
                                match data.kind {
                                    MessageKind::Message(msg) => {
                                        //受信処理を実行
                                        self.apply_message(msg, data.reply_to_mailbox, data.reply_token, reduction_count)
                                    }
                                    MessageKind::Duplicate => {
                                        //TODO World内にReplyを含んでいないことを確認する

                                        //自分自身のObjectの複製を作成する
                                        let standalone = duplicate_object(self);
                                        let id = standalone.object().id();

                                        //Objectの所有権と実行権をスケジューラに譲る。
                                        //Objectとやり取りするためのMailBoxを取得
                                        let mailbox = Self::register_scheduler(standalone);

                                        let objectref = ObjectRef::alloc(id, mailbox,self)?;

                                        //作成したObjectRefを返信する
                                        self.apply_message_finish(Ok(objectref.into_value()), data.reply_to_mailbox, data.reply_token)
                                    }
                                }

                            }
                            Err((reply_to_mailbox, reply_token, e)) => {
                                //自分自身のオブジェクト内でOOMが発生したため、エラーとして返信する
                                self.apply_message_finish(Err(ExecException::from(e)), reply_to_mailbox, reply_token)
                            }
                        }
                    } else {
                        //メッセージを受信できていない、正常終了
                        Ok(())
                    }
                } else {
                    //メールボックスの強参照が取得できなかった場合は、オブジェクトを削除しようとしている状態なので何もせずに終了
                    //TODO 通常終了と区別をつけるために特別な値を返すか？
                    Ok(())
                }
            }
        }
    }

    fn apply_message(&mut self
        , msg: Ref<Any>, reply_to_mailbox: Arc<Mutex<MailBox>>, reply_token: ReplyToken
        , mut reduction_count: usize) -> Result<(), OutOfMemory> {
        let obj = self;
        let message = msg.reach(obj);

        //レシーバーのパターンマッチ式がまだ構築されていなければ
        if obj.values.receiver_closure.is_none() {
            let mut builder_fun = ListBuilder::new(obj);
            //(fun)
            builder_fun.append(crate::value::syntax::literal::fun().cast_value(), obj)?;
            //(msg_var)
            let paramter = list::List::alloc_tail(literal::msg_symbol().cast_value(), obj)?.into_value();
            //(fun (msg_var))
            builder_fun.append(&paramter.reach(obj), obj)?;

            //パターンマッチ部分を構築
            //(match msg_var (pattern body) (pattern2 body2) ...)
            let match_ = {
                let mut builder_match = ListBuilder::new(obj);
                //(match)
                builder_match.append(crate::value::syntax::literal::match_().cast_value(), obj)?;
                //(match msg_var)
                builder_match.append(literal::msg_symbol().cast_value(), obj)?;

                for (pattern, body) in obj.values.receiver_vec.clone().into_iter() {
                    //(pattern body)
                    let pattern = pattern.reach(obj);
                    let body = body.reach(obj);

                    let clause = list::List::alloc(&pattern, &body, obj)?.into_value().reach(obj);
                    builder_match.append(&clause, obj)?;
                }
                builder_match.get()
            };
            //(fun (msg_var) (match msg_var (pattern body) (pattern2 body2) ...))
            builder_fun.append(&match_.into_value().reach(obj), obj)?;

            //メッセージレシーバ用のクロージャをコンパイル
            let receiver = builder_fun.get().into_value().reach(obj);

            //クロージャを生成するコードを実行
            let message_receiver = crate::eval::eval(&receiver, obj).unwrap();
            //実行結果は必ずコンパイル済みクロージャなのでuncheckedでキャスト
            let message_receiver = unsafe { message_receiver.cast_unchecked::<compiled::Closure>() }.clone();

            obj.values.receiver_closure = Some(message_receiver);

            //TODO パターンマッチ生成部分でもreduction_countを一定減少させる
            reduction_count = reduction_count.saturating_sub(100);
        }

        //メッセージに対してパターンマッチングと対応する処理を実行するクロージャ
        let closure = obj.values.receiver_closure.as_ref().unwrap().clone().reach(obj);
        //受信したメッセージを引数の形に変換
        let args_iter = std::iter::once(message.make());
        //VMの時間制限
        let limit = vm::WorkTimeLimit::Reductions(reduction_count);

        //メッセージをクロージャに適用してパターンマッチを実行する
        let result = vm::closure_call(&closure, args_iter, limit, obj);
        obj.apply_message_finish(result, reply_to_mailbox, reply_token)
    }

    fn apply_message_finish(&mut self, result: Result<Ref<Any>, vm::ExecException>
        , reply_to_mailbox: Arc<Mutex<MailBox>>, reply_token: ReplyToken) -> Result<(), OutOfMemory> {

        match result {
            Ok(result) => {
                if let Some(reply) = result.try_cast::<reply::Reply>() {
                    self.wait_reply(reply.clone(), reply_to_mailbox, reply_token)

                } else {
                    self.send_reply(Ok(result), reply_to_mailbox, reply_token)?;

                    //残ったreductions分もう一度do_workを実行する
                    let remain = self.vm_state().remain_reductions();
                    self.do_work(remain)
                }
            }
            Err(vm::ExecException::TimeLimit) => {
                //VMの状態をsuspendにして、次回のdo_work時に処理を継続する
                self.values.suspend_state = SuspendState::VMSuspend(reply_to_mailbox, reply_token);
                Ok(())
            },
            Err(vm::ExecException::WaitReply) => {
                //VMの状態をsuspendにして、次回のdo_work時に処理を継続する
                self.values.suspend_state = SuspendState::VMSuspend(reply_to_mailbox, reply_token);
                Ok(())
            }
            Err(vm::ExecException::MySelfObjectDeleted) => {
                //実行中のオブジェクトが削除されようとしているので、これ以上何もせずに終了させる
                Ok(())
            }
            Err(vm::ExecException::Exit) => {
                //メインプロセス以外でのExitは無視
                //TODO Exitはプロセスを削除するか？
                Ok(())
            }
            Err(vm::ExecException::ObjectSwitch(_)) => {
                //Objectの切り替えはグローバル環境のトップレベルでのみ許可されているため、ここでは絶対に発生しない。
                unreachable!()
            }
            Err(vm::ExecException::Exception(e)) => {
                self.send_reply(Err(e), reply_to_mailbox, reply_token)?;

                //残ったreductions分もう一度do_workを実行する
                let remain = self.vm_state().remain_reductions();
                self.do_work(remain)
            },
        }
    }

    fn wait_reply(&mut self, reply: Ref<reply::Reply>, reply_to_mailbox: Arc<Mutex<MailBox>>, reply_token: ReplyToken) -> Result<(), OutOfMemory> {
        let mut cap = reply.capture(self);
        match reply::Reply::try_get_reply_value(&mut cap, self) {
            ResultNone::Ok(result) => {
                self.send_reply(result, reply_to_mailbox, reply_token)?;

                //残ったreductions分もう一度do_workを実行する
                let remain = self.vm_state().remain_reductions();
                self.do_work(remain)
            }
            ResultNone::Err(oom) => {
                Err(oom)
            }
            ResultNone::None => {
                self.values.suspend_state = SuspendState::WaitReply(cap.take(), reply_to_mailbox, reply_token);
                Ok(())
            }
        }
    }

    fn send_reply(&mut self, result: NResult<Any, Exception>, reply_to_mailbox: Arc<Mutex<MailBox>>, reply_token: ReplyToken) -> Result<(), OutOfMemory> {
        match result {
            Ok(v) => {
                //結果を送信元のオブジェクト(MailBox)に返す
                {
                    //resultの値はMailBox上のヒープにコピーされるだけ。
                    //自分自身のGCは絶対に発生しないためinto_reachableを行う。
                    let result = unsafe { v.into_reachable() };

                    //返信先メールボックスのロックを取得
                    let mut reply_to_mailbox = reply_to_mailbox.lock().unwrap();
                    //返信を送信
                    //TODO 相手先メールボックスがOOMの可能性がある。どうする？
                    reply_to_mailbox.recv_reply(Ok(&result), reply_token)?;
                }
            }
            Err(err) => {
                //エラーが発生したら、そのエラーの内容をsend元に返信として伝える
                //resultの値はMailBox上のヒープにコピーされるだけ。
                //自分自身のGCは絶対に発生しないためinto_reachableを行う。

                //返信先メールボックスのロックを取得
                let mut reply_to_mailbox = reply_to_mailbox.lock().unwrap();
                //返信を送信
                //TODO 相手先メールボックスがOOMの可能性がある。どうする？
                reply_to_mailbox.recv_reply(Err(err), reply_token)?;
            }
        }

        Ok(())
    }

}

impl Eq for Object {}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        //同じヒープを持っている場合は同じコンテキストと判断する
        std::ptr::eq(&self.heap, &other.heap)
    }
}

impl Allocator for Object {
    fn alloc<T: NaviType>(&mut self) -> Result<UIPtr<T>, OutOfMemory> {
        self.heap.alloc(&mut self.values)
    }

    fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> Result<UIPtr<T>, OutOfMemory> {
        self.heap.alloc_with_additional_size(additional_size, &mut self.values)
    }

    fn force_allocation_space(&mut self, size: usize) -> Result<(), OutOfMemory> {
        self.heap.force_allocation_space(size, &mut self.values)
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

static SYMBOL_MSG: Lazy<GCAllocationStruct<symbol::StaticSymbol>> = Lazy::new(|| {
    symbol::gensym_static("msg")
});

fn func_exit(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    //終了させるための関数なので、エラーとしてExitを返すだけ
    Err(Exception::Exit)
}

static FUNC_EXIT: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("exit",
            &[],
            func_exit)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("exit", &Ref::new(&FUNC_EXIT.value));
}

mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn msg_symbol() -> Reachable<symbol::Symbol> {
        Reachable::new_static(SYMBOL_MSG.value.as_ref())
    }
}

//型パラメータがどうしても使えない場所で使用するAllocatorを実装する値を内包したenum
//TypeInfo内で保持されるclone_innerで使用している。
pub enum AnyAllocator<'a> {
    Object(&'a mut Object),
    MailBox(&'a mut MailBox),
}

impl <'a> Allocator for AnyAllocator<'a> {
    fn alloc<T: NaviType>(&mut self) -> Result<UIPtr<T>, OutOfMemory> {
        match self {
            AnyAllocator::Object(obj) => obj.alloc(),
            AnyAllocator::MailBox(mailbox) => mailbox.alloc(),
        }
    }

    fn alloc_with_additional_size<T: NaviType>(&mut self, additional_size: usize) -> Result<UIPtr<T>, OutOfMemory> {
        match self {
            AnyAllocator::Object(obj) => obj.alloc_with_additional_size(additional_size),
            AnyAllocator::MailBox(mailbox) => mailbox.alloc_with_additional_size(additional_size),
        }
    }

    fn force_allocation_space(&mut self, size: usize) -> Result<(), OutOfMemory> {
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

    fn do_gc(&mut self) {
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

fn duplicate_object(object: &Object) -> StandaloneObject {
    //メールボックスは新規作成する
    let mailbox = Arc::new(Mutex::new(MailBox::new()));
    //オブジェクトを識別するためのIDを生成
    let object_id = OBJECT_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    //ObjectはMailBoxを常に弱参照で保持する
    let obj = Object::dup(object, object_id, Arc::downgrade(&mailbox));

    StandaloneObject {
        object: obj,
        mailbox: mailbox
    }
}

//maybe oom
pub fn object_switch(cur_object: StandaloneObject, target_object: &ObjectRef) -> Result<StandaloneObject, Exception> {
    object_switch_inner(cur_object, target_object, true)
}

pub fn return_object_switch(mut cur_object: StandaloneObject) -> Option<StandaloneObject> {
    cur_object.object.take_prev_object()
        .map(|target_object| {
            //オブジェクトの確保を行わないため、例外は発生しない
            object_switch_inner(cur_object, target_object.as_ref(), false).unwrap()
        })
}

//maybe oom
fn object_switch_inner(cur_object: StandaloneObject, target_object: &ObjectRef, is_register_prev_object: bool) -> Result<StandaloneObject, Exception> {
    //TODO VM内のコードと重複が多いのでどうにかしたい。最後のObject::register_schedulerをVMの中では呼べないところだけ異なる。
    let mailbox = target_object.mailbox();
    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
    let mut standalone = Object::unregister_scheduler(mailbox);

    if is_register_prev_object {
        //現在のオブジェクトに対応するObjectRefを作成
        //※このObjectRefはSwitch先のオブジェクトのヒープに作成される
        let prev_object = cur_object.object().make_object_ref(&mut standalone.object).unwrap()?;
        //return-object-switchでもどれるようにするために移行元のオブジェクトを保存
        standalone.object.set_prev_object(&prev_object);
    }

    //現在のオブジェクトをスケジューラに登録
    Object::register_scheduler(cur_object);

    Ok(standalone)
}