//! Tests des racines fichiers configurées ([files.roots] du minilang.toml).
//! Couvre : le parsing TOML, la résolution/validation au démarrage
//! (resolve_roots), et le comportement de FileSystem.root / rootRW à
//! l'exécution (via Interpreter::set_file_roots). Les racines sont octroyées
//! par la config ; le code ne peut que les référencer par leur nom.

use mini_parser::config::{self, ProjectConfig, FileMode, FilesSection, RootConfig};
use mini_parser::interpreter::Interpreter;
use mini_parser::typechecker::TypeChecker;
use chumsky::Parser;
use mini_parser::parser::program_parser;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("ml_root_{}_{}_{}", std::process::id(), tag, n));
    std::fs::create_dir_all(&p).expect("mkdir");
    p
}

/// Exécute `src` (stdlib préfixée) avec les racines `roots` (nom → (path, writable)),
/// en capturant la sortie de print.
fn run_with_roots(src: &str, roots: HashMap<String, (String, bool)>) -> (i64, Vec<String>) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    let program = program_parser().parse(full.as_str())
        .unwrap_or_else(|e| panic!("Parse failed:\n{}",
            e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n")));
    let errs = TypeChecker::new(&program).check(&program);
    assert!(errs.is_empty(), "Typecheck: {:?}", errs.iter().map(|e| &e.0).collect::<Vec<_>>());
    let captured = Rc::new(RefCell::new(Vec::<String>::new()));
    let cap = captured.clone();
    let mut interp = Interpreter::new_with_print(&program,
        Box::new(move |l: &str| cap.borrow_mut().push(l.to_string())));
    interp.set_file_roots(roots);
    let ret = interp.run(&program).unwrap_or_else(|e| panic!("Run failed: {}", e));
    drop(interp);   // libère la closure (qui détient un clone de `captured`)
    (ret, Rc::try_unwrap(captured).unwrap().into_inner())
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing TOML [files.roots]
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_files_roots() {
    let cfg = ProjectConfig::parse(r#"
        [files.roots.data]
        path = "data"
        mode = "read-write"

        [files.roots.assets]
        path = "assets"
    "#).expect("config valide");
    let roots = cfg.files.roots.expect("roots présents");
    assert_eq!(roots["data"].path, "data");
    assert_eq!(roots["data"].mode, FileMode::ReadWrite);
    assert_eq!(roots["assets"].mode, FileMode::Read);   // défaut
}

#[test]
fn parse_files_root_unknown_field_is_error() {
    let err = ProjectConfig::parse(r#"
        [files.roots.data]
        path = "data"
        permission = "rw"
    "#).unwrap_err();
    assert!(err.contains("permission"), "message: {}", err);
}

#[test]
fn parse_files_root_bad_mode_is_error() {
    let err = ProjectConfig::parse(r#"
        [files.roots.data]
        path = "data"
        mode = "append"
    "#).unwrap_err();
    assert!(!err.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
//  resolve_roots : validation au démarrage
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn resolve_roots_existing_dirs() {
    let data = unique_dir("data");
    let assets = unique_dir("assets");
    let mut roots = HashMap::new();
    roots.insert("data".to_string(),
        RootConfig { path: data.to_string_lossy().to_string(), mode: FileMode::ReadWrite });
    roots.insert("assets".to_string(),
        RootConfig { path: assets.to_string_lossy().to_string(), mode: FileMode::Read });
    let files = FilesSection { roots: Some(roots) };

    let resolved = config::resolve_roots(&files, None).expect("résolution ok");
    assert!(resolved["data"].1, "data writable");
    assert!(!resolved["assets"].1, "assets read-only");

    std::fs::remove_dir_all(&data).ok();
    std::fs::remove_dir_all(&assets).ok();
}

#[test]
fn resolve_roots_missing_dir_is_error() {
    let mut roots = HashMap::new();
    roots.insert("ghost".to_string(),
        RootConfig { path: "/zzz_nexiste_pas_12345".to_string(), mode: FileMode::Read });
    let files = FilesSection { roots: Some(roots) };
    let err = config::resolve_roots(&files, None).unwrap_err();
    assert!(err.contains("ghost") && err.contains("introuvable"), "message: {}", err);
}

#[test]
fn resolve_roots_none_is_empty() {
    let resolved = config::resolve_roots(&FilesSection::default(), None).expect("ok");
    assert!(resolved.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
//  Comportement runtime : FileSystem.root / rootRW
// ─────────────────────────────────────────────────────────────────────────────

fn root_map(name: &str, dir: &std::path::Path, writable: bool) -> HashMap<String, (String, bool)> {
    let mut m = HashMap::new();
    m.insert(name.to_string(), (dir.to_string_lossy().to_string(), writable));
    m
}

#[test]
fn rootrw_write_and_read() {
    let dir = unique_dir("rw");
    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir data = fs.rootRW("data").getValue();
            data.writeText("hello.txt", "bonjour racine");
            print(data.readText("hello.txt").getValue());
            return 0;
        }
    "#, root_map("data", &dir, true));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bonjour racine"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn root_gives_read_access() {
    let dir = unique_dir("ro");
    std::fs::write(dir.join("banner.txt"), "BANNIERE").expect("write");
    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadDir assets = fs.root("assets").getValue();
            print(assets.readText("banner.txt").getValue());
            return 0;
        }
    "#, root_map("assets", &dir, false));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["BANNIERE"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rootrw_on_readonly_root_is_err() {
    let dir = unique_dir("rodenied");
    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            Result<ReadWriteDir, IoError> r = fs.rootRW("assets");   // configurée read
            if (r.isErr()) { print("refusé: " + r.getError().message()); }
            return 0;
        }
    "#, root_map("assets", &dir, false));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["refusé: racine 'assets' est en lecture seule"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_root_is_err() {
    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            Result<ReadDir, IoError> r = fs.root("inconnu");
            if (r.isErr()) { print(r.getError().message()); }
            return 0;
        }
    "#, HashMap::new());
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["racine 'inconnu' non configurée"]);
}

#[test]
fn configured_root_still_confines() {
    // Le confinement s'applique aussi à une racine configurée
    let dir = unique_dir("confine");
    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir data = fs.rootRW("data").getValue();
            Result<string, IoError> r = data.readText("../escape");
            if (r.isErr()) { print("bloqué"); }
            return 0;
        }
    "#, root_map("data", &dir, true));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bloqué"]);
    std::fs::remove_dir_all(&dir).ok();
}

// ─────────────────────────────────────────────────────────────────────────────
//  Garde-fou symlink (Unix uniquement : créer un symlink sous Windows exige des
//  privilèges). Le code de confinement est lui multi-plateforme.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(unix)]
#[test]
fn symlink_escaping_root_is_blocked() {
    use std::os::unix::fs::symlink;
    let root = unique_dir("symesc_root");
    let outside = unique_dir("symesc_out");
    std::fs::write(outside.join("secret.txt"), "TOPSECRET").expect("write secret");
    // root/link -> outside (symlink qui pointe HORS de la capacité)
    symlink(&outside, root.join("link")).expect("symlink");

    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir data = fs.rootRW("data").getValue();
            Result<string, IoError> r = data.readText("link/secret.txt");
            if (r.isErr()) { print("bloqué"); } else { print("FUITE: " + r.getValue()); }
            return 0;
        }
    "#, root_map("data", &root, true));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bloqué"]);   // le symlink ne permet pas de sortir

    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&outside).ok();
}

#[cfg(unix)]
#[test]
fn symlink_within_root_is_allowed() {
    use std::os::unix::fs::symlink;
    let root = unique_dir("syminr_root");
    std::fs::create_dir_all(root.join("real")).expect("mkdir real");
    std::fs::write(root.join("real/data.txt"), "OK").expect("write");
    // root/alias -> root/real (symlink interne, reste dans la capacité)
    symlink(root.join("real"), root.join("alias")).expect("symlink");

    let (ret, lines) = run_with_roots(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadDir data = fs.root("data").getValue();
            print(data.readText("alias/data.txt").getValue());
            return 0;
        }
    "#, root_map("data", &root, false));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["OK"]);   // symlink interne autorisé

    std::fs::remove_dir_all(&root).ok();
}
