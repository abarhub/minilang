// Expose les modules pour les tests d'intégration dans tests/
pub mod ast;
pub mod interpreter;
pub mod parser;
pub mod typechecker;

/// Bibliothèque standard embarquée — lib/std/lang/ et lib/std/collection/
pub const STDLIB: &str = concat!(
    // ── minilang.lang ──────────────────────────────────────────────────────────
    include_str!("../lib/std/lang/Object.mini"),    "\n",
    include_str!("../lib/std/lang/Option.mini"),    "\n",
    include_str!("../lib/std/lang/Result.mini"),    "\n",
    include_str!("../lib/std/lang/Either.mini"),    "\n",
    include_str!("../lib/std/lang/Pair.mini"),      "\n",
    include_str!("../lib/std/lang/Array.mini"),     "\n",
    include_str!("../lib/std/lang/String.mini"),    "\n",
    include_str!("../lib/std/lang/Character.mini"), "\n",
    include_str!("../lib/std/lang/Boolean.mini"),   "\n",
    include_str!("../lib/std/lang/Integer.mini"),   "\n",
    include_str!("../lib/std/lang/Float.mini"),     "\n",
    include_str!("../lib/std/lang/Double.mini"),    "\n",
    // ── minilang.collection ────────────────────────────────────────────────────
    include_str!("../lib/std/collection/List.mini"),      "\n",
    include_str!("../lib/std/collection/Set.mini"),       "\n",
    include_str!("../lib/std/collection/Map.mini"),       "\n",
    include_str!("../lib/std/collection/ArrayList.mini"), "\n",
    include_str!("../lib/std/collection/HashSet.mini"),   "\n",
    include_str!("../lib/std/collection/HashMap.mini"),
);
