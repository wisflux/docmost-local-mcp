# @wisflux/docmost-local-mcp

A local MCP server for [Docmost](https://docmost.com/) implemented in Rust and launched through `npx`.

The published npm package is a small Node launcher plus a platform-specific Rust binary. At runtime the binary handles stdio MCP traffic, local authentication UX, session storage, and Docmost API access.

## Features

- `list_spaces`: list available Docmost spaces
- `search_docs`: search documentation, optionally scoped to a space
- `get_page`: fetch a page and return its content as Markdown
- Native auth window on supported platforms, with browser fallback
- Explicit Docmost instance selection via startup config
- Session reuse with JWT expiry checks and automatic re-login
- OS keychain credential storage when available, with encrypted file fallback

## Requirements

- Node.js 18 or newer for `npx`
- A reachable Docmost instance with email/password authentication enabled

## Installation

Run directly with `npx`:

```bash
npx -y @wisflux/docmost-local-mcp --base-url=https://docs.example.com
```

## IDE Configuration

Most MCP clients can launch the server directly with `npx`.

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

You can also provide the base URL through an environment variable:

```bash
DOCMOST_BASE_URL=https://docs.example.com npx -y @wisflux/docmost-local-mcp
```

## Authentication Flow

1. The MCP client launches the server over stdio.
2. On the first authenticated tool call, the server starts a local HTTP login page on `127.0.0.1`.
3. The server opens a native auth window when available, or falls back to the system browser.
4. You enter your email and password there. If `--base-url` or `DOCMOST_BASE_URL` is set, the Docmost URL is preconfigured and locked.
5. The server signs in through `/api/auth/login`, extracts the `authToken` cookie, stores the session, and optionally stores credentials for automatic re-login.
6. Future requests reuse the saved token until it is close to expiry or rejected by Docmost.

## Platform Notes

The native auth window uses the system webview on each platform:

- macOS: `WKWebView`
- Windows: `WebView2`
- Linux: `WebKitGTK`

Important caveats:

- Windows needs the WebView2 runtime available
- Linux desktop environments need WebKitGTK packages installed
- When the binary is built without the `native-webview` feature, browser fallback is always used

## Local State

The server stores state in:

```text
~/.docmost-local-mcp/
```

Files used there:

- `config.json`: last base URL and email
- `session.json`: saved auth token and expiry
- `credentials.enc.json`: encrypted credential fallback when keychain storage is unavailable
- `credentials.key`: local encryption key for the encrypted fallback credentials file

Credentials are stored in the OS keychain when available. The encrypted file fallback is meant to avoid plain-text storage, but it is not equivalent to a hardware-backed secret store.

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

## Development

For maintainer and contributor workflow details, see `CONTRIBUTING.md`.

## License

MIT
