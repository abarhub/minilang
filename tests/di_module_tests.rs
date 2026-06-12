//! Tests de la phase 2 de l'injection de dépendances — minilang.
//! `module M { bind ...; }` — bindings explicites (choix d'implémentation),
//! valeurs de configuration (`with`), scope `transient`.
//! Comme en phase 1, tout est validé au typecheck.

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
fn parse_module_with_binds() {
    parses_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to ConsoleLogger;
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_bind_with_values() {
    parses_ok(r#"
        service class HttpClient {
            string baseUrl;
            HttpClient(string baseUrl) { this.baseUrl = baseUrl; }
        }
        module AppModule {
            bind HttpClient with ("https://api", );
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_transient_service() {
    parses_ok(r#"
        transient service class Ctx {}
        transient service mut class MutCtx {
            int value;
            mutable void set(int v) { value = v; }
        }
        int main() { return 0; }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — modules valides
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_bind_resolves_ambiguity() {
    // Deux implémentations + bind explicite → plus d'ambiguïté
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class FileLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to FileLogger;
        }
        int main() {
            Logger l = inject Logger;
            return 0;
        }
    "#);
}

#[test]
fn tc_bind_with_single_impl_is_ok() {
    // bind explicite même quand l'implémentation est unique — autorisé
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to ConsoleLogger;
        }
        int main() {
            Logger l = inject Logger;
            return 0;
        }
    "#);
}

#[test]
fn tc_with_config_values() {
    assert_tc_ok(r#"
        service class HttpClient {
            string baseUrl;
            int timeout;
            HttpClient(string baseUrl, int timeout) {
                this.baseUrl = baseUrl;
                this.timeout = timeout;
            }
        }
        module AppModule {
            bind HttpClient with ("https://api", 30);
        }
        int main() {
            HttpClient c = inject HttpClient;
            return 0;
        }
    "#);
}

#[test]
fn tc_mixed_dependency_and_config() {
    // Constructeur mixte : dépendance service + valeur de configuration
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class Job {
            Logger logger;
            string name;
            Job(Logger logger, string name) {
                this.logger = logger;
                this.name = name;
            }
        }
        module AppModule {
            bind Job with ("batch");
        }
        int main() {
            Job j = inject Job;
            return 0;
        }
    "#);
}

#[test]
fn tc_bind_to_with_combined() {
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        service class FileLogger implements Logger {
            string path;
            FileLogger(string path) { this.path = path; }
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to FileLogger with ("app.log");
        }
        int main() {
            Logger l = inject Logger;
            return 0;
        }
    "#);
}

#[test]
fn tc_multiple_modules_merged() {
    assert_tc_ok(r#"
        interface Logger { void log(string msg); }
        interface Repo { string find(); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class NullLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class MemRepo implements Repo {
            string find() { return "x"; }
        }
        service class SqlRepo implements Repo {
            string find() { return "y"; }
        }
        module LogModule  { bind Logger to ConsoleLogger; }
        module RepoModule { bind Repo to SqlRepo; }
        int main() {
            Logger l = inject Logger;
            Repo r = inject Repo;
            return 0;
        }
    "#);
}

#[test]
fn tc_transient_service() {
    assert_tc_ok(r#"
        transient service class Ctx {
            int val() { return 1; }
        }
        int main() {
            Ctx c = inject Ctx;
            return c.val();
        }
    "#);
}

#[test]
fn tc_transient_can_depend_on_singleton() {
    assert_tc_ok(r#"
        service class Config {
            int val() { return 1; }
        }
        transient service class Ctx {
            Config config;
            Ctx(Config config) { this.config = config; }
        }
        int main() {
            Ctx c = inject Ctx;
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — erreurs détectées à la compilation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_bind_unknown_target() {
    assert_tc_err(r#"
        module AppModule {
            bind Mystere to Autre;
        }
        int main() { return 0; }
    "#, "n'est ni une interface ni une classe service");
}

#[test]
fn tc_err_bind_to_unknown_class() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to Fantome;
        }
        int main() { return 0; }
    "#, "inconnue");
}

#[test]
fn tc_err_bind_to_non_service() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        class PlainLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger to PlainLogger;
        }
        int main() { return 0; }
    "#, "doit être déclarée `service`");
}

#[test]
fn tc_err_bind_to_not_implementing() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        service class Other {}
        module AppModule {
            bind Logger to Other;
        }
        int main() { return 0; }
    "#, "n'implémente pas");
}

#[test]
fn tc_err_duplicate_bind() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class A implements Logger {
            void log(string msg) { print(msg); }
        }
        service class B implements Logger {
            void log(string msg) { print(msg); }
        }
        module M1 { bind Logger to A; }
        module M2 { bind Logger to B; }
        int main() { return 0; }
    "#, "Binding dupliqué");
}

#[test]
fn tc_err_bind_without_effect() {
    assert_tc_err(r#"
        service class Foo {}
        module AppModule {
            bind Foo;
        }
        int main() { return 0; }
    "#, "sans effet");
}

#[test]
fn tc_err_with_on_interface_without_to() {
    assert_tc_err(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print(msg); }
        }
        module AppModule {
            bind Logger with ("x");
        }
        int main() { return 0; }
    "#, "nécessite `to`");
}

#[test]
fn tc_err_with_wrong_arity() {
    assert_tc_err(r#"
        service class HttpClient {
            string baseUrl;
            int timeout;
            HttpClient(string baseUrl, int timeout) {
                this.baseUrl = baseUrl;
                this.timeout = timeout;
            }
        }
        module AppModule {
            bind HttpClient with ("https://api");
        }
        int main() { return 0; }
    "#, "paramètre(s)");
}

#[test]
fn tc_err_with_wrong_type() {
    assert_tc_err(r#"
        service class HttpClient {
            int timeout;
            HttpClient(int timeout) { this.timeout = timeout; }
        }
        module AppModule {
            bind HttpClient with ("trente");
        }
        int main() { return 0; }
    "#, "incompatible");
}

#[test]
fn tc_err_config_param_without_with() {
    // Sans module, un paramètre non-service reste une erreur (comme en phase 1)
    assert_tc_err(r#"
        service class S {
            int n;
            S(int n) { this.n = n; }
        }
        int main() { return 0; }
    "#, "n'est pas injectable");
}

#[test]
fn tc_err_captive_dependency() {
    // Un singleton ne peut pas dépendre d'un transient
    assert_tc_err(r#"
        transient service class Ctx {}
        service class Holder {
            Ctx ctx;
            Holder(Ctx ctx) { this.ctx = ctx; }
        }
        int main() { return 0; }
    "#, "captive");
}

#[test]
fn tc_err_transient_without_service() {
    assert_tc_err(r#"
        transient class Foo {}
        int main() { return 0; }
    "#, "`transient` nécessite `service`");
}

#[test]
fn tc_err_still_ambiguous_without_bind() {
    // Sans bind, deux implémentations restent une erreur (régression phase 1)
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

// ─────────────────────────────────────────────────────────────────────────────
//  Exécution
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_bind_selects_implementation() {
    let (ret, lines) = run_output(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print("console:", msg); }
        }
        service class FileLogger implements Logger {
            void log(string msg) { print("file:", msg); }
        }
        module AppModule {
            bind Logger to FileLogger;
        }
        int main() {
            Logger l = inject Logger;
            l.log("hello");
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["file: hello"]);
}

#[test]
fn run_bind_applies_to_dependencies_too() {
    // Le binding s'applique aussi aux dépendances de constructeur
    let (ret, lines) = run_output(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print("console:", msg); }
        }
        service class FileLogger implements Logger {
            void log(string msg) { print("file:", msg); }
        }
        service class Job {
            Logger logger;
            Job(Logger logger) { this.logger = logger; }
            void run() { logger.log("job"); }
        }
        module AppModule {
            bind Logger to FileLogger;
        }
        int main() {
            Job j = inject Job;
            j.run();
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["file: job"]);
}

#[test]
fn run_with_config_values() {
    let (ret, lines) = run_output(r#"
        service class HttpClient {
            string baseUrl;
            int timeout;
            HttpClient(string baseUrl, int timeout) {
                this.baseUrl = baseUrl;
                this.timeout = timeout;
            }
            string describe() { return baseUrl + ":" + timeout.toString(); }
        }
        module AppModule {
            bind HttpClient with ("https://api", 30);
        }
        int main() {
            HttpClient c = inject HttpClient;
            print(c.describe());
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["https://api:30"]);
}

#[test]
fn run_mixed_dependency_and_config() {
    let (ret, lines) = run_output(r#"
        interface Logger { void log(string msg); }
        service class ConsoleLogger implements Logger {
            void log(string msg) { print("LOG:", msg); }
        }
        service class Job {
            Logger logger;
            string name;
            Job(Logger logger, string name) {
                this.logger = logger;
                this.name = name;
            }
            void run() { logger.log("start " + name); }
        }
        module AppModule {
            bind Job with ("batch");
        }
        int main() {
            Job j = inject Job;
            j.run();
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["LOG: start batch"]);
}

#[test]
fn run_bind_to_with_combined() {
    let (ret, lines) = run_output(r#"
        interface Logger { void log(string msg); }
        service class FileLogger implements Logger {
            string path;
            FileLogger(string path) { this.path = path; }
            void log(string msg) { print(path, ">", msg); }
        }
        module AppModule {
            bind Logger to FileLogger with ("app.log");
        }
        int main() {
            Logger l = inject Logger;
            l.log("ok");
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["app.log > ok"]);
}

#[test]
fn run_transient_creates_fresh_instances() {
    // Chaque inject d'un transient retourne une nouvelle instance
    let ret = run_ok(r#"
        transient service mut class Counter {
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
    assert_eq!(ret, 1);
}

#[test]
fn run_singleton_still_shared() {
    // Régression phase 1 : sans transient, le singleton reste partagé
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
