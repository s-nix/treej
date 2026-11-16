//! Nested and flat JSON output: schema validation and exact content.

mod common;

use common::*;
use serde_json::{Value, json};

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("invalid JSON ({e}):\n{text}"))
}

/// Checks every folder's `modified` looks like a timestamp, then replaces it
/// with "DIR" (folder times change as fixtures are built).
fn normalize_folder_times(v: &mut Value) {
    match v {
        Value::Array(items) => items.iter_mut().for_each(normalize_folder_times),
        Value::Object(map) => {
            if map.get("type") == Some(&json!("folder")) {
                let t = map["modified"]
                    .as_str()
                    .expect("folder has no modified time");
                assert!(
                    t.len() == 20 && t.ends_with('Z') && t.as_bytes()[10] == b'T',
                    "bad timestamp {t}"
                );
                map.insert("modified".into(), json!("DIR"));
            }
            if let Some(children) = map.get_mut("children") {
                normalize_folder_times(children);
            }
        }
        _ => {}
    }
}

#[test]
fn nested_exact_content() {
    let (_tmp, root) = fixture();
    let text = ok([root.as_os_str(), "-f".as_ref(), "--json".as_ref()]);
    assert!(
        text.starts_with("{\n  \"name\": \"project\",\n  \"path\": "),
        "not pretty-printed with 2 spaces:\n{text}"
    );
    let mut v = parse(&text);
    assert_valid("nested.schema.json", &v);
    normalize_folder_times(&mut v);

    let file = |name: &str, size: u64| json!({ "name": name, "type": "file", "size": size, "modified": FIXED_TIME });
    let folder = |name: &str, children: Vec<Value>| json!({ "name": name, "type": "folder", "modified": "DIR", "children": children });
    let expected = json!({
        "name": "project",
        "path": abs(&root),
        "type": "folder",
        "modified": "DIR",
        "children": [
            file("A.txt", 5),
            file("b.txt", 2),
            folder("alpha", vec![
                file("one.txt", 1),
                folder("sub", vec![file("deep.txt", 4)]),
            ]),
            folder("beta", vec![file("two.txt", 2)]),
            folder("Zeta", vec![]),
        ]
    });
    assert_eq!(v, expected);
}

#[test]
fn nested_without_files_switch_has_only_folders() {
    let (_tmp, root) = fixture();
    let mut v = parse(&ok([root.as_os_str(), "-j".as_ref()]));
    assert_valid("nested.schema.json", &v);
    normalize_folder_times(&mut v);
    let folder = |name: &str, children: Vec<Value>| json!({ "name": name, "type": "folder", "modified": "DIR", "children": children });
    assert_eq!(
        v["children"],
        json!([
            folder("alpha", vec![folder("sub", vec![])]),
            folder("beta", vec![]),
            folder("Zeta", vec![]),
        ])
    );
}

#[test]
fn flat_exact_content() {
    let (_tmp, root) = fixture();
    let text = ok([root.as_os_str(), "-f".as_ref(), "--json=flat".as_ref()]);
    assert!(
        text.starts_with("[\n  {\n    \"path\": \"A.txt\",\n"),
        "not pretty-printed with 2 spaces:\n{text}"
    );
    assert!(text.ends_with("}\n]\n"));
    let mut v = parse(&text);
    assert_valid("flat.schema.json", &v);
    normalize_folder_times(&mut v);

    let file = |path: &str, size: u64| json!({ "path": path, "type": "file", "size": size, "modified": FIXED_TIME });
    let folder = |path: &str| json!({ "path": path, "type": "folder", "modified": "DIR" });
    assert_eq!(
        v,
        json!([
            file("A.txt", 5),
            file("b.txt", 2),
            folder("alpha"),
            file("alpha/one.txt", 1),
            folder("alpha/sub"),
            file("alpha/sub/deep.txt", 4),
            folder("beta"),
            file("beta/two.txt", 2),
            folder("Zeta"),
        ])
    );
}

#[test]
fn flat_without_files_switch() {
    let (_tmp, root) = fixture();
    let v = parse(&ok([root.as_os_str(), "--json=flat".as_ref()]));
    assert_valid("flat.schema.json", &v);
    let paths: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["alpha", "alpha/sub", "beta", "Zeta"]);
}

#[test]
fn flat_empty_root_is_empty_array() {
    let (_tmp, root) = empty_root();
    assert_eq!(ok([root.as_os_str(), "--json=flat".as_ref()]), "[]\n");
    let v = parse(&ok([root.as_os_str(), "--json".as_ref()]));
    assert_valid("nested.schema.json", &v);
    assert_eq!(v["children"], json!([]));
}

#[test]
fn output_file() {
    let (tmp, root) = fixture();
    let target = tmp.path().join("out.json");
    let run = treej([
        root.as_os_str(),
        "-f".as_ref(),
        "-j".as_ref(),
        "-o".as_ref(),
        target.as_os_str(),
    ]);
    assert_eq!(run.code, 0);
    assert_eq!(run.stdout, "");
    let v = parse(&std::fs::read_to_string(&target).unwrap());
    assert_valid("nested.schema.json", &v);
    assert_eq!(v["name"], "project");
}

#[test]
fn schemas_reject_bad_documents() {
    // The schemas must not accept everything.
    assert!(!is_valid("nested.schema.json", &json!([])));
    assert!(!is_valid(
        "nested.schema.json",
        &json!({ "name": "x", "type": "folder" })
    ));
    assert!(!is_valid(
        "nested.schema.json",
        &json!({ "name": "x", "path": "/x", "type": "folder",
                 "children": [{ "name": "f", "type": "file", "children": [] }] })
    ));
    assert!(!is_valid(
        "nested.schema.json",
        &json!({ "name": "x", "path": "/x", "type": "folder",
                 "children": [{ "name": "d", "type": "folder", "size": 1 }] })
    ));
    assert!(!is_valid(
        "nested.schema.json",
        &json!({ "name": "x", "path": "/x", "type": "folder", "modified": "yesterday" })
    ));
    assert!(!is_valid("flat.schema.json", &json!({})));
    assert!(!is_valid(
        "flat.schema.json",
        &json!([{ "path": "a", "type": "device" }])
    ));
    assert!(!is_valid(
        "flat.schema.json",
        &json!([{ "path": "a", "type": "file", "extra": 1 }])
    ));
    assert!(is_valid(
        "flat.schema.json",
        &json!([{ "path": "a/b", "type": "symlink", "target": "../c" }])
    ));
}
