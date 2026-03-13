#!/usr/bin/env node

import { createWriteStream, chmodSync, mkdirSync, existsSync, readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { get } from "node:https";

const __dirname = dirname(fileURLToPath(import.meta.url));

const REPO = "wisflux/docmost-local-mcp";

const PLATFORM_MAP = {
  "darwin-arm64": "docmost-local-mcp",
  "darwin-x64": "docmost-local-mcp",
  "linux-arm64": "docmost-local-mcp",
  "linux-x64": "docmost-local-mcp",
  "win32-arm64": "docmost-local-mcp.exe",
  "win32-x64": "docmost-local-mcp.exe",
};

const targetKey = `${process.platform}-${process.arch}`;
const binaryName = PLATFORM_MAP[targetKey];

if (!binaryName) {
  console.warn(
    `@wisflux/docmost-local-mcp: unsupported platform ${process.platform}-${process.arch}, skipping binary download`,
  );
  process.exit(0);
}

const pkg = JSON.parse(readFileSync(join(__dirname, "package.json"), "utf8"));
const version = pkg.version;

const assetName = targetKey === "win32-arm64" || targetKey === "win32-x64"
  ? `docmost-local-mcp-${targetKey}.exe`
  : `docmost-local-mcp-${targetKey}`;

const url = `https://github.com/${REPO}/releases/download/v${version}/${assetName}`;

const binDir = join(__dirname, "bin");
const destPath = join(binDir, binaryName);

if (existsSync(destPath)) {
  process.exit(0);
}

mkdirSync(binDir, { recursive: true });

function download(url, dest, redirects = 0) {
  if (redirects > 5) {
    console.error("@wisflux/docmost-local-mcp: too many redirects downloading binary");
    process.exit(1);
  }

  get(url, (res) => {
    if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
      download(res.headers.location, dest, redirects + 1);
      return;
    }

    if (res.statusCode !== 200) {
      console.error(
        `@wisflux/docmost-local-mcp: failed to download binary (HTTP ${res.statusCode})`,
      );
      console.error(`  URL: ${url}`);
      console.error(
        "  The GitHub Release for this version may not exist yet. " +
        "You can build from source: https://github.com/" + REPO,
      );
      process.exit(1);
    }

    const file = createWriteStream(dest);
    res.pipe(file);
    file.on("finish", () => {
      file.close();
      if (process.platform !== "win32") {
        chmodSync(dest, 0o755);
      }
    });
    file.on("error", (err) => {
      console.error(`@wisflux/docmost-local-mcp: failed to write binary: ${err.message}`);
      process.exit(1);
    });
  }).on("error", (err) => {
    console.error(`@wisflux/docmost-local-mcp: download error: ${err.message}`);
    process.exit(1);
  });
}

download(url, destPath);
