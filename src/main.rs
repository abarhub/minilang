// ─────────────────────────────────────────────────────────────────────────────
//  main.rs – point d'entrée
//  RUST_LOG=info  cargo run -- example.mini
//
//  Configuration de projet optionnelle : un fichier `minilang.toml` est
//  cherché dans le répertoire du fichier source (ou le répertoire courant)
//  puis ses parents. Absent → configuration par défaut.
//  Priorité : CLI > minilang.toml > défauts.
// ─────────────────────────────────────────────────────────────────────────────

// Les modules sont exposés via lib.rs.
use mini_parser::ast::*;
use mini_parser::config;
use mini_parser::interpreter::Interpreter;
use mini_parser::typechecker::TypeChecker;

use std::{env, fs, path::{Path, PathBuf}, process};
use chumsky::Parser;
use log::{error, info};

/// Taille de pile du thread de travail. Le parser chumsky construit des stack
/// frames très profonds : en mode debug, la pile par défaut du thread principal
/// Windows (1 Mo) déborde dès le parsing. Les threads de test Rust (2 Mo)
/// passent ; 16 Mo donne une marge confortable, sans coût réel (mémoire
/// virtuelle réservée, engagée seulement à l'usage).
const STACK_SIZE: usize = 16 * 1024 * 1024;

fn main() {
    // Tout le travail s'exécute dans un thread à pile dimensionnée.
    let handle = std::thread::Builder::new()
        .name("minilang".to_string())
        .stack_size(STACK_SIZE)
        .spawn(real_main)
        .expect("impossible de créer le thread de travail");
    if handle.join().is_err() {
        // panic dans le thread de travail (déjà affiché sur stderr)
        process::exit(101);
    }
}

fn real_main() {
    let args: Vec<String> = env::args().collect();
    // Sous-commande : `mini_parser test [fichier|répertoire]`
    if args.get(1).map(|s| s.as_str()) == Some("test") {
        let exit_code = test_mode(args.get(2).map(|s| s.as_str()));
        process::exit(exit_code);
    }
    run_mode(&args);
}

// ── Chargement de la configuration ───────────────────────────────────────────

/// Charge le minilang.toml (optionnel) en partant de `start_dir`.
/// Le logger n'est pas encore initialisé à ce stade (le niveau peut venir de
/// la config) → les erreurs passent par eprintln!.
fn load_config_or_exit(start_dir: &Path) -> (config::ProjectConfig, Option<PathBuf>) {
    let loaded = config::load(start_dir).unwrap_or_else(|e| {
        eprintln!("Configuration invalide — {}", e); process::exit(1);
    });
    match loaded {
        Some((c, p)) => (c, Some(p)),
        None         => (config::ProjectConfig::default(), None),
    }
}

/// Logger : RUST_LOG > [runtime] log > `fallback`.
fn init_logger(cfg: &config::ProjectConfig, fallback: &str) {
    let default_log = cfg.runtime.log.as_deref().unwrap_or(fallback);
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(default_log),
    ).init();
}

/// Concatène les fichiers de [sources] include (relatifs au minilang.toml).
/// Préfixé au fichier exécuté, comme l'est la stdlib pour les tests Rust.
fn sources_prefix(cfg: &config::ProjectConfig, cfg_path: &Option<PathBuf>) -> String {
    let Some(includes) = &cfg.sources.include else { return String::new() };
    let base: PathBuf = cfg_path.as_ref()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let mut prefix = String::new();
    for inc in includes {
        let p = base.join(inc);
        match fs::read_to_string(&p) {
            Ok(s) => { prefix.push_str(&s); prefix.push('\n'); }
            Err(e) => {
                eprintln!("[sources] include — impossible de lire '{}' : {}", p.display(), e);
                process::exit(1);
            }
        }
    }
    prefix
}

// ── Mode run : exécute le main d'un programme ────────────────────────────────

fn run_mode(args: &[String]) {
    let cli_file: Option<&str> = args.get(1).map(|s| s.as_str());

    // Point de départ de la découverte : répertoire du fichier, sinon cwd
    let start_dir: PathBuf = match cli_file {
        Some(f) => Path::new(f).parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
        None => PathBuf::from("."),
    };
    let (cfg, cfg_path) = load_config_or_exit(&start_dir);
    init_logger(&cfg, "info");

    if let Some(p) = &cfg_path { info!("Configuration : {}", p.display()); }
    if let Some(name) = &cfg.project.name { info!("Projet '{}'", name); }

    // ── Fichier source : argument CLI, sinon [project] main ──────────────
    let path: PathBuf = match cli_file {
        Some(f) => PathBuf::from(f),
        None => match (&cfg.project.main, &cfg_path) {
            (Some(main), Some(cp)) =>
                cp.parent().unwrap_or(Path::new(".")).join(main),
            _ => {
                error!("Usage: {} <fichier.mini>  (ou [project] main dans {})",
                    args[0], config::CONFIG_FILE);
                process::exit(1);
            }
        },
    };
    info!("Lecture de '{}'", path.display());

    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        error!("Impossible de lire '{}' : {}", path.display(), e); process::exit(1);
    });
    // Stdlib + [sources] include préfixés au fichier exécuté — même
    // comportement que les API de test Rust (run_source / check_source)
    let prefix = format!("{}\n{}", mini_parser::STDLIB, sources_prefix(&cfg, &cfg_path));
    let full_source = format!("{}{}", prefix, source);

    info!("Parsing...");
    let mut program = match mini_parser::parser::program_parser().parse(full_source.as_str()) {
        Ok(p)  => { info!("AST construit ✓"); p }
        Err(errors) => {
            for e in &errors { error!("Syntaxe : {}", e); }
            process::exit(1);
        }
    };

    // ── Profil DI : [di] modules restreint les modules de binding actifs ──
    if let Some(active) = &cfg.di.modules {
        if let Err(e) = config::select_modules(&mut program, active) {
            error!("Configuration — {}", e);
            process::exit(1);
        }
        info!("Modules DI actifs : {}",
            if active.is_empty() { "aucun".to_string() } else { active.join(", ") });
    }
    let program = program;

    // ── Racines fichiers : [files.roots] (les répertoires doivent exister) ──
    let cfg_dir = cfg_path.as_ref().and_then(|p| p.parent());
    let file_roots = config::resolve_roots(&cfg.files, cfg_dir).unwrap_or_else(|e| {
        error!("Configuration [files] — {}", e);
        process::exit(1);
    });

    // AST : n'afficher que les déclarations du fichier utilisateur — on masque
    // celles du préfixe (stdlib + includes) en comptant ses déclarations.
    let skip = mini_parser::parser::program_parser().parse(prefix.as_str())
        .map(|p| DeclCounts::of(&p))
        .unwrap_or_default();
    print_program(&program, &skip);

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
    interp.set_file_roots(file_roots);
    let (mark, del) = match cfg.files.temp {
        config::TempCleanup::None   => (false, false),
        config::TempCleanup::Mark   => (true,  false),
        config::TempCleanup::Delete => (true,  true),
    };
    interp.set_temp_policy(mark, del);
    interp.set_files_unrestricted(cfg.files.unrestricted);
    match interp.run(&program) {
        Ok(code) => { println!("\n{}", "─".repeat(50)); info!("Code de sortie : {}", code); }
        Err(e)   => { error!("{}", e); process::exit(1); }
    }
}

// ── Mode test : exécute les fonctions `test` ─────────────────────────────────

/// Collecte récursivement les fichiers .mini d'un répertoire, triés.
fn collect_mini_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() { collect_mini_files(&p, out); }
        else if p.extension().map(|e| e == "mini").unwrap_or(false) { out.push(p); }
    }
}

/// `mini_parser test [fichier|répertoire]` — lance les tests et retourne le
/// code de sortie (0 = tout passe, 1 = au moins un échec ou une erreur).
fn test_mode(target: Option<&str>) -> i32 {
    // Point de départ de la découverte de la config
    let start_dir: PathBuf = match target {
        Some(t) if Path::new(t).is_dir() => PathBuf::from(t),
        Some(t) => Path::new(t).parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
        None => PathBuf::from("."),
    };
    let (cfg, cfg_path) = load_config_or_exit(&start_dir);
    // Sortie de test propre par défaut — RUST_LOG / [runtime] log prioritaires
    init_logger(&cfg, "warn");

    // ── Fichiers de tests ─────────────────────────────────────────────────
    let cfg_dir: PathBuf = cfg_path.as_ref()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let mut files: Vec<PathBuf> = vec![];
    match target {
        Some(t) if Path::new(t).is_file() => files.push(PathBuf::from(t)),
        Some(t) if Path::new(t).is_dir()  => collect_mini_files(Path::new(t), &mut files),
        Some(t) => { eprintln!("'{}' : fichier ou répertoire introuvable", t); return 1; }
        None => {
            let dir = cfg_dir.join(cfg.tests.dir.as_deref().unwrap_or("tests"));
            if !dir.is_dir() {
                eprintln!("Répertoire de tests introuvable : {} \
                           (configurez [tests] dir dans {})",
                    dir.display(), config::CONFIG_FILE);
                return 1;
            }
            collect_mini_files(&dir, &mut files);
        }
    }
    if files.is_empty() {
        eprintln!("Aucun fichier de test (.mini) trouvé");
        return 1;
    }

    // Profil DI des tests : [tests] modules > [di] modules > tous
    let active_modules = cfg.tests.modules.clone().or_else(|| cfg.di.modules.clone());
    // Stdlib + [sources] include, comme en mode run
    let prefix = format!("{}\n{}", mini_parser::STDLIB, sources_prefix(&cfg, &cfg_path));

    // ── Exécution ─────────────────────────────────────────────────────────
    let (mut total, mut failures, mut file_errors) = (0usize, 0usize, 0usize);
    for file in &files {
        println!("── {}", file.display());
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => { println!("  erreur : lecture impossible : {}", e);
                        file_errors += 1; continue; }
        };
        let full = format!("{}{}", prefix, source);
        let mut program = match mini_parser::parser::program_parser().parse(full.as_str()) {
            Ok(p) => p,
            Err(errors) => {
                for e in &errors { println!("  erreur de syntaxe : {}", e); }
                file_errors += 1; continue;
            }
        };
        if let Some(active) = &active_modules {
            if let Err(e) = config::select_modules(&mut program, active) {
                println!("  erreur de configuration : {}", e);
                file_errors += 1; continue;
            }
        }
        let type_errors = TypeChecker::new(&program).check(&program);
        if !type_errors.is_empty() {
            for e in &type_errors { println!("  {}", e); }
            file_errors += 1; continue;
        }
        let results = mini_parser::test_runner::run_tests(&program);
        if results.is_empty() {
            println!("  (aucune fonction test)");
        }
        for r in &results {
            total += 1;
            match &r.error {
                None    => println!("test {} ... ok", r.name),
                Some(e) => {
                    failures += 1;
                    println!("test {} ... ECHEC", r.name);
                    println!("    {}", e);
                }
            }
        }
    }

    // ── Bilan ─────────────────────────────────────────────────────────────
    println!();
    let mut bilan = format!("Résultat : {} test(s), {} échec(s)", total, failures);
    if file_errors > 0 { bilan.push_str(&format!(", {} fichier(s) en erreur", file_errors)); }
    println!("{}", bilan);
    if failures > 0 || file_errors > 0 { 1 } else { 0 }
}

// ── Affichage de l'AST ────────────────────────────────────────────────────────

fn pad(d: usize) -> String { "  ".repeat(d) }

/// Nombre de déclarations par catégorie. Les déclarations du préfixe
/// (stdlib + [sources] include) précèdent celles du fichier utilisateur dans
/// chaque Vec du Program — les compter permet de les masquer à l'affichage.
#[derive(Default)]
struct DeclCounts {
    imports: usize, aliases: usize, modules: usize, ifaces: usize,
    enums: usize, records: usize, classes: usize, funcs: usize,
}

impl DeclCounts {
    fn of(p: &Program) -> Self {
        DeclCounts {
            imports: p.imports.len(),
            aliases: p.type_aliases.len(),
            modules: p.modules.len(),
            ifaces:  p.interfaces.len(),
            enums:   p.enums.len(),
            records: p.records.len(),
            classes: p.classes.len(),
            funcs:   p.funcs.len(),
        }
    }
}

fn print_program(p: &Program, skip: &DeclCounts) {
    println!("\n{}\n  AST\n{}", "─".repeat(50), "─".repeat(50));
    let imports: Vec<_> = p.imports.iter().skip(skip.imports).collect();
    for imp in &imports { println!("import {};", imp.path); }
    if !imports.is_empty() { println!(); }
    let aliases: Vec<_> = p.type_aliases.iter().skip(skip.aliases).collect();
    for alias in &aliases { println!("type {} = {};", alias.name, alias.ty); }
    if !aliases.is_empty() { println!(); }
    for m in p.modules.iter().skip(skip.modules)         { print_module(m);        println!(); }
    for iface in p.interfaces.iter().skip(skip.ifaces)   { print_interface(iface); println!(); }
    for e in p.enums.iter().skip(skip.enums)             { print_enum(e);          println!(); }
    for c in p.classes.iter().skip(skip.classes)         { print_class(c);         println!(); }
    if let Some(main) = &p.main {
        println!("int main() {{");
        for s in &main.body { print_stmt(s, 1); }
        println!("}}");
    }
    println!("{}", "─".repeat(50));
}

fn print_module(m: &ModuleDef) {
    println!("module {} {{", m.name);
    for b in &m.binds {
        let to = b.to.as_deref().map(|t| format!(" to {}", t)).unwrap_or_default();
        let with = if b.with.is_empty() { String::new() }
                   else {
                       let a: Vec<String> = b.with.iter().map(fmt_expr).collect();
                       format!(" with ({})", a.join(", "))
                   };
        println!("{}  bind {}{}{};", pad(0), b.target, to, with);
    }
    println!("}}");
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
    let tr  = if c.is_transient { "transient " } else { "" };
    let svc = if c.is_service { "service " } else { "" };
    println!("{}{}class {}{}{}{} {{", tr, svc, c.name, tps, ext, imp);
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
    let mutable = if m.is_mutable { "mutable " } else { "" };
    println!("{}{}{}{} {}({})", pad(depth), m.visibility, mutable, m.return_type, m.name, ps.join(", "));
    println!("{}{{", pad(depth));
    for s in &m.body { print_stmt(s, depth + 1); }
    println!("{}}}", pad(depth));
}

fn print_stmt(s: &Stmt, d: usize) {
    match s {
        Stmt::VarDecl { qualifier, ty, name, init } => {
            let rhs = init.as_ref().map(|e| format!(" = {}", fmt_expr(e))).unwrap_or_default();
            println!("{}{}{} {}{};", pad(d), qualifier, ty, name, rhs);
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
        Stmt::Builtin          => println!("{}builtin;", pad(d)),
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
        Stmt::ForIn { var_type, var_name, iter_expr, body } => {
            println!("{}for ({} {} in {}) {{", pad(d), var_type, var_name, fmt_expr(iter_expr));
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
        Stmt::VarDecl { qualifier, ty, name, init } => {
            if let Some(e) = init { format!("{}{} {} = {}", qualifier, ty, name, fmt_expr(e)) }
            else { format!("{}{} {}", qualifier, ty, name) }
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
        Expr::CharLit(c)   => format!("'{}'", c),
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
        Expr::Inject(ty) => format!("inject {}", ty),
        Expr::EnumConstructor { enum_name, type_args, variant, args } => {
            let ta = if type_args.is_empty() { String::new() }
                     else { format!("<{}>", type_args.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")) };
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            if a.is_empty() { format!("{}{}::{}", enum_name, ta, variant) }
            else { format!("{}{}::{}({})", enum_name, ta, variant, a.join(", ")) }
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
        Expr::SafeFieldAccess { object, field } =>
            format!("{}?.{}", fmt_expr(object), field),
        Expr::SafeMethodCall { object, method, args } => {
            let a: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}?.{}({})", fmt_expr(object), method, a.join(", "))
        }
        Expr::NullCoalesce { expr, default } =>
            format!("({} ?? {})", fmt_expr(expr), fmt_expr(default)),
        Expr::ArrayLit { elem_type, elements } => {
            let es: Vec<String> = elements.iter().map(fmt_expr).collect();
            format!("new {}[]{{{}}}", elem_type, es.join(", "))
        }
        Expr::ArrayNew { elem_type, size, fill } => match fill {
            Some(f) => format!("new {}[{}]({})", elem_type, fmt_expr(size), fmt_expr(f)),
            None    => format!("new {}[{}]", elem_type, fmt_expr(size)),
        },
        Expr::Index { object, index } =>
            format!("{}[{}]", fmt_expr(object), fmt_expr(index)),
    }
}
