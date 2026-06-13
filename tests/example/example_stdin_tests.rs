//! Test de l'exemple example_stdin.mini — lecture de l'entrée standard.
//! On ne l'EXÉCUTE pas ici : StandardInput lirait le vrai stdin et bloquerait
//! (ou consommerait le stdin du test). On vérifie seulement qu'il typecheck.
//! Le comportement est couvert par tests/io_input_tests.rs via StringInput,
//! et vérifié manuellement par pipe (voir l'en-tête de l'exemple).

use mini_parser::typechecker::check_source;

#[test]
fn example_stdin_typechecks() {
    let src = include_str!("../../examples/example_stdin.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
}
