use crate::value::list::List;
use crate::value::syntax::Syntax;
use crate::value::symbol::Symbol;
use crate::ptr::*;
use crate::value::{self, *};
use crate::{let_listbuilder, with_cap, let_cap, new_cap};

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

    fn clone_inner(this: &RPtr<Self>, _obj: &mut Object) -> FPtr<Self> {
        //MatchFail型の値は常にImmidiate Valueなのでそのまま返す
        this.clone().into_fptr()
    }
}

impl MatchFail {

    fn is_type(other_typeinfo: &TypeInfo) -> bool {
        std::ptr::eq(&MATCHFAIL_TYPEINFO, other_typeinfo)
    }

    pub fn fail() -> RPtr<MatchFail> {
        RPtr::<MatchFail>::new_immidiate(IMMIDATE_MATCHFAIL)
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

type MatchClause<'a> = (Vec<&'a RPtr<Value>>, &'a RPtr<List>);

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

pub fn translate(args: &RPtr<List>, obj: &mut Object) -> FPtr<List> {
    //パターンリストを扱いやすいようにVecに分解
    let patterns = {
        let mut vec = Vec::<MatchClause>::new();
        for pat in args.as_ref().tail_ref().as_ref().iter() {
            if let Some(pat) = pat.try_cast::<List>() {
                if pat.as_ref().len_more_than(2) == false {
                    panic!("match clause require more than 2 length list. but got {:?}", pat)
                }
                let mut pat_vec = Vec::new();
                pat_vec.push(pat.as_ref().head_ref());
                vec.push((pat_vec, pat.as_ref().tail_ref()));

            } else {
                panic!("match clause require list. but got {:?}", pat)
            }
        }

        vec
    };


    let_listbuilder!(builder_let, obj);
    builder_let.append(&syntax::literal::let_().into_value(), obj);

    //generate unique symbol
    let_cap!(expr_tmp_symbol, symbol::Symbol::gensym("e", obj).into_value(), obj);
    let binders = {
        let bind = {
            let_listbuilder!(builder_bind, obj);
            builder_bind.append(&expr_tmp_symbol, obj);
            builder_bind.append(args.as_ref().head_ref(), obj); //マッチ対象の値を一時変数に代入する

            builder_bind.get().into_value()
        };
        with_cap!(bind, bind, obj, {
            list::List::alloc_tail(&bind, obj).into_value()
        })
    };
    with_cap!(binders, binders, obj, {
        builder_let.append(&binders, obj);
    });

    let mut cond_vec: Vec<&RPtr<Value>> = Vec::new();
    cond_vec.push(expr_tmp_symbol.as_reachable());

    let body = translate_inner(cond_vec, patterns, obj);
    with_cap!(body, body, obj, {
        builder_let.append(&body, obj);
    });

    //最後にmatchに失敗した値を捕まえてfalseを返すfail-catchを追加
    let_listbuilder!(builder_catch, obj);
    builder_catch.append(&literal::fail_catch().into_value(), obj);
    with_cap!(body, builder_let.get().into_value(), obj, {
        builder_catch.append(&body, obj);
    });
    builder_catch.append(&bool::Bool::false_().into_value(), obj);

    builder_catch.get()
}

fn translate_inner(exprs: Vec<&RPtr<Value>>, patterns: Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    fn trans(kind: PatKind, exprs: &Vec<&RPtr<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
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
        let_listbuilder!(builder, obj);
        builder.append(&literal::fail_catch().into_value(), obj);

        for (kind, patterns) in grouping {
            let exp = trans(kind, &exprs, &patterns, obj);
            with_cap!(exp, exp, obj, {
                builder.append(&exp, obj);
            });
        }
        builder.get().into_value()
    }

}

fn pattern_grouping(patterns: Vec<MatchClause>) -> Vec<(PatKind, Vec<MatchClause>)> {
    //パターンの種類ごとの節を保持する配列
    //パターン種類が現れた順序を保った配列になっている。
    let mut group : Vec<(PatKind, Vec<MatchClause>)> = Vec::new();

    for (pat, body) in patterns.iter() {
        //パターンの種類の判別
        let kind = if pat.is_empty() {
            PatKind::Empty

        } else {
            let tf = pat.last().unwrap().as_ref().get_typeinfo().as_ptr();
            if std::ptr::eq(tf, list::List::typeinfo().as_ptr()) {
                let list =  unsafe { pat.last().unwrap().cast_unchecked::<List>() };
                //長さがちょうど２のリストで
                if list.as_ref().len_exactly(2) {
                    let head = list.as_ref().head_ref();
                    //(bind x)なら
                    if head.as_ref().eq(syntax::literal::bind().into_value().as_ref()) {
                        PatKind::Bind

                    } else if head.as_ref().eq(syntax::literal::unquote().into_value().as_ref()) {
                        //(unqote x)なら
                        PatKind::Unquote
                    } else {
                        PatKind::List
                    }
                } else {
                    PatKind::List
                }

            } else if std::ptr::eq(tf, array::Array::typeinfo().as_ptr()) {
                PatKind::Array
            } else if std::ptr::eq(tf, tuple::Tuple::typeinfo().as_ptr()) {
                PatKind::Tuple
            } else {
                PatKind::Literal
            }
        };

        //同じパターン種類ごとのMatchClauseにまとめる
        if let Some((_, clauses)) = group.iter_mut().find(|(k, _)| *k == kind) {
            clauses.push((pat.clone(), body));
        } else {
            let mut clauses = Vec::<MatchClause>::new();
            clauses.push((pat.clone(), body));

            group.push((kind, clauses));
        }
    }

    group
}

fn translate_container_match<T: NaviType>(exprs: &Vec<&RPtr<Value>>, patterns: &Vec<MatchClause>
    , is_type_func: &RPtr<Func>, len_func: &RPtr<Func>, ref_func: &RPtr<Func>
    , pattern_len_func: fn(&T) -> usize, pattern_ref_func: fn(&T, usize) -> &RPtr<Value>
    , obj: &mut Object) -> FPtr<Value> {
    //長さごとにパターンを集めたVec
    let mut group = Vec::<(usize, Vec<MatchClause>)>::new();

    //同じサイズのコンテナごとにグルーピング
    for (pat, body) in patterns.iter() {
        let container_pat = unsafe { pat.last().unwrap().cast_unchecked::<T>() };
        let len = pattern_len_func(container_pat.as_ref());

        if let Some((_, clauses)) = group.iter_mut().find(|(l, _)| *l == len) {
            clauses.push((pat.clone(), body));
        } else {
            let mut clauses = Vec::<MatchClause>::new();
            clauses.push((pat.clone(), body));

            group.push((len, clauses));
        }
    }

    let target_expr = exprs.last().unwrap();

    let_listbuilder!(builder_if, obj);
    builder_if.append(&syntax::literal::if_().into_value(), obj);

    //predicate
    with_cap!(is_type, cons_list2(is_type_func.cast_value(), target_expr, obj), obj, {
        builder_if.append(&is_type, obj);
    });

    // true clause
    let true_clause = {
        let_listbuilder!(builder_let, obj);
        //(let)
        builder_let.append(&syntax::literal::let_().into_value(), obj);

        //generate unique symbol
        let_cap!(len_symbol, symbol::Symbol::gensym("len", obj).into_value(), obj);
        let binders = {
            //(len (???-len target))
            let bind = {
                let_listbuilder!(builder_bind, obj);
                builder_bind.append(&len_symbol, obj);
                with_cap!(v, cons_list2(len_func.cast_value(), target_expr, obj), obj, {
                    builder_bind.append(&v, obj);
                });
                builder_bind.get().into_value()
            };
            //((len (???-len target)))
            with_cap!(bind, bind, obj, {
                list::List::alloc_tail(&bind, obj).into_value()
            })
        };
        // (let ((len (???-len target))))
        with_cap!(binders, binders, obj, {
            builder_let.append(&binders, obj);
        });

        //(cond ...)
        let cond = {
            let_listbuilder!(builder_cond, obj);
            //(cond)
            builder_cond.append(&syntax::literal::cond().into_value(), obj);

            for (container_len, mut clauses) in group.into_iter() {

                let_listbuilder!(builder_cond_clause, obj);

                //(equal? ???-len len)
                let equal = {
                    let_cap!(v1, number::Integer::alloc(container_len as i64, obj).into_value(), obj);
                    cons_list3(&value::literal::equal().into_value(), v1.as_reachable(), len_symbol.as_reachable(), obj)
                };
                //((equal? ???-len len))
                with_cap!(equal, equal, obj, {
                    builder_cond_clause.append(&equal, obj);
                });

                //(let)
                let_listbuilder!(builder_inner_let, obj);
                builder_inner_let.append(&syntax::literal::let_().into_value(), obj);

                let_listbuilder!(builder_bindders, obj);
                //後々の処理の都合上、降順でコンテナ内の値を取得する
                for index in (0..container_len).rev() {
                    let_listbuilder!(builder_binder, obj);
                    //(v0)
                    with_cap!(sym, symbol::Symbol::gensym(String::from("v") + &index.to_string() , obj).into_value(), obj, {
                        builder_binder.append(&sym, obj);
                    });

                    //(???-ref container index)
                    let container_ref = with_cap!(index, number::Integer::alloc(index as i64, obj).into_value(), obj, {
                        cons_list3(ref_func.cast_value(), target_expr, index.as_reachable(), obj)
                    });

                    //(v0 (???-ref container index))
                    with_cap!(container_ref, container_ref, obj, {
                        builder_binder.append(&container_ref, obj);
                    });

                    //((v0 (???-ref container index)))
                    with_cap!(binder, builder_binder.get().into_value(), obj, {
                        builder_bindders.append(&binder, obj);
                    });
                }

                //(let (... (v1 (???-ref container 1)) (v0 (???-ref container 0)) ...))
                let_cap!(binders, builder_bindders.get(), obj);
                builder_inner_let.append(binders.as_reachable().cast_value(), obj);

                let mut exprs =  exprs[0..exprs.len()-1].to_vec();
                for bind in binders.as_reachable().as_ref().iter() {
                    let bind = unsafe { bind.cast_unchecked::<list::List>() };
                    let sym = bind.as_ref().head_ref();
                    exprs.push(sym);
                }

                //各Clauseの先頭要素にあるコンテナを展開して、Pattern配列に追加する
                for (pat, _) in clauses.iter_mut() {
                    let container = pat.pop().unwrap();
                    let container = unsafe { container.cast_unchecked::<T>() };
                    for index in (0..container_len).rev() {
                        pat.push(pattern_ref_func(container.as_ref(), index));
                    }
                }

                //(let (... (v1 (???-ref container 1)) (v0 (???-ref container 0)))
                //  inner matcher ...)
                let matcher= translate_inner(exprs, clauses, obj);
                with_cap!(matcher, matcher, obj, {
                    builder_inner_let.append(&matcher, obj);
                });

                //((equal? container-len len)
                // (let ........))
                with_cap!(inner_let, builder_inner_let.get().into_value(), obj, {
                    builder_cond_clause.append(&inner_let, obj);
                });

                //(cond
                //  ((equal? container-len len)
                //      (let ........)))
                with_cap!(cond_clause, builder_cond_clause.get().into_value(), obj, {
                    builder_cond.append(&cond_clause, obj);
                });
            }
            with_cap!(else_, cons_cond_fail(obj), obj, {
                builder_cond.append(&else_, obj);
            });

            builder_cond.get()
        };

        //(let ((len (container-len target)))
        //  (cond ...))
        with_cap!(cond, cond.into_value(), obj, {
            builder_let.append(&cond, obj);
        });
        builder_let.get()
    };

    with_cap!(true_clause, true_clause.into_value(), obj, {
        builder_if.append(&true_clause, obj);
    });

    //マッチ失敗用の値をfalse節に追加
    builder_if.append(&MatchFail::fail().into_value(), obj);

    builder_if.get().into_value()
}

fn translate_literal(exprs: &Vec<&RPtr<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let mut group = Vec::<(&RPtr<Value>, Vec<MatchClause>)>::new();

    //同じリテラルごとにグルーピングを行う
    for (mut pat, body) in patterns.clone().into_iter() {
        let literal_pat = pat.pop().unwrap();

        if let Some((_, clauses)) = group.iter_mut().find(|(v, _)| v.as_ref() == literal_pat.as_ref()) {
            clauses.push((pat, body));
        } else {
            let mut clauses = Vec::<MatchClause>::new();
            clauses.push((pat, body));
            group.push((literal_pat, clauses));
        }
    }

    let mut exprs = exprs.clone();
    let target = exprs.pop().unwrap();

    let_listbuilder!(builder_cond, obj);
    builder_cond.append(&syntax::literal::cond().into_value(), obj);

    for (literal, patterns) in group.into_iter() {
        let_listbuilder!(builder_cond_clause, obj);

        //(equal? target literal)
        let equal = cons_list3(&value::literal::equal().into_value()
            , target
            , literal
            , obj);
        //((equal? target literal))
        with_cap!(equal, equal, obj, {
            builder_cond_clause.append(&equal, obj);
        });

        //((equal? target literal) next-match)
        let matcher= translate_inner(exprs.clone(), patterns, obj);
        with_cap!(matcher, matcher, obj, {
            builder_cond_clause.append(&matcher, obj);
        });

        //(cond ((equal? target literal) next-match))
        let cond_clause = builder_cond_clause.get().into_value();
        with_cap!(cond_clause, cond_clause, obj, {
            builder_cond.append(&cond_clause, obj);
        });
    }

    //最後にマッチ失敗の節を追加
    with_cap!(else_, cons_cond_fail(obj), obj, {
        builder_cond.append(&else_, obj);
    });

    builder_cond.get().into_value()
}

fn translate_bind(exprs: &Vec<&RPtr<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let mut exprs = exprs.clone();
    let target = exprs.pop().unwrap();

    let_listbuilder!(builder_catch, obj);
    builder_catch.append(&literal::fail_catch().into_value(), obj);

    //TODO 最適化のために同じシンボルごとにグルーピングを行いたい
    for (mut pattern, body) in patterns.clone().into_iter() {
        let bind = pattern.pop().unwrap();
        //必ず(bind ???)というような形式になっているのでリストに変換
        let bind = unsafe { bind.cast_unchecked::<List>() };
        let val = bind.as_ref().tail_ref().as_ref().head_ref();

        if let Some(symbol) = val.try_cast::<Symbol>() {
            //束縛対象がアンダースコアなら束縛を行わない
            if symbol.as_ref().as_ref() == "_" {
                let mut patterns: Vec<MatchClause> = Vec::new();
                patterns.push((pattern, body));
                let matcher= translate_inner(exprs.clone(), patterns, obj);
                with_cap!(matcher, matcher, obj, {
                    builder_catch.append(&matcher, obj);
                });

            } else {
                let_listbuilder!(builder_let, obj);
                builder_let.append(&syntax::literal::let_().into_value(), obj);

                //(x target) for let bind part
                let binder = cons_list2(val, target, obj);
                //((x target)) for let binders part
                let binders = with_cap!(binder, binder, obj, {
                    list::List::alloc_tail(&binder, obj).into_value()
                });
                //(let ((x target)))
                with_cap!(binders, binders, obj, {
                    builder_let.append(&binders, obj);
                });

                let mut patterns: Vec<MatchClause> = Vec::new();
                patterns.push((pattern, body));
                let matcher= translate_inner(exprs.clone(), patterns, obj);
                with_cap!(matcher, matcher, obj, {
                    builder_let.append(&matcher, obj);
                });

                with_cap!(let_, builder_let.get().into_value(), obj, {
                    builder_catch.append(&let_, obj);
                });
            }
        } else {
            panic!("bind variable required symbol. but got {}", val.as_ref())
        }
    }

    builder_catch.get().into_value()
}

fn translate_empty(_exprs: &Vec<&RPtr<Value>>, patterns: &Vec<MatchClause>, obj: &mut Object) -> FPtr<Value> {
    let (_, body) = patterns.first().unwrap();

    let body = (*body).clone();
    list::List::alloc(&syntax::literal::begin().into_value(), &body, obj).into_value()
}

fn cons_list2(v1: &RPtr<Value>, v2: &RPtr<Value>, obj: &mut Object) -> FPtr<Value> {
    let_listbuilder!(builder, obj);
    builder.append(v1, obj);
    builder.append(v2, obj);

    builder.get().into_value()
}

fn cons_list3(v1: &RPtr<Value>, v2: &RPtr<Value>, v3: &RPtr<Value>, obj: &mut Object) -> FPtr<Value> {
    let_listbuilder!(builder, obj);
    builder.append(v1, obj);
    builder.append(v2, obj);
    builder.append(v3, obj);

    builder.get().into_value()
}

fn cons_cond_fail(obj: &mut Object) -> FPtr<Value> {
    let_listbuilder!(builder, obj);
    //TODO よく使うシンボルはsatic領域に確保してアロケーションを避ける
    with_cap!(else_, symbol::Symbol::alloc("else", obj).into_value(), obj, {
        builder.append(&else_, obj);
    });
    builder.append(&MatchFail::fail().into_value(), obj);
    builder.get().into_value()
}

fn syntax_fail_catch(args: &RPtr<list::List>, obj: &mut Object) -> FPtr<Value> {

    for sexp in args.as_ref().iter() {
        let e = crate::eval::eval(sexp, obj);
        if MatchFail::is_fail(e.as_ref()) == false {
            return e;
        }
    }

    MatchFail::fail().into_value().into_fptr()
}

static SYNTAX_FAIL_CATCH: Lazy<GCAllocationStruct<Syntax>> = Lazy::new(|| {
    GCAllocationStruct::new(syntax::Syntax::new("fail-catch", 0, 0, true, syntax_fail_catch))
});

pub mod literal {
    use crate::ptr::RPtr;
    use super::*;

    pub fn fail_catch() -> RPtr<Syntax> {
        RPtr::new(&SYNTAX_FAIL_CATCH.value as *const Syntax as *mut Syntax)
    }
}