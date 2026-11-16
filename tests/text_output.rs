//! Golden-file tests of the tree.com-style text output.

mod common;

use common::*;

#[test]
fn unicode_with_files() {
    let (_tmp, root) = fixture();
    assert_eq!(
        ok([root.as_os_str(), "-f".as_ref()]),
        golden("unicode_files.txt", &root)
    );
}

#[test]
fn unicode_folders_only() {
    let (_tmp, root) = fixture();
    assert_eq!(ok([&root]), golden("unicode_folders.txt", &root));
}

#[test]
fn ascii_with_files() {
    let (_tmp, root) = fixture();
    assert_eq!(
        ok([root.as_os_str(), "-f".as_ref(), "--ascii".as_ref()]),
        golden("ascii_files.txt", &root)
    );
}

#[test]
fn ascii_folders_only() {
    let (_tmp, root) = fixture();
    assert_eq!(
        ok([root.as_os_str(), "-a".as_ref()]),
        golden("ascii_folders.txt", &root)
    );
}

#[test]
fn no_subfolders_exist() {
    let (_tmp, root) = empty_root();
    file(root.join("a.txt"), "a");
    assert_eq!(ok([&root]), golden("no_subfolders.txt", &root));

    let (_tmp, root) = empty_root();
    assert_eq!(ok([&root]), golden("no_subfolders.txt", &root));
}

#[test]
fn files_only_root_with_files_switch() {
    let (_tmp, root) = empty_root();
    file(root.join("b.txt"), "b");
    file(root.join("a.txt"), "a");
    assert_eq!(
        ok([root.as_os_str(), "--files".as_ref()]),
        golden("files_only.txt", &root)
    );
}

#[test]
fn defaults_to_current_directory() {
    let (_tmp, root) = fixture();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_treej"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        golden("unicode_folders.txt", &root)
    );
}

#[test]
fn dot_and_dot_dot_in_root_are_resolved() {
    let (_tmp, root) = fixture();
    let messy = root
        .join("alpha")
        .join(".")
        .join("sub")
        .join("..")
        .join("..");
    assert_eq!(ok([&messy]), golden("unicode_folders.txt", &root));
    let v: serde_json::Value =
        serde_json::from_str(&ok([messy.as_os_str(), "-j".as_ref()])).unwrap();
    assert_eq!(v["name"], "project");
}

#[test]
fn sorting_is_case_insensitive_with_files_first() {
    let (_tmp, root) = empty_root();
    for name in ["c.txt", "B.txt", "a.txt"] {
        file(root.join(name), "");
    }
    for name in ["Ydir", "xdir", "Zdir"] {
        dir(root.join(name));
    }
    let out = ok([root.as_os_str(), "-fa".as_ref()]);
    let lines: Vec<&str> = out.lines().skip(1).collect();
    assert_eq!(
        lines,
        [
            "|   a.txt",
            "|   B.txt",
            "|   c.txt",
            "|",
            "+---xdir",
            "+---Ydir",
            "\\---Zdir"
        ]
    );
}

#[cfg(windows)]
#[test]
fn windows_switches_match_unix_switches() {
    let (_tmp, root) = fixture();
    assert_eq!(
        ok([root.as_os_str(), "/F".as_ref(), "/A".as_ref()]),
        golden("ascii_files.txt", &root)
    );
    assert_eq!(
        ok([root.as_os_str(), "/f".as_ref()]),
        golden("unicode_files.txt", &root)
    );
}
