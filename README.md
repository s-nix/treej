# treej

A cross-platform drop-in replacement for Windows `tree.com` that can also
export the tree as JSON. It runs on Windows, Linux and macOS.

```
C:\Work\project
│   readme.md
│
├───docs
│       guide.md
│
└───src
        main.rs
```

## Install

### Prebuilt binaries

Every `v*` tag publishes a GitHub Release with standalone binaries for:

| Platform | File |
|---|---|
| Windows x64 | `treej-<version>-x86_64-pc-windows-msvc.exe` |
| Windows ARM64 | `treej-<version>-aarch64-pc-windows-msvc.exe` |
| Linux x64 (static musl) | `treej-<version>-x86_64-unknown-linux-musl` |
| Linux ARM64 (static musl) | `treej-<version>-aarch64-unknown-linux-musl` |
| macOS Intel | `treej-<version>-x86_64-apple-darwin` |
| macOS Apple silicon | `treej-<version>-aarch64-apple-darwin` |

Each file has a matching `.sha256`, and `SHA256SUMS` lists them all. Rename the
download to `treej` (or `treej.exe`), put it on your `PATH`, and on Linux and
macOS mark it executable with `chmod +x treej`. The Windows builds link the C
runtime statically, so they don't need the Visual C++ Redistributable.

### From source

With a Rust toolchain (1.85 or newer):

```sh
cargo install --git https://github.com/s-nix/treej
```

## Usage

```
treej [PATH] [options]
```

`PATH` defaults to the current directory.

| Windows | Unix | Meaning |
|---|---|---|
| `/F` | `-f`, `--files` | Include files (otherwise folders only, like tree.com) |
| `/A` | `-a`, `--ascii` | Use ASCII line characters instead of box-drawing |
| `/J[:nested\|flat]` | `-j`, `--json[=nested\|flat]` | Output JSON instead of text. Default `nested` |
| `/H` | `-H`, `--hidden` | Include hidden and system items (excluded by default) |
| `/L:n` | `-L n`, `--depth n` | Limit recursion depth (`1` = only the root's direct children) |
| `/O:file` | `-o file`, `--output file` | Write to a file instead of stdout |
| `/?` | `-h`, `--help` | Show usage |

- **Windows-style switches work only on Windows.** On Linux and macOS a leading
  `/` starts an absolute path, so `treej /F` lists the folder `/F`.
- **Unix-style switches work everywhere**, including on Windows.
- On Windows, Windows-style switches and their values (`/j:FLAT`) and long
  options (`--FILES`) are case-insensitive. Short options are always
  case-sensitive, because `-h` (help) and `-H` (hidden) differ.
- Short options can be combined: `-fa`, `-L2`, `-ofile.txt`.
- `--json` takes its format only with `=`: `--json=flat`. `--depth` and
  `--output` accept `--depth 2` or `--depth=2`.
- `--` ends the options, so `treej -- -odd-name` lists a folder called
  `-odd-name`.
- An unknown switch prints an error and the usage text, then exits with code 1.

### Examples

Windows (cmd or PowerShell):

```bat
treej
treej C:\Work\project /F
treej /F /A > tree.txt
treej /J /O:tree.json
treej D:\Data /J:flat /L:2 /H
```

Any platform:

```sh
treej
treej ~/project -f
treej -fa > tree.txt
treej --json -o tree.json
treej /var/log --json=flat --depth 2 --hidden
treej src -j | jq '.children[].name'
```

## JSON output

JSON follows the same filters as text: files appear only with `/F`, hidden
items only with `/H`, and the depth limit applies. Output is pretty-printed
with 2-space indentation. JSON Schemas for both layouts are in
[`schema/`](schema/).

Fields:

| Field | Present on | Meaning |
|---|---|---|
| `name` | nested | File or folder name |
| `path` | nested root; every flat record | Nested: absolute path of the root. Flat: path relative to the root, always `/`-separated |
| `type` | all | `file`, `folder` or `symlink` |
| `size` | files | Size in bytes |
| `target` | symlinks | Link target, when it can be read |
| `modified` | all (when readable) | Last modification time, RFC 3339 UTC, whole seconds |
| `error` | items that could not be read | The error message; such folders have no `children` |
| `children` | nested folders | Files first, then folders, each sorted by name ignoring case. Absent when the folder was cut off by the depth limit or could not be read |

### Nested (`/J`, `/J:nested`, `-j`, `--json`, `--json=nested`)

A single object for the root ([schema](schema/nested.schema.json)):

```json
{
  "name": "project",
  "path": "C:\\Work\\project",
  "type": "folder",
  "modified": "2026-11-10T14:22:00Z",
  "children": [
    {
      "name": "readme.md",
      "type": "file",
      "size": 2048,
      "modified": "2026-11-10T14:22:00Z"
    },
    {
      "name": "src",
      "type": "folder",
      "modified": "2026-11-10T14:22:00Z",
      "children": []
    }
  ]
}
```

### Flat (`/J:flat`, `--json=flat`)

An array with one record per item; the root itself is not included
([schema](schema/flat.schema.json)). Records come in walk order: a folder's
files, then each subfolder followed by everything inside it. treej writes each
record as soon as it is read, so memory use stays low on very large trees.

```json
[
  {
    "path": "readme.md",
    "type": "file",
    "size": 2048,
    "modified": "2026-11-10T14:22:00Z"
  },
  {
    "path": "src",
    "type": "folder",
    "modified": "2026-11-10T14:22:00Z"
  },
  {
    "path": "src/main.rs",
    "type": "file",
    "size": 512,
    "modified": "2026-11-10T14:22:00Z"
  }
]
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Invalid arguments, or the root folder is missing or unreadable, or the output can't be written |
| 2 | Finished, but some folders or items could not be read (each one is reported on stderr) |

## Behavior

- **Order:** within each folder, files come first, then subfolders, each
  sorted by name ignoring case.
- **Symlinks and junctions are never followed.** Symbolic links, Windows
  junctions and other name-surrogate reparse points are reported as
  `"type": "symlink"` with a `"target"` when it can be read. Links to folders
  are listed with the folders (without children); other links are listed with
  the files and appear only with `/F`. Reparse points that are not links, such
  as OneDrive placeholders, are treated as ordinary files and folders.
- **Hidden items:** on Linux and macOS, names starting with `.`. On Windows,
  items with the Hidden or System attribute (a dotfile without those
  attributes is shown, as in Explorer). The root folder is always listed.
- **Unreadable folders:** treej keeps going. Text output prints the folder
  line without children; JSON adds an `error` field and leaves out
  `children`. Either way a warning goes to stderr and the exit code is 2.
- **Long paths:** paths over 260 characters work on Windows. The binary
  carries a `longPathAware` manifest, and file system calls use extended-length
  paths where needed.
- **Names that aren't valid Unicode** are converted lossily (invalid bytes
  become `�`) rather than causing an error.
- **Encoding:** output is always UTF-8. In a Windows console treej writes
  Unicode directly, so box-drawing characters show correctly whatever the
  console code page.

## Differences from tree.com

- No "Folder PATH listing for volume …" / "Volume serial number is …" header.
- The first line is the root's absolute path as given, with `.` and `..`
  resolved and its case kept (tree.com upper-cases it).
- Trailing whitespace is trimmed from the blank spacer lines printed after a
  folder's files.
- Lines end with `\n` on every platform, including when writing to a file on
  Windows (tree.com writes `\r\n`).
- Output is UTF-8, never an OEM code page.
- Hidden and system items are skipped unless `/H` is given.
- Junctions and symlinks are listed but not entered.
- Items are sorted by name, rather than listed in file system order.
- Extra switches: `/J`, `/H`, `/L`, `/O`, and the Unix-style options.

## Building

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

Release builds use LTO, a single codegen unit, `opt-level = 3` and stripped
symbols. `.cargo/config.toml` links the C runtime statically on Windows MSVC
targets.

CI (GitHub Actions) runs `cargo fmt --check`, clippy and the tests on Windows,
Linux and macOS for every push and pull request. Pushing a tag such as
`v0.1.0` also builds the six release binaries and publishes them with SHA-256
checksums.
