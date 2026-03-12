import { constants } from "node:fs";
import { access } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

import open from "open";

import type { AuthWindowSession } from "../types.js";
import { debugLog } from "../utils/debug.js";

const require = createRequire(import.meta.url);

const WINDOWS_BINARY_NAME = "docmost-auth-helper.exe";
const UNIX_BINARY_NAME = "docmost-auth-helper";

interface AuthWindowHandle {
  mode: "native" | "browser";
  waitForExit?: Promise<number | null>;
  close: () => void;
}

export async function launchAuthWindow(session: AuthWindowSession): Promise<AuthWindowHandle> {
  try {
    const helperBinary = await resolveAuthHelperBinary();
    return launchNativeHelper(helperBinary, session);
  } catch (error) {
    debugLog("auth-helper", "Native helper unavailable, falling back to browser", {
      error: toErrorMessage(error),
      fallbackUrl: session.fallbackUrl,
    });
    await open(session.fallbackUrl);
    return {
      mode: "browser",
      close: () => {},
    };
  }
}

export function getHelperPackageName(
  platform: NodeJS.Platform = process.platform,
  arch = process.arch,
): string | null {
  const key = `${platform}-${arch}`;
  switch (key) {
    case "darwin-arm64":
      return "@docmost-local-mcp/auth-helper-darwin-arm64";
    case "darwin-x64":
      return "@docmost-local-mcp/auth-helper-darwin-x64";
    case "linux-arm64":
      return "@docmost-local-mcp/auth-helper-linux-arm64";
    case "linux-x64":
      return "@docmost-local-mcp/auth-helper-linux-x64";
    case "win32-arm64":
      return "@docmost-local-mcp/auth-helper-win32-arm64";
    case "win32-x64":
      return "@docmost-local-mcp/auth-helper-win32-x64";
    default:
      return null;
  }
}

async function resolveAuthHelperBinary(): Promise<string> {
  const candidates = [
    process.env.DOCMOST_AUTH_HELPER_PATH,
    getPackagedHelperCandidate(),
    getLocalBuildCandidate("release"),
    getLocalBuildCandidate("debug"),
  ].filter((value): value is string => Boolean(value));

  for (const candidate of candidates) {
    if (await isRunnableFile(candidate)) {
      debugLog("auth-helper", "Resolved native helper binary", { candidate });
      return candidate;
    }
  }

  throw new Error(`No native auth helper binary found for ${process.platform}/${process.arch}.`);
}

function launchNativeHelper(binaryPath: string, session: AuthWindowSession): AuthWindowHandle {
  debugLog("auth-helper", "Launching native auth helper", {
    binaryPath,
    loginUrl: session.loginUrl,
  });

  const child = spawn(
    binaryPath,
    [
      "--url",
      session.loginUrl,
      "--success-url",
      session.successUrl,
      "--title",
      session.windowTitle,
      "--width",
      String(session.windowWidth),
      "--height",
      String(session.windowHeight),
    ],
    {
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    debugLog("auth-helper", "Native helper stdout", chunk.trim());
  });
  child.stderr.on("data", (chunk: string) => {
    debugLog("auth-helper", "Native helper stderr", chunk.trim());
  });

  return {
    mode: "native",
    waitForExit: new Promise<number | null>((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code) => resolve(code));
    }),
    close: () => {
      if (!child.killed) {
        child.kill("SIGTERM");
      }
    },
  };
}

function getPackagedHelperCandidate(): string | null {
  const packageName = getHelperPackageName();
  if (!packageName) {
    return null;
  }

  try {
    const packageJsonPath = require.resolve(`${packageName}/package.json`);
    return join(dirname(packageJsonPath), "bin", getBinaryName());
  } catch {
    return null;
  }
}

function getLocalBuildCandidate(profile: "debug" | "release"): string {
  return fileURLToPath(
    new URL(`../../native/auth-helper/target/${profile}/${getBinaryName()}`, import.meta.url),
  );
}

async function isRunnableFile(path: string): Promise<boolean> {
  try {
    await access(path, process.platform === "win32" ? constants.F_OK : constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function getBinaryName(): string {
  return process.platform === "win32" ? WINDOWS_BINARY_NAME : UNIX_BINARY_NAME;
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
