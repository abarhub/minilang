//! Tests du système de tests minilang.
//! `test void nom() { ... }` — fonctions de test exécutées par le runner ;
//! assertions builtin (assertTrue, assertFalse, assertEquals, assertNotEquals,
//! fail) ; main optionnel ; isolation : interpréteur (et conteneur DI) neuf
//! pour chaque test.

use chumsky::Parser;
use mini_parser::parser::program_parser;
use mini_parser::test_runner::{TestResult, run_tests, run_tests_source};
use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    match program_parser().parse(full.as_str()) {
        Ok(_) => {}
        Err(e) => panic!(
            "Parse failed:\n{}\n---\n{}",
            src,
            e.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

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

fn run(src: &str) -> Vec<TestResult> {
    assert_tc_ok(src);
    run_tests_source(src).unwrap_or_else(|e| panic!("Parse failed:\n{}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_test_function() {
    parses_ok(
        r#"
        test void monTest() {
            assertTrue(true);
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_file_without_main() {
    // Un fichier de tests n'a pas besoin de main
    parses_ok(
        r#"
        test void monTest() {
            assertEquals(1, 1);
        }
    "#,
    );
}

#[test]
fn parse_function_named_test_still_works() {
    // `test` n'est pas réservé comme nom de fonction
    parses_ok(
        r#"
        int test() { return 1; }
        int main() { return test(); }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_assertions_ok() {
    assert_tc_ok(
        r#"
        int add(int a, int b) { return a + b; }
        test void lesAssertions() {
            assertTrue(1 < 2);
            assertFalse(1 > 2);
            assertEquals(add(2, 3), 5);
            assertNotEquals("a", "b");
        }
    "#,
    );
}

#[test]
fn tc_err_test_with_params() {
    assert_tc_err(
        r#"
        test void monTest(int x) {
            assertTrue(true);
        }
    "#,
        "ne doit pas avoir de paramètres",
    );
}

#[test]
fn tc_err_test_non_void() {
    assert_tc_err(
        r#"
        test int monTest() {
            return 1;
        }
    "#,
        "doit retourner void",
    );
}

#[test]
fn tc_err_assert_true_non_bool() {
    assert_tc_err(
        r#"
        test void monTest() {
            assertTrue(42);
        }
    "#,
        "doit être bool",
    );
}

#[test]
fn tc_err_assert_equals_incomparable() {
    assert_tc_err(
        r#"
        test void monTest() {
            assertEquals(1, "un");
        }
    "#,
        "incomparables",
    );
}

#[test]
fn tc_err_assert_equals_arity() {
    assert_tc_err(
        r#"
        test void monTest() {
            assertEquals(1);
        }
    "#,
        "attend 2 arguments",
    );
}

#[test]
fn tc_err_fail_non_string() {
    assert_tc_err(
        r#"
        test void monTest() {
            fail(42);
        }
    "#,
        "doit être string",
    );
}

#[test]
fn tc_inject_allowed_in_test() {
    // Les fonctions test sont des fonctions de haut niveau → inject autorisé
    assert_tc_ok(
        r#"
        service class Foo {
            int answer() { return 42; }
        }
        test void monTest() {
            Foo f = inject Foo;
            assertEquals(f.answer(), 42);
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Runner
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn runner_passing_tests() {
    let results = run(r#"
        int add(int a, int b) { return a + b; }
        test void addition()  { assertEquals(add(2, 3), 5); }
        test void comparaison() { assertTrue(add(1, 1) == 2); }
    "#);
    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|r| r.passed()),
        "tous les tests doivent passer : {:?}",
        results
    );
    // Ordre de déclaration préservé
    assert_eq!(results[0].name, "addition");
    assert_eq!(results[1].name, "comparaison");
}

#[test]
fn runner_failing_assertion_message() {
    let results = run(r#"
        test void mauvaiseSomme() { assertEquals(2 + 2, 5); }
    "#);
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("le test doit échouer");
    assert!(
        err.contains("assertEquals") && err.contains("4") && err.contains("5"),
        "message inattendu : {}",
        err
    );
}

#[test]
fn runner_continues_after_failure() {
    // Un échec n'arrête pas les tests suivants
    let results = run(r#"
        test void casse()   { fail("boum"); }
        test void passe()   { assertTrue(true); }
        test void casse2()  { assertFalse(true); }
    "#);
    assert_eq!(results.len(), 3);
    assert!(!results[0].passed());
    assert!(results[1].passed());
    assert!(!results[2].passed());
    assert!(results[0].error.as_deref().unwrap().contains("boum"));
}

#[test]
fn runner_panic_is_caught_as_failure() {
    let results = run(r#"
        test void quiPanique() { panic("explosion"); }
    "#);
    assert!(results[0].error.as_deref().unwrap().contains("explosion"));
}

#[test]
fn runner_resets_di_container_between_tests() {
    // Chaque test repart d'un conteneur neuf : le singleton ne fuit pas
    let results = run(r#"
        service mut class Compteur {
            int value;
            Compteur() { this.value = 0; }
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        test void premier() {
            Compteur c = inject Compteur;
            c.increment();
            assertEquals(c.get(), 1);
        }
        test void second() {
            Compteur c = inject Compteur;
            c.increment();
            assertEquals(c.get(), 1);   // 2 si le singleton fuyait du test précédent
        }
    "#);
    assert!(
        results.iter().all(|r| r.passed()),
        "isolation des singletons attendue : {:?}",
        results
    );
}

#[test]
fn runner_test_with_di_profile() {
    // Profil DI de test sélectionné via config::select_modules
    let src = r#"
        interface Repo { string find(); }
        service class SqlRepo implements Repo {
            string find() { return "sql"; }
        }
        service class FakeRepo implements Repo {
            string find() { return "fake"; }
        }
        module ProdModule { bind Repo to SqlRepo; }
        module TestModule { bind Repo to FakeRepo; }
        test void utiliseLeFake() {
            Repo r = inject Repo;
            assertEquals(r.find(), "fake");
        }
    "#;
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    let mut program = program_parser().parse(full.as_str()).unwrap_or_else(|e| {
        panic!(
            "Parse failed:\n{}",
            e.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    mini_parser::config::select_modules(&mut program, &["TestModule".to_string()])
        .expect("sélection ok");
    let results = run_tests(&program);
    assert!(
        results[0].passed(),
        "le fake doit être injecté : {:?}",
        results
    );
}

#[test]
fn runner_no_test_functions() {
    let results = run(r#"
        int add(int a, int b) { return a + b; }
        int main() { return 0; }
    "#);
    assert!(results.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
//  Main optionnel — comportement du mode run préservé
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_mode_without_main_is_runtime_error() {
    use mini_parser::interpreter::Interpreter;
    let full = format!(
        "{}\n{}",
        mini_parser::STDLIB,
        "test void t() { assertTrue(true); }"
    );
    let program = program_parser().parse(full.as_str()).expect("parse ok");
    let err = Interpreter::new(&program).run(&program).unwrap_err();
    assert!(
        err.to_string().contains("main"),
        "message inattendu : {}",
        err
    );
}

#[test]
fn assertions_usable_in_main_too() {
    // Les assertions sont des fonctions normales, utilisables hors tests
    let ret = mini_parser::interpreter::run_source(
        r#"
        int main() {
            assertEquals(1 + 1, 2);
            return 0;
        }
    "#,
    )
    .expect("run ok");
    assert_eq!(ret, 0);
}
