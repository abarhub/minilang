//! Tests de l'interface HashCode — minilang stdlib.
//! Vérifie que hashCode() est disponible et cohérent sur int, bool, char,
//! string, float, double et Pair.

use mini_parser::interpreter::run_source;
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
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_int_hashcode() {
    parses_ok(r#"
        int main() {
            int n = 42;
            int h = n.hashCode();
            return h;
        }
    "#);
}

#[test]
fn parse_string_hashcode() {
    parses_ok(r#"
        int main() {
            string s = "hello";
            int h = s.hashCode();
            return 0;
        }
    "#);
}

#[test]
fn parse_pair_hashcode() {
    parses_ok(r#"
        int main() {
            Pair<int, int> p = Pair<int, int>::Of(1, 2);
            int h = p.hashCode();
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Typecheck
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_int_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            int n = 10;
            int h = n.hashCode();
            return h;
        }
    "#);
}

#[test]
fn tc_bool_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            bool b = true;
            int h = b.hashCode();
            return h;
        }
    "#);
}

#[test]
fn tc_char_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            char c = 'A';
            int h = c.hashCode();
            return h;
        }
    "#);
}

#[test]
fn tc_string_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            string s = "hello";
            int h = s.hashCode();
            return 0;
        }
    "#);
}

#[test]
fn tc_float_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            float f = 3.14;
            int h = f.hashCode();
            return 0;
        }
    "#);
}

#[test]
fn tc_double_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            double d = 2.718;
            int h = d.hashCode();
            return 0;
        }
    "#);
}

#[test]
fn tc_pair_hashcode_returns_int() {
    assert_tc_ok(r#"
        int main() {
            Pair<int, string> p = Pair<int, string>::Of(1, "a");
            int h = p.hashCode();
            return 0;
        }
    "#);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Interprétation — valeurs exactes ou propriétés
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_int_hashcode_is_value() {
    // hashCode() d'un int est la valeur elle-même
    assert_eq!(run_ok(r#"
        int main() {
            return 42.hashCode();
        }
    "#), 42);
}

#[test]
fn interp_int_hashcode_negative() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = -7;
            return n.hashCode();
        }
    "#), -7);
}

#[test]
fn interp_bool_hashcode_true() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = true;
            return b.hashCode();
        }
    "#), 1);
}

#[test]
fn interp_bool_hashcode_false() {
    assert_eq!(run_ok(r#"
        int main() {
            bool b = false;
            return b.hashCode();
        }
    "#), 0);
}

#[test]
fn interp_char_hashcode_is_codepoint() {
    // hashCode() d'un char est son point de code Unicode
    assert_eq!(run_ok(r#"
        int main() {
            char c = 'A';
            return c.hashCode();
        }
    "#), 'A' as i64);
}

#[test]
fn interp_char_hashcode_zero() {
    assert_eq!(run_ok(r#"
        int main() {
            char c = '0';
            return c.hashCode();
        }
    "#), '0' as i64);
}

#[test]
fn interp_string_hashcode_same_strings_equal() {
    // Deux chaînes identiques doivent avoir le même hashCode
    assert_eq!(run_ok(r#"
        int main() {
            string a = "hello";
            string b = "hello";
            if (a.hashCode() == b.hashCode()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_string_hashcode_different_strings() {
    // Deux chaînes différentes ont (très probablement) des hashCodes différents
    assert_eq!(run_ok(r#"
        int main() {
            string a = "hello";
            string b = "world";
            if (a.hashCode() == b.hashCode()) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn interp_string_empty_hashcode() {
    // hashCode() de la chaîne vide ne plante pas
    assert_eq!(run_ok(r#"
        int main() {
            string s = "";
            int h = s.hashCode();
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_float_hashcode_same_values_equal() {
    assert_eq!(run_ok(r#"
        int main() {
            float a = 3.14;
            float b = 3.14;
            if (a.hashCode() == b.hashCode()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_pair_hashcode_same_pairs_equal() {
    // Deux Pair identiques doivent avoir le même hashCode
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, int> p1 = Pair<int, int>::Of(3, 7);
            Pair<int, int> p2 = Pair<int, int>::Of(3, 7);
            if (p1.hashCode() == p2.hashCode()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_pair_hashcode_different_pairs() {
    // (1,2) et (2,1) doivent avoir des hashCodes différents (ordre compte)
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, int> p1 = Pair<int, int>::Of(1, 2);
            Pair<int, int> p2 = Pair<int, int>::Of(2, 1);
            if (p1.hashCode() == p2.hashCode()) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn interp_hashcode_consistent_with_equals_int() {
    // equals() → hashCode() identiques
    assert_eq!(run_ok(r#"
        int main() {
            int a = 100;
            int b = 100;
            if (a.equals(b)) {
                if (a.hashCode() == b.hashCode()) { return 1; }
            }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_hashcode_consistent_with_equals_string() {
    assert_eq!(run_ok(r#"
        int main() {
            string a = "minilang";
            string b = "minilang";
            if (a.equals(b)) {
                if (a.hashCode() == b.hashCode()) { return 1; }
            }
            return 0;
        }
    "#), 1);
}
