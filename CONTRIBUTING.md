# Contributing

Thanks for contributing to `@wisflux/docmost-local-mcp`.

This document covers local setup, native-webview prerequisites, release packaging, and maintainer-facing workflow details that are intentionally kept out of the public `README.md`.

## Prerequisites

- Node.js 18 or newer for npm packaging and launcher validation
- Rust toolchain (`cargo`, `rustc`)
- A reachable Docmost instance for manual auth testing

Platform-specific native-webview requirements:

- macOS: Xcode command line tools
- Windows: WebView2 runtime
- Linux: GTK/WebKitGTK development packages

## Local Setup

```bash
cargo build
```

Useful commands:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

To build a headless variant that always falls back to the browser:

```bash
cargo build --release --no-default-features
```

## Repository Layout

- `src/`: Rust MCP server, auth flow, Docmost client, storage, and ProseMirror conversion
- `npm/launcher/`: Node launcher package used by `npx`, plus postinstall binary downloader
- `.github/workflows/`: CI and release workflows

## Local npx-style test (launcher + binary)

To verify the full path (Node launcher → binary → MCP server) without publishing:

1. Build the release binary: `cargo build --release`
2. Place it where the launcher expects:

   ```bash
   mkdir -p npm/launcher/bin
   cp target/release/docmost-local-mcp npm/launcher/bin/
   ```

   On Windows, copy `docmost-local-mcp.exe` instead.

3. Run the launcher:

   ```bash
   node npm/launcher/cli.js --help
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}' | node npm/launcher/cli.js --base-url=https://example.com
   ```

## Local MCP Testing

Run the binary directly:

```bash
cargo run -- --base-url=https://docs.example.com
```

For MCP client configuration from a local checkout:

```json
{
  "mcpServers": {
    "docmost": {
      "command": "/absolute/path/to/docmost-local-mcp/target/debug/docmost-local-mcp",
      "args": ["--base-url=https://docs.example.com"]
    }
  }
}
```

## From-Scratch Auth Testing

To test first-time auth without touching your real saved state, run the MCP server with a temporary `HOME`:

```bash
TMP_HOME="$(mktemp -d /tmp/docmost-local-mcp-test.XXXXXX)"
HOME="$TMP_HOME" cargo run -- --base-url=https://docs.example.com
```

This forces the package to create a fresh `~/.docmost-local-mcp/` under the temporary home directory.

## Linux Build Dependencies

Ubuntu builds require:

```bash
sudo apt-get install -y \
  pkg-config \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev
```

If Linux native-webview builds fail locally or in CI, check these first.

## CI

`ci.yml` runs:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- a launcher smoke test with a mock binary in `bin/`
- release binary builds on:
  - `macos-15`
  - `macos-15-intel`
  - `ubuntu-24.04-arm`
  - `ubuntu-24.04`
  - `windows-11-arm`
  - `windows-2025`

## Making `npx @wisflux/docmost-local-mcp` work

The command works once the npm package is published and a matching GitHub Release exists with platform binaries. To publish a release:

1. Commit any changes and ensure CI passes on `main`.
2. Create and push a version tag (version is taken from the tag; e.g. `v0.2.0` → published as `0.2.0`):

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. The **Release** workflow will:
   - Build binaries for all 6 platforms
   - Create a GitHub Release with each binary as a downloadable asset
   - Publish the single `@wisflux/docmost-local-mcp` package to npm

4. When a user runs `npx -y @wisflux/docmost-local-mcp`, the `postinstall` script downloads the correct platform binary from the GitHub Release.

## Trusted Publishing

This project is set up for npm trusted publishing via GitHub Actions OIDC.

Before automated publishing works:

1. Open the npm package settings for `@wisflux/docmost-local-mcp`
2. Enable **Trusted publishing**
3. Choose **GitHub Actions**
4. Set the workflow filename to `release.yml`

No long-lived `NPM_TOKEN` secret is needed when this is configured.

## Packaging Model

This project publishes a single npm package (`@wisflux/docmost-local-mcp`) containing:

- `cli.js`: thin Node launcher that executes the platform binary
- `postinstall.js`: downloads the correct platform binary from GitHub Releases on install

Platform binaries are hosted as GitHub Release assets named `docmost-local-mcp-{platform}-{arch}` (e.g. `docmost-local-mcp-darwin-arm64`, `docmost-local-mcp-win32-x64.exe`).
