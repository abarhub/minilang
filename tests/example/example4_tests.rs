//! Test de l'exemple example4.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example4() -> (i64, Vec<String>) {
    let src = include_str!("../../examples/example4.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example4_returns_zero() {
    let (ret, _) = run_example4();
    assert_eq!(ret, 0);
}

#[test]
fn example4_output_lines() {
    let (_, lines) = run_example4();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Lambdas de base
        "=== Lambdas de base ===",
        "double(7)    = 14",       // 7 * 2
        "add(3, 4)    = 7",        // 3 + 4
        "square(9)    = 81",       // 9 * 9
        "get42()      = 42",       // () => 42
        // Paramètre unique sans parenthèses
        "\n=== Paramètre unique sans parenthèses ===",
        "negate(5)  = -5",
        "inc(41)    = 42",
        // Corps en bloc
        "\n=== Corps en bloc ===",
        "abs(-7)        = 7",
        "clamp(15,0,10) = 10",
        "clamp(-3,0,10) = 0",
        // Capture de variables
        "\n=== Capture de variables ===",
        "triple(7) = 21",          // 7 * factor(3)
        "add_base(42) = 142",      // 42 + base(100)
        // Appel inline
        "\n=== Appel inline ===",
        "(x=>x*x)(8)     = 64",
        "((a,b)=>a+b)(10,32) = 42",
        // Lambda passée à une méthode
        "\n=== Lambda passée à une méthode ===",
        "c.apply(x=>x*5) = 50",    // f(base=10) = 10 * 5
        "c.apply(x=>x+7) = 17",    // f(base=10) = 10 + 7
        // Lambda retournée
        "\n=== Lambda retournée ===",
        "adder(1)  = 16",           // 1 + base(10) + offset(5)
        "adder(20) = 35",           // 20 + base(10) + offset(5)
        // Higher-order
        "\n=== Higher-order ===",
        "times2(add1(20)) = 42",   // (20+1)*2
        "sum of squares 1..5 = 55", // 1+4+9+16+25
        // Enum + lambda
        "\n=== Enum + lambda ===",
        "d.code() = 3",             // Direction::East → 3
        // Réassignation de lambda
        "\n=== Réassignation de lambda ===",
        "transform(5) = 6",         // x => x + 1
        "après réassignation transform(5) = 50", // x => x * 10
    ];

    assert_eq!(lines.len(), expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(), lines.len(),
        lines.iter().enumerate().map(|(i,l)| format!("[{}] {:?}", i, l)).collect::<Vec<_>>().join("\n"));

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
