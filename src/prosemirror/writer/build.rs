//! The `DocBuilder` event walker that assembles ProseMirror JSON from Markdown events.

use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use serde_json::{Map, Value, json};

use super::{DocBuilder, ImageState, Mention};
use crate::prosemirror::nodes::{
    Frame, heading_level, is_whitespace_text, mention_node, text_node,
};

impl DocBuilder {
    pub(super) fn new() -> Self {
        Self {
            stack: vec![Frame::new("doc")],
            marks: Vec::new(),
            in_table_head: false,
            image: None,
            pending_mention: None,
        }
    }

    pub(super) fn handle(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::SoftBreak => self.push_break(true),
            Event::HardBreak => self.push_break(false),
            Event::Rule => self.append_to_top(json!({ "type": "horizontalRule" })),
            Event::TaskListMarker(checked) => self.task_marker(checked),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.push_frame("paragraph"),
            Tag::Heading { level, .. } => {
                let mut frame = Frame::new("heading");
                let mut attrs = Map::new();
                attrs.insert("level".to_string(), json!(heading_level(level)));
                frame.attrs = Some(attrs);
                self.stack.push(frame);
            }
            Tag::BlockQuote(_) => self.push_frame("blockquote"),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                let mut frame = Frame::new("codeBlock");
                let mut attrs = Map::new();
                attrs.insert("language".to_string(), Value::String(language));
                frame.attrs = Some(attrs);
                frame.code = Some(String::new());
                self.stack.push(frame);
            }
            Tag::List(Some(start)) => {
                let mut frame = Frame::new("orderedList");
                // The reader defaults to starting at 1; only record a non-default start.
                if start != 1 {
                    let mut attrs = Map::new();
                    attrs.insert("start".to_string(), json!(start));
                    frame.attrs = Some(attrs);
                }
                self.stack.push(frame);
            }
            Tag::List(None) => self.push_frame("bulletList"),
            Tag::Item => self.push_frame("listItem"),
            Tag::Table(_) => {
                self.in_table_head = false;
                self.push_frame("table");
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.push_frame("tableRow");
            }
            Tag::TableRow => self.push_frame("tableRow"),
            Tag::TableCell => {
                let cell_type = if self.in_table_head {
                    "tableHeader"
                } else {
                    "tableCell"
                };
                self.push_frame(cell_type);
            }
            Tag::Emphasis => self.marks.push(json!({ "type": "italic" })),
            Tag::Strong => self.marks.push(json!({ "type": "bold" })),
            Tag::Strikethrough => self.marks.push(json!({ "type": "strike" })),
            Tag::Link { dest_url, .. } => self.start_link(dest_url.as_ref()),
            Tag::Image { dest_url, .. } => {
                self.image = Some(ImageState {
                    src: dest_url.to_string(),
                    alt: String::new(),
                })
            }
            _ => {}
        }
    }

    /// A `user:`/`page:` URL with a non-empty id starts a mention; else a normal link mark.
    fn start_link(&mut self, url: &str) {
        let mention = url
            .strip_prefix("user:")
            .map(|id| ("user", id))
            .or_else(|| url.strip_prefix("page:").map(|id| ("page", id)));
        match mention {
            Some((entity_type, id)) if !id.is_empty() => {
                self.pending_mention = Some(Mention {
                    entity_type: entity_type.to_string(),
                    entity_id: id.to_string(),
                    label: String::new(),
                });
            }
            _ => self
                .marks
                .push(json!({ "type": "link", "attrs": { "href": url } })),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Emphasis => self.pop_mark("italic"),
            TagEnd::Strong => self.pop_mark("bold"),
            TagEnd::Strikethrough => self.pop_mark("strike"),
            TagEnd::Link => {
                if self.pending_mention.is_some() {
                    self.finish_mention();
                } else {
                    self.pop_mark("link");
                }
            }
            TagEnd::Image => self.finish_image(),
            TagEnd::Paragraph => self.finish_paragraph(),
            TagEnd::List(_) => self.finish_list(),
            TagEnd::TableHead => {
                self.in_table_head = false; // subsequent rows use tableCell
                self.close_frame();
            }
            TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::Item
            | TagEnd::Table
            | TagEnd::TableRow
            | TagEnd::TableCell => self.close_frame(),
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(image) = self.image.as_mut() {
            image.alt.push_str(text);
            return;
        }
        if let Some(mention) = self.pending_mention.as_mut() {
            mention.label.push_str(text);
            return;
        }
        if let Some(code) = self.stack.last_mut().and_then(|frame| frame.code.as_mut()) {
            code.push_str(text);
            return;
        }
        let marks = self.marks.clone();
        self.push_inline(text_node(text, &marks));
    }

    fn inline_code(&mut self, code: &str) {
        if let Some(mention) = self.pending_mention.as_mut() {
            mention.label.push_str(code); // code in a mention label is just display text
            return;
        }
        // The `code` mark must be innermost (first in the array) so the reader, which
        // applies marks in order, wraps the text with backticks before any surrounding
        // bold/italic/link — e.g. **`x`**, not `**x**`.
        let mut marks = vec![json!({ "type": "code" })];
        marks.extend(self.marks.clone());
        self.push_inline(text_node(code, &marks));
    }

    fn task_marker(&mut self, checked: bool) {
        if let Some(frame) = self.stack.last_mut() {
            frame.node_type = "taskItem".to_string();
            let mut attrs = Map::new();
            attrs.insert("checked".to_string(), Value::Bool(checked));
            frame.attrs = Some(attrs);
        }
        let depth = self.stack.len();
        if depth >= 2 {
            self.stack[depth - 2].is_task_list = true;
        }
    }

    /// A break inside an open mention label becomes a space; otherwise the usual break.
    fn push_break(&mut self, soft: bool) {
        if let Some(mention) = self.pending_mention.as_mut() {
            mention.label.push(' ');
        } else if soft {
            let marks = self.marks.clone();
            self.push_inline(text_node(" ", &marks));
        } else {
            self.push_inline(json!({ "type": "hardBreak" }));
        }
    }

    fn finish_mention(&mut self) {
        if let Some(mention) = self.pending_mention.take() {
            let node = mention_node(&mention.entity_type, &mention.entity_id, &mention.label);
            self.push_inline(node);
        }
    }

    /// Append an inline node, wrapping it in a paragraph when the current frame is a
    /// container (list item, table cell) that holds block children.
    fn push_inline(&mut self, node: Value) {
        if matches!(self.top_type(), Some("paragraph") | Some("heading")) {
            self.append_to_top(node);
        } else {
            self.append_to_trailing_paragraph(node);
        }
    }

    fn append_to_trailing_paragraph(&mut self, node: Value) {
        let Some(frame) = self.stack.last_mut() else {
            return;
        };
        let has_paragraph = matches!(
            frame
                .content
                .last()
                .and_then(|child| child.get("type"))
                .and_then(Value::as_str),
            Some("paragraph")
        );
        // Don't open a paragraph just to hold leading whitespace (e.g. a soft break at
        // a container boundary), which would render as a spurious empty paragraph.
        if !has_paragraph && is_whitespace_text(&node) {
            return;
        }
        if !has_paragraph {
            frame
                .content
                .push(json!({ "type": "paragraph", "content": [] }));
        }
        if let Some(Value::Object(paragraph)) = frame.content.last_mut()
            && let Some(Value::Array(items)) = paragraph.get_mut("content")
        {
            items.push(node);
        }
    }

    fn finish_image(&mut self) {
        if let Some(image) = self.image.take() {
            let node = json!({
                "type": "image",
                "attrs": { "src": image.src, "alt": image.alt }
            });
            self.append_to_top(node);
        }
    }

    fn finish_paragraph(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        let mut frame = self.stack.pop().unwrap();
        if frame.content.is_empty() {
            return;
        }
        // CommonMark wraps a top-level image in a paragraph; hoist it to a block image.
        let all_images = frame
            .content
            .iter()
            .all(|child| child.get("type").and_then(Value::as_str) == Some("image"));
        if all_images {
            for image in std::mem::take(&mut frame.content) {
                self.append_to_top(image);
            }
        } else {
            self.append_to_top(frame.into_value());
        }
    }

    fn finish_list(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        let mut frame = self.stack.pop().unwrap();
        // A list with any task marker is homogenized to a taskList; plain items become
        // unchecked task items so nothing is dropped (the reader skips mismatched kinds).
        if frame.is_task_list {
            for item in &mut frame.content {
                if item.get("type").and_then(Value::as_str) == Some("listItem")
                    && let Value::Object(object) = item
                {
                    object.insert("type".to_string(), Value::String("taskItem".to_string()));
                    object.insert("attrs".to_string(), json!({ "checked": false }));
                }
            }
            frame.node_type = "taskList".to_string();
        }
        self.append_to_top(frame.into_value());
    }

    fn close_frame(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        let frame = self.stack.pop().unwrap();
        self.append_to_top(frame.into_value());
    }

    fn pop_mark(&mut self, kind: &str) {
        if let Some(position) = self
            .marks
            .iter()
            .rposition(|mark| mark.get("type").and_then(Value::as_str) == Some(kind))
        {
            self.marks.remove(position);
        }
    }

    fn append_to_top(&mut self, node: Value) {
        if let Some(frame) = self.stack.last_mut() {
            frame.content.push(node);
        }
    }

    fn push_frame(&mut self, node_type: &str) {
        self.stack.push(Frame::new(node_type));
    }

    fn top_type(&self) -> Option<&str> {
        self.stack.last().map(|frame| frame.node_type.as_str())
    }

    pub(super) fn finish(mut self) -> Value {
        while self.stack.len() > 1 {
            let frame = self.stack.pop().unwrap();
            self.append_to_top(frame.into_value());
        }
        self.stack
            .pop()
            .map(Frame::into_value)
            .unwrap_or_else(|| json!({ "type": "doc", "content": [] }))
    }
}
