//! Tests du typechecker : vérifie que les erreurs de type sont bien détectées
//! et que les programmes corrects passent sans erreur.

use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn assert_ok(src: &str) {
    if let Err(errs) = check_source(src) {
        panic!("Typecheck inattendu échoué :\n{}\n---\n{}", src, errs.join("\n"));
    }
}

fn assert_error(src: &str, expected_fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!(
            "Typecheck aurait dû échouer (attendu : '{}') :\n{}",
            expected_fragment, src
        ),
        Err(errs) => {
            let all = errs.join("\n");
            assert!(
                all.contains(expected_fragment),
                "Message d'erreur attendu : '{}'\nObtenu : '{}'",
                expected_fragment,
                all
            );
        }
    }
}

// ── Programmes valides ────────────────────────────────────────────────────────

#[test]
fn test_valid_primitives() {
    assert_ok(r#"
        int main() {
            int    i = 42;
            float  f = 3.14;
            double d = 2.718;
            bool   b = true;
            string s = "ok";
            return 0;
        }
    "#);
}

#[test]
fn test_valid_arithmetic() {
    assert_ok(r#"
        int main() {
            int   a = 2 + 3 * 4 - 1;
            float b = 1.5 + 2.5;
            float c = 2.0 ** 8.0;
            int   d = 10 % 3;
            return 0;
        }
    "#);
}

#[test]
fn test_valid_class_with_constructor() {
    assert_ok(r#"
        class Point {
            int x;
            int y;
            Point(int a, int b) { x = a; y = b; }
            int getX() { return x; }
        }
        int main() {
            Point p = new Point(1, 2);
            int v = p.getX();
            return 0;
        }
    "#);
}

#[test]
fn test_valid_inheritance_subtype() {
    assert_ok(r#"
        class Animal {
            string name;
            Animal(string n) { name = n; }
            string getName() { return name; }
        }
        class Dog extends Animal {
            Dog(string n) { name = n; }
        }
        int main() {
            Dog d = new Dog("Rex");
            string s = d.getName();
            return 0;
        }
    "#);
}

#[test]
fn test_valid_interface_implementation() {
    assert_ok(r#"
        interface Describable {
            void describe();
        }
        class Cat implements Describable {
            string name;
            Cat(string n) { name = n; }
            void describe() { print(name); }
        }
        int main() {
            Cat c = new Cat("Mimi");
            c.describe();
            return 0;
        }
    "#);
}

#[test]
fn test_valid_if_bool_condition() {
    assert_ok(r#"
        int main() {
            bool b = 3 > 2;
            if (b) { print("ok"); }
            return 0;
        }
    "#);
}

#[test]
fn test_valid_for_loop() {
    assert_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 0; i < 10; i = i + 1) {
                sum = sum + i;
            }
            return sum;
        }
    "#);
}

#[test]
fn test_valid_while_loop() {
    assert_ok(r#"
        int main() {
            int i = 0;
            while (i < 5) { i = i + 1; }
            return 0;
        }
    "#);
}

#[test]
fn test_valid_generic_class() {
    assert_ok(r#"
        class Box<T> {
            T value;
            Box(T v) { value = v; }
            T get()  { return value; }
        }
        int main() {
            Box<int> b = new Box<int>(99);
            return 0;
        }
    "#);
}

#[test]
fn test_valid_string_concat() {
    assert_ok(r#"
        int main() {
            string s = "Hello" + ", " + "world!";
            return 0;
        }
    "#);
}

#[test]
fn test_valid_method_chain_3_levels() {
    assert_ok(r#"
        class A {
            int val;
            A(int v) { val = v; }
            int get() { return val; }
        }
        class B extends A {
            B(int v) { val = v; }
        }
        class C extends B {
            C(int v) { val = v; }
        }
        int main() {
            C c = new C(7);
            int x = c.get();
            return 0;
        }
    "#);
}

// ── Erreurs de type ───────────────────────────────────────────────────────────

#[test]
fn test_error_type_mismatch_int_string() {
    assert_error(
        r#"int main() { string s = 42; return 0; }"#,
        "incompatible",
    );
}

#[test]
fn test_error_type_mismatch_bool_int() {
    assert_error(
        r#"int main() { bool b = 1; return 0; }"#,
        "incompatible",
    );
}

#[test]
fn test_error_return_wrong_type() {
    assert_error(
        r#"int main() { return "oops"; }"#,
        "return",
    );
}

#[test]
fn test_error_if_condition_not_bool() {
    assert_error(
        r#"int main() { if (42) { print("x"); } return 0; }"#,
        "bool",
    );
}

#[test]
fn test_error_while_condition_not_bool() {
    assert_error(
        r#"int main() { int i = 0; while (i) { i = i + 1; } return 0; }"#,
        "bool",
    );
}

#[test]
fn test_error_for_condition_not_bool() {
    assert_error(
        r#"int main() { for (int i = 0; i; i = i + 1) {} return 0; }"#,
        "bool",
    );
}

#[test]
fn test_error_unknown_method() {
    assert_error(
        r#"
        class Foo { Foo() {} }
        int main() {
            Foo f = new Foo();
            f.nonExistent();
            return 0;
        }"#,
        "Méthode",
    );
}

#[test]
fn test_error_wrong_arg_count() {
    assert_error(
        r#"
        class Foo {
            Foo() {}
            void bar(int x) {}
        }
        int main() {
            Foo f = new Foo();
            f.bar(1, 2);
            return 0;
        }"#,
        "arg",
    );
}

#[test]
fn test_error_wrong_arg_type() {
    assert_error(
        r#"
        class Foo {
            Foo() {}
            void bar(int x) {}
        }
        int main() {
            Foo f = new Foo();
            f.bar("pas un int");
            return 0;
        }"#,
        "incompatible",
    );
}

#[test]
fn test_error_missing_interface_method() {
    assert_error(
        r#"
        interface Greetable {
            void greet();
            int score();
        }
        class Foo implements Greetable {
            void greet() { print("hi"); }
            // score() manquant
        }
        int main() { return 0; }"#,
        "n'implémente pas",
    );
}

#[test]
fn test_error_unknown_parent_class() {
    assert_error(
        r#"
        class Child extends NonExistent {}
        int main() { return 0; }"#,
        "inconnu",
    );
}

#[test]
fn test_error_constructor_wrong_arg_count() {
    assert_error(
        r#"
        class Point {
            int x;
            int y;
            Point(int a, int b) { x = a; y = b; }
        }
        int main() {
            Point p = new Point(1);
            return 0;
        }"#,
        "constructeur",
    );
}

#[test]
fn test_error_constructor_wrong_arg_type() {
    assert_error(
        r#"
        class Point {
            int x;
            Point(int a) { x = a; }
        }
        int main() {
            Point p = new Point("nope");
            return 0;
        }"#,
        "incompatible",
    );
}

#[test]
fn test_error_arithmetic_on_bool() {
    assert_error(
        r#"int main() { bool b = true + false; return 0; }"#,
        "non applicable",
    );
}

#[test]
fn test_error_logic_on_int() {
    assert_error(
        r#"int main() { bool b = 1 && 2; return 0; }"#,
        "bool",
    );
}

#[test]
fn test_error_inheritance_cycle() {
    assert_error(
        r#"
        class A extends B {}
        class B extends A {}
        int main() { return 0; }"#,
        "Cycle",
    );
}

// ── Promotions numériques (doivent être acceptées) ────────────────────────────

#[test]
fn test_int_to_float_promotion_ok() {
    assert_ok(r#"
        int main() {
            float f = 42;
            return 0;
        }
    "#);
}

#[test]
fn test_float_to_double_promotion_ok() {
    assert_ok(r#"
        int main() {
            double d = 3.14;
            return 0;
        }
    "#);
}
