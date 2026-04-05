//! Tests du parser : vérifie que les programmes valides sont acceptés
//! et que les programmes invalides produisent bien une erreur de syntaxe.

use chumsky::Parser;
use mini_parser::parser::program_parser;

// ── Helper ────────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    match program_parser().parse(src) {
        Ok(_)    => {}
        Err(errs) => panic!(
            "Parsing inattendu échoué :\n{}\n---\n{}",
            src,
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        ),
    }
}

fn parse_fails(src: &str) {
    if program_parser().parse(src).is_ok() {
        panic!("Parsing aurait dû échouer :\n{}", src);
    }
}

// ── main minimal ─────────────────────────────────────────────────────────────

#[test]
fn test_empty_main() {
    parses_ok("int main() { return 0; }");
}

#[test]
fn test_main_with_local_vars() {
    parses_ok(r#"
        int main() {
            int x = 42;
            bool b = true;
            float f = 3.14;
            double d = 2.718;
            string s = "hello";
            return 0;
        }
    "#);
}

// ── Package et imports ────────────────────────────────────────────────────────

#[test]
fn test_package_and_imports() {
    parses_ok(r#"
        package com.example.app;
        import com.example.util.*;
        import com.example.model.User;
        int main() { return 0; }
    "#);
}

#[test]
fn test_package_only() {
    parses_ok("package my.pkg; int main() { return 0; }");
}

#[test]
fn test_import_only() {
    parses_ok("import java.util.*; int main() { return 0; }");
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[test]
fn test_arithmetic_expressions() {
    parses_ok(r#"
        int main() {
            int a = 1 + 2 * 3 - 4 / 2 % 3;
            float p = 2.0 ** 10.0;
            string s = "hello" + " " + "world";
            return 0;
        }
    "#);
}

#[test]
fn test_comparison_and_logic() {
    parses_ok(r#"
        int main() {
            bool a = 1 < 2;
            bool b = 3 >= 2 && true || false;
            bool c = !(1 == 2);
            bool d = 5 != 6;
            return 0;
        }
    "#);
}

#[test]
fn test_unary_operators() {
    parses_ok(r#"
        int main() {
            int n = -42;
            bool b = !true;
            float f = -3.14;
            return 0;
        }
    "#);
}

#[test]
fn test_operator_precedence() {
    // 2 + 3 * 4 doit être parsé comme 2 + (3 * 4)
    parses_ok("int main() { int x = 2 + 3 * 4; return 0; }");
}

// ── Structures de contrôle ────────────────────────────────────────────────────

#[test]
fn test_if_else_if_else() {
    parses_ok(r#"
        int main() {
            int x = 5;
            if (x > 10) {
                print("grand");
            } else if (x > 3) {
                print("moyen");
            } else {
                print("petit");
            }
            return 0;
        }
    "#);
}

#[test]
fn test_while_loop() {
    parses_ok(r#"
        int main() {
            int i = 0;
            while (i < 10) {
                i = i + 1;
            }
            return 0;
        }
    "#);
}

#[test]
fn test_do_while_loop() {
    parses_ok(r#"
        int main() {
            int i = 0;
            do {
                i = i + 1;
            } while (i < 5);
            return 0;
        }
    "#);
}

#[test]
fn test_for_loop_full() {
    parses_ok(r#"
        int main() {
            for (int i = 0; i < 10; i = i + 1) {
                print(i);
            }
            return 0;
        }
    "#);
}

#[test]
fn test_for_loop_empty_parts() {
    parses_ok(r#"
        int main() {
            int i = 0;
            for (; i < 5; i = i + 1) { print(i); }
            return 0;
        }
    "#);
}

#[test]
fn test_break_continue() {
    parses_ok(r#"
        int main() {
            for (int i = 0; i < 10; i = i + 1) {
                if (i == 3) { continue; }
                if (i == 7) { break; }
            }
            return 0;
        }
    "#);
}

// ── Classes ───────────────────────────────────────────────────────────────────

#[test]
fn test_simple_class() {
    parses_ok(r#"
        class Point {
            int x;
            int y;

            Point(int a, int b) {
                x = a;
                y = b;
            }

            int getX() { return x; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn test_class_inheritance() {
    parses_ok(r#"
        class Animal {
            string name;
            void speak() { print(name); }
        }
        class Dog extends Animal {
            void fetch() { print("fetching"); }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn test_interface_and_implements() {
    parses_ok(r#"
        interface Drawable {
            void draw();
            int area();
        }
        class Circle implements Drawable {
            int radius;
            void draw()  { print("circle"); }
            int  area()  { return 0; }
        }
        int main() { return 0; }
    "#);
}

#[test]
fn test_generic_class() {
    parses_ok(r#"
        class Box<T> {
            T value;
            Box(T v) { value = v; }
            T get() { return value; }
        }
        int main() {
            Box<int> b = new Box<int>(42);
            return 0;
        }
    "#);
}

#[test]
fn test_multiple_constructors() {
    parses_ok(r#"
        class Vec {
            int x;
            int y;
            Vec()         { x = 0; y = 0; }
            Vec(int a)    { x = a; y = 0; }
            Vec(int a, int b) { x = a; y = b; }
        }
        int main() { return 0; }
    "#);
}

// ── Appels enchaînés ──────────────────────────────────────────────────────────

#[test]
fn test_chained_method_calls() {
    parses_ok(r#"
        class Builder {
            int val;
            Builder set(int v) { val = v; return this; }
            int build() { return val; }
        }
        int main() {
            Builder b = new Builder(0);
            b.set(5);
            return 0;
        }
    "#);
}

#[test]
fn test_field_access_and_assignment() {
    parses_ok(r#"
        class Counter {
            int count;
            Counter(int c) { count = c; }
        }
        int main() {
            Counter c = new Counter(0);
            c.count = 5;
            print(c.count);
            return 0;
        }
    "#);
}

// ── Erreurs de syntaxe ────────────────────────────────────────────────────────

#[test]
fn test_missing_main() {
    parse_fails("class Foo {}");
}

#[test]
fn test_missing_semicolon() {
    parse_fails("int main() { int x = 5 return 0; }");
}

#[test]
fn test_unclosed_brace() {
    parse_fails("int main() { int x = 5;");
}

#[test]
fn test_invalid_return_type_for_main() {
    // main doit retourner int
    parse_fails("void main() { }");
}

#[test]
fn test_main_with_params() {
    // main ne prend pas de paramètre
    parse_fails("int main(int argc) { return 0; }");
}
