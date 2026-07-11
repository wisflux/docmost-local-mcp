//! Markdown → ProseMirror JSON. The event-walking `DocBuilder` impl lives in [`build`].

mod build;

use pulldown_cmark::{Options, Parser};
use serde_json::Value;

use super::nodes::Frame;

/// Convert a Markdown string into a Docmost ProseMirror document
/// (`{ "type": "doc", "content": [...] }`).
///
/// This is the inverse of [`super::prosemirror_to_markdown`]. It emits the same node and
/// mark `type` strings (and attr keys) the reader recognizes — note the Tiptap-style mark
/// names `bold`/`italic` (not `strong`/`em`) — so the two round-trip. The `embed` node has
/// no Markdown form and is never produced.
///
/// Mentions use a link-style convention: `[Label](user:UUID)` becomes a user mention and
/// `[Label](page:UUID)` a page mention (any other link URL stays an ordinary link).
pub fn markdown_to_prosemirror(content: &str) -> Value {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut builder = DocBuilder::new();
    for event in Parser::new_ext(content, options) {
        builder.handle(event);
    }
    builder.finish()
}

struct ImageState {
    src: String,
    alt: String,
}

/// A pending mention captured between a `[label](user:ID)` link's start and end events.
struct Mention {
    entity_type: String,
    entity_id: String,
    label: String,
}

struct DocBuilder {
    stack: Vec<Frame>,
    marks: Vec<Value>,
    in_table_head: bool,
    image: Option<ImageState>,
    pending_mention: Option<Mention>,
}
