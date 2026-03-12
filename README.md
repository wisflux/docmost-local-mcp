# @wisflux/docmost-local-mcp

A local MCP server for [Docmost](https://docmost.com/) written in TypeScript for IDE integrations that launch via `npx`.

It starts as a stdio MCP server, launches a small native auth window when authentication is needed, logs in against your Docmost instance, and then reuses the saved session for future tool calls.

## Features

- `list_spaces`: list available Docmost spaces
- `search_docs`: search documentation, optionally scoped to a space
- `get_page`: fetch a page and return its content as Markdown
- Small native auth window instead of relying on a browser tab
- Explicit Docmost instance selection via startup config
- Session reuse with JWT expiry checks and automatic re-login when needed

## Requirements

- Node.js 20 or newer
- A reachable Docmost instance with email/password authentication enabled

## Installation

Install or run through `npx`:

```bash
npx @wisflux/docmost-local-mcp --base-url=https://docs.example.com
```

## IDE Configuration

Most MCP clients can launch the server directly with `npx`.

Example config shape:

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

You can also provide the instance URL through an environment variable:

```bash
DOCMOST_BASE_URL=https://docs.example.com npx @wisflux/docmost-local-mcp
```

## Authentication Flow

1. The IDE launches the MCP server over stdio.
2. On the first authenticated tool call, the server starts a tiny local HTTP page and launches a native auth helper window that loads it.
3. You enter your email and password there. If `--base-url` or `DOCMOST_BASE_URL` is set, the Docmost URL is preconfigured and locked.
4. The server signs in to Docmost via `/api/auth/login`, extracts the `authToken` cookie, and saves the session locally.
5. The helper window closes itself after successful login.
6. Future calls reuse the stored token until it is close to expiry or rejected by Docmost.

If the native helper is unavailable for the current platform, the server falls back to opening the local login page in your browser.

## Platform Notes

The native auth helper uses the system webview on each platform:

- macOS: `WKWebView`
- Windows: `WebView2`
- Linux: `WebKitGTK`

Important caveats:

- Windows needs the WebView2 runtime available
- Linux desktop environments need the relevant WebKitGTK packages installed
- Unsigned macOS binaries may show stricter launch prompts until the helper binaries are signed and notarized

All platform helper binaries are bundled inside the published npm package under `helpers/`. CI builds each binary and places it before publishing.

## Local State

The server stores state in:

```text
~/.docmost-local-mcp/
```

Files used there:

- `config.json`: last base URL and email
- `session.json`: saved auth token and expiry
- `credentials.enc.json`: encrypted email/password for automatic re-login
- `credentials.key`: local encryption key used for the encrypted credentials file

The encrypted credentials file is meant to avoid storing raw passwords in plain text, but it is not equivalent to using an OS keychain or a hardware-backed secret store.

## Tool Reference

### `list_spaces`

Returns Docmost space names, slugs, and IDs.

### `search_docs`

Inputs:

- `query`: required search text
- `space_id`: optional Docmost space ID

### `get_page`

Inputs:

- `slug_id`: the page slug ID returned by `search_docs`

## Contributing

See `CONTRIBUTING.md`.

## License

MIT
