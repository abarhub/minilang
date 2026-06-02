//! Tests du système de records — minilang.
//! record Name(type field, ...) — agrégat immuable avec getters générés,
//! equals, toString, hashCode, copy. Hérite de Record.

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
//  Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_simple_record() {
    parses_ok(r#"
        record Point(int x, int y) {}
        int main() { return 0; }
    "#);
}

#[test]
fn parse_record_with_method() {
    parses_ok(r#"
        record Point(int x, int y) {
            int sumCoords() { return x + y; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn parse_generic_record() {
    parses_ok(r#"
        record Wrapper<T>(T value) {}
        int main() { return 0; }
    "#);
}

#[test]
fn parse_record_implements() {
    parses_ok(r#"
        mut interface Printable { string toString(); }
        record Point(int x, int y) implements Printable {
            string toString() { return "Point"; }
        }
        int main() { return 0; }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — construction et getters
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_record_construction_and_getter() {
    // new Record(args) + appel du getter généré
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1, 2);
            int vx = p.getX();
            int vy = p.getY();
            return vx + vy;
        }
    "#);
}

#[test]
fn tc_record_wrong_arg_count() {
    assert_tc_err(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1);
            return 0;
        }
    "#, "arg(s)");
}

#[test]
fn tc_record_wrong_arg_type() {
    assert_tc_err(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1, "hello");
            return 0;
        }
    "#, "incompatible");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — champs privés
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_record_field_private_from_outside() {
    // Les champs d'un record sont privés comme ceux d'une classe
    assert_tc_err(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1, 2);
            return p.x;
        }
    "#, "privé");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — méthodes générées
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_record_equals() {
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point a = new Point(1, 2);
            Point b = new Point(1, 2);
            bool eq = a.equals(b);
            return 0;
        }
    "#);
}

#[test]
fn tc_record_tostring() {
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(3, 4);
            string s = p.toString();
            return 0;
        }
    "#);
}

#[test]
fn tc_record_hashcode() {
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1, 2);
            int h = p.hashCode();
            return h;
        }
    "#);
}

#[test]
fn tc_record_copy() {
    // copy prend Option<T> pour chaque champ
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p  = new Point(1, 2);
            Point p2 = p.copy(Option<int>::None, Option<int>::Some(10));
            return p2.getY();
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — méthode custom
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_record_custom_method() {
    assert_tc_ok(r#"
        record Point(int x, int y) {
            int sumCoords() { return x + y; }
        }
        int main() {
            Point p = new Point(3, 4);
            return p.sumCoords();
        }
    "#);
}

#[test]
fn tc_record_mutable_method_forbidden() {
    // Les méthodes mutable sont interdites dans un record
    assert_tc_err(r#"
        record Counter(int value) {
            mutable void increment() { value = value + 1; }
        }
        int main() { return 0; }
    "#, "mutable");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — héritage de Record
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_record_assignable_to_record_type() {
    // Un record hérite de Record — assignable à une variable de type Record
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p  = new Point(1, 2);
            Record r = p;
            return 0;
        }
    "#);
}

#[test]
fn tc_record_inherits_record_methods() {
    // Les méthodes de Record (equals, toString, hashCode) sont disponibles
    assert_tc_ok(r#"
        record Point(int x, int y) {}
        int main() {
            Point p = new Point(1, 2);
            bool eq = p.equals(p);
            string s = p.toString();
            int h    = p.hashCode();
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — records génériques (Pair)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_pair_record_construction() {
    // Pair est maintenant un record — new Pair(a, b) remplace Pair::Of(a, b)
    assert_tc_ok(r#"
        int main() {
            Pair<int, string> p = new Pair<int, string>(42, "hello");
            int   f = p.getFirst();
            string s = p.getSecond();
            return f;
        }
    "#);
}

#[test]
fn tc_generic_record() {
    assert_tc_ok(r#"
        record Wrapper<T>(T value) {}
        int main() {
            Wrapper<int> w = new Wrapper<int>(42);
            int v = w.getValue();
            return v;
        }
    "#);
}
