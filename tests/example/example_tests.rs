//! Test de l'exemple example.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e) => panic!("Erreur d'exécution :\n{}", e),
    }
}

// ── Valeur de retour ──────────────────────────────────────────────────────────

#[test]
fn example_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

// ── Toutes les lignes affichées ───────────────────────────────────────────────

#[test]
fn example_output_lines() {
    let (_, lines) = run_example();

    let expected: &[&str] = &[
        "Compteur : 42 | Actif : true",
        "Ratio : 3.14 | Précision : 2.718281828",
        "Hello, world!",
        "Nom de l'animal : ",   // a.name est "" par défaut
        "Nom :  | Age : 0",     // a.describe() avant setAge
        "Nom récupéré : ",      // a.getName() retourne ""
        "Zoo de  visite libre", // print interne de z.info()
        "Info zoo : ",          // print avec la valeur retournée par z.info()
    ];

    assert_eq!(
        lines.len(),
        expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes : {:#?}",
        expected.len(),
        lines.len(),
        lines
    );

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
