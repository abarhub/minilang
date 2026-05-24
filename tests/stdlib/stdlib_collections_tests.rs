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
fn test_list_get_some() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            Option<int> opt = l.get(1);
            if (opt.isSome()) { return opt.get(); }
            return -1;
        }
    "#), 20);
}

#[test]
fn test_list_get_none() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10);
            Option<int> opt = l.get(99);
            if (opt.isNone()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_list_set_valid() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20);
            bool ok = l.set(0, () => 99);
            if (ok) { return l.get(0).get(); }
            return -1;
        }
    "#), 99);
}

#[test]
fn test_list_set_out_of_bounds() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10);
            bool ok = l.set(99, () => 42);
            if (ok) { return 0; }
            return 1;
        }
    "#), 1);
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
fn test_list_index_of_found() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            Option<int> idx = l.indexOf(20);
            if (idx.isSome()) { return idx.get(); }
            return -1;
        }
    "#), 1);
}

#[test]
fn test_list_index_of_not_found() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20);
            Option<int> idx = l.indexOf(99);
            if (idx.isNone()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_list_find_found() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            Option<int> found = l.find(20);
            if (found.isSome()) { return found.get(); }
            return -1;
        }
    "#), 20);
}

#[test]
fn test_list_find_not_found() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20);
            Option<int> found = l.find(99);
            if (found.isNone()) { return 1; }
            return 0;
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

#[test]
fn test_list_grow_resize() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            int i = 0;
            while (i < 20) {
                l.add(i);
                i = i + 1;
            }
            return l.size();
        }
    "#), 20);
}

// ── Array méthodes Option ─────────────────────────────────────────────────────

#[test]
fn test_array_get_option_some() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> opt = arr.get(1);
            if (opt.isSome()) { return opt.get(); }
            return -1;
        }
    "#), 20);
}

#[test]
fn test_array_get_option_none() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> opt = arr.get(99);
            if (opt.isNone()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_array_index_of_found() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> idx = arr.indexOf(20);
            if (idx.isSome()) { return idx.get(); }
            return -1;
        }
    "#), 1);
}

#[test]
fn test_array_index_of_not_found() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> idx = arr.indexOf(99);
            if (idx.isNone()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_array_find_found() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> found = arr.find(30);
            if (found.isSome()) { return found.get(); }
            return -1;
        }
    "#), 30);
}

#[test]
fn test_array_find_not_found() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{10, 20, 30};
            Option<int> found = arr.find(99);
            if (found.isNone()) { return 1; }
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
            return m.get("x").get();
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
            return m.get("x").get();
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
fn test_map_get_missing_returns_none() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            Option<int> opt = m.get("missing");
            if (opt.isNone()) { return 1; }
            return 0;
        }
    "#), 1);
}

#[test]
fn test_map_get_some() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("key", 7);
            Option<int> opt = m.get("key");
            if (opt.isSome()) { return opt.get(); }
            return -1;
        }
    "#), 7);
}

// ── Typecheck ─────────────────────────────────────────────────────────────────

#[test]
fn test_tc_list_ok() {
    assert_tc_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1);
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
            return m.get("k").get();
        }
    "#), 42);
}

// ── forEach ───────────────────────────────────────────────────────────────────

#[test]
fn test_list_for_each() {
    // Les lambdas capturent les int par valeur ; on utilise un tableau (Rc partagé)
    // comme boîte mutable pour accumuler le résultat.
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3);
            int[] box = new int[]{0};
            l.forEach((x) => { box[0] = box[0] + x; });
            return box[0];
        }
    "#), 6);
}

#[test]
fn test_set_for_each() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(10); s.add(20); s.add(30);
            int[] box = new int[]{0};
            s.forEach((x) => { box[0] = box[0] + x; });
            return box[0];
        }
    "#), 60);
}

#[test]
fn test_map_for_each() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1); m.put("b", 2); m.put("c", 3);
            int[] box = new int[]{0};
            m.forEach((k, v) => { box[0] = box[0] + v; });
            return box[0];
        }
    "#), 6);
}

#[test]
fn test_array_for_each() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{5, 10, 15};
            int[] box = new int[]{0};
            arr.forEach((x) => { box[0] = box[0] + x; });
            return box[0];
        }
    "#), 30);
}

// ── Iterator ──────────────────────────────────────────────────────────────────

#[test]
fn test_list_iterator() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(4); l.add(5); l.add(6);
            Iterator<int> it = l.iterator();
            int sum = 0;
            Option<int> nxt = it.next();
            while (nxt.isSome()) {
                sum = sum + nxt.get();
                nxt = it.next();
            }
            return sum;
        }
    "#), 15);
}

#[test]
fn test_set_iterator() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(100); s.add(200);
            Iterator<int> it = s.iterator();
            int count = 0;
            Option<int> nxt = it.next();
            while (nxt.isSome()) {
                count = count + 1;
                nxt = it.next();
            }
            return count;
        }
    "#), 2);
}

// ── map.entries() ─────────────────────────────────────────────────────────────

#[test]
fn test_map_entries() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("x", 10); m.put("y", 20);
            ArrayList<Pair<string, int>> entries = m.entries();
            return entries.size();
        }
    "#), 2);
}

// ── for-in ────────────────────────────────────────────────────────────────────

#[test]
fn test_for_in_list() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3);
            int sum = 0;
            for (int x in l) {
                sum = sum + x;
            }
            return sum;
        }
    "#), 6);
}

#[test]
fn test_for_in_set() {
    assert_eq!(run_ok(r#"
        int main() {
            Set<int> s = new HashSet<int>();
            s.add(5); s.add(10); s.add(15);
            int sum = 0;
            for (int x in s) {
                sum = sum + x;
            }
            return sum;
        }
    "#), 30);
}

#[test]
fn test_for_in_array() {
    assert_eq!(run_ok(r#"
        int main() {
            int[] arr = new int[]{7, 8, 9};
            int sum = 0;
            for (int x in arr) {
                sum = sum + x;
            }
            return sum;
        }
    "#), 24);
}

#[test]
fn test_for_in_map_keys() {
    assert_eq!(run_ok(r#"
        int main() {
            Map<string, int> m = new HashMap<string, int>();
            m.put("a", 1); m.put("b", 2); m.put("c", 3);
            int count = 0;
            for (string k in m.keys()) {
                count = count + 1;
            }
            return count;
        }
    "#), 3);
}

// ── Iterator.forEach ──────────────────────────────────────────────────────────

#[test]
fn test_iterator_for_each_list() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(10); l.add(20); l.add(30);
            Iterator<int> it = l.iterator();
            int[] box = new int[]{0};
            it.forEach((x) => { box[0] = box[0] + x; });
            return box[0];
        }
    "#), 60);
}

#[test]
fn test_iterator_for_each_partial() {
    // Avancer l'itérateur manuellement, puis forEach consomme le reste.
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3); l.add(4);
            Iterator<int> it = l.iterator();
            it.next();                            // consomme 1
            int[] box = new int[]{0};
            it.forEach((x) => { box[0] = box[0] + x; }); // 2+3+4
            return box[0];
        }
    "#), 9);
}

#[test]
fn test_for_in_break() {
    assert_eq!(run_ok(r#"
        int main() {
            List<int> l = new ArrayList<int>();
            l.add(1); l.add(2); l.add(3); l.add(4); l.add(5);
            int sum = 0;
            for (int x in l) {
                if (x == 3) { break; }
                sum = sum + x;
            }
            return sum;
        }
    "#), 3);
}
