# Dockerfile — used by Glama's (glama.ai) build + introspection sandbox to run the
# MCP server in a headless container. This is NOT the primary distribution path:
# real users install with `npx -y @wisflux/docmost-local-mcp` (which downloads a
# prebuilt binary). This image exists so automated MCP directories can build the
# server, start it, and perform the tools/list introspection exchange.
#
# The server is built with `--no-default-features`, which disables the
# `native-webview` feature (tao/wry, GTK/WebKit). Auth therefore always uses the
# browser-fallback path, and the container needs no GUI libraries. Introspection
# (initialize + tools/list) requires neither a Docmost instance nor authentication.

# ---- build stage ----
FROM rust:1-slim-bookworm AS builder
WORKDIR /build
# reqwest uses rustls (no OpenSSL). keyring links against dbus on Linux.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --no-default-features

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        libdbus-1-3 \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/docmost-local-mcp /usr/local/bin/docmost-local-mcp
# The MCP server speaks JSON-RPC over stdio.
ENTRYPOINT ["docmost-local-mcp"]
