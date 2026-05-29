use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Value, json};

pub fn prosemirror_to_markdown(content: &Value) -> String {
    if !is_document_node(content) {
        return String::new();
    }

    let nodes = content
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    convert_nodes(&nodes, 0).trim().to_string()
}

fn is_document_node(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|node_type| node_type == "doc")
        .unwrap_or(false)
}

fn convert_nodes(nodes: &[Value], indent: usize) -> String {
    let mut output = Vec::new();

    for node in nodes {
        match node_type(node) {
            Some("paragraph") => {
                let text = extract_text(children(node));
                if !text.is_empty() {
                    output.push(text);
                    output.push(String::new());
                }
            }
            Some("heading") => {
                let level = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("level"))
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 6);
                output.push(format!(
                    "{} {}",
                    "#".repeat(level as usize),
                    extract_text(children(node))
                ));
                output.push(String::new());
            }
            Some("bulletList") => {
                output.push(convert_list(children(node), false, indent, 1));
                output.push(String::new());
            }
            Some("orderedList") => {
                let start = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("start"))
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                output.push(convert_list(children(node), true, indent, start));
                output.push(String::new());
            }
            Some("taskList") => {
                output.push(convert_task_list(children(node), indent));
                output.push(String::new());
            }
            Some("codeBlock") => {
                let language = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("language"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                output.push(format!("```{language}"));
                output.push(extract_text(children(node)));
                output.push("```".to_string());
                output.push(String::new());
            }
            Some("blockquote") => {
                let inner = convert_nodes(children(node), indent);
                let quoted = inner
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !quoted.is_empty() {
                    output.push(quoted);
                    output.push(String::new());
                }
            }
            Some("horizontalRule") => {
                output.push("---".to_string());
                output.push(String::new());
            }
            Some("table") => {
                let table = convert_table(node);
                if !table.is_empty() {
                    output.push(table);
                    output.push(String::new());
                }
            }
            Some("image") => {
                let src = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("src"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !src.is_empty() {
                    let alt = node
                        .get("attrs")
                        .and_then(|attrs| attrs.get("alt"))
                        .and_then(Value::as_str)
                        .unwrap_or("image");
                    output.push(format!("![{alt}]({src})"));
                    output.push(String::new());
                }
            }
            Some("embed") => {
                let src = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("src"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !src.is_empty() {
                    output.push(format!("[Embedded content]({src})"));
                    output.push(String::new());
                }
            }
            _ => {}
        }
    }

    output.join("\n").trim_end().to_string()
}

fn extract_text(content: &[Value]) -> String {
    let mut parts = Vec::new();

    for item in content {
        match node_type(item) {
            Some("text") => {
                let mut text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                if let Some(marks) = item.get("marks").and_then(Value::as_array) {
                    for mark in marks {
                        match node_type(mark) {
                            Some("bold") => text = format!("**{text}**"),
                            Some("italic") => text = format!("*{text}*"),
                            Some("code") => text = format!("`{text}`"),
                            Some("strike") => text = format!("~~{text}~~"),
                            Some("link") => {
                                let href = mark
                                    .get("attrs")
                                    .and_then(|attrs| attrs.get("href"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                if !href.is_empty() {
                                    text = format!("[{text}]({href})");
                                }
                            }
                            _ => {}
                        }
                    }
                }

                parts.push(text);
            }
            Some("hardBreak") => parts.push("\n".to_string()),
            _ if !children(item).is_empty() => parts.push(extract_text(children(item))),
            _ => {}
        }
    }

    parts.join("")
}

fn convert_list(items: &[Value], ordered: bool, indent: usize, start: u64) -> String {
    let mut output = Vec::new();
    let prefix = " ".repeat(indent);

    for (index, item) in items.iter().enumerate() {
        if node_type(item) != Some("listItem") {
            continue;
        }

        let mut first_paragraph = String::new();
        let mut nested_lists = Vec::new();

        for child in children(item) {
            match node_type(child) {
                Some("paragraph") if first_paragraph.is_empty() => {
                    first_paragraph = extract_text(children(child));
                }
                Some("bulletList") | Some("orderedList") => nested_lists.push(child.clone()),
                _ => {}
            }
        }

        let marker = if ordered {
            format!("{}. ", start + index as u64)
        } else {
            "- ".to_string()
        };
        output.push(format!("{prefix}{marker}{first_paragraph}"));

        for nested_list in nested_lists {
            output.push(convert_nodes(&[nested_list], indent + 2));
        }
    }

    output.join("\n")
}

fn convert_task_list(items: &[Value], indent: usize) -> String {
    let mut output = Vec::new();
    let prefix = " ".repeat(indent);

    for item in items {
        if node_type(item) != Some("taskItem") {
            continue;
        }

        let mut first_paragraph = String::new();
        let mut nested_lists = Vec::new();

        for child in children(item) {
            match node_type(child) {
                Some("paragraph") if first_paragraph.is_empty() => {
                    first_paragraph = extract_text(children(child));
                }
                Some("bulletList") | Some("orderedList") | Some("taskList") => {
                    nested_lists.push(child.clone())
                }
                _ => {}
            }
        }

        let checked = item
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let marker = if checked { "- [x] " } else { "- [ ] " };
        output.push(format!("{prefix}{marker}{first_paragraph}"));

        for nested_list in nested_lists {
            output.push(convert_nodes(&[nested_list], indent + 2));
        }
    }

    output.join("\n")
}

fn convert_table(table_node: &Value) -> String {
    let mut rows = Vec::new();

    for row in children(table_node) {
        if node_type(row) != Some("tableRow") {
            continue;
        }

        let mut cells = Vec::new();
        for cell in children(row) {
            match node_type(cell) {
                Some("tableCell") | Some("tableHeader") => {
                    let cell_text = convert_nodes(children(cell), 0)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    cells.push(cell_text);
                }
                _ => {}
            }
        }

        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    let Some(first_row) = rows.first() else {
        return String::new();
    };

    let mut lines = Vec::new();
    lines.push(format!("| {} |", first_row.join(" | ")));
    lines.push(format!(
        "| {} |",
        first_row
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | ")
    ));

    for row in rows.into_iter().skip(1) {
        lines.push(format!("| {} |", row.join(" | ")));
    }

    lines.join("\n")
}

fn node_type(node: &Value) -> Option<&str> {
    node.get("type").and_then(Value::as_str)
}

fn children(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Convert a Markdown string into a Docmost ProseMirror document
/// (`{ "type": "doc", "content": [...] }`).
///
/// This is the inverse of [`prosemirror_to_markdown`]. It emits the same node and mark
/// `type` strings (and attr keys) the reader recognizes — note the Tiptap-style mark
/// names `bold`/`italic` (not `strong`/`em`) — so the two round-trip. The `embed` node
/// has no Markdown form and is never produced.
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

/// A block container being assembled while walking the Markdown event stream.
struct Frame {
    node_type: String,
    attrs: Option<Map<String, Value>>,
    content: Vec<Value>,
    /// Set on a list frame once any of its items is a task item.
    is_task_list: bool,
    /// `Some` for a `codeBlock` frame; accumulates the raw (unmarked) code text.
    code: Option<String>,
}

impl Frame {
    fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.to_string(),
            attrs: None,
            content: Vec::new(),
            is_task_list: false,
            code: None,
        }
    }

    fn into_value(self) -> Value {
        let mut node = Map::new();
        node.insert("type".to_string(), Value::String(self.node_type));
        if let Some(attrs) = self.attrs {
            node.insert("attrs".to_string(), Value::Object(attrs));
        }

        let content = match self.code {
            Some(code) => {
                let code = code.strip_suffix('\n').unwrap_or(&code).to_string();
                if code.is_empty() {
                    Vec::new()
                } else {
                    vec![text_node(&code, &[])]
                }
            }
            None => self.content,
        };
        node.insert("content".to_string(), Value::Array(content));
        Value::Object(node)
    }
}

struct ImageState {
    src: String,
    alt: String,
}

struct DocBuilder {
    stack: Vec<Frame>,
    marks: Vec<Value>,
    in_table_head: bool,
    image: Option<ImageState>,
}

impl DocBuilder {
    fn new() -> Self {
        Self {
            stack: vec![Frame::new("doc")],
            marks: Vec::new(),
            in_table_head: false,
            image: None,
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::SoftBreak => {
                let marks = self.marks.clone();
                self.push_inline(text_node(" ", &marks));
            }
            Event::HardBreak => self.push_inline(json!({ "type": "hardBreak" })),
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
            Tag::Link { dest_url, .. } => self
                .marks
                .push(json!({ "type": "link", "attrs": { "href": dest_url.to_string() } })),
            Tag::Image { dest_url, .. } => {
                self.image = Some(ImageState {
                    src: dest_url.to_string(),
                    alt: String::new(),
                })
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Emphasis => self.pop_mark("italic"),
            TagEnd::Strong => self.pop_mark("bold"),
            TagEnd::Strikethrough => self.pop_mark("strike"),
            TagEnd::Link => self.pop_mark("link"),
            TagEnd::Image => self.finish_image(),
            TagEnd::Paragraph => self.finish_paragraph(),
            TagEnd::List(_) => self.finish_list(),
            TagEnd::TableHead => {
                // Subsequent rows are body rows whose cells are `tableCell`.
                self.in_table_head = false;
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
        if let Some(code) = self.stack.last_mut().and_then(|frame| frame.code.as_mut()) {
            code.push_str(text);
            return;
        }
        let marks = self.marks.clone();
        self.push_inline(text_node(text, &marks));
    }

    fn inline_code(&mut self, code: &str) {
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
        // CommonMark wraps a top-level image in a paragraph; hoist a lone-image
        // paragraph so the image becomes a block node the reader can render.
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
        // A taskList must contain only taskItems, and a bulletList/orderedList only
        // listItems — the reader skips the wrong kind. When a list has any task marker,
        // homogenize it to a taskList and coerce plain items to unchecked task items so
        // no item is dropped (rather than losing either the tasks or the plain items).
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

    fn finish(mut self) -> Value {
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

fn text_node(text: &str, marks: &[Value]) -> Value {
    let mut node = Map::new();
    node.insert("type".to_string(), Value::String("text".to_string()));
    node.insert("text".to_string(), Value::String(text.to_string()));
    if !marks.is_empty() {
        node.insert("marks".to_string(), Value::Array(marks.to_vec()));
    }
    Value::Object(node)
}

fn is_whitespace_text(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("text")
        && node.get("marks").is_none()
        && node
            .get("text")
            .and_then(Value::as_str)
            .map(|text| text.trim().is_empty())
            .unwrap_or(false)
}

fn heading_level(level: HeadingLevel) -> u64 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
