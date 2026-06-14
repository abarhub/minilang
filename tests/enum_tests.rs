//! Tests pour les enums et le pattern matching.

use chumsky::Parser;
use mini_parser::interpreter::run_source;
use mini_parser::parser::program_parser;
use mini_parser::typechecker::check_source;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    match program_parser().parse(src) {
        Ok(_) => {}
        Err(errs) => panic!(
            "Parse failed:\n{}\n---\n{}",
            src,
            errs.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(errs) = check_source(src) {
        panic!("Typecheck failed:\n{}\n---\n{}", src, errs.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!(
            "Typecheck should have failed (expected '{}'):\n{}",
            fragment, src
        ),
        Err(errs) => {
            let all = errs.join("\n");
            assert!(
                all.contains(fragment),
                "Expected fragment '{}' in:\n{}",
                fragment,
                all
            );
        }
    }
}

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n) => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

fn run_fails(src: &str) {
    if run_source(src).is_ok() {
        panic!("Should have failed:\n{}", src);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  PARSING
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_enum_no_fields() {
    parses_ok(
        r#"
        enum Color { Red, Green, Blue }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_enum_with_fields() {
    parses_ok(
        r#"
        enum Shape {
            Circle(float radius),
            Rectangle(float width, float height)
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_enum_with_methods() {
    parses_ok(
        r#"
        enum Dir { North, South;
            string name() { return "dir"; }
        }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_enum_constructor_no_args() {
    parses_ok(
        r#"
        enum Color { Red, Green }
        int main() {
            Color c = Color::Red;
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_enum_constructor_with_args() {
    parses_ok(
        r#"
        enum Shape { Circle(float radius) }
        int main() {
            Shape s = Shape::Circle(3.14);
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_match_wildcard() {
    parses_ok(
        r#"
        enum Color { Red, Green }
        int main() {
            Color c = Color::Red;
            match c {
                Color::Red   => { print("red"); }
                _            => { print("other"); }
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_match_with_bindings() {
    parses_ok(
        r#"
        enum Shape { Circle(float radius), Rectangle(float w, float h) }
        int main() {
            Shape s = Shape::Circle(5.0);
            match s {
                Shape::Circle(r)    => { print(r); }
                Shape::Rectangle(w, h) => { print(w, h); }
                _                   => {}
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_match_in_method() {
    parses_ok(
        r#"
        enum Dir { North, South;
            string label() {
                match this {
                    Dir::North => { return "N"; }
                    _          => { return "S"; }
                }
            }
        }
        int main() { return 0; }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  TYPECHECK
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_valid_enum_no_fields() {
    assert_tc_ok(
        r#"
        enum Color { Red, Green, Blue }
        int main() {
            Color c = Color::Red;
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_valid_enum_with_fields() {
    assert_tc_ok(
        r#"
        enum Msg { Text(string content), Number(int val) }
        int main() {
            Msg m = Msg::Text("hello");
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_valid_enum_match() {
    assert_tc_ok(
        r#"
        enum Color { Red, Green }
        int main() {
            Color c = Color::Green;
            match c {
                Color::Red   => { print("rouge"); }
                Color::Green => { print("vert"); }
                _            => {}
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_valid_match_bindings_types() {
    assert_tc_ok(
        r#"
        enum Msg { Val(int n) }
        int main() {
            Msg m = Msg::Val(42);
            match m {
                Msg::Val(n) => { int x = n + 1; }
                _           => {}
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_valid_enum_method() {
    assert_tc_ok(
        r#"
        enum Dir { North, South;
            string label() {
                match this {
                    Dir::North => { return "N"; }
                    _          => { return "S"; }
                }
            }
        }
        int main() {
            Dir d = Dir::North;
            string s = d.label();
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_error_unknown_enum() {
    assert_tc_err(
        r#"int main() { Ghost g = Ghost::Val; return 0; }"#,
        "inconnu",
    );
}

#[test]
fn tc_error_unknown_variant() {
    assert_tc_err(
        r#"
        enum Color { Red }
        int main() { Color c = Color::Purple; return 0; }
        "#,
        "Variante",
    );
}

#[test]
fn tc_error_wrong_variant_arg_count() {
    assert_tc_err(
        r#"
        enum Shape { Circle(float radius) }
        int main() { Shape s = Shape::Circle(1.0, 2.0); return 0; }
        "#,
        "champ",
    );
}

#[test]
fn tc_error_wrong_variant_arg_type() {
    assert_tc_err(
        r#"
        enum Shape { Circle(float radius) }
        int main() { Shape s = Shape::Circle("nope"); return 0; }
        "#,
        "incompatible",
    );
}

#[test]
fn tc_error_match_unknown_variant() {
    assert_tc_err(
        r#"
        enum Color { Red }
        int main() {
            Color c = Color::Red;
            match c {
                Color::Purple => { print("x"); }
                _             => {}
            }
            return 0;
        }
        "#,
        "Variante",
    );
}

#[test]
fn tc_error_match_wrong_binding_count() {
    assert_tc_err(
        r#"
        enum Msg { Val(int n) }
        int main() {
            Msg m = Msg::Val(1);
            match m {
                Msg::Val(a, b) => { print(a); }
                _              => {}
            }
            return 0;
        }
        "#,
        "binding",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  INTERPRÉTEUR
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_match_simple_variant() {
    // Retourne 1 si Red, 2 si Green
    assert_eq!(
        run_ok(
            r#"
        enum Color { Red, Green }
        int main() {
            Color c = Color::Red;
            match c {
                Color::Red   => { return 1; }
                Color::Green => { return 2; }
                _            => { return 0; }
            }
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_match_second_variant() {
    assert_eq!(
        run_ok(
            r#"
        enum Color { Red, Green }
        int main() {
            Color c = Color::Green;
            match c {
                Color::Red   => { return 1; }
                Color::Green => { return 2; }
                _            => { return 0; }
            }
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_match_wildcard() {
    assert_eq!(
        run_ok(
            r#"
        enum Color { Red, Green, Blue }
        int main() {
            Color c = Color::Blue;
            match c {
                Color::Red => { return 1; }
                _          => { return 99; }
            }
        }
    "#
        ),
        99
    );
}

#[test]
fn interp_match_extracts_binding() {
    // Le binding doit capturer la valeur du champ
    assert_eq!(
        run_ok(
            r#"
        enum Msg { Val(int n) }
        int main() {
            Msg m = Msg::Val(42);
            match m {
                Msg::Val(n) => { return n; }
                _           => { return -1; }
            }
        }
    "#
        ),
        42
    );
}

#[test]
fn interp_match_multiple_fields() {
    // Retourne la somme des deux champs
    assert_eq!(
        run_ok(
            r#"
        enum Pair { P(int a, int b) }
        int main() {
            Pair p = Pair::P(10, 32);
            match p {
                Pair::P(a, b) => { return a + b; }
                _             => { return 0; }
            }
        }
    "#
        ),
        42
    );
}

#[test]
fn interp_match_in_loop() {
    // Compte les Red dans une séquence (simulée avec if)
    assert_eq!(
        run_ok(
            r#"
        enum Color { Red, Green }
        int main() {
            int count = 0;
            Color c1 = Color::Red;
            Color c2 = Color::Green;
            Color c3 = Color::Red;
            match c1 { Color::Red => { count = count + 1; } _ => {} }
            match c2 { Color::Red => { count = count + 1; } _ => {} }
            match c3 { Color::Red => { count = count + 1; } _ => {} }
            return count;
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_enum_method_no_field() {
    // Méthode d'enum qui retourne une string selon la variante
    assert_eq!(
        run_ok(
            r#"
        enum Dir { North, South;
            int code() {
                match this {
                    Dir::North => { return 1; }
                    Dir::South => { return 2; }
                    _          => { return 0; }
                }
            }
        }
        int main() {
            Dir d = Dir::South;
            return d.code();
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_enum_method_with_field() {
    // Méthode d'enum qui utilise un champ de la variante
    assert_eq!(
        run_ok(
            r#"
        enum Shape {
            Circle(int radius),
            Square(int side);

            int perimeter() {
                match this {
                    Shape::Circle(radius) => { return 6 * radius; }
                    Shape::Square(side)   => { return 4 * side; }
                    _                     => { return 0; }
                }
            }
        }
        int main() {
            Shape s = Shape::Square(7);
            return s.perimeter();
        }
    "#
        ),
        28
    );
}

#[test]
fn interp_enum_method_returns_enum() {
    // Méthode qui retourne un autre enum
    assert_eq!(
        run_ok(
            r#"
        enum Coin { Heads, Tails;
            Coin flip() {
                match this {
                    Coin::Heads => { return Coin::Tails; }
                    _           => { return Coin::Heads; }
                }
            }
        }
        int main() {
            Coin c = Coin::Heads;
            Coin flipped = c.flip();
            match flipped {
                Coin::Heads => { return 1; }
                Coin::Tails => { return 2; }
                _           => { return 0; }
            }
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_enum_in_class_field() {
    // Un enum stocké dans un champ de classe
    assert_eq!(
        run_ok(
            r#"
        enum Status { Active, Inactive }
        class User {
            string name;
            Status status;
            User(string n) { name = n; status = Status::Active; }
            bool isActive() {
                match status {
                    Status::Active   => { return true; }
                    Status::Inactive => { return false; }
                    _                => { return false; }
                }
            }
        }
        int main() {
            User u = new User("Alice");
            if (u.isActive()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_nested_match() {
    // match dans un bras de match
    assert_eq!(
        run_ok(
            r#"
        enum A { X, Y }
        enum B { P, Q }
        int main() {
            A a = A::X;
            B b = B::Q;
            match a {
                A::X => {
                    match b {
                        B::P => { return 10; }
                        B::Q => { return 20; }
                        _    => { return 0; }
                    }
                }
                _ => { return -1; }
            }
        }
    "#
        ),
        20
    );
}

#[test]
fn interp_enum_result_pattern() {
    // Pattern Ok/Err style
    assert_eq!(
        run_ok(
            r#"
        enum Res { Ok(int value), Err(int code) }
        int main() {
            Res r = Res::Ok(7);
            match r {
                Res::Ok(v)  => { return v * 6; }
                Res::Err(c) => { return -c; }
                _           => { return 0; }
            }
        }
    "#
        ),
        42
    );
}

#[test]
fn interp_enum_match_assigns_variable() {
    // Le binding du match est utilisé dans une expression plus complexe
    assert_eq!(
        run_ok(
            r#"
        enum Box { Val(int n) }
        int main() {
            Box b = Box::Val(5);
            int result = 0;
            match b {
                Box::Val(n) => { result = n * n + 2 * n + 1; }
                _           => {}
            }
            return result;
        }
    "#
        ),
        36
    ); // (5+1)^2 = 36
}

#[test]
fn interp_unknown_variant_fails() {
    // Enum inconnu → erreur runtime
    run_fails(r#"int main() { Ghost g; return 0; }"#);
}

#[test]
fn interp_four_variants_dispatch() {
    // Quatre variantes, vérifie que le dispatch est correct
    assert_eq!(
        run_ok(
            r#"
        enum Op { Add, Sub, Mul, Div }
        int main() {
            int result = 0;
            Op op = Op::Mul;
            match op {
                Op::Add => { result = 10 + 5; }
                Op::Sub => { result = 10 - 5; }
                Op::Mul => { result = 10 * 5; }
                Op::Div => { result = 10 / 5; }
                _       => {}
            }
            return result;
        }
    "#
        ),
        50
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  GÉNÉRIQUES SUR LES ENUMS
// ─────────────────────────────────────────────────────────────────────────────

// ── Parsing ───────────────────────────────────────────────────────────────────

#[test]
fn parse_enum_generic_one_param() {
    parses_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_enum_generic_two_params() {
    parses_ok(
        r#"
        enum Result<T, E> { Ok(T value), Err(E error) }
        int main() { return 0; }
    "#,
    );
}

#[test]
fn parse_enum_generic_constructor_with_type_args() {
    parses_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(42);
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_enum_generic_constructor_no_type_args() {
    parses_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::None;
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_enum_generic_match() {
    parses_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(1);
            match x {
                Option::Some(v) => { return v; }
                Option::None    => { return 0; }
            }
            return 0;
        }
    "#,
    );
}

// ── Typechecker ───────────────────────────────────────────────────────────────

#[test]
fn tc_enum_generic_ok() {
    assert_tc_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(42);
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_enum_generic_wrong_type_arg() {
    assert_tc_err(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(true);
            return 0;
        }
    "#,
        "incompatible",
    );
}

#[test]
fn tc_enum_generic_wrong_param_count() {
    assert_tc_err(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int, bool>::Some(1);
            return 0;
        }
    "#,
        "paramètre(s) de type",
    );
}

#[test]
fn tc_enum_generic_result_ok() {
    assert_tc_ok(
        r#"
        enum Result<T, E> { Ok(T value), Err(E error) }
        int main() {
            Result<int, string> r = Result<int, string>::Ok(0);
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_enum_generic_match_binding_type() {
    assert_tc_ok(
        r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(10);
            int result = 0;
            match x {
                Option::Some(v) => { result = v; }
                Option::None    => {}
            }
            return result;
        }
    "#,
    );
}

// ── Interpréteur ─────────────────────────────────────────────────────────────

#[test]
fn interp_enum_generic_some_match() {
    assert_eq!(
        run_ok(
            r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::Some(42);
            match x {
                Option::Some(v) => { return v; }
                Option::None    => { return 0; }
            }
            return -1;
        }
    "#
        ),
        42
    );
}

#[test]
fn interp_enum_generic_none_match() {
    assert_eq!(
        run_ok(
            r#"
        enum Option<T> { Some(T value), None }
        int main() {
            Option<int> x = Option<int>::None;
            match x {
                Option::Some(v) => { return v; }
                Option::None    => { return 99; }
            }
            return -1;
        }
    "#
        ),
        99
    );
}

#[test]
fn interp_enum_generic_result_ok() {
    assert_eq!(
        run_ok(
            r#"
        enum Result<T, E> { Ok(T value), Err(E error) }
        int main() {
            Result<int, string> r = Result<int, string>::Ok(7);
            match r {
                Result::Ok(v)  => { return v; }
                Result::Err(e) => { return -1; }
            }
            return 0;
        }
    "#
        ),
        7
    );
}

#[test]
fn interp_enum_generic_result_err() {
    assert_eq!(
        run_ok(
            r#"
        enum Result<T, E> { Ok(T value), Err(E error) }
        int main() {
            Result<int, string> r = Result<int, string>::Err("echec");
            match r {
                Result::Ok(v)  => { return 1; }
                Result::Err(e) => { return 0; }
            }
            return -1;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_enum_generic_pair() {
    assert_eq!(
        run_ok(
            r#"
        enum Pair<A, B> { Of(A first, B second) }
        int main() {
            Pair<int, bool> p = Pair<int, bool>::Of(10, true);
            match p {
                Pair::Of(a, b) => { return a; }
            }
            return 0;
        }
    "#
        ),
        10
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  STDLIB  Option<T>  –  T?  /  ??  /  ?.  /  .get()  /  .isSome()  /  .isNone()
// ─────────────────────────────────────────────────────────────────────────────

// ── Syntaxe T? ───────────────────────────────────────────────────────────────

#[test]
fn parse_optional_type_sugar() {
    parses_ok("int main() { int? x = Option<int>::Some(1); return 0; }");
}

#[test]
fn parse_optional_string() {
    parses_ok(r#"int main() { string? s = Option<string>::Some("ok"); return 0; }"#);
}

#[test]
fn tc_optional_type_ok() {
    assert_tc_ok(
        r#"
        int main() {
            int? x = Option<int>::Some(42);
            int? y = Option<int>::None;
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_optional_type_wrong_inner() {
    assert_tc_err(
        r#"
        int main() {
            int? x = Option<int>::Some(true);
            return 0;
        }
    "#,
        "incompatible",
    );
}

// ── .get() ────────────────────────────────────────────────────────────────────

#[test]
fn interp_get_on_some() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::Some(42);
            return x.get();
        }
    "#
        ),
        42
    );
}

#[test]
fn interp_get_on_none_panics() {
    run_fails(
        r#"
        int main() {
            int? x = Option<int>::None;
            return x.get();
        }
    "#,
    );
}

// ── .isSome() / .isNone() ─────────────────────────────────────────────────────

#[test]
fn interp_is_some_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::Some(1);
            if (x.isSome()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_is_some_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::None;
            if (x.isSome()) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_is_none_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::None;
            if (x.isNone()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

// ── Opérateur ?? ─────────────────────────────────────────────────────────────

#[test]
fn parse_null_coalescing() {
    parses_ok("int main() { int? x = Option<int>::None; int v = x ?? 0; return v; }");
}

#[test]
fn tc_null_coalescing_ok() {
    assert_tc_ok(
        r#"
        int main() {
            int? x = Option<int>::Some(5);
            int v = x ?? 0;
            return v;
        }
    "#,
    );
}

#[test]
fn tc_null_coalescing_wrong_default() {
    assert_tc_err(
        r#"
        int main() {
            int? x = Option<int>::Some(5);
            int v = x ?? true;
            return 0;
        }
    "#,
        "incompatible",
    );
}

#[test]
fn interp_null_coalescing_some() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::Some(7);
            return x ?? 0;
        }
    "#
        ),
        7
    );
}

#[test]
fn interp_null_coalescing_none() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            int? x = Option<int>::None;
            return x ?? 42;
        }
    "#
        ),
        42
    );
}

// ── Opérateur ?. ─────────────────────────────────────────────────────────────

#[test]
fn parse_safe_method_call() {
    parses_ok(
        r#"
        class Box { int val; Box(int v) { this.val = v; } int value() { return this.val; } }
        int main() {
            Box? b = Option<Box>::Some(new Box(5));
            int? v = b?.value();
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_safe_method_call_ok() {
    assert_tc_ok(
        r#"
        class Num { int n; Num(int v) { this.n = v; } int get() { return this.n; } }
        int main() {
            Num? x = Option<Num>::Some(new Num(3));
            int? v = x?.get();
            return 0;
        }
    "#,
    );
}

#[test]
fn interp_safe_method_some() {
    assert_eq!(
        run_ok(
            r#"
        class Num { int n; Num(int v) { this.n = v; } int double() { return this.n * 2; } }
        int main() {
            Num? x = Option<Num>::Some(new Num(5));
            int? v = x?.double();
            return v ?? 0;
        }
    "#
        ),
        10
    );
}

#[test]
fn interp_safe_method_none() {
    assert_eq!(
        run_ok(
            r#"
        class Num { int n; Num(int v) { this.n = v; } int double() { return this.n * 2; } }
        int main() {
            Num? x = Option<Num>::None;
            int? v = x?.double();
            return v ?? 99;
        }
    "#
        ),
        99
    );
}

// ── Chaîne  ?.  +  ?? ────────────────────────────────────────────────────────

#[test]
fn interp_chain_safe_call_and_coalesce() {
    assert_eq!(
        run_ok(
            r#"
        class Counter { int n; Counter(int v) { this.n = v; } int inc() { return this.n + 1; } }
        int main() {
            Counter? c = Option<Counter>::Some(new Counter(9));
            int result = (c?.inc()) ?? 0;
            return result;
        }
    "#
        ),
        10
    );
}

#[test]
fn interp_chain_none_coalesce() {
    assert_eq!(
        run_ok(
            r#"
        class Counter { int n; Counter(int v) { this.n = v; } int inc() { return this.n + 1; } }
        int main() {
            Counter? c = Option<Counter>::None;
            int result = (c?.inc()) ?? 0;
            return result;
        }
    "#
        ),
        0
    );
}
