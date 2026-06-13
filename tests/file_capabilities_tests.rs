//! Tests des capacités de répertoire (minilang.io) — accès fichiers confiné.
//! FileSystem.tempDir() mint une racine RW temporaire ; on ne manipule pas de
//! chemin absolu, on dérive des sous-capacités (sub/subRW). '..' et chemins
//! absolus sont rejetés ; le mode lecture seule (ReadDir) est garanti à la
//! COMPILATION. Les répertoires temporaires (préfixe minilang_cap_) sont
//! nettoyés en fin de test.

use mini_parser::typechecker::check_source;
use mini_parser::interpreter::{run_source, run_source_with_output};
use std::sync::Mutex;

// Sérialise les tests qui créent/nettoient des répertoires temporaires :
// clean_caps() supprime par préfixe, donc deux tests en parallèle pourraient
// s'effacer mutuellement leurs répertoires. Le verrou l'empêche.
static CAP_LOCK: Mutex<()> = Mutex::new(());

// ── Helpers ───────────────────────────────────────────────────────────────────

fn clean_caps() {
    let tmp = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp) {
        for e in entries.flatten() {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("minilang_cap_") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck should pass:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!("Typecheck should have failed (expected '{}'):\n{}", fragment, src),
        Err(e) => {
            let all = e.join("\n");
            assert!(all.contains(fragment), "Expected '{}' in:\n{}", fragment, all);
        }
    }
}

fn run_ok(src: &str) -> i64 {
    let _g = CAP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert_tc_ok(src);
    let r = run_source(src).unwrap_or_else(|e| panic!("Run failed:\n{}\n---\n{}", src, e));
    clean_caps();
    r
}

fn run_output(src: &str) -> (i64, Vec<String>) {
    let _g = CAP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert_tc_ok(src);
    let r = run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e));
    clean_caps();
    r
}

// ── tempDir + lecture/écriture confinée ─────────────────────────────────────

#[test]
fn temp_dir_write_read_roundtrip() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            dir.writeText("notes.txt", "bonjour\nmonde");
            print(dir.readText("notes.txt").getValue());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bonjour\nmonde"]);
}

#[test]
fn write_creates_parent_dirs() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            dir.writeText("a/b/c.txt", "profond");   // crée a/ puis b/
            print(dir.readText("a/b/c.txt").getValue());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["profond"]);
}

#[test]
fn append_accumulates() {
    let ret = run_ok(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            dir.appendText("log.txt", "a");
            dir.appendText("log.txt", "b");
            string content = dir.readText("log.txt").getValue();
            if (content == "ab") { return 1; }
            return 0;
        }
    "#);
    assert_eq!(ret, 1);
}

#[test]
fn exists_and_delete() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            print(dir.exists("f.txt").toString());   // false
            dir.writeText("f.txt", "x");
            print(dir.exists("f.txt").toString());   // true
            dir.delete("f.txt");
            print(dir.exists("f.txt").toString());   // false
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["false", "true", "false"]);
}

// ── Confinement : '..' et chemins absolus rejetés ────────────────────────────

#[test]
fn escape_with_parent_dir_is_rejected() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            Result<string, IoError> r = dir.readText("../secret.txt");
            if (r.isErr()) { print("bloqué: " + r.getError().message()); }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bloqué: chemin hors de la capacité"]);
}

#[test]
fn absolute_path_is_rejected() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            Result<Unit, IoError> r = dir.writeText("/tmp/evil.txt", "x");
            if (r.isErr()) { print("bloqué"); }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bloqué"]);
}

// ── Dérivation de sous-capacités ─────────────────────────────────────────────

#[test]
fn subrw_derives_writable_child() {
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir  = fs.tempDir().getValue();
            ReadWriteDir logs = dir.subRW("logs");
            logs.writeText("app.log", "demarrage");
            // lisible aussi via le parent au chemin enfant
            print(dir.readText("logs/app.log").getValue());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["demarrage"]);
}

#[test]
fn sub_gives_read_only_view() {
    // sub() renvoie un ReadDir : lecture OK, écriture impossible (voir test tc)
    let (ret, lines) = run_output(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            dir.writeText("data/x.txt", "lu");
            ReadDir ro = dir.sub("data");
            print(ro.readText("x.txt").getValue());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["lu"]);
}

// ── Garantie de type : pas d'écriture via une capacité lecture seule ─────────

#[test]
fn tc_err_write_through_readdir() {
    assert_tc_err(r#"
        void f(ReadDir d) {
            d.writeText("x.txt", "y");   // writeText absent de ReadDir
        }
        int main() { return 0; }
    "#, "inconnue");
}

#[test]
fn tc_err_subrw_through_readdir() {
    assert_tc_err(r#"
        void f(ReadDir d) {
            ReadWriteDir rw = d.subRW("x");   // subRW absent de ReadDir
        }
        int main() { return 0; }
    "#, "inconnue");
}

#[test]
fn tc_readwritedir_usable_as_readdir() {
    // Atténuation : un ReadWriteDir passe là où un ReadDir est attendu
    assert_tc_ok(r#"
        string firstByteCount(ReadDir d) {
            return d.readBytes("f").getValue().length().toString();
        }
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir dir = fs.tempDir().getValue();
            dir.writeText("f", "abc");
            print(firstByteCount(dir));   // ReadWriteDir -> ReadDir
            return 0;
        }
    "#);
}

// ── Racine non-forgeable : new Directory() est inerte ───────────────────────

#[test]
fn new_directory_is_inert() {
    let (ret, lines) = run_output(r#"
        int main() {
            Directory d = new Directory();   // capacité non initialisée
            Result<string, IoError> r = d.readText("anything");
            if (r.isErr()) { print("inerte: " + r.getError().message()); }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["inerte: capacité non initialisée"]);
}

// ── Chaque tempDir() est distinct ───────────────────────────────────────────

#[test]
fn temp_dirs_are_isolated() {
    let ret = run_ok(r#"
        int main() {
            FileSystem fs = inject FileSystem;
            ReadWriteDir a = fs.tempDir().getValue();
            ReadWriteDir b = fs.tempDir().getValue();
            a.writeText("f.txt", "dans a");
            if (b.exists("f.txt")) { return 0; }   // b ne voit pas le fichier de a
            return 1;
        }
    "#);
    assert_eq!(ret, 1);
}
