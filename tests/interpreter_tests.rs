//! Tests de l'interpréteur : vérifie que les programmes s'exécutent
//! correctement et renvoient les bons codes de sortie.

use mini_parser::interpreter::run_source;

// ── Helper ────────────────────────────────────────────────────────────────────

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(code) => code,
        Err(e)   => panic!("Exécution inattendue échouée :\n{}\n---\n{}", src, e),
    }
}

fn run_fails(src: &str) {
    if run_source(src).is_ok() {
        panic!("Exécution aurait dû échouer :\n{}", src);
    }
}

// ── Retours et littéraux ──────────────────────────────────────────────────────

#[test]
fn test_return_literal() {
    assert_eq!(run_ok("int main() { return 42; }"), 42);
}

#[test]
fn test_return_zero() {
    assert_eq!(run_ok("int main() { return 0; }"), 0);
}

#[test]
fn test_return_negative() {
    assert_eq!(run_ok("int main() { return -7; }"), -7);
}

// ── Arithmétique entière ──────────────────────────────────────────────────────

#[test]
fn test_addition() {
    assert_eq!(run_ok("int main() { return 3 + 4; }"), 7);
}

#[test]
fn test_subtraction() {
    assert_eq!(run_ok("int main() { return 10 - 3; }"), 7);
}

#[test]
fn test_multiplication() {
    assert_eq!(run_ok("int main() { return 6 * 7; }"), 42);
}

#[test]
fn test_division() {
    assert_eq!(run_ok("int main() { return 20 / 4; }"), 5);
}

#[test]
fn test_modulo() {
    assert_eq!(run_ok("int main() { return 17 % 5; }"), 2);
}

#[test]
fn test_power() {
    assert_eq!(run_ok("int main() { int x = 2 ** 10; return x; }"), 1024);
}

#[test]
fn test_operator_precedence() {
    // 2 + 3 * 4 = 14, pas 20
    assert_eq!(run_ok("int main() { return 2 + 3 * 4; }"), 14);
}

#[test]
fn test_parentheses_override_precedence() {
    assert_eq!(run_ok("int main() { return (2 + 3) * 4; }"), 20);
}

#[test]
fn test_chained_arithmetic() {
    assert_eq!(run_ok("int main() { return 100 - 3 * 10 + 5 / 5; }"), 71);
}

#[test]
fn test_division_by_zero_fails() {
    run_fails("int main() { int x = 1 / 0; return x; }");
}

#[test]
fn test_modulo_by_zero_fails() {
    run_fails("int main() { int x = 5 % 0; return x; }");
}

// ── Variables locales ─────────────────────────────────────────────────────────

#[test]
fn test_local_var_assignment() {
    assert_eq!(run_ok(r#"
        int main() {
            int x = 10;
            int y = 20;
            int z = x + y;
            return z;
        }
    "#), 30);
}

#[test]
fn test_var_reassignment() {
    assert_eq!(run_ok(r#"
        int main() {
            int x = 1;
            x = x + 1;
            x = x * 3;
            return x;
        }
    "#), 6);
}

// ── if / else ─────────────────────────────────────────────────────────────────

#[test]
fn test_if_true_branch() {
    assert_eq!(run_ok(r#"
        int main() {
            if (true) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_if_false_branch() {
    assert_eq!(run_ok(r#"
        int main() {
            if (false) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn test_if_else() {
    assert_eq!(run_ok(r#"
        int main() {
            int x = 5;
            if (x > 10) { return 1; } else { return 2; }
        }
    "#), 2);
}

#[test]
fn test_if_else_if_else() {
    assert_eq!(run_ok(r#"
        int main() {
            int score = 75;
            if (score >= 90)      { return 4; }
            else if (score >= 75) { return 3; }
            else if (score >= 60) { return 2; }
            else                  { return 1; }
        }
    "#), 3);
}

#[test]
fn test_nested_if() {
    assert_eq!(run_ok(r#"
        int main() {
            int x = 5;
            int y = 3;
            if (x > 0) {
                if (y > 0) { return 1; }
                else       { return 2; }
            }
            return 0;
        }
    "#), 1);
}

// ── while ─────────────────────────────────────────────────────────────────────

#[test]
fn test_while_sum() {
    assert_eq!(run_ok(r#"
        int main() {
            int i   = 1;
            int sum = 0;
            while (i <= 10) {
                sum = sum + i;
                i = i + 1;
            }
            return sum;
        }
    "#), 55);
}

#[test]
fn test_while_not_entered() {
    assert_eq!(run_ok(r#"
        int main() {
            int i = 10;
            while (i < 5) { i = i + 1; }
            return i;
        }
    "#), 10);
}

// ── do-while ──────────────────────────────────────────────────────────────────

#[test]
fn test_do_while_executes_at_least_once() {
    assert_eq!(run_ok(r#"
        int main() {
            int i = 100;
            do { i = i + 1; } while (i < 5);
            return i;
        }
    "#), 101); // exécuté une fois même si condition fausse d'entrée
}

#[test]
fn test_do_while_sum() {
    assert_eq!(run_ok(r#"
        int main() {
            int i   = 1;
            int sum = 0;
            do {
                sum = sum + i;
                i = i + 1;
            } while (i <= 5);
            return sum;
        }
    "#), 15);
}

// ── for ───────────────────────────────────────────────────────────────────────

#[test]
fn test_for_sum() {
    assert_eq!(run_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 1; i <= 100; i = i + 1) {
                sum = sum + i;
            }
            return sum;
        }
    "#), 5050);
}

#[test]
fn test_for_not_entered() {
    assert_eq!(run_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 10; i < 5; i = i + 1) { sum = sum + 1; }
            return sum;
        }
    "#), 0);
}

#[test]
fn test_for_break() {
    assert_eq!(run_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 1; i <= 10; i = i + 1) {
                if (i == 6) { break; }
                sum = sum + i;
            }
            return sum;
        }
    "#), 15); // 1+2+3+4+5
}

#[test]
fn test_for_continue_skip_even() {
    assert_eq!(run_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 1; i <= 10; i = i + 1) {
                if (i % 2 == 0) { continue; }
                sum = sum + i;
            }
            return sum;
        }
    "#), 25); // 1+3+5+7+9
}

// ── Constructeurs & méthodes ──────────────────────────────────────────────────

#[test]
fn test_constructor_sets_fields() {
    assert_eq!(run_ok(r#"
        class Counter {
            int count;
            Counter(int n) { count = n; }
            int get() { return count; }
        }
        int main() {
            Counter c = new Counter(7);
            return c.get();
        }
    "#), 7);
}

#[test]
fn test_method_modifies_field() {
    assert_eq!(run_ok(r#"
        class Counter {
            int count;
            Counter() { count = 0; }
            void inc() { count = count + 1; }
            int get()  { return count; }
        }
        int main() {
            Counter c = new Counter();
            c.inc();
            c.inc();
            c.inc();
            return c.get();
        }
    "#), 3);
}

#[test]
fn test_method_with_params() {
    assert_eq!(run_ok(r#"
        class Calc {
            int val;
            Calc(int v) { val = v; }
            int add(int n) { return val + n; }
            int mul(int n) { return val * n; }
        }
        int main() {
            Calc c = new Calc(10);
            int a = c.add(5);
            int b = c.mul(3);
            return a + b;
        }
    "#), 45); // 15 + 30
}

#[test]
fn test_multiple_constructors_select_by_arity() {
    assert_eq!(run_ok(r#"
        class Vec {
            int x;
            int y;
            Vec()         { x = 0; y = 0; }
            Vec(int a)    { x = a; y = 0; }
            Vec(int a, int b) { x = a; y = b; }
            int sum() { return x + y; }
        }
        int main() {
            Vec v0 = new Vec();
            Vec v1 = new Vec(3);
            Vec v2 = new Vec(3, 4);
            return v0.sum() + v1.sum() + v2.sum();
        }
    "#), 10); // 0 + 3 + 7
}

// ── Héritage ──────────────────────────────────────────────────────────────────

#[test]
fn test_inherited_method() {
    assert_eq!(run_ok(r#"
        class Animal {
            int legs;
            Animal(int l) { legs = l; }
            int getLegs() { return legs; }
        }
        class Dog extends Animal {
            Dog() { legs = 4; }
        }
        int main() {
            Dog d = new Dog();
            return d.getLegs();
        }
    "#), 4);
}

#[test]
fn test_method_override() {
    assert_eq!(run_ok(r#"
        class Shape {
            int area() { return 0; }
        }
        class Square extends Shape {
            int side;
            Square(int s) { side = s; }
            int area() { return side * side; }
        }
        int main() {
            Square s = new Square(5);
            return s.area();
        }
    "#), 25);
}

#[test]
fn test_three_level_inheritance() {
    assert_eq!(run_ok(r#"
        class A { int val; A(int v) { val = v; } int get() { return val; } }
        class B extends A { B(int v) { val = v; } }
        class C extends B { C(int v) { val = v; } }
        int main() {
            C c = new C(99);
            return c.get();
        }
    "#), 99);
}

// ── Algorithmes classiques ────────────────────────────────────────────────────

#[test]
fn test_factorial_iterative() {
    assert_eq!(run_ok(r#"
        int main() {
            int n      = 10;
            int result = 1;
            int i      = n;
            while (i > 1) {
                result = result * i;
                i = i - 1;
            }
            return result;
        }
    "#), 3628800);
}

#[test]
fn test_fibonacci_iterative() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 10;
            int a = 0;
            int b = 1;
            for (int i = 0; i < n; i = i + 1) {
                int tmp = a + b;
                a = b;
                b = tmp;
            }
            return a;
        }
    "#), 55); // fib(10) = 55
}

#[test]
fn test_collatz_steps() {
    assert_eq!(run_ok(r#"
        int main() {
            int n     = 27;
            int steps = 0;
            while (n != 1) {
                if (n % 2 == 0) { n = n / 2; }
                else            { n = n * 3 + 1; }
                steps = steps + 1;
            }
            return steps;
        }
    "#), 111);
}

#[test]
fn test_gcd_euclid() {
    assert_eq!(run_ok(r#"
        int main() {
            int a = 48;
            int b = 18;
            while (b != 0) {
                int tmp = b;
                b = a % b;
                a = tmp;
            }
            return a;
        }
    "#), 6);
}

#[test]
fn test_sum_of_squares() {
    assert_eq!(run_ok(r#"
        int main() {
            int sum = 0;
            for (int i = 1; i <= 5; i = i + 1) {
                sum = sum + i * i;
            }
            return sum;
        }
    "#), 55); // 1+4+9+16+25
}

// ── Champs via accès direct ───────────────────────────────────────────────────

#[test]
fn test_field_assign_from_outside() {
    assert_eq!(run_ok(r#"
        class Point {
            int x;
            int y;
            Point() { x = 0; y = 0; }
            int sum() { return x + y; }
        }
        int main() {
            Point p = new Point();
            p.x = 3;
            p.y = 4;
            return p.sum();
        }
    "#), 7);
}

// ── Erreurs runtime ───────────────────────────────────────────────────────────

#[test]
fn test_runtime_unknown_variable() {
    run_fails("int main() { return x; }");
}

#[test]
fn test_runtime_unknown_method() {
    run_fails(r#"
        class Foo { Foo() {} }
        int main() { Foo f = new Foo(); f.nothing(); return 0; }
    "#);
}

#[test]
fn test_runtime_unknown_class() {
    run_fails("int main() { Ghost g; return 0; }");
}
