//! Test de l'exemple example_files.mini — I/O fichiers en bloc.
//! On ne l'EXÉCUTE pas ici : il crée/supprime un fichier dans le répertoire
//! courant (pollution + collisions en tests parallèles). On vérifie qu'il
//! typecheck ; le comportement est couvert par tests/files_tests.rs (chemins
//! temporaires uniques + nettoyage), et vérifié manuellement via le CLI.

use mini_parser::typechecker::check_source;

#[test]
fn example_files_typechecks() {
    let src = include_str!("../../examples/example_files/app.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
}
