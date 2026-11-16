//! Directory reading shared by the text and JSON renderers.
//!
//! The walker lists one folder at a time; each renderer drives the recursion
//! itself so it can stream output. Symlinks, junctions and other name-surrogate
//! reparse points are never followed.

use std::fs::{self, DirEntry, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Filters that decide which items appear.
#[derive(Debug, Clone, Copy, Default)]
pub struct Filter {
    pub files: bool,
    pub hidden: bool,
    /// Maximum depth; the root's children are at depth 1.
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
    Symlink,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Folder => "folder",
            Kind::Symlink => "symlink",
        }
    }
}

/// One item inside a folder.
#[derive(Debug, Clone)]
pub struct Entry {
    /// File name, converted lossily to UTF-8.
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
    /// Size in bytes, for files only.
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    /// Link target, for symlinks whose target could be read.
    pub target: Option<String>,
    /// Problem reading this item's metadata.
    pub error: Option<String>,
}

/// The visible contents of one folder, split the way tree.com prints them.
#[derive(Debug, Default)]
pub struct Listing {
    /// Files, plus symlinks that do not point at a folder. Empty unless the
    /// filter includes files.
    pub files: Vec<Entry>,
    /// Folders, plus symlinks that point at a folder (never descended into).
    pub folders: Vec<Entry>,
}

/// Lists folders and reports problems to stderr, remembering whether any
/// occurred so the program can exit with code 2.
pub struct Walker {
    pub filter: Filter,
    had_errors: bool,
}

impl Walker {
    pub fn new(filter: Filter) -> Self {
        Walker {
            filter,
            had_errors: false,
        }
    }

    pub fn had_errors(&self) -> bool {
        self.had_errors
    }

    /// Whether the children of a folder at `depth` should be listed.
    pub fn descend(&self, depth: usize) -> bool {
        self.filter.max_depth.is_none_or(|max| depth < max)
    }

    /// Writes a warning to stderr and marks the run as incomplete.
    pub fn warn(&mut self, path: &Path, what: &str, error: &dyn std::fmt::Display) {
        self.had_errors = true;
        let _ = writeln!(
            io::stderr().lock(),
            "treej: warning: {what} '{}': {error}",
            path.display()
        );
    }

    /// Reads a folder that is not the root. On failure, warns and returns the
    /// message so JSON output can record it.
    pub fn list_or_warn(&mut self, path: &Path) -> Result<Listing, String> {
        self.list(path).map_err(|e| {
            self.warn(path, "cannot read folder", &e);
            e.to_string()
        })
    }

    /// Reads a folder. Failing to open it is returned to the caller;
    /// problems with single items are warned about here.
    pub fn list(&mut self, path: &Path) -> io::Result<Listing> {
        let mut listing = Listing::default();
        for item in fs::read_dir(path)? {
            let de = match item {
                Ok(de) => de,
                Err(e) => {
                    self.warn(path, "cannot read an item in", &e);
                    continue;
                }
            };
            if let Some((entry, is_folderish)) = self.entry(&de) {
                if is_folderish {
                    listing.folders.push(entry);
                } else if self.filter.files {
                    listing.files.push(entry);
                }
            }
        }
        sort(&mut listing.files);
        sort(&mut listing.folders);
        Ok(listing)
    }

    /// Builds an entry, or `None` if it is filtered out. The flag says
    /// whether it belongs with the folders.
    fn entry(&mut self, de: &DirEntry) -> Option<(Entry, bool)> {
        let path = de.path();
        let name = de.file_name().to_string_lossy().into_owned();
        let file_type = match de.file_type() {
            Ok(t) => t,
            Err(e) => {
                self.warn(&path, "cannot read", &e);
                return None;
            }
        };
        let kind = if file_type.is_symlink() {
            Kind::Symlink
        } else if file_type.is_dir() {
            Kind::Folder
        } else {
            Kind::File
        };

        // Drop plain files early when they are not wanted. Symlinks go on,
        // since they may point at a folder.
        if kind == Kind::File && !self.filter.files {
            return None;
        }
        if !self.filter.hidden && is_dotfile(&name) {
            return None;
        }

        // DirEntry::metadata does not follow symlinks on any platform.
        let metadata = de.metadata();
        if let Ok(md) = &metadata {
            if !self.filter.hidden && has_hidden_attribute(md) {
                return None;
            }
        }

        let is_folderish = match kind {
            Kind::Folder => true,
            Kind::File => false,
            Kind::Symlink => link_points_to_folder(&path, metadata.as_ref().ok()),
        };
        if !is_folderish && !self.filter.files {
            return None;
        }

        let mut entry = Entry {
            name,
            path,
            kind,
            size: None,
            modified: None,
            target: None,
            error: None,
        };
        match metadata {
            Ok(md) => {
                entry.modified = md.modified().ok();
                if kind == Kind::File {
                    entry.size = Some(md.len());
                }
            }
            Err(e) => {
                self.warn(&entry.path, "cannot read metadata of", &e);
                entry.error = Some(e.to_string());
            }
        }
        if kind == Kind::Symlink {
            entry.target = fs::read_link(&entry.path).ok().map(|t| display_target(&t));
        }
        Some((entry, is_folderish))
    }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|e| (e.name.to_lowercase(), e.name.clone()));
}

/// Dotfiles count as hidden on Unix.
fn is_dotfile(name: &str) -> bool {
    cfg!(unix) && name.starts_with('.')
}

#[cfg(windows)]
fn has_hidden_attribute(md: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    md.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(windows))]
fn has_hidden_attribute(_md: &Metadata) -> bool {
    false
}

/// Whether a link should be grouped with folders. On Windows the link itself
/// carries the directory attribute (directory symlinks and junctions); on
/// Unix the target is stat'ed, without being traversed.
#[cfg(windows)]
fn link_points_to_folder(_path: &Path, md: Option<&Metadata>) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    md.is_some_and(|m| m.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0)
}

#[cfg(not(windows))]
fn link_points_to_folder(path: &Path, _md: Option<&Metadata>) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_dir())
}

/// Shows a link target without Windows verbatim prefixes.
fn display_target(target: &Path) -> String {
    let s = target.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\").or_else(|| s.strip_prefix(r"\??\")) {
        rest.to_owned()
    } else {
        s.into_owned()
    }
}
