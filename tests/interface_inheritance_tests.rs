//! Tests de l'héritage d'interface — `interface Sub extends A, B { ... }`.
//! Une classe implémentant Sub doit fournir les méthodes de Sub et de tous
//! ses parents (transitif) ; une sous-interface est sous-type de ses parents ;
//! la résolution de méthode remonte la chaîne des parents.

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

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e))
}

// ── Parsing ─────────────────────────────────────────────────────────────────

#[test]
fn parse_single_extends() {
    parses_ok(
        r#"
        interface A { void a(); }
        interface B extends A { void b(); }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_multiple_extends() {
    parses_ok(
        r#"
        interface A { void a(); }
        interface B { void b(); }
        interface C extends A, B { void c(); }
        int main() { return 0; }
    "#,
    );
}

// ── Typecheck ─────────────────────────────────────────────────────────────────

#[test]
fn tc_class_must_implement_inherited_methods() {
    assert_tc_ok(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name()  { return "Rex"; }
            string owner() { return "Alice"; }
        }
        int main() { Dog d = new Dog(); return 0; }
    "#,
    );
}

#[test]
fn tc_err_missing_inherited_method() {
    // Dog implémente Pet mais oublie name() (héritée d'Animal)
    assert_tc_err(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string owner() { return "Alice"; }
        }
        int main() { return 0; }
    "#,
        "n'implémente pas",
    );
}

#[test]
fn tc_err_missing_own_method() {
    assert_tc_err(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name() { return "Rex"; }
        }
        int main() { return 0; }
    "#,
        "n'implémente pas",
    );
}

#[test]
fn tc_multilevel_inheritance() {
    // A <- B <- C : une classe implémentant C doit tout fournir
    assert_tc_ok(
        r#"
        interface A { void a(); }
        interface B extends A { void b(); }
        interface C extends B { void c(); }
        class Impl implements C {
            void a() {}
            void b() {}
            void c() {}
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_diamond_inheritance() {
    // Diamant : D étend B et C qui étendent toutes deux A
    assert_tc_ok(
        r#"
        interface A { void a(); }
        interface B extends A { void b(); }
        interface C extends A { void c(); }
        interface D extends B, C { void d(); }
        class Impl implements D {
            void a() {}
            void b() {}
            void c() {}
            void d() {}
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn tc_err_extends_unknown() {
    assert_tc_err(
        r#"
        interface B extends Fantome { void b(); }
        int main() { return 0; }
    "#,
        "extends 'Fantome' inconnu",
    );
}

#[test]
fn tc_err_extends_cycle() {
    assert_tc_err(
        r#"
        interface A extends B { void a(); }
        interface B extends A { void b(); }
        int main() { return 0; }
    "#,
        "Cycle d'héritage d'interface",
    );
}

#[test]
fn tc_subinterface_is_subtype_of_parent() {
    // Une variable typée par le parent accepte une valeur typée par l'enfant
    assert_tc_ok(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name()  { return "Rex"; }
            string owner() { return "Alice"; }
        }
        int main() {
            Pet p = new Dog();
            Animal a = p;          // Pet est sous-type d'Animal
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_call_inherited_method_on_interface_type() {
    // Appel d'une méthode héritée via une variable typée par la sous-interface
    assert_tc_ok(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name()  { return "Rex"; }
            string owner() { return "Alice"; }
        }
        string describe(Pet p) {
            return p.name();       // name() héritée d'Animal, appelée sur Pet
        }
        int main() {
            Pet p = new Dog();
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_class_implementing_subinterface_compatible_with_parent_param() {
    assert_tc_ok(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name()  { return "Rex"; }
            string owner() { return "Alice"; }
        }
        string greet(Animal a) { return a.name(); }
        int main() {
            Dog d = new Dog();
            string s = greet(d);   // Dog -> Pet -> Animal
            return 0;
        }
    "#,
    );
}

// ── Exécution ─────────────────────────────────────────────────────────────────

#[test]
fn run_inherited_method_dispatch() {
    let (ret, lines) = run_output(
        r#"
        interface Animal { string name(); }
        interface Pet extends Animal { string owner(); }
        class Dog implements Pet {
            string name()  { return "Rex"; }
            string owner() { return "Alice"; }
        }
        string describe(Pet p) {
            return p.name() + " (de " + p.owner() + ")";
        }
        int main() {
            Pet p = new Dog();
            print(describe(p));
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["Rex (de Alice)"]);
}

// ── Généricité de l'héritage ────────────────────────────────────────────────
//  Les arguments de type passés à un parent générique (`extends Base<int>`,
//  `implements Box<int>`) sont conservés et substitués le long de la chaîne
//  d'héritage, y compris quand les paramètres de type sont renommés.

#[test]
fn parse_generic_extends_with_args() {
    parses_ok(
        r#"
        interface Container<T> { T get(); }
        interface Box<E> extends Container<E> { void put(E x); }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn run_iface_inherited_generic_method_renamed_param() {
    // Box<E> extends Container<E> : le retour T de Container devient int via E↦int
    let (ret, lines) = run_output(
        r#"
        interface Container<T> { T get(); }
        interface Box<E> extends Container<E> { void put(E x); }
        class IntBox implements Box<int> {
            int value;
            IntBox(int v) { this.value = v; }
            int get() { return this.value; }
            void put(int x) {}
        }
        int main() {
            Box<int> bx = new IntBox(7);
            int v = bx.get();        // get() héritée de Container, retour int
            print(v);
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["7"]);
}

#[test]
fn run_iface_extends_concrete_arg() {
    // IntSource extends Source<int> : arg concret indépendant des params enfant
    let (ret, lines) = run_output(
        r#"
        interface Source<T> { T produce(); }
        interface IntSource extends Source<int> {}
        class Fixed implements IntSource {
            int produce() { return 42; }
        }
        int main() {
            IntSource s = new Fixed();
            int p = s.produce();
            print(p);
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["42"]);
}

#[test]
fn run_class_extends_generic_parent() {
    // IntCell extends Base<int> : méthode ET champ hérités substitués T↦int
    let (ret, lines) = run_output(
        r#"
        mut class Base<T> {
            protected T val;
            T getVal() { return val; }
            mutable void setVal(T v) { val = v; }
        }
        mut class IntCell extends Base<int> {
            int doubled() {
                int z = val;               // champ hérité protected, T=int
                return this.getVal() * 2;  // méthode héritée, retour int
            }
        }
        int main() {
            IntCell c = new IntCell();
            c.setVal(21);
            print(c.getVal());
            print(c.doubled());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["21", "42"]);
}

#[test]
fn tc_err_generic_parent_wrong_arg_type() {
    // get() hérité retourne int (via Box<int>) → incompatible avec une string
    assert_tc_err(
        r#"
        interface Container<T> { T get(); }
        interface Box<E> extends Container<E> { void put(E x); }
        class IntBox implements Box<int> {
            int value;
            IntBox(int v) { this.value = v; }
            int get() { return this.value; }
            void put(int x) {}
        }
        int main() {
            Box<int> bx = new IntBox(7);
            string s = bx.get();    // get() retourne int, pas string
            return 0;
        }
    "#,
        "incompatible",
    );
}

#[test]
fn tc_err_generic_extends_arity() {
    assert_tc_err(
        r#"
        interface Pair<A, B> { A first(); }
        interface Bad extends Pair<int> { void x(); }
        int main() { return 0; }
    "#,
        "argument(s) de type attendu(s)",
    );
}

#[test]
fn run_subinterface_with_di() {
    // Héritage d'interface combiné à l'injection : on injecte via le parent
    let (ret, lines) = run_output(
        r#"
        interface Greeter { string greet(); }
        interface FancyGreeter extends Greeter { string wave(); }
        service class Hello implements FancyGreeter {
            string greet() { return "bonjour"; }
            string wave()  { return "(salut)"; }
        }
        int main() {
            FancyGreeter g = inject FancyGreeter;
            print(g.greet(), g.wave());
            return 0;
        }
    "#,
    );
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["bonjour (salut)"]);
}
