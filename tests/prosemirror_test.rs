use docmost_local_mcp::prosemirror::prosemirror_to_markdown;
use serde_json::json;

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
