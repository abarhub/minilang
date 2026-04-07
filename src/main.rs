// ─────────────────────────────────────────────────────────────────────────────
//  main.rs – point d'entrée
//  RUST_LOG=info  cargo run -- example.mini
// ─────────────────────────────────────────────────────────────────────────────

// Les modules sont exposés via lib.rs.
use mini_parser::ast::*;
use mini_parser::interpreter::Interpreter;
use mini_parser::typechecker::TypeChecker;

use std::{env, fs, process};
use chumsky::Parser;
use log::{error, info};

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    ).init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { error!("Usage: {} <fichier.mini>", args[0]); process::exit(1); }
    let path = &args[1];
    info!("Lecture de '{}'", path);

    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        error!("Impossible de lire '{}' : {}", path, e); process::exit(1);
    });

    info!("Parsing...");
    let program = match mini_parser::parser::program_parser().parse(source.as_str()) {
        Ok(p)  => { info!("AST construit ✓"); p }
        Err(errors) => {
            for e in &errors { error!("Syntaxe : {}", e); }
            process::exit(1);
        }
    };

    print_program(&program);

    info!("Vérification des types...");
    let tc = TypeChecker::new(&program);
    let type_errors = tc.check(&program);
    if !type_errors.is_empty() {
        for e in &type_errors { error!("  {}", e); }
        process::exit(1);
    }
    info!("Types OK ✓");

    println!("\n{}\n  EXÉCUTION\n{}\n", "─".repeat(50), "─".repeat(50));
    let mut interp = Interpreter::new(&program);
    match interp.run(&program) {
        Ok(code) => { println!("\n{}", "─".repeat(50)); info!("Code de sortie : {}", code); }
        Err(e)   => { error!("{}", e); process::exit(1); }
    }
}

// ── Affichage de l'AST ────────────────────────────────────────────────────────

fn pad(d: usize) -> String { "  ".repeat(d) }

fn print_program(p: &Program) {
    println!("\n{}\n  AST\n{}", "─".repeat(50), "─".repeat(50));
    if let Some(pkg) = &p.package { println!("package {};", pkg.path); }
    for imp in &p.imports { println!("import {};", imp.path); }
    if p.package.is_some() || !p.imports.is_empty() { println!(); }
    for iface in &p.interfaces { print_interface(iface); println!(); }
    for e in &p.enums          { print_enum(e);          println!(); }
    for c in &p.classes        { print_class(c);         println!(); }
    println!("int main() {{");
    for s in &p.main.body { print_stmt(s, 1); }
    println!("}}");
    println!("{}", "─".repeat(50));
}

fn print_interface(i: &InterfaceDef) {
    println!("interface {} {{", i.name);
    for sig in &i.methods {
        let ps: Vec<String> = sig.params.iter().map(|p| format!("{} {}", p.ty, p.name)).collect();
        println!("{}  {} {}({});", pad(0), sig.return_type, sig.name, ps.join(", "));
    }
    println!("}}");
}

fn print_enum(e: &EnumDef) {
    println!("enum {} {{", e.name);
    let variants: Vec<String> = e.variants.iter().map(|v| {
        if v.fields.is_empty() { v.name.clone() }
        else {
            let fs: Vec<String> = v.fields.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
            format!("{}({})", v.name, fs.join(", "))
        }
    }).collect();
    println!("{}  {}", pad(0), variants.join(", "));
    for m in &e.methods { println!(); print_method(m, 1); }
    println!("}}");
}

fn print_class(c: &ClassDef) {
    let tps = if c.type_params.is_empty() { String::new() }
              else { format!("<{}>", c.type_params.join(", ")) };
    let ext = c.parent.as_deref().map(|p| format!(" extends {}", p)).unwrap_or_default();
    let imp = if c.implements.is_empty() { String::new() }
              else { format!(" implements {}", c.implements.join(", ")) };
    println!("class {}{}{}{} {{", c.name, tps, ext, imp);
    for f in &c.fields { println!("{}  {} {};", pad(0), f.ty, f.name); }
    for ctor in &c.constructors {
        let ps: Vec<String> = ctor.params.iter().map(|p| format!("{} {}", p.ty, p.name)).collect();
        println!(); println!("{}  {}({}) {{", pad(0), c.name, ps.join(", "));
        for s in &ctor.body { print_stmt(s, 2); }
        println!("{}  }}", pad(0));
    }
    for m in &c.methods { println!(); print_method(m, 1); }
    println!("}}");
}

fn print_method(m: &Method, depth: usize) {
    let ps: Vec<String> = m.params.iter().map(|p| format!("{} {}", p.ty, p.name)).collect();
    println!("{}{} {}({})", pad(depth), m.return_type, m.name, ps.join(", "));
    println!("{}{{", pad(depth));
    for s in &m.body { print_stmt(s, depth + 1); }
    println!("{}}}", pad(depth));
}

fn print_stmt(s: &Stmt, d: usize) {
    match s {
        Stmt::VarDecl { ty, name, init } => {
            let rhs = init.as_ref().map(|e| format!(" = {}", fmt_expr(e))).unwrap_or_default();
            println!("{}{} {}{};", pad(d), ty, name, rhs);
        }
        Stmt::Assign { target, value }             => println!("{}{} = {};",    pad(d), target, fmt_expr(value)),
        Stmt::FieldAssign { object, field, value } => println!("{}{}.{} = {};", pad(d), object, field, fmt_expr(value)),
        Stmt::Print(args) => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            println!("{}print({});", pad(d), a.join(", "));
        }
        Stmt::Return(None)    => println!("{}return;", pad(d)),
        Stmt::Return(Some(e)) => println!("{}return {};", pad(d), fmt_expr(e)),
        Stmt::ExprStmt(e)     => println!("{}{};", pad(d), fmt_expr(e)),
        Stmt::Break            => println!("{}break;", pad(d)),
        Stmt::Continue         => println!("{}continue;", pad(d)),
        Stmt::If { condition, then_body, else_body } => {
            println!("{}if ({}) {{", pad(d), fmt_expr(condition));
            for s in then_body { print_stmt(s, d + 1); }
            if let Some(eb) = else_body {
                println!("{}}} else {{", pad(d));
                for s in eb { print_stmt(s, d + 1); }
            }
            println!("{}}}", pad(d));
        }
        Stmt::While { condition, body } => {
            println!("{}while ({}) {{", pad(d), fmt_expr(condition));
            for s in body { print_stmt(s, d + 1); }
            println!("{}}}", pad(d));
        }
        Stmt::DoWhile { body, condition } => {
            println!("{}do {{", pad(d));
            for s in body { print_stmt(s, d + 1); }
            println!("{}}} while ({});", pad(d), fmt_expr(condition));
        }
        Stmt::For { init, condition, update, body } => {
            let i = init.as_deref().map(fmt_stmt_inline).unwrap_or_default();
            let c = condition.as_ref().map(fmt_expr).unwrap_or_default();
            let u = update.as_deref().map(fmt_stmt_inline).unwrap_or_default();
            println!("{}for ({}; {}; {}) {{", pad(d), i, c, u);
            for s in body { print_stmt(s, d + 1); }
            println!("{}}}", pad(d));
        }
        Stmt::Match { expr, arms } => {
            println!("{}match {} {{", pad(d), fmt_expr(expr));
            for arm in arms {
                let pat = match &arm.pattern {
                    Pattern::Wildcard => "_".to_string(),
                    Pattern::Variant { name, bindings } => {
                        if bindings.is_empty() { name.clone() }
                        else { format!("{}({})", name, bindings.join(", ")) }
                    }
                };
                println!("{}  {} => {{", pad(d), pat);
                for s in &arm.body { print_stmt(s, d + 2); }
                println!("{}  }}", pad(d));
            }
            println!("{}}}", pad(d));
        }
    }
}

fn fmt_stmt_inline(s: &Stmt) -> String {
    match s {
        Stmt::VarDecl { ty, name, init } => {
            if let Some(e) = init { format!("{} {} = {}", ty, name, fmt_expr(e)) }
            else { format!("{} {}", ty, name) }
        }
        Stmt::Assign { target, value } => format!("{} = {}", target, fmt_expr(value)),
        Stmt::ExprStmt(e)              => fmt_expr(e),
        _ => "...".to_string(),
    }
}

fn fmt_expr(e: &Expr) -> String {
    match e {
        Expr::IntLit(n)    => n.to_string(),
        Expr::FloatLit(f)  => f.to_string(),
        Expr::BoolLit(b)   => b.to_string(),
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::Ident(n)     => n.clone(),
        Expr::UnaryOp { op, expr } => {
            let s = match op { UnaryOp::Neg => "-", UnaryOp::Not => "!" };
            format!("{}{}", s, fmt_expr(expr))
        }
        Expr::BinOp { left, op, right } =>
            format!("({} {} {})", fmt_expr(left), op, fmt_expr(right)),
        Expr::FieldAccess { object, field } =>
            format!("{}.{}", fmt_expr(object), field),
        Expr::MethodCall { object, method, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}.{}({})", fmt_expr(object), method, a.join(", "))
        }
        Expr::FunctionCall { name, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}({})", name, a.join(", "))
        }
        Expr::New { class_name, type_args, args } => {
            let ta = if type_args.is_empty() { String::new() }
                     else { format!("<{}>", type_args.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")) };
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("new {}{}({})", class_name, ta, a.join(", "))
        }
        Expr::EnumConstructor { enum_name, variant, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            if a.is_empty() { format!("{}::{}", enum_name, variant) }
            else { format!("{}::{}({})", enum_name, variant, a.join(", ")) }
        }
        Expr::Lambda { params, body } => {
            let ps = if params.len() == 1 {
                params[0].clone()
            } else {
                format!("({})", params.join(", "))
            };
            let b = match body {
                LambdaBody::Expr(e)     => fmt_expr(e),
                LambdaBody::Block(_)    => "{ ... }".to_string(),
            };
            format!("{} => {}", ps, b)
        }
        Expr::LambdaCall { callee, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}({})", fmt_expr(callee), a.join(", "))
        }
    }
}
