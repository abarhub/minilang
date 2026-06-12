// ─────────────────────────────────────────────────────────────────────────────
//  Runner de tests minilang
//
//  Exécute les fonctions déclarées `test` d'un programme. Chaque test tourne
//  dans un interpréteur fraîchement créé : environnement neuf et conteneur
//  d'injection réinitialisé (les singletons ne fuient pas d'un test à l'autre).
//  Un échec (assertion, panic, erreur runtime) n'arrête pas les tests suivants.
// ─────────────────────────────────────────────────────────────────────────────

use crate::ast::Program;
use crate::interpreter::Interpreter;

/// Résultat d'un test : `error` est None si le test a réussi.
#[derive(Debug)]
pub struct TestResult {
    pub name:  String,
    pub error: Option<String>,
}

impl TestResult {
    pub fn passed(&self) -> bool { self.error.is_none() }
}

/// Exécute toutes les fonctions `test` du programme, dans l'ordre de
/// déclaration. Retourne un résultat par test.
pub fn run_tests(program: &Program) -> Vec<TestResult> {
    program.funcs.iter()
        .filter(|f| f.is_test)
        .map(|f| {
            // Interpréteur neuf par test : isolation complète (singletons DI inclus)
            let mut interp = Interpreter::new(program);
            let error = interp.run_test(&f.name).err().map(|e| e.0);
            TestResult { name: f.name.clone(), error }
        })
        .collect()
}

/// Parse la source (stdlib incluse) et exécute ses tests.
/// API pratique pour les tests d'intégration Rust.
pub fn run_tests_source(src: &str) -> Result<Vec<TestResult>, String> {
    use chumsky::Parser;
    let full = format!("{}\n{}", crate::STDLIB, src);
    let program = crate::parser::program_parser()
        .parse(full.as_str())
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
    Ok(run_tests(&program))
}
