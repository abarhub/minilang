//! Tests de la Phase 1 du système d'immutabilité — minilang.
//! Couvre : readonly / immutable sur les variables, mutable sur les méthodes.

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

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing — qualificateurs de variables
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_readonly_variable() {
    parses_ok(r#"
        int main() {
            readonly int x = 42;
            return x;
        }
    "#);
}

#[test]
fn parse_immutable_variable() {
    parses_ok(r#"
        int main() {
            immutable int x = 10;
            return x;
        }
    "#);
}

#[test]
fn parse_mutable_method() {
    parses_ok(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_readonly_object() {
    parses_ok(r#"
        class Box {
            int value;
            mutable void set(int v) { value = v; }
            int get() { return value; }
        }
        int main() {
            Box b = new Box();
            readonly Box rb = b;
            return 0;
        }
    "#);
}

#[test]
fn parse_immutable_object() {
    parses_ok(r#"
        class Point {
            int x;
            int y;
            int getX() { return x; }
        }
        int main() {
            immutable Point p = new Point();
            return p.getX();
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — cas valides
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_mutable_can_call_mutable_method() {
    assert_tc_ok(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() {
            Counter c = new Counter();
            c.increment();
            return c.get();
        }
    "#);
}

#[test]
fn tc_readonly_can_call_non_mutable_method() {
    assert_tc_ok(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() {
            Counter c = new Counter();
            readonly Counter rc = c;
            return rc.get();
        }
    "#);
}

#[test]
fn tc_immutable_can_call_non_mutable_method() {
    assert_tc_ok(r#"
        class Point {
            int x;
            int getX() { return x; }
        }
        int main() {
            immutable Point p = new Point();
            return p.getX();
        }
    "#);
}

#[test]
fn tc_non_mutable_method_calls_non_mutable_on_this() {
    // Une méthode non-mutable peut appeler une autre méthode non-mutable sur this
    assert_tc_ok(r#"
        class Pair {
            int a;
            int b;
            int getA() { return a; }
            int sum() { return this.getA() + b; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn tc_mutable_method_can_call_mutable_on_this() {
    assert_tc_ok(r#"
        class Counter {
            int value;
            mutable void reset() { value = 0; }
            mutable void resetAndIncrement() {
                this.reset();
                value = value + 1;
            }
            int get() { return value; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn tc_readonly_primitive_read_ok() {
    assert_tc_ok(r#"
        int main() {
            readonly int x = 5;
            readonly int y = x;
            return y;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — erreurs attendues
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_readonly_calls_mutable() {
    assert_tc_err(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() {
            Counter c = new Counter();
            readonly Counter rc = c;
            rc.increment();
            return 0;
        }
    "#, "readonly");
}

#[test]
fn tc_err_immutable_calls_mutable() {
    assert_tc_err(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() {
            immutable Counter c = new Counter();
            c.increment();
            return 0;
        }
    "#, "immutable");
}

#[test]
fn tc_err_non_mutable_method_calls_mutable_on_this() {
    assert_tc_err(r#"
        class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int getAndIncrement() {
                this.increment();
                return value;
            }
        }
        int main() { return 0; }
    "#, "non-mutable");
}

#[test]
fn tc_err_readonly_on_list_add() {
    // List.add() est mutable → interdit sur readonly
    assert_tc_err(r#"
        int main() {
            List<int> lst = new ArrayList<int>();
            readonly List<int> rlst = lst;
            rlst.add(1);
            return 0;
        }
    "#, "readonly");
}

#[test]
fn tc_err_immutable_on_list_add() {
    assert_tc_err(r#"
        int main() {
            immutable List<int> lst = new ArrayList<int>();
            lst.add(1);
            return 0;
        }
    "#, "immutable");
}
