// Expose les modules pour les tests d'intégration dans tests/
pub mod ast;
pub mod interpreter;
pub mod parser;
pub mod typechecker;

/// Bibliothèque standard embarquée — tous les fichiers lib/std/*.mini
pub const STDLIB: &str = concat!(
    include_str!("../lib/std/option.mini"), "\n",
    include_str!("../lib/std/result.mini"), "\n",
    include_str!("../lib/std/either.mini"), "\n",
    include_str!("../lib/std/pair.mini"),   "\n",
    include_str!("../lib/std/array.mini"),  "\n",
    include_str!("../lib/std/string.mini"),    "\n",
    include_str!("../lib/std/character.mini"), "\n",
    include_str!("../lib/std/boolean.mini"),   "\n",
    include_str!("../lib/std/integer.mini"),   "\n",
    include_str!("../lib/std/float.mini"),     "\n",
    include_str!("../lib/std/double.mini"),
);
