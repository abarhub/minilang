// Expose les modules pour les tests d'intégration dans tests/
pub mod ast;
pub mod config;
pub mod interpreter;
pub mod parser;
pub mod test_runner;
pub mod typechecker;

/// Bibliothèque standard embarquée — lib/std/minilang/lang/ et lib/std/minilang/collection/
pub const STDLIB: &str = concat!(
    // ── minilang.lang ──────────────────────────────────────────────────────────
    include_str!("../lib/std/minilang/lang/Object.mini"),    "\n",
    include_str!("../lib/std/minilang/lang/HashCode.mini"),  "\n",
    include_str!("../lib/std/minilang/lang/Option.mini"),    "\n",
    include_str!("../lib/std/minilang/lang/Result.mini"),    "\n",
    include_str!("../lib/std/minilang/lang/Either.mini"),    "\n",
    include_str!("../lib/std/minilang/lang/Pair.mini"),      "\n",
    include_str!("../lib/std/minilang/lang/RefArray.mini"),   "\n",
    include_str!("../lib/std/minilang/lang/Array.mini"),     "\n",
    include_str!("../lib/std/minilang/lang/String.mini"),    "\n",
    include_str!("../lib/std/minilang/lang/Character.mini"), "\n",
    include_str!("../lib/std/minilang/lang/Boolean.mini"),   "\n",
    include_str!("../lib/std/minilang/lang/Integer.mini"),   "\n",
    include_str!("../lib/std/minilang/lang/Byte.mini"),      "\n",
    include_str!("../lib/std/minilang/lang/Float.mini"),     "\n",
    include_str!("../lib/std/minilang/lang/Double.mini"),    "\n",
    // ── minilang.collection ────────────────────────────────────────────────────
    include_str!("../lib/std/minilang/collection/Iterator.mini"),         "\n",
    include_str!("../lib/std/minilang/collection/Iterable.mini"),         "\n",
    include_str!("../lib/std/minilang/collection/List.mini"),             "\n",
    include_str!("../lib/std/minilang/collection/Set.mini"),              "\n",
    include_str!("../lib/std/minilang/collection/Map.mini"),              "\n",
    include_str!("../lib/std/minilang/collection/ArrayListIterator.mini"),"\n",
    include_str!("../lib/std/minilang/collection/HashSetIterator.mini"),  "\n",
    include_str!("../lib/std/minilang/collection/ArrayList.mini"),        "\n",
    include_str!("../lib/std/minilang/collection/HashSet.mini"),          "\n",
    include_str!("../lib/std/minilang/collection/HashMap.mini"),          "\n",
    // ── minilang.io ────────────────────────────────────────────────────────────
    include_str!("../lib/std/minilang/io/Unit.mini"),           "\n",
    include_str!("../lib/std/minilang/io/IoError.mini"),        "\n",
    include_str!("../lib/std/minilang/io/Output.mini"),         "\n",
    include_str!("../lib/std/minilang/io/Flushable.mini"),      "\n",
    include_str!("../lib/std/minilang/io/BufferedOutput.mini"), "\n",
    include_str!("../lib/std/minilang/io/Input.mini"),          "\n",
    include_str!("../lib/std/minilang/io/StringOutput.mini"),   "\n",
    include_str!("../lib/std/minilang/io/StringInput.mini"),    "\n",
    include_str!("../lib/std/minilang/io/BufferedWriter.mini"), "\n",
    include_str!("../lib/std/minilang/io/Bytes.mini"),          "\n",
    include_str!("../lib/std/minilang/io/Files.mini"),          "\n",
    include_str!("../lib/std/minilang/io/ReadDir.mini"),        "\n",
    include_str!("../lib/std/minilang/io/ReadWriteDir.mini"),   "\n",
    include_str!("../lib/std/minilang/io/Directory.mini"),      "\n",
    include_str!("../lib/std/minilang/io/FileSystem.mini"),     "\n",
    // ── minilang.system ──────────────────────────────────────────────────────────
    include_str!("../lib/std/minilang/system/StandardOutput.mini"), "\n",
    include_str!("../lib/std/minilang/system/StandardError.mini"),  "\n",
    include_str!("../lib/std/minilang/system/StandardInput.mini"),
);
