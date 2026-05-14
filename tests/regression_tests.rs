//! Tests de régression — chaque test correspond à un bug corrigé.
//! Ces tests garantissent qu'une régression sera détectée tôt.

use chumsky::Parser;
use mini_parser::parser::program_parser;
use mini_parser::interpreter::run_source;
use mini_parser::interpreter::run_source_with_output;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parses_ok(src: &str) {
    let full = format!("{}\n{}", mini_parser::STDLIB, src);
    match program_parser().parse(full.as_str()) {
        Ok(_) => {}
        Err(errs) => panic!(
            "Parse échoué :\n{}\n---\n{}",
            src,
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        ),
    }
}

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//  BUG #1 — Littéraux flottants commençant par zéro (ex : 0.5, 0.001)
//
//  Cause : le parser consommait le `0` comme entier, puis le `.` était
//  interprété comme un accès de champ, laissant le reste imparsable.
//  Correction : parseur `number_lit` unifié qui lit la partie entière puis
//  tente optionnellement `.chiffres` en un seul passage.
// ═════════════════════════════════════════════════════════════════════════════

/// 0.5 — cas minimal, fraction d'une demi-unité.
#[test]
fn regression_float_literal_zero_point_five_parses() {
    parses_ok(r#"
        int main() {
            float f = 0.5;
            return 0;
        }
    "#);
}

/// 0.001 — le cas original qui avait déclenché le bug.
#[test]
fn regression_float_literal_zero_point_001_parses() {
    parses_ok(r#"
        int main() {
            float f = 0.001;
            return 0;
        }
    "#);
}

/// 0.0 — cas limite zéro exact.
#[test]
fn regression_float_literal_zero_point_zero_parses() {
    parses_ok(r#"
        int main() {
            double d = 0.0;
            return 0;
        }
    "#);
}

/// 0.5 dans une expression arithmétique (contexte réel du bug).
#[test]
fn regression_float_zero_prefix_in_expression_parses() {
    parses_ok(r#"
        int main() {
            int n = 3;
            float f = n * 0.5;
            return 0;
        }
    "#);
}

/// Valeur calculée correcte : 3 * 0.5 == 1 (troncature toInt).
#[test]
fn regression_float_zero_prefix_evaluates_correctly() {
    assert_eq!(run_ok(r#"
        int main() {
            int n = 3;
            float f = n * 0.5;
            return f.toInt();
        }
    "#), 1);
}

/// 0.5 + 0.5 == 1.0 → toInt == 1.
#[test]
fn regression_float_zero_prefix_addition() {
    assert_eq!(run_ok(r#"
        int main() {
            float a = 0.5;
            float b = 0.5;
            return (a + b).toInt();
        }
    "#), 1);
}

/// Plusieurs littéraux 0.x dans le même scope.
#[test]
fn regression_multiple_zero_prefix_floats() {
    assert_eq!(run_ok(r#"
        int main() {
            float a = 0.25;
            float b = 0.75;
            return (a + b).toInt();
        }
    "#), 1);
}

/// 0.x avec double.
#[test]
fn regression_double_zero_prefix_literal() {
    assert_eq!(run_ok(r#"
        int main() {
            double d = 0.5;
            return d.toInt();
        }
    "#), 0); // troncature vers zéro : 0.5 → 0
}

// ═════════════════════════════════════════════════════════════════════════════
//  BUG #2 — Déclarations `package` / `import` ignorées après la stdlib
//
//  Cause : le parser attendait package puis imports AVANT les déclarations
//  de haut niveau. Or run_source_with_output() préfixe la stdlib (classes)
//  devant le source utilisateur. Si ce dernier commence par `package` ou
//  `import`, ces directives se retrouvaient après des classes et causaient
//  une erreur "found end of input".
//  Correction : package_decl et import_decl intégrés dans le choix
//  top_decl répété, donc acceptés n'importe où dans le flux.
// ═════════════════════════════════════════════════════════════════════════════

/// Source avec `package` seul — doit parser et s'exécuter.
#[test]
fn regression_package_decl_with_stdlib_prefix() {
    assert_eq!(run_ok(r#"
        package com.example.test;
        int main() {
            return 42;
        }
    "#), 42);
}

/// Source avec `import` seul — doit parser et s'exécuter.
#[test]
fn regression_import_decl_with_stdlib_prefix() {
    assert_eq!(run_ok(r#"
        import com.example.util.*;
        int main() {
            return 7;
        }
    "#), 7);
}

/// `package` + plusieurs `import` — le cas complet de example2.mini.
#[test]
fn regression_package_and_imports_with_stdlib_prefix() {
    assert_eq!(run_ok(r#"
        package com.example.zoo;
        import com.example.util.*;
        import com.example.math.MathHelper;
        int main() {
            return 0;
        }
    "#), 0);
}

/// Le code qui suit le package/import fonctionne correctement.
#[test]
fn regression_package_import_code_executes() {
    let (ret, lines) = run_source_with_output(r#"
        package com.example.test;
        import com.example.util.*;
        int main() {
            print("ok");
            return 1;
        }
    "#).expect("run failed");
    assert_eq!(ret, 1);
    assert_eq!(lines, vec!["ok"]);
}
