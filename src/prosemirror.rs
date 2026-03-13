use serde_json::Value;

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
                output.push(convert_list(children(node), false, indent));
                output.push(String::new());
            }
            Some("orderedList") => {
                output.push(convert_list(children(node), true, indent));
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

fn convert_list(items: &[Value], ordered: bool, indent: usize) -> String {
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
            format!("{}. ", index + 1)
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
