use once_cell::sync::Lazy;

use crate::object::mm::GCAllocationStruct;
use crate::ptr::*;
use crate::err::{self, NResult, OutOfMemory, Exception};
use crate::object::Object;
use crate::value::*;
use crate::value::any::Any;
use crate::value::array::ArrayBuilder;
use crate::value::symbol::Symbol;
use crate::value::list::List;
use crate::value::syntax::Syntax;
use crate::value::iform::*;
use crate::value::func::*;

struct LocalVar {
    pub name: Cap<Symbol>,
    pub init_form: Option<Cap<iform::IForm>>,
}

///
/// Compile Context
pub struct CCtx<'a> {
    frames: &'a mut Vec<Vec<LocalVar>>,
    toplevel: bool,
    tail: bool,
}

#[derive(Debug)]
pub enum SyntaxException {
    OutOfMemory,
    TypeMismatch(err::TypeMismatch),
    MalformedFormat(err::MalformedFormat),
    DisallowContext,
}

impl From<err::OutOfMemory> for SyntaxException {
    fn from(_: err::OutOfMemory) -> Self {
        SyntaxException::OutOfMemory
    }
}

impl From<err::TypeMismatch> for SyntaxException {
    fn from(this: err::TypeMismatch) -> Self {
        SyntaxException::TypeMismatch(this)
    }
}

impl From<err::MalformedFormat> for SyntaxException {
    fn from(this: err::MalformedFormat) -> Self {
        SyntaxException::MalformedFormat(this)
    }
}

impl From<err::DisallowContext> for SyntaxException {
    fn from(_: err::DisallowContext) -> Self {
        SyntaxException::DisallowContext
    }
}

impl From<SyntaxException> for Exception {
    fn from(this: SyntaxException) -> Self {
        match this {
            SyntaxException::OutOfMemory => Exception::OutOfMemory,
            SyntaxException::TypeMismatch(inner) => Exception::TypeMismatch(inner),
            SyntaxException::MalformedFormat(inner) => Exception::MalformedFormat(inner),
            SyntaxException::DisallowContext => Exception::DisallowContext,
        }
    }
}

pub fn compile_transform(sexp: &Reachable<Any>, obj: &mut Object) -> NResult<iform::IForm, SyntaxException> {
    let mut frames = Vec::new();
    let mut ctx = CCtx {
        frames: &mut frames,
        toplevel: true,
        tail: false,
    };

    pass_transform(sexp, &mut ctx, obj)
}

pub fn compile(sexp: &Reachable<Any>, obj: &mut Object) -> NResult<compiled::Code, SyntaxException> {
    let iform = compile_transform(sexp, obj)?.reach(obj);

    let code = codegen::code_generate(&iform, obj)?;
    Ok(code)
}

#[inline]
fn alloc_into_iform<T: AsIForm>(result: Result<Ref<T>, OutOfMemory>) -> NResult<IForm, SyntaxException> {
    match result {
        Ok(v) => Ok(v.into_iform()),
        Err(_) => Err(SyntaxException::OutOfMemory),
    }
}

//
// pass 1
// Covnerts S expression into intermediates form (IForm).
fn pass_transform(sexp: &Reachable<Any>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    if let Some(list) = sexp.try_cast::<List>() {
        if list.as_ref().is_nil() {
            alloc_into_iform(IFormConst::alloc(sexp, obj))

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
        alloc_into_iform(IFormConst::alloc(sexp, obj))
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

fn transform_symbol(symbol: &Reachable<Symbol>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    match lookup_localvar(symbol.as_ref(), ctx) {
        LookupResult::Var(_lvar) => {
            alloc_into_iform(IFormLRef::alloc(symbol, obj))
        }
        LookupResult::Const(constant) => {
            Ok(Ref::new(constant).into_iform())
        }
        LookupResult::Notfound => {
            alloc_into_iform(IFormGRef::alloc(symbol, obj))
        }
    }
}

fn transform_apply(list: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let is_tail = ctx.tail;
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
        tail: false,
    };

    //適用される値を変換
    let app =  pass_transform(&app.reach(obj), &mut ctx, obj)?.reach(obj);

    //引数部分の値を変換
    let count = list.as_ref().tail().as_ref().count();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj)?;

    for v in list.as_ref().tail().reach(obj).iter(obj) {
        let iform = pass_transform(&v.reach(obj), &mut ctx, obj)?;
        unsafe { builder_args.push_uncheck(&iform, obj) };
    }
    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    alloc_into_iform(IFormCall::alloc(&app, &args, is_tail, obj))
}

fn transform_tuple(tuple: &Reachable<tuple::Tuple>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let is_tail = ctx.tail;
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
        tail: false,
    };

    let app = pass_transform(&tuple::literal::tuple().into_value(), &mut ctx, obj)?.reach(obj);

    let count = tuple.as_ref().len();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj)?;

    for index in 0..count {
        let iform = pass_transform(&tuple.as_ref().get(index).reach(obj), &mut ctx, obj)?;
        unsafe { builder_args.push_uncheck(&iform, obj) };
    }
    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    alloc_into_iform(IFormCall::alloc(&app, &args, is_tail, obj))
}

fn transform_array(array: &Reachable<array::Array<Any>>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let is_tail = ctx.tail;
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
        tail: false,
    };

    let app = pass_transform(&array::literal::array().into_value(), &mut ctx, obj)?.reach(obj);

    let count = array.as_ref().len();
    let mut builder_args = ArrayBuilder::<IForm>::new(count, obj)?;

    for index in 0..count {
        let iform = pass_transform(&array.as_ref().get(index).reach(obj), &mut ctx, obj)?;
        unsafe { builder_args.push_uncheck(&iform, obj) };
    }
    let args = builder_args.get().reach(obj);

    //IFormCallを作成して戻り値にする
    alloc_into_iform(IFormCall::alloc(&app, &args, is_tail, obj))
}

fn transform_syntax(syntax: &Reachable<Syntax>, args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let syntax = syntax.as_ref();

    //TODO 引数の数や型のチェック

    //Syntaxを実行してIFormに変換する
    syntax.transform(args, ctx, obj)
}

fn syntax_if(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let mut test_ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
        tail: false,
    };
    let pred = pass_transform(&args.as_ref().head().reach(obj), &mut test_ctx, obj)?.reach(obj);

    let args = args.as_ref().tail();
    let true_ = args.as_ref().head().reach(obj);

    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: false,
        tail: ctx.tail,
    };

    let args = args.as_ref().tail();
    let false_ = if args.as_ref().is_nil() {
        alloc_into_iform(IFormConst::alloc(&bool::Bool::false_().into_value(), obj))
    } else {
        let false_ = args.as_ref().head().reach(obj);
        pass_transform(&false_, &mut ctx, obj)
    }?;

    let false_ = false_.reach(obj);
    let true_ = pass_transform(&true_, &mut ctx, obj)?.reach(obj);

    alloc_into_iform(IFormIf::alloc(&pred, &true_, &false_, obj))
}

fn syntax_cond(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    fn cond_inner(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
        let is_last = args.as_ref().tail().as_ref().is_nil();

        let clause = args.as_ref().head();
        if let Some(clause) = clause.try_cast::<List>() {
            let test = clause.as_ref().head().reach(obj);
            let then_clause = clause.as_ref().tail().reach(obj);

            //最後の節のTEST式がシンボルのelseなら、無条件でbody部分を実行するように変換する
            if is_last {
                if let Some(else_) = test.try_cast::<Symbol>() {
                    if else_.as_ref().as_ref() == "else" {
                        return transform_begin(&then_clause, ctx, obj);
                    }
                }
            }

            let mut test_ctx = CCtx {
                frames: ctx.frames,
                toplevel: false,
                tail: false,
            };
            //TEST部分を変換
            let test_iform = pass_transform(&test, &mut test_ctx, obj)?.reach(obj);
            //TESTの結果がtrueだったときに実行する式を変換
            let exprs_iform = transform_begin(&then_clause, ctx, obj)?.reach(obj);
            //TESTの結果がfalseだったときの次の節を変換
            let next_iform = if is_last {
                //最後の節ならfalseを返すようにする
                IFormConst::alloc(&bool::Bool::false_().into_value(), obj)?.into_iform()
            } else {
                //続きの節があるなら再帰的に変換する
                cond_inner(&args.as_ref().tail().reach(obj), ctx, obj)?
            }.reach(obj);

            alloc_into_iform(IFormIf::alloc(&test_iform, &exprs_iform, &next_iform, obj))

        } else {
            Err(err::TypeMismatch::new(clause, list::List::typeinfo()).into())
        }
    }

    //(cond)のようにテスト部分が空のcondであれば
    if args.as_ref().is_nil() {
        //無条件でfalseを返す
        alloc_into_iform(IFormConst::alloc(&bool::Bool::false_().into_value(), obj))
    } else {
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: false,
            tail: ctx.tail,
        };

        cond_inner(args, &mut ctx, obj)
    }
}

fn transform_begin(body: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //Beginは現在のコンテキスト(トップレベルや末尾文脈)をそのまま引き継いで各式を評価します

    let size = body.as_ref().count();
    let mut builder = array::ArrayBuilder::new(size, obj)?;

    for (index, sexp) in body.iter(obj).enumerate() {
        //最後の式か？
        let mut ctx = if index == size - 1 {
            //最後の式ならもともとのtail文脈を引き継ぐ
            CCtx {
                frames: ctx.frames,
                toplevel: ctx.toplevel,
                tail: ctx.tail,
            }
        } else {
            //途中の式はすべてtail文脈ではない
            CCtx {
                frames: ctx.frames,
                toplevel: ctx.toplevel,
                tail: false,
            }
        };

        let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj)?;
        unsafe { builder.push_uncheck(&iform, obj) };
    }

    alloc_into_iform(IFormSeq::alloc(&builder.get().reach(obj), obj))
}

fn syntax_begin(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    transform_begin(args, ctx, obj)
}

fn syntax_def_recv(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //def-recvはトップレベルのコンテキストで使用可能(letやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {
        let pat = args.as_ref().head().reach(obj);
        let body = args.as_ref().tail().reach(obj);

        alloc_into_iform(IFormDefRecv::alloc(&pat, &body, obj))
    } else {
        Err(SyntaxException::DisallowContext)
    }
}

fn syntax_fun(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let params = args.as_ref().head().reach(obj);
    if let Some(params) = params.try_cast::<List>() {
        let mut builder_params = ArrayBuilder::<Symbol>::new(params.as_ref().count(), obj)?;
        let mut local_frame: Vec<LocalVar> = Vec::new();

        //TODO keywordやrest引数の処理
        for param in params.iter(obj) {
            if let Some(symbol) = param.try_cast::<Symbol>() {
                unsafe { builder_params.push_uncheck(symbol, obj) };

                local_frame.push(LocalVar {
                    name: symbol.clone().capture(obj),
                    init_form: None,
                });

            } else {
                return Err(err::TypeMismatch::new(param, symbol::Symbol::typeinfo()).into());
            }
        }

        let params = builder_params.get().reach(obj);

        //ローカルフレームを追加
        ctx.frames.push(local_frame);
        //funのbodyは新しいトップレベルになる
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: true,
            tail: true,
        };

        //ローカルフレーム内でBody部分を変換
        let body = args.as_ref().tail().reach(obj);
        let body = transform_begin(&body, &mut ctx, obj)?.reach(obj);

        //ローカルフレーム削除
        ctx.frames.pop();

        alloc_into_iform(IFormFun::alloc(&params, &body, obj))
    } else {
        Err(err::TypeMismatch::new(params.make(), list::List::typeinfo()).into())
    }
}

fn syntax_local(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //ローカルフレームを作成する
    let frame: Vec<LocalVar> = Vec::new();

    //コンパイルコンテキストにローカルフレームをプッシュ
    ctx.frames.push(frame);
    //localは新しいトップレベルになる
    let mut ctx = CCtx {
        frames: ctx.frames,
        toplevel: true,
        tail: ctx.tail,
    };

    //ローカルフレームが積まれた状態でBody部分を変換
    let body = transform_begin(&args, &mut ctx, obj)?.reach(obj);

    ctx.frames.pop();

    alloc_into_iform(IFormLocal::alloc(&body, obj))
}

fn syntax_let(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    if ctx.toplevel == false {
        return Err(SyntaxException::DisallowContext);
    }

    let symbol = args.as_ref().head().reach(obj);
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: false,
            tail: false,
        };

        let value = args.as_ref().tail().as_ref().head().reach(obj);
        let iform = pass_transform(&value, &mut ctx, obj)?.reach(obj);

        //現在のローカルフレームに新しく定義した変数を追加
        if let Some(cur_frame) = ctx.frames.last_mut() {
            cur_frame.push(LocalVar {
                    name: symbol.make().capture(obj),
                    init_form: Some(iform.make().capture(obj)),
                });
        }

        alloc_into_iform(IFormLet::alloc(&symbol, &iform, false, obj))
    } else {
        Err(err::TypeMismatch::new(symbol.make(), symbol::Symbol::typeinfo()).into())
    }
}

fn syntax_let_global(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let symbol = args.as_ref().head().reach(obj);
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let mut ctx = CCtx {
            frames: ctx.frames,
            toplevel: false,
            tail: false,
        };

        let value = args.as_ref().tail().as_ref().head().reach(obj);
        let iform = pass_transform(&value, &mut ctx, obj)?.reach(obj);

        alloc_into_iform(IFormLet::alloc(&symbol, &iform, true, obj))
    } else {
        Err(err::TypeMismatch::new(symbol.make(), symbol::Symbol::typeinfo()).into())
    }
}

fn syntax_quote(args: &Reachable<List>, _ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let val = args.as_ref().head().reach(obj);
    alloc_into_iform(IFormConst::alloc(&val, obj))
}


#[allow(unused_variables)]
fn syntax_unquote(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    unimplemented!()
}

#[allow(unused_variables)]
fn syntax_bind(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    unimplemented!()
}

fn syntax_match(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //パターン部が一つもなければUnitを返す
    if args.as_ref().is_nil() {
        alloc_into_iform(IFormConst::alloc(&tuple::Tuple::unit().into_value(), obj))
    } else {
        let match_expr = crate::value::syntax::r#match::translate(args, obj)?.into_value().reach(obj);
        pass_transform(&match_expr, ctx, obj)
    }
}

fn syntax_fail_catch(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //fail-catchはmatch式の中でだけ使用される特殊な構文
    //引数の式を評価し、値がFAILでなければその値を返す。
    //引数全てFAILならFAILを返す。
    //特殊なor構文のような動作。

    let size = args.as_ref().count();
    debug_assert!(size != 0);

    let mut builder = array::ArrayBuilder::new(size, obj)?;

    for (index, sexp) in args.iter(obj).enumerate() {
        //最後の式か？
        let mut ctx = if index == size - 1 {
            //最後の式ならもともとのtail文脈を引き継ぐ
            CCtx {
                frames: ctx.frames,
                toplevel: false,
                tail: ctx.tail,
            }
        } else {
            //途中の式はすべてtail文脈ではない
            CCtx {
                frames: ctx.frames,
                toplevel: false,
                tail: false,
            }
        };

        let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj)?;
        unsafe { builder.push_uncheck(&iform, obj) };
    }

    alloc_into_iform(IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::MatchSuccess, obj))
}

fn syntax_and(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let size = args.as_ref().count();
    //(and)のように引数が一つもなければ
    if size == 0 {
        alloc_into_iform(IFormConst::alloc(&bool::Bool::true_().into_value(), obj))

    } else {
        let mut builder = array::ArrayBuilder::new(size, obj)?;

        for (index, sexp) in args.iter(obj).enumerate() {
            //最後の式か？
            let mut ctx = if index == size - 1 {
                //最後の式ならもともとのtail文脈を引き継ぐ
                CCtx {
                    frames: ctx.frames,
                    toplevel: false,
                    tail: ctx.tail,
                }
            } else {
                //途中の式はすべてtail文脈ではない
                CCtx {
                    frames: ctx.frames,
                    toplevel: false,
                    tail: false,
                }
            };
            let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj)?;
            unsafe { builder.push_uncheck(&iform, obj) };
        }

        alloc_into_iform(IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::And, obj))
    }
}

fn syntax_or(args: &Reachable<List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    let size = args.as_ref().count();
    //(or)のように引数が一つもなければ
    if size == 0 {
        alloc_into_iform(IFormConst::alloc(&bool::Bool::false_().into_value(), obj))

    } else {
        let mut builder = array::ArrayBuilder::new(size, obj)?;

        for (index, sexp) in args.iter(obj).enumerate() {
            //最後の式か？
            let mut ctx = if index == size - 1 {
                //最後の式ならもともとのtail文脈を引き継ぐ
                CCtx {
                    frames: ctx.frames,
                    toplevel: false,
                    tail: ctx.tail,
                }
            } else {
                //途中の式はすべてtail文脈ではない
                CCtx {
                    frames: ctx.frames,
                    toplevel: false,
                    tail: false,
                }
            };
            let iform = pass_transform(&sexp.reach(obj), &mut ctx, obj)?;
            unsafe { builder.push_uncheck(&iform, obj) };
        }

        alloc_into_iform(IFormAndOr::alloc(&builder.get().reach(obj), AndOrKind::Or, obj))
    }
}

fn syntax_object_switch(args: &Reachable<list::List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //TODO グローバル環境のbegin内にある場合、続きの式があるので動作がおかしくなる。
    //TODO 末尾文脈でのみ許可するようにしたい

    //object-switchはトップレベルのコンテキストで使用可能(localやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {

        let target_obj = args.as_ref().head().reach(obj);
        let iform = pass_transform(&target_obj, ctx, obj)?;
        let iform = iform.reach(obj);

      alloc_into_iform(IFormObjectSwitch::alloc(Some(&iform), obj))
    } else {
        Err(SyntaxException::DisallowContext)
    }
}

fn syntax_return_object_switch(_args: &Reachable<list::List>, ctx: &mut CCtx, obj: &mut Object) -> NResult<IForm, SyntaxException> {
    //TODO グローバル環境のbegin内にある場合、続きの式があるので動作がおかしくなる。
    //TODO 末尾文脈でのみ許可するようにしたい

    //object-switchはトップレベルのコンテキストで使用可能(localやfunで作成されたローカルフレーム内では使用不可能)
    if ctx.frames.is_empty() {
        alloc_into_iform(IFormObjectSwitch::alloc(None, obj))
    } else {
        Err(SyntaxException::DisallowContext)
    }
}

mod codegen {
    use core::panic;

    use crate::object::Allocator;
    use crate::object::mm::ptr_to_usize;
    use crate::ptr::*;
    use crate::err::*;
    use crate::vm;
    use crate::object::Object;
    use crate::value::{*, self};
    use crate::value::any::Any;
    use crate::value::iform::*;
    use crate::value::symbol::Symbol;
    use crate::vm::{write_u16, write_u8, write_usize};

    struct LocalFrame {
        frame: Vec<Cap<Symbol>>,
        free_vars: Option<Vec<(Cap<Symbol>, LocalRefer)>>,
    }

    //
    // Code Generation Arg
    struct CGCtx<'a> {
        pub buf: Vec<u8>,
        pub constants: &'a mut Vec<Cap<Any>>,
        pub frames: &'a mut Vec<LocalFrame>,
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

    pub fn code_generate(iform: &Reachable<IForm>, obj: &mut Object) -> NResult<compiled::Code, OutOfMemory> {
        let mut constants:Vec<Cap<Any>> = Vec::new();
        let mut frames:Vec<LocalFrame> = Vec::new();

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
        if iform.as_ref().force_global() || ctx.frames.is_empty() {
            //タグ
            write_u8(vm::tag::LET_GLOBAL, &mut ctx.buf);

            //キャプチャを取得して、キャプチャが保持する移動しないオブジェクトへの参照のポインタを書き込む。
            let symbol = iform.as_ref().symbol();
            let symbol = symbol.cast_value().clone();
            let index = ctx.add_constant(symbol, obj);

            write_u16(index as u16, &mut ctx.buf);


        } else {
            //ローカルフレーム内へのdef
            write_u8(vm::tag::LET_LOCAL, &mut ctx.buf);

            //ローカルフレーム内に新しいシンボルを追加
            let symbol = iform.as_ref().symbol();
            let symbol = symbol.capture(obj);
            ctx.frames.last_mut().unwrap().frame.push(symbol);
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
        ctx.frames.push(LocalFrame {
            frame: Vec::new(),
            free_vars: None,
        });

        //bodの式を順に評価
        pass_codegen(&iform.as_ref().body().reach(obj), ctx, obj);

        //フレームをpop
        write_u8(vm::tag::POP_ENV, &mut ctx.buf);
        ctx.frames.pop();
    }

    fn codegen_lref(iform: &Reachable<IFormLRef>, ctx: &mut CGCtx, obj: &mut Object) {
        let (tag, frame_offset, cell_index) = match lookup_local_refer(iform.as_ref().symbol(), ctx, obj) {
            LocalRefer::Normal(frame_offset, cell_index) => {
                (vm::tag::REF_LOCAL, frame_offset, cell_index)
            }
            LocalRefer::FreeVar(frame_offset, cell_index) => {
                (vm::tag::REF_FREE, frame_offset, cell_index)
            }
        };

        debug_assert!(frame_offset < u16::MAX as usize);
        debug_assert!(cell_index < u16::MAX as usize);

        //タグ
        write_u8(tag, &mut ctx.buf);
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
        //クロージャフレームの最初にはクロージャ自身が入っているためダミーのシンボルを先頭に追加
        new_frame.push(super::literal::app_symbol().make().capture(obj));

        for index in 0 ..  iform.as_ref().len_params() {
            new_frame.push(iform.as_ref().get_param(index).capture(obj));
        }

        ctx.frames.push(LocalFrame {
            frame: new_frame,
            free_vars: Some(Vec::new()),
        });
        let mut constants:Vec<Cap<Any>> = Vec::new();
        let buf_body = {
            let mut ctx_body = CGCtx {
                buf: Vec::new(),
                constants: &mut constants,
                frames: ctx.frames,
            };
            //クロージャの本体を変換
            pass_codegen(&iform.as_ref().body().reach(obj), &mut ctx_body, obj);

            //リターンタグ
            write_u8(vm::tag::RETURN, &mut ctx_body.buf);

            ctx_body.buf
        };
        let free_vars = ctx.frames.pop().unwrap().free_vars.unwrap();

        let closure_constant_start = ctx.constants.len();
        let closure_constant_len = constants.len();
        debug_assert!(closure_constant_start < u16::MAX as usize);
        debug_assert!(closure_constant_len < u16::MAX as usize);

        //Closure内の定数一覧の数を書き込む
        write_u16(closure_constant_start as u16, &mut ctx.buf);
        write_u16(closure_constant_len as u16, &mut ctx.buf);
        //Closure内で使用した定数を追加する
        ctx.constants.extend(constants);

        //本体の長さを書き込む
        let size = buf_body.len();
        debug_assert!(size < u16::MAX as usize);
        write_u16(size as u16, &mut ctx.buf);

        //自由変数の数を書き込む
        let num_free_vars = free_vars.len();
        debug_assert!(num_free_vars < u16::MAX as usize);
        write_u16(num_free_vars as u16, &mut ctx.buf);

        //本体を書き込む
        ctx.buf.extend(buf_body);

        //Closureに自由変数を取り込むための命令を書き込む
        for (index, (_, refer)) in free_vars.into_iter().enumerate() {

            let (tag, frame_offset, cell_index) = match refer {
                LocalRefer::Normal(frame_offset, cell_index) => {
                    (vm::tag::CAPTURE_FREE_REF_LOCAL, frame_offset, cell_index)
                }
                LocalRefer::FreeVar(frame_offset, cell_index) => {
                    (vm::tag::CAPTURE_FREE_REF_FREE, frame_offset, cell_index)
                }
            };

            debug_assert!(frame_offset < u16::MAX as usize);
            debug_assert!(cell_index < u16::MAX as usize);

            //タグ
            write_u8(tag, &mut ctx.buf);
            //参照先フレームインデックス
            write_u16(frame_offset as u16, &mut ctx.buf);
            //参照先フレーム内インデックス
            write_u16(cell_index as u16, &mut ctx.buf);
            //書き込み先のインデックス
            write_u16(index as u16, &mut ctx.buf);
        }
    }

    fn codegen_seq(iform: &Reachable<IFormSeq>, ctx: &mut CGCtx, obj: &mut Object) {
        for iform in iform.as_ref().body().reach(obj).iter() {
            pass_codegen(&iform.reach(obj), ctx, obj);
        }
    }

    fn codegen_call(iform: &Reachable<IFormCall>, ctx: &mut CGCtx, obj: &mut Object) {
        if iform.as_ref().is_tail() {
            write_u8(vm::tag::CALL_TAIL_PREPARE, &mut ctx.buf);
        } else {
            write_u8(vm::tag::CALL_PREPARE, &mut ctx.buf);
        }

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
        if iform.as_ref().is_tail() {
            write_u8(vm::tag::CALL_TAIL, &mut ctx.buf);
        } else {
            write_u8(vm::tag::CALL, &mut ctx.buf);
        }
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

    enum LocalRefer {
        Normal(usize, usize),
        FreeVar(usize, usize),
    }

    fn lookup_local_refer(symbol: Ref<Symbol>, ctx: &mut CGCtx, obj: &mut Object) -> LocalRefer {

        fn localrefer(refer: LocalRefer, symbol: Ref<Symbol>, free_vars_frames: Vec<(&mut Vec<(Cap<Symbol>, LocalRefer)>, usize)>, obj: &mut Object) -> LocalRefer {
            if free_vars_frames.is_empty() {
                refer
            } else {
                let mut refer = refer;
                for (free_vars, frame_offset) in free_vars_frames.into_iter().rev() {
                    let pos = free_vars.len();
                    free_vars.push((symbol.clone().capture(obj), refer));

                    refer = LocalRefer::FreeVar(frame_offset, pos);
                }

                refer
            }
        }

        let mut frame_offset = 0;
        let mut free_vars_frames: Vec<(&mut Vec<(Cap<Symbol>, LocalRefer)>, usize)> = Vec::new();
        //この関数内ではGCが発生しないため値を直接参照する
        for localframe in ctx.frames.iter_mut().rev() {
            for (cell_offset, sym) in localframe.frame.iter().rev().enumerate() {
                if sym.as_ref() == symbol.as_ref() {
                    return localrefer(LocalRefer::Normal(frame_offset, localframe.frame.len() - cell_offset - 1), symbol, free_vars_frames, obj);
                }
                }

            if let Some(free_vars) = localframe.free_vars.as_mut() {
                for (cell_offset, (sym, _)) in free_vars.iter().rev().enumerate() {
                    if sym.as_ref() == symbol.as_ref() {
                        return localrefer(LocalRefer::FreeVar(frame_offset, cell_offset), symbol, free_vars_frames, obj);
                    }
            }

                free_vars_frames.push((free_vars, frame_offset));
                frame_offset = 0;
            } else {
            frame_offset += 1;
            }
        }

        //pass1のtransformの時点でローカル変数の解決は完了している
        //ここで見つからないのは不具合なのでpanicさせる
        panic!("local variable not found {}", symbol.as_ref());
    }

}

fn func_compile(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = crate::vm::refer_arg::<Any>(0, obj).reach(obj);

    let compiled = compile(&v, obj)?;
    Ok(compiled.into_value())
}

fn func_compile_transform(_num_rest: usize, obj: &mut Object) -> NResult<Any, Exception> {
    let v = crate::vm::refer_arg::<Any>(0, obj).reach(obj);

    let compiled = compile_transform(&v, obj)?;
    Ok(compiled.into_value())
}

static SYMBOL_APP: Lazy<GCAllocationStruct<symbol::StaticSymbol>> = Lazy::new(|| {
    symbol::gensym_static("app")
});

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("if", 2, 1, false, syntax_if))
});

static SYNTAX_BEGIN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("begin", 0, 0, true, syntax_begin))
});

static SYNTAX_COND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("cond", 0, 0, true, syntax_cond))
});

static SYNTAX_DEF_RECV: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("def-recv", 1, 0, true, syntax_def_recv))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("fun", 1, 0, true, syntax_fun))
});

static SYNTAX_LOCAL: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("local", 1, 0, true, syntax_local))
});

static SYNTAX_LET: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let", 2, 0, false, syntax_let))
});

static SYNTAX_LET_GLOBAL: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let-global", 2, 0, false, syntax_let_global))
});

static SYNTAX_QUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("quote", 1, 0, false, syntax_quote))
});

static SYNTAX_UNQUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("unquote", 1, 0, false, syntax_unquote))
});

static SYNTAX_BIND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("bind", 1, 0, false, syntax_bind))
});

static SYNTAX_MATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("match", 1, 0, true, syntax_match))
});

static SYNTAX_AND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("and", 0, 0, true, syntax_and))
});

static SYNTAX_OR: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("or", 0, 0, true, syntax_or))
});

static SYNTAX_OBJECT_SWITCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("object-switch", 1, 0, false, syntax_object_switch))
});

static SYNTAX_RETURN_OBJECT_SWITCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("return-object-switch", 0, 0, false, syntax_return_object_switch))
});

static SYNTAX_FAIL_CATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(syntax::Syntax::new("fail-catch", 0, 0, true, syntax_fail_catch))
});

static FUNC_COMPILE: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("compile", &[
            Param::new("v", ParamKind::Require, Any::typeinfo()),
            ],
            func_compile)
    )
});

static FUNC_COMPILE_TRANSFORM: Lazy<GCAllocationStruct<Func>> = Lazy::new(|| {
    GCAllocationStruct::new(
        Func::new("compile-transform", &[
            Param::new("v", ParamKind::Require, Any::typeinfo()),
            ],
            func_compile_transform)
    )
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("if", &Ref::new(&SYNTAX_IF.value));
    obj.define_global_value("begin", &Ref::new(&SYNTAX_BEGIN.value));
    obj.define_global_value("cond", &Ref::new(&SYNTAX_COND.value));
    obj.define_global_value("def-recv", &Ref::new(&SYNTAX_DEF_RECV.value));
    obj.define_global_value("fun", &Ref::new(&SYNTAX_FUN.value));
    obj.define_global_value("local", &Ref::new(&SYNTAX_LOCAL.value));
    obj.define_global_value("let", &Ref::new(&SYNTAX_LET.value));
    obj.define_global_value("let-global", &Ref::new(&SYNTAX_LET_GLOBAL.value));
    obj.define_global_value("quote", &Ref::new(&SYNTAX_QUOTE.value));
    obj.define_global_value("unquote", &Ref::new(&SYNTAX_UNQUOTE.value));
    obj.define_global_value("bind", &Ref::new(&SYNTAX_BIND.value));
    obj.define_global_value("match", &Ref::new(&SYNTAX_MATCH.value));
    obj.define_global_value("and", &Ref::new(&SYNTAX_AND.value));
    obj.define_global_value("or", &Ref::new(&SYNTAX_OR.value));
    obj.define_global_value("object-switch", &Ref::new(&SYNTAX_OBJECT_SWITCH.value));
    obj.define_global_value("return-object-switch", &Ref::new(&SYNTAX_RETURN_OBJECT_SWITCH.value));
    obj.define_global_value("compile", &Ref::new(&FUNC_COMPILE.value));
    obj.define_global_value("compile-transform", &Ref::new(&FUNC_COMPILE_TRANSFORM.value));
}

pub mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn quote() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_QUOTE.value)
    }

    pub fn unquote() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_UNQUOTE.value)
    }

    pub fn bind() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_BIND.value)
    }

    pub fn fun() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_FUN.value)
    }

    pub fn local() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_LOCAL.value)
    }

    pub fn let_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_LET.value)
    }

    pub fn match_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_MATCH.value)
    }

    pub fn if_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_IF.value)
    }

    pub fn begin() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_BEGIN.value)
    }

    pub fn cond() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_COND.value)
    }

    pub fn fail_catch() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_FAIL_CATCH.value)
    }

    pub fn app_symbol() -> Reachable<symbol::Symbol> {
        Reachable::new_static(SYMBOL_APP.value.as_ref())
    }
}
