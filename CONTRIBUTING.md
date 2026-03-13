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
- `npm/launcher/`: thin Node launcher package used by `npx`
- `npm/platform-template/`: template used to build platform-specific npm binary packages
- `.github/workflows/`: CI and release workflows

## Local npx-style test (launcher + binary)

To verify the full path (Node launcher → platform binary → MCP server) without publishing:

1. Build the release binary: `cargo build --release`
2. Create a local platform package for your OS/arch (example for darwin-arm64):

   ```bash
   mkdir -p npm/platform-test-darwin-arm64/bin
   cp target/release/docmost-local-mcp npm/platform-test-darwin-arm64/bin/
   # package.json: name "@wisflux/docmost-local-mcp-darwin-arm64", "bin": { "docmost-local-mcp": "./bin/docmost-local-mcp" }
   ```

   Use the appropriate folder name for your platform (e.g. `linux-x64`, `win32-x64`). You can copy `npm/platform-template/package.json.tmpl` and substitute `__PACKAGE_NAME__`, `__OS__`, `__CPU__`, `__BINARY_NAME__` (e.g. `docmost-local-mcp.exe` on Windows).

3. Install the launcher with the local platform package and run:

   ```bash
   cd npm/launcher && npm install ../platform-test-darwin-arm64 --no-save
   node cli.js --help
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}' | node cli.js --base-url=https://example.com
   ```

   You should see the launcher’s help and then a JSON-RPC `initialize` result. The `platform-test-*` directory is for local use only (e.g. add to `.gitignore` if you create one).

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
- a launcher smoke test against a mocked platform package
- release binary builds on:
  - `macos-15`
  - `macos-15-intel`
  - `ubuntu-24.04-arm`
  - `ubuntu-24.04`
  - `windows-11-arm`
  - `windows-2025`

## Making `npx @wisflux/docmost-local-mcp` work

The command works once the meta package and platform packages are published to npm. To publish a release:

1. Commit any changes and ensure CI passes on `main`.
2. Create and push a version tag (version is taken from the tag; e.g. `v0.2.0` → published as `0.2.0`):

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. The **Release** workflow will build binaries for all platforms, package them, then publish the six platform packages and finally the meta package to npm. After it completes, anyone can run:

   ```bash
   npx -y @wisflux/docmost-local-mcp --base-url=https://docs.example.com
   ```

## Release Flow

`release.yml`:

1. builds the Rust binary on all supported runner targets
2. downloads the per-platform binary artifacts
3. creates six platform npm packages plus the meta launcher package
4. validates all package directories with `npm pack`
5. publishes platform packages first, then the meta package

Release publishing is triggered by tags like:

```bash
git tag v0.2.0
git push origin v0.2.0
```

## Trusted Publishing

This project is set up for npm trusted publishing via GitHub Actions OIDC.

Before automated publishing works:

1. Open the npm package settings for `@wisflux/docmost-local-mcp`
2. Enable **Trusted publishing**
3. Choose **GitHub Actions**
4. Set the workflow filename to `release.yml`

No long-lived `NPM_TOKEN` secret is needed when this is configured.

## Packaging Model

This project publishes:

- one meta npm package: `@wisflux/docmost-local-mcp`
- six platform binary packages:
  - `@wisflux/docmost-local-mcp-darwin-arm64`
  - `@wisflux/docmost-local-mcp-darwin-x64`
  - `@wisflux/docmost-local-mcp-linux-arm64`
  - `@wisflux/docmost-local-mcp-linux-x64`
  - `@wisflux/docmost-local-mcp-win32-arm64`
  - `@wisflux/docmost-local-mcp-win32-x64`

The meta package is only the Node launcher. The real runtime lives in the platform package binary that npm installs as an optional dependency for the current machine.
