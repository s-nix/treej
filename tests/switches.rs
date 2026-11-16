//! Command-line parsing, exercised through the real binary.

mod common;

use common::*;
use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn short_and_long_unix_switches_agree() {
    let (_tmp, root) = fixture();
    let r = root.as_os_str();
    assert_eq!(ok([r, "-f".as_ref()]), ok([r, "--files".as_ref()]));
    assert_eq!(ok([r, "-a".as_ref()]), ok([r, "--ascii".as_ref()]));
    assert_eq!(
        ok([r, "-L".as_ref(), "1".as_ref()]),
        ok([r, "--depth".as_ref(), "1".as_ref()])
    );
    assert_eq!(ok([r, "-L1".as_ref()]), ok([r, "--depth=1".as_ref()]));
    assert_eq!(
        ok([r, "-fa".as_ref()]),
        ok([r, "-f".as_ref(), "-a".as_ref()])
    );
    // Options may come before the path.
    assert_eq!(ok(["-f".as_ref(), r]), ok([r, "-f".as_ref()]));
}

#[test]
fn json_switches() {
    let (_tmp, root) = fixture();
    let r = root.as_os_str();
    assert!(json(&ok([r, "-j".as_ref()])).is_object());
    assert!(json(&ok([r, "--json".as_ref()])).is_object());
    assert!(json(&ok([r, "--json=nested".as_ref()])).is_object());
    assert!(json(&ok([r, "--json=flat".as_ref()])).is_array());
}

#[test]
fn help() {
    for arg in ["-h", "--help"] {
        let run = treej([arg]);
        assert_eq!(run.code, 0);
        assert!(run.stdout.contains("Usage: treej [PATH] [options]"));
        assert!(run.stdout.contains("--json"));
    }
}

#[test]
fn unknown_switch_exits_1_with_usage() {
    for arg in ["--bogus", "-z", "-fz", "--files=yes", "--json=tree"] {
        let run = treej([arg]);
        assert_eq!(run.code, 1, "{arg} should fail");
        assert!(run.stderr.contains("error"), "{arg}: {}", run.stderr);
        assert!(run.stderr.contains("Usage:"), "{arg}: {}", run.stderr);
        assert_eq!(run.stdout, "");
    }
}

#[test]
fn bad_values_exit_1() {
    let (_tmp, root) = fixture();
    let r = root.as_os_str();
    for args in [
        vec![r, "-L".as_ref(), "0".as_ref()],
        vec![r, "--depth=x".as_ref()],
        vec![r, "-L".as_ref()],
        vec![r, "-o".as_ref()],
        vec![r, r],
    ] {
        assert_eq!(treej(&args).code, 1, "{args:?} should fail");
    }
}

#[test]
fn missing_or_non_folder_root_exits_1() {
    let (tmp, root) = fixture();
    let run = treej([tmp.path().join("nope")]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("cannot access"));
    assert_eq!(run.stdout, "");

    let run = treej([root.join("A.txt")]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("not a folder"));
}

#[test]
fn double_dash_ends_switches() {
    let (_tmp, root) = fixture();
    let odd = root.join("-odd");
    dir(&odd);
    let out = ok(["--".as_ref(), odd.as_os_str()]);
    assert!(out.starts_with(&abs(&odd)));
}

#[cfg(unix)]
#[test]
fn windows_switches_are_paths_on_unix() {
    // On Unix "/F" is an absolute path, not a switch.
    let run = treej(["/F-treej-test-does-not-exist"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("cannot access"), "{}", run.stderr);
    assert!(!run.stderr.contains("Usage:"));
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn windows_switches_are_case_insensitive() {
        let (_tmp, root) = fixture();
        let r = root.as_os_str();
        let upper = ok([r, "/F".as_ref(), "/A".as_ref()]);
        assert_eq!(upper, ok([r, "/f".as_ref(), "/a".as_ref()]));
        assert_eq!(upper, ok([r, "-f".as_ref(), "-a".as_ref()]));
        assert_eq!(ok([r, "--FILES".as_ref()]), ok([r, "--files".as_ref()]));
    }

    #[test]
    fn windows_json_switches() {
        let (_tmp, root) = fixture();
        let r = root.as_os_str();
        assert!(json(&ok([r, "/J".as_ref()])).is_object());
        assert!(json(&ok([r, "/J:nested".as_ref()])).is_object());
        assert!(json(&ok([r, "/J:flat".as_ref()])).is_array());
        assert!(json(&ok([r, "/j:FLAT".as_ref()])).is_array());
        assert_eq!(ok([r, "/J:flat".as_ref()]), ok([r, "--json=flat".as_ref()]));
    }

    #[test]
    fn windows_depth_and_output() {
        let (tmp, root) = fixture();
        let r = root.as_os_str();
        assert_eq!(
            ok([r, "/L:2".as_ref()]),
            ok([r, "-L".as_ref(), "2".as_ref()])
        );
        assert_eq!(ok([r, "/l:1".as_ref()]), ok([r, "--depth=1".as_ref()]));

        let target = tmp.path().join("tree.txt");
        let mut switch = std::ffi::OsString::from("/O:");
        switch.push(&target);
        let run = treej([r, "/F".as_ref(), switch.as_os_str()]);
        assert_eq!(run.code, 0);
        assert_eq!(run.stdout, "");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            golden("unicode_files.txt", &root)
        );
    }

    #[test]
    fn windows_help() {
        let run = treej(["/?"]);
        assert_eq!(run.code, 0);
        assert!(run.stdout.contains("/J[:format]"));
    }

    #[test]
    fn windows_unknown_switch_exits_1() {
        for arg in ["/Z", "/F:1", "/J:tree", "/L", "/L:0", "/O"] {
            let run = treej([arg]);
            assert_eq!(run.code, 1, "{arg} should fail");
            assert!(run.stderr.contains("Usage:"));
        }
    }
}
