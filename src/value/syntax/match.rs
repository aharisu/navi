use crate::compile::{SyntaxException, self};
use crate::value::list::{List, ListBuilder};
use crate::value::symbol::Symbol;
use crate::ptr::*;
use crate::err::{self, NResult};
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
    None,
    None,
    None,
    None,
    None,
);

impl NaviType for MatchFail {
    fn typeinfo() -> &'static TypeInfo {
        &MATCHFAIL_TYPEINFO
    }

    fn clone_inner(&self, _allocator: &mut AnyAllocator) -> NResult<Self, OutOfMemory> {
        //MatchFail型の値は常にImmidiate Valueなのでそのまま返す
        Ok(Ref::new(self))
    }
}

impl MatchFail {

    pub fn fail() -> Reachable<MatchFail> {
        Reachable::<MatchFail>::new_immidiate(IMMIDATE_MATCHFAIL)
    }

    pub fn is_fail(val: &Any) -> bool {
        std::ptr::eq(val as *const Any, IMMIDATE_MATCHFAIL as *const Any)
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

type MatchClause = (Vec<Reachable<Any>>, Reachable<List>);

pub fn translate(args: &Reachable<List>, obj: &mut Object) -> NResult<List, SyntaxException> {
    //パターンリストを扱いやすいようにそれぞれVecに分解
    let patterns = {
        let mut vec = Vec::<MatchClause>::new();

        //matchの各節のパターン部分と実行式を分解する。
        for pat in args.as_ref().tail().reach(obj).iter(obj) {
            if let Some(pat) = pat.try_cast::<List>() {
                if pat.as_ref().len_more_than(2) == false {
                    return Err(err::MalformedFormat::new( Some(pat.cast_value().clone()), "match clause require more than 2 length list.").into());
                }
                //パターン部分と対応する実行式を分解する
                let mut pat_vec: Vec<Reachable<Any>> = Vec::new();
                pat_vec.push(pat.as_ref().head().reach(obj));

                let body = pat.as_ref().tail().reach(obj);

                vec.push((pat_vec, body));

            } else {
                return Err(err::TypeMismatch::new(pat, list::List::typeinfo()).into());
            }

        }
        vec
    };


    let mut builder_local = ListBuilder::new(obj);
    builder_local.push(compile::literal::local().cast_value(), obj)?;

    //generate unique symbol
    let expr_tmp_symbol = symbol::Symbol::gensym("e", obj)?.into_value().reach(obj);
    //(let e target_exp)
    let let_ = {
        let mut builder_let = ListBuilder::new(obj);
        builder_let.push(compile::literal::let_().cast_value(), obj)?;
        builder_let.push(&expr_tmp_symbol, obj)?;
        builder_let.push(&args.as_ref().head().reach(obj), obj)?; //マッチ対象の値を一時変数に代入する

        builder_let.get().into_value()
    };
    //(local (let e e target_exp))
    builder_local.push(&let_.reach(obj), obj)?;

    let mut cond_vec: Vec<Reachable<Any>> = Vec::new();
    cond_vec.push(expr_tmp_symbol);

    let body = translate_inner(cond_vec, patterns, obj)?;
    builder_local.push(&body.reach(obj), obj)?;

    //最後にmatchに失敗した値を捕まえてfalseを返すfail-catchを追加
    let mut builder_catch = ListBuilder::new(obj);
    builder_catch.push(compile::literal::fail_catch().cast_value(), obj)?;
    builder_catch.push(&builder_local.get().into_value().reach(obj), obj)?;
    builder_catch.push(bool::Bool::false_().cast_value(), obj)?;

    Ok(builder_catch.get())
}

fn translate_inner(exprs: Vec<Reachable<Any>>, patterns: Vec<MatchClause>, obj: &mut Object) -> NResult<Any, SyntaxException> {
    fn trans(kind: PatKind, exprs: &Vec<Reachable<Any>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> NResult<Any, SyntaxException> {
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
        builder.push(compile::literal::fail_catch().cast_value(), obj)?;

        for (kind, patterns) in grouping {
            let exp = trans(kind, &exprs, &patterns, obj)?;
            builder.push(&exp.reach(obj), obj)?;
        }
        Ok(builder.get().into_value())
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
            let tf = value::get_typeinfo(pat.last().unwrap().as_ref());
            if tf == list::List::typeinfo() {
                let list =  unsafe { pat.last().unwrap().cast_unchecked::<List>() };
                //長さがちょうど２のリストで
                if list.as_ref().len_exactly(2) {
                    let head = list.as_ref().head();

                    if head.as_ref().eq(compile::literal::bind().cast_value().as_ref()) {
                        //(bind x)なら
                        PatKind::Bind

                    } else if head.as_ref().eq(compile::literal::unquote().cast_value().as_ref()) {
                        //(unqote x)なら
                        PatKind::Unquote
                    } else {
                        PatKind::List
                    }
                } else {
                    PatKind::List
                }

            } else if tf == array::Array::<Any>::typeinfo() {
                PatKind::Array
            } else if tf == tuple::Tuple::typeinfo() {
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

fn translate_container_match<T: NaviType>(exprs: &Vec<Reachable<Any>>, patterns: &Vec<MatchClause>
    , is_type_func: &Reachable<Func>, len_func: &Reachable<Func>, ref_func: &Reachable<Func>
    , pattern_len_func: fn(&T) -> usize, pattern_ref_func: fn(&T, usize) -> Ref<Any>
    , obj: &mut Object) -> NResult<Any, SyntaxException> {

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
    builder_if.push(compile::literal::if_().cast_value(), obj)?;

    //predicate
    builder_if.push(&cons_list2(is_type_func.cast_value(), target_expr, obj)?.reach(obj), obj)?;

    // true clause
    let true_clause = {
        let mut builder_local = ListBuilder::new(obj);
        //(local)
        builder_local.push(compile::literal::local().cast_value(), obj)?;

        //generate unique symbol
        let len_symbol = symbol::Symbol::gensym("len", obj)?.into_value().reach(obj);
        let let_ = {
            //(let len (???-len target))
            let mut builder_let = ListBuilder::new(obj);
            builder_let.push(compile::literal::let_().cast_value(), obj)?;
            builder_let.push(&len_symbol, obj)?;
            builder_let.push(&cons_list2(len_func.cast_value(), target_expr, obj)?.reach(obj), obj)?;

            builder_let.get().into_value()
        };
        // (local (let len (???-len target)))
        builder_local.push(&let_.reach(obj), obj)?;

        //(cond ...)
        let cond = {
            let mut builder_cond = ListBuilder::new(obj);
            //(cond)
            builder_cond.push(compile::literal::cond().cast_value(), obj)?;

            for (container_len, mut clauses) in group.into_iter() {
                let mut builder_cond_clause = ListBuilder::new(obj);

                //(equal? ???-len len)
                let equal = {
                    let v1 = number::make_integer(container_len as i64, obj)?.reach(obj);
                    cons_list3(value::any::literal::equal().cast_value(), &v1, &len_symbol, obj)?
                };
                //((equal? ???-len len))
                builder_cond_clause.push(&equal.reach(obj), obj)?;

                //(local)
                let mut builder_inner_local = ListBuilder::new(obj);
                builder_inner_local.push(compile::literal::local().cast_value(), obj)?;

                let mut exprs:Vec<Reachable<Any>> = clone_veccap(&exprs[0..exprs.len()-1], obj);

                //後々の処理の都合上、降順でコンテナ内の値を取得する
                //(local ... (let v1 (???-ref container 1)) (let v0 (???-ref container 0)))
                for index in (0..container_len).rev() {
                    let mut builder_binder = ListBuilder::new(obj);
                    //(let)
                    builder_binder.push(compile::literal::let_().cast_value(), obj)?;
                    //(let v0)
                    let symbol = symbol::Symbol::gensym(String::from("v") + &index.to_string() , obj)?.into_value().reach(obj);
                    builder_binder.push(&symbol, obj)?;
                    exprs.push(symbol);

                    //(???-ref container index)
                    let container_ref = cons_list3(ref_func.cast_value(), target_expr
                        , &number::make_integer(index as i64, obj)?.reach(obj)
                        , obj)?;

                    //(let v0 (???-ref container index))
                    builder_binder.push(&container_ref.reach(obj), obj)?;

                    //(local (let v0 (???-ref container index)) ...)
                    builder_inner_local.push(&builder_binder.get().into_value().reach(obj), obj)?;
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
                let matcher= translate_inner(exprs, clauses, obj)?;
                builder_inner_local.push(&matcher.reach(obj), obj)?;

                //((equal? container-len len)
                // (local ........))
                builder_cond_clause.push(&builder_inner_local.get().into_value().reach(obj), obj)?;

                //(cond
                //  ((equal? container-len len)
                //      (local ........)))
                builder_cond.push(&builder_cond_clause.get().into_value().reach(obj), obj)?;
            }
            builder_cond.push(&cons_cond_fail(obj)?.reach(obj), obj)?;

            builder_cond.get()
        };

        //(local (let len (container-len target))
        //  (cond ...))
        builder_local.push(&cond.into_value().reach(obj), obj)?;
        builder_local.get()
    };

    builder_if.push(&true_clause.into_value().reach(obj), obj)?;

    //マッチ失敗用の値をfalse節に追加
    builder_if.push(MatchFail::fail().cast_value(), obj)?;

    Ok(builder_if.get().into_value())
}

fn translate_literal(exprs: &Vec<Reachable<Any>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> NResult<Any, SyntaxException> {
    let mut group = Vec::<(Reachable<Any>, Vec<MatchClause>)>::new();

    //同じリテラルごとにグルーピングを行う
    for (pat, body) in patterns.iter() {
        let mut pat: Vec<Reachable<Any>> = pat.iter().map(|cap| cap.clone(obj)).collect();
        let literal_pat = pat.pop().unwrap();

        //ReachableはCloneを実装していないため自動クローンしない。
        //値のMoveが必要なので新しいReachableを作成する
        let body = body.clone(obj);

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
    builder_cond.push(compile::literal::cond().cast_value(), obj)?;

    for (literal, patterns) in group.into_iter() {
        let mut builder_cond_clause = ListBuilder::new(obj);

        //(equal? target literal)
        let equal = cons_list3(value::any::literal::equal().cast_value()
            , &target
            , &literal
            , obj)?;
        //((equal? target literal))
        builder_cond_clause.push(&equal.reach(obj), obj)?;

        //ReachableはCloneを実装していないため自動クローンしない。
        //値のMoveが必要なので新しいReachableを作成する
        let exprs = exprs.iter()
            .map(|expr| expr.clone(obj))
            .collect()
            ;
        //((equal? target literal) next-match)
        let matcher= translate_inner(exprs, patterns, obj)?;
        builder_cond_clause.push(&matcher.reach(obj), obj)?;

        //(cond ((equal? target literal) next-match))
        let cond_clause = builder_cond_clause.get().into_value();
        builder_cond.push(&cond_clause.reach(obj), obj)?;
    }

    //最後にマッチ失敗の節を追加
    builder_cond.push(&cons_cond_fail(obj)?.reach(obj), obj)?;

    Ok(builder_cond.get().into_value())
}

fn translate_bind(exprs: &Vec<Reachable<Any>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> NResult<Any, SyntaxException> {
    let mut exprs: Vec<Reachable<Any>> = exprs.iter().map(|cap| cap.clone(obj)).collect();
    let target = exprs.pop().unwrap();

    let mut builder_catch = ListBuilder::new(obj);
    builder_catch.push(compile::literal::fail_catch().cast_value(), obj)?;

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

                let matcher= translate_inner(exprs, patterns, obj)?;
                builder_catch.push(&matcher.reach(obj), obj)?;

            } else {
                let mut builder_local = ListBuilder::new(obj);
                builder_local.push(compile::literal::local().cast_value(), obj)?;

                //(let x target)
                let let_ = {
                    let mut builder_let = ListBuilder::new(obj);
                    builder_let.push(compile::literal::let_().cast_value(), obj)?;
                    builder_let.push(&val.reach(obj), obj)?;
                    builder_let.push(&target, obj)?;
                    builder_let.get().into_value()
                };

                //(local (let x target))
                builder_local.push(&let_.reach(obj), obj)?;

                let mut patterns: Vec<MatchClause> = Vec::new();
                patterns.push((pattern, body));
                let matcher= translate_inner(exprs, patterns, obj)?;
                builder_local.push(&matcher.reach(obj), obj)?;

                builder_catch.push(&builder_local.get().into_value().reach(obj), obj)?;
            }
        } else {
            return Err(err::TypeMismatch::new(val, symbol::Symbol::typeinfo()).into());
        }
    }

    Ok(builder_catch.get().into_value())
}

fn translate_empty(_exprs: &Vec<Reachable<Any>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> NResult<Any, SyntaxException> {
    let (_, body) = patterns.first().unwrap();

    let body = body.clone(obj);
    list::List::alloc(compile::literal::begin().cast_value(), &body, obj)
        .map(Ref::into_value)
        .map_err(SyntaxException::from)
}

fn clone_veccap(vec: &[Reachable<Any>], obj: &mut Object) -> Vec<Reachable<Any>> {
    vec.iter().map(|cap| cap.clone(obj)).collect()
}

fn cons_list2(v1: &Reachable<Any>, v2: &Reachable<Any>, obj: &mut Object) -> NResult<Any, OutOfMemory> {
    let mut builder = ListBuilder::new(obj);
    builder.push(v1, obj)?;
    builder.push(v2, obj)?;

    Ok(builder.get().into_value())
}

fn cons_list3(v1: &Reachable<Any>, v2: &Reachable<Any>, v3: &Reachable<Any>, obj: &mut Object) -> NResult<Any, OutOfMemory> {
    let mut builder = ListBuilder::new(obj);
    builder.push(v1, obj)?;
    builder.push(v2, obj)?;
    builder.push(v3, obj)?;

    Ok(builder.get().into_value())
}

fn cons_cond_fail(obj: &mut Object) -> NResult<Any, OutOfMemory> {
    let mut builder = ListBuilder::new(obj);
    builder.push(&literal::else_symbol().into_value(), obj)?;
    builder.push(MatchFail::fail().cast_value(), obj)?;

    Ok(builder.get().into_value())
}

static SYMBOL_ELSE: Lazy<GCAllocationStruct<symbol::StaticSymbol>> = Lazy::new(|| {
    symbol::symbol_static("else")
});

pub mod literal {
    use crate::ptr::*;
    use super::*;

    pub fn else_symbol() -> Reachable<symbol::Symbol> {
        Reachable::new_static(SYMBOL_ELSE.value.as_ref())
    }
}