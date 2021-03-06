use std::fmt::Debug;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::mem::size_of;

use once_cell::sync::Lazy;

use crate::err::NResult;
use crate::err::OutOfMemory;
use crate::object::StandaloneObject;
use crate::object::mm::GCAllocationStruct;
use crate::object::mm::usize_to_ptr;

use crate::object::Object;
use crate::value::*;
use crate::value::any::Any;
use crate::value::app;
use crate::ptr::*;
use crate::err;

pub mod tag {
    pub const JUMP_OFFSET: u8 = 0;
    pub const IF: u8 = 1;
    pub const REF_LOCAL: u8 = 2;
    pub const REF_FREE: u8 = 23;
    pub const REF_GLOBAL: u8 = 3;
    pub const CONST_CAPTURE: u8 = 4;
    pub const CONST_STATIC: u8 = 5;
    pub const CONST_IMMIDIATE: u8 = 6;
    pub const PUSH_ARG: u8 = 7;
    pub const PUSH_ARG_UNCHECK: u8 = 24;
    pub const PUSH_APP: u8 = 25;
    pub const LET_LOCAL: u8 = 8;
    pub const LET_GLOBAL: u8 = 9;
    pub const DEF_RECV:u8 = 10;
    pub const OBJECT_SWITCH:u8 = 16;
    pub const RETURN_OBJECT_SWITCH:u8 = 26;
    pub const POP_ENV:u8 = 11;
    pub const PUSH_EMPTY_ENV:u8 = 12;
    pub const CLOSURE:u8 = 13;
    pub const CAPTURE_FREE_REF_LOCAL: u8 = 27;
    pub const CAPTURE_FREE_REF_FREE: u8 = 28;
    pub const RETURN:u8 = 14;
    pub const CALL_PREPARE:u8 = 15;
    pub const CALL_TAIL_PREPARE:u8 = 22;
    pub const CALL:u8 = 18;
    pub const CALL_TAIL:u8 = 17;
    pub const CALL_RESUME_FUNC:u8 = 29;
    pub const AND:u8 = 19;
    pub const OR:u8 = 20;
    pub const MATCH_SUCCESS:u8 = 21;

    //next number 30
}

#[derive(Debug)]
pub enum ExecException {
    ObjectSwitch(StandaloneObject),
    Exception(err::Exception),
}

impl From<err::OutOfMemory> for ExecException {
    fn from(_: err::OutOfMemory) -> Self {
        ExecException::Exception(err::Exception::OutOfMemory)
    }
}

impl From<err::Exception> for ExecException {
    fn from(this: err::Exception) -> Self {
        ExecException::Exception(this)
    }
}

#[derive(Debug)]
#[repr(C)]
struct Continuation {
    prev: *mut Continuation,
    pc: u64,
    env: *mut Environment,
    argp: *mut Environment,
    code: Option<Ref<compiled::Code>>,
}

#[derive(Debug)]
#[repr(C)]
struct Environment {
    //一つ上の階層のEnvironmentを指すポインタ。
    //Environmentはスタック内に確保されるため値のMoveは行われないため、ポインタを直接持つ。
    up: *mut Environment,
    //Environmentが持つデータの数(ローカル変数の数)
    size: usize,
    //ローカル変数への参照がsize分だけココ以降のデータ内に保存されている。
}

#[repr(C)]
struct FuncSuspendInfo {
    code: Ref<compiled::Code>,
    pc: u64,
    resume_func: fn(&mut Object) -> NResult<Any, err::Exception>,
}

pub fn save_func_suspend_info(resume_func: fn(&mut Object) -> NResult<Any, err::Exception>, obj: &mut Object) {
    let info = FuncSuspendInfo {
        code: obj.vm_state().code.clone(),
        pc: obj.vm_state().pc,
        resume_func: resume_func,
    };
    obj.vm_state().stack.push(info);

    //スタックに保存されたresume_funcを呼ぶための命令を実行するコードをvm_satateに設定
    obj.vm_state().code = literal::code_call_suspend_func().make();
    obj.vm_state().pc = 0;
}

#[derive(Debug)]
pub struct VMStack {
    stack: *mut u8,
    pos: *mut u8,
    stack_layout: std::alloc::Layout,
}

impl VMStack {
    fn new(size: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(size, size_of::<usize>()).unwrap();
        let stack = unsafe { std::alloc::alloc(layout) };

        VMStack {
            stack: stack,
            pos: stack,
            stack_layout: layout,
        }
    }

    pub fn push<T>(&mut self, v: T) -> *mut T {
        let t_ptr = unsafe {
            let t_ptr = self.pos as *mut T;

            self.pos = self.pos.add(size_of::<T>());
            t_ptr.write(v);

            t_ptr
        };

        t_ptr
    }

    pub fn pop<T>(&mut self) -> T {
        unsafe {
            self.pos = self.pos.sub(size_of::<T>());
            let t_ptr = self.pos as *mut T;

            t_ptr.read()
        }
    }

    fn pop_from_size(&mut self, decriment: usize) {
        unsafe {
            self.pos = self.pos.sub(decriment);
        }
    }

}

impl Drop for VMStack {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.stack, self.stack_layout)
        }
    }
}

pub fn is_true(v: &Any) -> bool {
    //predの結果がfalse値の場合だけ、falseとして扱う。それ以外の値はすべてtrue
    if let Some(v) = v.try_cast::<bool::Bool>() {
        v.is_true()
    } else {
        true
    }
}

#[inline]
pub fn refer_arg<T: NaviType>(index: usize, obj: &mut Object) -> Ref<T> {
    let v = refer_local_var(obj.vm_state().env, 0, index + 1);
    //Funcの引数はすべて型チェックされている前提なのでuncheckedでキャストする
    unsafe { v.cast_unchecked::<T>().clone() }
}

#[inline]
pub fn refer_rest_arg<T: NaviType>(index: usize, rest_index: usize, obj: &mut Object) -> Ref<T> {
    refer_arg::<T>(index + rest_index, obj)
}

fn refer_local_var(mut env: *const Environment, mut frame_offset: usize, index: usize) -> Ref<Any> {
    //目的の位置まで環境を上に上に順に辿っていく
    while frame_offset > 0 {
        env = unsafe { (*env).up };
        frame_offset -= 1;
    }

    //目的の環境内にあるローカルフレームから値を取得
    unsafe {
        //ローカルフレームは環境ヘッダの後ろ側にある
        let frame_ptr = env.add(1) as *mut Ref<Any>;
        let cell = frame_ptr.add(index as usize);
        (*cell).clone()
    }
}

#[derive(Debug)]
pub struct VMState {
    reductions: usize,
    code: Ref<compiled::Code>,
    pc: u64, //code中の現在プログラムカウンタ
    acc: Ref<Any>,
    stack: VMStack,
    cont: *mut Continuation,
    env: *mut Environment,
    argp: *mut Environment,
}

impl VMState {
    pub fn new() -> Self {
        VMState {
            reductions: 0,
            code: Ref::from(std::ptr::null_mut()), //ダミーのためにヌルポインターで初期化
            pc: 0,
            acc: bool::Bool::false_().into_ref().into_value(),
            stack: VMStack::new(1024 * 3),
            cont: std::ptr::null_mut(),
            env: std::ptr::null_mut(),
            argp: std::ptr::null_mut(),
        }
    }

    #[inline(always)]
    pub fn remain_reductions(&self) -> usize {
        self.reductions
    }

    #[inline(always)]
    pub fn stack(&mut self) -> &mut VMStack {
        &mut self.stack
    }

    pub fn for_each_all_alived_value(&mut self, arg: *mut u8, callback: fn(&mut Ref<Any>, *mut u8)) {
        if self.code.raw_ptr().is_null() == false {
            callback(self.code.cast_mut_value(), arg);
        }

        callback(&mut self.acc, arg);

        {
            let mut cont = self.cont;
            while cont.is_null() == false {
                unsafe {
                    if let Some(code) = (*cont).code.as_mut() {
                        callback(code.cast_mut_value(), arg);
                    }

                    cont = (*cont).prev;
                }
            }
        }

        {
            let mut env = self.env;
            while env.is_null() == false {
                unsafe {
                    //ローカルフレーム内の変数の数
                    let len = (*env).size;

                    //ローカルフレームは環境ヘッダの後ろ側にある
                    let frame_ptr = env.add(1) as *mut Ref<Any>;
                    for index in 0 .. len {
                        let cell = frame_ptr.add(index as usize);
                        let v = &mut *cell;
                        callback(v, arg);
                    }

                    env = (*env).up;
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum WorkTimeLimit {
    Inf,
    Reductions(usize),
    TakeOver,
}

pub fn app_call(app: &Reachable<app::App>, args_iter: impl Iterator<Item=Ref<Any>>
    , limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecException> {
    //ContinuationとEnvironmentのフレームをプッシュ
    {
        let mut buf: Vec<u8> = Vec::with_capacity(1);

        //関数の呼び出し前準備
        write_u8(tag::CALL_PREPARE, &mut buf);

        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //関数呼び出しの準備段階の実行は、実行時間に制限を設けない
        code_execute(&code,  WorkTimeLimit::Inf, obj).unwrap();
    }

    //APPをプッシュ
    {
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        //APP型は保証されているので型チェックなしでPUSHさせる
        write_u8(tag::PUSH_ARG_UNCHECK, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //accにFuncオブジェクトを設定
        obj.vm_state().acc = app.cast_value().make();
        //関数呼び出しの準備段階の実行は、実行時間に制限を設けない
        code_execute(&code, WorkTimeLimit::Inf, obj).unwrap();
    }

    //全ての引数をプッシュ
    {
        //PUSHするだけのコードを作成
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        write_u8(tag::PUSH_ARG, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        for arg in args_iter {
            //引数を順にaccに入れてPUSHを実行
            obj.vm_state().acc = arg.clone();

            //関数呼び出しの準備段階の実行は、実行時間に制限を設けない
            code_execute(&code, WorkTimeLimit::Inf, obj)?;
        }
    }

    //CALL命令を実行
    code_execute(&literal::code_call(), limit, obj)
}

pub fn code_execute(code: &Reachable<compiled::Code>, limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecException> {
    //実行対象のコードを設定
    obj.vm_state().code = code.make();
    obj.vm_state().pc = 0;

    loop {
        //直接executを実行する場合は最大最後まで実行できるようにするためにのワークサイズを設定する
        obj.vm_state().reductions = match limit {
            WorkTimeLimit::Inf => usize::MAX,
            WorkTimeLimit::Reductions(reductions) => reductions,
            //少なくとも一つの命令は実行完了できるように最低数(11)を指定する
            WorkTimeLimit::TakeOver => obj.vm_state().reductions.max(11),
         };

        //CALLを実行。結果を返り値にする
        match execute(obj) {
            Ok(result) => {
                return Ok(result);
            }
            Err(ExecException::ObjectSwitch(standalone)) => {
                return Err(ExecException::ObjectSwitch(standalone));
            }
            //特定のエラーは補足して処理を継続する
            Err(ExecException::Exception(err)) => {
                match err {
                    err::Exception::TimeLimit => {
                        if limit == WorkTimeLimit::Inf {
                            //実行時間がInfの場合はloopを継続して終了まで実行させる
                            //loopを継続させるためにここでは何もしない
                        } else {
                            //実行時間に制限があるときだけ、エラーを返す
                            return Err(ExecException::Exception(err));
                        }
                    }
                    err::Exception::WaitReply => {
                        if limit == WorkTimeLimit::Inf {
                            //他スレッドの処理が終わるまで時スレッドの処理をブロックして待つ。
                            //3ミリ秒という数字に理由はない。
                            std::thread::sleep(std::time::Duration::from_millis(3));
                        } else {
                            //実行時間に制限があるときだけ、エラーを返す
                            return Err(ExecException::Exception(err));
                        }
                    }
                    other => {
                        //その他の戻り値はそのまま返す
                        return Err(ExecException::Exception(other));
                    }
                }
            }
        }
    }
}

pub fn resume(limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecException> {
    obj.vm_state().reductions = match limit {
        WorkTimeLimit::Inf => usize::MAX,
        WorkTimeLimit::Reductions(reductions) => reductions,
        WorkTimeLimit::TakeOver => obj.vm_state().reductions,
        };

    execute(obj)
}

fn execute(obj: &mut Object) -> Result<Ref<Any>, ExecException> {
    let mut program = Cursor::new(obj.vm_state().code.as_ref().program());
    program.seek(SeekFrom::Start(obj.vm_state().pc as u64)).unwrap();

    macro_rules! tag_return {
        () => {
            //Continuationに保存されている状態を復元
            unsafe {
                let state = obj.vm_state();
                //スタック内でContinuationの値があるアドレスをスタックポインタにする
                state.stack.pos = state.cont as *mut u8;
                state.env = (*state.cont).env;
                state.argp = (*state.cont).argp;

                if let Some(cd) = (*state.cont).code.take() {
                    program = Cursor::new(cd.as_ref().program());
                    state.code = cd;
                    program.seek(SeekFrom::Start((*state.cont).pc as u64)).unwrap();
                }

                state.cont = (*state.cont).prev;
            }
        };
    }

    macro_rules! complete_arg {
        () => {
            //引数準備のためのEnv(argp)をEnvリストにつなげる
            let argp= obj.vm_state().argp;
            unsafe {
                (*argp).up = obj.vm_state().env;
            }
            //完成したargpを現在環境に設定する
            obj.vm_state().env = argp;
        };
    }

    macro_rules! let_local {
        ($exp:expr) => {
            let exp = $exp;
            obj.vm_state().stack.push(exp);
            //新しく追加した分、環境内のローカルフレームサイズを増やす
            unsafe {
                (*obj.vm_state().env).size += 1;
            }
        };
    }

    macro_rules! push_arg {
        ($exp:expr) => {
            let exp = $exp;
            obj.vm_state().stack.push(exp);
            //新しく追加した分、環境内のローカルフレームサイズを増やす
            unsafe {
                (*obj.vm_state().argp).size += 1;
            }
        };
    }

    let mut acc_reduce: usize = 0;

    macro_rules! reduce {
        ($exp:expr) => {
            acc_reduce += $exp;
        };
    }

    macro_rules! reduce_with_check_timelimit {
        ($exp:expr) => {
            reduce!($exp);
            let remain = obj.vm_state().reductions.saturating_sub(acc_reduce);
            if remain == 0 {
                //続きから実行できるように、PCを設定
                obj.vm_state().pc = program.position();

                return Err(ExecException::Exception(err::Exception::TimeLimit));
            } else {
                acc_reduce = 0;
                obj.vm_state().reductions = remain;
            }
        };
    }

    loop {
        let tag = {
            let mut tmp: [u8;1] = [0];
            //これ以上タグがなければ実行終了
            if program.read_exact(&mut tmp).is_err() {
                break;
            }

            tmp[0]
        };
        match tag {
            tag::JUMP_OFFSET => {
                let offset = read_u16(&mut program);
                program.seek(SeekFrom::Current(offset as i64)).unwrap();
            }
            tag::IF => {
                if is_true(obj.vm_state().acc.as_ref()) {
                    //falseだったときのジャンプオフセットを読み飛ばす
                    program.seek(SeekFrom::Current(2 as i64)).unwrap();

                } else {
                    //true節を読み飛ばす
                    let offset = read_u16(&mut program);
                    program.seek(SeekFrom::Current(offset as i64)).unwrap();
                }

                reduce!(1);
            }
            tag::REF_LOCAL => {
                let frame_offset = read_u16(&mut program);
                let cell_index = read_u16(&mut program);

                //目的の環境内にあるローカルフレームから値を取得
                obj.vm_state().acc = refer_local_var(obj.vm_state().env, frame_offset as usize, cell_index as usize);
            }
            tag::REF_FREE => {
                let frame_offset = read_u16(&mut program);
                let cell_index = read_u16(&mut program);

                //目的の環境内にあるローカルフレームから値を取得
                let closure = refer_local_var(obj.vm_state().env, frame_offset as usize, 0);
                if let Some(closure) = closure.try_cast::<compiled::Closure>() {
                    //クロージャ内で保持している自由変数を取得
                    obj.vm_state().acc = closure.as_ref().get(cell_index as usize);

                } else {
                    //0番目の値がclosure以外の場合、不具合なのでパニックさせる
                    panic!("need closure. but got {}", closure.as_ref())
                }
            }
            tag::REF_GLOBAL => {
                let const_index = read_u16(&mut program);
                let symbol = obj.vm_state().code.as_ref().get_constant(const_index as usize);
                let symbol = unsafe { symbol.cast_unchecked::<symbol::Symbol>() };
                if let Some(v) = obj.find_global_value(symbol.as_ref()) {
                    obj.vm_state().acc = v;

                } else {
                    return Err(ExecException::Exception(err::Exception::UnboundVariable(
                        err::UnboundVariable::new(symbol.clone())
                        )));
                }

                reduce!(2);
            }
            tag::CONST_CAPTURE => {
                let const_index = read_u16(&mut program);
                obj.vm_state().acc = obj.vm_state().code.as_ref().get_constant(const_index as usize);
            }
            tag::CONST_STATIC
            | tag::CONST_IMMIDIATE => {
                let data = read_usize(&mut program);
                let ptr = usize_to_ptr::<Any>(data);
                obj.vm_state().acc = ptr.into();
            }
            tag::PUSH_ARG => {
                let mut arg = obj.vm_state().acc.clone();

                let argp_env = obj.vm_state().argp;
                //引数準備中フレームからappを取得
                let app = refer_local_var(argp_env, 0, 0);
                //PUSH_APPの時点で型チェックされているので無条件でAPPに変換する
                let app = unsafe { app.cast_unchecked::<app::App>() };

                let index = unsafe { (*argp_env).size - 1 };
                let parameter = app.as_ref().parameter();
                let params = parameter.params();
                let param = if params.is_empty() {
                        None
                    } else if index < params.len() {
                        Some(&params[index])
                    } else if params[params.len() - 1].kind == app::ParamKind::Rest {
                        Some(&params[params.len() - 1])
                    } else {
                        None
                    };
                if let Some(param) = param {
                    // reply check
                    if param.force && arg.has_replytype() {
                        let result = check_reply(&mut arg, obj);

                        //まだ返信がない場合は、
                        if result? == false {
                            //もう一度PUSH_ARGが実行できるように、現在位置-1をresume後のPCとする
                            obj.vm_state().pc = program.position() - 1;

                            //引数の値にReply待ちを含んでいるため、返信を待つ
                            return Err(ExecException::Exception(err::Exception::WaitReply));
                        }
                    }

                    // type check
                    if arg.is_type(param.typeinfo) == false {
                        return Err(ExecException::Exception(err::Exception::ArgTypeMismatch(
                            err::ArgTypeMismatch::new(
                                String::from(app.as_ref().name()), index + 1,
                                arg, param.typeinfo))));
                    }
                }

                push_arg!(arg);
            }
            tag::PUSH_ARG_UNCHECK => {
                push_arg!(obj.vm_state().acc.clone());
            }
            tag::PUSH_APP => {
                let app = obj.vm_state().acc.clone();
                //TODO Reply check

                if app.is::<app::App>() {
                    // OK!!  do nothing
                } else {
                    return Err(ExecException::Exception(err::Exception::TypeMismatch(
                        err::TypeMismatch::new(app, app::App::typeinfo())
                    )));
                }

                push_arg!(app);
            }
            tag::LET_LOCAL => {
                let_local!(obj.vm_state().acc.clone());
            }
            tag::LET_GLOBAL => {
                let const_index = read_u16(&mut program);
                let symbol = obj.vm_state().code.as_ref().get_constant(const_index as usize);
                let symbol = unsafe { symbol.cast_unchecked::<symbol::Symbol>() };

                let acc = obj.vm_state().acc.clone();
                obj.define_global_value(symbol.as_ref(), &acc);

                reduce!(5);
            }
            tag::DEF_RECV => {
                let pattern_index = read_u16(&mut program);
                let body_index = read_u16(&mut program);

                let code = obj.vm_state().code.as_ref();
                let pattern = code.get_constant(pattern_index as usize).reach(obj);
                let body = unsafe { code.get_constant(body_index as usize).cast_unchecked::<list::List>() }.clone().reach(obj);

                //現在のオブジェクトにレシーバーを追加する
                obj.add_receiver(&pattern, &body);

                obj.vm_state().acc = bool::Bool::true_().into_ref().into_value();
            }
            tag::OBJECT_SWITCH => {
                let mut target_obj = obj.vm_state().acc.clone();

                // reply check
                if target_obj.has_replytype() {
                    let result = check_reply(&mut target_obj, obj);
                    //まだ返信がない場合は、
                    if result? == false {
                        //もう一度OBJECT_SWITCHが実行できるように、現在位置-1をresume後のPCとする
                        obj.vm_state().pc = program.position() - 1;

                        //引数の値にReply待ちを含んでいるため、返信を待つ
                        return Err(ExecException::Exception(err::Exception::WaitReply));
                    }
                }

                if let Some(target_obj) = target_obj.try_cast::<object_ref::ObjectRef>() {
                    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
                    let mailbox = target_obj.as_ref().mailbox();
                    let mut standalone = Object::unregister_scheduler(mailbox);

                    //現在のオブジェクトに対応するObjectRefを作成
                    //※このObjectRefはSwitch先のオブジェクトのヒープに作成される
                    let prev_object = obj.make_object_ref(standalone.mut_object());

                    //※現在実行中のObjectに対応するMailBoxは必ず存在している(StandaloneObjectの中で保持している)ため、unwrapする。
                    let prev_object = prev_object.unwrap()?;

                    //return-object-switchでもどれるようにするために移行元のオブジェクトを保存
                    standalone.mut_object().set_prev_object(&prev_object);

                    //object-siwtchはグローバル環境でしか現れない
                    return Err(ExecException::ObjectSwitch(standalone));
                } else {
                    return Err(ExecException::Exception(err::Exception::TypeMismatch(
                        err::TypeMismatch::new(target_obj, object_ref::ObjectRef::typeinfo())
                        )));
                }
            }
            tag::RETURN_OBJECT_SWITCH => {
                if let Some(prev_obj) = obj.take_prev_object() {
                    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
                    let mailbox = prev_obj.as_ref().mailbox();
                    let standalone = Object::unregister_scheduler(mailbox);

                    //object-siwtchはグローバル環境でしか現れない
                    return Err(ExecException::ObjectSwitch(standalone));
                } else {
                    return Err(ExecException::Exception(err::Exception::Other(format!("No return Object"))));
                }
            }
            tag::POP_ENV => {
                debug_assert!(!obj.vm_state().env.is_null());
                unsafe {
                    let local_frame_size = (*obj.vm_state().env).size;
                    //Envヘッダーとローカル変数のサイズ分、スタックポインタを下げる
                    let size = size_of::<Environment>() + (size_of::<Ref<Any>>() * local_frame_size);
                    obj.vm_state().stack.pop_from_size(size);

                    //現在のenvポインタを一つ上の環境に差し替える
                    obj.vm_state().env = (*obj.vm_state().env).up;
                };
            }
            tag::PUSH_EMPTY_ENV => {
                let new_env = Environment {
                    up: obj.vm_state().env,
                    size: 0,
                };
                //envポインタを新しく追加したポインタに差し替える
                obj.vm_state().env = obj.vm_state().stack.push(new_env);
            }
            tag::CLOSURE => {
                let num_args = read_u8(&mut program) as usize;

                //Closure内で使用されている定数一覧を取得するための変数
                let constant_start = read_u16(&mut program) as usize;
                let constant_len = read_u16(&mut program) as usize;

                //Closure内で使用している定数一覧を取得
                let constants = obj.vm_state().code.as_ref().get_constant_slice(constant_start, constant_start + constant_len);

                //Closure本体式の長さ
                let body_size = read_u16(&mut program) as usize;

                //自由変数の数
                let num_free_vars = read_u16(&mut program) as usize;

                //プログラムの中からClosureの本体を切り出す
                let mut closure_body:Vec<u8> = Vec::new();
                let cur = program.position() as usize;
                let buf = &program.get_ref()[cur .. cur + body_size];
                closure_body.write(buf).unwrap();

                //読み込んだClosure本体のデータ分、プログラムカウンタを進める
                program.seek(SeekFrom::Current(body_size as i64)).unwrap();

                let params:Vec<app::Param> = (0 .. num_args)
                    .map(|_| app::Param::new("v", app::ParamKind::Require, any::Any::typeinfo()))
                    .collect()
                    ;
                let parameter = app::Parameter::new(&params);

                obj.vm_state().acc = compiled::Closure::alloc(closure_body, constants, parameter, num_free_vars, obj)?.into_value();

                reduce!(5);
            }
            tag::CAPTURE_FREE_REF_LOCAL => {
                let frame_offset = read_u16(&mut program);
                let cell_index = read_u16(&mut program);

                //目的の環境内にあるローカルフレームから値を取得
                let v = refer_local_var(obj.vm_state().env, frame_offset as usize, cell_index as usize);

                let closure = &mut obj.vm_state().acc;
                let cell_index = read_u16(&mut program);
                if let Some(closure) = closure.try_cast_mut::<compiled::Closure>() {
                    closure.set(&v, cell_index as usize)
                } else {
                    panic!("need closure. but got {}", closure.as_ref())
                }
            }
            tag::CAPTURE_FREE_REF_FREE => {
                let frame_offset = read_u16(&mut program);
                let cell_index = read_u16(&mut program);

                //目的の環境内にあるローカルフレームから値を取得
                let closure = refer_local_var(obj.vm_state().env, frame_offset as usize, 0);
                if let Some(closure) = closure.try_cast::<compiled::Closure>() {
                    //クロージャ内で保持している自由変数を取得
                    let v = closure.as_ref().get(cell_index as usize);

                    let closure = &mut obj.vm_state().acc;
                    let set_cell_index = read_u16(&mut program);
                    if let Some(closure) = closure.try_cast_mut::<compiled::Closure>() {
                        closure.set(&v, set_cell_index as usize)
                    } else {
                        panic!("need closure. but got {}", closure.as_ref())
                    }

                } else {
                    //0番目の値がclosure以外の場合、不具合なのでパニックさせる
                    panic!("need closure. but got {}", closure.as_ref())
                }

            }

            tag::RETURN => {
                //Continuationに保存されている状態を復元
                tag_return!();
            }
            tag::CALL_PREPARE => {
                let new_cont = Continuation {
                    prev: obj.vm_state().cont,
                    code: None, //実行コードはCursorが所有権を持っているので実際にCallされる直前に設定する
                    pc: 0, //復帰するPCはCall直前に設定する
                    env: obj.vm_state().env,
                    argp: obj.vm_state().argp,
                };
                //contポインタを新しく追加したポインタに差し替える
                obj.vm_state().cont = obj.vm_state().stack.push(new_cont);

                let new_env = Environment {
                    up: std::ptr::null_mut(), //準備段階ではupポインタはNULLにする
                    size: 0,
                };
                //argpポインタを新しく追加したポインタに差し替える
                obj.vm_state().argp = obj.vm_state().stack.push(new_env);
            }
            tag::CALL_TAIL_PREPARE => {
                //末尾文脈のCALLではContinuationフレームを積まない
                let new_env = Environment {
                    up: std::ptr::null_mut(), //準備段階ではupポインタはNULLにする
                    size: 0,
                };
                //argpポインタを新しく追加したポインタに差し替える
                obj.vm_state().argp = obj.vm_state().stack.push(new_env);
            }
            tag::CALL |
            tag::CALL_TAIL => {
                //TODO よく使う命令にif文を増やしたくない。どうにかして共通部分をくくりだしたい
                if tag == tag::CALL_TAIL {
                    //現在のローカルフレームを破棄する
                    unsafe {
                        let vmstate = obj.vm_state();

                        //Stack内の破棄するローカルフレームのバイト数を計算
                        let discard_bytes = size_of::<Environment>() + (*vmstate.env).size * size_of::<Ref<Any>>();
                        //Callする関数用の新しいローカルフレームのバイト数を計算
                        let new_frame_bytes = size_of::<Environment>() + (*vmstate.argp).size * size_of::<Ref<Any>>();

                        //破棄するローカルフレームを上書きするように、新しいローカルフレームの位置をずらす。
                        std::ptr::copy(vmstate.argp as *mut u8, vmstate.env as * mut u8, new_frame_bytes);
                        //argpポインタをずらした先のアドレスに修正
                        vmstate.argp = vmstate.env;

                        //フレームをずらしたことにより使用しなくなった領域分、スタックポインタを戻す
                        vmstate.stack.  pop_from_size(discard_bytes);

                        //envポインタをロールバック
                        obj.vm_state().env = (*obj.vm_state().env).up;
                    }
                }

                //引数構築の完了処理を行う。
                complete_arg!();

                let env = obj.vm_state().env;
                //ローカルフレームの0番目にappが入っているので取得
                let any = refer_local_var(env, 0, 0);
                //PUSH_APPの時点で型チェックされているので無条件でAPPに変換する
                let app = unsafe { any.cast_unchecked::<app::App>() };
                let parameter = app.as_ref().parameter();

                //関数に渡そうとしている引数の数(先頭に必ずapp自身が入っているので-1する)
                let num_args = unsafe { (*env).size } - 1;
                let mut num_args_remain = num_args;

                if num_args_remain < parameter.num_require() {
                    //必須の引数が足らないエラー
                    return Err(ExecException::Exception(err::Exception::Other(format!("Illegal number of argument.\nThe function {}.\n  require:{}, optional:{}, rest:{}\n  but got {} arguments."
                        , app.as_ref().name()
                        , parameter.num_require(),  parameter.num_optional(), parameter.has_rest(), num_args
                    ))));
                }
                num_args_remain -= parameter.num_require();

                if num_args_remain < parameter.num_optional() {
                    //Optionalに対応する引数がない場合は、足りない分だけUnitをデフォルト値として追加する
                    for _ in 0..(parameter.num_optional() - num_args_remain) {
                        let_local!(tuple::Tuple::unit().into_value().make());
                    }
                }
                num_args_remain = num_args_remain.saturating_sub(parameter.num_optional());

                //rest引数がない関数に対して過剰な引数を渡している場合は
                if num_args_remain != 0 && parameter.has_rest() == false {
                    //エラー
                    return Err(ExecException::Exception(err::Exception::Other(format!("Illegal number of argument.\nThe function {}.\n  require:{}, optional:{}, rest:{}\n  but got {} arguments."
                        , app.as_ref().name()
                        , parameter.num_require(),  parameter.num_optional(), parameter.has_rest(), num_args
                    ))));
                }

                if let Some(func) = any.try_cast::<func::Func>() {

                    //関数内部でPCを参照する場合があるので更新
                    obj.vm_state().pc = program.position();

                    if tag != tag::CALL_TAIL {
                        //実行するプログラムが保存されたバッファを切り替えるため
                        //現在実行中のプログラムをContinuationの中に保存する
                        unsafe {
                            (*obj.vm_state().cont).code = Some(obj.vm_state().code.clone());
                            (*obj.vm_state().cont).pc = program.position();
                        }
                    }

                    //関数本体を実行
                    let result = func.as_ref().apply(num_args_remain, obj);

                    match result {
                        Ok(v) => {
                            //リターン処理を実行
                            tag_return!();

                            obj.vm_state().acc = v;

                            reduce_with_check_timelimit!(10);
                        }
                        Err(err) => {
                            return Err(ExecException::Exception(err));
                        }
                    }
                } else if let Some(closure) = any.try_cast::<compiled::Closure>() {
                    if tag != tag::CALL_TAIL {
                        //実行するプログラムが保存されたバッファを切り替えるため
                        //現在実行中のプログラムをContinuationの中に保存する
                        unsafe {
                            (*obj.vm_state().cont).code = Some(obj.vm_state().code.clone());
                            (*obj.vm_state().cont).pc = program.position();
                        }
                    }

                    let code = closure.as_ref().code();

                    //カーソルをクロージャ本体の実行コードに切り替え
                    program = Cursor::new(code.as_ref().program());
                    obj.vm_state().pc = 0;
                    obj.vm_state().code = code;

                    reduce_with_check_timelimit!(5);
                }
            }
            tag::CALL_RESUME_FUNC => {
                let suspend_info: FuncSuspendInfo = obj.vm_state().stack.pop();
                obj.vm_state().code = suspend_info.code;
                obj.vm_state().pc = suspend_info.pc;
                let result = (suspend_info.resume_func)(obj);
                match result {
                    Ok(v) => {
                        //リターン処理を実行
                        tag_return!();

                        obj.vm_state().acc = v;

                        reduce_with_check_timelimit!(10);
                    }
                    Err(err) => {
                        return Err(ExecException::Exception(err));
                    }
                }
            }
            tag::AND => {
                let offset = read_u16(&mut program);
                if is_true(obj.vm_state().acc.as_ref()) == false {
                    program.seek(SeekFrom::Current(offset as i64)).unwrap();
                }
            }
            tag::OR => {
                let offset = read_u16(&mut program);
                if is_true(obj.vm_state().acc.as_ref()) {
                    program.seek(SeekFrom::Current(offset as i64)).unwrap();
                }
            }
            tag::MATCH_SUCCESS => {
                let offset = read_u16(&mut program);
                if syntax::r#match::MatchFail::is_fail(obj.vm_state().acc.as_ref()) == false {
                    program.seek(SeekFrom::Current(offset as i64)).unwrap();
                }
            }
            _ => unreachable!()
        }
    }

    let result = obj.vm_state().acc.clone();
    //accに入ったままだとGC時に回収されないため、結果の値をaccから外す
    obj.vm_state().acc = bool::Bool::false_().into_ref().into_value();

    Ok(result)
}

#[inline]
fn check_reply(v: &mut Ref<Any>, obj: &mut Object) -> Result<bool, OutOfMemory> {
    //スタック領域内のFPtrをCapとして扱わせる
    let mut cap = Cap::new(v as *mut Ref<Any>);
    //返信がないかを確認
    let result = crate::value::check_reply(&mut cap, obj);
    //そのままDropさせると確保していない内部領域のfreeが走ってしまうのでforgetさせる。
    std::mem::forget(cap);

    result
}

pub fn write_u8<T: Write>(v: u8, buf: &mut T) {
    buf.write_all(&v.to_le_bytes()).unwrap()
}

pub fn write_u16<T: Write>(v: u16, buf: &mut T) {
    buf.write_all(&v.to_le_bytes()).unwrap()
}

#[allow(dead_code)]
pub fn write_u32<T: Write>(v: u32, buf: &mut T) {
    buf.write_all(&v.to_le_bytes()).unwrap()
}

pub fn write_usize<T: Write>(v: usize, buf: &mut T) {
    buf.write_all(&v.to_le_bytes()).unwrap()
}

fn read_u8<T: Read>(buf: &mut T) -> u8 {
    let mut tmp: [u8;1] = [0];
    buf.read_exact(&mut tmp).unwrap();

    tmp[0]
}

fn read_u16<T: Read>(buf: &mut T) -> u16 {
    let mut tmp: [u8;2] = [0, 0];
    buf.read_exact(&mut tmp).unwrap();

    u16::from_le_bytes(tmp)
}

#[allow(dead_code)]
fn read_u32<T: Read>(buf: &mut T) -> u32 {
    let mut tmp: [u8;4] = [0, 0, 0, 0];
    buf.read_exact(&mut tmp).unwrap();

    u32::from_le_bytes(tmp)
}

fn read_usize<T: Read>(buf: &mut T) -> usize {
    #[cfg(target_pointer_width="32")]
    let mut tmp: [u8;4] = [0, 0, 0, 0];
    #[cfg(target_pointer_width="64")]
    let mut tmp: [u8;8] = [0, 0, 0, 0, 0, 0, 0, 0];

    buf.read_exact(&mut tmp).unwrap();

    usize::from_le_bytes(tmp)
}

static CODE_CALL_SUSPEND_FUNC: Lazy<GCAllocationStruct<compiled::Code>> = Lazy::new(|| {
    let mut buf: Vec<u8> = Vec::with_capacity(1);
    write_u8(tag::CALL_RESUME_FUNC, &mut buf);
    let code = compiled::Code::new(buf, Vec::new());
    GCAllocationStruct::new(code)
});

static CODE_CALL: Lazy<GCAllocationStruct<compiled::Code>> = Lazy::new(|| {
    //CALLするだけのコードを作成
    let mut buf: Vec<u8> = Vec::with_capacity(1);
    write_u8(tag::CALL, &mut buf);
    let code = compiled::Code::new(buf, Vec::new());
    GCAllocationStruct::new(code)
});

mod literal {
    use crate::ptr::Reachable;
    use crate::value::compiled;

    use super::*;

    pub fn code_call_suspend_func() -> Reachable<compiled::Code> {
        Reachable::new_static(&CODE_CALL_SUSPEND_FUNC.value)
    }

    pub fn code_call() -> Reachable<compiled::Code> {
        Reachable::new_static(&CODE_CALL.value)
    }

}

#[cfg(test)]
mod tests {
    use crate::eval::exec;
    use crate::object::Object;
    use crate::value::*;
    use crate::value::any::Any;
    use crate::ptr::*;

    #[test]
    fn test_tail_rec() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(let loop (fun (n) (if (= n 0) n (loop (- n 1)))))";
            exec::<Any>(program, obj);

            let program = "(loop 100000)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(0, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }

    #[test]
    fn test_free_var() {
        let mut obj = Object::new_for_test();
        let obj = &mut obj;
        let mut ans_obj = Object::new_for_test();
        let ans_obj = &mut ans_obj;

        {
            let program = "(let add-x (local (let x ((fun () 10))) (fun (n) (+ x n))))";

            exec::<Any>(program, obj);

            let program = "(add-x 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(11, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

        {
            let program = r#"
            (let add
                (local
                    (let x ((fun () 10)))
                    (let y ((fun () 20)))
                    (fun ()
                        (let z ((fun () 30)))
                        (fun (n)
                            (local
                                (let zz ((fun () 40)))
                                (+ x y z zz n))))))"#;
            exec::<Any>(program, obj);

            let program = "(let add (add))";
            exec::<Any>(program, obj);

            let program = "(add 1)";
            let result = exec::<Any>(program, obj).capture(obj);
            let ans = number::make_integer(101, ans_obj).unwrap();
            assert_eq!(result.as_ref(), ans.as_ref());
        }

    }

}