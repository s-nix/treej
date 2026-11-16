//! Hand-written command-line parser that accepts both Windows-style (`/F`)
//! and Unix-style (`-f`, `--files`) switches.
//!
//! Windows-style switches are only recognized when `windows` is true, because
//! on Unix a leading `/` starts an absolute path. On Windows, Windows-style
//! switches, their values and long Unix-style options are case-insensitive;
//! short Unix-style options stay case-sensitive everywhere (`-h` is help,
//! `-H` is hidden).

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

/// The two JSON layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonFormat {
    Nested,
    Flat,
}

/// Fully parsed options for a listing run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Root folder; `None` means the current directory.
    pub path: Option<PathBuf>,
    pub files: bool,
    pub ascii: bool,
    pub json: Option<JsonFormat>,
    pub hidden: bool,
    pub depth: Option<usize>,
    pub output: Option<PathBuf>,
}

/// What the program should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(Options),
    Help,
}

/// An invalid command line. The message does not include the usage text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, CliError> {
    Err(CliError(msg.into()))
}

/// Usage text for the current platform.
pub fn usage() -> &'static str {
    if cfg!(windows) {
        USAGE_WINDOWS
    } else {
        USAGE_UNIX
    }
}

const USAGE_WINDOWS: &str = "\
treej - display a folder tree like tree.com, with JSON export

Usage: treej [PATH] [options]

  PATH defaults to the current directory.

Options:
  /F            -f, --files              Include files (folders only by default)
  /A            -a, --ascii              Use ASCII instead of box-drawing characters
  /J[:format]   -j, --json[=format]      Output JSON; format is nested (default) or flat
  /H            -H, --hidden             Include hidden and system items
  /L:n          -L n, --depth n          Limit recursion to n levels
  /O:file       -o file, --output file   Write to a file instead of stdout
  /?            -h, --help               Show this help

Exit codes: 0 success, 1 invalid arguments or unreadable root,
            2 finished but some folders could not be read.
";

const USAGE_UNIX: &str = "\
treej - display a folder tree like tree.com, with JSON export

Usage: treej [PATH] [options]

  PATH defaults to the current directory.

Options:
  -f, --files              Include files (folders only by default)
  -a, --ascii              Use ASCII instead of box-drawing characters
  -j, --json[=format]      Output JSON; format is nested (default) or flat
  -H, --hidden             Include hidden items (dotfiles)
  -L n, --depth n          Limit recursion to n levels
  -o file, --output file   Write to a file instead of stdout
  -h, --help               Show this help

Exit codes: 0 success, 1 invalid arguments or unreadable root,
            2 finished but some folders could not be read.
";

/// Parses the arguments (without the program name). `windows` selects
/// Windows rules: `/X` switches and case-insensitive matching.
pub fn parse<I>(args: I, windows: bool) -> Result<Command, CliError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut opts = Options::default();
    let mut args = args.into_iter();
    let mut switches_done = false;

    while let Some(arg) = args.next() {
        let bytes = arg.as_encoded_bytes();
        if !switches_done {
            if bytes == b"--" {
                switches_done = true;
                continue;
            }
            if windows && bytes.first() == Some(&b'/') {
                if parse_windows_switch(&arg, windows, &mut opts)? {
                    return Ok(Command::Help);
                }
                continue;
            }
            if bytes.len() > 1 && bytes[0] == b'-' {
                let help = if bytes.starts_with(b"--") {
                    parse_long(&arg, windows, &mut args, &mut opts)?
                } else {
                    parse_short(&arg, &mut args, &mut opts)?
                };
                if help {
                    return Ok(Command::Help);
                }
                continue;
            }
        }
        if opts.path.is_some() {
            return err(format!(
                "unexpected extra argument '{}'",
                arg.to_string_lossy()
            ));
        }
        opts.path = Some(PathBuf::from(arg));
    }
    Ok(Command::Run(opts))
}

/// Splits `arg` (minus its first `skip` bytes) at the first `sep`, returning
/// the name as a string and the optional value as an `OsStr`.
fn split_switch(arg: &OsStr, skip: usize, sep: u8) -> Result<(String, Option<&OsStr>), CliError> {
    let bytes = &arg.as_encoded_bytes()[skip..];
    let (name, value) = match bytes.iter().position(|&b| b == sep) {
        Some(i) => (&bytes[..i], Some(&bytes[i + 1..])),
        None => (bytes, None),
    };
    let name = match std::str::from_utf8(name) {
        Ok(s) => s.to_owned(),
        Err(_) => return err(format!("unknown switch '{}'", arg.to_string_lossy())),
    };
    // SAFETY: `value` starts right after an ASCII separator inside an
    // `OsStr`'s encoded bytes and runs to its end, which the
    // `from_encoded_bytes_unchecked` contract allows.
    let value = value.map(|v| unsafe { OsStr::from_encoded_bytes_unchecked(v) });
    Ok((name, value))
}

fn eq(a: &str, b: &str, windows: bool) -> bool {
    if windows {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Parses `/X[:value]`. Returns true when help was requested.
fn parse_windows_switch(arg: &OsStr, windows: bool, opts: &mut Options) -> Result<bool, CliError> {
    let shown = arg.to_string_lossy();
    let (name, value) = split_switch(arg, 1, b':')?;
    let upper = name.to_ascii_uppercase();
    let no_value = |value: Option<&OsStr>| match value {
        Some(_) => err(format!("switch '/{name}' does not take a value")),
        None => Ok(()),
    };
    match upper.as_str() {
        "F" => {
            no_value(value)?;
            opts.files = true;
        }
        "A" => {
            no_value(value)?;
            opts.ascii = true;
        }
        "H" => {
            no_value(value)?;
            opts.hidden = true;
        }
        "?" => {
            no_value(value)?;
            return Ok(true);
        }
        "J" => opts.json = Some(json_format(value, &shown, windows)?),
        "L" => match value {
            Some(v) => opts.depth = Some(depth(v)?),
            None => return err("switch '/L' needs a depth, as in /L:2"),
        },
        "O" => match value {
            Some(v) if !v.is_empty() => opts.output = Some(PathBuf::from(v)),
            _ => return err("switch '/O' needs a file name, as in /O:tree.json"),
        },
        _ => return err(format!("unknown switch '{shown}'")),
    }
    Ok(false)
}

/// Parses `--name[=value]`, taking a following argument for options that need
/// a value. Returns true when help was requested.
fn parse_long<I>(
    arg: &OsStr,
    windows: bool,
    rest: &mut I,
    opts: &mut Options,
) -> Result<bool, CliError>
where
    I: Iterator<Item = OsString>,
{
    let shown = arg.to_string_lossy();
    let (name, value) = split_switch(arg, 2, b'=')?;
    let is = |n: &str| eq(&name, n, windows);
    let no_value = || match value {
        Some(_) => err(format!("option '--{name}' does not take a value")),
        None => Ok(()),
    };
    let mut take_value = || -> Result<OsString, CliError> {
        match value {
            Some(v) => Ok(v.to_owned()),
            None => rest
                .next()
                .ok_or_else(|| CliError(format!("option '--{name}' needs a value"))),
        }
    };

    if is("files") {
        no_value()?;
        opts.files = true;
    } else if is("ascii") {
        no_value()?;
        opts.ascii = true;
    } else if is("hidden") {
        no_value()?;
        opts.hidden = true;
    } else if is("help") {
        no_value()?;
        return Ok(true);
    } else if is("json") {
        opts.json = Some(json_format(value, &shown, windows)?);
    } else if is("depth") {
        opts.depth = Some(depth(&take_value()?)?);
    } else if is("output") {
        let v = take_value()?;
        if v.is_empty() {
            return err("option '--output' needs a file name");
        }
        opts.output = Some(PathBuf::from(v));
    } else {
        return err(format!("unknown option '{shown}'"));
    }
    Ok(false)
}

/// Parses a cluster of short options such as `-fa` or `-L2`. Returns true
/// when help was requested.
fn parse_short<I>(arg: &OsStr, rest: &mut I, opts: &mut Options) -> Result<bool, CliError>
where
    I: Iterator<Item = OsString>,
{
    let bytes = arg.as_encoded_bytes();
    let mut i = 1;
    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        match c {
            b'f' => opts.files = true,
            b'a' => opts.ascii = true,
            b'j' => opts.json = Some(JsonFormat::Nested),
            b'H' => opts.hidden = true,
            b'h' => return Ok(true),
            b'L' | b'o' => {
                let value = if i < bytes.len() {
                    // SAFETY: the split point follows an ASCII byte, which the
                    // `from_encoded_bytes_unchecked` contract allows.
                    let v = unsafe { OsStr::from_encoded_bytes_unchecked(&bytes[i..]) };
                    v.to_owned()
                } else {
                    rest.next()
                        .ok_or_else(|| CliError(format!("option '-{}' needs a value", c as char)))?
                };
                if c == b'L' {
                    opts.depth = Some(depth(&value)?);
                } else {
                    if value.is_empty() {
                        return err("option '-o' needs a file name");
                    }
                    opts.output = Some(PathBuf::from(value));
                }
                return Ok(false);
            }
            _ => {
                let shown = if c.is_ascii() {
                    format!("-{}", c as char)
                } else {
                    arg.to_string_lossy().into_owned()
                };
                return err(format!("unknown switch '{shown}'"));
            }
        }
    }
    Ok(false)
}

fn json_format(value: Option<&OsStr>, shown: &str, windows: bool) -> Result<JsonFormat, CliError> {
    let Some(value) = value else {
        return Ok(JsonFormat::Nested);
    };
    let v = value.to_string_lossy();
    if eq(&v, "nested", windows) {
        Ok(JsonFormat::Nested)
    } else if eq(&v, "flat", windows) {
        Ok(JsonFormat::Flat)
    } else {
        err(format!(
            "invalid JSON format in '{shown}': expected 'nested' or 'flat'"
        ))
    }
}

fn depth(value: &OsStr) -> Result<usize, CliError> {
    let s = value.to_string_lossy();
    match s.trim().parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => err(format!(
            "invalid depth '{s}': expected a positive whole number"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], windows: bool) -> Result<Command, CliError> {
        parse(args.iter().map(OsString::from), windows)
    }

    fn opts(args: &[&str], windows: bool) -> Options {
        match run(args, windows) {
            Ok(Command::Run(o)) => o,
            other => panic!("expected options for {args:?}, got {other:?}"),
        }
    }

    #[test]
    fn defaults() {
        assert_eq!(opts(&[], true), Options::default());
        assert_eq!(opts(&[], false), Options::default());
    }

    #[test]
    fn windows_switches() {
        let o = opts(
            &["C:\\x", "/F", "/a", "/J:FLAT", "/h", "/L:2", "/o:out.json"],
            true,
        );
        assert_eq!(o.path, Some(PathBuf::from("C:\\x")));
        assert!(o.files && o.ascii && o.hidden);
        assert_eq!(o.json, Some(JsonFormat::Flat));
        assert_eq!(o.depth, Some(2));
        assert_eq!(o.output, Some(PathBuf::from("out.json")));
        assert_eq!(opts(&["/J"], true).json, Some(JsonFormat::Nested));
        assert_eq!(opts(&["/j:nested"], true).json, Some(JsonFormat::Nested));
        assert_eq!(run(&["/?"], true), Ok(Command::Help));
    }

    #[test]
    fn windows_switches_are_paths_on_unix() {
        let o = opts(&["/F"], false);
        assert_eq!(o.path, Some(PathBuf::from("/F")));
        assert!(!o.files);
    }

    #[test]
    fn unix_switches() {
        for windows in [false, true] {
            let o = opts(
                &[
                    "-f",
                    "-a",
                    "--json=flat",
                    "-H",
                    "-L",
                    "3",
                    "-o",
                    "x.txt",
                    "dir",
                ],
                windows,
            );
            assert!(o.files && o.ascii && o.hidden);
            assert_eq!(o.json, Some(JsonFormat::Flat));
            assert_eq!(o.depth, Some(3));
            assert_eq!(o.output, Some(PathBuf::from("x.txt")));
            assert_eq!(o.path, Some(PathBuf::from("dir")));

            let o = opts(
                &[
                    "--files",
                    "--ascii",
                    "--json",
                    "--hidden",
                    "--depth=4",
                    "--output=y",
                ],
                windows,
            );
            assert!(o.files && o.ascii && o.hidden);
            assert_eq!(o.json, Some(JsonFormat::Nested));
            assert_eq!(o.depth, Some(4));
            assert_eq!(o.output, Some(PathBuf::from("y")));

            let o = opts(&["-faj", "-L2"], windows);
            assert!(o.files && o.ascii);
            assert_eq!(o.json, Some(JsonFormat::Nested));
            assert_eq!(o.depth, Some(2));

            assert_eq!(run(&["-h"], windows), Ok(Command::Help));
            assert_eq!(run(&["--help"], windows), Ok(Command::Help));
        }
    }

    #[test]
    fn long_options_case_insensitive_only_on_windows() {
        assert!(opts(&["--FILES"], true).files);
        assert!(run(&["--FILES"], false).is_err());
        assert_eq!(opts(&["--json=FLAT"], true).json, Some(JsonFormat::Flat));
        assert!(run(&["--json=FLAT"], false).is_err());
    }

    #[test]
    fn double_dash_ends_switches() {
        assert_eq!(opts(&["--", "-f"], false).path, Some(PathBuf::from("-f")));
        assert_eq!(opts(&["--", "/F"], true).path, Some(PathBuf::from("/F")));
    }

    #[test]
    fn errors() {
        for (args, windows) in [
            (&["/X"][..], true),
            (&["/F:yes"][..], true),
            (&["/J:tree"][..], true),
            (&["/L"][..], true),
            (&["/L:0"][..], true),
            (&["/L:abc"][..], true),
            (&["/O"][..], true),
            (&["/O:"][..], true),
            (&["--bogus"][..], false),
            (&["-x"][..], false),
            (&["-fz"][..], false),
            (&["-L"][..], false),
            (&["--depth"][..], false),
            (&["--files=1"][..], false),
            (&["--json=tree"][..], false),
            (&["a", "b"][..], false),
        ] {
            assert!(run(args, windows).is_err(), "{args:?} should fail");
        }
    }
}
