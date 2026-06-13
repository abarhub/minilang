//! Tests du système d'I/O — phase 2 : entrée (Input) et sortie bufferisée.
//! On exerce StringInput (double mémoire) et BufferedWriter — jamais
//! StandardInput, qui lirait le vrai stdin et bloquerait les tests. Le câblage
//! StandardInput est validé au typecheck et par vérification manuelle (pipe).

use mini_parser::typechecker::check_source;
use mini_parser::interpreter::{run_source, run_source_with_output};

// ── Helpers ───────────────────────────────────────────────────────────────────

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
    assert_tc_ok(src);
    run_source(src).unwrap_or_else(|e| panic!("Run failed:\n{}\n---\n{}", src, e))
}

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
//  StringInput — lecture en mémoire (pur minilang)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn string_input_read_lines_until_eof() {
    let (ret, lines) = run_output(r#"
        int main() {
            StringInput in = new StringInput();
            in.feed("alpha\nbeta\ngamma");
            bool fini = false;
            while (!fini) {
                match in.readLine().getValue() {
                    Option::Some(l) => { print(l); }
                    Option::None    => { fini = true; }
                }
            }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn string_input_eof_returns_ok_none() {
    let ret = run_ok(r#"
        int main() {
            StringInput in = new StringInput();
            in.feed("x\n");
            in.readLine();                 // consomme "x"
            Option<string> r = in.readLine().getValue();
            if (r.isNone()) { return 1; }  // EOF
            return 0;
        }
    "#);
    assert_eq!(ret, 1);
}

#[test]
fn string_input_read_char() {
    let (ret, lines) = run_output(r#"
        int main() {
            StringInput in = new StringInput();
            in.feed("ab");
            match in.readChar().getValue() {
                Option::Some(c) => { print(c.toString()); }
                Option::None    => { print("?"); }
            }
            match in.readChar().getValue() {
                Option::Some(c) => { print(c.toString()); }
                Option::None    => { print("?"); }
            }
            match in.readChar().getValue() {
                Option::Some(c) => { print(c.toString()); }
                Option::None    => { print("eof"); }
            }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["a", "b", "eof"]);
}

#[test]
fn string_input_read_all() {
    let (ret, lines) = run_output(r#"
        int main() {
            StringInput in = new StringInput();
            in.feed("tout le contenu");
            print(in.readAll().getValue());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["tout le contenu"]);
}

#[test]
fn string_input_usable_as_input_interface() {
    let (ret, lines) = run_output(r#"
        string firstLine(Input src) {
            match src.readLine().getValue() {
                Option::Some(l) => { return l; }
                Option::None    => { return "(vide)"; }
            }
        }
        int main() {
            StringInput in = new StringInput();
            in.feed("première\nseconde");
            print(firstLine(in));
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["première"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Lecture via injection — bind Input to StringInput (profil de test)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bind_input_to_string_input() {
    // Un service consomme Input ; un module le binde sur StringInput ; on
    // alimente le même singleton avant de faire travailler le consommateur.
    let (ret, lines) = run_output(r#"
        service class Echoer {
            Input src;
            Echoer(Input src) { this.src = src; }
            void echo() {
                bool fini = false;
                while (!fini) {
                    match src.readLine().getValue() {
                        Option::Some(l) => { print("> " + l); }
                        Option::None    => { fini = true; }
                    }
                }
            }
        }
        module TestModule { bind Input to StringInput; }
        int main() {
            StringInput in = inject StringInput;   // même singleton que celui injecté dans Echoer
            in.feed("un\ndeux");
            Echoer e = inject Echoer;
            e.echo();
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["> un", "> deux"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  BufferedWriter — sortie bufferisée concrète
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn buffered_writer_holds_until_flush() {
    let (ret, lines) = run_output(r#"
        int main() {
            StringOutput sink = new StringOutput();
            BufferedWriter bw = new BufferedWriter(sink);
            bw.write("a");
            bw.writeLine("b");
            print("avant flush: [" + sink.content() + "]");   // rien encore
            bw.flush();
            print("après flush: [" + sink.content() + "]");
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["avant flush: []", "après flush: [ab\n]"]);
}

#[test]
fn buffered_writer_is_buffered_output() {
    // BufferedWriter implémente BufferedOutput, utilisable de façon abstraite
    let (ret, lines) = run_output(r#"
        void emit(BufferedOutput o) {
            o.write("x");
            o.flush();
        }
        int main() {
            StringOutput sink = new StringOutput();
            BufferedWriter bw = new BufferedWriter(sink);
            emit(bw);
            print(sink.content());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["x"]);
}

// ─────────────────────────────────────────────────────────────────────────────
//  StandardInput — câblage validé au typecheck (jamais exécuté ici : bloquerait)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_standard_input_injectable() {
    assert_tc_ok(r#"
        int main() {
            StandardInput in = inject StandardInput;
            Result<Option<string>, IoError> r = in.readLine();
            return 0;
        }
    "#);
}

#[test]
fn tc_err_class_missing_input_method() {
    assert_tc_err(r#"
        mut class HalfInput implements Input {
            mutable Result<Option<string>, IoError> readLine() {
                return Result<Option<string>, IoError>::Ok(Option<string>::None);
            }
        }
        int main() { return 0; }
    "#, "n'implémente pas");
}
