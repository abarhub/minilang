//! Tests des classes Boolean, Integer, Float, Double — minilang stdlib.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;
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
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
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
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Boolean
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_bool_to_string() {
    parses_ok(r#"
        int main() {
            bool b = true;
            string s = b.toString();
            return 0;
        }
    "#);
}

#[test]
fn tc_bool_to_string_returns_string() {
    assert_tc_ok(r#"
        int main() {
            bool b = false;
            string s = b.toString();
            return 0;
        }
    "#);
}

#[test]
fn tc_bool_not_returns_bool() {
    assert_tc_ok(r#"
        int main() {
            bool b = true;
            bool n = b.not();
            return 0;
        }
    "#);
}

#[test]
fn tc_bool_and_wrong_arg() {
    assert_tc_err(r#"
        int main() {
            bool b = true;
            bool r = b.and(1);
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn interp_bool_not_true() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = true;
            if (b.not()) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_bool_not_false() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = false;
            if (b.not()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_bool_and() {
    assert_eq!(run_ok(r#"
        int main() {
            bool a = true;
            bool b = false;
            if (a.and(b)) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_bool_or() {
    assert_eq!(run_ok(r#"
        int main() {
            bool a = false;
            bool b = true;
            if (a.or(b)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_bool_equals_true() {
    assert_eq!(run_ok(r#"
        int main() {
            bool a = true;
            if (a.equals(true)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_bool_equals_false() {
    assert_eq!(run_ok(r#"
        int main() {
            bool a = true;
            if (a.equals(false)) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_bool_to_string_true() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = true;
            string s = b.toString();
            return s.length();
        }
    "#), 4); // "true" = 4 chars
}

#[test]
fn interp_bool_to_string_false() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = false;
            string s = b.toString();
            return s.length();
        }
    "#), 5); // "false" = 5 chars
}

// ─────────────────────────────────────────────────────────────────────────────
//  Integer
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_int_abs() {
    parses_ok(r#"
        int main() {
            int n = -5;
            return n.abs();
        }
    "#);
}

#[test]
fn tc_int_abs_returns_int() {
    assert_tc_ok(r#"
        int main() {
            int n = -3;
            int a = n.abs();
            return a;
        }
    "#);
}

#[test]
fn tc_int_to_float_returns_float() {
    assert_tc_ok(r#"
        int main() {
            int n = 5;
            float f = n.toFloat();
            return 0;
        }
    "#);
}

#[test]
fn tc_int_to_string_returns_string() {
    assert_tc_ok(r#"
        int main() {
            int n = 42;
            string s = n.toString();
            return 0;
        }
    "#);
}

#[test]
fn tc_int_min_wrong_arg() {
    assert_tc_err(r#"
        int main() {
            int n = 5;
            int m = n.min(true);
            return m;
        }
    "#, "incompatible");
}

#[test]
fn interp_int_abs_negative() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = -42;
            return n.abs();
        }
    "#), 42);
}

#[test]
fn interp_int_abs_positive() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 7;
            return n.abs();
        }
    "#), 7);
}

#[test]
fn interp_int_min() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 3;
            return a.min(7);
        }
    "#), 3);
}

#[test]
fn interp_int_max() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 3;
            return a.max(7);
        }
    "#), 7);
}

#[test]
fn interp_int_pow() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 2;
            return n.pow(10);
        }
    "#), 1024);
}

#[test]
fn interp_int_is_positive() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 5;
            if (n.isPositive()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_is_negative() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = -3;
            if (n.isNegative()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_is_zero() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 0;
            if (n.isZero()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_is_even() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 4;
            if (n.isEven()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_is_odd() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 7;
            if (n.isOdd()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_compare_to_less() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 3;
            return a.compareTo(5);
        }
    "#), -1);
}

#[test]
fn interp_int_compare_to_equal() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 5;
            return a.compareTo(5);
        }
    "#), 0);
}

#[test]
fn interp_int_compare_to_greater() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 9;
            return a.compareTo(5);
        }
    "#), 1);
}

#[test]
fn interp_int_equals_true() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 42;
            if (n.equals(42)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_int_to_binary_string() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 10;
            string s = n.toBinaryString();
            return s.length();
        }
    "#), 4); // "1010"
}

#[test]
fn interp_int_to_string_length() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 12345;
            string s = n.toString();
            return s.length();
        }
    "#), 5);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Float
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_float_abs() {
    parses_ok(r#"
        int main() {
            float f = -3.5;
            float a = f.abs();
            return 0;
        }
    "#);
}

#[test]
fn tc_float_abs_returns_float() {
    assert_tc_ok(r#"
        int main() {
            float f = -1.5;
            float a = f.abs();
            return 0;
        }
    "#);
}

#[test]
fn tc_float_to_int_returns_int() {
    assert_tc_ok(r#"
        int main() {
            float f = 3.7;
            int n = f.toInt();
            return n;
        }
    "#);
}

#[test]
fn interp_float_abs() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = -3.5;
            float a = f.abs();
            return a.toInt();
        }
    "#), 3);
}

#[test]
fn interp_float_floor() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 3.9;
            return f.floor().toInt();
        }
    "#), 3);
}

#[test]
fn interp_float_ceil() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 3.1;
            return f.ceil().toInt();
        }
    "#), 4);
}

#[test]
fn interp_float_round_up() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 3.6;
            return f.round().toInt();
        }
    "#), 4);
}

#[test]
fn interp_float_round_down() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 3.4;
            return f.round().toInt();
        }
    "#), 3);
}

#[test]
fn interp_float_is_positive() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 1.5;
            if (f.isPositive()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_float_is_negative() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = -0.5;
            if (f.isNegative()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_float_to_int_truncates() {
    assert_eq!(run_ok(r#"
        int main() {
            float f = 7.9;
            return f.toInt();
        }
    "#), 7);
}

#[test]
fn interp_float_min() {
    assert_eq!(run_ok(r#"
        int main() {
            float a = 2.5;
            float b = 8.0;
            return a.min(b).toInt();
        }
    "#), 2);
}

#[test]
fn interp_float_max() {
    assert_eq!(run_ok(r#"
        int main() {
            float a = 2.5;
            float b = 8.0;
            return a.max(b).toInt();
        }
    "#), 8);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Double
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_double_floor() {
    parses_ok(r#"
        int main() {
            double d = 4.9;
            double f = d.floor();
            return 0;
        }
    "#);
}

#[test]
fn tc_double_abs_returns_double() {
    assert_tc_ok(r#"
        int main() {
            double d = -2.5;
            double a = d.abs();
            return 0;
        }
    "#);
}

#[test]
fn tc_double_to_int_returns_int() {
    assert_tc_ok(r#"
        int main() {
            double d = 5.9;
            int n = d.toInt();
            return n;
        }
    "#);
}

#[test]
fn interp_double_abs() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = -9.9;
            double a = d.abs();
            return a.toInt();
        }
    "#), 9);
}

#[test]
fn interp_double_floor() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = 5.8;
            return d.floor().toInt();
        }
    "#), 5);
}

#[test]
fn interp_double_ceil() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = 5.2;
            return d.ceil().toInt();
        }
    "#), 6);
}

#[test]
fn interp_double_to_int_truncates() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = 3.99;
            return d.toInt();
        }
    "#), 3);
}

#[test]
fn interp_double_is_positive() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = 1.5;
            if (d.isPositive()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_double_min_max() {
    assert_eq!(run_ok(r#"
        int main() {
            double a = 1.5;
            double b = 9.5;
            return a.min(b).toInt() + a.max(b).toInt();
        }
    "#), 10); // 1 + 9
}
