//! Test de l'exemple example_char.mini — type char, comparaisons, escape.

use mini_parser::interpreter::run_source_with_output;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example_char.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e) => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example_char_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

#[test]
fn example_char_output_lines() {
    let (_, lines) = run_example();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Littéraux char
        "A",
        "7",
        " ",
        // Séquences d'échappement reconnues
        "saut de ligne OK",
        "tabulation OK",
        "antislash OK",
        "apostrophe OK",
        "nul OK",
        // Comparaisons == et !=
        "c1 == c2 : vrai",
        "c1 != c3 : vrai",
        // Valeur par défaut '\0'
        "valeur par defaut = nul OK",
        // Réaffectation
        "reassignation OK",
        // CharBox
        "m",
        "CharBox.equals OK",
        // classify()
        "classify a = 1 OK",
        "classify e = 2 OK",
        "classify z = 0 (autre) OK",
        // Fin
        "fin de l'exemple char",
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
