use crate::mm::{GCAllocationStruct};
use crate::eval::{eval, self};
use crate::value::symbol::Symbol;
use crate::value::list::List;
use crate::{value::*, let_listbuilder, with_cap, let_cap, new_cap};
use crate::value::list;
use crate::ptr::*;
use crate::context::Context;
use std::fmt::{Debug, Display};
use std::panic;
use once_cell::sync::Lazy;

pub mod r#match;

pub struct Syntax {
    name: String,
    require: usize,
    optional: usize,
    has_rest: bool,
    body: fn(&RPtr<list::List>, &mut Object) -> FPtr<Value>,
}

static SYNTAX_TYPEINFO: TypeInfo = new_typeinfo!(
    Syntax,
    "Syntax",
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

    fn clone_inner(this: &RPtr<Self>, _obj: &mut Object) -> FPtr<Self> {
        //Syntaxのインスタンスはヒープ上に作られることがないため、自分自身を返す
        this.clone().into_fptr()
    }
}

impl Syntax {

    pub fn new<T: Into<String>>(name: T, require: usize, optional: usize, has_rest: bool
        , body: fn(&RPtr<list::List>, &mut Object) -> FPtr<Value>
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

    pub fn check_arguments<T>(&self, args: &T) -> bool
    where
        T: AsReachable<list::List>
    {
        let args = args.as_reachable();
        let count = args.as_ref().count();
        if count < self.require {
            false
        } else if self.has_rest == false && count > self.require + self.optional {
            false
        } else {
            true
        }
    }

    pub fn apply<T>(&self, args: &T, obj: &mut Object) -> FPtr<Value>
    where
        T: AsReachable<list::List>
    {
        (self.body)(args.as_reachable(), obj)
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

fn syntax_if(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    let pred = eval(args.as_ref().head_ref(), obj);
    let pred = is_true(pred.as_ref());

    let args = args.as_ref().tail_ref();
    if pred {
        eval(args.as_ref().head_ref(), obj)

    } else {
        let args = args.as_ref().tail_ref();
        if args.as_ref().is_nil() {
            bool::Bool::false_().into_value().into_fptr()
        } else {
            eval(args.as_ref().head_ref(), obj)
        }
    }
}

fn syntax_begin(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    do_begin(args, obj)
}

pub(crate) fn do_begin<T>(body: &T, obj: &mut Object) -> FPtr<Value>
where
    T: AsReachable<list::List>,
{
    let body = body.as_reachable();

    let mut last: Option<FPtr<Value>> = None;
    for sexp in body.as_ref().iter() {
        let e = eval::eval(sexp, obj);
        last = Some(e);
    }

    if let Some(last) = last {
        last
    } else {
        tuple::Tuple::unit().into_value().into_fptr()
    }
}

fn syntax_cond(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    for (sexp, info) in args.as_ref().iter_with_info() {
        if let Some(clause) = sexp.try_cast::<list::List>() {
            let test = clause.as_ref().head_ref();

            //最後の節のTESTがシンボルのelseの場合、無条件でbody部分を評価します
            if let Some(else_) = test.try_cast::<symbol::Symbol>() {
                if else_.as_ref().as_ref() == "else" && info.is_tail {
                    return do_begin(clause.as_ref().tail_ref(), obj);
                }
            }

            //TEST式を評価
            let result = eval::eval(test, obj);
            //TESTの結果がtrueなら続く式を実行して結果を返す
            if is_true(result.as_ref()) {
                return do_begin(clause.as_ref().tail_ref(), obj);
            }

        } else {
            panic!("cond clause require list. but got {:?}", sexp.as_ref());
        }
    }

    bool::Bool::false_().into_value().into_fptr()
}

fn syntax_def(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    let symbol = args.as_ref().head_ref();
    if let Some(symbol) = symbol.try_cast::<Symbol>() {
        let value = args.as_ref().tail_ref().as_ref().head_ref();
        let value = eval::eval(value, obj);
        with_cap!(value, value.clone(), obj, {
            obj.context().add_to_current_frame(symbol, &value);
        });

        value
    } else {
        panic!("def variable require symbol. But got {}", symbol.as_ref());
    }
}

fn syntax_def_recv(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    if obj.context().is_toplevel() {
        let pat = args.as_ref().head_ref();
        let body = args.as_ref().tail_ref();

        //現在のコンテキストにレシーバーを追加する
        obj.add_receiver(pat, body);

        //どの値を返すべき？
        bool::Bool::true_().into_value().into_fptr()
    } else {
        panic!("def-recv allow only top-level context")
    }
}

pub(crate) fn syntax_fun(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {

    //引数指定の内容を解析
    let_listbuilder!(builder_param, obj);
    let params = args.as_ref().head_ref();
    if let Some(params) = params.try_cast::<list::List>() {
        //TODO :optionalと:rest引数の対応
        for p in params.as_ref().iter() {
            match p.try_cast::<symbol::Symbol>() {
                Some(sym) => {
                    builder_param.append(sym.cast_value(), obj);
                }
                None => {
                    panic!("parameter require symbol. But got {:?}", p.as_ref())
                }
            }
        }
    } else {
        panic!("The fun paramters require list. But got {:?}", params.as_ref())
    }

    let (list_ptr, size) = builder_param.get_with_size();
    let params_ptr = with_cap!(list, list_ptr, obj, {
        array::Array::from_list(&list, Some(size), obj)
    });
    let_cap!(params, params_ptr, obj);
    let body = args.as_ref().tail_ref();

    closure::Closure::alloc(&params, body, obj).into_value()
}

fn syntax_let(args: &RPtr<List>, obj: &mut Object) -> FPtr<Value> {

    let mut symbol_list = Vec::<&RPtr<Symbol>>::new();
    let_listbuilder!(val_list, obj);
    //局所変数指定の内容を解析
    let binders = args.as_ref().head_ref();
    if let Some(binders) = binders.try_cast::<List>() {
        for bind in binders.as_ref().iter() {
            if let Some(bind) = bind.try_cast::<List>() {
                if bind.as_ref().count() != 2 {
                    panic!("The let bind part require 2 length list. But got {:?}", bind.as_ref())
                }
                let symbol = bind.as_ref().head_ref();
                if let Some(symbol) = symbol.try_cast::<symbol::Symbol>() {
                    symbol_list.push(symbol);

                    let val = bind.as_ref().tail_ref().as_ref().head_ref();
                    let val = eval::eval(val, obj);
                    with_cap!(val, val, obj, {
                        val_list.append(&val, obj);
                    });

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

    with_cap!(val_list, val_list.get(), obj, {
        //ローカルフレームを構築
        let a = val_list.as_reachable().as_ref();
        let frame: Vec::<(&RPtr<Symbol>, &RPtr<Value>)> = symbol_list.iter().zip(a.iter())
            .map(|(s, v)| (*s, v))
            .collect();

        ////ローカルフレームを環境にプッシュ
        obj.context().push_local_frame(&frame);
    });

    //Closure本体を実行
    let result = syntax::do_begin(args.as_ref().tail_ref(), obj);

    //ローカルフレームを環境からポップ
    obj.context().pop_local_frame();

    result
}

fn syntax_quote(args: &RPtr<list::List>, _obj: &mut Object) -> FPtr<Value> {
    let sexp = args.as_ref().head_ref();
    sexp.clone().into_fptr()
}

fn syntax_unquote(_args: &RPtr<list::List>, _obj: &mut Object) -> FPtr<Value> {
    unimplemented!()
}

fn syntax_bind(_args: &RPtr<list::List>, _obj: &mut Object) -> FPtr<Value> {
    unimplemented!()
}

fn syntax_and(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    let mut last: Option<FPtr<Value>> = None;
    for sexp in args.as_ref().iter() {
        let result = eval::eval(sexp, obj);
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

fn syntax_or(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    for sexp in args.as_ref().iter() {
        let result = eval::eval(sexp, obj);
        if is_true(result.as_ref()) {
            return result;
        }
    }

    bool::Bool::false_().into_fptr().into_value()
}

fn syntax_match(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {
    //パターン部が一つもなければUnitを返す
    if args.as_ref().is_nil() {
        tuple::Tuple::unit().into_value().into_fptr()
    } else {
        let match_expr = r#match::translate(args, obj).into_value();
        with_cap!(expr, match_expr, obj, {
            eval::eval(&expr, obj)
        })
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
    ctx.define_value("if", &RPtr::new(&SYNTAX_IF.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("begin", &RPtr::new(&SYNTAX_BEGIN.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("cond", &RPtr::new(&SYNTAX_COND.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("def", &RPtr::new(&SYNTAX_DEF.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("def-recv", &RPtr::new(&SYNTAX_DEF_RECV.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("fun", &RPtr::new(&SYNTAX_FUN.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("let", &RPtr::new(&SYNTAX_LET.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("quote", &RPtr::new(&SYNTAX_QUOTE.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("unquote", &RPtr::new(&SYNTAX_UNQUOTE.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("bind", &RPtr::new(&SYNTAX_BIND.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("match", &RPtr::new(&SYNTAX_MATCH.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("and", &RPtr::new(&SYNTAX_AND.value as *const Syntax as *mut Syntax).into_value());
    ctx.define_value("or", &RPtr::new(&SYNTAX_OR.value as *const Syntax as *mut Syntax).into_value());
}

pub mod literal {
    use crate::ptr::RPtr;
    use super::*;

    pub fn quote() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_QUOTE.value as *const Syntax as *mut Syntax)
    }

    pub fn unquote() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_UNQUOTE.value as *const Syntax as *mut Syntax)
    }

    pub fn bind() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_BIND.value as *const Syntax as *mut Syntax)
    }

    pub fn fun() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_FUN.value as *const Syntax as *mut Syntax)
    }

    pub fn let_() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_LET.value as *const Syntax as *mut Syntax)
    }

    pub fn if_() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_IF.value as *const Syntax as *mut Syntax)
    }

    pub fn begin() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_BEGIN.value as *const Syntax as *mut Syntax)
    }

    pub fn cond() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_COND.value as *const Syntax as *mut Syntax)
    }
}