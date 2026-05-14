//! Test de l'exemple example5.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example5() -> (i64, Vec<String>) {
    let src = include_str!("../examples/example5.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

#[test]
fn example5_returns_zero() {
    let (ret, _) = run_example5();
    assert_eq!(ret, 0);
}

#[test]
fn example5_output_lines() {
    let (_, lines) = run_example5();

    for (i, l) in lines.iter().enumerate() {
        eprintln!("[{}] {:?}", i, l);
    }

    let expected: &[&str] = &[
        // Lambdas typées de base
        "=== Lambdas typées ===",
        "double(7)      = 14",
        "square(9)      = 81",
        "inc(41)        = 42",
        "add(10, 32)    = 42",
        "mul(6, 7)      = 42",
        "isPositive(5)  = true",
        "isPositive(-3) = false",
        // Corps en bloc
        "\n=== Corps en bloc ===",
        "abs(-7)        = 7",
        "abs(3)         = 3",
        "clamp(15,0,10) = 10",
        "clamp(-3,0,10) = 0",
        // Alias de types
        "\n=== Alias de types ===",
        "triple(14) = 42",             // 14 * 3
        // Capture lexicale
        "\n=== Capture de variables ===",
        "times_factor(8) = 40",        // 8 * factor(5)
        // Lambda passée en argument
        "\n=== Lambda passée en argument ===",
        "c.apply(x=>x*4)  = 40",       // f(base=10) = 10 * 4
        "c.apply(x=>x+32) = 42",       // f(base=10) = 10 + 32
        // Lambda retournée
        "\n=== Lambda retournée ===",
        "adder(1)  = 16",              // 1 + base(10) + offset(5)
        "adder(20) = 35",              // 20 + base(10) + offset(5)
        // Appel inline
        "\n=== Appel inline (x => x)(args) ===",
        "(x=>x*x)(8)         = 64",
        "((a,b)=>a+b)(10,32) = 42",
        // Composition
        "\n=== Composition ===",
        "times2(add1(20)) = 42",       // (20+1)*2
        // Enum → lambda typée
        "\n=== Enum → lambda typée ===",
        "fadd(3, 4) = 7",
        "fmul(6, 7) = 42",
        // Higher-order
        "\n=== Higher-order ===",
        "somme des doubles 1..10 = 110", // 2+4+6+8+10+12+14+16+18+20
        // Lambda récursive
        "\n=== Lambda récursive ===",
        "factorial(7) = 5040",
    ];

    assert_eq!(lines.len(), expected.len(),
        "Nombre de lignes attendu : {}, obtenu : {}\nLignes réelles :\n{}",
        expected.len(), lines.len(),
        lines.iter().enumerate().map(|(i,l)| format!("[{}] {:?}", i, l)).collect::<Vec<_>>().join("\n"));

    for (i, (got, exp)) in lines.iter().zip(expected.iter()).enumerate() {
        assert_eq!(got, exp, "Ligne {} incorrecte", i);
    }
}
