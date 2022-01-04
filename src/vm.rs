use std::fmt::Debug;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::mem::size_of;
use std::panic;

use crate::object::mm::usize_to_ptr;
use crate::ptr::Reachable;

use crate::object::Object;
use crate::value::*;
use crate::ptr::*;

pub mod tag {
    pub const JUMP_OFFSET: u8 = 0;
    pub const IF: u8 = 1;
    pub const REF_LOCAL: u8 = 2;
    pub const REF_GLOBAL: u8 = 3;
    pub const CONST_CAPTURE: u8 = 4;
    pub const CONST_STATIC: u8 = 5;
    pub const CONST_IMMIDIATE: u8 = 6;
    pub const PUSH: u8 = 7;
    pub const DEF_LOCAL: u8 = 8;
    pub const DEF_GLOBAL: u8 = 9;
    pub const DEF_RECV:u8 = 10;
    pub const POP_ENV:u8 = 11;
    pub const PUSH_EMPTY_ENV:u8 = 12;
    pub const CLOSURE:u8 = 13;
    pub const RETURN:u8 = 14;
    pub const PUSH_CONT:u8 = 15;
    pub const PUSH_CONT_FOR_FUNC_CALL:u8 = 16;
    pub const PUSH_ARG_PREPARE_ENV:u8 = 17;
    pub const CALL:u8 = 18;
    pub const AND:u8 = 19;
    pub const OR:u8 = 20;
    pub const MATCH_SUCCESS:u8 = 21;
    pub const TUPLE:u8 = 22;
    pub const ARRAY:u8 = 23;

    //next number 23
}

#[derive(Debug)]
#[repr(C)]
struct Continuation {
    prev: *mut Continuation,
    pc: i64,
    env: *mut Environment,
    argp: *mut Environment,
    code: Option<FPtr<compiled::Code>>,
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

pub fn is_true(v: &Value) -> bool {
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

pub fn refer_arg<T: NaviType>(index: usize, obj: &mut Object) -> FPtr<T> {
    let v = refer_local_var(obj.vm_state().env, index);
    //Funcの引数はすべて型チェックされている前提なのでuncheckedでキャストする
    unsafe { v.cast_unchecked::<T>().clone() }
}

fn refer_local_var(env: *const Environment, index: usize) -> FPtr<Value> {
    //目的の環境内にあるローカルフレームから値を取得
    unsafe {
        //ローカルフレームは環境ヘッダの後ろ側にある
        let frame_ptr = env.add(1) as *mut FPtr<Value>;
        let cell = frame_ptr.add(index as usize);
        (*cell).clone()
    }
}

#[derive(Debug)]
pub struct VMState {
    code: FPtr<compiled::Code>,
    acc: FPtr<Value>,
    stack: VMStack,
    cont: *mut Continuation,
    env: *mut Environment,
    argp: *mut Environment,
}

impl VMState {
    pub fn new() -> Self {
        VMState {
            code: FPtr::from_ptr(std::ptr::null_mut()), //ダミーのためにヌルポインターで初期化
            acc: bool::Bool::false_().into_value().into_fptr(),
            stack: VMStack::new(1024 * 3),
            cont: std::ptr::null_mut(),
            env: std::ptr::null_mut(),
            argp: std::ptr::null_mut(),
        }
    }

    pub fn for_each_all_alived_value(&self, arg: *mut u8, callback: fn(&FPtr<Value>, *mut u8)) {
        if self.code.as_ptr().is_null() == false {
            callback(self.code.cast_value(), arg);
        }

        if self.acc.as_ptr().is_null() == false {
            callback(&self.acc, arg);
        }

        {
            let mut cont = self.cont;
            while cont.is_null() == false {
                unsafe {
                    if let Some(code) = (*cont).code.as_ref() {
                        callback(code.cast_value(), arg);
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
                    let frame_ptr = env.add(1) as *mut FPtr<Value>;
                    for index in 0 .. len {
                        let cell = frame_ptr.add(index as usize);
                        let v = &*cell;
                        callback(v, arg);
                    }

                    env = (*env).up;
                }
            }
        }
    }
}

pub fn func_call(func: &Reachable<func::Func>, args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    //ContinuationとEnvironmentのフレームをプッシュ
    {
        let mut buf: Vec<u8> = Vec::with_capacity(3);

        //push continuation
        write_u8(tag::PUSH_CONT_FOR_FUNC_CALL, &mut buf);

        //push env header
        write_u8(tag::PUSH_ARG_PREPARE_ENV, &mut buf);
        //pushされる引数の数
        let num_args = args.as_ref().count();
        debug_assert!(num_args < u8::MAX as usize);
        write_u8(num_args as u8, &mut buf);

        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        execute(&code, obj);
    }

    //全ての引数をプッシュ
    {
        //PUSHするだけのコードを作成
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        write_u8(tag::PUSH, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        for arg in args.iter(obj) {
            //引数を順にaccに入れてPUSHを実行
            obj.vm_state().acc = arg.clone();
            execute(&code, obj);
        }
    }

    //FuncをCALL命令で実行
    {
        //CALLするだけのコードを作成
        let mut buf: Vec<u8> = Vec::with_capacity(1);
        write_u8(tag::CALL, &mut buf);
        let code = compiled::Code::new(buf, Vec::new());
        let code = Reachable::new_static(&code);

        //accにFuncオブジェクトを設定
        obj.vm_state().acc = FPtr::new(func.cast_value().as_ref());

        //CALLを実行。結果を返り値にする
        execute(&code, obj)
    }
}

pub fn execute(code: &Reachable<compiled::Code>, obj: &mut Object) -> FPtr<Value> {
    obj.vm_state().code = FPtr::new(code.as_ref());
    let mut program = Cursor::new(code.as_ref().program());

    //これ以降、code変数は使用しない。
    //間違えて参照してしまわないように、適当な型の値でShadowingしておく。
    #[allow(unused_variables)]
    let code :std::marker::PhantomData<bool> = std::marker::PhantomData;

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
                }
                if (*state.cont).pc > 0 {
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
            }
            tag::CONST_CAPTURE => {
                let const_index = read_u16(&mut program);
                obj.vm_state().acc = obj.vm_state().code.as_ref().get_constant(const_index as usize);
            }
            tag::CONST_STATIC
            | tag::CONST_IMMIDIATE => {
                let data = read_usize(&mut program);
                let ptr = usize_to_ptr::<Value>(data);
                obj.vm_state().acc = FPtr::from_ptr(ptr);
            }
            tag::PUSH => {
                push(obj.vm_state().acc.clone(), &mut obj.vm_state().stack);
            }
            tag::DEF_LOCAL => {
                let_local!(obj.vm_state().acc.clone());
            }
            tag::DEF_GLOBAL => {
                let const_index = read_u16(&mut program);
                let symbol = obj.vm_state().code.as_ref().get_constant(const_index as usize);
                let symbol = unsafe { symbol.cast_unchecked::<symbol::Symbol>() };

                let acc = obj.vm_state().acc.as_ref();
                obj.define_global_value(symbol.as_ref(), acc);
            }
            tag::DEF_RECV => {
                //TODO
                let _pattern_index = read_u16(&mut program);
                let _body_index = read_u16(&mut program);
            }
            tag::POP_ENV => {
                debug_assert!(!obj.vm_state().env.is_null());
                unsafe {
                    let local_frame_size = (*obj.vm_state().env).size;
                    //Envヘッダーとローカル変数のサイズ分、スタックポインタを下げる
                    let size = size_of::<Environment>() + (size_of::<FPtr<Value>>() * local_frame_size);
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
            }
            tag::RETURN => {
                //Continuationに保存されている状態を復元
                tag_return!();

            }
            tag::PUSH_CONT => {
                let cont_offset = read_u16(&mut program);

                let new_cont = Continuation {
                    prev: obj.vm_state().cont,
                    code: None, //実行コードはCursorが所有権を持っているので実際にCallされる直前に設定する
                    pc: (program.position() + cont_offset as u64) as i64,
                    env: obj.vm_state().env,
                    argp: obj.vm_state().argp,
                };
                //contポインタを新しく追加したポインタに差し替える
                obj.vm_state().cont = push(new_cont, &mut obj.vm_state().stack);
            }
            tag::PUSH_CONT_FOR_FUNC_CALL => {
                let new_cont = Continuation {
                    prev: obj.vm_state().cont,
                    code: None, //実行コードはCursorが所有権を持っているので実際にCallされる直前に設定する
                    pc: -1, //Func呼び出しでは関数呼び出し後に続けてプログラムを読み込めばいいので、PCの保存は行わない
                    env: obj.vm_state().env,
                    argp: obj.vm_state().argp,
                };
                //contポインタを新しく追加したポインタに差し替える
                obj.vm_state().cont = push(new_cont, &mut obj.vm_state().stack);
            }
            tag::PUSH_ARG_PREPARE_ENV => {
                let size = read_u8(&mut program);
                let new_env = Environment {
                    up: std::ptr::null_mut(), //準備段階ではupポインタはNULLにする
                    size: size as usize,
                };
                //argpポインタを新しく追加したポインタに差し替える
                obj.vm_state().argp = push(new_env, &mut obj.vm_state().stack);
            }
            tag::CALL => {
                //引数構築の完了処理を行う。
                complete_arg!();

                let acc = obj.vm_state().acc.clone();
                if let Some(func) = acc.try_cast::<func::Func>() {
                    fn check_type(v: &FPtr<Value>, param: &func::Param) -> bool {
                        v.is_type(param.typeinfo)
                    }

                    let env = obj.vm_state().env;
                    //関数に渡そうとしている引数の数
                    let num_args = unsafe { (*env).size };
                    //関数に定義されているパラメータ指定配列
                    let paramter = func.as_ref().get_paramter();

                    for (index, param) in paramter.iter().enumerate() {
                        match param.kind {
                            func::ParamKind::Require => {
                                if index < num_args {
                                    let arg = refer_local_var(env, index);
                                    if check_type(&arg, param) == false {
                                        //TODO 型チェックエラー
                                        panic!("type error");
                                    }
                                } else {
                                    //TODO 必須の引数が足らないエラー
                                    panic!("Invalid argument. require {}, bot got {}", paramter.len(), num_args);
                                }
                            }
                            func::ParamKind::Optional => {
                                if index < num_args {
                                    let arg = refer_local_var(env, index);
                                    if check_type(&arg, param) == false {
                                        //型チェックエラー
                                        panic!("type error");
                                    }
                                } else {
                                    //Optionalなパラメータに対応する引数がなければUnitをデフォルトとして追加
                                    let_local!(FPtr::new(tuple::Tuple::unit().cast_value().as_ref()));
                                }
                            }
                            func::ParamKind::Rest => {
                                if index < num_args {
                                    let rest_count = num_args - index;

                                    //以降すべての引数をリストにまとめる
                                    let mut rest = list::ListBuilder::new(obj);
                                    for index in index .. num_args {
                                        let arg = refer_local_var(env, index);
                                        if check_type(&arg, param) == false {
                                            //TODO 型チェックエラー
                                        panic!("type error");
                                        }
                                        rest.append(&arg.reach(obj), obj);
                                    }

                                    //リストに詰め込んだ分、ローカルフレーム内の引数を削除
                                    unsafe { (*env).size -= rest_count; }
                                    //併せてスタックポインタも下げる
                                    pop_from_size(&mut obj.vm_state().stack, rest_count * size_of::<FPtr<Value>>());

                                    //削除した代わりに、リストをローカルフレームに追加
                                    let_local!(rest.get());

                                } else {
                                    //Optionalなパラメータに対応する引数がなければnilをデフォルトとして追加
                                    let_local!(FPtr::new(list::List::nil().cast_value().as_ref()));
                                }
                            }
                        }
                    }

                    //関数本体を実行
                    obj.vm_state().acc = func.as_ref().apply(obj);

                    //リターン処理を実行
                    tag_return!();

                } else if let Some(closure) = acc.try_cast::<closure::Closure>() {
                    let size = unsafe { (*obj.vm_state().env).size };
                    let mut builder = array::ArrayBuilder::<Value>::new(size, obj);
                    for index in 0..size {
                        builder.push(refer_local_var(obj.vm_state().env, index).as_ref(), obj);
                    }
                    let args = builder.get().reach(obj);

                    if closure.as_ref().process_arguments_descriptor(args.iter(), obj) {
                        obj.vm_state().acc = closure.as_ref().apply(args.iter(), obj);

                        //リターン処理を実行
                        tag_return!();

                    } else {
                        panic!("Invalid arguments: {:?} {:?}", closure.as_ref(), args.as_ref())
                    }

                } else if let Some(closure) = acc.try_cast::<compiled::Closure>() {
                    //引数の数などが正しいかを確認
                    let num_require = closure.as_ref().arg_descriptor();
                    let num_args = unsafe { (*obj.vm_state().env).size };
                    if num_require != num_args {
                        panic!("Invalid arguments. require:{} actual:{}", num_require, num_args)
                    }

                    //実行するプログラムが保存されたバッファを切り替えるため
                    //現在実行中のプログラムをContinuationの中に保存する
                    unsafe {
                        (*obj.vm_state().cont).code = Some(obj.vm_state().code.clone());
                    }

                    let code = closure.as_ref().code();

                    //カーソルをクロージャ本体の実行コードに切り替え
                    program = Cursor::new(code.as_ref().program());
                    obj.vm_state().code = code;
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
                    let arg = refer_local_var(obj.vm_state().env, index).as_ref();
                    builder.push(arg, obj);
                }

                obj.vm_state().acc = builder.get().into_value();

                //リターン処理を実行
                tag_return!();
            }
            tag::ARRAY => {
                //引数構築の完了処理を行う。
                complete_arg!();

                let size = unsafe { (*obj.vm_state().env).size };
                let mut builder = array::ArrayBuilder::<Value>::new(size, obj);
                for index in 0..size {
                    let arg = refer_local_var(obj.vm_state().env, index).as_ref();
                    builder.push(arg, obj);
                }

                obj.vm_state().acc = builder.get().into_value();

                //リターン処理を実行
                tag_return!();
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

    obj.vm_state().acc.clone()
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