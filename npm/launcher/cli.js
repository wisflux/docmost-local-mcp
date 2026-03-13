#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(import.meta.url);

const PLATFORM_PACKAGES = {
  "darwin-arm64": {
    packageName: "@wisflux/docmost-local-mcp-darwin-arm64",
    binaryName: "docmost-local-mcp",
  },
  "darwin-x64": {
    packageName: "@wisflux/docmost-local-mcp-darwin-x64",
    binaryName: "docmost-local-mcp",
  },
  "linux-arm64": {
    packageName: "@wisflux/docmost-local-mcp-linux-arm64",
    binaryName: "docmost-local-mcp",
  },
  "linux-x64": {
    packageName: "@wisflux/docmost-local-mcp-linux-x64",
    binaryName: "docmost-local-mcp",
  },
  "win32-arm64": {
    packageName: "@wisflux/docmost-local-mcp-win32-arm64",
    binaryName: "docmost-local-mcp.exe",
  },
  "win32-x64": {
    packageName: "@wisflux/docmost-local-mcp-win32-x64",
    binaryName: "docmost-local-mcp.exe",
  },
};

const targetKey = `${process.platform}-${process.arch}`;
const target = PLATFORM_PACKAGES[targetKey];

if (!target) {
  console.error(
    `Unsupported platform for @wisflux/docmost-local-mcp: ${process.platform}/${process.arch}`,
  );
  process.exit(1);
}

let packageJsonPath;
try {
  packageJsonPath = require.resolve(`${target.packageName}/package.json`);
} catch (error) {
  console.error(
    `The platform package ${target.packageName} is not installed. Reinstall the package or rerun npx.`,
  );
  process.exit(1);
}

const binaryPath = path.join(path.dirname(packageJsonPath), "bin", target.binaryName);

try {
  execFileSync(binaryPath, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (error) {
  if (typeof error.status === "number") {
    process.exit(error.status);
  }

  console.error(`Failed to launch ${binaryPath}: ${error.message}`);
  process.exit(1);
}
