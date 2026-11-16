//! Shared helpers for the integration tests: running the binary and building
//! fixture folders in a temp dir.
#![allow(dead_code)]

use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use tempfile::TempDir;

/// 2026-11-10T14:22:00Z, applied to fixture files.
pub const FIXED_SECS: u64 = 1_794_320_520;
pub const FIXED_TIME: &str = "2026-11-10T14:22:00Z";

pub struct Run {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

/// Runs treej with the given arguments. Output must be valid UTF-8.
pub fn treej<I, S>(args: I) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::new(env!("CARGO_BIN_EXE_treej"))
        .args(args)
        .output()
        .expect("failed to run treej");
    Run {
        stdout: String::from_utf8(out.stdout).expect("stdout is not UTF-8"),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().expect("treej was killed by a signal"),
    }
}

/// Runs treej and expects exit code 0.
pub fn ok<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let run = treej(args);
    assert_eq!(run.code, 0, "treej failed; stderr:\n{}", run.stderr);
    run.stdout
}

/// Creates a file with the given contents and the fixed modification time.
pub fn file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
    let f = File::options().write(true).open(path).unwrap();
    f.set_modified(UNIX_EPOCH + Duration::from_secs(FIXED_SECS))
        .unwrap();
}

pub fn dir(path: impl AsRef<Path>) {
    fs::create_dir_all(path).unwrap();
}

/// The standard fixture, rooted at `<tmp>/project`:
///
/// ```text
/// project/
///   A.txt        (5 bytes)
///   b.txt        (2 bytes)
///   alpha/one.txt          (1 byte)
///   alpha/sub/deep.txt     (4 bytes)
///   beta/two.txt           (2 bytes)
///   Zeta/        (empty)
/// ```
pub fn fixture() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("project");
    file(root.join("A.txt"), "hello");
    file(root.join("b.txt"), "bb");
    file(root.join("alpha").join("one.txt"), "1");
    file(root.join("alpha").join("sub").join("deep.txt"), "deep");
    file(root.join("beta").join("two.txt"), "22");
    dir(root.join("Zeta"));
    (tmp, root)
}

/// An empty project folder in a temp dir.
pub fn empty_root() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("project");
    dir(&root);
    (tmp, root)
}

/// The root line treej prints for `path`.
pub fn abs(path: &Path) -> String {
    std::path::absolute(path).unwrap().display().to_string()
}

/// Reads `tests/golden/<name>`, normalizing line endings and filling in the
/// root path.
pub fn golden(name: &str, root: &Path) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .replace("\r\n", "\n")
        .replace("<ROOT>", &abs(root))
}

/// Loads a schema from `schema/`.
pub fn schema(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Asserts that `instance` is valid against the named schema.
pub fn assert_valid(schema_name: &str, instance: &serde_json::Value) {
    let validator = jsonschema::validator_for(&schema(schema_name)).expect("schema is invalid");
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "output does not match {schema_name}:\n{}\n{instance:#}",
        errors.join("\n")
    );
}

/// Whether `instance` is valid against the named schema.
pub fn is_valid(schema_name: &str, instance: &serde_json::Value) -> bool {
    jsonschema::validator_for(&schema(schema_name))
        .expect("schema is invalid")
        .is_valid(instance)
}
