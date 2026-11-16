//! treej: a cross-platform drop-in replacement for Windows `tree.com` with
//! JSON export.

mod cli;
mod render_json;
mod render_text;
mod timefmt;
mod walk;

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use cli::{Command, JsonFormat, Options};
use walk::{Filter, Walker};

const EXIT_OK: u8 = 0;
const EXIT_USAGE: u8 = 1;
const EXIT_PARTIAL: u8 = 2;

fn main() -> ExitCode {
    // Recursion depth follows folder depth, so give it a generous stack.
    let worker = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("failed to start worker thread");
    ExitCode::from(worker.join().unwrap_or(EXIT_USAGE))
}

fn run() -> u8 {
    let command = match cli::parse(std::env::args_os().skip(1), cfg!(windows)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("treej: error: {e}\n");
            eprint!("{}", cli::usage());
            return EXIT_USAGE;
        }
    };
    let opts = match command {
        Command::Help => {
            print!("{}", cli::usage());
            return EXIT_OK;
        }
        Command::Run(opts) => opts,
    };

    match list(&opts) {
        Ok(code) => code,
        Err(Failure::Fatal(msg)) => {
            eprintln!("treej: error: {msg}");
            EXIT_USAGE
        }
        // The reader went away (e.g. `treej | head`); that is not an error.
        Err(Failure::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe => EXIT_OK,
        Err(Failure::Io(e)) => {
            eprintln!("treej: error: cannot write output: {e}");
            EXIT_USAGE
        }
    }
}

enum Failure {
    Fatal(String),
    Io(io::Error),
}

impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Failure::Io(e)
    }
}

fn list(opts: &Options) -> Result<u8, Failure> {
    let given = opts.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = std::path::absolute(&given)
        .map(|p| normalize(&p))
        .map_err(|e| Failure::Fatal(format!("invalid path '{}': {e}", given.display())))?;

    let root_meta = fs::metadata(&root)
        .map_err(|e| Failure::Fatal(format!("cannot access '{}': {e}", root.display())))?;
    if !root_meta.is_dir() {
        return Err(Failure::Fatal(format!(
            "'{}' is not a folder",
            root.display()
        )));
    }

    let mut walker = Walker::new(Filter {
        files: opts.files,
        hidden: opts.hidden,
        max_depth: opts.depth,
    });
    let root_listing = walker
        .list(&root)
        .map_err(|e| Failure::Fatal(format!("cannot read '{}': {e}", root.display())))?;

    let sink: Box<dyn Write> = match &opts.output {
        Some(path) => Box::new(
            File::create(path)
                .map_err(|e| Failure::Fatal(format!("cannot create '{}': {e}", path.display())))?,
        ),
        None => Box::new(io::stdout().lock()),
    };
    let mut out = BufWriter::with_capacity(64 * 1024, sink);

    match opts.json {
        None => render_text::render(
            &mut out,
            &mut walker,
            &root.display().to_string(),
            root_listing,
            opts.ascii,
        )?,
        Some(JsonFormat::Nested) => {
            let info = render_json::RootInfo {
                name: root_name(&root),
                path: root.to_string_lossy().into_owned(),
                modified: root_meta.modified().ok().map(timefmt::rfc3339),
            };
            render_json::render_nested(&mut out, &mut walker, info, root_listing)?
        }
        Some(JsonFormat::Flat) => render_json::render_flat(&mut out, &mut walker, root_listing)?,
    }
    out.flush()?;

    Ok(if walker.had_errors() {
        EXIT_PARTIAL
    } else {
        EXIT_OK
    })
}

/// Removes `.` and `..` components lexically, the way a shell's `cd ..`
/// does. (On Windows `absolute` already does this; on Unix it keeps `..`.)
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The root's own name, or the whole path for a drive or filesystem root.
fn root_name(root: &Path) -> String {
    match root.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => root.to_string_lossy().into_owned(),
    }
}
