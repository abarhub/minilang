//! Tests des lambdas : parsing, typecheck, interprétation.

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
                "Expected '{}' in:\n{}", fragment, all);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  PARSING
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_lambda_single_param() {
    parses_ok("int main() { fn f = x => x; return 0; }");
}

#[test]
fn parse_lambda_multi_param() {
    parses_ok("int main() { fn f = (x, y) => x; return 0; }");
}

#[test]
fn parse_lambda_no_param() {
    parses_ok("int main() { fn f = () => 42; return 0; }");
}

#[test]
fn parse_lambda_block_body() {
    parses_ok("int main() { fn f = x => { return x; }; return 0; }");
}

#[test]
fn parse_lambda_typed_single() {
    parses_ok("int main() { fn(int) -> int f = x => x + 1; return 0; }");
}

#[test]
fn parse_lambda_typed_multi() {
    parses_ok("int main() { fn(int, int) -> int add = (a, b) => a + b; return 0; }");
}

#[test]
fn parse_lambda_typed_no_param() {
    parses_ok("int main() { fn() -> int get = () => 42; return 0; }");
}

#[test]
fn parse_lambda_typed_returns_fn() {
    parses_ok("int main() { fn(int) -> fn(int) -> int f = x => y => x + y; return 0; }");
}

#[test]
fn parse_type_alias() {
    parses_ok("type MyFn = fn(int) -> int; int main() { return 0; }");
}

#[test]
fn parse_type_alias_used() {
    parses_ok("type IntOp = fn(int) -> int; int main() { IntOp f = x => x; return 0; }");
}

#[test]
fn parse_lambda_call_direct() {
    parses_ok("int main() { fn(int)->int f = x => x; return f(1); }");
}

#[test]
fn parse_lambda_call_inline() {
    parses_ok("int main() { return (x => x)(5); }");
}

#[test]
fn parse_lambda_nested() {
    parses_ok("int main() { fn f = x => y => x; return 0; }");
}

#[test]
fn parse_method_typed_fn_param() {
    parses_ok(r#"
        class C {
            C() {}
            int apply(fn(int) -> int f) { return f(1); }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_method_returns_fn_type() {
    parses_ok(r#"
        class C {
            int v;
            C(int x) { v = x; }
            fn(int) -> int getAdder() { return x => x + v; }
        }
        int main() { return 0; }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  TYPECHECK
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_untyped_lambda_ok() {
    tc_ok("int main() { fn f = x => x; return 0; }");
}

#[test]
fn tc_typed_lambda_ok() {
    tc_ok("int main() { fn(int) -> int f = x => x + 1; return 0; }");
}

#[test]
fn tc_typed_lambda_block_ok() {
    tc_ok("int main() { fn(int) -> int f = x => { return x * 2; }; return 0; }");
}

#[test]
fn tc_type_alias_ok() {
    tc_ok("type F = fn(int) -> int; int main() { F f = x => x; return 0; }");
}

#[test]
fn tc_type_alias_multi_ok() {
    tc_ok("type Op = fn(int, int) -> int; int main() { Op f = (a, b) => a + b; return 0; }");
}

#[test]
fn tc_typed_lambda_wrong_return() {
    tc_err(
        r#"int main() { fn(int) -> int f = x => true; return 0; }"#,
        "attendu",
    );
}

#[test]
fn tc_typed_lambda_wrong_arg_count() {
    tc_err(
        r#"int main() { fn(int, int) -> int f = x => x; return 0; }"#,
        "paramètre",
    );
}

#[test]
fn tc_typed_lambda_call_wrong_type() {
    tc_err(
        r#"int main() { fn(int) -> int f = x => x; int r = f(true); return r; }"#,
        "≠",
    );
}

#[test]
fn tc_fn_param_typed_ok() {
    tc_ok(r#"
        class C {
            C() {}
            int apply(fn(int) -> int f) { return f(1); }
        }
        int main() {
            C c = new C();
            int r = c.apply(x => x * 2);
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  INTERPRÉTEUR
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_identity_untyped() {
    assert_eq!(ok("int main() { fn f = x => x; return f(7); }"), 7);
}

#[test]
fn interp_identity_typed() {
    assert_eq!(ok("int main() { fn(int) -> int f = x => x; return f(7); }"), 7);
}

#[test]
fn interp_add_typed() {
    assert_eq!(ok("int main() { fn(int,int) -> int add = (a, b) => a + b; return add(3, 4); }"), 7);
}

#[test]
fn interp_lambda_arithmetic() {
    // (x, y) => x + y * 8  avec x=1, y=2  → 17
    assert_eq!(ok("int main() {
        fn(int,int) -> int f = (x, y) => x + y * 8;
        return f(1, 2);
    }"), 17);
}

#[test]
fn interp_lambda_block() {
    assert_eq!(ok("int main() {
        fn(int) -> int double = x => { return x * 2; };
        return double(21);
    }"), 42);
}

#[test]
fn interp_no_param() {
    assert_eq!(ok("int main() { fn() -> int get42 = () => 42; return get42(); }"), 42);
}

#[test]
fn interp_type_alias() {
    assert_eq!(ok("type F = fn(int) -> int; int main() { F f = x => x * 3; return f(14); }"), 42);
}

#[test]
fn interp_inline_call() {
    assert_eq!(ok("int main() { return (x => x * x)(6); }"), 36);
}

#[test]
fn interp_captures_variable() {
    assert_eq!(ok("int main() {
        int base = 10;
        fn(int) -> int add_base = x => x + base;
        return add_base(5);
    }"), 15);
}

#[test]
fn interp_capture_frozen_at_creation() {
    // La valeur capturée est figée au moment de la création
    assert_eq!(ok("int main() {
        int n = 1;
        fn(int) -> int f = x => x + n;
        n = 100;   // ne modifie pas la capture
        return f(0);
    }"), 1);
}

#[test]
fn interp_lambda_passed_to_method() {
    assert_eq!(ok(r#"
        class C {
            int v;
            C(int x) { v = x; }
            int apply(fn(int) -> int f) { return f(v); }
        }
        int main() {
            C c = new C(6);
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
            fn(int) -> int makeAdder() { return x => x + offset; }
        }
        int main() {
            Builder b = new Builder(100);
            fn(int) -> int adder = b.makeAdder();
            return adder(42);
        }
    "#), 142);
}

#[test]
fn interp_higher_order_sum() {
    assert_eq!(ok("int main() {
        fn(int) -> int double = x => x * 2;
        int sum = 0;
        for (int i = 1; i <= 5; i = i + 1) {
            sum = sum + double(i);
        }
        return sum;
    }"), 30);
}

#[test]
fn interp_lambda_conditional() {
    assert_eq!(ok("int main() {
        fn(int) -> int abs_val = x => {
            if (x < 0) { return -x; }
            return x;
        };
        return abs_val(-7) + abs_val(3);
    }"), 10);
}

#[test]
fn interp_compose_two_lambdas() {
    assert_eq!(ok("int main() {
        fn(int) -> int add1   = x => x + 1;
        fn(int) -> int times3 = x => x * 3;
        return times3(add1(4));  // (4+1)*3 = 15
    }"), 15);
}

#[test]
fn interp_lambda_factorial() {
    assert_eq!(ok("int main() {
        fn(int) -> int fact = n => {
            int r = 1; int i = n;
            while (i > 1) { r = r * i; i = i - 1; }
            return r;
        };
        return fact(6);
    }"), 720);
}

#[test]
fn interp_nested_lambda() {
    // x => y => x (ignore y, retourne x)
    assert_eq!(ok("int main() {
        fn outer = x => y => x;
        fn inner = outer(42);
        return inner(0);
    }"), 42);
}

#[test]
fn interp_wrong_arg_count_fails() {
    assert!(run_source("int main() { fn(int,int) -> int f = (a,b) => a+b; return f(1); }").is_err());
}
