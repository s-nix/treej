//! tree.com-compatible text output.

use std::io::{self, Write};

use crate::walk::{Kind, Listing, Walker};

/// Line-drawing pieces: branch, last branch, vertical continuation, blank.
struct Glyphs {
    tee: &'static str,
    last: &'static str,
    bar: &'static str,
    blank: &'static str,
}

const UNICODE: Glyphs = Glyphs {
    tee: "├───",
    last: "└───",
    bar: "│   ",
    blank: "    ",
};

const ASCII: Glyphs = Glyphs {
    tee: "+---",
    last: "\\---",
    bar: "|   ",
    blank: "    ",
};

/// Writes the tree. `root_listing` is the already-read root folder.
pub fn render<W: Write>(
    out: &mut W,
    walker: &mut Walker,
    root_display: &str,
    root_listing: Listing,
    ascii: bool,
) -> io::Result<()> {
    let glyphs = if ascii { &ASCII } else { &UNICODE };
    writeln!(out, "{root_display}")?;
    if root_listing.folders.is_empty() && !walker.filter.files {
        writeln!(out, "No subfolders exist")?;
        return Ok(());
    }
    let mut prefix = String::new();
    render_listing(out, walker, glyphs, &mut prefix, root_listing, 0)
}

/// Prints one folder's contents. `depth` is the folder's own depth.
fn render_listing<W: Write>(
    out: &mut W,
    walker: &mut Walker,
    g: &Glyphs,
    prefix: &mut String,
    listing: Listing,
    depth: usize,
) -> io::Result<()> {
    if !listing.files.is_empty() {
        let pad = if listing.folders.is_empty() {
            g.blank
        } else {
            g.bar
        };
        for file in &listing.files {
            writeln!(out, "{prefix}{pad}{}", file.name)?;
        }
        let spacer = format!("{prefix}{pad}");
        writeln!(out, "{}", spacer.trim_end())?;
    }

    let count = listing.folders.len();
    for (i, folder) in listing.folders.into_iter().enumerate() {
        let is_last = i + 1 == count;
        let branch = if is_last { g.last } else { g.tee };
        writeln!(out, "{prefix}{branch}{}", folder.name)?;

        if folder.kind != Kind::Folder || !walker.descend(depth + 1) {
            continue;
        }
        // Unreadable folders are printed normally, without children.
        let Ok(children) = walker.list_or_warn(&folder.path) else {
            continue;
        };
        let len = prefix.len();
        prefix.push_str(if is_last { g.blank } else { g.bar });
        render_listing(out, walker, g, prefix, children, depth + 1)?;
        prefix.truncate(len);
    }
    Ok(())
}
