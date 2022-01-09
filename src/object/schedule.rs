use std::cell::RefCell;
use std::thread::{self};
use std::sync::{mpsc, Arc, Weak};


use super::Object;

pub struct Scheduler {
    tx: mpsc::Sender<Envelope>,
    join_handle: thread::JoinHandle<()>,
    //obj: Vec<Weak<Object>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        let (tx, join_handle) = scheduler_main();

        Scheduler {
            tx,
            join_handle,
        }
    }

    pub fn add_object(&self, obj: Arc<RefCell<Object>>) {
        let msg = Envelope::NewObj(obj);
        //TODO sendに失敗した場合、スケジューラスレッドがpanicなどで消えている可能性がある。
        //再起動処理をするか？
        self.tx.send(msg).unwrap();
    }
}

enum Envelope {
    NewObj(Arc<RefCell<Object>>),
}

unsafe impl Send for Envelope {}

fn scheduler_main() -> (mpsc::Sender<Envelope>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<Envelope>();

    let join_handle = thread::spawn(move || {
        let mut objects: Vec<Weak<RefCell<Object>>> = Vec::new();
        let mut index: usize = 0;

        loop {
            match rx.try_recv() {
                Ok(Envelope::NewObj(obj)) => {
                    //弱参照としてオブジェクトを保持する
                    objects.push(Arc::downgrade(&obj));
                },
                Err(mpsc::TryRecvError::Empty) => {
                    //do noting
                },
                Err(mpsc::TryRecvError::Disconnected) => {
                    //End of scheduler thread
                    break;
                }
            }

            //待機中のオブジェクトが存在しているなら
            if objects.is_empty() == false {
                //待機中のオブジェクトを実行する
                index = index % objects.len();
                let obj = &objects[index];
                match obj.upgrade() {
                    Some(obj) => {
                        //オブジェクトへの参照を取得できたら、実行する。
                        obj.borrow_mut().do_work(1000);

                        index += 1;
                    },
                    None => {
                        //参照先のオブジェクトは消えてしまっているので、スケジューラー内からも削除する
                        objects.remove(index);
                    }
                }
            }

            //TODO 1ループの実行時間を計測し、短すぎる場合は適当なスリープを入れる
            thread::sleep(std::time::Duration::from_millis(0));
        }
    });

    (tx, join_handle)
}
