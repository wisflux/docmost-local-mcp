//! Conversion between Docmost's ProseMirror JSON page/comment bodies and Markdown.
//!
//! - [`reader`] — ProseMirror JSON → Markdown (used by read tools such as `get_page`).
//! - [`writer`] — Markdown → ProseMirror JSON (used by `update_page` and the comment
//!   tools). `create_page` instead uploads raw Markdown to Docmost's import endpoint.
//! - [`nodes`] — ProseMirror node builders shared by the writer.

mod nodes;
mod reader;
mod writer;

pub use reader::prosemirror_to_markdown;
pub use writer::markdown_to_prosemirror;
