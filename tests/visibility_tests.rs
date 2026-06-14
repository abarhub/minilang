//! Tests du système de visibilité — minilang.
//! Champs toujours privés, méthodes : public (défaut) / protected / private.

use chumsky::Parser;
use mini_parser::parser::program_parser;
use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    match program_parser().parse(full.as_str()) {
        Ok(_) => {}
        Err(e) => panic!(
            "Parse failed:\n{}\n---\n{}",
            src,
            e.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck should pass:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!(
            "Typecheck should have failed (expected '{}'):\n{}",
            fragment, src
        ),
        Err(e) => {
            let all = e.join("\n");
            assert!(
                all.contains(fragment),
                "Expected '{}' in:\n{}",
                fragment,
                all
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_private_method() {
    parses_ok(
        r#"
        mut class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int getValue() { return value; }
            private bool isValid() { return value >= 0; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_protected_method() {
    parses_ok(
        r#"
        mut class Animal {
            string name;
            string getName() { return name; }
            protected string buildLabel() { return name; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_private_mutable_method() {
    // private et mutable sont compatibles
    parses_ok(
        r#"
        mut class Counter {
            int value;
            private mutable void reset() { value = 0; }
            mutable void increment() { value = value + 1; }
            int getValue() { return value; }
        }
        int main() { return 0; }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — champs privés
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_field_access_via_this_ok() {
    // Accès à un champ via this dans une méthode → OK
    assert_tc_ok(
        r#"
        mut class Counter {
            int value;
            int getValue() { return value; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_err_field_access_from_outside() {
    // Accès direct à un champ depuis l'extérieur → ERREUR
    assert_tc_err(
        r#"
        mut class Counter {
            int value;
            int getValue() { return value; }
        }
        int main() {
            Counter c = new Counter();
            return c.value;
        }
    "#,
        "privé",
    );
}

#[test]
fn tc_err_field_assign_from_outside() {
    // Affectation directe d'un champ depuis l'extérieur → ERREUR
    assert_tc_err(
        r#"
        mut class Counter {
            int value;
            int getValue() { return value; }
        }
        int main() {
            Counter c = new Counter();
            c.value = 5;
            return 0;
        }
    "#,
        "privé",
    );
}

#[test]
fn tc_field_access_same_class_other_instance() {
    // Accès au champ d'une autre instance de la même classe → OK (privé-par-classe)
    assert_tc_ok(
        r#"
        mut class Counter {
            int value;
            bool equals(Counter other) { return this.value == other.value; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_public_method_callable_from_outside() {
    // Méthode publique accessible depuis n'importe où
    assert_tc_ok(
        r#"
        mut class Counter {
            int value;
            mutable void increment() { value = value + 1; }
            int getValue() { return value; }
        }
        int main() {
            Counter c = new Counter();
            c.increment();
            return c.getValue();
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — méthodes private
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_private_method_callable_from_same_class() {
    // Une méthode privée peut être appelée depuis une autre méthode de la même classe
    assert_tc_ok(
        r#"
        mut class Counter {
            int value;
            private bool isValid() { return value >= 0; }
            mutable void increment() {
                if (this.isValid()) { value = value + 1; }
            }
            int getValue() { return value; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_err_private_method_from_outside() {
    // Méthode privée inaccessible depuis l'extérieur
    assert_tc_err(
        r#"
        mut class Counter {
            int value;
            private bool isValid() { return value >= 0; }
            int getValue() { return value; }
        }
        int main() {
            Counter c = new Counter();
            c.isValid();
            return 0;
        }
    "#,
        "privée",
    );
}

#[test]
fn tc_err_private_method_from_subclass() {
    // Méthode privée inaccessible depuis une sous-classe
    assert_tc_err(
        r#"
        mut class Animal {
            string name;
            private bool validate() { return true; }
            string getName() { return name; }
        }
        mut class Dog extends Animal {
            void bark() { this.validate(); }
        }
        int main() { return 0; }
    "#,
        "privée",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — méthodes protected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_protected_method_callable_from_same_class() {
    assert_tc_ok(
        r#"
        mut class Animal {
            string name;
            protected string buildLabel() { return name; }
            string getLabel() { return this.buildLabel(); }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_protected_method_callable_from_subclass() {
    // Méthode protégée accessible depuis une sous-classe
    assert_tc_ok(
        r#"
        mut class Animal {
            string name;
            string getName() { return name; }
            protected string buildLabel() { return name; }
        }
        mut class Dog extends Animal {
            string describe() { return this.buildLabel(); }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_err_protected_method_from_outside() {
    // Méthode protégée inaccessible depuis l'extérieur
    assert_tc_err(
        r#"
        mut class Animal {
            string name;
            string getName() { return name; }
            protected string buildLabel() { return name; }
        }
        int main() {
            Animal a = new Animal();
            a.buildLabel();
            return 0;
        }
    "#,
        "protégée",
    );
}

#[test]
fn tc_err_protected_method_from_unrelated_class() {
    // Méthode protégée inaccessible depuis une classe sans lien de parenté
    assert_tc_err(
        r#"
        mut class Animal {
            string name;
            protected string buildLabel() { return name; }
        }
        mut class Car {
            void test() {
                Animal a = new Animal();
                a.buildLabel();
            }
        }
        int main() { return 0; }
    "#,
        "protégée",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck — getter/setter (convention Java)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_getter_setter_pattern() {
    // Pattern complet : champ privé + getter public + setter mutable public
    assert_tc_ok(
        r#"
        mut class Person {
            string name;
            int age;

            string getName() { return name; }
            int getAge()     { return age;  }

            mutable void setName(string n) { name = n; }
            mutable void setAge(int a)     { age  = a; }
        }
        int main() {
            Person p = new Person();
            p.setName("Alice");
            p.setAge(30);
            return p.getAge();
        }
    "#,
    );
}

#[test]
fn tc_private_helper_used_internally() {
    // Méthode privée utilisée comme helper interne
    assert_tc_ok(
        r#"
        mut class MathHelper {
            int value;
            private int doubleValue() { return value * 2; }
            int getDoubled() { return this.doubleValue(); }
        }
        int main() {
            MathHelper m = new MathHelper();
            return m.getDoubled();
        }
    "#,
    );
}

#[test]
fn tc_inheritance_protected_getter() {
    // Sous-classe utilise une méthode protégée du parent
    assert_tc_ok(
        r#"
        mut class Shape {
            int size;
            int getSize() { return size; }
            protected int computeArea() { return size * size; }
        }
        mut class Square extends Shape {
            int area() { return this.computeArea(); }
        }
        int main() {
            Square s = new Square();
            return s.area();
        }
    "#,
    );
}
