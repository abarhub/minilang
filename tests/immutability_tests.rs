//! Tests des phases 1 et 2 du système d'immutabilité — minilang.
//! Phase 1 : readonly / immutable sur les variables, mutable sur les méthodes.
//! Phase 2 : mot-clé `mut` sur les classes/interfaces — audit du système.

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
//  Phase 1 — Parsing
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
        mut class Counter {
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
        mut class Box {
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
        mut class Point {
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
//  Phase 2 — Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_mut_class() {
    parses_ok(r#"
        mut class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int get() { return value; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_mut_interface() {
    parses_ok(r#"
        mut interface Resettable {
            mutable void reset();
            int getValue();
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_non_mut_class() {
    // Une classe sans mut parse correctement (usage limité aux var mutables)
    parses_ok(r#"
        class Helper {
            int compute(int x) { return x * 2; }
        }
        int main() {
            Helper h = new Helper();
            return h.compute(5);
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Phase 1 — Typecheck valide
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_mutable_can_call_mutable_method() {
    assert_tc_ok(r#"
        mut class Counter {
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
        mut class Counter {
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
        mut class Point {
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
    assert_tc_ok(r#"
        mut class MyPair {
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
        mut class Counter {
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
//  Phase 2 — Typecheck valide
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_non_mut_class_usable_as_mutable() {
    // Une classe sans `mut` peut être utilisée comme variable mutable (défaut)
    assert_tc_ok(r#"
        class Helper {
            int compute(int x) { return x * 2; }
        }
        int main() {
            Helper h = new Helper();
            return h.compute(3);
        }
    "#);
}

#[test]
fn tc_mut_class_readonly_ok() {
    assert_tc_ok(r#"
        mut class Counter {
            int value;
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
fn tc_mut_class_immutable_ok() {
    assert_tc_ok(r#"
        mut class Point {
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
fn tc_stdlib_list_readonly_ok() {
    // List est mut → on peut déclarer readonly List
    assert_tc_ok(r#"
        int main() {
            List<int> lst = new ArrayList<int>();
            readonly List<int> rlst = lst;
            return rlst.size();
        }
    "#);
}

#[test]
fn tc_enum_always_mut_readonly_ok() {
    // Les enums sont mut implicitement
    assert_tc_ok(r#"
        int main() {
            Option<int> opt = Option<int>::Some(42);
            readonly Option<int> ropt = opt;
            return 0;
        }
    "#);
}

#[test]
fn tc_enum_always_mut_immutable_ok() {
    assert_tc_ok(r#"
        int main() {
            immutable Option<int> opt = Option<int>::Some(42);
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Phase 1 — Typecheck erreurs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_readonly_calls_mutable() {
    assert_tc_err(r#"
        mut class Counter {
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
        mut class Counter {
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
        mut class Counter {
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

// ─────────────────────────────────────────────────────────────────────────────
//  Phase 2 — Typecheck erreurs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_err_non_mut_class_readonly() {
    // Classe sans `mut` → interdit avec readonly
    assert_tc_err(r#"
        class Helper {
            int compute(int x) { return x * 2; }
        }
        int main() {
            Helper h = new Helper();
            readonly Helper rh = h;
            return 0;
        }
    "#, "mut");
}

#[test]
fn tc_err_non_mut_class_immutable() {
    // Classe sans `mut` → interdit avec immutable
    assert_tc_err(r#"
        class Helper {
            int compute(int x) { return x * 2; }
        }
        int main() {
            immutable Helper h = new Helper();
            return 0;
        }
    "#, "mut");
}
