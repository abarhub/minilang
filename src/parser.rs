// ─────────────────────────────────────────────────────────────────────────────
//  Parser – chumsky 0.9
// ─────────────────────────────────────────────────────────────────────────────

use chumsky::prelude::*;
use chumsky::recursive::Recursive;
use log::debug;
use crate::ast::*;

fn ws() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just("//")
        .then(none_of('\n').repeated()).ignored()
        .or(just("/*").then(take_until(just("*/"))).ignored())
        .or(filter(|c: &char| c.is_whitespace()).ignored())
        .repeated().ignored()
}

fn kw(word: &'static str) -> impl Parser<char, (), Error = Simple<char>> + Clone {
    text::ident::<char, Simple<char>>()
        .try_map(move |id: String, span| {
            if id == word { Ok(()) }
            else { Err(Simple::custom(span, format!("expected '{}'", word))) }
        })
        .padded_by(ws())
}

// ── Type  ─────────────────────────────────────────────────────────────────────
//  Nouveauté : `fn(T, T) -> T`  et  `fn` (non annoté)

fn type_parser() -> impl Parser<char, Type, Error = Simple<char>> + Clone {
    recursive(|ty| {
        let generic_args = ty.clone()
            .separated_by(just(',').padded_by(ws())).at_least(1)
            .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()));

        // `fn(T, T) -> T`  ou  `fn` seul
        let fn_type = kw("fn")
            .ignore_then(
                ty.clone()
                    .separated_by(just(',').padded_by(ws())).allow_trailing()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                    .then_ignore(just("->").padded_by(ws()))
                    .then(ty.clone())
                    .or_not()
            )
            .map(|maybe| match maybe {
                Some((params, ret)) => Type::FnType(params, Box::new(ret)),
                None                => Type::Fn,
            });

        let base = choice((
            kw("int")   .to(Type::Int),
            kw("byte")  .to(Type::Byte),
            kw("bool")  .to(Type::Bool),
            kw("string").to(Type::Str),
            kw("char")  .to(Type::Char),
            kw("float") .to(Type::Float),
            kw("double").to(Type::Double),
            kw("void")  .to(Type::Void),
            fn_type,   // fn / fn(...)->T  — avant ident
            text::ident()
                // `inject` est un mot-clé réservé — sinon `inject X;` serait
                // parsé comme la déclaration d'une variable X de type inject
                .try_map(|n: String, span| if n == "inject" {
                    Err(Simple::custom(span, "'inject' est un mot-clé réservé"))
                } else { Ok(n) })
                .padded_by(ws())
                .then(generic_args.or_not())
                .map(|(n, a)| match a {
                    Some(a) => Type::Generic(n, a),
                    None    => Type::UserDefined(n),
                }),
        ));

        base.then(
            just('[').padded_by(ws()).then(just(']').padded_by(ws())).repeated()
        )
        .then(just('?').padded_by(ws()).or_not())
        .map(|((t, v), opt)| {
            let t = v.into_iter().fold(t, |acc, _| Type::Array(Box::new(acc)));
            if opt.is_some() { Type::Generic("Option".to_string(), vec![t]) } else { t }
        })
    })
}

// ── Parser record (extrait de program_parser pour alléger son stack frame) ───

/// Parseur de déclaration `record` extrait dans une fonction séparée pour
/// réduire la taille du stack frame de `program_parser()`.
/// Retourne un `BoxedParser` (heap-alloué) afin que sa taille sur la pile de
/// l'appelant soit un simple pointeur (8 octets).
fn record_def_parser() -> chumsky::BoxedParser<'static, char, RecordDef, Simple<char>> {
    let ty = type_parser();

    let param = ty.clone()
        .then(text::ident().padded_by(ws()))
        .map(|(ty, name)| Param { ty, name });
    let params = param.separated_by(just(',').padded_by(ws())).allow_trailing();

    // Bloc de corps de méthode : consomme { ... } en gérant une seule
    // imbrication d'accolades — suffit pour les méthodes de record.
    // `builtin;` est aussi accepté comme corps natif.
    let braced_block: chumsky::BoxedParser<char, Vec<Stmt>, Simple<char>> =
        just('{').padded_by(ws())
            .ignore_then(take_until(just('}')))
            .map(|_| vec![Stmt::Builtin])
            .or(
                text::keyword("builtin").padded_by(ws())
                    .then_ignore(just(';').padded_by(ws()))
                    .map(|_| vec![Stmt::Builtin])
            )
            .boxed();

    // Type params : [immutable|readonly] Ident séparés par ','
    let type_param_list = choice((
            kw("immutable").to(Qualifier::Immutable),
            kw("readonly") .to(Qualifier::Readonly),
        )).or_not().map(|q| q.unwrap_or(Qualifier::Mutable))
        .then(text::ident().padded_by(ws()))
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    // Visibilité + mutable
    let vis_mut: chumsky::BoxedParser<char, (Visibility, bool), Simple<char>> = choice((
            kw("private")  .to(Visibility::Private),
            kw("protected").to(Visibility::Protected),
        )).or_not().map(|v| v.unwrap_or(Visibility::Public))
        .then(kw("mutable").to(true).or_not().map(|m| m.unwrap_or(false)))
        .boxed();

    let record_method = vis_mut
        .then(ty.clone())
        .then(text::ident().padded_by(ws()))
        .then(params.delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then(braced_block)
        .map(|(((((visibility, is_mutable), rt), n), p), b)|
            Method { visibility, is_mutable, return_type: rt, name: n, params: p, body: b });

    // Champs dans les parenthèses : `(int x, string name)`
    let record_fields = ty.clone()
        .then(text::ident().padded_by(ws()))
        .map(|(ty, name)| Field { ty, name })
        .separated_by(just(',').padded_by(ws())).allow_trailing()
        .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()));

    // implements Iface1, Iface2
    let record_implements = kw("implements")
        .ignore_then(
            text::ident().padded_by(ws())
                .then(
                    just('<')
                        .ignore_then(type_parser()
                            .separated_by(just(',').padded_by(ws())).at_least(1))
                        .then_ignore(just('>').padded_by(ws()))
                        .or_not()
                )
                .map(|(name, _)| name)
                .separated_by(just(',').padded_by(ws())).at_least(1)
        )
        .or_not().map(|v| v.unwrap_or_default());

    kw("record")
        .ignore_then(text::ident().padded_by(ws()))
        .then(type_param_list)
        .then(record_fields)
        .then(record_implements)
        .then(record_method.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|((((name, tp), fields), implements), methods)| {
            let type_param_constraints: Vec<(String, Qualifier)> = tp.iter()
                .filter(|(q, _)| *q != Qualifier::Mutable)
                .map(|(q, n)| (n.clone(), q.clone()))
                .collect();
            let type_params: Vec<String> = tp.into_iter().map(|(_, n)| n).collect();
            RecordDef { name, type_params, type_param_constraints, fields, methods, implements }
        })
        .boxed()
}

// ── Point d'entrée ────────────────────────────────────────────────────────────

pub fn program_parser() -> impl Parser<char, Program, Error = Simple<char>> {
    let ty = type_parser();

    let param = ty.clone()
        .then(text::ident().padded_by(ws()))
        .map(|(ty, name)| Param { ty, name });

    let params = param.clone()
        .separated_by(just(',').padded_by(ws())).allow_trailing();

    // Forward-declaration (récursion mutuelle expr ↔ stmt)
    let mut stmt_fwd: Recursive<char, Stmt, Simple<char>> = Recursive::declare();

    // ── Expressions ───────────────────────────────────────────────────────────

    let expr: BoxedParser<char, Expr, Simple<char>> = recursive(|expr| {
        let call_args = expr.clone()
            .separated_by(just(',').padded_by(ws())).allow_trailing()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()));

        let str_char = just('\\').ignore_then(choice((
            just('n').to('\n'),
            just('t').to('\t'),
            just('r').to('\r'),
            just('\\').to('\\'),
            just('"').to('"'),
            just('0').to('\0'),
        ))).or(none_of('"'));

        // Chaîne multiligne : """..."""  (peut contenir des sauts de ligne)
        let ml_str_lit = just("\"\"\"")
            .ignore_then(
                take_until(just("\"\"\""))
                    .map(|(chars, _): (Vec<char>, _)| chars.into_iter().collect::<String>())
            )
            .map(Expr::StringLit)
            .padded_by(ws());

        let str_lit = ml_str_lit.or(
            just('"')
                .ignore_then(str_char.repeated().collect::<String>())
                .then_ignore(just('"'))
                .map(Expr::StringLit).padded_by(ws())
        );

        let char_escape = just('\\').ignore_then(choice((
            just('n') .to('\n'),
            just('t') .to('\t'),
            just('r') .to('\r'),
            just('\\').to('\\'),
            just('\'').to('\''),
            just('0') .to('\0'),
        )));
        let char_lit = just('\'')
            .ignore_then(char_escape.or(none_of('\'')))
            .then_ignore(just('\''))
            .map(Expr::CharLit)
            .padded_by(ws());

        // Unified number literal parser: parse integer part first, then optionally
        // a '.' followed by digits to produce a float.  This avoids the chumsky
        // backtracking issue where consuming the integer part of "0" and then
        // failing on the '.' check left the int parser unable to match.
        // `text::int(10)` interprète la partie entière (pas de zéros initiaux
        // significatifs) ; pour la partie fractionnaire on utilise `digits(10)`
        // afin de conserver les zéros initiaux (ex : ".001" → "001", pas "1").
        let number_lit = text::int(10)
            .then(just('.').ignore_then(text::digits(10)).or_not())
            .map(|(i, maybe_frac): (String, Option<String>)| match maybe_frac {
                Some(f) => Expr::FloatLit(format!("{}.{}", i, f).parse().unwrap()),
                None    => Expr::IntLit(i.parse().unwrap()),
            })
            .padded_by(ws());

        let bool_lit = choice((
            kw("true") .to(Expr::BoolLit(true)),
            kw("false").to(Expr::BoolLit(false)),
        ));

        let new_type_args = type_parser()
            .separated_by(just(',').padded_by(ws()))
            .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()));

        let new_expr = kw("new")
            .ignore_then(text::ident().padded_by(ws()))
            .then(new_type_args.or_not().map(|v| v.unwrap_or_default()))
            .then(call_args.clone())
            .map(|((cn, ta), args)| Expr::New { class_name: cn, type_args: ta, args });

        let enum_ctor_type_args = type_parser()
            .separated_by(just(',').padded_by(ws())).at_least(1)
            .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
            .or_not().map(|v| v.unwrap_or_default());

        let enum_ctor = text::ident().padded_by(ws())
            .then(enum_ctor_type_args)
            .then_ignore(just("::").padded_by(ws()))
            .then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not().map(|v| v.unwrap_or_default()))
            .map(|(((en, ta), v), a)| Expr::EnumConstructor {
                enum_name: en, type_args: ta, variant: v, args: a,
            });

        let this_kw = kw("this").to(Expr::Ident("this".to_string()));

        // `inject T` — résolution d'un service (classe service ou interface)
        let inject_expr = kw("inject")
            .ignore_then(text::ident().padded_by(ws()))
            .map(|name| Expr::Inject(Type::UserDefined(name)));

        let ident_or_call = text::ident().padded_by(ws())
            .then(call_args.clone().or_not())
            .map(|(name, args)| match args {
                Some(a) => Expr::FunctionCall { name, args: a },
                None    => Expr::Ident(name),
            });

        // `(expr)` ou `(expr)(args)` pour l'appel inline de lambda
        let paren_or_call = expr.clone()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            .then(call_args.clone().or_not())
            .map(|(e, maybe_args)| match maybe_args {
                None    => e,
                Some(a) => Expr::LambdaCall { callee: Box::new(e), args: a },
            });

        // new T[]{1, 2, 3}  — type_parser() consomme "int[]" comme Type::Array(Int)
        let array_lit = kw("new")
            .ignore_then(type_parser())
            .then(
                expr.clone()
                    .separated_by(just(',').padded_by(ws())).allow_trailing()
                    .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()))
            )
            .try_map(|(t, elems), span| match t {
                Type::Array(inner) => Ok(Expr::ArrayLit { elem_type: *inner, elements: elems }),
                _ => Err(Simple::custom(span, "new T[]{...} requires array type T[]")),
            });

        // new int[5]       — tableau de taille n, valeur par défaut
        // new int[5](0)    — tableau de taille n, initialisé avec 0
        let array_new = kw("new")
            .ignore_then(type_parser())
            .then(expr.clone()
                .delimited_by(just('[').padded_by(ws()), just(']').padded_by(ws())))
            .then(expr.clone()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                .or_not())
            .map(|((t, size), fill)| Expr::ArrayNew {
                elem_type: t,
                size: Box::new(size),
                fill: fill.map(Box::new),
            });

        let atom = choice((
            str_lit, char_lit, number_lit, bool_lit,
            this_kw, inject_expr, array_lit, array_new, new_expr, enum_ctor, ident_or_call,
            paren_or_call,
        ));

        // Postfix : .field  .method(args)  ?.field  ?.method(args)  [idx]
        #[derive(Clone)]
        enum Postfix {
            Field(String), Method(String, Vec<Expr>),
            SafeField(String), SafeMethod(String, Vec<Expr>),
            Index(Box<Expr>),
        }

        let safe_postfix_op = just("?.").padded_by(ws())
            .ignore_then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not())
            .map(|(name, args)| match args {
                Some(a) => Postfix::SafeMethod(name, a),
                None    => Postfix::SafeField(name),
            });

        let postfix_op = just('.').padded_by(ws())
            .ignore_then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not())
            .map(|(name, args)| match args {
                Some(a) => Postfix::Method(name, a),
                None    => Postfix::Field(name),
            });

        let index_postfix = expr.clone()
            .delimited_by(just('[').padded_by(ws()), just(']').padded_by(ws()))
            .map(|e| Postfix::Index(Box::new(e)));

        let postfix = atom
            .then(choice((index_postfix, safe_postfix_op.or(postfix_op))).repeated())
            .foldl(|obj, pf| match pf {
                Postfix::Field(f)        => Expr::FieldAccess      { object: Box::new(obj), field: f },
                Postfix::Method(m, a)    => Expr::MethodCall       { object: Box::new(obj), method: m, args: a },
                Postfix::SafeField(f)    => Expr::SafeFieldAccess  { object: Box::new(obj), field: f },
                Postfix::SafeMethod(m,a) => Expr::SafeMethodCall   { object: Box::new(obj), method: m, args: a },
                Postfix::Index(i)        => Expr::Index { object: Box::new(obj), index: i },
            });

        // Hiérarchie arithmétique / logique
        let pow = postfix.clone()
            .then(just("**").padded_by(ws()).to(BinOp::Pow).then(postfix.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let unary = recursive(|u| choice((
            just('-').padded_by(ws()).ignore_then(u.clone())
                .map(|e| Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(e) }),
            just('!').padded_by(ws()).ignore_then(u.clone())
                .map(|e| Expr::UnaryOp { op: UnaryOp::Not, expr: Box::new(e) }),
            pow.clone(),
        )));

        let mul = unary.clone()
            .then(choice((
                just('%').padded_by(ws()).to(BinOp::Mod),
                just('/').padded_by(ws()).to(BinOp::Div),
                just('*').padded_by(ws()).to(BinOp::Mul),
            )).then(unary.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let add = mul.clone()
            .then(choice((
                just('+').padded_by(ws()).to(BinOp::Add),
                just('-').padded_by(ws()).to(BinOp::Sub),
            )).then(mul.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let rel = add.clone()
            .then(choice((
                just("<=").padded_by(ws()).to(BinOp::Le),
                just(">=").padded_by(ws()).to(BinOp::Ge),
                just('<') .padded_by(ws()).to(BinOp::Lt),
                just('>') .padded_by(ws()).to(BinOp::Gt),
            )).then(add.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let eq = rel.clone()
            .then(choice((
                just("==").padded_by(ws()).to(BinOp::Eq),
                just("!=").padded_by(ws()).to(BinOp::Ne),
            )).then(rel.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let and = eq.clone()
            .then(just("&&").padded_by(ws()).to(BinOp::And).then(eq.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        let arith = and.clone()
            .then(just("||").padded_by(ws()).to(BinOp::Or).then(and.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── Lambda (priorité la plus basse) ───────────────────────────────────
        let lambda_block = stmt_fwd.clone().repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

        // Corps = bloc  ou  expr récursif (supporte x => y => z)
        let lambda_body_p = lambda_block.map(LambdaBody::Block)
            .or(expr.clone().map(|e| LambdaBody::Expr(Box::new(e))));

        let lambda_multi = text::ident().padded_by(ws())
            .separated_by(just(',').padded_by(ws())).allow_trailing()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            .then_ignore(just("=>").padded_by(ws()))
            .then(lambda_body_p.clone())
            .map(|(ps, body)| { debug!("λ({})", ps.len()); Expr::Lambda { params: ps, body } });

        let lambda_single = text::ident().padded_by(ws())
            .then_ignore(just("=>").padded_by(ws()))
            .then(lambda_body_p)
            .map(|(p, body)| { debug!("λ(1)"); Expr::Lambda { params: vec![p], body } });

        // `??` — null coalescing (priorité inférieure à ||, supérieure aux lambdas)
        let null_coal = arith.clone()
            .then(just("??").padded_by(ws()).ignore_then(arith.clone()).or_not())
            .map(|(e, d)| match d {
                None      => e,
                Some(def) => Expr::NullCoalesce { expr: Box::new(e), default: Box::new(def) },
            });

        choice((lambda_multi, lambda_single, null_coal)).boxed()
    }).boxed();

    // ── Type primitif pour les déclarations de variable ───────────────────────

    let kw_type = choice((
        kw("int")   .to(Type::Int),
        kw("byte")  .to(Type::Byte),
        kw("bool")  .to(Type::Bool),
        kw("string").to(Type::Str),
        kw("char")  .to(Type::Char),
        kw("float") .to(Type::Float),
        kw("double").to(Type::Double),
    ))
    .then(just('[').padded_by(ws()).then(just(']').padded_by(ws())).repeated())
    .then(just('?').padded_by(ws()).or_not())
    .map(|((t, v), opt)| {
        let t = v.into_iter().fold(t, |acc, _| Type::Array(Box::new(acc)));
        if opt.is_some() { Type::Generic("Option".to_string(), vec![t]) } else { t }
    });

    // ── Instructions ──────────────────────────────────────────────────────────

    let stmt_impl: BoxedParser<char, Stmt, Simple<char>> = recursive(|stmt| {
        let body = stmt.clone().repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

        let print_stmt = kw("print")
            .ignore_then(
                expr.clone().separated_by(just(',').padded_by(ws())).allow_trailing()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            )
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::Print);

        let return_stmt = kw("return")
            .ignore_then(expr.clone().or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::Return);

        let break_stmt    = kw("break")   .then_ignore(just(';').padded_by(ws())).to(Stmt::Break);
        let continue_stmt = kw("continue").then_ignore(just(';').padded_by(ws())).to(Stmt::Continue);

        let if_stmt = kw("if")
            .ignore_then(expr.clone()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(body.clone())
            .then(kw("else").ignore_then(
                stmt.clone().map(|s| vec![s]).or(body.clone())
            ).or_not())
            .map(|((cond, tb), eb)| Stmt::If { condition: cond, then_body: tb, else_body: eb });

        let while_stmt = kw("while")
            .ignore_then(expr.clone()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(body.clone())
            .map(|(cond, b)| Stmt::While { condition: cond, body: b });

        let do_while = kw("do")
            .ignore_then(body.clone())
            .then_ignore(kw("while"))
            .then(expr.clone()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then_ignore(just(';').padded_by(ws()))
            .map(|(b, cond)| Stmt::DoWhile { body: b, condition: cond });

        let for_init = choice((
            kw_type.clone()
                .then(text::ident().padded_by(ws()))
                .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
                .map(|((ty, name), init)| Box::new(Stmt::VarDecl { qualifier: Qualifier::Mutable, ty, name, init })),
            type_parser()
                .then(text::ident().padded_by(ws()))
                .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
                .map(|((ty, name), init)| Box::new(Stmt::VarDecl { qualifier: Qualifier::Mutable, ty, name, init })),
            text::ident().padded_by(ws())
                .then_ignore(just('=').padded_by(ws()))
                .then(expr.clone())
                .map(|(t, v)| Box::new(Stmt::Assign { target: t, value: v })),
        ));

        let for_update = choice((
            text::ident().padded_by(ws())
                .then_ignore(just('.').padded_by(ws()))
                .then(text::ident().padded_by(ws()))
                .then_ignore(just('=').padded_by(ws()))
                .then(expr.clone())
                .map(|((o, f), v)| Box::new(Stmt::FieldAssign { object: o, field: f, value: v })),
            text::ident().padded_by(ws())
                .then_ignore(just('=').padded_by(ws()))
                .then(expr.clone())
                .map(|(t, v)| Box::new(Stmt::Assign { target: t, value: v })),
            expr.clone().map(|e| Box::new(Stmt::ExprStmt(e))),
        ));

        // for (Type varName in expr) { body }  — for-in (essayé en premier)
        let for_in_stmt = kw("for")
            .ignore_then(
                type_parser()
                    .then(text::ident().padded_by(ws()))
                    .then_ignore(kw("in"))
                    .then(expr.clone())
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            )
            .then(body.clone())
            .map(|(((ty, name), iter), b)| Stmt::ForIn {
                var_type: ty, var_name: name, iter_expr: Box::new(iter), body: b
            });

        // for (init; cond; update) { body }  — for classique
        let for_stmt = kw("for")
            .ignore_then(
                for_init.or_not()
                    .then_ignore(just(';').padded_by(ws()))
                    .then(expr.clone().or_not())
                    .then_ignore(just(';').padded_by(ws()))
                    .then(for_update.or_not())
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            )
            .then(body.clone())
            .map(|(((init, cond), upd), b)| Stmt::For {
                init, condition: cond, update: upd, body: b
            });

        let pattern = choice((
            just('_').padded_by(ws()).to(Pattern::Wildcard),
            text::ident().padded_by(ws())
                .then_ignore(just("::").padded_by(ws()))
                .then(text::ident().padded_by(ws()))
                .then(
                    text::ident().padded_by(ws())
                        .separated_by(just(',').padded_by(ws())).allow_trailing()
                        .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                        .or_not().map(|v| v.unwrap_or_default())
                )
                .map(|((_en, name), bindings)| Pattern::Variant { name, bindings }),
        ));

        let match_arm = pattern
            .then_ignore(just("=>").padded_by(ws()))
            .then(body.clone())
            .map(|(pattern, body)| MatchArm { pattern, body });

        let match_stmt = kw("match")
            .ignore_then(expr.clone())
            .then(match_arm.repeated()
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
            .map(|(e, arms)| Stmt::Match { expr: e, arms });

        let builtin_stmt = kw("builtin")
            .then_ignore(just(';').padded_by(ws()))
            .to(Stmt::Builtin);

        // Qualificateur optionnel : readonly | immutable (défaut = Mutable)
        let qualifier = choice((
            kw("readonly") .to(Qualifier::Readonly),
            kw("immutable").to(Qualifier::Immutable),
        )).or_not().map(|q| q.unwrap_or(Qualifier::Mutable));

        let kw_var_decl = qualifier.clone()
            .then(kw_type.clone())
            .then(text::ident().padded_by(ws()))
            .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(|(((qualifier, ty), name), init)| Stmt::VarDecl { qualifier, ty, name, init });

        // type_parser() capte fn(T)->T, les types génériques, etc.
        let generic_var_decl = qualifier.clone()
            .then(type_parser())
            .then(text::ident().padded_by(ws()))
            .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(|(((qualifier, ty), name), init)| Stmt::VarDecl { qualifier, ty, name, init });

        let field_assign = text::ident().padded_by(ws())
            .then_ignore(just('.').padded_by(ws()))
            .then(text::ident().padded_by(ws()))
            .then_ignore(just('=').padded_by(ws()))
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws()))
            .map(|((o, f), v)| Stmt::FieldAssign { object: o, field: f, value: v });

        let assign_stmt = text::ident().padded_by(ws())
            .then_ignore(just('=').padded_by(ws()))
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws()))
            .map(|(t, v)| Stmt::Assign { target: t, value: v });

        let expr_stmt = expr.clone()
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::ExprStmt);

        choice((
            print_stmt, return_stmt, break_stmt, continue_stmt,
            builtin_stmt,
            if_stmt, while_stmt, do_while, for_in_stmt, for_stmt, match_stmt,
            kw_var_decl, field_assign, generic_var_decl, assign_stmt, expr_stmt,
        ))
        .padded_by(ws())
        .boxed()
    }).boxed();

    stmt_fwd.define(stmt_impl);
    let stmt = stmt_fwd;

    let body = stmt.clone().repeated()
        .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

    // Corps de méthode : bloc normal  OU  `builtin;`  (implémentation native)
    let method_body = body.clone()
        .or(kw("builtin").then_ignore(just(';').padded_by(ws())).to(vec![Stmt::Builtin]));

    // ── Membres de classe ─────────────────────────────────────────────────────

    enum CM { F(Field), C(Constructor), M(Method) }

    let class_member = {
        let ctor = text::ident().padded_by(ws())
            .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(body.clone())
            .map(|((_n, p), b)| CM::C(Constructor { params: p, body: b }));

        // Préfixe visibility+mutable boxé pour limiter la profondeur de type
        let vis_mut: chumsky::BoxedParser<char, (Visibility, bool), Simple<char>> = choice((
                kw("private")  .to(Visibility::Private),
                kw("protected").to(Visibility::Protected),
            )).or_not().map(|v| v.unwrap_or(Visibility::Public))
            .then(kw("mutable").to(true).or_not().map(|m| m.unwrap_or(false)))
            .boxed();

        let method = vis_mut
            .then(ty.clone())
            .then(text::ident().padded_by(ws()))
            .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(method_body.clone())
            .map(|(((((visibility, is_mutable), rt), n), p), b)| CM::M(Method { visibility, is_mutable, return_type: rt, name: n, params: p, body: b }));

        let field = ty.clone()
            .then(text::ident().padded_by(ws()))
            .then_ignore(just(';').padded_by(ws()))
            .map(|(ty, name)| CM::F(Field { ty, name }));

        choice((ctor, method, field))
    };

    // ── Enum ──────────────────────────────────────────────────────────────────

    let enum_variant = text::ident().padded_by(ws())
        .then(
            param.clone().separated_by(just(',').padded_by(ws())).allow_trailing()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                .or_not().map(|v| v.unwrap_or_default())
        )
        .map(|(name, fields)| EnumVariant { name, fields });

    let vis_mut_enum: chumsky::BoxedParser<char, (Visibility, bool), Simple<char>> = choice((
            kw("private")  .to(Visibility::Private),
            kw("protected").to(Visibility::Protected),
        )).or_not().map(|v| v.unwrap_or(Visibility::Public))
        .then(kw("mutable").to(true).or_not().map(|m| m.unwrap_or(false)))
        .boxed();

    let enum_method = vis_mut_enum
        .then(ty.clone())
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then(method_body.clone())
        .map(|(((((visibility, is_mutable), rt), n), p), b)| Method { visibility, is_mutable, return_type: rt, name: n, params: p, body: b });

    // Paramètre de type avec contrainte optionnelle : [immutable|readonly] Ident
    let type_param_entry = choice((
        kw("immutable").to(Qualifier::Immutable),
        kw("readonly") .to(Qualifier::Readonly),
    )).or_not().map(|q| q.unwrap_or(Qualifier::Mutable))
      .then(text::ident().padded_by(ws()));

    let enum_type_params = type_param_entry.clone()
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    let enum_implements = kw("implements")
        .ignore_then(
            text::ident().padded_by(ws())
                .then(
                    just('<')
                        .ignore_then(type_parser().separated_by(just(',').padded_by(ws())).at_least(1))
                        .then_ignore(just('>').padded_by(ws()))
                        .or_not()
                )
                .map(|(name, _)| name)
                .separated_by(just(',').padded_by(ws())).at_least(1)
        )
        .or_not().map(|v| v.unwrap_or_default());

    let enum_def = kw("enum")
        .ignore_then(text::ident().padded_by(ws()))
        .then(enum_type_params)
        .then(enum_implements)
        .then(
            enum_variant.separated_by(just(',').padded_by(ws())).allow_trailing()
                .then(just(';').padded_by(ws()).ignore_then(enum_method.repeated())
                    .or_not().map(|v| v.unwrap_or_default()))
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()))
        )
        .map(|(((name, tp), implements), (variants, methods))| {
            let type_param_constraints: Vec<(String, Qualifier)> = tp.iter()
                .filter(|(q, _)| *q != Qualifier::Mutable)
                .map(|(q, n)| (n.clone(), q.clone()))
                .collect();
            let type_params: Vec<String> = tp.into_iter().map(|(_, n)| n).collect();
            EnumDef { name, type_params, type_param_constraints, implements, variants, methods }
        });

    // ── Interface ─────────────────────────────────────────────────────────────

    let method_sig = kw("mutable").to(true).or_not().map(|m| m.unwrap_or(false))
        .then(ty.clone())
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then_ignore(just(';').padded_by(ws()))
        .map(|(((is_mutable, rt), n), p)| MethodSig { is_mutable, return_type: rt, name: n, params: p });

    // ── Classe ────────────────────────────────────────────────────────────────

    let type_param_list = type_param_entry.clone()
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    // extends Iface1, Iface2  — interfaces étendues (type args éventuels ignorés)
    let extends_ifaces = kw("extends")
        .ignore_then(
            text::ident().padded_by(ws())
                .then(
                    just('<')
                        .ignore_then(type_parser().separated_by(just(',').padded_by(ws())).at_least(1))
                        .then_ignore(just('>').padded_by(ws()))
                        .or_not()
                )
                .map(|(name, _)| name)
                .separated_by(just(',').padded_by(ws())).at_least(1)
        )
        .or_not().map(|v| v.unwrap_or_default());

    let interface_def = kw("mut").to(true).or_not().map(|m| m.unwrap_or(false))
        .then_ignore(kw("interface"))
        .then(text::ident().padded_by(ws()))
        .then(type_param_list.clone())
        .then(extends_ifaces)
        .then(method_sig.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|((((is_mut, name), tp), parents), methods)| {
            let type_param_constraints: Vec<(String, Qualifier)> = tp.iter()
                .filter(|(q, _)| *q != Qualifier::Mutable)
                .map(|(q, n)| (n.clone(), q.clone()))
                .collect();
            let type_params: Vec<String> = tp.into_iter().map(|(_, n)| n).collect();
            InterfaceDef { is_mut, name, type_params, type_param_constraints, parents, methods }
        });

    let class_def = kw("transient").to(true).or_not().map(|t| t.unwrap_or(false))
        .then(kw("service").to(true).or_not().map(|s| s.unwrap_or(false)))
        .then(kw("mut").to(true).or_not().map(|m| m.unwrap_or(false)))
        .then_ignore(kw("class"))
        .then(text::ident().padded_by(ws()))
        .then(type_param_list.clone())
        .then(kw("extends").ignore_then(text::ident().padded_by(ws())).or_not())
        .then(kw("implements")
            .ignore_then(
                text::ident().padded_by(ws())
                    .then(
                        just('<')
                            .ignore_then(type_parser().separated_by(just(',').padded_by(ws())).at_least(1))
                            .then_ignore(just('>').padded_by(ws()))
                            .or_not()
                    )
                    .map(|(name, _)| name)
                    .separated_by(just(',').padded_by(ws())).at_least(1)
            )
            .or_not().map(|v| v.unwrap_or_default()))
        .then(class_member.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|(((((((is_transient, is_service), is_mut), name), tp), parent), impls), members)| {
            let mut fields = vec![]; let mut ctors = vec![]; let mut methods = vec![];
            for m in members { match m {
                CM::F(f) => fields.push(f),
                CM::C(c) => ctors.push(c),
                CM::M(m) => methods.push(m),
            }}
            let type_param_constraints: Vec<(String, Qualifier)> = tp.iter()
                .filter(|(q, _)| *q != Qualifier::Mutable)
                .map(|(q, n)| (n.clone(), q.clone()))
                .collect();
            let type_params: Vec<String> = tp.into_iter().map(|(_, n)| n).collect();
            ClassDef { is_service, is_transient, is_mut, name, type_params, type_param_constraints,
                       parent, implements: impls, fields, constructors: ctors, methods }
        });

    // ── Module d'injection : module Name { bind ...; } ───────────────────────

    let bind_decl = kw("bind")
        .ignore_then(text::ident().padded_by(ws()))
        .then(kw("to").ignore_then(text::ident().padded_by(ws())).or_not())
        .then(kw("with")
            .ignore_then(
                expr.clone()
                    .separated_by(just(',').padded_by(ws())).allow_trailing()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
            )
            .or_not().map(|v| v.unwrap_or_default()))
        .then_ignore(just(';').padded_by(ws()))
        .map(|((target, to), with)| BindDecl { target, to, with });

    let module_def = kw("module")
        .ignore_then(text::ident().padded_by(ws()))
        .then(bind_decl.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|(name, binds)| ModuleDef { name, binds });

    // ── Record ────────────────────────────────────────────────────────────────

    // Parser record dans une fonction séparée pour réduire la taille du stack
    // frame de program_parser() (trop de variables locales → stack overflow).
    let record_def = record_def_parser();

    // ── Alias de type : `type Name = T;` ─────────────────────────────────────

    let type_alias = kw("type")
        .ignore_then(text::ident().padded_by(ws()))
        .then_ignore(just('=').padded_by(ws()))
        .then(ty.clone())
        .then_ignore(just(';').padded_by(ws()))
        .map(|(name, ty)| { debug!("alias {} = {}", name, ty); TypeAlias { name, ty } });

    // ── main ──────────────────────────────────────────────────────────────────

    let main_func = kw("int").ignore_then(kw("main"))
        .ignore_then(just('(').padded_by(ws()).then(just(')').padded_by(ws())))
        .ignore_then(body.clone())
        .map(|stmts| MainFunc { body: stmts });

    // ── Package & imports ─────────────────────────────────────────────────────

    let dotted = text::ident()
        .then(just('.').ignore_then(text::ident()).repeated())
        .map(|(h, t)| { let mut p = vec![h]; p.extend(t); p.join(".") });

    let package_decl = kw("package")
        .ignore_then(dotted.clone().padded_by(ws()))
        .then_ignore(just(';').padded_by(ws()))
        .map(|path| PackageDecl { path });

    let import_path = text::ident()
        .then(just('.').ignore_then(
            text::ident().or(just('*').to("*".to_string()))).repeated().at_least(1))
        .map(|(h, t)| {
            let mut p = vec![h]; p.extend(t.iter().cloned());
            let wildcard = p.last().map(|s| s.as_str()) == Some("*");
            Import { path: p.join("."), wildcard }
        });

    let import_decl = kw("import")
        .ignore_then(import_path.padded_by(ws()))
        .then_ignore(just(';').padded_by(ws()));

    // ── Programme complet ─────────────────────────────────────────────────────

    // Déclarations de haut niveau dans n'importe quel ordre.
    // Package et import peuvent apparaître n'importe où (nécessaire lorsque la
    // stdlib est préfixée au source et que ce dernier possède ses propres
    // directives package/import).
    enum TopDecl {
        Package(PackageDecl),
        Import(Import),
        Alias(TypeAlias),
        Module(ModuleDef),
        Iface(InterfaceDef),
        Enum(EnumDef),
        Record(RecordDef),
        Class(ClassDef),
        Func(FuncDef),
    }

    // Fonction de haut niveau : `[test] Type name(params) { body }`
    // On exclut "main" pour que main_func le prenne en charge séparément.
    // Le préfixe `test` marque une fonction de test (runner `mini_parser test`).
    let func_def = kw("test").to(true).or_not().map(|t| t.unwrap_or(false))
        .then(ty.clone())
        .then(text::ident().padded_by(ws()).try_map(|name: String, span| {
            if name == "main" {
                Err(Simple::custom(span, "main is reserved"))
            } else {
                Ok(name)
            }
        }))
        .then(params.clone()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then(body.clone())
        .map(|((((is_test, rt), name), p), b)|
            FuncDef { is_test, return_type: rt, name, params: p, body: b });

    // Chaque parser est boxé avant d'entrer dans choice() pour que la struct
    // ChoiceParser ne contienne que des pointeurs (16 octets chacun) et non
    // les types concrets complets — évite le stack overflow du stack frame de
    // program_parser() qui est déjà très chargé en variables locales.
    let top_decl: chumsky::BoxedParser<char, TopDecl, Simple<char>> = choice((
        package_decl.map(TopDecl::Package).boxed(),
        import_decl  .map(TopDecl::Import) .boxed(),
        type_alias   .map(TopDecl::Alias)  .boxed(),
        module_def   .map(TopDecl::Module) .boxed(),
        interface_def.map(TopDecl::Iface)  .boxed(),
        enum_def     .map(TopDecl::Enum)   .boxed(),
        record_def   .map(TopDecl::Record) .boxed(),
        class_def    .map(TopDecl::Class)  .boxed(),
        func_def     .map(TopDecl::Func)   .boxed(),
    )).boxed();

    ws()
        .ignore_then(top_decl.repeated())
        .then(main_func.or_not())   // optionnel : un fichier de tests n'a pas de main
        .then_ignore(ws())
        .then_ignore(end())
        .map(|(decls, main)| {
            let mut pkg = None;
            let mut imports = vec![];
            let mut aliases = vec![]; let mut modules = vec![]; let mut ifaces = vec![];
            let mut enums = vec![]; let mut records = vec![]; let mut classes = vec![];
            let mut funcs = vec![];
            for d in decls { match d {
                TopDecl::Package(p)  => { if pkg.is_none() { pkg = Some(p); } }
                TopDecl::Import(i)   => imports.push(i),
                TopDecl::Alias(a)    => aliases.push(a),
                TopDecl::Module(m)   => modules.push(m),
                TopDecl::Iface(i)    => ifaces.push(i),
                TopDecl::Enum(e)     => enums.push(e),
                TopDecl::Record(r)   => records.push(r),
                TopDecl::Class(c)    => classes.push(c),
                TopDecl::Func(f)     => funcs.push(f),
            }}
            Program { package: pkg, imports, type_aliases: aliases, modules,
                      interfaces: ifaces, enums, records, classes, funcs, main }
        })
}
