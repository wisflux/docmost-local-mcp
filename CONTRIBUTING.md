# Contributing

Thanks for contributing to `@wisflux/docmost-local-mcp`.

This document covers local setup, native helper development, release workflow, and other maintainer-facing details that are intentionally kept out of the public `README.md`.

## Prerequisites

- Node.js 20 or newer
- Rust toolchain (`cargo`, `rustc`)
- A reachable Docmost instance for manual auth testing

Platform-specific native helper requirements:

- macOS: Xcode command line tools
- Windows: WebView2 runtime
- Linux: system packages for GTK/WebKitGTK

## Local Setup

```bash
npm install
npm run build
```

Useful commands:

```bash
npm run dev
npm run typecheck
npm run test
npm run build
npm run build:helper
npm run package:helper:local
```

`npm run package:helper:local` builds the Rust auth helper in release mode and copies the current platform binary into `helpers/<platform>-<arch>/`.

## Repository Layout

- `src/`: TypeScript MCP server, auth flow, and Docmost client
- `native/auth-helper/`: Rust native helper that opens the auth window
- `helpers/`: Bundled native helper binaries included in the published npm package
- `.github/workflows/`: CI and release workflows

## Local MCP Testing

For local checkout testing, run the built CLI directly:

```json
{
  "mcpServers": {
    "docmost": {
      "command": "node",
      "args": [
        "/absolute/path/to/docmost-local-mcp/dist/cli.js",
        "--base-url=https://docs.example.com"
      ]
    }
  }
}
```

You can also run it directly:

```bash
node dist/cli.js --base-url=https://docs.example.com
```

## From-Scratch Auth Testing

To test first-time auth without touching your real saved state, run the MCP server with a temporary `HOME`:

```bash
TMP_HOME="$(mktemp -d /tmp/docmost-local-mcp-test.XXXXXX)"
HOME="$TMP_HOME" npx @wisflux/docmost-local-mcp --base-url=https://docs.example.com
```

This forces the package to create a fresh `~/.docmost-local-mcp/` under the temporary home directory.

## Native Helper Notes

The native helper is a small Rust app using:

- `tao` for native windowing/event loop
- `wry` for the system webview

It is responsible only for:

- opening a fixed-size auth window
- loading the local login URL
- detecting success via navigation to `/success`
- exiting with a meaningful status code

It does not store tokens or talk to Docmost directly.

## Linux Build Dependencies

The CI workflows install these packages on Ubuntu runners:

```bash
sudo apt-get install -y \
  pkg-config \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev
```

If Linux helper builds fail locally or in CI, check these first.

## CI

`ci.yml` runs:

- Node typecheck/build/test on `ubuntu-24.04`
- native helper builds on:
  - `macos-15`
  - `macos-15-intel`
  - `ubuntu-24.04-arm`
  - `ubuntu-24.04`
  - `windows-11-arm`
  - `windows-2025`

Each helper build uploads its bundled binary as an artifact.

## Release Flow

`release.yml`:

1. builds the native helper on all supported runner targets
2. downloads all helper artifacts into `helpers/`
3. verifies the full bundled helper set exists
4. builds and tests the npm package
5. publishes to npm with trusted publishing

Release publishing is triggered by tags like:

```bash
git tag v0.1.1
git push origin v0.1.1
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

This project publishes as a single npm package.

That package contains:

- `dist/` TypeScript build output
- `helpers/` native binaries for all supported target platforms

CI is responsible for assembling the full `helpers/` directory before `npm publish`.
