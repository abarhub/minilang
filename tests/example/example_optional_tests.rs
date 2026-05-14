//! Test de l'exemple example_optional.mini — T?, ??, ?. sur Option<T>.

use mini_parser::interpreter::run_source_with_output;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example_optional.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example_optional_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

#[test]
fn example_optional_output_lines() {
    let (_, lines) = run_example();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // isSome / isNone
        "maybe_int est present",
        "absent_int est absent",
        // .get()
        "valeur : 42",
        // ?? valeur par défaut
        "a = 42",    // Some(42) ?? 0  → 42
        "b = 99",    // None     ?? 99 → 99
        "s = bonjour",
        // ?. appel de méthode sûr
        "Bonjour Alice",   // u1?.greet() → Some("Bonjour Alice") ?? "personne"
        "personne",        // u2?.greet() → None ?? "personne"
        // ?. + ?? sur int
        "age1 = 31",  // u1?.nextAge() = Some(31) ?? 0
        "age2 = 0",   // u2?.nextAge() = None     ?? 0
        // match sur Option<User>
        "trouvé : Bob",   // findUser(2)
    ];

    assert_eq!(lines.len(), expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(), lines.len(),
        lines.iter().enumerate().map(|(i,l)| format!("[{}] {:?}", i, l)).collect::<Vec<_>>().join("\n"));

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
