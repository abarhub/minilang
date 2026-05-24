//! Tests unitaires pour List<T>, Set<T>, Map<K,V>.

use mini_parser::interpreter::run_source;
use mini_parser::typechecker::check_source;

fn run_ok(src: &str) -> i64 {
    match run_source(src) {
        Ok(n)  => n,
        Err(e) => panic!("Runtime error:\n{}\n---\n{}", src, e),
    }
}

fn assert_tc_ok(src: &str) {
    if let Err(e) = check_source(src) {
        panic!("Typecheck failed:\n{}\n---\n{}", src, e.join("\n"));
    }
}

// ── ArrayList (via interface List) ────────────────────────────────────────────

#[test]
fn test_list_add_size() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3);
            return l.size();
        }
    "#), 3);
}

#[test]
fn test_list_get() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            return l.get(1);
        }
    "#), 20);
}

#[test]
fn test_list_set() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20);
            l.set(0, 99);
            return l.get(0);
        }
    "#), 99);
}

#[test]
fn test_list_contains_true() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(5);
            if (l.contains(5)) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_list_contains_false() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(5);
            if (l.contains(99)) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn test_list_remove() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            l.remove(1);
            return l.size();
        }
    "#), 2);
}

#[test]
fn test_list_is_empty() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            if (l.isEmpty()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_list_clear() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2);
            l.clear();
            return l.size();
        }
    "#), 0);
}

#[test]
fn test_list_string_elements() {
    assert_eq!(run_ok(r#"
        int main() {
            List<string> l = new ArrayList<string>();
            l.add("a"); l.add("b"); l.add("c");
            if (l.contains("b")) { return 1; }
            return 0;
        }
    "#), 1);
}

// ── HashSet (via interface Set) ───────────────────────────────────────────────

#[test]
fn test_set_add_no_dup() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(1); s.add(2); s.add(1);
            return s.size();
        }
    "#), 2);
}

#[test]
fn test_set_add_returns_true_new() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            bool r = s.add(42);
            if (r) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_set_add_returns_false_dup() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(42);
            bool r = s.add(42);
            if (r) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn test_set_contains() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<string> s = new HashSet<string>();
            s.add("hello");
            if (s.contains("hello")) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_set_remove() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(1); s.add(2);
            bool r = s.remove(1);
            if (r && s.size() == 1) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_set_is_empty() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            if (s.isEmpty()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_set_clear() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(1); s.add(2); s.add(3);
            s.clear();
            return s.size();
        }
    "#), 0);
}

// ── HashMap (via interface Map) ───────────────────────────────────────────────

#[test]
fn test_map_put_get() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("x", 42);
            return m.get("x");
        }
    "#), 42);
}

#[test]
fn test_map_put_updates() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("x", 1);
            m.put("x", 99);
            return m.get("x");
        }
    "#), 99);
}

#[test]
fn test_map_size() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1); m.put("b", 2); m.put("c", 3);
            return m.size();
        }
    "#), 3);
}

#[test]
fn test_map_contains_key() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("key", 1);
            if (m.containsKey("key")) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_map_contains_key_false() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1);
            if (m.containsKey("z")) { return 0; }
            return 1;
        }
    "#), 1);
}

#[test]
fn test_map_remove() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1); m.put("b", 2);
            bool r = m.remove("a");
            if (r && m.size() == 1) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_map_is_empty() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            if (m.isEmpty()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_map_clear() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1); m.put("b", 2);
            m.clear();
            return m.size();
        }
    "#), 0);
}

#[test]
fn test_map_get_missing_returns_null() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            int v = m.get("missing");
            return 0;
        }
    "#), 0);
}

// ── Typecheck ─────────────────────────────────────────────────────────────────

#[test]
fn test_tc_list_ok() {
    assert_tc_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1);
            int n = l.get(0);
            int s = l.size();
            return 0;
        }
    "#);
}

#[test]
fn test_tc_set_ok() {
    assert_tc_ok(r#"
        int main() {
            Set<string> s = new HashSet<string>();
            bool r = s.add("x");
            bool c = s.contains("x");
            return 0;
        }
    "#);
}

#[test]
fn test_tc_map_ok() {
    assert_tc_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("k", 1);
            bool c = m.containsKey("k");
            int s = m.size();
            return 0;
        }
    "#);
}

// ── Interface usage tests ─────────────────────────────────────────────────────

#[test]
fn test_list_interface_usage() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3);
            return l.size();
        }
    "#), 3);
}

#[test]
fn test_set_interface_usage() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<string> s = new HashSet<string>();
            s.add("x");
            if (s.contains("x")) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_map_interface_usage() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("k", 42);
            return m.get("k");
        }
    "#), 42);
}
