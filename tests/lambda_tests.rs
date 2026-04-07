//! Tests des lambdas et fermetures.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;
use chumsky::Parser;
use mini_parser::parser::program_parser;

fn parses_ok(src: &str) {
    if let Err(e) = program_parser().parse(src) {
        panic!("Parse failed:\n{}\n---\n{}",
            src, e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"));
    }
}

fn ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error in:\n{}\n---\n{}", src, e),
    }
}

fn tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!("Should have failed (expected '{}'):\n{}", fragment, src),
        Err(e) => {
            let all = e.join("\n");
            assert!(all.contains(fragment),
                "Expected '{}' in errors:\n{}", fragment, all);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  PARSING
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_lambda_single_param_expr() {
    parses_ok("int main() { fn f = x => x; return 0; }");
}

#[test]
fn parse_lambda_multi_param_expr() {
    parses_ok("int main() { fn f = (x, y) => x; return 0; }");
}

#[test]
fn parse_lambda_no_param() {
    parses_ok("int main() { fn f = () => 42; return 0; }");
}

#[test]
fn parse_lambda_block_body() {
    parses_ok(r#"int main() {
        fn f = (x, y) => { return x; };
        return 0;
    }"#);
}

#[test]
fn parse_lambda_single_param_block() {
    parses_ok("int main() { fn f = x => { return x; }; return 0; }");
}

#[test]
fn parse_lambda_call_direct() {
    parses_ok("int main() { fn f = x => x; return f(1); }");
}

#[test]
fn parse_lambda_call_inline() {
    // Appel direct d'une lambda anonyme via paren_or_call (expr)(args)
    parses_ok("int main() { return (x => x)(5); }");
}

#[test]
fn parse_lambda_call_multi_inline() {
    parses_ok("int main() { int r = ((a, b) => a + b)(3, 4); return r; }");
}

#[test]
fn parse_lambda_call_multi() {
    parses_ok("int main() { fn add = (a, b) => a; return add(1, 2); }");
}

#[test]
fn parse_lambda_in_expression() {
    parses_ok("int main() { int r = (x => x)(3); return r; }");
}

#[test]
fn parse_lambda_arithmetic_body() {
    parses_ok("int main() { fn f = (x, y) => x + y * 8; return 0; }");
}

#[test]
fn parse_lambda_nested() {
    parses_ok("int main() { fn outer = x => y => x; return 0; }");
}

// ─────────────────────────────────────────────────────────────────────────────
//  TYPECHECK
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_lambda_stored_as_fn() {
    tc_ok("int main() { fn f = x => x; return 0; }");
}

#[test]
fn tc_lambda_multi_param() {
    tc_ok("int main() { fn add = (a, b) => a; return 0; }");
}

#[test]
fn tc_lambda_block() {
    tc_ok(r#"int main() {
        fn f = (x) => { return x; };
        return 0;
    }"#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  INTERPRÉTEUR
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_identity_lambda() {
    assert_eq!(ok("int main() { fn f = x => x; return f(7); }"), 7);
}

#[test]
fn interp_add_lambda() {
    assert_eq!(ok("int main() { fn add = (a, b) => a + b; return add(3, 4); }"), 7);
}

#[test]
fn interp_lambda_arithmetic() {
    // (x, y) => x + y * 8  avec x=1, y=2  → 1 + 16 = 17
    assert_eq!(ok("int main() {
        fn f = (x, y) => x + y * 8;
        return f(1, 2);
    }"), 17);
}

#[test]
fn interp_lambda_block_body() {
    assert_eq!(ok("int main() {
        fn double = x => { return x * 2; };
        return double(21);
    }"), 42);
}

#[test]
fn interp_lambda_block_multi_stmts() {
    assert_eq!(ok("int main() {
        fn f = (a, b) => {
            int sum = a + b;
            int product = a * b;
            return sum + product;
        };
        return f(3, 4);
    }"), 19); // (3+4) + (3*4) = 7 + 12 = 19
}

#[test]
fn interp_lambda_no_params() {
    assert_eq!(ok("int main() { fn get42 = () => 42; return get42(); }"), 42);
}

#[test]
fn interp_inline_lambda_call() {
    // Appel direct de lambda anonyme via postfix
    assert_eq!(ok("int main() { return (x => x * x)(6); }"), 36);
}

#[test]
fn interp_lambda_captures_variable() {
    // La lambda capture `base` défini dans le scope courant
    assert_eq!(ok("int main() {
        int base = 10;
        fn add_base = x => x + base;
        return add_base(5);
    }"), 15);
}

#[test]
fn interp_lambda_capture_does_not_see_later_change() {
    // La capture est faite au moment de la création : valeur de `n` = 1
    assert_eq!(ok("int main() {
        int n = 1;
        fn f = x => x + n;
        n = 100;          // ne modifie pas la capture
        return f(0);
    }"), 1);
}

#[test]
fn interp_lambda_reassigned() {
    assert_eq!(ok("int main() {
        fn f = x => x + 1;
        f = x => x + 10;
        return f(5);
    }"), 15);
}

#[test]
fn interp_lambda_passed_to_method() {
    assert_eq!(ok(r#"
        class Calc {
            int val;
            Calc(int v) { val = v; }
            int apply(fn f) { return f(val); }
        }
        int main() {
            Calc c = new Calc(6);
            return c.apply(x => x * 7);
        }
    "#), 42);
}

#[test]
fn interp_lambda_returned_from_method() {
    assert_eq!(ok(r#"
        class Builder {
            int offset;
            Builder(int n) { offset = n; }
            fn makeAdder() { return x => x + offset; }
        }
        int main() {
            Builder b = new Builder(100);
            fn adder = b.makeAdder();
            return adder(42);
        }
    "#), 142);
}

#[test]
fn interp_higher_order_map_sum() {
    // Simule map+sum : applique f à 1..5 et additionne
    assert_eq!(ok("int main() {
        fn double = x => x * 2;
        int sum = 0;
        for (int i = 1; i <= 5; i = i + 1) {
            sum = sum + double(i);
        }
        return sum;
    }"), 30); // 2+4+6+8+10 = 30
}

#[test]
fn interp_lambda_uses_outer_loop_var() {
    // À chaque itération, la lambda capture la valeur courante de `acc`
    assert_eq!(ok("int main() {
        int acc = 0;
        for (int i = 1; i <= 5; i = i + 1) {
            fn add = x => acc + x;
            acc = add(i);
        }
        return acc;
    }"), 15); // 1+2+3+4+5
}

#[test]
fn interp_lambda_conditional() {
    assert_eq!(ok("int main() {
        fn abs_val = x => {
            if (x < 0) { return -x; }
            return x;
        };
        int a = abs_val(-7);
        int b = abs_val(3);
        return a + b;
    }"), 10);
}

#[test]
fn interp_lambda_recursive_via_var() {
    // Factorielle itérative dans une lambda
    assert_eq!(ok("int main() {
        fn fact = n => {
            int r = 1;
            int i = n;
            while (i > 1) { r = r * i; i = i - 1; }
            return r;
        };
        return fact(6);
    }"), 720);
}

#[test]
fn interp_two_lambdas_compose() {
    // compose manuellement : f(g(x))
    assert_eq!(ok("int main() {
        fn add1  = x => x + 1;
        fn times3 = x => x * 3;
        return times3(add1(4));
    }"), 15); // (4+1)*3 = 15
}

#[test]
fn interp_lambda_string_body() {
    // Lambda sur les strings
    assert_eq!(ok(r#"int main() {
        fn greet = name => "Hello " + name;
        string s = greet("world");
        print(s);
        return 0;
    }"#), 0);
}

#[test]
fn interp_wrong_arg_count_fails() {
    // Trop peu d'arguments → RuntimeError
    assert!(run_source("int main() { fn f = (a, b) => a + b; return f(1); }").is_err());
}
