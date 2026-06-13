//! Test de l'exemple example_files_config/app.mini — racines fichiers configurées.
//! On ne fait que typechecker : l'exécution dépend du minilang.toml (racines)
//! et touche le disque. Le comportement des racines est couvert par
//! tests/file_roots_tests.rs, et l'exemple est vérifié manuellement via le CLI.

use mini_parser::typechecker::check_source;

#[test]
fn example_files_config_typechecks() {
    let src = include_str!("../../examples/example_files_config/app.mini");
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}", e.join("\n"));
    }
}
