//! Test de l'exemple example_file_capabilities.mini — accès fichiers par capacités.
//! On ne l'EXÉCUTE pas ici (il crée un répertoire temporaire) : on vérifie le
//! typecheck. Le comportement est couvert par tests/file_capabilities_tests.rs
//! (avec nettoyage) et vérifié manuellement via le CLI.

use mini_parser::typechecker::check_source;

#[test]
fn example_file_capabilities_typechecks() {
    let src = include_str!("../../examples/example_file_capabilities.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
}
