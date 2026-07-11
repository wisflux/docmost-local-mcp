//! ProseMirror node builders and the writer's block-assembly `Frame`.

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use pulldown_cmark::HeadingLevel;
use serde_json::{Map, Value, json};

/// A block container being assembled while walking the Markdown event stream.
pub(super) struct Frame {
    pub(super) node_type: String,
    pub(super) attrs: Option<Map<String, Value>>,
    pub(super) content: Vec<Value>,
    /// Set on a list frame once any of its items is a task item.
    pub(super) is_task_list: bool,
    /// `Some` for a `codeBlock` frame; accumulates the raw (unmarked) code text.
    pub(super) code: Option<String>,
}

impl Frame {
    pub(super) fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.to_string(),
            attrs: None,
            content: Vec::new(),
            is_task_list: false,
            code: None,
        }
    }

    pub(super) fn into_value(self) -> Value {
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

pub(super) fn text_node(text: &str, marks: &[Value]) -> Value {
    let mut node = Map::new();
    node.insert("type".to_string(), Value::String("text".to_string()));
    node.insert("text".to_string(), Value::String(text.to_string()));
    if !marks.is_empty() {
        node.insert("marks".to_string(), Value::Array(marks.to_vec()));
    }
    Value::Object(node)
}

pub(super) fn is_whitespace_text(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("text")
        && node.get("marks").is_none()
        && node
            .get("text")
            .and_then(Value::as_str)
            .map(|text| text.trim().is_empty())
            .unwrap_or(false)
}

pub(super) fn heading_level(level: HeadingLevel) -> u64 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Build an inline `mention` node. `entity_type` is `"user"` or `"page"`; `entity_id` is
/// the referenced user/page UUID; `label` is the display text (Docmost renders `@label`).
pub(super) fn mention_node(entity_type: &str, entity_id: &str, label: &str) -> Value {
    json!({
        "type": "mention",
        "attrs": {
            // Docmost dedups mentions by this id, so each occurrence needs a unique one.
            "id": random_uuid(),
            "label": label,
            "entityType": entity_type,
            "entityId": entity_id,
        }
    })
}

/// A random UUIDv4 string, for a mention node's `id`.
fn random_uuid() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant
    let h = |b: u8| format!("{b:02x}");
    format!(
        "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
        h(bytes[0]),
        h(bytes[1]),
        h(bytes[2]),
        h(bytes[3]),
        h(bytes[4]),
        h(bytes[5]),
        h(bytes[6]),
        h(bytes[7]),
        h(bytes[8]),
        h(bytes[9]),
        h(bytes[10]),
        h(bytes[11]),
        h(bytes[12]),
        h(bytes[13]),
        h(bytes[14]),
        h(bytes[15]),
    )
}
