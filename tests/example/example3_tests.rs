//! Test de l'exemple example3.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example3() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example3.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example3_returns_zero() {
    let (ret, _) = run_example3();
    assert_eq!(ret, 0);
}

#[test]
fn example3_output_lines() {
    let (_, lines) = run_example3();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Directions
        "=== Directions ===",
        "Direction : Nord",
        "Opposée   : Sud",
        // Walker  (\n dans le littéral → vrai saut de ligne dans la chaîne)
        "\n=== Walker ===",
        "Alice est en ( 4 , 3 )",
        "Distance de l'origine : 7",
        // Formes
        "\n=== Formes ===",
        "Aire cercle    : 78.53975",   // 3.14159 * 5.0 * 5.0
        "Aire rectangle : 24",          // 4.0 * 6.0
        "Aire triangle  : 12",          // 0.5 * 3.0 * 8.0
        // Pattern matching
        "\n=== Pattern matching ===",
        "C'est un cercle, rayon = 10",
        // MathResult
        "\n=== MathResult ===",
        "10 / 2 = 5",
        "Erreur : Division par zéro",
        "r1.isOk() = true",
        "r2.isOk() = false",
        "r1.unwrap() = 5",
        // Wildcard
        "\n=== Wildcard ===",
        "Pas au nord (wildcard)",
    ];

    assert_eq!(lines.len(), expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(), lines.len(),
        lines.iter().enumerate().map(|(i,l)| format!("[{}] {:?}", i, l)).collect::<Vec<_>>().join("\n"));

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
