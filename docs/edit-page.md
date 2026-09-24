# `edit_page`: in-place edits

`edit_page` changes part of an existing page without resending the whole body, and without
detaching inline comment threads.

```
get page JSON ──► apply operations to the JSON ──► dry run: diff + updatedAt
                                                └─► write: same JSON back (replace)
```

## Why it exists

`update_page` rebuilds the whole body from Markdown. Markdown has no form for a comment
mark, so every inline comment thread on the page is detached, and blocks nobody touched are
re-serialised (callouts are even dropped by the reader). `edit_page` never goes through
Markdown for content it does not change: it edits the stored ProseMirror JSON and sends it
back. Every node outside an edited range stays equal (`serde_json::Value` equality; key
order in the serialised JSON is not preserved, and is not meaningful).

## Operations

Matching is against the Markdown that `get_page` renders, per block, so a `find` string can
be copied from a `get_page` result. Every match must be unique. Operations apply in order,
each to the result of the previous one. If any operation does not apply, nothing is written.

| `op` | Fields | Effect |
|---|---|---|
| `replace_text` | `find`, `replace` | Replace a span inside one block (paragraph, heading, code block, table cell), or a whole table row given as `\| a \| b \|` (an empty `replace` deletes the row). |
| `insert_blocks` | `anchor`, `position` (`before`/`after`), `markdown` | Insert Markdown blocks next to the top-level block that contains `anchor`. |
| `append_table_row` | `anchor`, `row` | Insert rows (one per line) after the table row that contains `anchor`. |
| `merge_table_rows` | `rows` (two or more anchors) | Merge adjacent rows of one table into the first, on the JSON: its cells stay, and the other rows' last-cell content is appended with one space, marks included. |

Rules the transform enforces:

- **A span may cross marks** (`which **blocks no range**. Use`), but may not start or end
  inside mark syntax (`*blocks`): that fails.
- **Marks that cover the whole match and are not written in `find` are kept.** Replacing
  `nginx` inside a bold, commented span gives bold, commented new text. This is what keeps a
  thread anchored when its exact text is rewritten.
- **A write that removes the last mark of a comment thread is refused**, naming the comment id,
  unless `allow_detaching_comments` is true. A dry run shows the diff and lists that thread.
- **A span that contains an inline node with no Markdown form** (it would be deleted
  silently) is refused.
- **Whole-row replace:** the first replacement row maps onto the old row, and a cell whose
  Markdown is unchanged is kept as is, marks included. Further rows, and appended rows, get
  fresh cells with the template cell's type and attrs. A comment mark is never copied. A row
  appended after a header row gets `tableCell` cells.
- **A hard line break is `\n` in `find`.** Inside a table row, `get_page` shows it as a space;
  a `find` inside one cell must use `\n`.
- **Code blocks take `replace` literally**, with no Markdown parsing.
- `insert_blocks` anchors match only the top-level block's rendering; text inside a callout
  (which the reader does not render) cannot be an anchor.

## Write protocol and concurrency

1. Call with the default `dry_run: true`. The result is a diff per operation, the count of
   unchanged top-level blocks, and the page's `updatedAt`.
2. Call again with `dry_run: false` and `expected_updated_at` set to that value. The write is
   refused if `updatedAt` changed. A write without `expected_updated_at` is refused.
3. After the write the page is re-fetched and compared with the document that was sent. The
   server adds default attributes to new nodes (block `indent`, link `rel`/`target`/`class`/
   `title`/`internal`, an empty mark `attrs`) and joins adjacent text nodes that carry the same
   marks; the comparison accepts those two differences only, so any other difference (text,
   marks, an added block) is reported as a warning.

How the write reaches the page (Docmost source, v0.95.0,
`collaboration/collaboration.handler.ts`): `POST /api/pages/update` with
`operation: "replace"` opens a Hocuspocus direct connection and, in one transaction, deletes
the whole Yjs fragment and applies a Yjs document built from the JSON. Open editors receive
that change live. `updatedAt` and the stored `content` change when the document is stored:
at once for the REST write, but debounced (10 s, at most 45 s) for edits typed in an editor.

What the `updatedAt` guard covers, and what it does not:

- **Covered:** any change stored before the dry run was read: other REST writes, and editor
  edits older than the store debounce.
- **Not covered (residual risk):** text typed in an open editor in the last 10–45 s before the
  write. It is not in the stored content yet, so the dry run cannot see it, and the replace
  deletes it. REST exposes no editor presence, so the tool cannot detect this. Prefer a quiet
  moment for shared pages.

## Measured on 2026-09-24 (live instance, scratch page)

- Two inline comments created in the editor; `edit_page` replaced a sentence next to one
  thread, replaced a row, appended a row and inserted a section. Afterwards both comment ids
  were still present as marks (a probe dry run that removes each thread's text is refused and
  names it), and `get_comments` still lists both selections.
- With no editor open, the stored content equalled the sent document at once and 45 s later.
- **With the page open in a browser, the editor added an empty paragraph at the top a few
  seconds after each write.** Closing the tab removed the effect. The mechanism (likely the
  editor reacting to the momentarily empty fragment) is a hypothesis, not verified. The tool
  output says so after every write.
- Read-only dry runs of 20 operations against three real pages (171 k, 44 k and 37 k
  characters of Markdown): every operation matched exactly once, no thread would be detached,
  and only the edited blocks changed (266/271, 61/67, 48/51 top-level blocks unchanged).
