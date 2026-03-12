import { chmodSync, copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";

import { getHelperTarget } from "./helper-targets.mjs";

const platform = process.argv[2] ?? process.platform;
const arch = process.argv[3] ?? process.arch;
const rootDir = new URL("../", import.meta.url).pathname;

const target = getHelperTarget(platform, arch);
if (!target) {
  throw new Error(`Unsupported helper target: ${platform}/${arch}`);
}

const sourcePath = join(rootDir, "native", "auth-helper", "target", "release", target.binaryName);
if (!existsSync(sourcePath)) {
  throw new Error(`Built helper binary not found at ${sourcePath}`);
}

const destinationPath = join(rootDir, "packages", target.packageDir, "bin", target.binaryName);
mkdirSync(dirname(destinationPath), { recursive: true });
copyFileSync(sourcePath, destinationPath);

if (platform !== "win32") {
  chmodSync(destinationPath, 0o755);
}

process.stdout.write(`Copied ${sourcePath} -> ${destinationPath}\n`);
