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
            kw("bool")  .to(Type::Bool),
            kw("string").to(Type::Str),
            kw("char")  .to(Type::Char),
            kw("float") .to(Type::Float),
            kw("double").to(Type::Double),
            kw("void")  .to(Type::Void),
            fn_type,   // fn / fn(...)->T  — avant ident
            text::ident().padded_by(ws())
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
            this_kw, array_lit, array_new, new_expr, enum_ctor, ident_or_call,
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
                .map(|((ty, name), init)| Box::new(Stmt::VarDecl { ty, name, init })),
            type_parser()
                .then(text::ident().padded_by(ws()))
                .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
                .map(|((ty, name), init)| Box::new(Stmt::VarDecl { ty, name, init })),
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

        let index_assign = text::ident().padded_by(ws())
            .then(expr.clone()
                .delimited_by(just('[').padded_by(ws()), just(']').padded_by(ws())))
            .then_ignore(just('=').padded_by(ws()))
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws()))
            .map(|((name, idx), val)| Stmt::IndexAssign {
                name, index: Box::new(idx), value: val,
            });

        let kw_var_decl = kw_type.clone()
            .then(text::ident().padded_by(ws()))
            .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(|((ty, name), init)| Stmt::VarDecl { ty, name, init });

        // type_parser() capte fn(T)->T, les types génériques, etc.
        let generic_var_decl = type_parser()
            .then(text::ident().padded_by(ws()))
            .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(|((ty, name), init)| Stmt::VarDecl { ty, name, init });

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
            kw_var_decl, index_assign, field_assign, generic_var_decl, assign_stmt, expr_stmt,
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

        let method = ty.clone()
            .then(text::ident().padded_by(ws()))
            .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(method_body.clone())
            .map(|(((rt, n), p), b)| CM::M(Method { return_type: rt, name: n, params: p, body: b }));

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

    let enum_method = ty.clone()
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then(method_body.clone())
        .map(|(((rt, n), p), b)| Method { return_type: rt, name: n, params: p, body: b });

    let enum_type_params = text::ident().padded_by(ws())
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    let enum_def = kw("enum")
        .ignore_then(text::ident().padded_by(ws()))
        .then(enum_type_params)
        .then(
            enum_variant.separated_by(just(',').padded_by(ws())).allow_trailing()
                .then(just(';').padded_by(ws()).ignore_then(enum_method.repeated())
                    .or_not().map(|v| v.unwrap_or_default()))
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()))
        )
        .map(|((name, type_params), (variants, methods))| EnumDef { name, type_params, variants, methods });

    // ── Interface ─────────────────────────────────────────────────────────────

    let method_sig = ty.clone()
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then_ignore(just(';').padded_by(ws()))
        .map(|((rt, n), p)| MethodSig { return_type: rt, name: n, params: p });

    // ── Classe ────────────────────────────────────────────────────────────────

    let type_param_list = text::ident().padded_by(ws())
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    let interface_def = kw("interface")
        .ignore_then(text::ident().padded_by(ws()))
        .then(type_param_list.clone())
        .then(method_sig.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|((name, tp), methods)| InterfaceDef { name, type_params: tp, methods });

    let class_def = kw("class")
        .ignore_then(text::ident().padded_by(ws()))
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
        .map(|((((name, tp), parent), impls), members)| {
            let mut fields = vec![]; let mut ctors = vec![]; let mut methods = vec![];
            for m in members { match m {
                CM::F(f) => fields.push(f),
                CM::C(c) => ctors.push(c),
                CM::M(m) => methods.push(m),
            }}
            ClassDef { name, type_params: tp, parent, implements: impls,
                       fields, constructors: ctors, methods }
        });

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
        Iface(InterfaceDef),
        Enum(EnumDef),
        Class(ClassDef),
        Func(FuncDef),
    }

    // Fonction de haut niveau : `Type name(params) { body }`
    // On exclut "main" pour que main_func le prenne en charge séparément.
    let func_def = ty.clone()
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
        .map(|(((rt, name), p), b)| FuncDef { return_type: rt, name, params: p, body: b });

    let top_decl = choice((
        package_decl.map(TopDecl::Package),
        import_decl.map(TopDecl::Import),
        type_alias.map(TopDecl::Alias),
        interface_def.map(TopDecl::Iface),
        enum_def.map(TopDecl::Enum),
        class_def.map(TopDecl::Class),
        func_def.map(TopDecl::Func),
    ));

    ws()
        .ignore_then(top_decl.repeated())
        .then(main_func)
        .then_ignore(ws())
        .then_ignore(end())
        .map(|(decls, main)| {
            let mut pkg = None;
            let mut imports = vec![];
            let mut aliases = vec![]; let mut ifaces = vec![];
            let mut enums = vec![]; let mut classes = vec![];
            let mut funcs = vec![];
            for d in decls { match d {
                TopDecl::Package(p)  => { if pkg.is_none() { pkg = Some(p); } }
                TopDecl::Import(i)   => imports.push(i),
                TopDecl::Alias(a)    => aliases.push(a),
                TopDecl::Iface(i)    => ifaces.push(i),
                TopDecl::Enum(e)     => enums.push(e),
                TopDecl::Class(c)    => classes.push(c),
                TopDecl::Func(f)     => funcs.push(f),
            }}
            Program { package: pkg, imports, type_aliases: aliases,
                      interfaces: ifaces, enums, classes, funcs, main }
        })
}
