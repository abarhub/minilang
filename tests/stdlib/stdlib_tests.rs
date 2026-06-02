//! Tests de la bibliothèque standard minilang.
//! Couvre Option<T>, Result<T,E>, Either<L,R>, Pair<A,B>.

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

fn assert_tc_err(src: &str, fragment: &str) {
    match check_source(src) {
        Ok(()) => panic!("Typecheck should have failed (expected '{}'):\n{}", fragment, src),
        Err(e) => {
            let all = e.join("\n");
            assert!(all.contains(fragment),
                "Expected '{}' in:\n{}", fragment, all);
        }
    }
}

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

fn run_fails(src: &str) {
    if run_source(src).is_ok() {
        panic!("Should have failed:\n{}", src);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Result<T, E>
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_result_ok_variant() {
    parses_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(0);
            return 0;
        }
    "#);
}

#[test]
fn parse_result_err_variant() {
    parses_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("oops");
            return 0;
        }
    "#);
}

#[test]
fn tc_result_ok() {
    assert_tc_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(42);
            return 0;
        }
    "#);
}

#[test]
fn tc_result_err() {
    assert_tc_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("echec");
            return 0;
        }
    "#);
}

#[test]
fn tc_result_wrong_value_type() {
    assert_tc_err(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(true);
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn tc_result_wrong_error_type() {
    assert_tc_err(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err(42);
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn interp_result_get_value_ok() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(7);
            return r.getValue();
        }
    "#), 7);
}

#[test]
fn interp_result_get_value_on_err_panics() {
    run_fails(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("fail");
            return r.getValue();
        }
    "#);
}

#[test]
fn interp_result_get_error_err() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("echec");
            if (r.isErr()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_result_get_error_on_ok_panics() {
    run_fails(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(1);
            bool b = r.isOk();
            return r.getError();
        }
    "#);
}

#[test]
fn interp_result_is_ok_true() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(0);
            if (r.isOk()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_result_is_ok_false() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("e");
            if (r.isOk()) { return 1; }
            return 0;
        }
    "#), 0);
}

#[test]
fn interp_result_is_err_true() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Err("e");
            if (r.isErr()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_result_match() {
    assert_eq!(run_ok(r#"
        int main() {
            Result<int, string> r = Result<int, string>::Ok(21);
            match r {
                Result::Ok(v)  => { return v * 2; }
                Result::Err(e) => { return -1; }
            }
            return 0;
        }
    "#), 42);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Either<L, R>
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_either_left() {
    parses_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left(1);
            return 0;
        }
    "#);
}

#[test]
fn tc_either_ok() {
    assert_tc_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Right("ok");
            return 0;
        }
    "#);
}

#[test]
fn tc_either_wrong_left_type() {
    assert_tc_err(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left("wrong");
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn interp_either_get_left() {
    assert_eq!(run_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left(5);
            return e.getLeft();
        }
    "#), 5);
}

#[test]
fn interp_either_get_left_on_right_panics() {
    run_fails(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Right("x");
            return e.getLeft();
        }
    "#);
}

#[test]
fn interp_either_get_right_on_left_panics() {
    run_fails(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left(1);
            bool b = e.isLeft();
            return e.getRight();
        }
    "#);
}

#[test]
fn interp_either_is_left_true() {
    assert_eq!(run_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left(0);
            if (e.isLeft()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_either_is_right_true() {
    assert_eq!(run_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Right("r");
            if (e.isRight()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn interp_either_match() {
    assert_eq!(run_ok(r#"
        int main() {
            Either<int, string> e = Either<int, string>::Left(10);
            match e {
                Either::Left(l)  => { return l + 1; }
                Either::Right(r) => { return -1; }
            }
            return 0;
        }
    "#), 11);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Pair<A, B>
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_pair_new() {
    // Pair est maintenant un record — new Pair(a, b) remplace Pair::Of(a, b)
    parses_ok(r#"
        int main() {
            Pair<int, bool> p = new Pair<int, bool>(1, true);
            return 0;
        }
    "#);
}

#[test]
fn tc_pair_ok() {
    assert_tc_ok(r#"
        int main() {
            Pair<int, string> p = new Pair<int, string>(3, "hello");
            return 0;
        }
    "#);
}

#[test]
fn tc_pair_wrong_first_type() {
    assert_tc_err(r#"
        int main() {
            Pair<int, string> p = new Pair<int, string>(true, "hello");
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn tc_pair_wrong_second_type() {
    assert_tc_err(r#"
        int main() {
            Pair<int, string> p = new Pair<int, string>(1, 42);
            return 0;
        }
    "#, "incompatible");
}

#[test]
fn interp_pair_get_first() {
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, string> p = new Pair<int, string>(7, "x");
            return p.getFirst();
        }
    "#), 7);
}

#[test]
fn interp_pair_get_second() {
    // Les records ne supportent pas le match — on utilise le getter
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, int> p = new Pair<int, int>(3, 9);
            return p.getSecond();
        }
    "#), 9);
}

#[test]
fn interp_pair_both_elements() {
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, int> p = new Pair<int, int>(4, 6);
            return p.getFirst() + p.getSecond();
        }
    "#), 10);
}

#[test]
fn interp_pair_sum() {
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int, int> p = new Pair<int, int>(13, 29);
            return p.getFirst() + p.getSecond();
        }
    "#), 42);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Interopérabilité entre types stdlib
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interp_result_in_option() {
    assert_eq!(run_ok(r#"
        int main() {
            Option<Result<int, string>> r =
                Option<Result<int, string>>::Some(Result<int, string>::Ok(42));
            match r {
                Option::Some(res) => { return res.getValue(); }
                Option::None      => { return 0; }
            }
            return -1;
        }
    "#), 42);
}

#[test]
fn interp_pair_of_options() {
    assert_eq!(run_ok(r#"
        int main() {
            Pair<int?, int?> p = new Pair<int?, int?>(
                Option<int>::Some(3),
                Option<int>::None
            );
            int a = p.getFirst() ?? 0;
            int b = p.getSecond() ?? 99;
            return a + b;
        }
    "#), 102);
}
