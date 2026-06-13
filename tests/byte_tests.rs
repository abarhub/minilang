//! Tests du type primitif `byte` (octet non signé 0–255) et des conversions
//! string <-> byte[] via la classe utilitaire injectable Bytes.
//! byte est un type de stockage : pas d'arithmétique, conversions via int
//! (int.toByte() -> Option<byte>, byte.toInt() -> int).

use mini_parser::typechecker::check_source;
use mini_parser::interpreter::{run_source, run_source_with_output};
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
        panic!("Typecheck should pass:\n{}\n---\n{}", src, e.join("\n"));
    }
}

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!("Typecheck should have failed (expected '{}'):\n{}", fragment, src),
        Err(e) => {
            let all = e.join("\n");
            assert!(all.contains(fragment), "Expected '{}' in:\n{}", fragment, all);
        }
    }
}

fn run_ok(src: &str) -> i64 {
    assert_tc_ok(src);
    run_source(src).unwrap_or_else(|e| panic!("Run failed:\n{}\n---\n{}", src, e))
}

fn run_output(src: &str) -> (i64, Vec<String>) {
    assert_tc_ok(src);
    run_source_with_output(src).unwrap_or_else(|e| panic!("Run failed:\n{}", e))
}

// ── Parsing / type ─────────────────────────────────────────────────────────────

#[test]
fn parse_byte_decls() {
    parses_ok(r#"
        int main() {
            byte b;
            byte[] data = new byte[4];
            return 0;
        }
    "#);
}

#[test]
fn byte_default_is_zero() {
    let ret = run_ok(r#"
        int main() {
            byte b;
            return b.toInt();
        }
    "#);
    assert_eq!(ret, 0);
}

// ── Conversions int <-> byte ────────────────────────────────────────────────

#[test]
fn int_to_byte_in_range() {
    let ret = run_ok(r#"
        int main() {
            int n = 200;
            byte b = n.toByte().get();
            return b.toInt();
        }
    "#);
    assert_eq!(ret, 200);
}

#[test]
fn int_to_byte_out_of_range_is_none() {
    let ret = run_ok(r#"
        int main() {
            int n = 300;
            if (n.toByte().isNone()) { return 1; }   // hors 0..255
            return 0;
        }
    "#);
    assert_eq!(ret, 1);
}

#[test]
fn int_to_byte_negative_is_none() {
    let ret = run_ok(r#"
        int main() {
            int n = 0 - 1;
            if (n.toByte().isNone()) { return 1; }
            return 0;
        }
    "#);
    assert_eq!(ret, 1);
}

#[test]
fn byte_roundtrip_via_int() {
    let (ret, lines) = run_output(r#"
        int main() {
            int n = 65;
            byte b = n.toByte().get();
            print(b.toString());        // "65"
            print(b.toInt().toString()); // "65"
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["65", "65"]);
}

#[test]
fn byte_equals() {
    let ret = run_ok(r#"
        int main() {
            byte a = (65).toByte().get();
            byte b = (65).toByte().get();
            byte c = (66).toByte().get();
            if (a.equals(b) && !a.equals(c)) { return 1; }
            return 0;
        }
    "#);
    assert_eq!(ret, 1);
}

// ── byte[] : tableau d'octets ───────────────────────────────────────────────

#[test]
fn byte_array_set_and_get() {
    let ret = run_ok(r#"
        int main() {
            byte[] data = new byte[3];
            match data.set(0) { Option::Some(r) => { r.set((10).toByte().get()); } Option::None => {} }
            match data.set(1) { Option::Some(r) => { r.set((20).toByte().get()); } Option::None => {} }
            int sum = 0;
            sum = sum + data.get(0).get().toInt();
            sum = sum + data.get(1).get().toInt();
            return sum;     // 30
        }
    "#);
    assert_eq!(ret, 30);
}

// ── Type safety : byte n'est pas int ────────────────────────────────────────

#[test]
fn tc_err_byte_not_assignable_from_int() {
    assert_tc_err(r#"
        int main() {
            byte b = 65;     // int -> byte interdit sans conversion
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn tc_err_byte_no_arithmetic() {
    assert_tc_err(r#"
        int main() {
            byte a = (1).toByte().get();
            byte b = (2).toByte().get();
            byte c = a + b;   // pas d'arithmétique sur byte
            return 0;
        }
    "#, "non applicable");
}

#[test]
fn tc_err_byte_array_not_int_array() {
    assert_tc_err(r#"
        int main() {
            byte[] data = new int[]{1, 2, 3};   // int[] -> byte[] interdit
            return 0;
        }
    "#, "incompatible");
}

// ── Conversions string <-> byte[] via Bytes (injectable) ────────────────────

#[test]
fn bytes_encode_utf8() {
    // "AB" -> [65, 66]
    let ret = run_ok(r#"
        int main() {
            Bytes bytes = inject Bytes;
            byte[] data = bytes.encodeUtf8("AB");
            int total = data.get(0).get().toInt() + data.get(1).get().toInt();
            return total;    // 65 + 66 = 131
        }
    "#);
    assert_eq!(ret, 131);
}

#[test]
fn bytes_encode_utf8_length_multibyte() {
    // 'é' = 2 octets en UTF-8
    let ret = run_ok(r#"
        int main() {
            Bytes bytes = inject Bytes;
            byte[] data = bytes.encodeUtf8("é");
            return data.length();    // 2
        }
    "#);
    assert_eq!(ret, 2);
}

#[test]
fn bytes_roundtrip_encode_decode() {
    let (ret, lines) = run_output(r#"
        int main() {
            Bytes bytes = inject Bytes;
            byte[] data = bytes.encodeUtf8("héllo");
            Result<string, IoError> r = bytes.decodeUtf8(data);
            print(r.getValue());    // "héllo"
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["héllo"]);
}

#[test]
fn bytes_decode_invalid_utf8_is_err() {
    // 0xFF seul n'est pas de l'UTF-8 valide
    let (ret, lines) = run_output(r#"
        int main() {
            byte[] bad = new byte[1];
            match bad.set(0) { Option::Some(r) => { r.set((255).toByte().get()); } Option::None => {} }
            Bytes bytes = inject Bytes;
            Result<string, IoError> r = bytes.decodeUtf8(bad);
            if (r.isErr()) { print("erreur: " + r.getError().message()); }
            return 0;
        }
    "#);
    assert_eq!(ret, 0);
    assert_eq!(lines, vec!["erreur: séquence UTF-8 invalide"]);
}
