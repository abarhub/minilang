//! Test de l'exemple example_errors.mini — le typechecker doit rejeter ce source.

use mini_parser::typechecker::check_source;

fn errors_for_example() -> Vec<String> {
    let src = include_str!("../examples/example_errors.mini");
    match check_source(src) {
        Err(errs) => errs,
        Ok(())    => panic!("Le typechecker aurait dû détecter des erreurs dans example_errors.mini"),
    }
}

// ── Le typechecker doit rejeter le fichier ────────────────────────────────────

/// Le fichier contient des erreurs intentionnelles : le typechecker doit échouer.
#[test]
fn example_errors_is_rejected_by_typechecker() {
    let src = include_str!("../examples/example_errors.mini");
    assert!(check_source(src).is_err(),
        "Le typechecker aurait dû refuser example_errors.mini");
}

// ── Chaque erreur intentionnelle est bien détectée ────────────────────────────

/// ERREUR 1 : BadImpl n'implémente pas getValue() requis par Printable.
#[test]
fn example_errors_missing_interface_method() {
    let all = errors_for_example().join("\n");
    assert!(all.contains("getValue") || all.contains("BadImpl") || all.contains("Printable"),
        "Erreur d'interface non détectée dans :\n{}", all);
}

/// ERREUR 2 : `string s = 42` — affectation int → string.
#[test]
fn example_errors_type_mismatch_int_to_string() {
    let all = errors_for_example().join("\n");
    assert!(all.contains("int") || all.contains("string") || all.contains("incompatible"),
        "Erreur de type int→string non détectée dans :\n{}", all);
}

/// ERREUR 3 : `b.fly()` — méthode inexistante sur Base.
#[test]
fn example_errors_unknown_method() {
    let all = errors_for_example().join("\n");
    assert!(all.contains("fly") || all.contains("méthode") || all.contains("method"),
        "Méthode inconnue 'fly' non détectée dans :\n{}", all);
}

/// ERREUR 5 : `if (n)` — condition non booléenne.
#[test]
fn example_errors_non_bool_condition() {
    let all = errors_for_example().join("\n");
    assert!(all.contains("bool") || all.contains("condition") || all.contains("int"),
        "Condition non booléenne non détectée dans :\n{}", all);
}
