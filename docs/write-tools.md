# Write Tools: `create_page` & `update_page`

This document describes the two write tools added to the Docmost MCP server, how
they get Markdown body content into Docmost, the exact Docmost API they call, and
the caveats that matter when running against a self-hosted instance.

> Status: implemented, and verified live end-to-end against Docmost **v0.25.3**.
> The rest of the server remains read-only; these are the first write tools.

## Overview

| Tool | Purpose | Docmost endpoint |
| --- | --- | --- |
| `create_page` | Create a new page in a space from Markdown | `POST /api/pages/import` (body) or `POST /api/pages/create` (title-only) |
| `update_page` | Update an existing page's title (and body, on newer Docmost) | `POST /api/pages/update` |

Both tools accept **Markdown** for the page body. Callers never deal with
Docmost's internal ProseMirror format. Both tools are marked `read_only_hint = false`.

The key design decision: **`create_page` sends body content through the import
endpoint**, not the JSON create endpoint. See [Why import](#why-create-uses-the-import-endpoint).

## Tool reference

### `create_page`

| Input | Required | Description |
| --- | --- | --- |
| `space_id` | yes | Docmost space ID (UUID) to create the page in |
| `title` | yes | Page title |
| `markdown` | no | Page body as Markdown |
| `parent_page_id` | no | Parent page ID to nest under — **title-only pages only** |

Returns a confirmation with the new page's **ID**, **slug ID**, and space ID.
Use the slug ID with `get_page`, or the ID as `page_id` for `update_page`.

Example (MCP arguments):

```json
{
  "space_id": "018f...uuid",
  "title": "Release Notes",
  "markdown": "- First **stable** release\n- See [docs](https://example.com)"
}
```

Note: when `markdown` is provided the page is created via the import endpoint,
which always places the page at the **space root** — `parent_page_id` is honored
only for title-only pages (which use the plain create endpoint).

### `update_page`

| Input | Required | Description |
| --- | --- | --- |
| `page_id` | yes | Page ID **or** slug ID to update |
| `title` | no | New title (omit to leave unchanged) |
| `markdown` | no | New body as Markdown; replaces the existing content (omit to leave unchanged) |

Returns a confirmation with the page's ID, slug ID, and title.

> **Title updates work on all Docmost versions.** Updating an existing page's
> **body** via REST works only on newer Docmost; on older self-hosted servers
> (≤ v0.25.x) the body is edited solely through the collaborative editor and a REST
> body update is not applied (see [Version reality](#version-reality)). To set body
> content reliably there, create a new page with `create_page` instead.

## How it works

```
MCP client
  → create_page / update_page (src/server/tools.rs)
      → DocmostClient::create_page / update_page  (src/docmost_client.rs)
          create_page WITH body  → POST /api/pages/import   (multipart .md upload)
          create_page title-only → POST /api/pages/create
          update_page            → POST /api/pages/update
      → format_created_page / format_updated_page  (src/server/render.rs)
  ← Markdown confirmation string
```

### Why `create_page` uses the import endpoint

A Docmost page body is stored in two places: the `content` column (ProseMirror
JSON) **and** the `ydoc` column (a Yjs binary the collaborative editor reads from
as the source of truth). The JSON `POST /api/pages/create` does not write a usable
body on older servers — on Docmost ≤ v0.25.x the create DTO has no `content` field
at all, so the body is silently dropped (the page is created with an empty body).

The **import endpoint regenerates the `ydoc`**: it converts the uploaded Markdown
to ProseMirror *and* builds the matching Yjs document, so the body persists and
renders correctly. This is the same path Docmost's own "Import" UI uses, and it
works across Docmost versions. That is why `create_page` routes body content
through it.

The server does the Markdown → ProseMirror conversion, so `create_page` uploads
**raw Markdown** (it does not pre-convert). The project also ships a Rust
`markdown_to_prosemirror` converter (in [`src/prosemirror.rs`](../src/prosemirror.rs),
the inverse of the `prosemirror_to_markdown` reader); `update_page` uses it for the
newer-Docmost body-update path.

## Docmost API details

Verified against the tagged Docmost **v0.25.3** source and live against a running
v0.25.3 server. All endpoints sit under the global `/api` prefix and return HTTP
200 with the envelope `{ "data": ..., "success": true, "status": 200 }`.

### `POST /api/pages/import` (create with body)

Multipart form:

| Part | Type | Notes |
| --- | --- | --- |
| `spaceId` | text field | required |
| `file` | file part | the page body as a `.md` file (name **must** end in `.md`; Docmost validates by extension, not MIME). Max 10 MB. |

The importer takes the **first level-1 heading** (`# ...`) as the page **title**
and strips it from the body; otherwise it falls back to the file name. So
`create_page` prepends `# {title}` to the body before uploading, which sets the
title deterministically. The response returns the new page's `id`, `slugId`, and
`title` synchronously. Import has **no `parentPageId`** parameter — pages land at
the space root. Requires JWT auth + space edit permission.

### `POST /api/pages/create` (title-only)

| Field | Required | Notes |
| --- | --- | --- |
| `spaceId` | yes | UUID |
| `title` | no | Plain text |
| `parentPageId` | no | Nest under this page (same space) |

On v0.25.x this endpoint sets only title/parent/icon — it does **not** persist a
body. `create_page` uses it only when no `markdown` body is supplied.

### `POST /api/pages/update`

| Field | Required | Notes |
| --- | --- | --- |
| `pageId` | yes | Accepts a UUID **or** a slug ID |
| `title` | no | Omit to leave unchanged — works on all versions |
| `content` | no | ProseMirror document object — newer Docmost only |
| `operation` | conditionally | `append` \| `prepend` \| `replace` — required when sending content |
| `format` | conditionally | sent as `json` whenever content is sent |

`update_page` always sends `operation: "replace"` + `format: "json"` when a body is
provided (these are required together by newer Docmost; `operation` has no server
default). On ≤ v0.25.x the body change is **not applied** — see below.

## Version reality

This was confirmed live against the target server:

- **v0.25.3** (the live server tested): `create_page` with a body works via the
  import endpoint (verified — body persists and renders in the UI). `update_page`
  sets the **title** only; an existing page's **body** cannot be set via REST
  (body lives in the collaborative `ydoc`, reachable only through Docmost's
  Yjs/Hocuspocus websocket, which this client does not implement).
- **Newer Docmost**: the `POST /api/pages/update` `content`/`operation`/`format`
  body-update path also applies, so `update_page` can replace a body too.

## Permissions

Writes require an authenticated user with the right Docmost permissions (create →
space create / parent edit; update → page edit). Insufficient permission returns
`403`, surfaced to the caller as a clear error (the server does not panic).

## Files changed

| File | Change |
| --- | --- |
| [`Cargo.toml`](../Cargo.toml) | Add `pulldown-cmark`; enable reqwest `multipart` |
| [`src/prosemirror.rs`](../src/prosemirror.rs) | Add `markdown_to_prosemirror` + builder |
| [`src/types.rs`](../src/types.rs) | Add `CreatePageInput`, `UpdatePageInput` |
| [`src/docmost_client.rs`](../src/docmost_client.rs) | Add `create_page`, `import_markdown_page`, `update_page` |
| [`src/server/tools.rs`](../src/server/tools.rs) | Add the two `#[tool]` methods |
| [`src/server/render.rs`](../src/server/render.rs) | Add `format_created_page`, `format_updated_page` |
| [`src/server.rs`](../src/server.rs) | Update the server instructions string |
| `tests/` | `docmost_write_test.rs` (request-shape assertions) + converter tests in `prosemirror_test.rs` + tool coverage in `mcp_server_test.rs` |

## Tests

- **Converter** (`tests/prosemirror_test.rs`): per-node/mark unit tests, plus a
  full round-trip and regression tests for inline-code-inside-bold ordering,
  mixed task/plain lists, ordered-list start index, and empty input.
- **Client** (`tests/docmost_write_test.rs`): a mock Docmost asserts the exact
  requests — create-with-body uploads a `page.md` multipart file with `spaceId`
  and a prepended `# {title}`; title-only create uses `/api/pages/create` and
  never the import endpoint; update sends `operation` + `format` only with content.
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
| Client (`docmost_write_test.rs`) | Exact HTTP request shape, against a **mock** axum server | No (fake server) |
| Tool surface (`mcp_server_test.rs`) | Tools registered with the right schemas | No |

These prove *"the client sends what Docmost expects"* — but **not** that your
running server accepts it and a page actually appears. That last mile was verified
manually (see below).

## End-to-end verification

`create_page` was verified end-to-end against a live Docmost **v0.25.3** instance:
log in → `create_page` with a rich Markdown body → `get_page` confirms the body
persisted → `update_page` title → the title changes and the body survives. The
resulting page was also confirmed visually in the Docmost UI (headings, marks,
nested lists, task checkboxes, code block, blockquote, and table all rendered).

To re-run an equivalent check by hand, point the MCP server at your instance and
call the `create_page` tool with a `space_id`, `title`, and `markdown` body, then
open the returned page in Docmost.

## Roadmap

Remaining planned write tools (not yet implemented): `duplicate_page`,
`copy_page_to_space`, `move_page`, `move_page_to_space`, `create_space`,
`update_space`, `create_comment`, `update_comment`.
