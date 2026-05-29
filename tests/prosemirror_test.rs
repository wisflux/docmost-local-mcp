use docmost_local_mcp::prosemirror::{markdown_to_prosemirror, prosemirror_to_markdown};
use serde_json::{Value, json};

#[test]
fn renders_headings_paragraphs_links_and_code_blocks() {
    let markdown = prosemirror_to_markdown(&json!({
        "type": "doc",
        "content": [
            {
                "type": "heading",
                "attrs": { "level": 2 },
                "content": [{ "type": "text", "text": "Overview" }]
            },
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Visit " },
                    {
                        "type": "text",
                        "text": "Docmost",
                        "marks": [{ "type": "link", "attrs": { "href": "https://docmost.com" } }]
                    }
                ]
            },
            {
                "type": "codeBlock",
                "attrs": { "language": "ts" },
                "content": [{ "type": "text", "text": "console.log('hello');" }]
            }
        ]
    }));

    assert!(markdown.contains("## Overview"));
    assert!(markdown.contains("Visit [Docmost](https://docmost.com)"));
    assert!(markdown.contains("```ts"));
    assert!(markdown.contains("console.log('hello');"));
}

#[test]
fn renders_nested_unordered_lists_and_tables() {
    let markdown = prosemirror_to_markdown(&json!({
        "type": "doc",
        "content": [
            {
                "type": "bulletList",
                "content": [{
                    "type": "listItem",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [{ "type": "text", "text": "Parent" }]
                        },
                        {
                            "type": "bulletList",
                            "content": [{
                                "type": "listItem",
                                "content": [{
                                    "type": "paragraph",
                                    "content": [{ "type": "text", "text": "Child" }]
                                }]
                            }]
                        }
                    ]
                }]
            },
            {
                "type": "table",
                "content": [
                    {
                        "type": "tableRow",
                        "content": [
                            {
                                "type": "tableHeader",
                                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "A" }] }]
                            },
                            {
                                "type": "tableHeader",
                                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "B" }] }]
                            }
                        ]
                    },
                    {
                        "type": "tableRow",
                        "content": [
                            {
                                "type": "tableCell",
                                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "1" }] }]
                            },
                            {
                                "type": "tableCell",
                                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "2" }] }]
                            }
                        ]
                    }
                ]
            }
        ]
    }));

    assert!(markdown.contains("- Parent"));
    assert!(markdown.contains("  - Child"));
    assert!(markdown.contains("| A | B |"));
    assert!(markdown.contains("| 1 | 2 |"));
}

#[test]
fn renders_task_lists_with_checked_states() {
    let markdown = prosemirror_to_markdown(&json!({
        "type": "doc",
        "content": [
            {
                "type": "taskList",
                "content": [{
                    "type": "taskItem",
                    "attrs": { "checked": false },
                    "content": [{
                        "type": "paragraph",
                        "attrs": { "id": "bwjgrvbdamfz" },
                        "content": [{ "type": "text", "text": "Monitoring " }]
                    }]
                }]
            },
            {
                "type": "taskList",
                "content": [
                    {
                        "type": "taskItem",
                        "attrs": { "checked": false },
                        "content": [{
                            "type": "paragraph",
                            "attrs": { "id": "kmuomrhntdgh" },
                            "content": [{
                                "type": "text",
                                "text": "Performance Logging should be more to test multiple theories , not just raw logging and then making theories from them"
                            }]
                        }]
                    },
                    {
                        "type": "taskItem",
                        "attrs": { "checked": false },
                        "content": [{
                            "type": "paragraph",
                            "attrs": { "id": "kbtervxilajg" },
                            "content": [{
                                "type": "text",
                                "text": "Whims drive tasks completion rather than pre-planned flow"
                            }]
                        }]
                    }
                ]
            }
        ]
    }));

    assert!(markdown.contains("- [ ] Monitoring"));
    assert!(markdown.contains("- [ ] Performance Logging should be more to test multiple theories , not just raw logging and then making theories from them"));
    assert!(markdown.contains("- [ ] Whims drive tasks completion rather than pre-planned flow"));
}

// --- markdown_to_prosemirror (write path) ---

/// Find the first descendant node of the given `type` anywhere in the tree.
fn find_node<'a>(value: &'a Value, node_type: &str) -> Option<&'a Value> {
    if value.get("type").and_then(Value::as_str) == Some(node_type) {
        return Some(value);
    }
    value
        .get("content")
        .and_then(Value::as_array)
        .and_then(|children| {
            children
                .iter()
                .find_map(|child| find_node(child, node_type))
        })
}

fn mark_types(text_node: &Value) -> Vec<String> {
    text_node
        .get("marks")
        .and_then(Value::as_array)
        .map(|marks| {
            marks
                .iter()
                .filter_map(|mark| mark.get("type").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn markdown_to_prosemirror_wraps_a_doc_node() {
    let doc = markdown_to_prosemirror("hello world");
    assert_eq!(doc.get("type").and_then(Value::as_str), Some("doc"));
    let paragraph = find_node(&doc, "paragraph").expect("paragraph node");
    let text = find_node(paragraph, "text").expect("text node");
    assert_eq!(
        text.get("text").and_then(Value::as_str),
        Some("hello world")
    );
}

#[test]
fn markdown_to_prosemirror_heading_carries_level_attr() {
    let doc = markdown_to_prosemirror("## Overview");
    let heading = find_node(&doc, "heading").expect("heading node");
    assert_eq!(
        heading
            .get("attrs")
            .and_then(|attrs| attrs.get("level"))
            .and_then(Value::as_u64),
        Some(2)
    );
}

#[test]
fn markdown_to_prosemirror_uses_tiptap_mark_names() {
    // The reader matches "bold"/"italic"/"strike" (NOT strong/em/strikethrough);
    // emitting the wrong names would silently break round-trip.
    let doc = markdown_to_prosemirror("**b** *i* ~~s~~");
    let paragraph = find_node(&doc, "paragraph").expect("paragraph node");
    let texts: Vec<&Value> = paragraph
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter(|node| node.get("type").and_then(Value::as_str) == Some("text"))
        .collect();
    let all_marks: Vec<String> = texts.iter().flat_map(|node| mark_types(node)).collect();
    assert!(
        all_marks.contains(&"bold".to_string()),
        "marks: {all_marks:?}"
    );
    assert!(
        all_marks.contains(&"italic".to_string()),
        "marks: {all_marks:?}"
    );
    assert!(
        all_marks.contains(&"strike".to_string()),
        "marks: {all_marks:?}"
    );
    assert!(!all_marks.iter().any(|m| m == "strong" || m == "em"));
}

#[test]
fn markdown_to_prosemirror_inline_code_and_link() {
    let doc = markdown_to_prosemirror("see `code` and [site](https://example.com)");
    let paragraph = find_node(&doc, "paragraph").expect("paragraph node");
    let texts = paragraph.get("content").and_then(Value::as_array).unwrap();

    let code = texts
        .iter()
        .find(|node| mark_types(node).contains(&"code".to_string()))
        .expect("inline code text node");
    assert_eq!(code.get("text").and_then(Value::as_str), Some("code"));

    let link = texts
        .iter()
        .find(|node| mark_types(node).contains(&"link".to_string()))
        .expect("link text node");
    let href = link
        .get("marks")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|mark| mark.get("type").and_then(Value::as_str) == Some("link"))
        .and_then(|mark| mark.get("attrs"))
        .and_then(|attrs| attrs.get("href"))
        .and_then(Value::as_str);
    assert_eq!(href, Some("https://example.com"));
}

#[test]
fn markdown_to_prosemirror_fenced_code_block() {
    let doc = markdown_to_prosemirror("```ts\nconsole.log(1);\n```");
    let code_block = find_node(&doc, "codeBlock").expect("codeBlock node");
    assert_eq!(
        code_block
            .get("attrs")
            .and_then(|attrs| attrs.get("language"))
            .and_then(Value::as_str),
        Some("ts")
    );
    let text = find_node(code_block, "text").expect("code text node");
    assert_eq!(
        text.get("text").and_then(Value::as_str),
        Some("console.log(1);")
    );
    // Inline marks must NOT be applied inside a code block.
    assert!(text.get("marks").is_none());
}

#[test]
fn markdown_to_prosemirror_nested_bullet_list() {
    let doc = markdown_to_prosemirror("- Parent\n  - Child");
    let outer = find_node(&doc, "bulletList").expect("outer bulletList");
    let outer_item = outer
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .expect("outer listItem");
    assert_eq!(
        outer_item.get("type").and_then(Value::as_str),
        Some("listItem")
    );
    // The item holds a paragraph ("Parent") and a nested bulletList ("Child").
    let nested = find_node(outer_item, "bulletList").expect("nested bulletList");
    let child_text = find_node(nested, "text").expect("child text");
    assert_eq!(
        child_text.get("text").and_then(Value::as_str),
        Some("Child")
    );
}

#[test]
fn markdown_to_prosemirror_task_list_checked_states() {
    let doc = markdown_to_prosemirror("- [ ] a\n- [x] b");
    let task_list = find_node(&doc, "taskList").expect("taskList node");
    let items = task_list.get("content").and_then(Value::as_array).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].get("type").and_then(Value::as_str),
        Some("taskItem")
    );
    assert_eq!(
        items[0]
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        items[1]
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn markdown_to_prosemirror_table_distinguishes_header_and_cell() {
    let doc = markdown_to_prosemirror("| A | B |\n| --- | --- |\n| 1 | 2 |");
    let table = find_node(&doc, "table").expect("table node");
    assert!(find_node(table, "tableHeader").is_some());
    assert!(find_node(table, "tableCell").is_some());
}

#[test]
fn markdown_to_prosemirror_rule_and_image() {
    let hr_doc = markdown_to_prosemirror("text\n\n---\n\nmore");
    assert!(find_node(&hr_doc, "horizontalRule").is_some());

    let img_doc = markdown_to_prosemirror("![the alt](https://example.com/x.png)");
    let image = find_node(&img_doc, "image").expect("image node");
    let attrs = image.get("attrs").expect("image attrs");
    assert_eq!(
        attrs.get("src").and_then(Value::as_str),
        Some("https://example.com/x.png")
    );
    assert_eq!(attrs.get("alt").and_then(Value::as_str), Some("the alt"));
}

#[test]
fn markdown_round_trips_through_prosemirror() {
    let source = "# Title\n\nSome **bold** and *italic* and `code` and a [link](https://example.com).\n\n- one\n- two\n\n1. first\n2. second\n\n- [ ] todo\n- [x] done\n\n```rs\nlet x = 1;\n```\n\n> a quote\n\n---\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
    let rendered = prosemirror_to_markdown(&markdown_to_prosemirror(source));

    for needle in [
        "# Title",
        "**bold**",
        "*italic*",
        "`code`",
        "[link](https://example.com)",
        "- one",
        "1. first",
        "- [ ] todo",
        "- [x] done",
        "```rs",
        "let x = 1;",
        "> a quote",
        "---",
        "| A | B |",
        "| 1 | 2 |",
    ] {
        assert!(
            rendered.contains(needle),
            "missing {needle:?} in:\n{rendered}"
        );
    }
}
