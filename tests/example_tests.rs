//! Test de l'exemple example.mini — vérifie le retour et les sorties print.

use mini_parser::interpreter::run_source_with_output;

fn run_example() -> (i64, Vec<String>) {
    let src = include_str!("../examples/example.mini");
    match run_source_with_output(src) {
        Ok(result) => result,
        Err(e)     => panic!("Erreur d'exécution :\n{}", e),
    }
}

// ── Valeur de retour ──────────────────────────────────────────────────────────

#[test]
fn example_returns_zero() {
    let (ret, _) = run_example();
    assert_eq!(ret, 0);
}

// ── Nombre de lignes affichées ────────────────────────────────────────────────

#[test]
fn example_output_line_count() {
    let (_, lines) = run_example();
    // 8 appels print au total (dont 1 depuis z.info() appelé dans un print)
    assert_eq!(lines.len(), 8);
}

// ── Contenu des lignes ────────────────────────────────────────────────────────

#[test]
fn example_line_compteur() {
    let (_, lines) = run_example();
    assert_eq!(lines[0], "Compteur : 42 | Actif : true");
}

#[test]
fn example_line_ratio() {
    let (_, lines) = run_example();
    // float 3.14, double 2.718281828 — affichage Rust f64 standard
    assert!(lines[1].starts_with("Ratio : 3.14 | Précision : 2.718281828"),
        "ligne inattendue : {:?}", lines[1]);
}

#[test]
fn example_line_message() {
    let (_, lines) = run_example();
    assert_eq!(lines[2], "Hello, world!");
}

#[test]
fn example_line_animal_name_field() {
    let (_, lines) = run_example();
    // a.name est une chaîne vide (valeur par défaut)
    assert_eq!(lines[3], "Nom de l'animal : ");
}

#[test]
fn example_line_describe() {
    let (_, lines) = run_example();
    // a.describe() → name="" age=0 (après setAge(5) le describe est avant)
    assert_eq!(lines[4], "Nom :  | Age : 0");
}

#[test]
fn example_line_get_name() {
    let (_, lines) = run_example();
    // a.getName() renvoie name=""
    assert_eq!(lines[5], "Nom récupéré : ");
}

#[test]
fn example_line_zoo_info_inner() {
    let (_, lines) = run_example();
    // z.info() fait un print("Zoo de", location, extra) ; location="" par défaut
    assert_eq!(lines[6], "Zoo de  visite libre");
}

#[test]
fn example_line_zoo_info_outer() {
    let (_, lines) = run_example();
    // print("Info zoo :", z.info(...)) ; z.info() retourne location=""
    assert_eq!(lines[7], "Info zoo : ");
}
