//! Test de l'exemple example_byte.mini — type byte et conversions string<->byte[].
//! La sortie passe par StandardOutput (vers le vrai stdout, non capturé) ; on
//! vérifie le typecheck et l'exécution à 0. Le comportement détaillé est couvert
//! par tests/byte_tests.rs.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;

#[test]
fn example_byte_typechecks_and_runs() {
    let src = include_str!("../../examples/example_byte.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
    match run_source(src) {
        Ok(code) => assert_eq!(code, 0),
        Err(e) => panic!("Erreur d'exécution :\n{}", e),
    }
}
