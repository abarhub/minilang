//! Tests du système d'entrée/sortie — packages minilang.io et minilang.system.
//! Hiérarchie unique orientée texte (string) : Output / Flushable /
//! BufferedOutput, erreurs via Result<Unit, IoError>. StringOutput (capture
//! mémoire) et StandardOutput/StandardError (services natifs, injectables).

use mini_parser::interpreter::{run_source, run_source_with_output};
use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck should pass:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!(
            "Typecheck should have failed (expected '{}'):\n{}",
            fragment, src
        ),
        Err(e) => {
            let all = e.join("\n");
            assert!(
                all.contains(fragment),
                "Expected '{}' in:\n{}",
                fragment,
                all
            );
        }
    }
}

fn run_ok(src: &str) -> i64 {
    assert_tc_ok(src);
    run_source(src).unwrap_or_else(|e| panic!("Run failed:\n{}\n---\n{}", src, e))
}

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
//  StringOutput — capture mémoire, écrite en minilang pur
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn string_output_accumulates() {
    let (ret, lines) = run_output(
        r#"
        int main() {
            StringOutput out = new StringOutput();
            out.write("a");
            out.write("b");
            out.writeLine("c");
            out.write("d");
            print(out.content());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["abc\nd"]);
}

#[test]
fn string_output_write_returns_ok() {
    let ret = run_ok(
        r#"
        import minilang.io.Unit;
        int main() {
            StringOutput out = new StringOutput();
            Result<Unit, IoError> r = out.write("x");
            if (r.isOk())  { return 1; }
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 1);
}

#[test]
fn string_output_usable_as_output_interface() {
    // StringOutput est un BufferedOutput, donc un Output
    let (ret, lines) = run_output(
        r#"
        void emit(Output o) {
            o.write("hello ");
            o.writeLine("world");
        }
        int main() {
            StringOutput so = new StringOutput();
            emit(so);
            print(so.content());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["hello world\n"]);
}

#[test]
fn string_output_usable_as_buffered_output() {
    let (ret, lines) = run_output(
        r#"
        void emit(BufferedOutput o) {
            o.write("data");
            o.flush();
        }
        int main() {
            StringOutput so = new StringOutput();
            emit(so);
            print(so.content());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["data"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Result<Unit, IoError> — chemin d'erreur
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn custom_output_error_path() {
    let (ret, lines) = run_output(
        r#"
        mut class FailingOutput implements Output {
            mutable Result<Unit, IoError> write(string s) {
                return Result<Unit, IoError>::Err(IoError::WriteFailed("disk full"));
            }
            mutable Result<Unit, IoError> writeLine(string s) {
                return Result<Unit, IoError>::Err(IoError::Other("nope"));
            }
        }
        int main() {
            Output o = new FailingOutput();
            Result<Unit, IoError> r = o.write("x");
            if (r.isErr()) {
                print("erreur:", r.getError().message());
            }
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["erreur: disk full"]);
}

#[test]
fn io_error_message_variants() {
    let (ret, lines) = run_output(
        r#"
        int main() {
            IoError e1 = IoError::BrokenPipe;
            IoError e2 = IoError::ReadFailed("oops");
            print(e1.message());
            print(e2.message());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["broken pipe", "oops"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Services standard (minilang.system) — injectables
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn standard_output_injectable_and_ok() {
    // Écrit réellement sur stdout (capturé par cargo) ; on vérifie le Result.
    let ret = run_ok(
        r#"
        import minilang.io.Unit;
        int main() {
            StandardOutput out = inject StandardOutput;
            Result<Unit, IoError> r = out.writeLine("depuis StandardOutput");
            if (r.isOk()) { return 0; }
            return 1;
        }
    "#,
    );
    assert_eq!(ret, 0);
}

#[test]
fn standard_error_injectable() {
    let ret = run_ok(
        r#"
        int main() {
            StandardError err = inject StandardError;
            err.writeLine("un avertissement");
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
}

#[test]
fn inject_output_is_ambiguous_without_bind() {
    // StandardOutput, StandardError et StringOutput implémentent tous Output
    assert_tc_err(
        r#"
        int main() {
            Output o = inject Output;
            return 0;
        }
    "#,
        "ambigu",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Swap d'implémentation par binding DI (le cas d'usage clé)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bind_output_to_string_output_captures() {
    // Le service consommateur écrit sur Output ; un module binde Output sur
    // StringOutput ; on relit ce qui a été écrit via le même singleton.
    let (ret, lines) = run_output(
        r#"
        service class Report {
            Output out;
            Report(Output out) { this.out = out; }
            void render() {
                out.writeLine("ligne 1");
                out.writeLine("ligne 2");
            }
        }
        module TestModule { bind Output to StringOutput; }
        int main() {
            Report r = inject Report;
            r.render();
            StringOutput captured = inject StringOutput;   // même singleton
            print(captured.content());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["ligne 1\nligne 2\n"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Garde-fous de typage
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_class_missing_buffered_method() {
    // implements BufferedOutput exige write + writeLine + flush (transitif)
    assert_tc_err(
        r#"
        mut class Half implements BufferedOutput {
            mutable Result<Unit, IoError> write(string s) {
                return Result<Unit, IoError>::Ok(new Unit());
            }
        }
        int main() { return 0; }
    "#,
        "n'implémente pas",
    );
}
