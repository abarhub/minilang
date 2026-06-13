//! Tests de la politique de nettoyage des répertoires temporaires
//! ([files] temp = none | mark | delete). On pilote Interpreter::set_temp_policy
//! et on inspecte les répertoires `minilang_cap_<pid>_*` créés par tempDir().
//! Filtrés par PID (isolation entre binaires de test) + sérialisés par verrou
//! (CAP_SEQ partagé dans le process), avec nettoyage.

use mini_parser::config::{ProjectConfig, TempCleanup, FilesSection};
use mini_parser::interpreter::Interpreter;
use mini_parser::typechecker::TypeChecker;
use chumsky::Parser;
use mini_parser::parser::program_parser;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

static TEMP_LOCK: Mutex<()> = Mutex::new(());
const MARKER: &str = ".minilang-temp";

fn cap_prefix() -> String { format!("minilang_cap_{}_", std::process::id()) }

/// Répertoires temp de CE processus présents actuellement.
fn snapshot() -> HashSet<PathBuf> {
    let pref = cap_prefix();
    let mut set = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with(&pref) {
                set.insert(e.path());
            }
        }
    }
    set
}

/// Exécute un programme qui crée un répertoire temp, avec la politique donnée.
/// Retourne les répertoires temp de ce process créés pendant l'exécution.
fn run_tempdir_with_policy(create_marker: bool, delete_at_end: bool) -> Vec<PathBuf> {
    let _g = TEMP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let src = r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir d = fs.tempDir().getValue();
            d.writeText("f.txt", "x");   // s'assure que le répertoire est bien utilisable
            return 0;
        }
    "#;
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    let program = program_parser().parse(full.as_str()).expect("parse");
    assert!(TypeChecker::new(&program).check(&program).is_empty());

    let before = snapshot();
    let mut interp = Interpreter::new_with_print(&program, Box::new(|_l: &str| {}));
    interp.set_temp_policy(create_marker, delete_at_end);
    interp.run(&program).expect("run");
    drop(interp);
    let after = snapshot();
    after.difference(&before).cloned().collect()
}

fn cleanup(dirs: &[PathBuf]) {
    for d in dirs { let _ = std::fs::remove_dir_all(d); }
}

// ── Comportement runtime ────────────────────────────────────────────────────

#[test]
fn mark_creates_marker() {
    let created = run_tempdir_with_policy(true, false);
    assert_eq!(created.len(), 1, "un répertoire temp créé");
    assert!(created[0].join(MARKER).is_file(), "marqueur .minilang-temp présent");
    cleanup(&created);
}

#[test]
fn none_creates_no_marker() {
    let created = run_tempdir_with_policy(false, false);
    assert_eq!(created.len(), 1);
    assert!(!created[0].join(MARKER).exists(), "aucun marqueur en mode none");
    cleanup(&created);
}

#[test]
fn delete_removes_dir_at_end() {
    let created = run_tempdir_with_policy(true, true);
    // Le répertoire a été créé puis supprimé en fin de run() : le diff est vide.
    assert!(created.is_empty(), "le répertoire temp doit être supprimé en fin de programme");
}

// ── Parsing de la config ────────────────────────────────────────────────────

#[test]
fn temp_default_is_mark() {
    let cfg = ProjectConfig::parse("").expect("ok");
    assert_eq!(cfg.files.temp, TempCleanup::Mark);
    assert_eq!(FilesSection::default().temp, TempCleanup::Mark);
}

#[test]
fn parse_temp_modes() {
    for (s, expected) in [
        ("none", TempCleanup::None),
        ("mark", TempCleanup::Mark),
        ("delete", TempCleanup::Delete),
    ] {
        let cfg = ProjectConfig::parse(&format!("[files]\ntemp = \"{}\"\n", s)).expect("ok");
        assert_eq!(cfg.files.temp, expected, "mode {}", s);
    }
}

#[test]
fn parse_temp_invalid_is_error() {
    let err = ProjectConfig::parse("[files]\ntemp = \"purge\"\n").unwrap_err();
    assert!(!err.is_empty());
}
