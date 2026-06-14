//! Tests des classes String et Character — minilang stdlib.

use chumsky::Parser;
use mini_parser::interpreter::run_source;
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
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
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

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n) => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  String — Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_string_length() {
    parses_ok(
        r#"
        int main() {
            string s = "hello";
            int n = s.length();
            return n;
        }
    "#,
    );
}

#[test]
fn parse_string_charat() {
    parses_ok(
        r#"
        int main() {
            string s = "abc";
            Option<char> c = s.charAt(0);
            return 0;
        }
    "#,
    );
}

#[test]
fn parse_string_split() {
    parses_ok(
        r#"
        int main() {
            string s = "a,b,c";
            List<string> parts = s.split(",");
            return parts.size();
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  String — Typecheck
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_string_length_returns_int() {
    assert_tc_ok(
        r#"
        int main() {
            string s = "hello";
            int n = s.length();
            return n;
        }
    "#,
    );
}

#[test]
fn tc_string_charat_returns_char() {
    assert_tc_ok(
        r#"
        int main() {
            string s = "abc";
            Option<char> c = s.charAt(0);
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_string_contains_returns_bool() {
    assert_tc_ok(
        r#"
        int main() {
            string s = "hello world";
            bool b = s.contains("world");
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_string_split_returns_array() {
    assert_tc_ok(
        r#"
        int main() {
            string s = "a,b,c";
            List<string> parts = s.split(",");
            return parts.size();
        }
    "#,
    );
}

#[test]
fn tc_string_charat_wrong_arg() {
    assert_tc_err(
        r#"
        int main() {
            string s = "hello";
            char c = s.charAt(true);
            return 0;
        }
    "#,
        "incompatible",
    );
}

#[test]
fn tc_string_to_upper_returns_string() {
    assert_tc_ok(
        r#"
        int main() {
            string s = "hello";
            string u = s.toUpperCase();
            return 0;
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  String — Interprétation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_string_length() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            return s.length();
        }
    "#
        ),
        5
    );
}

#[test]
fn interp_string_is_empty_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hi";
            if (s.isEmpty()) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_string_is_empty_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "";
            if (s.isEmpty()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_string_charat() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "abc";
            match s.charAt(1) {
                Option::Some(c) => { return c.toInt(); }
                Option::None    => { return -1; }
            }
        }
    "#
        ),
        'b' as i64
    );
}

#[test]
fn interp_string_charat_oob_returns_none() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hi";
            match s.charAt(10) {
                Option::Some(c) => { return 1; }
                Option::None    => { return 0; }
            }
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_string_contains_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello world";
            if (s.contains("world")) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_string_contains_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello world";
            if (s.contains("xyz")) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_string_substring() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            string sub = s.substring(1, 4);
            return sub.length();
        }
    "#
        ),
        3
    );
}

#[test]
fn interp_string_to_upper() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            string u = s.toUpperCase();
            match u.charAt(0) {
                Option::Some(c) => { return c.toInt(); }
                Option::None    => { return -1; }
            }
        }
    "#
        ),
        'H' as i64
    );
}

#[test]
fn interp_string_to_lower() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "HELLO";
            string l = s.toLowerCase();
            match l.charAt(0) {
                Option::Some(c) => { return c.toInt(); }
                Option::None    => { return -1; }
            }
        }
    "#
        ),
        'h' as i64
    );
}

#[test]
fn interp_string_starts_with_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            if (s.startsWith("hel")) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_string_starts_with_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            if (s.startsWith("world")) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_string_ends_with() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            if (s.endsWith("llo")) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_string_index_of_found() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            match s.indexOf("ll") {
                Option::Some(i) => { return i; }
                Option::None    => { return -1; }
            }
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_string_index_of_not_found() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            match s.indexOf("xyz") {
                Option::Some(i) => { return i; }
                Option::None    => { return -1; }
            }
        }
    "#
        ),
        -1
    );
}

#[test]
fn interp_string_trim() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "  hi  ";
            string t = s.trim();
            return t.length();
        }
    "#
        ),
        2
    );
}

#[test]
fn interp_string_replace() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "aabbaa";
            string r = s.replace("aa", "x");
            return r.length();
        }
    "#
        ),
        4
    );
}

#[test]
fn interp_string_equals_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            if (s.equals("hello")) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_string_equals_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "hello";
            if (s.equals("world")) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_string_split_count() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "a,b,c";
            List<string> parts = s.split(",");
            return parts.size();
        }
    "#
        ),
        3
    );
}

#[test]
fn interp_string_split_access() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "x,y,z";
            List<string> parts = s.split(",");
            string first = parts.get(0).get();
            return first.length();
        }
    "#
        ),
        1
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Character — Parsing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_char_is_letter() {
    parses_ok(
        r#"
        int main() {
            char c = 'a';
            bool b = c.isLetter();
            return 0;
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Character — Typecheck
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tc_char_is_letter_returns_bool() {
    assert_tc_ok(
        r#"
        int main() {
            char c = 'a';
            bool b = c.isLetter();
            return 0;
        }
    "#,
    );
}

#[test]
fn tc_char_to_int_returns_int() {
    assert_tc_ok(
        r#"
        int main() {
            char c = 'A';
            int n = c.toInt();
            return n;
        }
    "#,
    );
}

#[test]
fn tc_char_to_string_returns_string() {
    assert_tc_ok(
        r#"
        int main() {
            char c = 'x';
            string s = c.toString();
            return s.length();
        }
    "#,
    );
}

#[test]
fn tc_char_to_upper_returns_char() {
    assert_tc_ok(
        r#"
        int main() {
            char c = 'a';
            char u = c.toUpperCase();
            return 0;
        }
    "#,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Character — Interprétation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_char_is_letter_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'a';
            if (c.isLetter()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_is_letter_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = '3';
            if (c.isLetter()) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_char_is_digit_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = '7';
            if (c.isDigit()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_is_digit_false() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'z';
            if (c.isDigit()) { return 1; }
            return 0;
        }
    "#
        ),
        0
    );
}

#[test]
fn interp_char_is_whitespace() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = ' ';
            if (c.isWhitespace()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_is_upper_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'A';
            if (c.isUpperCase()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_is_lower_true() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'a';
            if (c.isLowerCase()) { return 1; }
            return 0;
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_to_upper() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'a';
            char u = c.toUpperCase();
            return u.toInt();
        }
    "#
        ),
        'A' as i64
    );
}

#[test]
fn interp_char_to_lower() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'Z';
            char l = c.toLowerCase();
            return l.toInt();
        }
    "#
        ),
        'z' as i64
    );
}

#[test]
fn interp_char_to_int_a() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'A';
            return c.toInt();
        }
    "#
        ),
        65
    );
}

#[test]
fn interp_char_to_string_length() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            char c = 'x';
            string s = c.toString();
            return s.length();
        }
    "#
        ),
        1
    );
}

#[test]
fn interp_char_chained_with_string() {
    assert_eq!(
        run_ok(
            r#"
        int main() {
            string s = "Hello";
            match s.charAt(0) {
                Option::Some(c) => { if (c.isUpperCase()) { return 1; } }
                Option::None    => { }
            }
            return 0;
        }
    "#
        ),
        1
    );
}
