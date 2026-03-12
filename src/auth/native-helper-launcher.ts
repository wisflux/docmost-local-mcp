import { constants } from "node:fs";
import { access } from "node:fs/promises";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import open from "open";

import type { AuthWindowSession } from "../types.js";
import { debugLog } from "../utils/debug.js";

const WINDOWS_BINARY_NAME = "docmost-auth-helper.exe";
const UNIX_BINARY_NAME = "docmost-auth-helper";

const SUPPORTED_TARGETS = [
  "darwin-arm64",
  "darwin-x64",
  "linux-arm64",
  "linux-x64",
  "win32-arm64",
  "win32-x64",
] as const;

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

export function getBundledHelperPath(
  platform: string = process.platform,
  arch: string = process.arch,
): string | null {
  const key = `${platform}-${arch}`;
  if (!SUPPORTED_TARGETS.includes(key as (typeof SUPPORTED_TARGETS)[number])) {
    return null;
  }

  const binaryName = platform === "win32" ? WINDOWS_BINARY_NAME : UNIX_BINARY_NAME;
  return fileURLToPath(new URL(`../../helpers/${key}/${binaryName}`, import.meta.url));
}

async function resolveAuthHelperBinary(): Promise<string> {
  const candidates = [
    process.env.DOCMOST_AUTH_HELPER_PATH,
    getBundledHelperPath(),
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

function getLocalBuildCandidate(profile: "debug" | "release"): string {
  const binaryName = process.platform === "win32" ? WINDOWS_BINARY_NAME : UNIX_BINARY_NAME;
  return new URL(
    `../../native/auth-helper/target/${profile}/${binaryName}`,
    import.meta.url,
  ).pathname;
}

async function isRunnableFile(path: string): Promise<boolean> {
  try {
    await access(path, process.platform === "win32" ? constants.F_OK : constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
