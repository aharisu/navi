use core::panic;

use once_cell::sync::Lazy;

use crate::object::mm::GCAllocationStruct;
use crate::ptr::*;
use crate::object::Object;
use crate::value::*;
use crate::value::any::Any;
use crate::value::array::ArrayBuilder;
use crate::value::symbol::Symbol;
use crate::value::list::List;
use crate::value::syntax::Syntax;
use crate::value::iform::*;

struct LocalVar {
    pub name: Cap<Symbol>,
    pub init_form: Option<Cap<iform::IForm>>,
}

///
/// Compile Context
pub struct CCtx<'a> {
    frames: &'a mut Vec<Vec<LocalVar>>,
    toplevel: bool,
}

pub fn compile(sexp: &Reachable<Any>, obj: &mut Object) -> Ref<compiled::Code> {
    let mut frames = Vec::new();
    let mut ctx = CCtx {
        frames: &mut frames,
        toplevel: true,
    };

    let iform = pass_transform(sexp, &mut ctx, obj).reach(obj);
    //dbg!((sexp.as_ref(), iform.as_ref()));

    codegen::code_generate(&iform, obj)
}

//
// pass 1
// Covnerts S expression into intermediates form (IForm).
fn pass_transform(sexp: &Reachable<Any>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    if let Some(list) = sexp.try_cast::<List>() {
        if list.as_ref().is_nil() {
            IFormConst::alloc(sexp, obj).into_iform()

        } else {
            transform_apply(list, ctx, obj)
        }

    } else if let Some(symbol) = sexp.try_cast::<Symbol>() {
        transform_symbol(symbol, ctx, obj)

    } else if let Some(tuple) = sexp.try_cast::<tuple::Tuple>() {
        transform_tuple(tuple, ctx, obj)

    } else if let Some(array) = sexp.try_cast::<array::Array<Any>>() {
        transform_array(array, ctx, obj)

    } else {
        IFormConst::alloc(sexp, obj).into_iform()
    }

}

enum LookupResult<'a> {
    Notfound,
    Const(&'a IFormConst),
    Var(&'a mut LocalVar),
}

fn lookup_localvar<'a>(symbol: &Symbol, ctx: &'a mut CCtx) -> LookupResult<'a> {
    //この関数内ではGCが発生しないため値を直接参照する
    let mut symbol = symbol;
    let mut last_found_lvar:Option<&mut LocalVar> = None;

    for frame in ctx.frames.iter_mut().rev() {
        for lvar in frame.iter_mut().rev() {
            if lvar.name.as_ref() == symbol {
                //let構文などで直接初期化式が指定してある場合は、さらに細かく調査する
                if let Some(init_form) = &lvar.init_form {

                    if let Some(lref) = init_form.try_cast::<IFormLRef>() {
                        //ローカル変数が別の変数を束縛していたら、ループを続けて対象の変数を探す
                        symbol = lref.as_ref().symbol().as_ref();
                        last_found_lvar = Some(lvar);

                    } else if let Some(constant) = init_form.try_cast::<IFormConst>() {
                        //ローカル変数が定数を束縛していたら直接定数を返す

                        //TODO ここas_ref()で値の参照を直接返しているが問題はないのか？FPtrやCapの参照で取り扱わなくて大丈夫？
                        return LookupResult::Const(constant.as_ref());
                    } else {
                        //それ以外の場合は調査不能なのでlvarを返す

                        return LookupResult::Var(lvar);
                    }
                } else {
                    return LookupResult::Var(lvar);
                }
            }
        }
    }

    if let Some(lvar) = last_found_lvar {
        LookupResult::Var(lvar)

    } else {
        LookupResult::Notfound
    }
}


fn get_binding_variable(symbol: &Symbol, ctx: &mut CCtx, obj: &mut Object) -> Option<Ref<Any>> {
    match lookup_localvar(symbol, ctx) {
        LookupResult::Var(lvar) => {
            //ローカル変数が、グローバル変数を束縛していたら
            if let Some(init_form) =  &lvar.init_form {
                if let Some(gref) = init_form.try_cast::<IFormGRef>() {
                    //find global & return
                    return obj.find_global_value(gref.as_ref().symbol().as_ref());
                }
            }

            //funのパラメータなどの追跡不能なローカル変数の場合はNotfoundを返す
            None
        }
        LookupResult::Const(constant) => {
            Some(constant.value())
        }
        LookupResult::Notfound => {
            //find global
            obj.find_global_value(symbol)
        }
    }
}

fn transform_symbol(symbol: &Reachable<Symbol>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    match lookup_localvar(symbol.as_ref(), ctx) {
        LookupResult::Var(_lvar) => {
            IFormLRef::alloc(symbol, obj).into_iform()
        }
        LookupResult::Const(constant) => {
            Ref::new(constant).into_iform()
        }
        LookupResult::Notfound => {
            IFormGRef::alloc(symbol, obj).into_iform()
        }
    }
}

fn transform_apply(list: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let app = list.as_ref().head();

    //Syntaxを適用しているなら、Syntaxを適用して変換する
    if let Some(symbol) = app.try_cast::<Symbol>() {
        //get binding variable
        if let Some(val) = get_binding_variable(symbol.as_ref(), ctx, obj) {
            if let Some(syntax) = val.try_cast::<Syntax>() {
                return transform_syntax(&syntax.clone().reach(obj), &list.as_ref().tail().reach(obj), ctx, obj);
            }
        }

    } else if let Some(syntax) = app.try_cast::<Syntax>() {
        return transform_syntax(&syntax.clone().reach(obj), &list.as_ref().tail().reach(obj), ctx, obj);
    }

    //Syntax以外の場合は関数呼び出しとして変換する
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    //適用される値を変換
    let app =  pass_transform(&app.reach(obj), &mut ctx, obj).reach(obj);

    //引数部分の値を変換
    let count = list.as_ref().tail().as_ref().count();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj);

    list.as_ref().tail().reach(obj).iter(obj)
        .for_each(|v| {
            let iform = pass_transform(&v.reach(obj), &mut ctx, obj);
            builder_args.push(&iform, obj);
        });
    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    IFormCall::alloc(&app, &args, obj).into_iform()
}

fn transform_tuple(tuple: &Reachable<tuple::Tuple>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    let count = tuple.as_ref().len();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj);

    for index in 0..count {
        let iform = pass_transform(&tuple.as_ref().get(index).reach(obj), &mut ctx, obj);
        builder_args.push(&iform, obj);
    }

    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    IFormContainer::alloc(&args, iform::ContainerKind::Tuple, obj).into_iform()
}

fn transform_array(array: &Reachable<array::Array<Any>>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    let count = array.as_ref().len();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj);

    for index in 0..count {
        let iform = pass_transform(&array.as_ref().get(index).reach(obj), &mut ctx, obj);
        builder_args.push(&iform, obj);
    }

    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    IFormContainer::alloc(&args, iform::ContainerKind::Array, obj).into_iform()
}

fn transform_syntax(syntax: &Reachable<Syntax>, args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let syntax = syntax.as_ref();

    //TODO 引数の数や型のチェック

    //Syntaxを実行してIFormに変換する
    syntax.transform(args, ctx, obj)
}

pub(crate) fn syntax_if(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    let pred = pass_transform(&args.as_ref().head().reach(obj), &mut ctx, obj).reach(obj);

    let args = args.as_ref().tail();
    let true_ = args.as_ref().head().reach(obj);

    let args = args.as_ref().tail();
    let false_ = if args.as_ref().is_nil() {
        IFormConst::alloc(&bool::Bool::false_().into_value(), obj).into_iform()
    } else {
        let false_ = args.as_ref().head().reach(obj);
        pass_transform(&false_, &mut ctx, obj)
    };

    let false_ = false_.reach(obj);
    let true_ = pass_transform(&true_, &mut ctx, obj).reach(obj);

    IFormIf::alloc(&pred, &true_, &false_, obj).into_iform()
}

pub(crate) fn syntax_cond(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    fn cond_inner(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
        let is_last = args.as_ref().tail().as_ref().is_nil();

        let clause = args.as_ref().head();
        if let Some(clause) = clause.try_cast::<List>() {
            let test = clause.as_ref().head().reach(obj);

            //最後の節のTEST式がシンボルのelseなら、無条件でbody部分を実行するように変換する
            if is_last {
                if let Some(else_) = test.try_cast::<Symbol>() {
                    if else_.as_ref().as_ref() == "else" {
                        return transform_begin(&clause.as_ref().tail().reach(obj), ctx, obj);
                    }
                }
            }

            let then_clause = clause.as_ref().tail().reach(obj);
            //TEST部分を変換
            let test_iform = pass_transform(&test, ctx, obj).reach(obj);
            //TESTの結果がtrueだったときに実行する式を変換
            let exprs_iform = transform_begin(&then_clause, ctx, obj).reach(obj);
            //TESTの結果がfalseだったときの次の節を変換
            let next_iform = if is_last {
                //最後の節ならfalseを返すようにする
                IFormConst::alloc(&bool::Bool::false_().into_value(), obj).into_iform()
            } else {
                //続きの節があるなら再帰的に変換する
                cond_inner(&args.as_ref().tail().reach(obj), ctx, obj)
            }.reach(obj);

            IFormIf::alloc(&test_iform, &exprs_iform, &next_iform, obj).into_iform()

        } else {
            panic!("cond clause require list. but got {:?}", clause.as_ref());
        }
    }

    //(cond)のようにテスト部分が空のcondであれば
    if args.as_ref().is_nil() {
        //無条件でfalseを返す
        IFormConst::alloc(&bool::Bool::false_().into_value(), obj).into_iform()
    } else {
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: false,
        };

        cond_inner(args, &mut ctx, obj)
    }
}

fn transform_begin(body: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //Beginは現在のコンテキスト(トップレベルや末尾文脈)をそのまま引き継いで各式を評価します

    let size = body.as_ref().count();
    let mut builder = array::ArrayBuilder::new(size, obj);

    for sexp in body.iter(obj) {
        let iform = pass_transform(&sexp.reach(obj), ctx, obj);
        builder.push(&iform, obj);
    }

    IFormSeq::alloc(&builder.get().reach(obj), obj).into_iform()
}

pub(crate) fn syntax_begin(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    transform_begin(args, ctx, obj)
}

pub fn syntax_def_recv(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //def-recvはトップレベルのコンテキストで使用可能(letやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {
        let pat = args.as_ref().head().reach(obj);
        let body = args.as_ref().tail().reach(obj);

        IFormDefRecv::alloc(&pat, &body, obj).into_iform()
    } else {
        panic!("def-recv allow only top-level context")
    }
}

pub fn syntax_fun(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let params = args.as_ref().head().reach(obj);
    if let Some(params) = params.try_cast::<List>() {
        let mut builder_params = ArrayBuilder::<Symbol>::new(params.as_ref().count(), obj);
        let mut local_frame: Vec<LocalVar> = Vec::new();

        //TODO keywordやrest引数の処理
        for param in params.iter(obj) {
            if let Some(symbol) = param.try_cast::<Symbol>() {
                builder_params.push(symbol , obj);
                local_frame.push(LocalVar {
                    name: symbol.clone().capture(obj),
                    init_form: None,
                });

            } else {
                panic!("parameter require symbol. But got {:?}", param.as_ref())
            }
        }

        let params = builder_params.get().reach(obj);

        //ローカルフレームを追加
        ctx.frames.push(local_frame);
        //funのbodyは新しいトップレベルになる
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: true,
        };

        //ローカルフレーム内でBody部分を変換
        let body = args.as_ref().tail().reach(obj);
        let body = transform_begin(&body, &mut ctx, obj).reach(obj);

        //ローカルフレーム削除
        ctx.frames.pop();

        IFormFun::alloc(&params, &body, obj).into_iform()
    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }

}

pub fn syntax_local(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //ローカルフレームを作成する
    let frame: Vec<LocalVar> = Vec::new();

    //コンパイルコンテキストにローカルフレームをプッシュ
    ctx.frames.push(frame);
    //localは新しいトップレベルになる
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: true,
    };

    //ローカルフレームが積まれた状態でBody部分を変換
    let body = transform_begin(&args, &mut ctx, obj).reach(obj);

    ctx.frames.pop();

    IFormLocal::alloc(&body, obj).into_iform()
}

pub fn syntax_let(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    if ctx.toplevel == false {
        panic!("The let syntax is allowed in top level context.");
    }

    let symbol = args.as_ref().head().reach(obj);
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: false,
        };

        let value = args.as_ref().tail().as_ref().head().reach(obj);
        let iform = pass_transform(&value, &mut ctx, obj).reach(obj);

        //現在のローカルフレームに新しく定義した変数を追加
        if let Some(cur_frame) = ctx.frames.last_mut() {
            cur_frame.push(LocalVar {
                    name: symbol.make().capture(obj),
                    init_form: Some(iform.make().capture(obj)),
                });
        }

        IFormLet::alloc(&symbol, &iform, obj).into_iform()
    } else {
        panic!("let variable require symbol. But got {}", symbol.as_ref());
    }
}

pub fn syntax_quote(args: &Reachable<List>, _ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let val = args.as_ref().head().reach(obj);
    IFormConst::alloc(&val, obj).into_iform()
}


#[allow(unused_variables)]
pub fn syntax_unquote(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    unimplemented!()
}

#[allow(unused_variables)]
pub fn syntax_bind(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    unimplemented!()
}

pub fn syntax_match(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //パターン部が一つもなければUnitを返す
    if args.as_ref().is_nil() {
        IFormConst::alloc(&tuple::Tuple::unit().into_value(), obj).into_iform()
    } else {
        let match_expr = crate::value::syntax::r#match::translate(args, obj).into_value().reach(obj);
        pass_transform(&match_expr, ctx, obj)
    }
}

pub fn syntax_fail_catch(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    //fail-catchはmatch式の中でだけ使用される特殊な構文
    //引数の式を評価し、値がFAILでなければその値を返す。
    //引数全てFAILならFAILを返す。
    //特殊なor構文のような動作。

    let size = args.as_ref().count();
    debug_assert!(size != 0);

    let mut builder = array::ArrayBuilder::new(size, obj);

    for sexp in args.iter(obj) {
        let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj);
        builder.push(&iform, obj);
    }

    IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::MatchSuccess, obj).into_iform()
}

pub fn syntax_and(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    let size = args.as_ref().count();
    //(and)のように引数が一つもなければ
    if size == 0 {
        IFormConst::alloc(&bool::Bool::true_().into_value(), obj).into_iform()

    } else {
        let mut builder = array::ArrayBuilder::new(size, obj);

        for sexp in args.iter(obj) {
            let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj);
            builder.push(&iform, obj);
        }

        IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::And, obj).into_iform()
    }
}

pub fn syntax_or(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
    };

    let size = args.as_ref().count();
    //(or)のように引数が一つもなければ
    if size == 0 {
        IFormConst::alloc(&bool::Bool::false_().into_value(), obj).into_iform()

    } else {
        let mut builder = array::ArrayBuilder::new(size, obj);

        for sexp in args.iter(obj) {
            let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj);
            builder.push(&iform, obj);
        }

        IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::Or, obj).into_iform()
    }
}

pub fn syntax_object_switch(args: &Reachable<list::List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //TODO グローバル環境のbegin内にある場合、続きの式があるので動作がおかしくなる。
    //TODO 末尾文脈でのみ許可するようにしたい

    //object-switchはトップレベルのコンテキストで使用可能(localやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {

        let target_obj = args.as_ref().head().reach(obj);
        let iform = pass_transform(&target_obj, ctx, obj);
        let iform = iform.reach(obj);

        IFormObjectSwitch::alloc(Some(&iform), obj).into_iform()
    } else {
        panic!("object-switch allow only top-level context")
    }
}

pub fn syntax_return_object_switch(_args: &Reachable<list::List>, ctx: &mut CCtx, obj: &mut Object) -> Ref<IForm> {
    //TODO グローバル環境のbegin内にある場合、続きの式があるので動作がおかしくなる。
    //TODO 末尾文脈でのみ許可するようにしたい

    //object-switchはトップレベルのコンテキストで使用可能(localやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {
        IFormObjectSwitch::alloc(None, obj).into_iform()
    } else {
        panic!("return-object-switch allow only top-level context")
    }
}


mod codegen {
    use core::panic;

    use crate::object::Allocator;
    use crate::object::mm::ptr_to_usize;
    use crate::ptr::*;
    use crate::vm;
    use crate::object::Object;
    use crate::value::{*, self};
    use crate::value::any::Any;
    use crate::value::iform::*;
    use crate::value::symbol::Symbol;
    use crate::vm::{write_u16, write_u8, write_usize};

    //
    // Code Generation Arg
    struct CGCtx<'a> {
        pub buf: Vec<u8>,
        pub constants: &'a mut Vec<Cap<Any>>,
        pub frames: &'a mut Vec<Vec<Cap<Symbol>>>,
    }

    impl <'a> CGCtx<'a> {
        pub fn add_constant(&mut self, v: Ref<Any>, obj: &mut Object) -> usize {
            //同じ値が既に存在しているなら
            if let Some((index, _)) = self.constants.iter().enumerate()
                    .find(|(_index, constant)|  constant.as_ref() == v.as_ref()) {
                //新しく追加はせずに既存の値のインデックスを返す
                index

            } else {
                let result = self.constants.len();
                self.constants.push(v.capture(obj));

                result
            }
        }
    }

    //
    //
    // generate VM Code
    //
    //

    pub fn code_generate(iform: &Reachable<IForm>, obj: &mut Object) -> Ref<compiled::Code> {
        let mut constants:Vec<Cap<Any>> = Vec::new();
        let mut frames:Vec<Vec<Cap<Symbol>>> = Vec::new();

        let mut ctx = CGCtx {
            buf: Vec::new(),
            constants: &mut constants,
            frames: &mut frames,
        };

        pass_codegen(iform, &mut ctx, obj);

        compiled::Code::alloc(ctx.buf, constants, obj)
    }

    fn pass_codegen(iform: &Reachable<IForm>, ctx: &mut CGCtx, obj: &mut Object) {
        match iform.as_ref().kind() {
            IFormKind::Let => {
                codegen_let(unsafe { iform.cast_unchecked::<IFormLet>() }, ctx, obj)
            },
            IFormKind::If => {
                codegen_if(unsafe { iform.cast_unchecked::<IFormIf>() }, ctx, obj)
            },
            IFormKind::Local => {
                codegen_local(unsafe { iform.cast_unchecked::<IFormLocal>() }, ctx, obj)
            },
            IFormKind::LRef => {
                codegen_lref(unsafe { iform.cast_unchecked::<IFormLRef>() }, ctx, obj)
            },
            IFormKind::GRef => {
                codegen_gref(unsafe { iform.cast_unchecked::<IFormGRef>() }, ctx, obj)
            },
            IFormKind::Fun => {
                codegen_fun(unsafe { iform.cast_unchecked::<IFormFun>() }, ctx, obj)
            },
            IFormKind::Seq => {
                codegen_seq(unsafe { iform.cast_unchecked::<IFormSeq>() }, ctx, obj)
            },
            IFormKind::Call => {
                codegen_call(unsafe { iform.cast_unchecked::<IFormCall>() }, ctx, obj)
            },
            IFormKind::Const => {
                codegen_const(unsafe { iform.cast_unchecked::<IFormConst>() }, ctx, obj)
            },
            IFormKind::AndOr => {
                codegen_andor(unsafe { iform.cast_unchecked::<IFormAndOr>() }, ctx, obj)
            },
            IFormKind::Container => {
                codegen_container(unsafe { iform.cast_unchecked::<IFormContainer>() }, ctx, obj)
            },
            IFormKind::DefRecv => {
                codegen_defrecv(unsafe { iform.cast_unchecked::<IFormDefRecv>() }, ctx, obj)
            },
            IFormKind::ObjectSwitch => {
                codegen_object_switch(unsafe { iform.cast_unchecked::<IFormObjectSwitch>() }, ctx, obj)
            },
        }
    }

    fn codegen_let(iform: &Reachable<IFormLet>, ctx: &mut CGCtx, obj: &mut Object) {
        pass_codegen(&iform.as_ref().val().reach(obj), ctx, obj);

        //グローバル環境へのdefか？
        if ctx.frames.is_empty() {
            //タグ
            write_u8(vm::tag::LET_GLOBAL, &mut ctx.buf);

            //キャプチャを取得して、キャプチャが保持する移動しないオブジェクトへの参照のポインタを書き込む。
            let symbol = iform.as_ref().symbol();
            let symbol = symbol.cast_value().clone();
            let index = ctx.add_constant(symbol, obj);

            write_u16(index as u16, &mut ctx.buf);


        } else {
            //TODO !!!!!! ifやand/or などで実行されなかった式の中にdefがあっても
            //有効なローカル変数としてコンパイルされるが、実行時には存在しない可能性があるためランタイムエラーになる。
            //ifやand/orを実行するときは新しいフレームが必要になる可能性がある。

            //ローカルフレーム内へのdef
            write_u8(vm::tag::LET_LOCAL, &mut ctx.buf);

            //ローカルフレーム内に新しいシンボルを追加
            let symbol = iform.as_ref().symbol();
            let symbol = symbol.capture(obj);
            ctx.frames.last_mut().unwrap().push(symbol);
        }
    }

    fn codegen_if(iform: &Reachable<IFormIf>, ctx: &mut CGCtx, obj: &mut Object) {
        //TEST式を先に書き込む
        pass_codegen(&iform.as_ref().test().reach(obj), ctx, obj);

        //THEN式とELSE式はJUMPさせるためにバッファの大きさを知る必要がある。
        //それぞれ別のバッファを作成してそちらに書き込むようにする

        let buf_then = {
            let mut ctx_then = CGCtx {
                buf: Vec::new(),
                constants: ctx.constants,
                frames: ctx.frames,
            };
            pass_codegen(&iform.as_ref().then().reach(obj), &mut ctx_then, obj);
            ctx_then.buf
        };

        let buf_else = {
            let mut ctx_else = CGCtx {
                buf: Vec::new(),
                constants: ctx.constants,
                frames: ctx.frames,
            };
            pass_codegen(&iform.as_ref().else_().reach(obj), &mut ctx_else, obj);

            ctx_else.buf
        };

        //タグ
        write_u8(vm::tag::IF, &mut ctx.buf);

        //ジャンプ先までのオフセットを書き込む
        //※オフセットは2Byteで足りる？
        //then式の長さ + ジャンプ命令3Byte
        let jump_offset = buf_then.len() + 3;
        debug_assert!(jump_offset < u16::MAX as usize);
        write_u16(jump_offset as u16, &mut ctx.buf);

        //THEN式を書き込む
        ctx.buf.extend(buf_then);

        //ELSE式をスキップするためのジャンプを書き込む
        write_u8(vm::tag::JUMP_OFFSET, &mut ctx.buf);
        debug_assert!(buf_else.len() < u16::MAX as usize);
        write_u16(buf_else.len() as u16, &mut ctx.buf);

        //ELSE式を書き込む
        ctx.buf.extend(buf_else);
    }

    fn codegen_local(iform: &Reachable<IFormLocal>, ctx: &mut CGCtx, obj: &mut Object) {
        //新しいフレームをpush
        write_u8(vm::tag::PUSH_EMPTY_ENV, &mut ctx.buf);
        ctx.frames.push(Vec::new());

        //TODO letはlocal内のトップレベルコンテキストのみ許可される

        //bodの式を順に評価
        pass_codegen(&iform.as_ref().body().reach(obj), ctx, obj);

        //フレームをpop
        write_u8(vm::tag::POP_ENV, &mut ctx.buf);
        ctx.frames.pop();
    }

    fn codegen_lref(iform: &Reachable<IFormLRef>, ctx: &mut CGCtx, _obj: &mut Object) {
        let (frame_offset, cell_index) = lookup_local_refer(iform.as_ref().symbol().as_ref(), ctx);
        debug_assert!(frame_offset < u16::MAX as usize);
        debug_assert!(cell_index < u16::MAX as usize);

        //タグ
        write_u8(vm::tag::REF_LOCAL, &mut ctx.buf);
        //フレームインデックス
        write_u16(frame_offset as u16, &mut ctx.buf);
        //フレーム内インデックス
        write_u16(cell_index as u16, &mut ctx.buf);
    }

    fn codegen_gref(iform: &Reachable<IFormGRef>, ctx: &mut CGCtx, obj: &mut Object) {
        //タグ
        write_u8(vm::tag::REF_GLOBAL, &mut ctx.buf);

        //キャプチャを取得して、キャプチャが保持する移動しないオブジェクトへの参照のポインタを書き込む。
        let symbol = iform.as_ref().symbol();
        let symbol = symbol.cast_value().clone();
        let index = ctx.add_constant(symbol, obj);
        write_u16(index as u16, &mut ctx.buf);
    }

    fn codegen_fun(iform: &Reachable<IFormFun>, ctx: &mut CGCtx, obj: &mut Object) {
        //タグ
        write_u8(vm::tag::CLOSURE, &mut ctx.buf);
        //引数の数
        //TODO requireやrest、optionalの対応
        write_u8(iform.as_ref().len_params() as u8, &mut ctx.buf);

        let mut new_frame: Vec<Cap<Symbol>> = Vec::new();
        //TODO ダミーシンボルめんどくさい。
        //クロージャフレームの最初にはクロージャ自身が入っているためダミーのシンボルを先頭に追加
        new_frame.push(super::literal::app_symbol().make().capture(obj));

        for index in 0 ..  iform.as_ref().len_params() {
            new_frame.push(iform.as_ref().get_param(index).capture(obj));
        }

        ctx.frames.push(new_frame);
        let mut constants:Vec<Cap<Any>> = Vec::new();
        let buf_body = {
            let mut ctx_body = CGCtx {
                buf: Vec::new(),
                constants: &mut constants,
                frames: ctx.frames,
            };
            //クロージャの本体を変換
            pass_codegen(&iform.as_ref().body().reach(obj), &mut ctx_body, obj);

            ctx_body.buf
        };
        ctx.frames.pop();

        let closure_constant_start = ctx.constants.len();
        let closure_constant_len = constants.len();
        debug_assert!(closure_constant_start < u16::MAX as usize);
        debug_assert!(closure_constant_len < u16::MAX as usize);

        //Closure内の定数一覧の数を書き込む
        write_u16(closure_constant_start as u16, &mut ctx.buf);
        write_u16(closure_constant_len as u16, &mut ctx.buf);
        //Closure内で使用した定数を追加する
        ctx.constants.extend(constants);

        //本体の長さを書き込む(本体の末尾にRETURNタグが書き込まれるので+1している)
        let size = buf_body.len() + 1;
        debug_assert!(size < u16::MAX as usize);
        write_u16(size as u16, &mut ctx.buf);

        //本体を書き込む
        ctx.buf.extend(buf_body);

        //リターンタグ
        write_u8(vm::tag::RETURN, &mut ctx.buf);
    }

    fn codegen_seq(iform: &Reachable<IFormSeq>, ctx: &mut CGCtx, obj: &mut Object) {
        for iform in iform.as_ref().body().reach(obj).iter() {
            pass_codegen(&iform.reach(obj), ctx, obj);
        }
    }

    fn codegen_call(iform: &Reachable<IFormCall>, ctx: &mut CGCtx, obj: &mut Object) {
        //push continuation
        write_u8(vm::tag::PUSH_CONT, &mut ctx.buf);

        //push env header
        write_u8(vm::tag::PUSH_ARG_PREPARE_ENV, &mut ctx.buf);

        //eval app
        pass_codegen(&iform.as_ref().app().reach(obj), ctx, obj);
        write_u8(vm::tag::PUSH_APP, &mut ctx.buf);

        //eval and push argument
        let num_args = iform.as_ref().len_args();
        for index in 0..num_args {
            let arg = iform.as_ref().get_arg(index).reach(obj);
            pass_codegen(&arg, ctx, obj);
            write_u8(vm::tag::PUSH_ARG, &mut ctx.buf);
        }

        //apply
        write_u8(vm::tag::CALL, &mut ctx.buf);
    }

    fn codegen_const(iform: &Reachable<IFormConst>, ctx: &mut CGCtx, obj: &mut Object) {
        let v = iform.as_ref().value().as_ref();

        //ヒープ内に存在するオブジェクトか？
        if obj.is_in_heap_object(v) {
            write_u8(vm::tag::CONST_CAPTURE, &mut ctx.buf);
            //キャプチャを取得して、キャプチャが保持する移動しないオブジェクトへの参照のポインタを書き込む。
            let index = ctx.add_constant(iform.as_ref().value(), obj);

            write_u16(index as u16, &mut ctx.buf);

        } else if value::value_is_pointer(v) { //値はポインタか？
            //ヒープ外へのポインタはStaticなオブジェクト
            write_u8(vm::tag::CONST_STATIC, &mut ctx.buf);
            //Staticなオブジェクトは移動することがないのでポインタをそのまま書き込む
            let data = ptr_to_usize(v as *const Any);
            write_usize(data, &mut ctx.buf);

        } else {
            //IMMIDIATE VALUEならそのまま書き込む
            write_u8(vm::tag::CONST_IMMIDIATE, &mut ctx.buf);
            //そもそも単なる値なのでそのまま書き込む
            let data = ptr_to_usize(v as *const Any);
            write_usize(data, &mut ctx.buf);
        }
    }

    fn codegen_andor(iform: &Reachable<IFormAndOr>, ctx: &mut CGCtx, obj: &mut Object) {
        let num_exprs = iform.as_ref().len_exprs();
        let mut expr_buf_vec: Vec<Vec<u8>> = Vec::new();

        //次の処理の簡単にするために、後ろの式から順に変換する
        //expr_buf_vecの一番後ろには、and/or式の第一引数が来る。
        for index in (0..num_exprs).rev() {
            let buf_expr = {
                let mut ctx_expr = CGCtx {
                    buf: Vec::new(),
                    constants: ctx.constants,
                    frames: ctx.frames,
                };
                pass_codegen(&iform.as_ref().get_expr(index).reach(obj), &mut ctx_expr, obj);

                ctx_expr.buf
            };

            expr_buf_vec.push(buf_expr);
        }

        let kind = iform.as_ref().kind();
        for _ in 0..num_exprs {
            let buf = expr_buf_vec.pop().unwrap();
            //引数式の実行部分を追加
            ctx.buf.extend(buf);

            //結果が確定したときにJUMPする先のオフセットを計算
            //全ての式の一番後ろの位置を計算している(+3は各式を評価した後にand/or判定するためのタグとオフセットの3Byte分。
            //-3は一番後ろの式にはand/or判定タグがなく帳消しにするため)
            let offset = expr_buf_vec.iter().fold(0isize, |total, buf| total + buf.len() as isize + 3) - 3;
            if offset > 0 {
                let tag = match kind {
                    AndOrKind::And => vm::tag::AND,
                    AndOrKind::Or =>  vm::tag::OR,
                    AndOrKind::MatchSuccess =>  vm::tag::MATCH_SUCCESS,
                };
                write_u8(tag, &mut ctx.buf);

                //結果が確定したときに飛ぶ先を書き込む
                debug_assert!((offset as usize) < (u16::MAX as usize));
                write_u16(offset as u16, &mut ctx.buf);
            }
        }
    }

    fn codegen_container(iform: &Reachable<IFormContainer>, ctx: &mut CGCtx, obj: &mut Object) {
        //push continuation
        write_u8(vm::tag::PUSH_CONT, &mut ctx.buf);

        //push env header
        write_u8(vm::tag::PUSH_ARG_PREPARE_ENV, &mut ctx.buf);

        //eval and push argument
        let num_args = iform.as_ref().len_exprs();
        for index in 0..num_args {
            let arg = iform.as_ref().get_expr(index).reach(obj);
            pass_codegen(&arg, ctx, obj);
            write_u8(vm::tag::PUSH_ARG_UNCHECK, &mut ctx.buf);
        }

        //call
        let tag = match iform.as_ref().kind() {
            iform::ContainerKind::Tuple => vm::tag::TUPLE,
            iform::ContainerKind::Array => vm::tag::ARRAY,
        };

        write_u8(tag, &mut ctx.buf);
    }

    fn codegen_defrecv(iform: &Reachable<IFormDefRecv>, ctx: &mut CGCtx, obj: &mut Object) {
        write_u8(vm::tag::DEF_RECV, &mut ctx.buf);

        let index = ctx.add_constant(iform.as_ref().pattern(), obj);
        write_u16(index as u16, &mut ctx.buf);

        let index = ctx.add_constant(iform.as_ref().body().cast_value().clone(), obj);
        write_u16(index as u16, &mut ctx.buf);
    }

    fn codegen_object_switch(iform: &Reachable<IFormObjectSwitch>, ctx: &mut CGCtx, obj: &mut Object) {
        if let Some(target_obj) =  iform.as_ref().target_obj() {
            let target_obj = target_obj.reach(obj);
            pass_codegen(&target_obj, ctx, obj);

            write_u8(vm::tag::OBJECT_SWITCH, &mut ctx.buf);
        } else {
            write_u8(vm::tag::RETURN_OBJECT_SWITCH, &mut ctx.buf);
        }
    }

    fn lookup_local_refer(symbol: &Symbol, ctx: &CGCtx) -> (usize, usize) {
        let mut frame_offset = 0;
        //この関数内ではGCが発生しないため値を直接参照する
        for frame in ctx.frames.iter().rev() {
            let mut cell_offset = 0;
            for sym in frame.iter().rev() {
                if sym.as_ref() == symbol {
                    return (frame_offset, frame.len() - cell_offset - 1);
                }

                cell_offset += 1;
            }

            frame_offset += 1;
        }

        panic!("local variable not found {}", symbol);
    }

}

static SYMBOL_APP: Lazy<GCAllocationStruct<symbol::StaticSymbol>> = Lazy::new(|| {
    symbol::gensym_static("app")
});

mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn app_symbol() -> Reachable<symbol::Symbol> {
        Reachable::new_static(SYMBOL_APP.value.as_ref())
    }
}