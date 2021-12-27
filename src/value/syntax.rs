use crate::eval::eval;
use crate::value::symbol::Symbol;
use crate::value::list::List;
use crate::cap_eval;
use crate::value::*;
use crate::value::list;
use crate::ptr::*;
use crate::object::mm::GCAllocationStruct;
use crate::object::context::Context;
use std::fmt::{Debug, Display};
use std::panic;
use once_cell::sync::Lazy;

use super::array::ArrayBuilder;

pub mod r#match;

pub struct Syntax {
    name: String,
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&Reachable<list::List>, &mut Object) -> FPtr<Value>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
    std::mem::size_of::<Syntax>(),
    None,
    Syntax::eq,
    Syntax::clone_inner,
    Display::fmt,
    Syntax::is_type,
    None,
    None,
    None,
);

impl NaviType for Syntax {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&SYNTAX_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _obj: &mut Object) -> FPtr<Self> {
        //Syntaxのインスタンスはヒープ上に作られることがないため、自分自身を返す
        FPtr::new(self)
    }
}

impl Syntax {

    pub fn new<T: Into<String>>(name: T, require: usize, optional: usize, has_rest: bool
        , body: fn(&Reachable<list::List>, &mut Object) -> FPtr<Value>
    ) -> Self {
        Syntax {
            name: name.into(),
            require: require,
            optional: optional,
            has_rest: has_rest,
            body: body,
        }
    }

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&SYNTAX_TYPEINFO, other_typeinfo)
    }

    pub fn check_arguments(&self, args: &Reachable<list::List>) -> bool {
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn apply(&self, args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
        (self.body)(&args, obj)
    }
}

impl Eq for Syntax { }

impl PartialEq for Syntax {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

impl Display for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

fn is_true(v: &Value) -> bool {
    //predの結果がfalse値の場合だけ、falseとして扱う。それ以外の値はすべてtrue
    if let Some(v) = v.try_cast::<bool::Bool>() {
        v.is_true()
    } else {
        true
    }
}

fn syntax_if(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    let pred = cap_eval!(args.as_ref().head(), obj);
    let pred = is_true(pred.as_ref());

    let args = args.as_ref().tail();
    if pred {
        cap_eval!(args.as_ref().head(), obj)

    } else {
        let args = args.as_ref().tail();
        if args.as_ref().is_nil() {
            bool::Bool::false_().into_fptr().into_value()
        } else {
            cap_eval!(args.as_ref().head(), obj)
        }
    }
}

fn syntax_begin(args: &Reachable<List>, obj: &mut Object) -> FPtr<Value> {
    do_begin(args, obj)
}

pub(crate) fn do_begin(body: &Reachable<List>, obj: &mut Object) -> FPtr<Value> {
    let mut last: Option<FPtr<Value>> = None;
    for sexp in body.iter(obj) {
        //ここのFPtrはあえてCaptureしない
        //beginは最後に評価した式の結果だけを返せばいいので、
        //次のループでeval中にGCでこの結果が回収されたとしても関係ない。
        let e = cap_eval!(sexp, obj);
        last = Some(e);
    }

    if let Some(last) = last {
        last
    } else {
        tuple::Tuple::unit().into_fptr().into_value()
    }
}

fn syntax_cond(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    for (sexp, info) in args.iter_with_info(obj) {
        let sexp = sexp.reach(obj);

        if let Some(clause) = sexp.try_cast::<list::List>() {
            let test = clause.as_ref().head().reach(obj);

            //最後の節のTESTがシンボルのelseの場合、無条件でbody部分を評価します
            if let Some(else_) = test.try_cast::<symbol::Symbol>() {
                if else_.as_ref().as_ref() == "else" && info.is_tail {
                    let body = clause.as_ref().tail().reach(obj);

                    return do_begin(&body, obj);
                }
            }

            //TEST式を評価
            let result = eval(&test, obj);
            //TESTの結果がtrueなら続く式を実行して結果を返す
            if is_true(result.as_ref()) {
                let body = clause.as_ref().tail().reach(obj);
                return do_begin(&body, obj);
            }

        } else {
            panic!("cond clause require list. but got {:?}", sexp.as_ref());
        }
    }

    bool::Bool::false_().into_fptr().into_value()
}

fn syntax_def(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    let symbol = args.as_ref().head().reach(obj);
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let value = args.as_ref().tail().as_ref().head().reach(obj);
        let value = eval(&value, obj).reach(obj);

        obj.context().add_to_current_frame(symbol, &value);

        value.into_fptr()
    } else {
        panic!("def variable require symbol. But got {}", symbol.as_ref());
    }
}

fn syntax_def_recv(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    if obj.context().is_toplevel() {
        let pat = args.as_ref().head().reach(obj);
        let body = args.as_ref().tail().reach(obj);

        //現在のコンテキストにレシーバーを追加する
        obj.add_receiver(&pat, &body);

        //どの値を返すべき？
        bool::Bool::true_().into_fptr().into_value()
    } else {
        panic!("def-recv allow only top-level context")
    }
}

pub(crate) fn syntax_fun(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    let params = args.as_ref().head();
    if let Some(params) = params.try_cast::<list::List>() {
        let params = params.clone().reach(obj);

        let mut builder_params = ArrayBuilder::<symbol::Symbol>::new(params.as_ref().count(), obj);

        //TODO :optionalと:rest引数の対応
        for param in params.iter(obj) {
            match param.try_cast::<symbol::Symbol>() {
                Some(symbol) => {
                    builder_params.push(symbol.as_ref(), obj);
                }
                None => {
                    panic!("parameter require symbol. But got {:?}", param.as_ref())
                }
            }
        }


        let params = builder_params.get().reach(obj);
        let body = args.as_ref().tail().reach(obj);

        closure::Closure::alloc(&params, &body, obj).into_value()

    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }
}

fn syntax_let(args: &Reachable<List>, obj: &mut Object) -> FPtr<Value> {

    let mut symbol_vec: Vec<Reachable<Symbol>> = Vec::new();
    let mut val_vec: Vec<Reachable<Value>> = Vec::new();

    //局所変数指定の内容を解析
    let binders = args.as_ref().head().reach(obj);
    if let Some(binders) = binders.try_cast::<List>() {

        for bind in binders.iter(obj) {
            let bind = bind.reach(obj);

            if let Some(bind) = bind.try_cast::<List>() {
                if bind.as_ref().count() != 2 {
                    panic!("The let bind part require 2 length list. But got {:?}", bind.as_ref())
                }

                let symbol = bind.as_ref().head();
                if let Some(symbol) = symbol.try_cast::<symbol::Symbol>() {
                    symbol_vec.push(symbol.clone().reach(obj));

                    let val = bind.as_ref().tail().as_ref().head().reach(obj);
                    let val = eval(&val, obj).reach(obj);

                    val_vec.push(val);

                } else {
                    panic!("The let bind paramter require symbol. But got {:?}", symbol.as_ref())
                }

            } else {
                panic!("The let bind part require list. But got {:?}", bind.as_ref())
            }
        }
    } else {
        panic!("The let bind part require list. But got {:?}", binders.as_ref())
    }

    let frame: Vec::<(&Symbol, &Value)> = symbol_vec.into_iter().zip(val_vec.into_iter())
        .map(|(s, v)| (s.as_ref(), v.as_ref()))
        .collect();

    ////ローカルフレームを環境にプッシュ
    obj.context().push_local_frame(&frame);

    //Closure本体を実行
    let body = args.as_ref().tail().reach(obj);
    let result = syntax::do_begin(&body, obj);

    //ローカルフレームを環境からポップ
    obj.context().pop_local_frame();

    result
}

fn syntax_quote(args: &Reachable<list::List>, _obj: &mut Object) -> FPtr<Value> {
    let sexp = args.as_ref().head();
    sexp
}

fn syntax_unquote(_args: &Reachable<list::List>, _obj: &mut Object) -> FPtr<Value> {
    unimplemented!()
}

fn syntax_bind(_args: &Reachable<list::List>, _obj: &mut Object) -> FPtr<Value> {
    unimplemented!()
}

fn syntax_and(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    let mut last: Option<FPtr<Value>> = None;
    for sexp in args.iter(obj) {
        let result = cap_eval!(sexp, obj);
        if is_true(result.as_ref()) == false {
            return bool::Bool::false_().into_fptr().into_value();
        }

        last = Some(result);
    }

    if let Some(last) = last {
        last
    } else {
        bool::Bool::true_().into_fptr().into_value()
    }
}

fn syntax_or(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    for sexp in args.iter(obj) {
        let result = cap_eval!(sexp, obj);
        if is_true(result.as_ref()) {
            return result;
        }
    }

    bool::Bool::false_().into_fptr().into_value()
}

fn syntax_match(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    //パターン部が一つもなければUnitを返す
    if args.as_ref().is_nil() {
        tuple::Tuple::unit().into_fptr().into_value()
    } else {
        let match_expr = r#match::translate(args, obj).into_value();
        cap_eval!(match_expr, obj)
    }
}

static SYNTAX_IF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("if", 2, 1, false, syntax_if))
});

static SYNTAX_BEGIN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("begin", 0, 0, true, syntax_begin))
});

static SYNTAX_COND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("cond", 0, 0, true, syntax_cond))
});

static SYNTAX_DEF: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("def", 2, 0, false, syntax_def))
});

static SYNTAX_DEF_RECV: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("def-recv", 1, 0, true, syntax_def_recv))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("fun", 1, 0, true, syntax_fun))
});

static SYNTAX_LET: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let", 1, 0, true, syntax_let))
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

pub fn register_global(ctx: &mut Context) {
    ctx.define_value("if", Reachable::new_static(&SYNTAX_IF.value).cast_value());
    ctx.define_value("begin", Reachable::new_static(&SYNTAX_BEGIN.value).cast_value());
    ctx.define_value("cond", Reachable::new_static(&SYNTAX_COND.value).cast_value());
    ctx.define_value("def", Reachable::new_static(&SYNTAX_DEF.value).cast_value());
    ctx.define_value("def-recv", Reachable::new_static(&SYNTAX_DEF_RECV.value).cast_value());
    ctx.define_value("fun", Reachable::new_static(&SYNTAX_FUN.value).cast_value());
    ctx.define_value("let", Reachable::new_static(&SYNTAX_LET.value).cast_value());
    ctx.define_value("quote", Reachable::new_static(&SYNTAX_QUOTE.value).cast_value());
    ctx.define_value("unquote", Reachable::new_static(&SYNTAX_UNQUOTE.value).cast_value());
    ctx.define_value("bind", Reachable::new_static(&SYNTAX_BIND.value).cast_value());
    ctx.define_value("match", Reachable::new_static(&SYNTAX_MATCH.value).cast_value());
    ctx.define_value("and", Reachable::new_static(&SYNTAX_AND.value).cast_value());
    ctx.define_value("or", Reachable::new_static(&SYNTAX_OR.value).cast_value());
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

    pub fn let_() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_LET.value)
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
}