use crate::value::list::{List, ListBuilder};
use crate::value::syntax::Syntax;
use crate::value::symbol::Symbol;
use crate::{ptr::*, cap_append};
use crate::value::{self, *};

#[derive(PartialEq)]
pub struct MatchFail { }

static MATCHFAIL_TYPEINFO : TypeInfo = new_typeinfo!(
    MatchFail,
    "MatchFail",
    0, None, //Immidiate Valueだけなのでサイズは常に0
    MatchFail::eq,
    MatchFail::clone_inner,
    std::fmt::Display::fmt,
    MatchFail::is_type,
    None,
    None,
    None,
);

impl NaviType for MatchFail {
    fn typeinfo() -> NonNullConst<TypeInfo> {
        NonNullConst::new_unchecked(&MATCHFAIL_TYPEINFO as *const TypeInfo)
    }

    fn clone_inner(&self, _obj: &mut Object) -> FPtr<Self> {
        //MatchFail型の値は常にImmidiate Valueなのでそのまま返す
        FPtr::new(self)
    }
}

impl MatchFail {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&MATCHFAIL_TYPEINFO, other_typeinfo)
    }

    pub fn fail() -> Reachable<MatchFail> {
        Reachable::<MatchFail>::new_immidiate(IMMIDATE_MATCHFAIL)
    }

    pub fn is_fail(val: &Value) -> bool {
        std::ptr::eq(val as *const Value, IMMIDATE_MATCHFAIL as *const Value)
    }
}

impl std::fmt::Display for MatchFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FAIL")
    }
}

impl std::fmt::Debug for MatchFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FAIL")
    }
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum PatKind {
    List,
    Array,
    Tuple,
    Literal,
    Unquote,
    Bind,
    Empty,
}

type MatchClause = (Vec<Reachable<Value>>, Reachable<List>);

pub fn translate(args: &Reachable<List>, obj: &mut Object) -> FPtr<List> {
    //パターンリストを扱いやすいようにそれぞれVecに分解
    let patterns = {
        let mut vec = Vec::<MatchClause>::new();

        //matchの各節のパターン部分と実行式を分解する。
        for pat in args.as_ref().tail().reach(obj).iter(obj) {
            if let Some(pat) = pat.try_cast::<List>() {
                if pat.as_ref().len_more_than(2) == false {
                    panic!("match clause require more than 2 length list. but got {:?}", pat)
                }
                //パターン部分と対応する実行式を分解する
                let mut pat_vec: Vec<Reachable<Value>> = Vec::new();
                pat_vec.push(pat.as_ref().head().reach(obj));

                let body = pat.as_ref().tail().reach(obj);

                vec.push((pat_vec, body));

            } else {
                panic!("match clause require list. but got {:?}", pat.as_ref())
            }

        }
        vec
    };


    let mut builder_local = ListBuilder::new(obj);
    builder_local.append(syntax::literal::local().cast_value(), obj);

    //generate unique symbol
    let expr_tmp_symbol = symbol::Symbol::gensym("e", obj).into_value().reach(obj);
    //(let e target_exp)
    let let_ = {
        let mut builder_let = ListBuilder::new(obj);
        builder_let.append(syntax::literal::def().cast_value(), obj);
        builder_let.append(&expr_tmp_symbol, obj);
        cap_append!(builder_let, args.as_ref().head(), obj); //マッチ対象の値を一時変数に代入する

        builder_let.get().into_value()
    };
    //(local (let e e target_exp))
    cap_append!(builder_local, let_, obj);

    let mut cond_vec: Vec<Reachable<Value>> = Vec::new();
    cond_vec.push(expr_tmp_symbol);

    let body = translate_inner(cond_vec, patterns, obj);
    cap_append!(builder_local, body, obj);

    //最後にmatchに失敗した値を捕まえてfalseを返すfail-catchを追加
    let mut builder_catch = ListBuilder::new(obj);
    builder_catch.append(literal::fail_catch().cast_value(), obj);
    cap_append!(builder_catch, builder_local.get().into_value(), obj);
    builder_catch.append(bool::Bool::false_().cast_value(), obj);

    builder_catch.get()
}

fn translate_inner(exprs: Vec<Reachable<Value>>, patterns: Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    fn trans(kind: PatKind, exprs: &Vec<Reachable<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
        match kind {
            PatKind::List => {
                translate_container_match(exprs, patterns
                    , &list::literal::is_list()
                    , &list::literal::list_len()
                    , &list::literal::list_ref()
                    , list::List::count
                    , list::List::get
                    , obj)
            }
            PatKind::Array => {
                translate_container_match(exprs, patterns
                    , &array::literal::is_array()
                    , &array::literal::array_len()
                    , &array::literal::array_ref()
                    , array::Array::len
                    , array::Array::get
                    , obj)
            }
            PatKind::Tuple => {
                translate_container_match(exprs, patterns
                    , &tuple::literal::is_tuple()
                    , &tuple::literal::tuple_len()
                    , &tuple::literal::tuple_ref()
                    , tuple::Tuple::len
                    , tuple::Tuple::get
                    , obj)
            }
            PatKind::Literal => {
                translate_literal(exprs, patterns, obj)
            }
            PatKind::Unquote => {
                unimplemented!()
            }
            PatKind::Bind => {
                translate_bind(exprs, patterns, obj)
            }
            PatKind::Empty => {
                translate_empty(exprs, patterns, obj)
            }
        }
    }

    let mut grouping = pattern_grouping(patterns);
    if grouping.len() == 1 {
        let (kind, patterns) = grouping.pop().unwrap();
        trans(kind, &exprs, &patterns, obj)

    } else {
        let mut builder = ListBuilder::new(obj);
        builder.append(literal::fail_catch().cast_value(), obj);

        for (kind, patterns) in grouping {
            let exp = trans(kind, &exprs, &patterns, obj);
            cap_append!(builder, exp, obj);
        }
        builder.get().into_value()
    }

}

fn pattern_grouping(patterns: Vec<MatchClause>) -> Vec<(PatKind, Vec<MatchClause>)> {
    //パターンの種類ごとの節を保持する配列
    //パターン種類が現れた順序を保った配列になっている。
    let mut group : Vec<(PatKind, Vec<MatchClause>)> = Vec::new();

    for (pat, body) in patterns.into_iter() {
        //パターンの種類の判別
        let kind = if pat.is_empty() {
            PatKind::Empty

        } else {
            let tf = value::get_typeinfo(pat.last().unwrap().as_ref()).as_ptr();
            if std::ptr::eq(tf, list::List::typeinfo().as_ptr()) {
                let list =  unsafe { pat.last().unwrap().cast_unchecked::<List>() };
                //長さがちょうど２のリストで
                if list.as_ref().len_exactly(2) {
                    let head = list.as_ref().head();

                    if head.as_ref().eq(syntax::literal::bind().cast_value().as_ref()) {
                        //(bind x)なら
                        PatKind::Bind

                    } else if head.as_ref().eq(syntax::literal::unquote().cast_value().as_ref()) {
                        //(unqote x)なら
                        PatKind::Unquote
                    } else {
                        PatKind::List
                    }
                } else {
                    PatKind::List
                }

            } else if std::ptr::eq(tf, array::Array::<Value>::typeinfo().as_ptr()) {
                PatKind::Array
            } else if std::ptr::eq(tf, tuple::Tuple::typeinfo().as_ptr()) {
                PatKind::Tuple
            } else {
                PatKind::Literal
            }
        };

        //同じパターン種類ごとのMatchClauseにまとめる
        if let Some((_, clauses)) = group.iter_mut().find(|(k, _)| *k == kind) {
            clauses.push((pat, body));

        } else {
            let mut clauses = Vec::<MatchClause>::new();
            clauses.push((pat, body));

            group.push((kind, clauses));
        }
    }

    group
}

fn translate_container_match<T: NaviType>(exprs: &Vec<Reachable<Value>>, patterns: &Vec<MatchClause>
    , is_type_func: &Reachable<Func>, len_func: &Reachable<Func>, ref_func: &Reachable<Func>
    , pattern_len_func: fn(&T) -> usize, pattern_ref_func: fn(&T, usize) -> FPtr<Value>
    , obj: &mut Object) -> FPtr<Value> {

    //長さごとにパターンを集めたVec
    let mut group: Vec<(usize, Vec<MatchClause>)>  = Vec::new();

    //同じサイズのコンテナごとにグルーピング
    for (pat, body) in patterns.clone().into_iter() {
        let container_pat = unsafe { pat.last().unwrap().cast_unchecked::<T>() };
        let len = pattern_len_func(container_pat.as_ref());

        //ReachableはCloneトレイトを実装していないので手動でクローンする
        let pat = clone_veccap(pat, obj);
        let body = body.clone(obj);
        if let Some((_, clauses)) = group.iter_mut().find(|(l, _)| *l == len) {
            clauses.push((pat, body));

        } else {
            let mut clauses: Vec<MatchClause> = Vec::new();
            clauses.push((pat, body));

            group.push((len, clauses));
        }
    }

    let target_expr = exprs.last().unwrap();

    let mut builder_if = ListBuilder::new(obj);
    builder_if.append(syntax::literal::if_().cast_value(), obj);

    //predicate
    cap_append!(builder_if, cons_list2(is_type_func.cast_value(), target_expr, obj), obj);

    // true clause
    let true_clause = {
        let mut builder_local = ListBuilder::new(obj);
        //(local)
        builder_local.append(syntax::literal::local().cast_value(), obj);

        //generate unique symbol
        let len_symbol = symbol::Symbol::gensym("len", obj).into_value().reach(obj);
        let let_ = {
            //(let len (???-len target))
            let mut builder_let = ListBuilder::new(obj);
            builder_let.append(syntax::literal::def().cast_value(), obj);
            builder_let.append(&len_symbol, obj);
            cap_append!(builder_let, cons_list2(len_func.cast_value(), target_expr, obj), obj);

            builder_let.get().into_value()
        };
        // (local (let len (???-len target)))
        cap_append!(builder_local, let_, obj);

        //(cond ...)
        let cond = {
            let mut builder_cond = ListBuilder::new(obj);
            //(cond)
            builder_cond.append(syntax::literal::cond().cast_value(), obj);

            for (container_len, mut clauses) in group.into_iter() {
                let mut builder_cond_clause = ListBuilder::new(obj);

                //(equal? ???-len len)
                let equal = {
                    let v1 = number::Integer::alloc(container_len as i64, obj).into_value().reach(obj);
                    cons_list3(value::literal::equal().cast_value(), &v1, &len_symbol, obj)
                };
                //((equal? ???-len len))
                cap_append!(builder_cond_clause, equal, obj);

                //(local)
                let mut builder_inner_local = ListBuilder::new(obj);
                builder_inner_local.append(syntax::literal::local().cast_value(), obj);

                let mut exprs:Vec<Reachable<Value>> = clone_veccap(&exprs[0..exprs.len()-1], obj);

                //後々の処理の都合上、降順でコンテナ内の値を取得する
                //(local ... (let v1 (???-ref container 1)) (let v0 (???-ref container 0)))
                for index in (0..container_len).rev() {
                    let mut builder_binder = ListBuilder::new(obj);
                    //(let)
                    builder_binder.append(syntax::literal::def().cast_value(), obj);
                    //(let v0)
                    let symbol = symbol::Symbol::gensym(String::from("v") + &index.to_string() , obj).into_value().reach(obj);
                    builder_binder.append(&symbol, obj);
                    exprs.push(symbol);

                    //(???-ref container index)
                    let container_ref = cons_list3(ref_func.cast_value(), target_expr
                        , &number::Integer::alloc(index as i64, obj).into_value().reach(obj)
                        , obj);

                    //(let v0 (???-ref container index))
                    cap_append!(builder_binder, container_ref, obj);

                    //(local (let v0 (???-ref container index)) ...)
                    cap_append!(builder_inner_local, builder_binder.get().into_value(), obj);
                }

                //各Clauseの先頭要素にあるコンテナを展開して、Pattern配列に追加する
                for (pat, _) in clauses.iter_mut() {
                    let container = pat.pop().unwrap();
                    let container = unsafe { container.cast_unchecked::<T>() };
                    for index in (0..container_len).rev() {
                        pat.push(pattern_ref_func(container.as_ref(), index).reach(obj));
                    }
                }

                //(local ... (let v1 (???-ref container 1)) (let v0 (???-ref container 0))
                //  inner matcher ...)
                let matcher= translate_inner(exprs, clauses, obj);
                cap_append!(builder_inner_local, matcher, obj);

                //((equal? container-len len)
                // (local ........))
                cap_append!(builder_cond_clause, builder_inner_local.get().into_value(), obj);

                //(cond
                //  ((equal? container-len len)
                //      (local ........)))
                cap_append!(builder_cond, builder_cond_clause.get().into_value(), obj);
            }
            cap_append!(builder_cond, cons_cond_fail(obj), obj);

            builder_cond.get()
        };

        //(local (let len (container-len target))
        //  (cond ...))
        cap_append!(builder_local, cond.into_value(), obj);
        builder_local.get()
    };

    cap_append!(builder_if, true_clause.into_value(), obj);

    //マッチ失敗用の値をfalse節に追加
    builder_if.append(MatchFail::fail().cast_value(), obj);

    builder_if.get().into_value()
}

fn translate_literal(exprs: &Vec<Reachable<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let mut group = Vec::<(Reachable<Value>, Vec<MatchClause>)>::new();

    //同じリテラルごとにグルーピングを行う
    for (pat, body) in patterns.iter() {
        let mut pat: Vec<Reachable<Value>> = pat.iter().map(|cap| cap.clone(obj)).collect();
        let literal_pat = pat.pop().unwrap();

        //ReachableはCloneを実装していないため自動クローンしない。
        //値のMoveが必要なので新しいReachableを作成する
        let body = FPtr::new(body.as_ref()).reach(obj);

        if let Some((_, clauses)) = group.iter_mut().find(|(v, _)| v.as_ref() == literal_pat.as_ref()) {
            clauses.push((pat, body));
        } else {
            let mut clauses = Vec::<MatchClause>::new();
            clauses.push((pat, body));
            group.push((literal_pat, clauses));
        }
    }

    let mut exprs = clone_veccap(exprs, obj);
    let target = exprs.pop().unwrap();

    let mut builder_cond = ListBuilder::new(obj);
    builder_cond.append(syntax::literal::cond().cast_value(), obj);

    for (literal, patterns) in group.into_iter() {
        let mut builder_cond_clause = ListBuilder::new(obj);

        //(equal? target literal)
        let equal = cons_list3(value::literal::equal().cast_value()
            , &target
            , &literal
            , obj);
        //((equal? target literal))
        cap_append!(builder_cond_clause, equal, obj);

        //ReachableはCloneを実装していないため自動クローンしない。
        //値のMoveが必要なので新しいReachableを作成する
        let exprs = exprs.iter()
            .map(|cap| FPtr::new(cap.as_ref()).reach(obj))
            .collect()
            ;
        //((equal? target literal) next-match)
        let matcher= translate_inner(exprs, patterns, obj);
        cap_append!(builder_cond_clause, matcher, obj);

        //(cond ((equal? target literal) next-match))
        let cond_clause = builder_cond_clause.get().into_value();
        cap_append!(builder_cond, cond_clause, obj);
    }

    //最後にマッチ失敗の節を追加
    cap_append!(builder_cond, cons_cond_fail(obj), obj);

    builder_cond.get().into_value()
}

fn translate_bind(exprs: &Vec<Reachable<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let mut exprs: Vec<Reachable<Value>> = exprs.iter().map(|cap| cap.clone(obj)).collect();
    let target = exprs.pop().unwrap();

    let mut builder_catch = ListBuilder::new(obj);
    builder_catch.append(literal::fail_catch().cast_value(), obj);

    //TODO 最適化のために同じシンボルごとにグルーピングを行いたい
    for (pattern, body) in patterns.iter() {
        //ReachableはCloneを実装していないため自動クローンしない。
        //値のMoveが必要なので新しいReachableを作成する
        let mut pattern = clone_veccap(pattern, obj);
        let body = body.clone(obj);

        //exprsのクローンを作成する
        let exprs = exprs.iter().map(|cap| cap.clone(obj)).collect();

        let bind = pattern.pop().unwrap();
        //必ず(bind ???)というような形式になっているのでリストに変換
        let bind = unsafe { bind.cast_unchecked::<List>() };
        let val = bind.as_ref().tail().as_ref().head();

        if let Some(symbol) = val.try_cast::<Symbol>() {
            //束縛対象がアンダースコアなら束縛を行わない
            if symbol.as_ref().as_ref() == "_" {
                let mut patterns: Vec<MatchClause> = Vec::new();
                patterns.push((pattern, body));

                let matcher= translate_inner(exprs, patterns, obj);
                cap_append!(builder_catch, matcher, obj);

            } else {
                let mut builder_local = ListBuilder::new(obj);
                builder_local.append(syntax::literal::local().cast_value(), obj);

                //(let x target)
                let let_ = {
                    let mut builder_let = ListBuilder::new(obj);
                    builder_let.append(syntax::literal::def().cast_value(), obj);
                    builder_let.append(&val.reach(obj), obj);
                    builder_let.append(&target, obj);
                    builder_let.get().into_value()
                };

                //(local (let x target))
                cap_append!(builder_local, let_, obj);

                let mut patterns: Vec<MatchClause> = Vec::new();
                patterns.push((pattern, body));
                let matcher= translate_inner(exprs, patterns, obj);
                cap_append!(builder_local, matcher, obj);

                cap_append!(builder_catch, builder_local.get().into_value(), obj);
            }
        } else {
            panic!("bind variable required symbol. but got {}", val.as_ref())
        }
    }

    builder_catch.get().into_value()
}

fn translate_empty(_exprs: &Vec<Reachable<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let (_, body) = patterns.first().unwrap();

    let body = body.clone(obj);
    list::List::alloc(syntax::literal::begin().cast_value(), &body, obj).into_value()
}

fn clone_veccap(vec: &[Reachable<Value>], obj: &mut Object) -> Vec<Reachable<Value>> {
    vec.iter().map(|cap| cap.clone(obj)).collect()
}

fn cons_list2(v1: &Reachable<Value>, v2: &Reachable<Value>, obj: &mut Object) -> FPtr<Value> {
    let mut builder = ListBuilder::new(obj);
    builder.append(v1, obj);
    builder.append(v2, obj);

    builder.get().into_value()
}

fn cons_list3(v1: &Reachable<Value>, v2: &Reachable<Value>, v3: &Reachable<Value>, obj: &mut Object) -> FPtr<Value> {
    let mut builder = ListBuilder::new(obj);
    builder.append(v1, obj);
    builder.append(v2, obj);
    builder.append(v3, obj);

    builder.get().into_value()
}

fn cons_cond_fail(obj: &mut Object) -> FPtr<Value> {
    let mut builder = ListBuilder::new(obj);
    //TODO よく使うシンボルはsatic領域に確保してアロケーションを避ける
    cap_append!(builder, symbol::Symbol::alloc("else", obj).into_value(), obj);
    builder.append(MatchFail::fail().cast_value(), obj);
    builder.get().into_value()
}

fn syntax_fail_catch(args: &Reachable<list::List>, obj: &mut Object) -> FPtr<Value> {

    for sexp in args.iter(obj) {
        let e = crate::eval::eval(&sexp.reach(obj), obj);
        if MatchFail::is_fail(e.as_ref()) == false {
            return e;
        }
    }

    MatchFail::fail().into_fptr().into_value()
}

static SYNTAX_FAIL_CATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(syntax::Syntax::new("fail-catch", 0, 0, true, syntax_fail_catch, crate::compile::syntax_fail_catch))
});

pub mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn fail_catch() -> Reachable<Syntax> {
        Reachable::new_static(&SYNTAX_FAIL_CATCH.value)
    }
}