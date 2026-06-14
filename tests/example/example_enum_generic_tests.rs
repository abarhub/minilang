//! Test de l'exemple example_enum_generic.mini — enums génériques Option/Result/Either/Pair.

use mini_parser::interpreter::run_source_with_output;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example_enum_generic.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e) => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example_enum_generic_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

#[test]
fn example_enum_generic_output_lines() {
    let (_, lines) = run_example();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Option<int>
        "Option::Some : 42",
        "Option::None OK",
        // Option<string>
        "message : bonjour",
        // Result<int, string>
        "10 / 2 = 5",
        "erreur capturee : division par zero",
        // Either<int, string>
        "Left : 99",
        "Right : texte",
        // Pair<int, bool>
        "Pair : 7 true",
        // find_first(5) → Some(10)
        "trouve : 10",
        // find_first(-1) → None
        "rien : OK",
    ];

    assert_eq!(
        lines.len(),
        expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(),
        lines.len(),
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| format!("[{}] {:?}", i, l))
            .collect::<Vec<_>>()
            .join("\n")
    );

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
