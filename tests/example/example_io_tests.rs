//! Test de l'exemple example_io.mini — système d'entrée/sortie.
//! Les écritures passent par StandardOutput (natif, vers le vrai stdout) donc
//! ne sont pas capturées par run_source_with_output ; on vérifie que l'exemple
//! typecheck et s'exécute jusqu'au bout (code 0). Les assertions détaillées sur
//! le contenu sont dans tests/io_tests.rs via StringOutput.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;

#[test]
fn example_io_typechecks_and_runs() {
    let src = include_str!("../../examples/example_io.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
    match run_source(src) {
        Ok(code) => assert_eq!(code, 0),
        Err(e) => panic!("Erreur d'exécution :\n{}", e),
    }
}
