// Expose les modules pour les tests d'intégration dans tests/
pub mod ast;
pub mod interpreter;
pub mod parser;
pub mod typechecker;

/// Bibliothèque standard embarquée (option.mini, …)
pub const STDLIB: &str = include_str!("../lib/std/option.mini");
