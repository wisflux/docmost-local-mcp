# @wisflux/docmost-local-mcp

MCP server for [Docmost](https://docmost.com/) that is built for self-hosted instances, especially deployments that do not have an enterprise license but still want reliable MCP access from local IDEs and AI tools.

The package is launched with `npx`, while the actual server is a Rust binary downloaded from GitHub Releases during install. That binary handles stdio MCP traffic, local authentication UX, session storage, and Docmost API access.

> The main reason this project exists: bring MCP access to self-hosted Docmost setups without making an enterprise license a prerequisite.

## Why This Project

Many MCP integrations are designed around hosted or enterprise assumptions. This project is intentionally optimized for self-hosted Docmost:

- Works against your own Docmost base URL
- Uses Docmost email/password authentication
- Stores session state locally for reuse
- Opens a local auth flow instead of requiring a separate hosted control plane
- Ships as a simple `npx` entrypoint for easy IDE integration

If you run your own Docmost and want it available inside Cursor, Claude Desktop, or another MCP client, this package is the straightforward path.

## Highlights

- Strong fit for self-hosted Docmost instances without enterprise licensing
- Rust server core with a small Node launcher for predictable local installs
- Native auth window on supported platforms, with browser fallback
- Explicit Docmost instance selection via startup config
- Session reuse with JWT expiry checks and automatic re-login
- OS keychain credential storage on supported platforms
- Clean tool surface for spaces, pages, comments, members, and current user context

## Available Tools

- `list_spaces`: list available Docmost spaces
- `get_space`: fetch details for a specific space
- `search_docs`: search documentation, optionally scoped to a space
- `search_pages`: backward-compatible alias for `search_docs`
- `get_page`: fetch a page and return its content as Markdown
- `list_pages`: list recent pages in a space
- `list_child_pages`: list child pages for a parent page ID
- `get_comments`: list comments for a page
- `list_workspace_members`: list workspace members
- `get_current_user`: fetch the authenticated user and workspace context
- `create_page`: create a new page in a space from Markdown content
- `update_page`: update an existing page's title and/or Markdown content
- `duplicate_page`: duplicate a page (and its sub-pages) within its space
- `copy_page_to_space`: copy a page (and its sub-pages) into a different space
- `move_page`: move a page under a new parent page, or to the space root
- `move_page_to_space`: move a page (and its sub-pages) to a different space
- `create_space`: create a new space with a name and URL slug
- `update_space`: update a space's name, slug, and/or description
- `create_comment`: add a page-level comment to a page from Markdown
- `update_comment`: replace an existing comment's body with new Markdown

## Roadmap

All planned read and write tools are now implemented. `create_comment` adds
page-level comments; comments anchored to a specific text selection (inline
comments) require the collaborative editor's cursor positions and are out of
scope for this REST-based server.

## Requirements

- Node.js 18 or newer for `npx`
- A reachable Docmost instance
- Email/password authentication enabled in that Docmost instance

## Quick Start

Run the server directly with `npx`:

```bash
npx -y @wisflux/docmost-local-mcp --base-url=https://docs.example.com
```

You can also provide the base URL with an environment variable:

```bash
DOCMOST_BASE_URL=https://docs.example.com npx -y @wisflux/docmost-local-mcp
```

## MCP Client Configuration

Most MCP clients can launch the server directly with `npx`:

```json
{
  "mcpServers": {
    "docmost": {
      "command": "npx",
      "args": ["-y", "@wisflux/docmost-local-mcp", "--base-url=https://docs.example.com"]
    }
  }
}
```

This setup works well when you want a fixed Docmost instance per client configuration. If `--base-url` or `DOCMOST_BASE_URL` is set, the login page shows that URL prefilled and locks the field. If no base URL is configured, the login flow asks for it during interactive sign-in.

## Authentication Flow

1. Your MCP client launches the server over stdio.
2. On the first authenticated tool call, the server starts a local HTTP login page on `127.0.0.1`.
3. The server opens a native auth window when available, or falls back to the system browser.
4. You enter your email and password there. If `--base-url` or `DOCMOST_BASE_URL` is set, the Docmost URL is prefilled and locked.
5. The server signs in through `/api/auth/login`, extracts the `authToken` cookie, stores the session, and optionally stores credentials for automatic re-login.
6. Future requests reuse the saved token until it is close to expiry or rejected by Docmost.

## Local State And Credential Storage

The server stores local state in:

```text
~/.docmost-local-mcp/
```

Files used there:

- `config.json`: last base URL and email
- `session.json`: saved auth token and expiry

Credentials are stored in the OS keychain when available, which is the preferred path on supported platforms.

If secure OS credential storage is unavailable, the server falls back to encrypted local credential storage so it can still support login reuse without writing plain-text credentials. That fallback is intentionally secondary to keychain-backed storage.

## Platform Notes

The native auth window uses the system webview on each platform:

- macOS: `WKWebView`
- Windows: `WebView2`
- Linux: `WebKitGTK`

Important caveats:

- Windows needs the WebView2 runtime available
- Linux desktop environments need WebKitGTK packages installed
- When the binary is built without the `native-webview` feature, browser fallback is always used

## Tool Reference

### `list_spaces`

Returns Docmost space names, slugs, and IDs.

### `search_docs`

Inputs:

- `query`: required search text
- `space_id`: optional Docmost space ID

### `search_pages`

Inputs:

- `query`: required search text
- `space_id`: optional Docmost space ID

This is a backward-compatible alias for page search. `search_docs` remains available.

### `get_space`

Inputs:

- `space_id`: required Docmost space ID

### `get_page`

Inputs:

- `slug_id`: the page slug ID returned by `search_docs`

### `list_pages`

Inputs:

- `space_id`: required Docmost space ID
- `limit`: optional page count limit
- `cursor`: optional pagination cursor

### `list_child_pages`

Inputs:

- `page_id`: required parent page ID
- `limit`: optional page count limit
- `cursor`: optional pagination cursor

### `get_comments`

Inputs:

- `page_id`: required page ID
- `limit`: optional comment count limit
- `cursor`: optional pagination cursor

### `list_workspace_members`

Inputs:

- `limit`: optional member count limit
- `cursor`: optional pagination cursor
- `query`: optional member search text
- `admin_view`: optional admin visibility flag

### `get_current_user`

Inputs:

- none

### `create_page`

Inputs:

- `space_id`: required Docmost space ID (UUID) to create the page in
- `title`: required page title
- `markdown`: optional page body as Markdown
- `parent_page_id`: optional parent page ID to nest under (title-only pages only)

When `markdown` is provided, the page body is sent through Docmost's **import** endpoint
(`POST /api/pages/import`), which is the only mechanism that reliably persists page body
content across Docmost versions (including older self-hosted servers). Pages created with
a body land at the space root — `parent_page_id` is honored only for title-only pages.

### `update_page`

Inputs:

- `page_id`: required Docmost page ID or slug ID
- `title`: optional new title (omit to leave unchanged)
- `markdown`: optional new body as Markdown; replaces the existing content (omit to leave unchanged)

Updating a page **title** works on all Docmost versions. Updating an existing page's
**body** via REST works only on newer Docmost; on older self-hosted servers (e.g. v0.25.x)
the body is edited solely through the collaborative editor and a REST body update is not
applied. To set body content reliably there, create a new page with `create_page` instead.

For the full design, Markdown→ProseMirror conversion details, verified Docmost API
fields, and version caveats, see [docs/write-tools.md](docs/write-tools.md).

## Development

For maintainer and contributor workflow details, see `CONTRIBUTING.md`.

## License

MIT
