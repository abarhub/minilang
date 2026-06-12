//! Tests du système d'injection de dépendances — minilang.
//! `service class X` — classe instanciable par le conteneur d'injection ;
//! `inject T` — résolution d'un service (singleton) dans main ou une fonction
//! de haut niveau. Tout est validé au typecheck : binding manquant, binding
//! ambigu, cycle de dépendances, types non injectables.

use mini_parser::typechecker::check_source;
use mini_parser::interpreter::{run_source, run_source_with_output};
use chumsky::Parser;
use mini_parser::parser::program_parser;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    match program_parser().parse(full.as_str()) {
        Ok(_) => {}
        Err(e) => panic!("Parse failed:\n{}\n---\n{}",
            src, e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n")),
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
            assert!(all.contains(fragment),
                "Expected '{}' in:\n{}", fragment, all);
        }
    }
}

fn run_ok(src: &str) -> i64 {
    assert_tc_ok(src);
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Run failed:\n{}\n---\n{}", src, e),
    }
}

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    match run_source_with_output(src) {
        Ok(r)  => r,
        Err(e) => panic!("Run failed:\n{}\n---\n{}", src, e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_service_class() {
    parses_ok(r#"
        service class Foo {
            int answer() { return 42; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_service_mut_class() {
    parses_ok(r#"
        service mut class Counter {
            int value;
            mutable void increment() { value = value + 1; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_inject_expr() {
    parses_ok(r#"
        service class Foo {}
        int main() {
            Foo f = inject Foo;
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — cas valides
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_inject_simple_service() {
    assert_tc_ok(r#"
        service class Foo {
            int answer() { return 42; }
        }
        int main() {
            Foo f = inject Foo;
            return f.answer();
        }
    "#);
}

#[test]
fn tc_inject_by_interface() {
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        int main() {
            Logger l = inject Logger;
            l.log("hello");
            return 0;
        }
    "#);
}

#[test]
fn tc_service_with_interface_dependency() {
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class UserService {
            Logger logger;
            UserService(Logger logger) { this.logger = logger; }
            void hello() { logger.log("hello"); }
        }
        int main() {
            UserService s = inject UserService;
            s.hello();
            return 0;
        }
    "#);
}

#[test]
fn tc_dependency_chain() {
    // A dépend de B qui dépend de C — pas de cycle
    assert_tc_ok(r#"
        service class C {
            int val() { return 7; }
        }
        service class B {
            C c;
            B(C c) { this.c = c; }
            int val() { return c.val(); }
        }
        service class A {
            B b;
            A(B b) { this.b = b; }
            int val() { return b.val(); }
        }
        int main() {
            A a = inject A;
            return a.val();
        }
    "#);
}

#[test]
fn tc_inject_in_toplevel_function() {
    assert_tc_ok(r#"
        service class Foo {
            int answer() { return 42; }
        }
        int helper() {
            Foo f = inject Foo;
            return f.answer();
        }
        int main() { return helper(); }
    "#);
}

#[test]
fn tc_two_impls_without_injection_is_ok() {
    // Deux implémentations ne sont une erreur QUE si on injecte l'interface
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class A implements Logger {
            void log(string msg) { print(msg); }
        }
        service class B implements Logger {
            void log(string msg) { print(msg); }
        }
        int main() {
            A a = inject A;
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — erreurs détectées à la compilation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_inject_non_service_class() {
    assert_tc_err(r#"
        class Plain {}
        int main() {
            Plain p = inject Plain;
            return 0;
        }
    "#, "doit être déclarée `service`");
}

#[test]
fn tc_err_inject_unknown_type() {
    assert_tc_err(r#"
        int main() {
            int x = 0;
            x = 1;
            inject Mystere;
            return 0;
        }
    "#, "n'est pas injectable");
}

#[test]
fn tc_err_missing_binding() {
    // Interface sans aucune implémentation service
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class UserService {
            Logger logger;
            UserService(Logger logger) { this.logger = logger; }
        }
        int main() {
            UserService s = inject UserService;
            return 0;
        }
    "#, "Aucun service n'implémente");
}

#[test]
fn tc_err_ambiguous_binding() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class A implements Logger {
            void log(string msg) { print(msg); }
        }
        service class B implements Logger {
            void log(string msg) { print(msg); }
        }
        int main() {
            Logger l = inject Logger;
            return 0;
        }
    "#, "ambigu");
}

#[test]
fn tc_err_ambiguous_dependency() {
    // L'ambiguïté est aussi détectée sur une dépendance de constructeur
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class A implements Logger {
            void log(string msg) { print(msg); }
        }
        service class B implements Logger {
            void log(string msg) { print(msg); }
        }
        service class UserService {
            Logger logger;
            UserService(Logger logger) { this.logger = logger; }
        }
        int main() { return 0; }
    "#, "ambigu");
}

#[test]
fn tc_err_dependency_cycle() {
    assert_tc_err(r#"
        service class Alpha {
            Beta b;
            Alpha(Beta b) { this.b = b; }
        }
        service class Beta {
            Alpha a;
            Beta(Alpha a) { this.a = a; }
        }
        int main() { return 0; }
    "#, "Cycle de dépendances");
}

#[test]
fn tc_err_self_cycle() {
    assert_tc_err(r#"
        service class Selfish {
            Selfish s;
            Selfish(Selfish s) { this.s = s; }
        }
        int main() { return 0; }
    "#, "Cycle de dépendances");
}

#[test]
fn tc_err_non_injectable_param() {
    assert_tc_err(r#"
        service class S {
            int n;
            S(int n) { this.n = n; }
        }
        int main() { return 0; }
    "#, "n'est pas injectable");
}

#[test]
fn tc_err_dependency_on_plain_class() {
    assert_tc_err(r#"
        class Plain {}
        service class S {
            Plain p;
            S(Plain p) { this.p = p; }
        }
        int main() { return 0; }
    "#, "doit être déclarée `service`");
}

#[test]
fn tc_err_multiple_constructors() {
    assert_tc_err(r#"
        service mut class S {
            int n;
            S() { this.n = 0; }
            S(S other) { this.n = 1; }
        }
        int main() { return 0; }
    "#, "au plus un constructeur");
}

#[test]
fn tc_err_generic_service() {
    assert_tc_err(r#"
        service class Box<T> {}
        int main() { return 0; }
    "#, "générique");
}

#[test]
fn tc_err_inject_inside_class_method() {
    // inject est réservé à main et aux fonctions de haut niveau
    assert_tc_err(r#"
        service class Foo {
            int answer() { return 42; }
        }
        class Caller {
            int doIt() {
                Foo f = inject Foo;
                return f.answer();
            }
        }
        int main() { return 0; }
    "#, "n'est autorisé que dans");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Exécution
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_inject_simple_service() {
    let ret = run_ok(r#"
        service class Foo {
            int answer() { return 42; }
        }
        int main() {
            Foo f = inject Foo;
            return f.answer();
        }
    "#);
    assert_eq!(ret, 42);
}

#[test]
fn run_inject_interface_resolves_to_impl() {
    let (ret, lines) = run_output(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print("LOG:", msg); }
        }
        int main() {
            Logger l = inject Logger;
            l.log("hello");
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["LOG: hello"]);
}

#[test]
fn run_transitive_dependencies_wired() {
    let (ret, lines) = run_output(r#"
        interface Repo { string findUser(); }
        service class MemoryRepo implements Repo {
            string findUser() { return "alice"; }
        }
        service class UserService {
            Repo repo;
            UserService(Repo repo) { this.repo = repo; }
            string greet() { return "hello " + repo.findUser(); }
        }
        int main() {
            UserService s = inject UserService;
            print(s.greet());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["hello alice"]);
}

#[test]
fn run_services_are_singletons() {
    // Deux inject du même service retournent la même instance
    let ret = run_ok(r#"
        service mut class Counter {
            int value;
            Counter() { this.value = 0; }
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() {
            Counter a = inject Counter;
            Counter b = inject Counter;
            a.increment();
            a.increment();
            b.increment();
            return b.get();
        }
    "#);
    assert_eq!(ret, 3);
}

#[test]
fn run_shared_dependency_is_same_instance() {
    // Deux services dépendant du même service partagent le singleton
    let ret = run_ok(r#"
        service mut class Store {
            int value;
            Store() { this.value = 0; }
            mutable void add(int n) { value = value + n; }
            int get() { return value; }
        }
        service class Writer {
            Store store;
            Writer(Store store) { this.store = store; }
            void write() { store.add(5); }
        }
        service class Reader {
            Store store;
            Reader(Store store) { this.store = store; }
            int read() { return store.get(); }
        }
        int main() {
            Writer w = inject Writer;
            Reader r = inject Reader;
            w.write();
            return r.read();
        }
    "#);
    assert_eq!(ret, 5);
}

#[test]
fn run_inject_in_toplevel_function() {
    let ret = run_ok(r#"
        service class Foo {
            int answer() { return 42; }
        }
        int helper() {
            Foo f = inject Foo;
            return f.answer();
        }
        int main() { return helper(); }
    "#);
    assert_eq!(ret, 42);
}
