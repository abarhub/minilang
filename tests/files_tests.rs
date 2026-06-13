//! Tests de l'I/O fichiers en bloc (classe utilitaire injectable Files).
//! Chaque test utilise un chemin temporaire unique et nettoie derrière lui.
//! Les chemins sont en slashs avant (acceptés par std::fs, y compris Windows)
//! pour pouvoir être insérés tels quels dans une string literal minilang.

use mini_parser::typechecker::{check_source, TypeChecker};
use mini_parser::interpreter::{Interpreter, run_source};
use chumsky::Parser;
use mini_parser::parser::program_parser;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Crée un chemin temporaire unique (slashs avant) sans créer le fichier.
fn temp_path(tag: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir()
        .join(format!("minilang_files_{}_{}_{}.tmp", std::process::id(), tag, n));
    p.to_string_lossy().replace('\\', "/")
}

fn cleanup(path: &str) { let _ = std::fs::remove_file(path); }

/// Exécute `src` avec l'accès fichiers brut autorisé ([files] unrestricted) —
/// la classe `Files` est gardée par défaut.
fn run_output(src: &str) -> (i64, Vec<String>) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck should pass:\n{}\n---\n{}", src, e.join("\n"));
    }
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    let program = program_parser().parse(full.as_str()).expect("parse");
    assert!(TypeChecker::new(&program).check(&program).is_empty());
    let captured = Rc::new(RefCell::new(Vec::<String>::new()));
    let cap = captured.clone();
    let mut interp = Interpreter::new_with_print(&program,
        Box::new(move |l: &str| cap.borrow_mut().push(l.to_string())));
    interp.set_files_unrestricted(true);
    let ret = interp.run(&program).unwrap_or_else(|e| panic!("Run failed:\n{}", e));
    drop(interp);
    (ret, Rc::try_unwrap(captured).unwrap().into_inner())
}

fn run_ok(src: &str) -> i64 {
    run_output(src).0
}

// ─────────────────────────────────────────────────────────────────────────────
//  Garde : Files brut interdit par défaut (sûr par défaut)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn files_denied_without_unrestricted() {
    // Le typecheck passe (le garde est au runtime), mais l'exécution échoue
    // tant que [files] unrestricted = true n'est pas activé.
    let src = r#"
        int main() {
            Files files = inject Files;
            files.writeText("ne_devrait_pas_exister.txt", "x");
            return 0;
        }
    "#;
    assert!(check_source(src).is_ok());
    let err = run_source(src).unwrap_err();   // run_source = défaut (gardé)
    assert!(err.contains("non autorisé"), "message inattendu : {}", err);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Texte : write / read / append
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn write_then_read_text() {
    let path = temp_path("rwtext");
    let (ret, lines) = run_output(&format!(r#"
        int main() {{
            Files files = inject Files;
            files.writeText("{p}", "bonjour\nmonde");
            Result<string, IoError> r = files.readText("{p}");
            print(r.getValue());
            return 0;
        }}
    "#, p = path));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bonjour\nmonde"]);
    cleanup(&path);
}

#[test]
fn write_overwrites() {
    let path = temp_path("overwrite");
    run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            files.writeText("{p}", "premier");
            files.writeText("{p}", "second");
            return 0;
        }}
    "#, p = path));
    let content = std::fs::read_to_string(&path).expect("fichier écrit");
    assert_eq!(content, "second");
    cleanup(&path);
}

#[test]
fn append_text_accumulates() {
    let path = temp_path("append");
    run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            files.appendText("{p}", "a");      // crée le fichier
            files.appendText("{p}", "b");
            files.appendText("{p}", "c");
            return 0;
        }}
    "#, p = path));
    let content = std::fs::read_to_string(&path).expect("fichier écrit");
    assert_eq!(content, "abc");
    cleanup(&path);
}

#[test]
fn read_text_missing_file_is_err() {
    let path = temp_path("missing");   // jamais créé
    let ret = run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            Result<string, IoError> r = files.readText("{p}");
            if (r.isErr()) {{ return 1; }}
            return 0;
        }}
    "#, p = path));
    assert_eq!(ret, 1);
}

#[test]
fn read_text_invalid_utf8_is_err() {
    // On écrit un octet 0xFF brut en bytes, puis readText doit échouer.
    let path = temp_path("badutf8");
    let (ret, lines) = run_output(&format!(r#"
        int main() {{
            Files files = inject Files;
            byte[] bad = new byte[1];
            match bad.set(0) {{ Option::Some(r) => {{ r.set((255).toByte().get()); }} Option::None => {{}} }}
            files.writeBytes("{p}", bad);
            Result<string, IoError> r = files.readText("{p}");
            if (r.isErr()) {{ print("err"); }} else {{ print("ok"); }}
            return 0;
        }}
    "#, p = path));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["err"]);
    cleanup(&path);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Binaire : writeBytes / readBytes (aller-retour fidèle)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn write_then_read_bytes_roundtrip() {
    let path = temp_path("rwbytes");
    let ret = run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            byte[] data = new byte[3];
            match data.set(0) {{ Option::Some(r) => {{ r.set((0).toByte().get()); }}   Option::None => {{}} }}
            match data.set(1) {{ Option::Some(r) => {{ r.set((255).toByte().get()); }} Option::None => {{}} }}
            match data.set(2) {{ Option::Some(r) => {{ r.set((128).toByte().get()); }} Option::None => {{}} }}
            files.writeBytes("{p}", data);

            byte[] back = files.readBytes("{p}").getValue();
            int sum = 0;
            sum = sum + back.get(0).get().toInt();   // 0
            sum = sum + back.get(1).get().toInt();   // 255
            sum = sum + back.get(2).get().toInt();   // 128
            return sum;                               // 383
        }}
    "#, p = path));
    assert_eq!(ret, 383);
    // Vérifie aussi les octets bruts côté Rust
    let raw = std::fs::read(&path).expect("fichier écrit");
    assert_eq!(raw, vec![0u8, 255u8, 128u8]);
    cleanup(&path);
}

#[test]
fn text_written_then_read_as_bytes() {
    // "AB" écrit en texte → relu en octets = [65, 66]
    let path = temp_path("textbytes");
    let ret = run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            files.writeText("{p}", "AB");
            byte[] data = files.readBytes("{p}").getValue();
            return data.get(0).get().toInt() + data.get(1).get().toInt();  // 131
        }}
    "#, p = path));
    assert_eq!(ret, 131);
    cleanup(&path);
}

// ─────────────────────────────────────────────────────────────────────────────
//  exists / delete
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn exists_and_delete() {
    let path = temp_path("existsdel");
    let (ret, lines) = run_output(&format!(r#"
        int main() {{
            Files files = inject Files;
            print(files.exists("{p}").toString());   // false
            files.writeText("{p}", "x");
            print(files.exists("{p}").toString());   // true
            files.delete("{p}");
            print(files.exists("{p}").toString());   // false
            return 0;
        }}
    "#, p = path));
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["false", "true", "false"]);
    cleanup(&path);
}

#[test]
fn delete_missing_file_is_err() {
    let path = temp_path("delmissing");
    let ret = run_ok(&format!(r#"
        int main() {{
            Files files = inject Files;
            Result<Unit, IoError> r = files.delete("{p}");
            if (r.isErr()) {{ return 1; }}
            return 0;
        }}
    "#, p = path));
    assert_eq!(ret, 1);
}
