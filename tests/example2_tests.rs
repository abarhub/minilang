//! Test de l'exemple example2.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example2() -> (i64, Vec<String>) {
    let src = include_str!("../examples/example2.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example2_returns_zero() {
    let (ret, _) = run_example2();
    assert_eq!(ret, 0);
}

#[test]
fn example2_output_lines() {
    let (_, lines) = run_example2();

    // -- Affiche les lignes réelles pour faciliter le diagnostic
    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Animal a
        "Animal: Félix | age: 4",
        // Dog d (3 tricks)
        "Chien: Rex | race: Labrador | tours: 3",
        "Rex aboie : Woof!",
        // GuideDog g
        "Chien guide: Buddy | race: Golden | proprio: Alice",
        "Buddy aboie : Woof!",
        // Héritage de méthodes
        "Age de Rex dans 5 ans: 8",
        "Score de Rex: 7.5",
        // Pair
        "Pair( age de Buddy , 5 )",
        // if/else
        "Mention : Bien",
        // MathUtils
        "6! = 720",
        "6^4 = 1296",
        // printFib(8)
        "fib[ 0 ] = 0",
        "fib[ 1 ] = 1",
        "fib[ 2 ] = 1",
        "fib[ 3 ] = 2",
        "fib[ 4 ] = 3",
        "fib[ 5 ] = 5",
        "fib[ 6 ] = 8",
        "fib[ 7 ] = 13",
        // demoOps
        "17 % 5 = 2",
        "2.0 ** 10.0 = 1024",
        "Hello, World!",
        "(3+4)*2 - 1/2 = 13.5",
        // for + break/continue (pairs 0..8 exclu)
        "Pairs de 0 à 10 :",
        "  0",
        "  2",
        "  4",
        "  6",
        // Collatz(100)
        "Collatz(100) : 25 étapes",
    ];

    assert_eq!(lines.len(), expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(), lines.len(),
        lines.iter().enumerate().map(|(i,l)| format!("[{}] {:?}", i, l)).collect::<Vec<_>>().join("\n"));

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
