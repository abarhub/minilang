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
        .map(|(t, v)| v.into_iter().fold(t, |acc, _| Type::Array(Box::new(acc))))
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

        let str_lit = just('"')
            .ignore_then(none_of('"').repeated().collect::<String>())
            .then_ignore(just('"'))
            .map(Expr::StringLit).padded_by(ws());

        let float_lit = text::int(10).then_ignore(just('.')).then(text::int(10))
            .map(|(i, f): (String, String)|
                Expr::FloatLit(format!("{}.{}", i, f).parse().unwrap()))
            .padded_by(ws());

        let int_lit = text::int(10)
            .map(|s: String| Expr::IntLit(s.parse().unwrap()))
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

        let enum_ctor = text::ident().padded_by(ws())
            .then_ignore(just("::").padded_by(ws()))
            .then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not().map(|v| v.unwrap_or_default()))
            .map(|((en, v), a)| Expr::EnumConstructor { enum_name: en, variant: v, args: a });

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

        let atom = choice((
            str_lit, float_lit, int_lit, bool_lit,
            this_kw, new_expr, enum_ctor, ident_or_call,
            paren_or_call,
        ));

        // Postfix : .field  .method(args)
        #[derive(Clone)]
        enum Postfix { Field(String), Method(String, Vec<Expr>) }

        let postfix_op = just('.').padded_by(ws())
            .ignore_then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not())
            .map(|(name, args)| match args {
                Some(a) => Postfix::Method(name, a),
                None    => Postfix::Field(name),
            });

        let postfix = atom
            .then(postfix_op.repeated())
            .foldl(|obj, pf| match pf {
                Postfix::Field(f)     => Expr::FieldAccess { object: Box::new(obj), field: f },
                Postfix::Method(m, a) => Expr::MethodCall  { object: Box::new(obj), method: m, args: a },
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

        choice((lambda_multi, lambda_single, arith)).boxed()
    }).boxed();

    // ── Type primitif pour les déclarations de variable ───────────────────────

    let kw_type = choice((
        kw("int")   .to(Type::Int),
        kw("bool")  .to(Type::Bool),
        kw("string").to(Type::Str),
        kw("float") .to(Type::Float),
        kw("double").to(Type::Double),
    ))
    .then(just('[').padded_by(ws()).then(just(']').padded_by(ws())).repeated())
    .map(|(t, v)| v.into_iter().fold(t, |acc, _| Type::Array(Box::new(acc))));

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
            if_stmt, while_stmt, do_while, for_stmt, match_stmt,
            kw_var_decl, field_assign, generic_var_decl, assign_stmt, expr_stmt,
        ))
        .padded_by(ws())
        .boxed()
    }).boxed();

    stmt_fwd.define(stmt_impl);
    let stmt = stmt_fwd;

    let body = stmt.clone().repeated()
        .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

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
            .then(body.clone())
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
        .then(body.clone())
        .map(|(((rt, n), p), b)| Method { return_type: rt, name: n, params: p, body: b });

    let enum_def = kw("enum")
        .ignore_then(text::ident().padded_by(ws()))
        .then(
            enum_variant.separated_by(just(',').padded_by(ws())).allow_trailing()
                .then(just(';').padded_by(ws()).ignore_then(enum_method.repeated())
                    .or_not().map(|v| v.unwrap_or_default()))
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()))
        )
        .map(|(name, (variants, methods))| EnumDef { name, variants, methods });

    // ── Interface ─────────────────────────────────────────────────────────────

    let method_sig = ty.clone()
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then_ignore(just(';').padded_by(ws()))
        .map(|((rt, n), p)| MethodSig { return_type: rt, name: n, params: p });

    let interface_def = kw("interface")
        .ignore_then(text::ident().padded_by(ws()))
        .then(method_sig.repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())))
        .map(|(name, methods)| InterfaceDef { name, methods });

    // ── Classe ────────────────────────────────────────────────────────────────

    let type_param_list = text::ident().padded_by(ws())
        .separated_by(just(',').padded_by(ws())).at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not().map(|v| v.unwrap_or_default());

    let class_def = kw("class")
        .ignore_then(text::ident().padded_by(ws()))
        .then(type_param_list)
        .then(kw("extends").ignore_then(text::ident().padded_by(ws())).or_not())
        .then(kw("implements")
            .ignore_then(text::ident().padded_by(ws())
                .separated_by(just(',').padded_by(ws())).at_least(1))
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
        .ignore_then(body)
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

    ws()
        .ignore_then(package_decl.or_not())
        .then(import_decl.repeated())
        .then(type_alias.repeated())       // ← alias avant interfaces
        .then(interface_def.repeated())
        .then(enum_def.repeated())
        .then(class_def.repeated())
        .then(main_func)
        .then_ignore(ws())
        .then_ignore(end())
        .map(|((((((pkg, imp), aliases), ifaces), enums), classes), main)| Program {
            package: pkg, imports: imp, type_aliases: aliases,
            interfaces: ifaces, enums, classes, main,
        })
}
