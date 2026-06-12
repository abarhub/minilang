//! Test de l'exemple example_di.mini — injection de dépendances.
//! service class + inject : câblage par constructeur, résolution d'interface,
//! singletons partagés.

use mini_parser::interpreter::run_source_with_output;
use mini_parser::typechecker::check_source;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example_di.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example_di_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

#[test]
fn example_di_output_lines() {
    let (_, lines) = run_example();
    let expected: &[&str] = &[
        "[LOG] recherche utilisateur alice",
        "bonjour alice",
        "[LOG] recherche utilisateur inconnu",
        "bonjour inconnu",
        "appels : 3",
    ];
    assert_eq!(lines, expected);
}
