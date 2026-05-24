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
        "get(99) isNone=true",
        "contains(20)=true",
        "indexOf(20)=1",
        "find(30)=30",
        "after remove(0) size=2",
        "get(0) after remove=20",
        "arr.get(1)=15",
        "arr.get(99) isNone=true",
        "arr.indexOf(15)=1",
        "arr.find(25)=25",
        "set size=2",
        "add new=true",
        "add dup=false",
        "contains hello=true",
        "after remove size=1",
        "map size=2",
        "alice=99",
        "missing isNone=true",
        "containsKey bob=true",
        "after remove map size=1",
    ]);
}
