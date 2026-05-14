//! Tests des fonctions de haut niveau (en dehors de toute classe).
//! Chaque test couvre un aspect précis de la fonctionnalité.

use chumsky::Parser;
use mini_parser::parser::program_parser;
use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    match program_parser().parse(full.as_str()) {
        Ok(_) => {}
        Err(errs) => panic!(
            "Parse échoué :\n{}\n---\n{}",
            src,
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        ),
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck échoué :\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!("Typecheck aurait dû échouer (attendu '{}') :\n{}", fragment, src),
        Err(e) => {
            let all = e.join("\n");
            assert!(all.contains(fragment),
                "Attendu '{}' dans :\n{}", fragment, all);
        }
    }
}

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error :\n{}\n---\n{}", src, e),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//  Parsing
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_simple_toplevel_func() {
    parses_ok(r#"
        int add(int a, int b) { return a + b; }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_void_toplevel_func() {
    parses_ok(r#"
        void greet(string name) { print(name); }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_multiple_toplevel_funcs() {
    parses_ok(r#"
        int double(int x) { return x * 2; }
        int square(int x) { return x * x; }
        bool isPositive(int x) { return x > 0; }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_func_with_no_params() {
    parses_ok(r#"
        int answer() { return 42; }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_func_before_and_after_class() {
    parses_ok(r#"
        int helper(int x) { return x + 1; }
        class Foo { int v; Foo(int v) { this.v = v; } }
        int other(int x) { return x - 1; }
        int main() { return 0; }
    "#);
}

// ═════════════════════════════════════════════════════════════════════════════
//  Typecheck — cas valides
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn tc_func_return_type_matches() {
    assert_tc_ok(r#"
        int add(int a, int b) { return a + b; }
        int main() { int r = add(1, 2); return r; }
    "#);
}

#[test]
fn tc_func_bool_return() {
    assert_tc_ok(r#"
        bool isEven(int n) { return n % 2 == 0; }
        int main() {
            bool b = isEven(4);
            return 0;
        }
    "#);
}

#[test]
fn tc_func_string_param() {
    assert_tc_ok(r#"
        void say(string msg) { print(msg); }
        int main() { say("bonjour"); return 0; }
    "#);
}

#[test]
fn tc_func_used_in_expression() {
    assert_tc_ok(r#"
        int triple(int x) { return x * 3; }
        int main() {
            int r = triple(4) + 1;
            return r;
        }
    "#);
}

// ═════════════════════════════════════════════════════════════════════════════
//  Typecheck — erreurs
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn tc_func_wrong_arg_type() {
    assert_tc_err(r#"
        int double(int x) { return x * 2; }
        int main() {
            int r = double(true);
            return r;
        }
    "#, "incompatible");
}

#[test]
fn tc_func_wrong_arg_count() {
    assert_tc_err(r#"
        int add(int a, int b) { return a + b; }
        int main() {
            int r = add(1);
            return r;
        }
    "#, "attendus");
}

#[test]
fn tc_unknown_toplevel_func() {
    assert_tc_err(r#"
        int main() {
            int r = doesNotExist(1);
            return r;
        }
    "#, "inconnue");
}

// ═════════════════════════════════════════════════════════════════════════════
//  Interprétation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn interp_func_add() {
    assert_eq!(run_ok(r#"
        int add(int a, int b) { return a + b; }
        int main() { return add(3, 4); }
    "#), 7);
}

#[test]
fn interp_func_no_params() {
    assert_eq!(run_ok(r#"
        int answer() { return 42; }
        int main() { return answer(); }
    "#), 42);
}

#[test]
fn interp_func_with_local_vars() {
    // Somme 1+2+...+10 = 55
    assert_eq!(run_ok(r#"
        int sumUpTo(int n) {
            int s = 0;
            for (int i = 1; i <= n; i = i + 1) {
                s = s + i;
            }
            return s;
        }
        int main() { return sumUpTo(10); }
    "#), 55);
}

#[test]
fn interp_func_factorial() {
    // 7! = 5040
    assert_eq!(run_ok(r#"
        int factorial(int n) {
            int r = 1;
            int i = n;
            while (i > 1) { r = r * i; i = i - 1; }
            return r;
        }
        int main() { return factorial(7); }
    "#), 5040);
}

#[test]
fn interp_func_called_multiple_times() {
    // 4 + 9 + 16 = 29
    assert_eq!(run_ok(r#"
        int square(int x) { return x * x; }
        int main() {
            return square(2) + square(3) + square(4);
        }
    "#), 29);
}

#[test]
fn interp_func_bool_return() {
    assert_eq!(run_ok(r#"
        bool isEven(int n) { return n % 2 == 0; }
        int main() {
            if (isEven(8)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_func_params_are_local() {
    // x dans addOne (=5) ne doit pas écraser x dans main (=10)
    assert_eq!(run_ok(r#"
        int addOne(int x) { return x + 1; }
        int main() {
            int x = 10;
            int r = addOne(5);
            return r;
        }
    "#), 6);
}

#[test]
fn interp_func_local_vars_dont_leak() {
    // Les variables locales de la fonction ne doivent pas être visibles dans main
    assert_eq!(run_ok(r#"
        int compute(int n) {
            int local = n * 99;
            return local;
        }
        int main() {
            int result = compute(2);  // local = 198 dans compute
            return result;
        }
    "#), 198);
}

#[test]
fn interp_multiple_funcs_composition() {
    // double(inc(20)) = (20+1)*2 = 42
    assert_eq!(run_ok(r#"
        int double(int x)  { return x * 2; }
        int inc(int x)     { return x + 1; }
        int main() {
            return double(inc(20));
        }
    "#), 42);
}

#[test]
fn interp_func_with_class_param() {
    // Fonction de haut niveau prenant un objet en paramètre
    assert_eq!(run_ok(r#"
        class Counter {
            int n;
            Counter(int start) { n = start; }
            int get() { return n; }
        }
        int getValue(Counter c) { return c.get(); }
        int main() {
            Counter c = new Counter(99);
            return getValue(c);
        }
    "#), 99);
}

#[test]
fn interp_func_conditional() {
    // classify(-5)=-1, classify(0)=0, classify(3)=1 → sum = 0
    assert_eq!(run_ok(r#"
        int classify(int n) {
            if (n < 0)  { return -1; }
            if (n == 0) { return 0;  }
            return 1;
        }
        int main() {
            return classify(-5) + classify(0) + classify(3);
        }
    "#), 0);
}

#[test]
fn interp_func_with_loop_and_break() {
    // Trouve le premier multiple de 7 au-delà de 40
    assert_eq!(run_ok(r#"
        int firstMultipleAbove(int factor, int threshold) {
            int n = threshold + 1;
            while (n % factor != 0) {
                n = n + 1;
            }
            return n;
        }
        int main() {
            return firstMultipleAbove(7, 40);  // 42
        }
    "#), 42);
}
