//! Tests de l'égalité Object pour les types de la bibliothèque standard.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
    }
}

#[test]
fn test_integer_equals_same() {
    assert_eq!(run_ok("int main() { int a = 42; bool r = a.equals(42); if (r) { return 1; } return 0; }"), 1);
}

#[test]
fn test_integer_equals_different() {
    assert_eq!(run_ok("int main() { int a = 42; bool r = a.equals(99); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_integer_equals_wrong_type_returns_false() {
    assert_eq!(run_ok(r#"int main() { int a = 42; bool r = a.equals("hello"); if (r) { return 0; } return 1; }"#), 1);
}

#[test]
fn test_float_equals_same() {
    assert_eq!(run_ok("int main() { float a = 3.14; bool r = a.equals(3.14); if (r) { return 1; } return 0; }"), 1);
}

#[test]
fn test_float_equals_different() {
    assert_eq!(run_ok("int main() { float a = 3.14; bool r = a.equals(2.71); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_float_equals_wrong_type_returns_false() {
    assert_eq!(run_ok("int main() { float a = 1.0; bool r = a.equals(1); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_boolean_equals_true_true() {
    assert_eq!(run_ok("int main() { bool a = true; bool r = a.equals(true); if (r) { return 1; } return 0; }"), 1);
}

#[test]
fn test_boolean_equals_true_false() {
    assert_eq!(run_ok("int main() { bool a = true; bool r = a.equals(false); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_boolean_equals_wrong_type_returns_false() {
    assert_eq!(run_ok("int main() { bool a = true; bool r = a.equals(1); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_string_equals_same() {
    assert_eq!(run_ok(r#"int main() { string s = "hello"; bool r = s.equals("hello"); if (r) { return 1; } return 0; }"#), 1);
}

#[test]
fn test_string_equals_different() {
    assert_eq!(run_ok(r#"int main() { string s = "hello"; bool r = s.equals("world"); if (r) { return 0; } return 1; }"#), 1);
}

#[test]
fn test_string_equals_wrong_type_returns_false() {
    assert_eq!(run_ok("int main() { string s = \"hi\"; bool r = s.equals(42); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_char_equals_same() {
    assert_eq!(run_ok("int main() { char c = 'a'; bool r = c.equals('a'); if (r) { return 1; } return 0; }"), 1);
}

#[test]
fn test_char_equals_different() {
    assert_eq!(run_ok("int main() { char c = 'a'; bool r = c.equals('b'); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_char_equals_wrong_type_returns_false() {
    assert_eq!(run_ok("int main() { char c = 'a'; bool r = c.equals(97); if (r) { return 0; } return 1; }"), 1);
}

#[test]
fn test_object_equals_same_instance() {
    assert_eq!(run_ok(r#"
        class Point { int x; Point(int a) { x = a; } }
        int main() {
            Point p = new Point(1);
            if (p.equals(p)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_object_equals_different_instances() {
    assert_eq!(run_ok(r#"
        class Point { int x; Point(int a) { x = a; } }
        int main() {
            Point a = new Point(1);
            Point b = new Point(1);
            if (a.equals(b)) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn test_object_variable_accepts_subclass() {
    assert_tc_ok(r#"
        class Node { int val; Node(int v) { val = v; } }
        int main() {
            Node n = new Node(5);
            Object o = n;
            return 0;
        }
    "#);
}

#[test]
fn test_equals_override_value_semantics() {
    assert_eq!(run_ok(r#"
        class IntBox {
            int val;
            IntBox(int v) { val = v; }
            bool equals(Object other) {
                return val == 42;
            }
        }
        int main() {
            IntBox a = new IntBox(42);
            IntBox b = new IntBox(99);
            if (a.equals(b)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_tc_object_param_accepts_int() {
    assert_tc_ok(r#"
        int check(Object o) { return 1; }
        int main() {
            int n = 42;
            return 0;
        }
    "#);
}
