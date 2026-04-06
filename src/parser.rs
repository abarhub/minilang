// ─────────────────────────────────────────────────────────────────────────────
//  Parser – chumsky 0.9
// ─────────────────────────────────────────────────────────────────────────────

use chumsky::prelude::*;
use log::debug;

use crate::ast::*;

// ── Whitespace + commentaires ─────────────────────────────────────────────────

fn ws() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just("//")
        .then(none_of('\n').repeated())
        .ignored()
        .or(filter(|c: &char| c.is_whitespace()).ignored())
        .repeated()
        .ignored()
}

// ── Keyword helper (évite le problème de lifetime de text::keyword) ───────────

fn kw(word: &'static str) -> impl Parser<char, (), Error = Simple<char>> + Clone {
    text::ident::<char, Simple<char>>()
        .try_map(move |ident: String, span| {
            if ident == word { Ok(()) }
            else { Err(Simple::custom(span, format!("expected keyword '{}'", word))) }
        })
        .padded_by(ws())
}

// ── Type ─────────────────────────────────────────────────────────────────────

fn type_parser() -> impl Parser<char, Type, Error = Simple<char>> + Clone {
    recursive(|ty| {
        let generic_args = ty.clone()
            .separated_by(just(',').padded_by(ws()))
            .at_least(1)
            .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()));

        let base = choice((
            kw("int")   .to(Type::Int),
            kw("bool")  .to(Type::Bool),
            kw("string").to(Type::Str),
            kw("float") .to(Type::Float),
            kw("double").to(Type::Double),
            kw("void")  .to(Type::Void),
            text::ident().padded_by(ws())
                .then(generic_args.or_not())
                .map(|(name, args)| match args {
                    Some(a) => Type::Generic(name, a),
                    None    => Type::UserDefined(name),
                }),
        ));

        base.then(
            just('[').padded_by(ws())
                .then(just(']').padded_by(ws()))
                .repeated(),
        )
        .map(|(t, v)| v.into_iter().fold(t, |acc, _| Type::Array(Box::new(acc))))
    })
}

// ── Point d'entrée ────────────────────────────────────────────────────────────

pub fn program_parser() -> impl Parser<char, Program, Error = Simple<char>> {
    let ty = type_parser();

    // ── Paramètres ────────────────────────────────────────────────────────────

    let param = ty.clone()
        .then(text::ident().padded_by(ws()))
        .map(|(ty, name)| Param { ty, name });

    let params = param.clone()
        .separated_by(just(',').padded_by(ws()))
        .allow_trailing();

    // ── Expressions ───────────────────────────────────────────────────────────

    let expr: BoxedParser<char, Expr, Simple<char>> = recursive(|expr| {
        let call_args = expr.clone()
            .separated_by(just(',').padded_by(ws()))
            .allow_trailing()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()));

        // ── atom ──────────────────────────────────────────────────────────────

        let string_lit = just('"')
            .ignore_then(none_of('"').repeated().collect::<String>())
            .then_ignore(just('"'))
            .map(Expr::StringLit)
            .padded_by(ws());

        let float_lit = text::int(10)
            .then_ignore(just('.'))
            .then(text::int(10))
            .map(|(i, f): (String, String)| {
                Expr::FloatLit(format!("{}.{}", i, f).parse().expect("float"))
            })
            .padded_by(ws());

        let int_lit = text::int(10)
            .map(|s: String| Expr::IntLit(s.parse().expect("int")))
            .padded_by(ws());

        let bool_lit = choice((
            kw("true") .to(Expr::BoolLit(true)),
            kw("false").to(Expr::BoolLit(false)),
        ));

        // new Foo<T>(args)
        let new_type_args = type_parser()
            .separated_by(just(',').padded_by(ws()))
            .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()));

        let new_expr = kw("new")
            .ignore_then(text::ident().padded_by(ws()))
            .then(new_type_args.or_not().map(|v| v.unwrap_or_default()))
            .then(call_args.clone())
            .map(|((class_name, type_args), args)| Expr::New { class_name, type_args, args });

        // EnumName::Variant  ou  EnumName::Variant(args)
        // doit être essayé AVANT ident_or_call
        let enum_ctor = text::ident()
            .padded_by(ws())
            .then_ignore(just("::").padded_by(ws()))
            .then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not().map(|v| v.unwrap_or_default()))
            .map(|((enum_name, variant), args)| {
                Expr::EnumConstructor { enum_name, variant, args }
            });

        let this_kw = kw("this").to(Expr::Ident("this".to_string()));

        let ident_or_call = text::ident()
            .padded_by(ws())
            .then(call_args.clone().or_not())
            .map(|(name, maybe_args)| match maybe_args {
                Some(args) => Expr::FunctionCall { name, args },
                None       => Expr::Ident(name),
            });

        let paren = expr.clone()
            .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()));

        let atom = choice((
            string_lit,
            float_lit,
            int_lit,
            bool_lit,
            this_kw,
            new_expr,
            enum_ctor,     // avant ident_or_call (commence par IDENT::)
            ident_or_call,
            paren,
        ));

        // ── postfix ───────────────────────────────────────────────────────────

        let postfix_op = just('.')
            .padded_by(ws())
            .ignore_then(text::ident().padded_by(ws()))
            .then(call_args.clone().or_not());

        let postfix = atom
            .then(postfix_op.repeated())
            .foldl(|obj, (name, maybe_args)| match maybe_args {
                Some(args) => Expr::MethodCall { object: Box::new(obj), method: name, args },
                None       => Expr::FieldAccess { object: Box::new(obj), field: name },
            });

        // ── pow ───────────────────────────────────────────────────────────────
        let pow = postfix.clone()
            .then(just("**").padded_by(ws()).to(BinOp::Pow).then(postfix.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── unary ─────────────────────────────────────────────────────────────
        let unary = recursive(|unary| {
            choice((
                just('-').padded_by(ws()).ignore_then(unary.clone())
                    .map(|e| Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(e) }),
                just('!').padded_by(ws()).ignore_then(unary.clone())
                    .map(|e| Expr::UnaryOp { op: UnaryOp::Not, expr: Box::new(e) }),
                pow.clone(),
            ))
        });

        // ── mul ───────────────────────────────────────────────────────────────
        let mul_op = choice((
            just('%').padded_by(ws()).to(BinOp::Mod),
            just('/').padded_by(ws()).to(BinOp::Div),
            just('*').padded_by(ws()).to(BinOp::Mul),
        ));
        let mul = unary.clone()
            .then(mul_op.then(unary.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── add ───────────────────────────────────────────────────────────────
        let add_op = choice((
            just('+').padded_by(ws()).to(BinOp::Add),
            just('-').padded_by(ws()).to(BinOp::Sub),
        ));
        let add = mul.clone()
            .then(add_op.then(mul.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── rel ───────────────────────────────────────────────────────────────
        let rel_op = choice((
            just("<=").padded_by(ws()).to(BinOp::Le),
            just(">=").padded_by(ws()).to(BinOp::Ge),
            just('<').padded_by(ws()).to(BinOp::Lt),
            just('>').padded_by(ws()).to(BinOp::Gt),
        ));
        let rel = add.clone()
            .then(rel_op.then(add.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── eq ────────────────────────────────────────────────────────────────
        let eq_op = choice((
            just("==").padded_by(ws()).to(BinOp::Eq),
            just("!=").padded_by(ws()).to(BinOp::Ne),
        ));
        let eq = rel.clone()
            .then(eq_op.then(rel.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        // ── and / or ──────────────────────────────────────────────────────────
        let and = eq.clone()
            .then(just("&&").padded_by(ws()).to(BinOp::And).then(eq.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) });

        and.clone()
            .then(just("||").padded_by(ws()).to(BinOp::Or).then(and.clone()).repeated())
            .foldl(|l, (op, r)| Expr::BinOp { left: Box::new(l), op, right: Box::new(r) })
            .boxed()
    })
    .boxed();

    // ── Type primitif (pour déclarations de variable) ─────────────────────────

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

    let stmt: BoxedParser<char, Stmt, Simple<char>> = recursive(|stmt| {
        let body = stmt.clone()
            .repeated()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

        // ── print ─────────────────────────────────────────────────────────────
        let print_stmt = kw("print")
            .ignore_then(
                expr.clone()
                    .separated_by(just(',').padded_by(ws()))
                    .allow_trailing()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())),
            )
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::Print);

        // ── return ────────────────────────────────────────────────────────────
        let return_stmt = kw("return")
            .ignore_then(expr.clone().or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::Return);

        // ── break / continue ──────────────────────────────────────────────────
        let break_stmt    = kw("break")   .then_ignore(just(';').padded_by(ws())).to(Stmt::Break);
        let continue_stmt = kw("continue").then_ignore(just(';').padded_by(ws())).to(Stmt::Continue);

        // ── if ────────────────────────────────────────────────────────────────
        let if_stmt = kw("if")
            .ignore_then(
                expr.clone()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())),
            )
            .then(body.clone())
            .then(
                kw("else")
                    .ignore_then(stmt.clone().map(|s| vec![s]).or(body.clone()))
                    .or_not(),
            )
            .map(|((condition, then_body), else_body)| {
                Stmt::If { condition, then_body, else_body }
            });

        // ── while ─────────────────────────────────────────────────────────────
        let while_stmt = kw("while")
            .ignore_then(
                expr.clone()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())),
            )
            .then(body.clone())
            .map(|(condition, body)| Stmt::While { condition, body });

        // ── do-while ──────────────────────────────────────────────────────────
        let do_while = kw("do")
            .ignore_then(body.clone())
            .then_ignore(kw("while"))
            .then(
                expr.clone()
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())),
            )
            .then_ignore(just(';').padded_by(ws()))
            .map(|(body, condition)| Stmt::DoWhile { body, condition });

        // ── for ───────────────────────────────────────────────────────────────
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
                .map(|(target, value)| Box::new(Stmt::Assign { target, value })),
        ));

        let for_update = choice((
            text::ident().padded_by(ws())
                .then_ignore(just('.').padded_by(ws()))
                .then(text::ident().padded_by(ws()))
                .then_ignore(just('=').padded_by(ws()))
                .then(expr.clone())
                .map(|((object, field), value)| Box::new(Stmt::FieldAssign { object, field, value })),
            text::ident().padded_by(ws())
                .then_ignore(just('=').padded_by(ws()))
                .then(expr.clone())
                .map(|(target, value)| Box::new(Stmt::Assign { target, value })),
            expr.clone().map(|e| Box::new(Stmt::ExprStmt(e))),
        ));

        let for_stmt = kw("for")
            .ignore_then(
                for_init.or_not()
                    .then_ignore(just(';').padded_by(ws()))
                    .then(expr.clone().or_not())
                    .then_ignore(just(';').padded_by(ws()))
                    .then(for_update.or_not())
                    .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())),
            )
            .then(body.clone())
            .map(|(((init, condition), update), body)| {
                Stmt::For { init, condition, update, body }
            });

        // ── match ─────────────────────────────────────────────────────────────
        //
        //  match expr {
        //      EnumName::Variant(x, y) => { stmts }
        //      EnumName::Variant       => { stmts }
        //      _                       => { stmts }
        //  }

        // Pattern : "EnumName::Variant(bindings...)"  |  "EnumName::Variant"  |  "_"
        let pattern = choice((
            // wildcard
            just('_').padded_by(ws()).to(Pattern::Wildcard),
            // variant avec ou sans bindings
            text::ident()
                .padded_by(ws())
                .then_ignore(just("::").padded_by(ws()))
                .then(text::ident().padded_by(ws()))
                .then(
                    text::ident()
                        .padded_by(ws())
                        .separated_by(just(',').padded_by(ws()))
                        .allow_trailing()
                        .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                        .or_not()
                        .map(|v| v.unwrap_or_default()),
                )
                .map(|((_enum_name, name), bindings)| Pattern::Variant { name, bindings }),
        ));

        let match_arm = pattern
            .then_ignore(just("=>").padded_by(ws()))
            .then(body.clone())
            .map(|(pattern, body)| MatchArm { pattern, body });

        let match_stmt = kw("match")
            .ignore_then(expr.clone())
            .then(
                match_arm
                    .repeated()
                    .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())),
            )
            .map(|(e, arms)| {
                debug!("match ({} bras)", arms.len());
                Stmt::Match { expr: e, arms }
            });

        // ── Déclarations de variable ──────────────────────────────────────────

        let kw_var_decl = kw_type.clone()
            .then(text::ident().padded_by(ws()))
            .then(just('=').padded_by(ws()).ignore_then(expr.clone()).or_not())
            .then_ignore(just(';').padded_by(ws()))
            .map(|((ty, name), init)| Stmt::VarDecl { ty, name, init });

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
            .map(|((object, field), value)| Stmt::FieldAssign { object, field, value });

        let assign_stmt = text::ident().padded_by(ws())
            .then_ignore(just('=').padded_by(ws()))
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws()))
            .map(|(target, value)| Stmt::Assign { target, value });

        let expr_stmt = expr.clone()
            .then_ignore(just(';').padded_by(ws()))
            .map(Stmt::ExprStmt);

        choice((
            print_stmt,
            return_stmt,
            break_stmt,
            continue_stmt,
            if_stmt,
            while_stmt,
            do_while,
            for_stmt,
            match_stmt,
            kw_var_decl,
            field_assign,
            generic_var_decl,
            assign_stmt,
            expr_stmt,
        ))
        .padded_by(ws())
        .boxed()
    })
    .boxed();

    // ── Corps { stmts } ───────────────────────────────────────────────────────

    let body = stmt.clone()
        .repeated()
        .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

    // ── Membres de classe ─────────────────────────────────────────────────────

    enum ClassMember { Field(Field), Constructor(Constructor), Method(Method) }

    let class_member = {
        let constructor = text::ident().padded_by(ws())
            .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(body.clone())
            .map(|((_name, params), body)| ClassMember::Constructor(Constructor { params, body }));

        let method = ty.clone()
            .then(text::ident().padded_by(ws()))
            .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
            .then(body.clone())
            .map(|(((return_type, name), params), body)| {
                ClassMember::Method(Method { return_type, name, params, body })
            });

        let field = ty.clone()
            .then(text::ident().padded_by(ws()))
            .then_ignore(just(';').padded_by(ws()))
            .map(|(ty, name)| ClassMember::Field(Field { ty, name }));

        choice((constructor, method, field))
    };

    // ── Définition d'enum ─────────────────────────────────────────────────────
    //
    //  enum Direction {
    //      North,
    //      Point(int x, int y);           ← variante avec champs
    //      string name() { ... }          ← méthode
    //  }
    //
    //  Séparateur entre variantes : virgule.
    //  Séparateur entre variantes et méthodes : ';' (point-virgule facultatif).

    // Variante : IDENT  ou  IDENT(params)
    let enum_variant = text::ident()
        .padded_by(ws())
        .then(
            param.clone()
                .separated_by(just(',').padded_by(ws()))
                .allow_trailing()
                .delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws()))
                .or_not()
                .map(|v| v.unwrap_or_default()),
        )
        .map(|(name, fields)| EnumVariant { name, fields });

    // Méthode d'enum (même syntaxe que méthode de classe)
    let enum_method = ty.clone()
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then(body.clone())
        .map(|(((return_type, name), params), body)| Method { return_type, name, params, body });

    // Contenu d'un enum :
    //   variant, variant, ... ;   method method ...
    // Le ';' sépare les variantes des méthodes (facultatif s'il n'y a pas de méthodes).
    let enum_body = enum_variant
        .separated_by(just(',').padded_by(ws()))
        .allow_trailing()
        .then(
            just(';').padded_by(ws())
                .ignore_then(enum_method.repeated())
                .or_not()
                .map(|v| v.unwrap_or_default()),
        )
        .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

    let enum_def = kw("enum")
        .ignore_then(text::ident().padded_by(ws()))
        .then(enum_body)
        .map(|(name, (variants, methods))| {
            debug!("enum '{}' ({} variantes, {} méthodes)", name, variants.len(), methods.len());
            EnumDef { name, variants, methods }
        });

    // ── Interface ─────────────────────────────────────────────────────────────

    let method_sig = ty.clone()
        .then(text::ident().padded_by(ws()))
        .then(params.clone().delimited_by(just('(').padded_by(ws()), just(')').padded_by(ws())))
        .then_ignore(just(';').padded_by(ws()))
        .map(|((return_type, name), params)| MethodSig { return_type, name, params });

    let interface_def = kw("interface")
        .ignore_then(text::ident().padded_by(ws()))
        .then(
            method_sig.repeated()
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())),
        )
        .map(|(name, methods)| { debug!("interface '{}'", name); InterfaceDef { name, methods } });

    // ── Classe ────────────────────────────────────────────────────────────────

    let type_param_list = text::ident()
        .padded_by(ws())
        .separated_by(just(',').padded_by(ws()))
        .at_least(1)
        .delimited_by(just('<').padded_by(ws()), just('>').padded_by(ws()))
        .or_not()
        .map(|v| v.unwrap_or_default());

    let class_def = kw("class")
        .ignore_then(text::ident().padded_by(ws()))
        .then(type_param_list)
        .then(kw("extends").ignore_then(text::ident().padded_by(ws())).or_not())
        .then(
            kw("implements")
                .ignore_then(
                    text::ident().padded_by(ws())
                        .separated_by(just(',').padded_by(ws()))
                        .at_least(1),
                )
                .or_not()
                .map(|v| v.unwrap_or_default()),
        )
        .then(
            class_member.repeated()
                .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())),
        )
        .map(|((((name, type_params), parent), implements), members)| {
            debug!("class '{}'", name);
            let mut fields = vec![];
            let mut constructors = vec![];
            let mut methods = vec![];
            for m in members {
                match m {
                    ClassMember::Field(f)       => fields.push(f),
                    ClassMember::Constructor(c) => constructors.push(c),
                    ClassMember::Method(m)      => methods.push(m),
                }
            }
            ClassDef { name, type_params, parent, implements, fields, constructors, methods }
        });

    // ── main ──────────────────────────────────────────────────────────────────

    let main_func = kw("int")
        .ignore_then(kw("main"))
        .ignore_then(just('(').padded_by(ws()).then(just(')').padded_by(ws())))
        .ignore_then(body)
        .map(|stmts| { debug!("main ({} stmts)", stmts.len()); MainFunc { body: stmts } });

    // ── Package & imports ─────────────────────────────────────────────────────

    let dot_ident = just('.').ignore_then(text::ident());
    let dotted_path = text::ident()
        .then(dot_ident.repeated())
        .map(|(h, t)| { let mut p = vec![h]; p.extend(t); p.join(".") });

    let package_decl = kw("package")
        .ignore_then(dotted_path.clone().padded_by(ws()))
        .then_ignore(just(';').padded_by(ws()))
        .map(|path| PackageDecl { path });

    let import_path = text::ident()
        .then(
            just('.').ignore_then(text::ident().or(just('*').to("*".to_string())))
                .repeated()
                .at_least(1),
        )
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
        .then(interface_def.repeated())
        .then(enum_def.repeated())
        .then(class_def.repeated())
        .then(main_func)
        .then_ignore(ws())
        .then_ignore(end())
        .map(|(((((package, imports), interfaces), enums), classes), main)| {
            debug!(
                "Programme: pkg={:?} enums={} classes={}",
                package, enums.len(), classes.len()
            );
            Program { package, imports, interfaces, enums, classes, main }
        })
}
