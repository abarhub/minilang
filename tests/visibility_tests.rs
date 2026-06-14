//! Tests du système de visibilité — minilang.
//! Champs : private (défaut) / protected. Méthodes : public (défaut) / protected
//! / private. Inclut les appels `super(...)` au constructeur parent.

use chumsky::Parser;
use mini_parser::interpreter::run_source_with_output;
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

// ─────────────────────────────────────────────────────────────────────────────
//  Champs protected
// ─────────────────────────────────────────────────────────────────────────────

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e))
}

#[test]
fn parse_protected_field() {
    parses_ok(
        r#"
        mut class A { protected int x; }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_protected_field_accessible_in_subclass() {
    // Champ protected hérité : lisible via this et par nom nu dans la sous-classe
    assert_tc_ok(
        r#"
        mut class Base { protected int val; }
        mut class Sub extends Base {
            int viaThis() { return this.val; }
            int viaBare() { return val; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_err_private_field_not_in_subclass_via_this() {
    // Champ privé hérité : inaccessible même via this depuis une sous-classe
    assert_tc_err(
        r#"
        mut class Base { int secret; }
        mut class Sub extends Base {
            int leak() { return this.secret; }
        }
        int main() { return 0; }
    "#,
        "privé",
    );
}

#[test]
fn tc_err_private_field_not_in_subclass_bare() {
    assert_tc_err(
        r#"
        mut class Base { int secret; }
        mut class Sub extends Base {
            int leak() { return secret; }
        }
        int main() { return 0; }
    "#,
        "privé",
    );
}

#[test]
fn tc_err_protected_field_from_outside() {
    // protected ne donne pas l'accès externe
    assert_tc_err(
        r#"
        mut class A { protected int x; }
        int main() {
            A a = new A();
            return a.x;
        }
    "#,
        "protected",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  super(...) — appel du constructeur parent
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn run_super_initializes_parent_fields() {
    let (ret, lines) = run_output(
        r#"
        mut class Animal {
            protected string name;
            Animal(string n) { this.name = n; }
            string describe() { return name; }
        }
        mut class Dog extends Animal {
            string breed;
            Dog(string n, string b) { super(n); this.breed = b; }
            string full() { return name + " (" + breed + ")"; }
        }
        int main() {
            Dog d = new Dog("Rex", "Husky");
            print(d.describe());
            print(d.full());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["Rex", "Rex (Husky)"]);
}

#[test]
fn run_super_nested_three_levels() {
    let (ret, lines) = run_output(
        r#"
        mut class A { protected int a; A(int x) { this.a = x; } }
        mut class B extends A { protected int b; B(int x, int y) { super(x); this.b = y; } }
        mut class C extends B {
            int c;
            C(int x, int y, int z) { super(x, y); this.c = z; }
            int sum() { return a + b + c; }
        }
        int main() {
            C obj = new C(1, 2, 3);
            print(obj.sum());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["6"]);
}

#[test]
fn tc_err_super_missing_when_parent_has_ctor() {
    assert_tc_err(
        r#"
        mut class A { int x; A(int v) { this.x = v; } }
        mut class B extends A { int y; B(int v) { this.y = v; } }
        int main() { return 0; }
    "#,
        "super(...)",
    );
}

#[test]
fn tc_err_super_not_first_statement() {
    assert_tc_err(
        r#"
        mut class A { int x; A(int v) { this.x = v; } }
        mut class B extends A {
            int y;
            B(int v) { this.y = v; super(v); }
        }
        int main() { return 0; }
    "#,
        "première instruction",
    );
}

#[test]
fn tc_err_super_wrong_arity() {
    assert_tc_err(
        r#"
        mut class A { int x; A(int v) { this.x = v; } }
        mut class B extends A { B() { super(); } }
        int main() { return 0; }
    "#,
        "aucun constructeur",
    );
}

#[test]
fn tc_err_super_outside_constructor() {
    assert_tc_err(
        r#"
        mut class A { int x; A(int v) { this.x = v; } }
        mut class B extends A {
            B(int v) { super(v); }
            void oops() { super(1); }
        }
        int main() { return 0; }
    "#,
        "constructeur",
    );
}
