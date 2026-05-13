//! Tests du type Array<T> — minilang stdlib.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;
use chumsky::Parser;
use mini_parser::parser::program_parser;

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
        Ok(()) => panic!("Should have failed (expected '{}'):\n{}", fragment, src),
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

fn run_fails(src: &str) {
    if run_source(src).is_ok() {
        panic!("Should have failed:\n{}", src);
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

#[test]
fn parse_array_lit() {
    parses_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            return 0;
        }
    "#);
}

#[test]
fn parse_array_new_size() {
    parses_ok(r#"
        int main() {
            int[] a = new int[5];
            return 0;
        }
    "#);
}

#[test]
fn parse_array_index_access() {
    parses_ok(r#"
        int main() {
            int[] a = new int[]{10, 20};
            int x = a[0];
            return x;
        }
    "#);
}

#[test]
fn parse_array_index_assign() {
    parses_ok(r#"
        int main() {
            int[] a = new int[2];
            a[0] = 42;
            return 0;
        }
    "#);
}

// ── Typecheck ─────────────────────────────────────────────────────────────────

#[test]
fn tc_array_lit_ok() {
    assert_tc_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            return 0;
        }
    "#);
}

#[test]
fn tc_array_new_ok() {
    assert_tc_ok(r#"
        int main() {
            bool[] a = new bool[10];
            return 0;
        }
    "#);
}

#[test]
fn tc_array_index_ok() {
    assert_tc_ok(r#"
        int main() {
            int[] a = new int[]{7};
            int x = a[0];
            return x;
        }
    "#);
}

#[test]
fn tc_array_wrong_elem_type() {
    assert_tc_err(r#"
        int main() {
            int[] a = new int[]{true};
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn tc_array_wrong_assign_type() {
    assert_tc_err(r#"
        int main() {
            int[] a = new int[2];
            a[0] = true;
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn tc_array_index_must_be_int() {
    assert_tc_err(r#"
        int main() {
            int[] a = new int[]{1};
            int x = a[true];
            return x;
        }
    "#, "int");
}

#[test]
fn tc_array_length_returns_int() {
    assert_tc_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            int n = a.length();
            return n;
        }
    "#);
}

#[test]
fn tc_array_contains_returns_bool() {
    assert_tc_ok(r#"
        int main() {
            int[] a = new int[]{1, 2};
            bool b = a.contains(1);
            return 0;
        }
    "#);
}

// ── Interprétation ────────────────────────────────────────────────────────────

#[test]
fn interp_array_lit_get() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{10, 20, 30};
            return a[1];
        }
    "#), 20);
}

#[test]
fn interp_array_new_default_int() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[3];
            return a[2];
        }
    "#), 0);
}

#[test]
fn interp_array_assign() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[3];
            a[1] = 99;
            return a[1];
        }
    "#), 99);
}

#[test]
fn interp_array_length() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{5, 6, 7, 8};
            return a.length();
        }
    "#), 4);
}

#[test]
fn interp_array_contains_true() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            if (a.contains(2)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_array_contains_false() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            if (a.contains(9)) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_array_fill() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3};
            a.fill(7);
            return a[0] + a[1] + a[2];
        }
    "#), 21);
}

#[test]
fn interp_array_get_method() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{100, 200};
            return a.get(0);
        }
    "#), 100);
}

#[test]
fn interp_array_set_method() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[2];
            a.set(0, 42);
            return a.get(0);
        }
    "#), 42);
}

#[test]
fn interp_array_oob_panics() {
    run_fails(r#"
        int main() {
            int[] a = new int[]{1};
            return a[5];
        }
    "#);
}

#[test]
fn interp_array_negative_index_panics() {
    run_fails(r#"
        int main() {
            int[] a = new int[]{1, 2};
            return a[-1];
        }
    "#);
}

#[test]
fn interp_array_loop_sum() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{1, 2, 3, 4, 5};
            int sum = 0;
            int i = 0;
            while (i < a.length()) {
                sum = sum + a[i];
                i = i + 1;
            }
            return sum;
        }
    "#), 15);
}

#[test]
fn interp_array_bool_type() {
    assert_eq!(run_ok(r#"
        int main() {
            bool[] flags = new bool[]{true, false, true};
            if (flags[0]) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_array_in_result() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] a = new int[]{42};
            Result<int[], string> r = Result<int[], string>::Ok(a);
            int[] b = r.getValue();
            return b[0];
        }
    "#), 42);
}
