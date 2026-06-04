# Write Tools: `create_page` & `update_page`

This document describes the two write tools added to the Docmost MCP server, how
they map Markdown to Docmost's page format, the exact Docmost API they call, and
the caveats that matter when running against a self-hosted instance.

> Status: implemented and tested. Branch `feat/write-tools-create-update-page`.
> The rest of the server remains read-only; these are the first write tools.

## Overview

| Tool | Purpose | Docmost endpoint |
| --- | --- | --- |
| `create_page` | Create a new page in a space from Markdown | `POST /api/pages/create` |
| `update_page` | Update an existing page's title and/or Markdown body | `POST /api/pages/update` |

Both tools accept **Markdown** for the page body. The server converts that
Markdown into Docmost's native **ProseMirror/Tiptap JSON** document before
sending it, so callers never deal with ProseMirror directly. Both tools are
marked `read_only_hint = false`.

## Tool reference

### `create_page`

| Input | Required | Description |
| --- | --- | --- |
| `space_id` | yes | Docmost space ID (UUID) to create the page in |
| `title` | yes | Page title |
| `markdown` | no | Page body as Markdown (converted to ProseMirror) |
| `parent_page_id` | no | Parent page ID to nest under — must be in the **same space** |

Returns a confirmation with the new page's **ID**, **slug ID**, and space ID.
Use the slug ID with `get_page`, or the ID as `page_id` for `update_page`.

Example (MCP arguments):

```json
{
  "space_id": "018f...uuid",
  "title": "Release Notes",
  "markdown": "# v1.0\n\n- First **stable** release\n- See [docs](https://example.com)"
}
```

### `update_page`

| Input | Required | Description |
| --- | --- | --- |
| `page_id` | yes | Page ID **or** slug ID to update |
| `title` | no | New title (omit to leave unchanged) |
| `markdown` | no | New body as Markdown; **replaces** the existing content (omit to leave unchanged) |

Returns a confirmation with the page's ID, slug ID, and title.

> Content changes are applied through Docmost's collaborative editor and may take
> a moment to fully persist after the call returns (see
> [Update semantics](#update-semantics-important)). The tool reports success from
> the returned id/slug/title, not by echoing content back.

## How it works

```
MCP client
  → create_page / update_page (src/server/tools.rs)
      → markdown_to_prosemirror(markdown)   (src/prosemirror.rs)
      → DocmostClient::create_page / update_page  (src/docmost_client.rs)
          → POST {base_url}/api/pages/{create|update}   (bearer auth, 401-retry)
      → format_created_page / format_updated_page  (src/server/render.rs)
  ← Markdown confirmation string
```

### Markdown → ProseMirror conversion

`markdown_to_prosemirror` (in [`src/prosemirror.rs`](../src/prosemirror.rs)) is
the inverse of the existing `prosemirror_to_markdown` reader and lives in the
same module so the two stay in sync. It is built on the **`pulldown-cmark`**
CommonMark parser (with tables, strikethrough, and task lists enabled) and walks
the event stream into a `{ "type": "doc", "content": [...] }` tree.

Supported nodes and marks (round-trip with the reader):

- **Blocks:** paragraph, heading (`level`), bullet list, ordered list (`start`
  preserved), task list (`checked`), list item, code block (`language`),
  blockquote, horizontal rule, table (header/body cells), image (`src`, `alt`).
- **Inline marks:** bold, italic, inline code, strikethrough, link (`href`),
  hard break.

Important detail: Docmost uses **Tiptap-style mark names** — `bold`/`italic`,
**not** `strong`/`em`. The converter emits those exact names; emitting the wrong
ones would silently break rendering and round-tripping.

Not produced: the `embed` node has no Markdown form, so it is never emitted on
the write path (it is still read on the get path).

## Docmost API details (verified)

Verified against the open-source Docmost server (`docmost/docmost`, `main`
branch). Both endpoints sit under the global `/api` prefix and return HTTP 200
with the envelope `{ "data": ..., "success": true, "status": 200 }`.

### `POST /api/pages/create`

| Field | Required | Notes |
| --- | --- | --- |
| `spaceId` | yes | UUID; the only required field |
| `title` | no | Plain text |
| `parentPageId` | no | Must be a non-deleted page in the **same** space |
| `content` | no | The ProseMirror document **object** (see below) |
| `format` | no | Defaults to `json` |

On create, Docmost persists the content fully (content column + collaborative
ydoc), so a created page shows its body immediately.

### `POST /api/pages/update`

| Field | Required | Notes |
| --- | --- | --- |
| `pageId` | yes | Accepts a UUID **or** a slug ID |
| `title` | no | Omit to leave unchanged |
| `content` | no | ProseMirror document object |
| `operation` | conditionally | `append` \| `prepend` \| `replace` — **required when sending content** |
| `format` | conditionally | Defaults to `json`, but send it explicitly with content |

`spaceId` and `parentPageId` are accepted by the DTO but **ignored** by update
(use Docmost's move / move-to-space endpoints to relocate a page).

### The `content` field — object, not a string

With `format` defaulting to `json`, Docmost uses `content` **verbatim** as the
ProseMirror document. It must be a JSON **object**:

```json
{ "type": "doc", "content": [ { "type": "paragraph", "content": [ { "type": "text", "text": "hi" } ] } ] }
```

A stringified JSON would fail Docmost's validation (`400 Invalid content
format`). The client embeds the object directly and never stringifies it, and
never sends `format` on create (the default is correct).

### Update semantics (important)

To change a page body via update, Docmost requires `content` **and**
`operation` **and** `format` to all be present — and `operation` has **no
default**. So `update_page` always sends `operation: "replace"` and
`format: "json"` whenever content is provided. Omitting `operation` would make
the body change silently no-op.

Update routes the change through Docmost's **Yjs collaborative editor**
(`updatePageContent`), not a direct DB write. This merges correctly with
concurrent editors, but persistence back to the stored content column happens via
the Hocuspocus/Yjs pipeline and **can be asynchronous** — the value returned in
the immediate response may not yet reflect the change.

## Version compatibility caveat

The `operation`/`format` fields and the Yjs-based update path are relatively
**new** in Docmost. An older self-hosted build may instead do a direct content
write and may not accept `operation`/`format`. If `update_page` content changes
do not take effect on your instance, confirm `POST /api/pages/update` against
your running Docmost version. The `create` endpoint and body shape are stable.

## Permissions

Writes require an authenticated user with the right Docmost permissions:

- create a root page → space "create" permission;
- create a child page → edit permission on the parent;
- update → page-level edit permission.

Insufficient permission returns `403`, surfaced to the caller as a clear error
(the server does not panic).

## Files changed

| File | Change |
| --- | --- |
| [`Cargo.toml`](../Cargo.toml) | Add `pulldown-cmark` (no default features) |
| [`src/prosemirror.rs`](../src/prosemirror.rs) | Add `markdown_to_prosemirror` + builder |
| [`src/types.rs`](../src/types.rs) | Add `CreatePageInput`, `UpdatePageInput` |
| [`src/docmost_client.rs`](../src/docmost_client.rs) | Add `create_page`, `update_page` client methods |
| [`src/server/tools.rs`](../src/server/tools.rs) | Add the two `#[tool]` methods |
| [`src/server/render.rs`](../src/server/render.rs) | Add `format_created_page`, `format_updated_page` |
| [`src/server.rs`](../src/server.rs) | Update the server instructions string |
| `tests/` | `docmost_write_test.rs` (request-body assertions) + converter tests in `prosemirror_test.rs` + tool coverage in `mcp_server_test.rs` + `live_e2e_test.rs` (gated live test) |
| `scripts/` | `e2e-live.sh` runner + `e2e-live.env.example` template |

## Tests

- **Converter** (`tests/prosemirror_test.rs`): per-node/mark unit tests, plus a
  full round-trip and regression tests for inline-code-inside-bold ordering,
  mixed task/plain lists, ordered-list start index, and empty input.
- **Client** (`tests/docmost_write_test.rs`): a mock Docmost asserts the exact
  request bodies — content is an object, optional fields are omitted when absent,
  and update sends `operation` + `format` only when content is present.
- **Tool surface** (`tests/mcp_server_test.rs`): `create_page`/`update_page` are
  registered with the expected required-field schemas.

Run locally (this project is edition 2024 → needs rustc ≥ 1.85; use
`--no-default-features` if GTK/WebKitGTK system libs for the native webview are
not installed — see [`CLAUDE.md`](../CLAUDE.md)):

```bash
cargo test --no-default-features
```

### What the automated tests do and do not cover

| Layer | Verifies | Hits a real Docmost? |
| --- | --- | --- |
| Unit (`prosemirror_test.rs`) | Markdown↔ProseMirror conversion + round-trip | No |
| Client (`docmost_write_test.rs`) | Exact HTTP request body, against a **mock** axum server | No (fake server) |
| Tool surface (`mcp_server_test.rs`) | Tools registered with the right schemas | No |

These prove *"the client sends what the verified Docmost API expects"* — but **not**
that your running server accepts it and a page actually appears. That last mile is
the live E2E test below.

## End-to-end testing against a real Docmost

A gated live test ([`tests/live_e2e_test.rs`](../tests/live_e2e_test.rs)) walks the
full path against a real server:

1. **headless login** with email/password (no interactive browser);
2. `list_spaces` → pick a space (or use `DOCMOST_SPACE_ID`);
3. `create_page` with a sample Markdown document (headings, marks, lists, task
   lists, code block, table);
4. `get_page` → assert the body round-trips back to the expected Markdown;
5. `update_page` (new title + body), then **`get_page` re-polled** up to ~10× to
   account for Docmost's asynchronous Yjs persistence;
6. report the created page's slug/id.

It is marked `#[ignore]`, so it never runs in CI or a normal `cargo test`, and it
reads all credentials from the environment at run time — nothing secret is committed.

### How to run it

```bash
# Option A: a local, git-ignored env file
cp scripts/e2e-live.env.example .env.e2e
$EDITOR .env.e2e          # set DOCMOST_BASE_URL / DOCMOST_EMAIL / DOCMOST_PASSWORD
./scripts/e2e-live.sh

# Option B: inline env, no file
DOCMOST_BASE_URL=https://docs.example.com \
DOCMOST_EMAIL=you@example.com \
DOCMOST_PASSWORD=secret \
cargo test --test live_e2e_test --no-default-features -- --ignored --nocapture
```

Environment variables:

| Var | Required | Meaning |
| --- | --- | --- |
| `DOCMOST_BASE_URL` | yes | Your Docmost base URL |
| `DOCMOST_EMAIL` / `DOCMOST_PASSWORD` | yes | Login (used headlessly; never stored in the repo) |
| `DOCMOST_SPACE_ID` | no | Space to create in (default: first from `list_spaces`) |
| `DOCMOST_PARENT_PAGE_ID` | no | Create the page under this parent (same space) |

Notes:
- The test **creates and updates a real page**. There is no delete tool yet, so the
  page remains afterwards — remove it from the Docmost UI if you want. It uses an
  obvious throwaway title.
- The session is cached in a temp dir (and `DOCMOST_DISABLE_KEYRING=1` is set), so it
  does not touch your real `~/.docmost-local-mcp` or OS keyring.
- If the update step fails the poll, that is the **version/async caveat** in action:
  confirm the page in the UI and check your Docmost version supports the
  `operation`/`format` update path.

## Roadmap

Remaining planned write tools (not yet implemented): `duplicate_page`,
`copy_page_to_space`, `move_page`, `move_page_to_space`, `create_space`,
`update_space`, `create_comment`, `update_comment`.
