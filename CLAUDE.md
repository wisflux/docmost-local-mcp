# CLAUDE.md

Guidance for working in this repository.

## What this is

`docmost-local-mcp` is a **Rust, read-only MCP server** that fronts a self-hosted [Docmost](https://docmost.com) instance for local IDE / AI tools (Cursor, Claude Desktop, etc.). It speaks MCP over **stdio** using [`rmcp`](https://docs.rs/rmcp) 0.6, authenticates to Docmost with email/password (storing the session locally), and exposes documentation as MCP tools. It is distributed as an `npx` package (`@wisflux/docmost-local-mcp`) whose Node launcher downloads the Rust binary from GitHub Releases.

Edition **2024**. The crate is both a binary and a library (`docmost_local_mcp`); integration tests in `tests/` consume the library.

## Commands

```bash
cargo build                 # debug build (default feature: native-webview = tao + wry)
cargo build --release       # release build
cargo test                  # unit + integration tests (tests/ use tempfile + mock axum servers)
cargo fmt --check                                        # format — CI runs this
cargo clippy --all-targets --all-features -- -D warnings  # lint — CI fails on any warning

# Run the MCP server locally (talks MCP over stdio):
cargo run -- --base-url=https://docs.example.com
DOCMOST_BASE_URL=https://docs.example.com cargo run

# Build without the native auth window (forces browser-fallback login):
cargo build --no-default-features
```

- `--base-url` (or `DOCMOST_BASE_URL`) selects the Docmost instance; it's optional — if absent the interactive login asks for it.
- `DOCMOST_DISABLE_KEYRING=1` skips the OS keyring and uses the encrypted-file credential fallback (used by tests).
- `DEBUG_DOCMOST_MCP=1` (or `true`) enables debug logging (via `debug::debug_log`, prefix `[docmost-local-mcp][ts][scope]`), which goes to **stderr** — never stdout, which is reserved for the MCP protocol.

## Architecture

Request path: **MCP client → `DocmostMcpServer` (`#[tool_router]`) → `DocmostClient` → Docmost `/api/...` → Markdown string back to the client.**

- [src/main.rs](src/main.rs) — `clap` CLI. Default command builds `StartupConfig` and serves over `stdio()`. Hidden `auth-window` subcommand launches the webview helper.
- [src/server.rs](src/server.rs) + [src/server/tools.rs](src/server/tools.rs) — `DocmostMcpServer { client, tool_router }`, `#[tool_handler] impl ServerHandler`. **12 tools**: 10 read-only (`list_spaces`, `search_docs`, `search_pages` [alias], `get_space`, `get_page`, `list_pages`, `list_child_pages`, `get_comments`, `list_workspace_members`, `get_current_user`, all `read_only_hint = true`) plus 2 **write** tools (`create_page`, `update_page`, `read_only_hint = false`) that accept Markdown and convert it to ProseMirror before POSTing.
- [src/server/render.rs](src/server/render.rs) — formats domain structs into the **Markdown** strings tools return (results are truncated: search 5, lists 10, members 20).
- [src/prosemirror.rs](src/prosemirror.rs) — converts a Docmost page's ProseMirror JSON (`content`) into Markdown (headings, lists, task lists, tables, code blocks, marks).
- [src/docmost_client.rs](src/docmost_client.rs) — `reqwest` wrapper. Every call is `POST {base_url}{endpoint}` with `bearer_auth(token)`; responses are unwrapped from an `{ "data": ... }` envelope. List shapes normalized by `normalize_list_result` / `normalize_cursor_list_result`. **Retries once on HTTP 401** after reauthenticating.
- [src/auth/](src/auth/) — `manager.rs` (session lifecycle: reuse saved session unless within 2 min of JWT expiry, else reauth via saved credentials or interactive login), `local_server.rs` (axum login page on `127.0.0.1:<random>`), `webview.rs` (native `tao`/`wry` window with browser fallback).
- [src/storage/](src/storage/) — `state_store.rs` persists to `~/.docmost-local-mcp/` (`config.json`, `session.json`); credentials go to the **OS keyring first** (`keyring_store.rs`), falling back to an **AES-256-GCM** encrypted file. Writes are atomic (temp + rename) with `0o600` perms.
- [src/types.rs](src/types.rs) — all serde domain models (`#[serde(rename_all = "camelCase")]`) + the `JsonSchema` tool-input structs.
- [npm/launcher/](npm/launcher/) — the Node `npx` launcher (`cli.js`) + `postinstall.js` that downloads the platform binary from GitHub Releases. CI (`.github/workflows/ci.yml`) runs rust checks, a launcher smoke test, and release builds for 6 platforms.

## Conventions & gotchas

- **Mostly read; two write tools.** `create_page` and `update_page` are implemented (`read_only_hint = false`); other write tools (`duplicate_page`, `move_page`, `create_space`, …) remain on the README roadmap. Write specifics: Docmost expects page `content` as a ProseMirror JSON **object** verbatim (`format` defaults to `json` — never stringify it); on **update**, content is applied only when `content` + `operation` + `format` are all sent, so `update_page` always sends `operation:"replace"` + `format:"json"`, and the change routes through Docmost's Yjs collaborative editor (may persist asynchronously). Keep new tools read-only unless explicitly adding write support.
- Tools return **human-readable Markdown**, not raw JSON. New tools should add a formatter in `render.rs` rather than dumping serde output.
- Tool args are `schemars::JsonSchema` structs in `types.rs`, passed as `Parameters<T>`; required fields are non-`Option`, optional fields use `#[serde(default)] Option<T>`.
- Auth is **lazy** — it triggers on the first authenticated tool call, not at startup. Never log tokens, passwords, or cookies to stdout.
- The Docmost API wraps payloads in `{ "data": ... }` and returns lists either bare or under `items`; use the existing normalize helpers.
- Adding a tool means touching three places: input struct in `types.rs`, `#[tool]` method in `server/tools.rs`, client method (+ endpoint) in `docmost_client.rs`, and usually a formatter in `render.rs`. Mirror an existing tool and add a check to `tests/mcp_server_test.rs` (which asserts the expected tool list and schemas).

See `CONTRIBUTING.md` for maintainer/release workflow.
