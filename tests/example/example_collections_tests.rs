//! Tests de l'exemple collections (List, Set, Map).

use mini_parser::interpreter::run_source_with_output;

fn run_with_output(src: &str) -> (i64, Vec<String>) {
    match run_source_with_output(src) {
        Ok(r)  => r,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

#[test]
fn example_collections_output() {
    let (_, lines) = run_with_output(include_str!("../../examples/example_collections.mini"));
    assert_eq!(lines, vec![
        "size=3",
        "get(1)=20",
        "contains(20)=true",
        "after remove(0) size=2",
        "get(0) after remove=20",
        "set size=2",
        "add new=true",
        "add dup=false",
        "contains hello=true",
        "after remove size=1",
        "map size=2",
        "alice=99",
        "containsKey bob=true",
        "after remove map size=1",
    ]);
}
