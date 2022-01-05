use crate::eval::eval;
use crate::cap_eval;
use crate::value::*;
use crate::value::list::{self, List};
use crate::value::symbol::Symbol;
use crate::ptr::*;
use crate::object::mm::GCAllocationStruct;
use crate::compile;
use crate::vm::is_true;

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
    body: fn(&Reachable<List>, &mut Object) -> FPtr<Value>,
    transform_body: fn(&Reachable<List>, &mut compile::CCtx, &mut Object) -> FPtr<crate::value::iform::IForm>,
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
        , translate_body: fn(&Reachable<list::List>, &mut crate::compile::CCtx, &mut Object) -> FPtr<iform::IForm>,
    ) -> Self {
        Syntax {
            name: name.into(),
            require: require,
            optional: optional,
            has_rest: has_rest,
            body: body,
            transform_body: translate_body,
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

    pub fn transform(&self, args: &Reachable<List>, ctx: &mut compile::CCtx, obj: &mut Object) -> FPtr<iform::IForm> {
        (self.transform_body)(args, ctx, obj)
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

fn syntax_local(args: &Reachable<List>, obj: &mut Object) -> FPtr<Value> {
    //空のフレームを追加
    let frame: Vec<(&Symbol, &Value)> = Vec::new();
    ////ローカルフレームを環境にプッシュ
    obj.context().push_local_frame(&frame);

    //Closure本体を実行
    let result = syntax::do_begin(&args, obj);

    //ローカルフレームを環境からポップ
    obj.context().pop_local_frame();
    result
}

fn syntax_let(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {
    let symbol = args.as_ref().head().reach(obj);
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let value = args.as_ref().tail().as_ref().head().reach(obj);
        let value = eval(&value, obj).reach(obj);

        if obj.context().add_to_current_frame(symbol, &value) == false {
            obj.define_global_value(symbol.as_ref(), value.as_ref())
        }

        value.into_fptr()
    } else {
        panic!("let variable require symbol. But got {}", symbol.as_ref());
    }
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
    GCAllocationStruct::new(Syntax::new("if", 2, 1, false, syntax_if, compile::syntax_if))
});

static SYNTAX_BEGIN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("begin", 0, 0, true, syntax_begin, compile::syntax_begin))
});

static SYNTAX_COND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("cond", 0, 0, true, syntax_cond, compile::syntax_cond))
});

static SYNTAX_DEF_RECV: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("def-recv", 1, 0, true, syntax_def_recv, compile::syntax_def_recv))
});

static SYNTAX_FUN: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("fun", 1, 0, true, syntax_fun, compile::syntax_fun))
});

static SYNTAX_LOCAL: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("local", 1, 0, true, syntax_local, compile::syntax_local))
});

static SYNTAX_LET: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("let", 2, 0, false, syntax_let, compile::syntax_let))
});

static SYNTAX_QUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("quote", 1, 0, false, syntax_quote, compile::syntax_quote))
});

static SYNTAX_UNQUOTE: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("unquote", 1, 0, false, syntax_unquote, compile::syntax_unquote))
});

static SYNTAX_BIND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("bind", 1, 0, false, syntax_bind, compile::syntax_bind))
});

static SYNTAX_MATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("match", 1, 0, true, syntax_match, compile::syntax_match))
});

static SYNTAX_AND: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("and", 0, 0, true, syntax_and, compile::syntax_and))
});

static SYNTAX_OR: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(Syntax::new("or", 0, 0, true, syntax_or, compile::syntax_or))
});

pub fn register_global(obj: &mut Object) {
    obj.define_global_value("if", &SYNTAX_IF.value);
    obj.define_global_value("begin", &SYNTAX_BEGIN.value);
    obj.define_global_value("cond", &SYNTAX_COND.value);
    obj.define_global_value("def-recv", &SYNTAX_DEF_RECV.value);
    obj.define_global_value("fun", &SYNTAX_FUN.value);
    obj.define_global_value("local", &SYNTAX_LOCAL.value);
    obj.define_global_value("let", &SYNTAX_LET.value);
    obj.define_global_value("quote", &SYNTAX_QUOTE.value);
    obj.define_global_value("unquote", &SYNTAX_UNQUOTE.value);
    obj.define_global_value("bind", &SYNTAX_BIND.value);
    obj.define_global_value("match", &SYNTAX_MATCH.value);
    obj.define_global_value("and", &SYNTAX_AND.value);
    obj.define_global_value("or", &SYNTAX_OR.value);
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