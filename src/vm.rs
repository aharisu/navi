use std::fmt::Debug;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::mem::size_of;
use std::panic;

use crate::object::StandaloneObject;
use crate::object::mm::usize_to_ptr;
use crate::ptr::Reachable;

use crate::object::Object;
use crate::value::*;
use crate::value::any::Any;
use crate::ptr::*;

pub mod tag {
    pub const JUMP_OFFSET: u8 = 0;
    pub const IF: u8 = 1;
    pub const REF_LOCAL: u8 = 2;
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
    pub const RETURN:u8 = 14;
    pub const PUSH_CONT:u8 = 15;
    pub const PUSH_ARG_PREPARE_ENV:u8 = 17;
    pub const CALL:u8 = 18;
    pub const AND:u8 = 19;
    pub const OR:u8 = 20;
    pub const MATCH_SUCCESS:u8 = 21;
    pub const TUPLE:u8 = 22;
    pub const ARRAY:u8 = 23;

    //next number 26
}

#[derive(Debug)]
pub enum ExecError {
    TimeLimit,
    WaitReply,
    ObjectSwitch(StandaloneObject),
    Exception,
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

#[derive(Debug)]
struct VMStack {
    stack: *mut u8,
    pos: *mut u8,
    stack_layout: std::alloc::Layout,
}

impl VMStack {
    pub fn new(size: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(size, size_of::<usize>()).unwrap();
        let stack = unsafe { std::alloc::alloc(layout) };

        VMStack {
            stack: stack,
            pos: stack,
            stack_layout: layout,
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

fn push<T>(v: T, stack: &mut VMStack) -> *mut T {
    let t_ptr = unsafe {
        let t_ptr = stack.pos as *mut T;

        stack.pos = stack.pos.add(size_of::<T>());
        t_ptr.write(v);

        t_ptr
    };

    t_ptr
}

fn pop_from_size(stack: &mut VMStack, decriment: usize) {
    unsafe {
        stack.pos = stack.pos.sub(decriment);
    }
}

pub fn refer_arg<T: NaviType>(index: usize, obj: &mut Object) -> Ref<T> {
    let v = refer_local_var(obj.vm_state().env, index + 1);
    //Funcの引数はすべて型チェックされている前提なのでuncheckedでキャストする
    unsafe { v.cast_unchecked::<T>().clone() }
}

fn refer_local_var(env: *const Environment, index: usize) -> Ref<Any> {
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
    suspend_pc: usize, //途中終了時のプログラムカウンタ
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
            suspend_pc: 0,
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
}

pub fn func_call(func: &Reachable<func::Func>, args_iter: impl Iterator<Item=Ref<Any>>
    , limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    app_call(func.cast_value(), args_iter, limit, obj)
}

pub fn closure_call(closure: &Reachable<compiled::Closure>, args_iter: impl Iterator<Item=Ref<Any>>
    , limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    app_call(closure.cast_value(), args_iter, limit, obj)
}

fn app_call(app: &Reachable<Any>, args_iter: impl Iterator<Item=Ref<Any>>
    , limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    //ContinuationとEnvironmentのフレームをプッシュ
    {
        let mut buf: Vec<u8> = Vec::with_capacity(3);

        //push continuation
        write_u8(tag::PUSH_CONT, &mut buf);

        //push env header
        write_u8(tag::PUSH_ARG_PREPARE_ENV, &mut buf);

        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //関数呼び出しの準備段階の実行は、実行時間に制限を設けない
        code_execute(&code,  WorkTimeLimit::Inf, obj).unwrap();
    }

    //APPをプッシュ
    {
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        write_u8(tag::PUSH_APP, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //accにFuncオブジェクトを設定
        obj.vm_state().acc = app.make();
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
            code_execute(&code, WorkTimeLimit::Inf, obj).unwrap();
        }
    }

    //FuncをCALL命令で実行
    {
        //CALLするだけのコードを作成
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        write_u8(tag::CALL, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //コードを実行
        code_execute(&code, limit, obj)
    }
}

pub fn code_execute(code: &Reachable<compiled::Code>, limit: WorkTimeLimit, obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    //実行対象のコードを設定
    obj.vm_state().code = code.make();
    obj.vm_state().suspend_pc = 0;

    loop {
        //直接executを実行する場合は最大最後まで実行できるようにするためにのワークサイズを設定する
        obj.vm_state().reductions = match limit {
            WorkTimeLimit::Inf => usize::MAX,
            WorkTimeLimit::Reductions(reductions) => reductions,
         };

        //CALLを実行。結果を返り値にする
        match execute(obj) {
            //特定のエラーは補足して処理を継続する
            Err(ExecError::TimeLimit) => {
                if limit == WorkTimeLimit::Inf {
                    //実行時間がInfの場合はloopを継続して終了まで実行させる
                    //loopを継続させるためにここでは何もしない
                } else {
                    //実行時間に制限があるときだけ、エラーを返す
                    return Err(ExecError::TimeLimit);
                }
            }
            Err(ExecError::WaitReply) => {
                if limit == WorkTimeLimit::Inf {
                    //他スレッドの処理が終わるまで時スレッドの処理をブロックして待つ。
                    //3ミリ秒という数字に理由はない。
                    std::thread::sleep(std::time::Duration::from_millis(3));
                } else {
                    //実行時間に制限があるときだけ、エラーを返す
                    return Err(ExecError::WaitReply);
                }
            }
            other => {
                //その他の戻り値はそのまま返す
                return other;
            }
        }
    }
}

pub fn resume(reductions: usize, obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    obj.vm_state().reductions = reductions;
    execute(obj)
}

fn execute(obj: &mut Object) -> Result<Ref<Any>, ExecError> {
    let mut program = Cursor::new(obj.vm_state().code.as_ref().program());
    program.seek(SeekFrom::Start(obj.vm_state().suspend_pc as u64)).unwrap();

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
            push($exp, &mut obj.vm_state().stack);
            //新しく追加した分、環境内のローカルフレームサイズを増やす
            unsafe {
                (*obj.vm_state().env).size += 1;
            }
        };
    }

    macro_rules! push_arg {
        ($exp:expr) => {
            push($exp, &mut obj.vm_state().stack);
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
                obj.vm_state().suspend_pc = program.position() as usize;

                return Err(ExecError::TimeLimit);
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
                let mut frame_offset = read_u16(&mut program);
                let cell_index = read_u16(&mut program);

                //目的の位置まで環境を上に上に順に辿っていく
                let mut target_env = obj.vm_state().env;
                while frame_offset > 0 {
                    target_env = unsafe { (*target_env).up };
                    frame_offset -= 1;
                }

                //目的の環境内にあるローカルフレームから値を取得
                obj.vm_state().acc = refer_local_var(target_env, cell_index as usize);
            }
            tag::REF_GLOBAL => {
                let const_index = read_u16(&mut program);
                let symbol = obj.vm_state().code.as_ref().get_constant(const_index as usize);
                let symbol = unsafe { symbol.cast_unchecked::<symbol::Symbol>() };
                if let Some(v) = obj.find_global_value(symbol.as_ref()) {
                    obj.vm_state().acc = v;

                } else {
                    panic!("global variable not found. {}", symbol.as_ref());
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
                let app = refer_local_var(argp_env, 0);
                if let Some(func) = app.try_cast::<func::Func>() {
                    let index = unsafe { (*argp_env).size - 1 };
                    let parameter = func.as_ref().get_paramter();
                    let param = if parameter.is_empty() {
                            None
                        } else if index < parameter.len() {
                            Some(&parameter[index])
                        } else if parameter[parameter.len() - 1].kind == func::ParamKind::Rest {
                            Some(&parameter[parameter.len() - 1])
                        } else {
                            None
                        };
                    if let Some(param) = param {
                        // reply check
                        if param.force && arg.has_replytype() {
                            //スタック領域内のFPtrをCapとして扱わせる
                            let mut cap = Cap::new(&mut arg as *mut Ref<Any>);
                            //返信がないかを確認
                            let ok = crate::value::check_reply(&mut cap, obj);
                            //そのままDropさせると確保していない内部領域のfreeが走ってしまうのでforgetさせる。
                            std::mem::forget(cap);

                            //まだ返信がない場合は、
                            if  ok == false {
                                //もう一度PUSH_ARGが実行できるように、現在位置-1をresume後のPCとする
                                obj.vm_state().suspend_pc = (program.position() - 1) as usize;

                                //引数の値にReply待ちを含んでいるため、返信を待つ
                                return Err(ExecError::WaitReply);
                            }
                        }

                        // type check
                        if arg.is_type(param.typeinfo) == false {
                            //TODO 型チェックエラー
                            panic!("type error");
                        }
                    }

                    push_arg!(arg);

                } else {
                    push_arg!(obj.vm_state().acc.clone());
                }

            }
            tag::PUSH_ARG_UNCHECK => {
                push_arg!(obj.vm_state().acc.clone());
            }
            tag::PUSH_APP => {
                let app = obj.vm_state().acc.clone();
                if app.is::<func::Func>() || app.is::<compiled::Closure>() {
                    // OK!!  do nothing
                } else {
                    panic!("Not Applicable: {}", app.as_ref())
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
                let target_obj = obj.vm_state().acc.clone();
                if let Some(target_obj) = target_obj.try_cast::<object_ref::ObjectRef>() {
                    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
                    let mailbox = target_obj.as_ref().mailbox();
                    let mut standalone = Object::unregister_scheduler(mailbox);

                    //現在のオブジェクトに対応するObjectRefを作成
                    //※このObjectRefはSwitch先のオブジェクトのヒープに作成される
                    let prev_object = obj.make_object_ref(standalone.mut_object()).unwrap();
                    //return-object-switchでもどれるようにするために移行元のオブジェクトを保存
                    standalone.mut_object().set_prev_object(&prev_object);

                    //object-siwtchはグローバル環境でしか現れない
                    return Err(ExecError::ObjectSwitch(standalone));
                } else {
                    panic!("Not Object {}", target_obj.as_ref())
                }
            }
            tag::RETURN_OBJECT_SWITCH => {
                if let Some(prev_obj) = obj.take_prev_object() {
                    //ObjectRefからObjectを取得(この時点でスケジューラからは切り離されている)
                    let mailbox = prev_obj.as_ref().mailbox();
                    let standalone = Object::unregister_scheduler(mailbox);

                    //object-siwtchはグローバル環境でしか現れない
                    return Err(ExecError::ObjectSwitch(standalone));
                } else {
                    panic!("No return Object")
                }
            }
            tag::POP_ENV => {
                debug_assert!(!obj.vm_state().env.is_null());
                unsafe {
                    let local_frame_size = (*obj.vm_state().env).size;
                    //Envヘッダーとローカル変数のサイズ分、スタックポインタを下げる
                    let size = size_of::<Environment>() + (size_of::<Ref<Any>>() * local_frame_size);
                    pop_from_size(&mut obj.vm_state().stack, size);

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
                obj.vm_state().env = push(new_env, &mut obj.vm_state().stack);
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

                //プログラムの中からClosureの本体を切り出す
                let mut closure_body:Vec<u8> = Vec::new();
                let cur = program.position() as usize;
                let buf = &program.get_ref()[cur .. cur + body_size];
                closure_body.write(buf).unwrap();

                //読み込んだClosure本体のデータ分、プログラムカウンタを進める
                program.seek(SeekFrom::Current(body_size as i64)).unwrap();

                obj.vm_state().acc = compiled::Closure::alloc(closure_body, constants, num_args, obj).into_value();

                reduce!(5);
            }
            tag::RETURN => {
                //Continuationに保存されている状態を復元
                tag_return!();

            }
            tag::PUSH_CONT => {
                //let cont_offset = read_u16(&mut program);

                let new_cont = Continuation {
                    prev: obj.vm_state().cont,
                    code: None, //実行コードはCursorが所有権を持っているので実際にCallされる直前に設定する
                    pc: 0, //復帰するPCはCall直前に設定する
                    env: obj.vm_state().env,
                    argp: obj.vm_state().argp,
                };
                //contポインタを新しく追加したポインタに差し替える
                obj.vm_state().cont = push(new_cont, &mut obj.vm_state().stack);
            }
            tag::PUSH_ARG_PREPARE_ENV => {
                let new_env = Environment {
                    up: std::ptr::null_mut(), //準備段階ではupポインタはNULLにする
                    size: 0,
                };
                //argpポインタを新しく追加したポインタに差し替える
                obj.vm_state().argp = push(new_env, &mut obj.vm_state().stack);
            }
            tag::CALL => {
                //引数構築の完了処理を行う。
                complete_arg!();

                let env = obj.vm_state().env;
                //ローカルフレームの0番目にappが入っているので取得
                let app = refer_local_var(env, 0);

                if let Some(func) = app.try_cast::<func::Func>() {
                    //関数に渡そうとしている引数の数(先頭に必ずfunc自身が入っているので-1する)
                    let num_args = unsafe { (*env).size } - 1;
                    let mut num_args_remain = num_args;

                    if num_args_remain < func.as_ref().num_require() {
                        //TODO 必須の引数が足らないエラー
                        panic!("Illegal number of argument. require:{}, optional:{}, rest:{}, bot got {} arguments.", func.as_ref().num_require(),  func.as_ref().num_optional(), func.as_ref().has_rest(), num_args);
                    }
                    num_args_remain -= func.as_ref().num_require();

                    if num_args_remain < func.as_ref().num_optional() {
                        //Optionalに対応する引数がない場合は、足りない分だけUnitをデフォルト値として追加する
                        for _ in 0..(func.as_ref().num_optional() - num_args_remain) {
                            let_local!(tuple::Tuple::unit().into_value().make());
                        }
                    }
                    num_args_remain = num_args_remain.saturating_sub(func.as_ref().num_optional());

                    if num_args_remain == 0 {
                        if func.as_ref().has_rest() {
                            //restなパラメータに対応する引数がなければnilをデフォルトとして追加
                            let_local!(list::List::nil().into_value().make());
                        }

                    } else {
                        if func.as_ref().has_rest() {
                            //残りの引数をスタックからPopしてリストに構築しなおして、再度スタックに積む
                            //以降すべての引数をリストにまとめる
                            let mut rest = list::ListBuilder::new(obj);

                            for index in (num_args - num_args_remain) .. num_args {
                                //0番目にはapp自体が入っていて引数はインデックス1から始まっているため+1をして引数を取得
                                let arg = refer_local_var(env, index + 1);
                                rest.append(&arg.reach(obj), obj);
                            }

                                //リストに詰め込んだ分、ローカルフレーム内の引数を削除
                            unsafe { (*env).size -= num_args_remain; }
                            //併せてスタックポインタも下げる
                            pop_from_size(&mut obj.vm_state().stack, num_args_remain * size_of::<Ref<Any>>());

                            //削除した代わりに、リストをローカルフレームに追加
                            let_local!(rest.get());

                        } else {
                            //TODO 引数の数が多すぎるエラー
                            panic!("Illegal number of argument. require:{}, optional:{}, rest:{}, bot got {} arguments.", func.as_ref().num_require(),  func.as_ref().num_optional(), func.as_ref().has_rest(), num_args);
                        }
                    }

                    //関数本体を実行
                    obj.vm_state().acc = func.as_ref().apply(obj);

                    //リターン処理を実行
                    tag_return!();
                    reduce_with_check_timelimit!(10);

                } else if let Some(closure) = app.try_cast::<compiled::Closure>() {
                    //引数の数などが正しいかを確認
                    let num_require = closure.as_ref().arg_descriptor();
                    let num_args = unsafe { (*obj.vm_state().env).size } - 1;
                    if num_require != num_args {
                        panic!("Invalid arguments. require:{} actual:{}", num_require, num_args)
                    }

                    //実行するプログラムが保存されたバッファを切り替えるため
                    //現在実行中のプログラムをContinuationの中に保存する
                    unsafe {
                        (*obj.vm_state().cont).code = Some(obj.vm_state().code.clone());
                        (*obj.vm_state().cont).pc = program.position();
                    }

                    let code = closure.as_ref().code();

                    //カーソルをクロージャ本体の実行コードに切り替え
                    program = Cursor::new(code.as_ref().program());
                    obj.vm_state().code = code;

                    reduce_with_check_timelimit!(5);
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
            tag::TUPLE => {
                //引数構築の完了処理を行う。
                complete_arg!();

                let size = unsafe { (*obj.vm_state().env).size };
                let mut builder = tuple::TupleBuilder::new(size, obj);
                for index in 0..size {
                    let arg = refer_local_var(obj.vm_state().env, index);
                    builder.push(&arg, obj);
                }

                obj.vm_state().acc = builder.get().into_value();

                //リターン処理を実行
                tag_return!();

                reduce!(5);
            }
            tag::ARRAY => {
                //引数構築の完了処理を行う。
                complete_arg!();

                let size = unsafe { (*obj.vm_state().env).size };
                let mut builder = array::ArrayBuilder::<Any>::new(size, obj);
                for index in 0..size {
                    let arg = refer_local_var(obj.vm_state().env, index);
                    builder.push(&arg, obj);
                }

                obj.vm_state().acc = builder.get().into_value();

                //リターン処理を実行
                tag_return!();

                reduce!(5);
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

    Ok(obj.vm_state().acc.clone())
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