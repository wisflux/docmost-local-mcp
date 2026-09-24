//! In-place edits on a ProseMirror document (`edit_page`). Only the matched range changes:
//! every other node and mark — comment marks, callouts, mentions, attrs — is kept as is.
//!
//! Matching runs against the same Markdown the reader renders for `get_page`, per block,
//! so a caller can copy a `find` string straight from a `get_page` result.

use std::collections::BTreeSet;
use std::slice::from_ref;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use super::markdown_to_prosemirror;
use super::reader::{convert_nodes, extract_text};
use crate::types::{EditOperation, InsertPosition};

/// Nodes whose children are inline content, the unit a `replace_text` span must lie in.
const TEXTBLOCKS: [&str; 4] = ["paragraph", "heading", "codeBlock", "detailsSummary"];
/// Marks the reader renders as Markdown syntax; all other marks (comment, highlight, …)
/// are invisible in a `find` string.
const VISIBLE_MARKS: [&str; 5] = ["bold", "italic", "code", "strike", "link"];

/// One applied operation, rendered as Markdown before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditChange {
    pub description: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone)]
pub struct EditOutcome {
    pub doc: Value,
    pub changes: Vec<EditChange>,
    /// Comment ids whose last mark the edits removed (their threads would be detached).
    pub detached_comments: Vec<String>,
    /// Top-level blocks carried over unchanged, out of the original count.
    pub unchanged_blocks: (usize, usize),
}

/// Apply `operations` in order. Fails on the first one that does not apply exactly once, so a
/// caller never writes a partly edited document.
pub fn apply_edits(doc: &Value, operations: &[EditOperation]) -> Result<EditOutcome> {
    if doc.get("type").and_then(Value::as_str) != Some("doc") {
        bail!("the page content is not a ProseMirror document");
    }
    let mut edited = doc.clone();
    let mut changes = Vec::new();
    for (index, operation) in operations.iter().enumerate() {
        let change = match operation {
            EditOperation::ReplaceText { find, replace } => {
                replace_text(&mut edited, find, replace)
            }
            EditOperation::InsertBlocks {
                anchor,
                position,
                markdown,
            } => insert_blocks(&mut edited, anchor, *position, markdown),
            EditOperation::MergeTableRows { rows } => merge_table_rows(&mut edited, rows),
            EditOperation::AppendTableRow { anchor, row } => {
                append_table_row(&mut edited, anchor, row)
            }
        };
        match change {
            Ok(change) => changes.push(change),
            Err(error) => bail!("operation {} failed, nothing written: {error}", index + 1),
        }
    }

    let after = comment_ids(&edited);
    let detached_comments = comment_ids(doc).difference(&after).cloned().collect();
    let unchanged_blocks = (
        count_unchanged(children(doc), children(&edited)),
        children(doc).len(),
    );
    Ok(EditOutcome {
        doc: edited,
        changes,
        detached_comments,
        unchanged_blocks,
    })
}

fn replace_text(doc: &mut Value, find: &str, replace: &str) -> Result<EditChange> {
    if find.is_empty() {
        bail!("`find` is empty");
    }
    let mut inline_hits = Vec::new();
    let mut row_hits = Vec::new();
    for (pointer, node) in walk(doc) {
        match node_type(node) {
            Some(kind) if TEXTBLOCKS.contains(&kind) => {
                let markdown = extract_text(children(node));
                inline_hits.extend(
                    find_all(&markdown, find)
                        .into_iter()
                        .map(|at| (pointer.clone(), at)),
                );
            }
            Some("tableRow") if row_markdown(node) == find => row_hits.push(pointer),
            _ => {}
        }
    }
    let count = inline_hits.len() + row_hits.len();
    if count != 1 {
        bail!(
            "`find` matches {count} times, expected exactly 1: {}",
            preview(find)
        );
    }

    if let Some(pointer) = row_hits.pop() {
        let row = doc.pointer(&pointer).cloned().unwrap_or(Value::Null);
        // An empty replacement deletes the row.
        let rows = if replace.trim().is_empty() {
            Vec::new()
        } else {
            build_rows(&row, replace, false)?
        };
        let after = rows.iter().map(row_markdown).collect::<Vec<_>>().join("\n");
        splice_sibling(doc, &pointer, 0, rows);
        return Ok(EditChange {
            description: format!("replace_text: table row at {pointer}"),
            before: find.to_string(),
            after,
        });
    }

    let (pointer, start) = inline_hits.pop().expect("exactly one hit");
    let block = doc.pointer_mut(&pointer).expect("pointer from walk");
    let before = extract_text(children(block));
    splice_inline(block, start, find, replace)?;
    Ok(EditChange {
        description: format!(
            "replace_text: {} at {pointer}",
            node_type(block).unwrap_or("block")
        ),
        before,
        after: extract_text(children(block)),
    })
}

/// Replace `find`, which starts at byte `start` of the block's rendered Markdown, with
/// `replace`. Text nodes are split at the match boundaries; nodes outside stay untouched.
fn splice_inline(block: &mut Value, start: usize, find: &str, replace: &str) -> Result<()> {
    let kids = children(block).to_vec();
    let spans = inline_spans(&kids);
    let end = start + find.len();
    let first = spans
        .iter()
        .position(|span| start < span.end)
        .expect("match inside block");
    let last = spans
        .iter()
        .position(|span| end > span.start && end <= span.end)
        .expect("match inside block");
    let start_cut = spans[first].cut(start, false);
    let end_cut = spans[last].cut(end, true);

    let (mut before, mut covered, mut after) = (Vec::new(), Vec::new(), Vec::new());
    for (index, (node, span)) in kids.iter().zip(&spans).enumerate() {
        let lo = match index.cmp(&first) {
            std::cmp::Ordering::Less => span.len,
            std::cmp::Ordering::Equal => start_cut,
            std::cmp::Ordering::Greater => 0,
        };
        let hi = match index.cmp(&last) {
            std::cmp::Ordering::Less => span.len,
            std::cmp::Ordering::Equal => end_cut,
            std::cmp::Ordering::Greater => 0,
        }
        .max(lo);
        before.extend(piece(node, 0, lo));
        covered.extend(piece(node, lo, hi));
        after.extend(piece(node, hi, span.len));
    }

    if let Some(hidden) = covered
        .iter()
        .find(|node| extract_text(from_ref(node)).is_empty())
    {
        bail!(
            "the match contains a `{}` node that has no Markdown form and would be deleted",
            node_type(hidden).unwrap_or("unknown")
        );
    }
    let common = common_marks(&covered);
    let inherited: Vec<Value> = if extract_text(&covered) == find {
        common
            .into_iter()
            .filter(|mark| !is_visible(mark))
            .collect()
    } else if extract_text(&strip_visible(&covered, &common)) == find {
        common
    } else {
        bail!(
            "`find` starts or ends inside Markdown syntax (between a `**`, `` ` `` or `[` and \
             its text); widen or narrow it to whole marks: {}",
            preview(find)
        );
    };

    let new_nodes = if node_type(block) == Some("codeBlock") {
        // Code is literal text: no Markdown parsing, no marks.
        (!replace.is_empty())
            .then(|| json!({ "type": "text", "text": replace }))
            .into_iter()
            .collect()
    } else {
        with_marks(inline_nodes(replace)?, &inherited)
    };
    let content: Vec<Value> = before.into_iter().chain(new_nodes).chain(after).collect();
    block["content"] = Value::Array(content);
    Ok(())
}

/// Where a child's rendering sits in the block's Markdown. `len` is the text length for a
/// text node, 1 for an atom (mention, hard break).
struct Span {
    start: usize,
    end: usize,
    prefix: usize,
    len: usize,
    is_text: bool,
}

impl Span {
    /// Map a Markdown byte offset to an offset inside this node (clamped to the node).
    fn cut(&self, at: usize, is_end: bool) -> usize {
        if self.is_text {
            at.saturating_sub(self.start + self.prefix).min(self.len)
        } else if is_end {
            usize::from(at >= self.end)
        } else {
            usize::from(at > self.start)
        }
    }
}

fn inline_spans(kids: &[Value]) -> Vec<Span> {
    let mut position = 0;
    kids.iter()
        .map(|node| {
            let width = extract_text(from_ref(node)).len();
            let text = node
                .get("text")
                .and_then(Value::as_str)
                .filter(|_| node_type(node) == Some("text"));
            let (prefix, len, is_text) = match text {
                Some(text) => {
                    // The reader wraps the text in mark syntax exactly once; a sentinel shows
                    // how many bytes of syntax come before it.
                    let mut probe = node.clone();
                    probe["text"] = json!("\u{0}");
                    let prefix = extract_text(from_ref(&probe)).find('\u{0}').unwrap_or(0);
                    (prefix, text.len(), true)
                }
                None => (0, 1, false),
            };
            let span = Span {
                start: position,
                end: position + width,
                prefix,
                len,
                is_text,
            };
            position += width;
            span
        })
        .collect()
}

/// The part `[lo, hi)` of a node: a text slice with the same marks, or the whole atom.
fn piece(node: &Value, lo: usize, hi: usize) -> Option<Value> {
    if lo >= hi {
        return None;
    }
    let mut piece = node.clone();
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        if lo == 0 && hi == text.len() {
            return Some(piece);
        }
        piece["text"] = json!(&text[lo..hi]);
    }
    Some(piece)
}

fn marks_of(node: &Value) -> &[Value] {
    node.get("marks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn common_marks(nodes: &[Value]) -> Vec<Value> {
    let Some((first, rest)) = nodes.split_first() else {
        return Vec::new();
    };
    marks_of(first)
        .iter()
        .filter(|mark| rest.iter().all(|node| marks_of(node).contains(mark)))
        .cloned()
        .collect()
}

fn is_visible(mark: &Value) -> bool {
    VISIBLE_MARKS.contains(&node_type(mark).unwrap_or(""))
}

fn strip_visible(nodes: &[Value], common: &[Value]) -> Vec<Value> {
    nodes
        .iter()
        .map(|node| {
            let mut node = node.clone();
            if let Some(marks) = node.get_mut("marks").and_then(Value::as_array_mut) {
                marks.retain(|mark| !(is_visible(mark) && common.contains(mark)));
            }
            node
        })
        .collect()
}

/// Add `inherited` marks to every text node that has no mark of the same type yet.
fn with_marks(nodes: Vec<Value>, inherited: &[Value]) -> Vec<Value> {
    nodes
        .into_iter()
        .map(|mut node| {
            if node_type(&node) != Some("text") || inherited.is_empty() {
                return node;
            }
            let mut marks = marks_of(&node).to_vec();
            for mark in inherited {
                if !marks.iter().any(|own| node_type(own) == node_type(mark)) {
                    marks.push(mark.clone());
                }
            }
            node["marks"] = Value::Array(marks);
            node
        })
        .collect()
}

/// Inline Markdown → inline nodes. Leading and trailing whitespace is kept (the Markdown
/// parser would trim it).
fn inline_nodes(markdown: &str) -> Result<Vec<Value>> {
    let core = markdown.trim();
    let lead = &markdown[..markdown.len() - markdown.trim_start().len()];
    let trail = &markdown[markdown.trim_end().len()..];
    if core.is_empty() {
        return Ok((!markdown.is_empty())
            .then(|| json!({ "type": "text", "text": markdown }))
            .into_iter()
            .collect());
    }
    let parsed = markdown_to_prosemirror(core);
    let blocks = children(&parsed);
    if blocks.len() != 1 || node_type(&blocks[0]) != Some("paragraph") {
        bail!(
            "`replace` must be inline Markdown (one paragraph); use insert_blocks for new blocks"
        );
    }
    let space = |text: &str| (!text.is_empty()).then(|| json!({ "type": "text", "text": text }));
    Ok(space(lead)
        .into_iter()
        .chain(children(&blocks[0]).iter().cloned())
        .chain(space(trail))
        .collect())
}

fn insert_blocks(
    doc: &mut Value,
    anchor: &str,
    position: InsertPosition,
    markdown: &str,
) -> Result<EditChange> {
    if anchor.is_empty() {
        bail!("`anchor` is empty");
    }
    let hits: Vec<usize> = children(doc)
        .iter()
        .enumerate()
        .flat_map(|(index, block)| {
            find_all(&convert_nodes(from_ref(block), 0), anchor)
                .into_iter()
                .map(move |_| index)
        })
        .collect();
    if hits.len() != 1 {
        bail!(
            "`anchor` matches {} times in top-level blocks, expected exactly 1: {}",
            hits.len(),
            preview(anchor)
        );
    }
    let blocks = children(&markdown_to_prosemirror(markdown)).to_vec();
    if blocks.is_empty() {
        bail!("`markdown` produced no blocks");
    }
    let index = hits[0] + usize::from(position == InsertPosition::After);
    let anchor_block = convert_nodes(from_ref(&children(doc)[hits[0]]), 0);
    let after = convert_nodes(&blocks, 0);
    if let Some(content) = doc.get_mut("content").and_then(Value::as_array_mut) {
        content.splice(index..index, blocks);
    }
    Ok(EditChange {
        description: format!(
            "insert_blocks: {} block /content/{} {}",
            if position == InsertPosition::After {
                "after"
            } else {
                "before"
            },
            hits[0],
            preview(&anchor_block)
        ),
        before: String::new(),
        after,
    })
}

fn append_table_row(doc: &mut Value, anchor: &str, row: &str) -> Result<EditChange> {
    let pointer = &unique_row(doc, anchor)?;
    let template = doc.pointer(pointer).cloned().unwrap_or(Value::Null);
    let rows = build_rows(&template, row, true)?;
    let after = rows.iter().map(row_markdown).collect::<Vec<_>>().join("\n");
    splice_sibling(doc, pointer, 1, rows);
    Ok(EditChange {
        description: format!(
            "append_table_row: after row {pointer} {}",
            preview(&row_markdown(&template))
        ),
        before: String::new(),
        after,
    })
}

fn merge_table_rows(doc: &mut Value, anchors: &[String]) -> Result<EditChange> {
    if anchors.len() < 2 {
        bail!("`rows` needs at least two rows");
    }
    let pointers = anchors
        .iter()
        .map(|anchor| unique_row(doc, anchor))
        .collect::<Result<Vec<_>>>()?;
    let split: Vec<(&str, usize)> = pointers
        .iter()
        .map(|pointer| {
            let (parent, index) = pointer.rsplit_once('/').expect("child pointer");
            (parent, index.parse().expect("numeric index"))
        })
        .collect();
    let (parent, first) = split[0];
    if split
        .iter()
        .enumerate()
        .any(|(offset, (p, index))| *p != parent || *index != first + offset)
    {
        bail!("`rows` must name adjacent rows of one table, in order");
    }
    let rows: Vec<Value> = pointers
        .iter()
        .map(|p| doc.pointer(p).cloned().unwrap_or(Value::Null))
        .collect();
    let before = rows.iter().map(row_markdown).collect::<Vec<_>>().join("\n");
    let mut inline = Vec::new();
    for (offset, row) in rows.iter().enumerate() {
        let paragraph = match children(row).last().map(children) {
            Some([paragraph]) if node_type(paragraph) == Some("paragraph") => paragraph,
            _ => bail!(
                "the last cell of row {} is not a single paragraph",
                anchors[offset]
            ),
        };
        if offset > 0 {
            inline.push(json!({ "type": "text", "text": " " }));
        }
        inline.extend(children(paragraph).iter().cloned());
    }
    let mut merged = rows[0].clone();
    let last = children(&merged).len() - 1;
    merged["content"][last]["content"][0]["content"] = Value::Array(inline);
    let after = row_markdown(&merged);
    if let Some(siblings) = doc.pointer_mut(parent).and_then(Value::as_array_mut) {
        siblings.splice(first..first + rows.len(), [merged]);
    }
    Ok(EditChange {
        description: format!("merge_table_rows: {} rows at {}", rows.len(), pointers[0]),
        before,
        after,
    })
}

/// The pointer of the one table row whose Markdown contains `anchor`.
fn unique_row(doc: &Value, anchor: &str) -> Result<String> {
    if anchor.is_empty() {
        bail!("`anchor` is empty");
    }
    let hits: Vec<String> = walk(doc)
        .into_iter()
        .filter(|(_, node)| node_type(node) == Some("tableRow"))
        .flat_map(|(pointer, node)| {
            find_all(&row_markdown(node), anchor)
                .into_iter()
                .map(move |_| pointer.clone())
        })
        .collect();
    match hits.as_slice() {
        [pointer] => Ok(pointer.clone()),
        _ => bail!(
            "`anchor` matches {} times in table rows, expected exactly 1: {}",
            hits.len(),
            preview(anchor)
        ),
    }
}

/// Build rows from Markdown lines, shaped like `template` (cell types and attrs). Without
/// `fresh`, the first line maps onto `template` itself: a cell whose Markdown is unchanged
/// is kept as is, marks included. Every other row gets new cells, so no comment mark is
/// ever copied.
fn build_rows(template: &Value, markdown: &str, fresh: bool) -> Result<Vec<Value>> {
    let old_cells = children(template);
    let header_row = old_cells
        .iter()
        .all(|cell| node_type(cell) == Some("tableHeader"));
    let lines: Vec<&str> = markdown
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() {
        bail!("no table row given");
    }
    lines
        .iter()
        .enumerate()
        .map(|(line_index, line)| {
            let parsed = parse_row(line, old_cells.len())?;
            let cells = parsed
                .into_iter()
                .zip(old_cells)
                .map(|(new, old)| {
                    let keeps_old = !fresh && line_index == 0;
                    if keeps_old && cell_markdown(&new) == cell_markdown(old) {
                        return old.clone();
                    }
                    let mut cell = old.clone();
                    if !keeps_old && header_row {
                        cell["type"] = json!("tableCell");
                    }
                    cell["content"] = new["content"].clone();
                    cell
                })
                .collect();
            let mut row = template.clone();
            row["content"] = Value::Array(cells);
            Ok(row)
        })
        .collect()
}

/// Parse one `| a | b |` line into `cells` table cells.
fn parse_row(line: &str, cells: usize) -> Result<Vec<Value>> {
    let table = markdown_to_prosemirror(&format!("{line}\n|{}\n", " --- |".repeat(cells)));
    let rows = match children(&table) {
        [node] if node_type(node) == Some("table") => children(node),
        _ => &[],
    };
    match rows {
        // pulldown-cmark only accepts a header whose cell count matches the delimiter row.
        [row] => Ok(children(row)
            .iter()
            .map(|cell| {
                let mut cell = cell.clone();
                if children(&cell).is_empty() {
                    cell["content"] = json!([{ "type": "paragraph" }]);
                }
                cell
            })
            .collect()),
        _ => bail!(
            "expected one Markdown table row with {cells} cells: {}",
            preview(line)
        ),
    }
}

/// Replace the node at `pointer` with `nodes`, keeping it when `keep` is 1 (insert after).
fn splice_sibling(doc: &mut Value, pointer: &str, keep: usize, nodes: Vec<Value>) {
    let (parent, index) = pointer.rsplit_once('/').expect("child pointer");
    let index: usize = index.parse().expect("numeric index");
    if let Some(siblings) = doc.pointer_mut(parent).and_then(Value::as_array_mut) {
        siblings.splice(index + keep..index + 1, nodes);
    }
}

/// A cell as the reader renders it inside a table row.
fn cell_markdown(cell: &Value) -> String {
    convert_nodes(children(cell), 0)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A table row as the reader renders it: `| a | b |`.
fn row_markdown(row: &Value) -> String {
    let cells: Vec<String> = children(row)
        .iter()
        .filter(|cell| matches!(node_type(cell), Some("tableCell" | "tableHeader")))
        .map(cell_markdown)
        .collect();
    if cells.is_empty() {
        return String::new();
    }
    format!("| {} |", cells.join(" | "))
}

/// Every node with its JSON pointer, depth first.
fn walk(root: &Value) -> Vec<(String, &Value)> {
    fn visit<'a>(node: &'a Value, pointer: String, out: &mut Vec<(String, &'a Value)>) {
        for (index, child) in children(node).iter().enumerate() {
            visit(child, format!("{pointer}/content/{index}"), out);
        }
        out.push((pointer, node));
    }
    let mut out = Vec::new();
    visit(root, String::new(), &mut out);
    out
}

/// Start offsets of every occurrence, overlapping ones included.
fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    (0..haystack.len())
        .filter(|&at| haystack.is_char_boundary(at) && haystack[at..].starts_with(needle))
        .collect()
}

fn comment_ids(root: &Value) -> BTreeSet<String> {
    walk(root)
        .into_iter()
        .flat_map(|(_, node)| marks_of(node).iter())
        .filter(|mark| node_type(mark) == Some("comment"))
        .filter_map(|mark| mark["attrs"]["commentId"].as_str().map(str::to_string))
        .collect()
}

/// Top-level blocks of `before` found unchanged, in order, in `after`.
// ponytail: greedy in-order match, O(n²) on block count; fine for pages of a few hundred blocks.
fn count_unchanged(before: &[Value], after: &[Value]) -> usize {
    let mut next = 0;
    before
        .iter()
        .filter(|block| {
            match after[next..]
                .iter()
                .position(|candidate| candidate == *block)
            {
                Some(offset) => {
                    next += offset + 1;
                    true
                }
                None => false,
            }
        })
        .count()
}

fn preview(text: &str) -> String {
    let short: String = text.chars().take(80).collect();
    if short.len() < text.len() {
        format!("{short:?}…")
    } else {
        format!("{short:?}")
    }
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

/// Adjacent text nodes with the same marks, joined (the server stores them joined).
fn join_text_runs(nodes: &[Value]) -> Vec<Value> {
    let mut joined: Vec<Value> = Vec::with_capacity(nodes.len());
    for node in nodes {
        if let Some(last) = joined.last_mut() {
            let both_text = node_type(last) == Some("text") && node_type(node) == Some("text");
            if both_text && marks_of(last) == marks_of(node) {
                let text = format!(
                    "{}{}",
                    last["text"].as_str().unwrap_or(""),
                    node["text"].as_str().unwrap_or("")
                );
                last["text"] = Value::String(text);
                continue;
            }
        }
        joined.push(node.clone());
    }
    joined
}

/// Whether the stored document is what was sent, allowing only the `attrs` keys the server
/// adds as defaults (measured: block `indent`, link `rel`/`target`/…, empty mark `attrs`), and
/// adjacent text nodes with the same marks joined into one.
/// Every text, type, mark, child and attr value that was sent must be equal.
pub fn stored_matches_sent(sent: &Value, stored: &Value) -> bool {
    match (sent, stored) {
        (Value::Object(sent), Value::Object(stored)) => {
            let attrs_ok = match (sent.get("attrs"), stored.get("attrs")) {
                (Some(Value::Object(a)), Some(Value::Object(b))) => a
                    .iter()
                    .all(|(key, value)| b.get(key).is_some_and(|v| stored_matches_sent(value, v))),
                (None, Some(Value::Object(_)) | None) => true,
                (a, b) => a == b,
            };
            attrs_ok
                && sent.len()
                    + usize::from(!sent.contains_key("attrs") && stored.contains_key("attrs"))
                    == stored.len()
                && sent
                    .iter()
                    .filter(|(key, _)| *key != "attrs")
                    .all(|(key, value)| {
                        stored
                            .get(key)
                            .is_some_and(|v| stored_matches_sent(value, v))
                    })
        }
        (Value::Array(sent), Value::Array(stored)) => {
            let (sent, stored) = (join_text_runs(sent), join_text_runs(stored));
            sent.len() == stored.len()
                && sent
                    .iter()
                    .zip(&stored)
                    .all(|(a, b)| stored_matches_sent(a, b))
        }
        _ => sent == stored,
    }
}
