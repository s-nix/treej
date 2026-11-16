//! Depth limits, hidden items, symlinks and junctions, unreadable folders,
//! long paths and odd file names.

mod common;

use std::path::Path;

use common::*;
use serde_json::Value;

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// Paths of all records in flat output.
fn flat_paths(root: &Path, extra: &[&str]) -> Vec<String> {
    let mut args = vec![root.as_os_str(), "--json=flat".as_ref()];
    args.extend(extra.iter().map(std::ffi::OsStr::new));
    let v = parse(&ok(args));
    assert_valid("flat.schema.json", &v);
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_owned())
        .collect()
}

fn find<'a>(records: &'a Value, path: &str) -> &'a Value {
    records
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
        .unwrap_or_else(|| panic!("no record for {path} in {records:#}"))
}

// ---------------------------------------------------------------- depth

#[test]
fn depth_limits_text() {
    let (_tmp, root) = fixture();
    let r = root.as_os_str();
    let one = ok([r, "-L".as_ref(), "1".as_ref()]);
    assert_eq!(
        one,
        format!("{}\n├───alpha\n├───beta\n└───Zeta\n", abs(&root))
    );
    let one_files = ok([r, "-f".as_ref(), "-L".as_ref(), "1".as_ref()]);
    assert!(one_files.contains("A.txt"));
    assert!(!one_files.contains("one.txt"));

    let two = ok([r, "-f".as_ref(), "-L".as_ref(), "2".as_ref()]);
    assert!(two.contains("one.txt") && two.contains("└───sub"));
    assert!(!two.contains("deep.txt"));
}

#[test]
fn depth_limits_json() {
    let (_tmp, root) = fixture();
    assert_eq!(
        flat_paths(&root, &["-f", "-L", "1"]),
        ["A.txt", "b.txt", "alpha", "beta", "Zeta"]
    );
    assert_eq!(
        flat_paths(&root, &["-L", "2"]),
        ["alpha", "alpha/sub", "beta", "Zeta"]
    );

    let v = parse(&ok([
        root.as_os_str(),
        "-j".as_ref(),
        "-L".as_ref(),
        "1".as_ref(),
    ]));
    assert_valid("nested.schema.json", &v);
    for child in v["children"].as_array().unwrap() {
        assert!(
            child.get("children").is_none(),
            "depth-limited folder should have no children: {child}"
        );
    }
}

// ---------------------------------------------------------------- hidden

/// Adds hidden items to `root` the platform's way and returns their names.
fn add_hidden(root: &Path) -> Vec<&'static str> {
    if cfg!(windows) {
        file(root.join("hidden.txt"), "h");
        file(root.join("system.txt"), "s");
        dir(root.join("hiddendir").join("inner"));
        for (flag, name) in [
            ("+h", "hidden.txt"),
            ("+s", "system.txt"),
            ("+h", "hiddendir"),
        ] {
            let status = std::process::Command::new("attrib")
                .arg(flag)
                .arg(root.join(name))
                .status()
                .expect("attrib failed to run");
            assert!(status.success());
        }
        vec!["hidden.txt", "system.txt", "hiddendir", "hiddendir/inner"]
    } else {
        file(root.join(".hidden.txt"), "h");
        dir(root.join(".hiddendir").join("inner"));
        vec![".hidden.txt", ".hiddendir", ".hiddendir/inner"]
    }
}

#[test]
fn hidden_items_are_filtered() {
    let (_tmp, root) = fixture();
    let hidden = add_hidden(&root);

    // Excluded by default, in text and JSON.
    let text = ok([root.as_os_str(), "-f".as_ref()]);
    assert_eq!(text, golden("unicode_files.txt", &root));
    let visible = flat_paths(&root, &["-f"]);
    for name in &hidden {
        assert!(
            !visible.contains(&name.to_string()),
            "{name} should be hidden"
        );
    }

    // Included with -H.
    let all = flat_paths(&root, &["-f", "-H"]);
    for name in &hidden {
        assert!(
            all.contains(&name.to_string()),
            "{name} missing with -H: {all:?}"
        );
    }
    let text = ok([root.as_os_str(), "-f".as_ref(), "--hidden".as_ref()]);
    assert!(text.contains("hiddendir"));
}

#[cfg(windows)]
#[test]
fn hidden_switch_windows_style() {
    let (_tmp, root) = fixture();
    add_hidden(&root);
    assert_eq!(
        ok([root.as_os_str(), "/F".as_ref(), "/H".as_ref()]),
        ok([root.as_os_str(), "-f".as_ref(), "-H".as_ref()])
    );
}

#[cfg(windows)]
#[test]
fn dotfiles_are_visible_on_windows() {
    let (_tmp, root) = fixture();
    file(root.join(".env"), "x");
    assert!(flat_paths(&root, &["-f"]).contains(&".env".to_string()));
}

// ---------------------------------------------------------------- links

/// Checks a link record: reported as a symlink, with a target, not entered.
fn assert_link_not_followed(root: &Path, link: &str, expect_target_contains: &str) {
    let v = parse(&ok([
        root.as_os_str(),
        "-f".as_ref(),
        "--json=flat".as_ref(),
    ]));
    assert_valid("flat.schema.json", &v);
    let rec = find(&v, link);
    assert_eq!(rec["type"], "symlink", "{rec}");
    assert!(rec.get("size").is_none());
    let target = rec["target"].as_str().expect("symlink has no target");
    assert!(
        target.contains(expect_target_contains),
        "target {target} should mention {expect_target_contains}"
    );
    let prefix = format!("{link}/");
    assert!(
        !v.as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"].as_str().unwrap().starts_with(&prefix)),
        "{link} was followed"
    );

    let nested = parse(&ok([root.as_os_str(), "-f".as_ref(), "-j".as_ref()]));
    assert_valid("nested.schema.json", &nested);
    let node = nested["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == link)
        .unwrap();
    assert_eq!(node["type"], "symlink");
    assert!(node.get("children").is_none());

    // Text mode lists the link like a folder, without its contents.
    let text = ok([root.as_os_str(), "-f".as_ref()]);
    let line = text
        .lines()
        .position(|l| l.ends_with(link))
        .unwrap_or_else(|| panic!("{link} missing from text output:\n{text}"));
    let next = text.lines().nth(line + 1).unwrap_or("");
    assert!(!next.contains("one.txt"), "{link} was followed:\n{text}");
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    use std::os::unix::fs::symlink;
    let (_tmp, root) = fixture();
    symlink(root.join("alpha"), root.join("link_dir")).unwrap();
    symlink("A.txt", root.join("link_file")).unwrap();
    symlink("missing", root.join("link_broken")).unwrap();
    // A loop would hang a walker that followed links.
    symlink(".", root.join("alpha").join("loop")).unwrap();

    assert_link_not_followed(&root, "link_dir", "alpha");

    let v = parse(&ok([
        root.as_os_str(),
        "-f".as_ref(),
        "--json=flat".as_ref(),
    ]));
    assert_eq!(
        find(&v, "link_file"),
        &serde_json::json!({ "path": "link_file", "type": "symlink", "target": "A.txt",
                 "modified": find(&v, "link_file")["modified"] })
    );
    assert_eq!(find(&v, "link_broken")["target"], "missing");
    assert_eq!(find(&v, "alpha/loop")["type"], "symlink");

    // Without -f, only links to folders are listed (with the folders).
    let folders = flat_paths(&root, &[]);
    assert!(folders.contains(&"link_dir".to_string()));
    assert!(!folders.contains(&"link_file".to_string()));
    assert!(!folders.contains(&"link_broken".to_string()));
}

#[cfg(windows)]
#[test]
fn junctions_are_not_followed() {
    let (_tmp, root) = fixture();
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(root.join("junction"))
        .arg(root.join("alpha"))
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed");
    assert_link_not_followed(&root, "junction", "alpha");
    assert!(flat_paths(&root, &[]).contains(&"junction".to_string()));
}

#[cfg(windows)]
#[test]
fn symlinks_are_not_followed_when_allowed() {
    let (_tmp, root) = fixture();
    // Creating symlinks needs Developer Mode or an elevated prompt.
    if std::os::windows::fs::symlink_dir(root.join("alpha"), root.join("link_dir")).is_err() {
        eprintln!("skipping: not allowed to create symlinks here");
        return;
    }
    std::os::windows::fs::symlink_file(root.join("A.txt"), root.join("link_file")).unwrap();
    assert_link_not_followed(&root, "link_dir", "alpha");
    let v = parse(&ok([
        root.as_os_str(),
        "-f".as_ref(),
        "--json=flat".as_ref(),
    ]));
    assert_eq!(find(&v, "link_file")["type"], "symlink");
    assert!(!flat_paths(&root, &[]).contains(&"link_file".to_string()));
}

// ---------------------------------------------------------------- errors

#[cfg(unix)]
#[test]
fn unreadable_folder_reports_error_and_exits_2() {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let (_tmp, root) = fixture();
    let locked = root.join("locked");
    file(locked.join("secret.txt"), "s");
    fs::set_permissions(&locked, Permissions::from_mode(0o000)).unwrap();
    // Restore permissions on the way out so the temp dir can be deleted.
    struct Restore<'a>(&'a Path);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.0, Permissions::from_mode(0o755));
        }
    }
    let _restore = Restore(&locked);
    if fs::read_dir(&locked).is_ok() {
        eprintln!("skipping: running with privileges that ignore permissions");
        return;
    }

    // Nested JSON.
    let run = treej([root.as_os_str(), "-f".as_ref(), "-j".as_ref()]);
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("warning") && run.stderr.contains("locked"));
    let v = parse(&run.stdout);
    assert_valid("nested.schema.json", &v);
    let node = v["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "locked")
        .unwrap();
    assert!(node["error"].as_str().unwrap().len() > 3);
    assert!(node.get("children").is_none());
    // The rest of the tree is still there.
    assert!(
        v["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["name"] == "Zeta")
    );

    // Flat JSON.
    let run = treej([root.as_os_str(), "-f".as_ref(), "--json=flat".as_ref()]);
    assert_eq!(run.code, 2);
    let v = parse(&run.stdout);
    assert_valid("flat.schema.json", &v);
    assert!(find(&v, "locked")["error"].is_string());
    assert!(!run.stdout.contains("secret.txt"));

    // Text: the folder is printed normally.
    let run = treej([root.as_os_str(), "-f".as_ref()]);
    assert_eq!(run.code, 2);
    assert!(run.stdout.contains("├───locked"));
    assert!(run.stderr.contains("locked"));

    // An unreadable root is fatal.
    let run = treej([&locked]);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
}

// ---------------------------------------------------------------- long paths

#[test]
fn paths_longer_than_260_characters() {
    let (_tmp, root) = empty_root();
    let segment = "a_rather_long_folder_name_0123456789";
    let mut deep = root.clone();
    for i in 0..10 {
        deep.push(format!("{segment}_{i}"));
    }
    file(deep.join("deepest.txt"), "x");
    assert!(deep.join("deepest.txt").as_os_str().len() > 300);

    let text = ok([root.as_os_str(), "-f".as_ref()]);
    assert!(text.contains("deepest.txt"), "{text}");
    assert!(text.contains(&format!("{segment}_9")));

    let paths = flat_paths(&root, &["-f"]);
    let last = paths.last().unwrap();
    assert!(last.ends_with("/deepest.txt"));
    assert!(last.len() > 300);

    // The deep folder itself works as a root, too.
    let run = treej([deep.as_os_str(), "-f".as_ref()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("deepest.txt"));
}

#[cfg(windows)]
#[test]
fn binary_declares_long_path_awareness() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_treej")).unwrap();
    let needle = b"longPathAware";
    assert!(
        exe.windows(needle.len()).any(|w| w == needle),
        "the embedded manifest should declare longPathAware"
    );
}

// ---------------------------------------------------------------- odd names

#[test]
fn unicode_names() {
    let (_tmp, root) = empty_root();
    file(root.join("naïve café ☕.txt"), "x");
    dir(root.join("日本語"));
    let text = ok([root.as_os_str(), "-f".as_ref()]);
    assert!(text.contains("naïve café ☕.txt"));
    assert!(text.contains("└───日本語"));
    assert_eq!(flat_paths(&root, &["-f"]), ["naïve café ☕.txt", "日本語"]);
}

#[cfg(unix)]
#[test]
fn non_utf8_names_are_converted_lossily() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let (_tmp, root) = empty_root();
    let name = OsStr::from_bytes(b"bad\xffname.txt");
    if std::fs::write(root.join(name), "x").is_err() {
        eprintln!("skipping: file system rejects non-UTF-8 names");
        return;
    }
    let text = ok([root.as_os_str(), "-f".as_ref()]);
    assert!(text.contains("bad\u{FFFD}name.txt"), "{text}");
    assert_eq!(flat_paths(&root, &["-f"]), ["bad\u{FFFD}name.txt"]);
}

#[cfg(windows)]
#[test]
fn non_unicode_names_are_converted_lossily() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let (_tmp, root) = empty_root();
    // "bad" + an unpaired surrogate + "name.txt"
    let mut wide: Vec<u16> = "bad".encode_utf16().collect();
    wide.push(0xD800);
    wide.extend("name.txt".encode_utf16());
    std::fs::write(root.join(OsString::from_wide(&wide)), "x").unwrap();
    let text = ok([root.as_os_str(), "-f".as_ref()]);
    assert!(text.contains("bad\u{FFFD}name.txt"), "{text}");
    assert_eq!(flat_paths(&root, &["-f"]), ["bad\u{FFFD}name.txt"]);
}
