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

// --- markdown_to_prosemirror: extended edge-case coverage ---

/// The direct children of the top-level `doc` node.
fn doc_children(doc: &Value) -> &Vec<Value> {
    doc.get("content")
        .and_then(Value::as_array)
        .expect("doc has a content array")
}

fn node_type(node: &Value) -> Option<&str> {
    node.get("type").and_then(Value::as_str)
}

#[test]
fn markdown_to_prosemirror_empty_input_is_empty_doc() {
    let doc = markdown_to_prosemirror("");
    assert_eq!(node_type(&doc), Some("doc"));
    assert!(
        doc_children(&doc).is_empty(),
        "empty markdown must yield an empty doc, got: {doc}"
    );
}

#[test]
fn markdown_to_prosemirror_whitespace_only_input_is_empty_doc() {
    let doc = markdown_to_prosemirror("   \n\n  \t\n");
    assert_eq!(node_type(&doc), Some("doc"));
    assert!(
        doc_children(&doc).is_empty(),
        "whitespace-only markdown must not create spurious nodes, got: {doc}"
    );
}

#[test]
fn markdown_to_prosemirror_all_heading_levels() {
    for level in 1..=6u64 {
        let hashes = "#".repeat(level as usize);
        let doc = markdown_to_prosemirror(&format!("{hashes} Heading {level}"));
        let heading = find_node(&doc, "heading").expect("heading node");
        assert_eq!(
            heading
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_u64),
            Some(level),
            "wrong level for {hashes}"
        );
    }
}

#[test]
fn markdown_to_prosemirror_multiple_block_paragraphs() {
    let doc = markdown_to_prosemirror("First paragraph.\n\nSecond paragraph.");
    let paragraphs: Vec<&Value> = doc_children(&doc)
        .iter()
        .filter(|node| node_type(node) == Some("paragraph"))
        .collect();
    assert_eq!(paragraphs.len(), 2, "expected two block paragraphs: {doc}");
}

#[test]
fn markdown_to_prosemirror_blockquote_wraps_paragraph() {
    let doc = markdown_to_prosemirror("> quoted line");
    let quote = find_node(&doc, "blockquote").expect("blockquote node");
    let paragraph = find_node(quote, "paragraph").expect("paragraph inside blockquote");
    let text = find_node(paragraph, "text").expect("text inside quote paragraph");
    assert_eq!(
        text.get("text").and_then(Value::as_str),
        Some("quoted line")
    );
}

#[test]
fn markdown_to_prosemirror_ordered_list_records_non_default_start() {
    let doc = markdown_to_prosemirror("3. three\n4. four");
    let list = find_node(&doc, "orderedList").expect("orderedList node");
    assert_eq!(
        list.get("attrs")
            .and_then(|attrs| attrs.get("start"))
            .and_then(Value::as_u64),
        Some(3),
        "non-default start must be recorded: {list}"
    );
    // And the reader renders it back with the right starting index.
    let rendered = prosemirror_to_markdown(&doc);
    assert!(rendered.contains("3. three"), "rendered:\n{rendered}");
    assert!(rendered.contains("4. four"), "rendered:\n{rendered}");
}

#[test]
fn markdown_to_prosemirror_ordered_list_omits_default_start() {
    let doc = markdown_to_prosemirror("1. one\n2. two");
    let list = find_node(&doc, "orderedList").expect("orderedList node");
    let start = list
        .get("attrs")
        .and_then(|attrs| attrs.get("start"))
        .and_then(Value::as_u64);
    assert!(
        start.is_none(),
        "a start of 1 is the default and must be omitted, got: {list}"
    );
}

#[test]
fn markdown_to_prosemirror_mixed_task_and_plain_list_homogenizes_to_task_list() {
    // A list that mixes a task item with a plain item is coerced entirely to a taskList,
    // with the plain item becoming an unchecked task item (so nothing is dropped).
    let doc = markdown_to_prosemirror("- [x] done\n- plain item");
    let task_list = find_node(&doc, "taskList").expect("mixed list homogenized to taskList");
    let items = task_list
        .get("content")
        .and_then(Value::as_array)
        .expect("taskList items");
    assert_eq!(items.len(), 2, "both items must survive: {task_list}");
    assert!(
        items.iter().all(|item| node_type(item) == Some("taskItem")),
        "every item must be a taskItem: {task_list}"
    );
    assert_eq!(
        items[0]
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        items[1]
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool),
        Some(false),
        "the coerced plain item must be an unchecked task"
    );
}

#[test]
fn markdown_to_prosemirror_inline_code_mark_is_innermost() {
    // The reader applies marks in array order, so `code` must come first (innermost) to
    // render **`x`** rather than `**x**`.
    let doc = markdown_to_prosemirror("**`snippet`**");
    let paragraph = find_node(&doc, "paragraph").expect("paragraph node");
    let code_node = paragraph
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|node| mark_types(node).contains(&"code".to_string()))
        .expect("code-marked text node");
    let marks = mark_types(code_node);
    let code_pos = marks.iter().position(|m| m == "code").unwrap();
    let bold_pos = marks.iter().position(|m| m == "bold").unwrap();
    assert!(
        code_pos < bold_pos,
        "code mark must precede bold, marks: {marks:?}"
    );
}

#[test]
fn markdown_to_prosemirror_hard_break_becomes_hard_break_node() {
    // Two trailing spaces before the newline is a CommonMark hard break.
    let doc = markdown_to_prosemirror("line one  \nline two");
    assert!(
        find_node(&doc, "hardBreak").is_some(),
        "hard break node expected: {doc}"
    );
}

#[test]
fn markdown_to_prosemirror_soft_break_joins_with_space() {
    let doc = markdown_to_prosemirror("line one\nline two");
    let paragraph = find_node(&doc, "paragraph").expect("paragraph node");
    let joined: String = paragraph
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|node| node.get("text").and_then(Value::as_str))
        .collect();
    assert_eq!(joined, "line one line two", "soft break should be a space");
}

#[test]
fn markdown_to_prosemirror_nested_ordered_list_inside_bullet() {
    let doc = markdown_to_prosemirror("- outer\n  1. inner");
    let bullet = find_node(&doc, "bulletList").expect("outer bulletList");
    let nested = find_node(bullet, "orderedList").expect("nested orderedList");
    let text = find_node(nested, "text").expect("nested text");
    assert_eq!(text.get("text").and_then(Value::as_str), Some("inner"));
}

#[test]
fn markdown_to_prosemirror_code_block_without_language_has_empty_language() {
    let doc = markdown_to_prosemirror("```\nplain code\n```");
    let code_block = find_node(&doc, "codeBlock").expect("codeBlock node");
    assert_eq!(
        code_block
            .get("attrs")
            .and_then(|attrs| attrs.get("language"))
            .and_then(Value::as_str),
        Some(""),
        "no fence info => empty language: {code_block}"
    );
    let text = find_node(code_block, "text").expect("code text");
    assert_eq!(text.get("text").and_then(Value::as_str), Some("plain code"));
}

#[test]
fn markdown_to_prosemirror_table_body_cells_are_not_headers() {
    let doc = markdown_to_prosemirror("| H1 | H2 |\n| --- | --- |\n| a | b |");
    let table = find_node(&doc, "table").expect("table node");
    let rows: Vec<&Value> = table
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .collect();
    assert_eq!(rows.len(), 2, "one header row + one body row: {table}");
    // Header row uses tableHeader cells.
    assert!(find_node(rows[0], "tableHeader").is_some());
    // Body row uses tableCell (not tableHeader).
    assert!(find_node(rows[1], "tableCell").is_some());
    assert!(
        find_node(rows[1], "tableHeader").is_none(),
        "body row must not contain header cells: {:?}",
        rows[1]
    );
}

#[test]
fn markdown_to_prosemirror_lone_image_is_a_block_node() {
    // A paragraph containing only an image is hoisted so the image is a block child of doc.
    let doc = markdown_to_prosemirror("![alt text](https://example.com/pic.png)");
    let first = doc_children(&doc).first().expect("a top-level node");
    assert_eq!(
        node_type(first),
        Some("image"),
        "lone image should be hoisted to a block node: {doc}"
    );
}

#[test]
fn markdown_to_prosemirror_link_carries_href_and_text() {
    let doc = markdown_to_prosemirror("see [the site](https://example.com/page)");
    let link_node = find_node(&doc, "paragraph")
        .and_then(|p| p.get("content"))
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|node| mark_types(node).contains(&"link".to_string()))
        .cloned()
        .expect("link text node");
    assert_eq!(
        link_node.get("text").and_then(Value::as_str),
        Some("the site")
    );
    let href = link_node
        .get("marks")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|mark| mark.get("type").and_then(Value::as_str) == Some("link"))
        .and_then(|mark| mark.get("attrs"))
        .and_then(|attrs| attrs.get("href"))
        .and_then(Value::as_str);
    assert_eq!(href, Some("https://example.com/page"));
}

#[test]
fn markdown_round_trips_ordered_start_blockquote_and_strike() {
    let source = "> a quote with ~~struck~~ text\n\n5. five\n6. six\n";
    let rendered = prosemirror_to_markdown(&markdown_to_prosemirror(source));
    for needle in ["> a quote", "~~struck~~", "5. five", "6. six"] {
        assert!(
            rendered.contains(needle),
            "missing {needle:?} in:\n{rendered}"
        );
    }
}

// --- @mentions ---

fn all_nodes_of<'a>(value: &'a Value, node_type: &str, out: &mut Vec<&'a Value>) {
    if value.get("type").and_then(Value::as_str) == Some(node_type) {
        out.push(value);
    }
    if let Some(children) = value.get("content").and_then(Value::as_array) {
        for child in children {
            all_nodes_of(child, node_type, out);
        }
    }
}

#[test]
fn markdown_to_prosemirror_user_mention() {
    let doc = markdown_to_prosemirror("Hey [Jane Doe](user:019c-jane) welcome!");
    let mention = find_node(&doc, "mention").expect("mention node");
    let attrs = mention.get("attrs").expect("mention attrs");
    assert_eq!(
        attrs.get("entityType").and_then(Value::as_str),
        Some("user")
    );
    assert_eq!(
        attrs.get("entityId").and_then(Value::as_str),
        Some("019c-jane")
    );
    assert_eq!(attrs.get("label").and_then(Value::as_str), Some("Jane Doe"));
    // A unique id is required (Docmost dedups mentions by it).
    assert!(
        attrs
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty())
    );
    // The mention is inline in the paragraph, alongside the surrounding text.
    let paragraph = find_node(&doc, "paragraph").unwrap();
    let joined: String = paragraph
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|n| n.get("text").and_then(Value::as_str))
        .collect();
    assert!(joined.contains("Hey ") && joined.contains(" welcome!"));
}

#[test]
fn markdown_to_prosemirror_page_mention() {
    let doc = markdown_to_prosemirror("see [Roadmap](page:019c-page)");
    let mention = find_node(&doc, "mention").expect("mention node");
    let attrs = mention.get("attrs").unwrap();
    assert_eq!(
        attrs.get("entityType").and_then(Value::as_str),
        Some("page")
    );
    assert_eq!(
        attrs.get("entityId").and_then(Value::as_str),
        Some("019c-page")
    );
}

#[test]
fn markdown_to_prosemirror_multiple_mentions_get_unique_ids() {
    let doc = markdown_to_prosemirror("[A](user:u1) and [B](user:u2)");
    let mut mentions = Vec::new();
    all_nodes_of(&doc, "mention", &mut mentions);
    assert_eq!(mentions.len(), 2, "both mentions must be produced");
    let id0 = mentions[0].get("attrs").unwrap().get("id").unwrap();
    let id1 = mentions[1].get("attrs").unwrap().get("id").unwrap();
    assert_ne!(
        id0, id1,
        "each mention needs a distinct id (Docmost dedups by id)"
    );
}

#[test]
fn markdown_to_prosemirror_ordinary_link_is_not_a_mention() {
    let doc = markdown_to_prosemirror("[site](https://example.com)");
    assert!(
        find_node(&doc, "mention").is_none(),
        "http link is not a mention"
    );
    // It is a normal link mark instead.
    let paragraph = find_node(&doc, "paragraph").unwrap();
    let has_link = paragraph
        .get("content")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|n| mark_types(n).contains(&"link".to_string()));
    assert!(has_link, "expected a link mark");
}

#[test]
fn mention_round_trips_through_markdown() {
    // mention -> markdown (reader) -> mention (writer), entityId preserved.
    let doc = markdown_to_prosemirror("ping [Jane](user:uid-1)");
    let back = markdown_to_prosemirror(&prosemirror_to_markdown(&doc));
    let mention = find_node(&back, "mention").expect("mention survives round-trip");
    assert_eq!(
        mention
            .get("attrs")
            .and_then(|a| a.get("entityId"))
            .and_then(Value::as_str),
        Some("uid-1")
    );
}

#[test]
fn comment_supported_elements_convert() {
    // The element subset Docmost comments support (StarterKit + link + mention): headings,
    // marks, inline code, lists, blockquote, code block, and mentions.
    let md = "## Heading\n\nSome **bold** *italic* `code` and [Jane](user:u9).\n\n- one\n- two\n\n> quote\n\n```rs\nlet x = 1;\n```";
    let doc = markdown_to_prosemirror(md);
    for node_type in [
        "heading",
        "paragraph",
        "bulletList",
        "blockquote",
        "codeBlock",
        "mention",
    ] {
        assert!(
            find_node(&doc, node_type).is_some(),
            "comment should support {node_type}"
        );
    }
}
