//! JSON output in the nested (one root object) and flat (streamed array)
//! layouts.

use std::io::{self, Write};

use serde::Serialize;
use serde::ser::{SerializeSeq, Serializer as _};
use serde_json::ser::{PrettyFormatter, Serializer};

use crate::timefmt::rfc3339;
use crate::walk::{Entry, Kind, Listing, Walker};

/// A node of the nested layout.
#[derive(Serialize)]
struct Node {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<Node>>,
}

/// A record of the flat layout.
#[derive(Serialize)]
struct Record<'a> {
    path: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

fn pretty<W: Write>(out: W) -> Serializer<W, PrettyFormatter<'static>> {
    Serializer::with_formatter(out, PrettyFormatter::with_indent(b"  "))
}

/// Writes the nested layout: the whole tree as one object.
pub fn render_nested<W: Write>(
    out: &mut W,
    walker: &mut Walker,
    root: RootInfo,
    root_listing: Listing,
) -> io::Result<()> {
    let root = Node {
        name: root.name,
        path: Some(root.path),
        kind: Kind::Folder.as_str(),
        size: None,
        target: None,
        modified: root.modified,
        error: None,
        children: Some(children(walker, root_listing, 0)),
    };
    root.serialize(&mut pretty(&mut *out))?;
    writeln!(out)
}

/// What the nested layout needs to know about the root folder.
pub struct RootInfo {
    pub name: String,
    pub path: String,
    pub modified: Option<String>,
}

/// Builds the child nodes of a folder at `depth`.
fn children(walker: &mut Walker, listing: Listing, depth: usize) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(listing.files.len() + listing.folders.len());
    for entry in listing.files.into_iter().chain(listing.folders) {
        let mut node = Node {
            name: entry.name,
            path: None,
            kind: entry.kind.as_str(),
            size: entry.size,
            target: entry.target,
            modified: entry.modified.map(rfc3339),
            error: entry.error,
            children: None,
        };
        // Depth-limited and unreadable folders have no `children`.
        if entry.kind == Kind::Folder && node.error.is_none() && walker.descend(depth + 1) {
            match walker.list_or_warn(&entry.path) {
                Ok(sub) => node.children = Some(children(walker, sub, depth + 1)),
                Err(message) => node.error = Some(message),
            }
        }
        nodes.push(node);
    }
    nodes
}

/// Writes the flat layout, streaming one record at a time.
pub fn render_flat<W: Write>(
    out: &mut W,
    walker: &mut Walker,
    root_listing: Listing,
) -> io::Result<()> {
    let mut ser = pretty(&mut *out);
    let mut seq = ser.serialize_seq(None)?;
    emit(&mut seq, walker, "", root_listing, 0)?;
    seq.end()?;
    writeln!(out)
}

fn emit<S>(
    seq: &mut S,
    walker: &mut Walker,
    parent: &str,
    listing: Listing,
    depth: usize,
) -> io::Result<()>
where
    S: SerializeSeq<Ok = (), Error = serde_json::Error>,
{
    for entry in listing.files.into_iter().chain(listing.folders) {
        let path = if parent.is_empty() {
            entry.name.clone()
        } else {
            format!("{parent}/{}", entry.name)
        };
        let mut sub = None;
        let mut error = entry.error.clone();
        if entry.kind == Kind::Folder && error.is_none() && walker.descend(depth + 1) {
            match walker.list_or_warn(&entry.path) {
                Ok(listing) => sub = Some(listing),
                Err(message) => error = Some(message),
            }
        }
        seq.serialize_element(&record(&path, &entry, error.as_deref()))?;
        if let Some(listing) = sub {
            emit(seq, walker, &path, listing, depth + 1)?;
        }
    }
    Ok(())
}

fn record<'a>(path: &'a str, entry: &'a Entry, error: Option<&'a str>) -> Record<'a> {
    Record {
        path,
        kind: entry.kind.as_str(),
        size: entry.size,
        target: entry.target.as_deref(),
        modified: entry.modified.map(rfc3339),
        error,
    }
}
