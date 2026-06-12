//! Tests du fichier de configuration de projet — minilang.toml.
//! Fichier optionnel : parsing TOML strict (champ inconnu = erreur),
//! découverte en remontant les répertoires, sélection du profil DI
//! ([di] modules restreint les modules de binding actifs).

use mini_parser::config::{self, ProjectConfig};
use mini_parser::typechecker::TypeChecker;
use mini_parser::interpreter::Interpreter;
use chumsky::Parser;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_program(src: &str) -> mini_parser::ast::Program {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    mini_parser::parser::program_parser()
        .parse(full.as_str())
        .unwrap_or_else(|e| panic!("Parse failed:\n{}",
            e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n")))
}

fn typecheck(program: &mini_parser::ast::Program) -> Vec<String> {
    TypeChecker::new(program).check(program).iter().map(|e| e.0.clone()).collect()
}

fn run(program: &mini_parser::ast::Program) -> i64 {
    Interpreter::new(program).run(program)
        .unwrap_or_else(|e| panic!("Run failed: {}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing du TOML
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_empty_config_is_default() {
    let cfg = ProjectConfig::parse("").expect("config vide valide");
    assert_eq!(cfg, ProjectConfig::default());
    assert!(cfg.project.name.is_none());
    assert!(cfg.project.main.is_none());
    assert!(cfg.di.modules.is_none());
    assert!(cfg.runtime.log.is_none());
}

#[test]
fn parse_full_config() {
    let cfg = ProjectConfig::parse(r#"
        [project]
        name = "mon-appli"
        main = "src/app.mini"

        [di]
        modules = ["ProdModule", "LogModule"]

        [runtime]
        log = "debug"
    "#).expect("config complète valide");
    assert_eq!(cfg.project.name.as_deref(), Some("mon-appli"));
    assert_eq!(cfg.project.main.as_deref(), Some("src/app.mini"));
    assert_eq!(cfg.di.modules.as_deref(),
               Some(&["ProdModule".to_string(), "LogModule".to_string()][..]));
    assert_eq!(cfg.runtime.log.as_deref(), Some("debug"));
}

#[test]
fn parse_partial_config() {
    let cfg = ProjectConfig::parse(r#"
        [di]
        modules = []
    "#).expect("config partielle valide");
    assert_eq!(cfg.di.modules.as_deref(), Some(&[][..]));
    assert!(cfg.project.name.is_none());
}

#[test]
fn parse_unknown_section_is_error() {
    // Détection précoce des typos : section inconnue = erreur
    let err = ProjectConfig::parse(r#"
        [projet]
        name = "typo"
    "#).unwrap_err();
    assert!(err.contains("projet"), "message inattendu : {}", err);
}

#[test]
fn parse_unknown_key_is_error() {
    let err = ProjectConfig::parse(r#"
        [project]
        nom = "typo"
    "#).unwrap_err();
    assert!(err.contains("nom"), "message inattendu : {}", err);
}

#[test]
fn parse_wrong_type_is_error() {
    let err = ProjectConfig::parse(r#"
        [di]
        modules = "ProdModule"
    "#).unwrap_err();
    assert!(!err.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
//  Découverte du fichier (remontée des répertoires)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn find_config_walks_up_directories() {
    let root = std::env::temp_dir()
        .join(format!("minilang_cfg_test_{}", std::process::id()));
    let nested = root.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    let cfg_path = root.join(config::CONFIG_FILE);
    std::fs::write(&cfg_path, "[project]\nname = \"x\"\n").expect("write");

    // Trouvé depuis un sous-répertoire profond
    let found = config::find_config(&nested).expect("config trouvée");
    assert_eq!(found.canonicalize().unwrap(), cfg_path.canonicalize().unwrap());

    // load() retourne la config parsée
    let (cfg, path) = config::load(&nested).expect("load ok").expect("config présente");
    assert_eq!(cfg.project.name.as_deref(), Some("x"));
    assert_eq!(path.canonicalize().unwrap(), cfg_path.canonicalize().unwrap());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_without_config_returns_none() {
    let root = std::env::temp_dir()
        .join(format!("minilang_nocfg_test_{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("mkdir");
    // Note : la remontée s'arrête à la racine du disque ; on suppose qu'aucun
    // minilang.toml ne traîne dans les parents du répertoire temporaire.
    let result = config::load(&root).expect("load ok");
    assert!(result.is_none());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_invalid_config_is_error() {
    let root = std::env::temp_dir()
        .join(format!("minilang_badcfg_test_{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("mkdir");
    std::fs::write(root.join(config::CONFIG_FILE), "[[[pas du toml").expect("write");
    let err = config::load(&root).unwrap_err();
    assert!(err.contains(config::CONFIG_FILE), "message inattendu : {}", err);
    std::fs::remove_dir_all(&root).ok();
}

// ─────────────────────────────────────────────────────────────────────────────
//  Sélection du profil DI — [di] modules
// ─────────────────────────────────────────────────────────────────────────────

const TWO_PROFILES_SRC: &str = r#"
    interface Repo { string find(); }
    service class SqlRepo implements Repo {
        string find() { return "sql"; }
    }
    service class FakeRepo implements Repo {
        string find() { return "fake"; }
    }
    module ProdModule { bind Repo to SqlRepo; }
    module TestModule { bind Repo to FakeRepo; }
    int main() {
        Repo r = inject Repo;
        if (r.find() == "sql") { return 1; }
        if (r.find() == "fake") { return 2; }
        return 0;
    }
"#;

#[test]
fn select_modules_picks_profile() {
    // Profil prod → SqlRepo
    let mut program = parse_program(TWO_PROFILES_SRC);
    config::select_modules(&mut program, &["ProdModule".to_string()]).expect("sélection ok");
    assert_eq!(program.modules.len(), 1);
    assert!(typecheck(&program).is_empty(), "typecheck doit passer");
    assert_eq!(run(&program), 1);

    // Profil test → FakeRepo
    let mut program = parse_program(TWO_PROFILES_SRC);
    config::select_modules(&mut program, &["TestModule".to_string()]).expect("sélection ok");
    assert!(typecheck(&program).is_empty(), "typecheck doit passer");
    assert_eq!(run(&program), 2);
}

#[test]
fn without_selection_both_profiles_conflict() {
    // Sans sélection, les deux modules sont fusionnés → binding dupliqué
    let program = parse_program(TWO_PROFILES_SRC);
    let errors = typecheck(&program);
    assert!(errors.iter().any(|e| e.contains("dupliqué")),
        "binding dupliqué attendu, trouvé : {:?}", errors);
}

#[test]
fn select_modules_unknown_name_is_error() {
    let mut program = parse_program(TWO_PROFILES_SRC);
    let err = config::select_modules(&mut program, &["Inconnu".to_string()]).unwrap_err();
    assert!(err.contains("Inconnu"), "message inattendu : {}", err);
    assert!(err.contains("ProdModule"), "doit lister les modules connus : {}", err);
}

#[test]
fn select_modules_empty_disables_all() {
    // Liste vide = aucun module actif → l'ambiguïté Repo réapparaît
    let mut program = parse_program(TWO_PROFILES_SRC);
    config::select_modules(&mut program, &[]).expect("sélection vide ok");
    assert!(program.modules.is_empty());
    let errors = typecheck(&program);
    assert!(errors.iter().any(|e| e.contains("ambigu")),
        "binding ambigu attendu, trouvé : {:?}", errors);
}
