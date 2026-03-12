import { mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { StateStore } from "../src/storage/state-store.js";

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

describe("StateStore", () => {
  it("persists config, session, and encrypted credentials", async () => {
    const baseDir = await createTempDir();
    const store = new StateStore(baseDir);

    await store.writeConfig({
      baseUrl: "https://docs.example.com",
      email: "jane@example.com",
      lastAuthenticatedAt: "2026-03-12T00:00:00.000Z",
    });
    await store.writeSession({
      token: "token-value",
      expiresAt: "2026-03-12T01:00:00.000Z",
      savedAt: "2026-03-12T00:00:00.000Z",
    });
    await store.writeCredentials({
      email: "jane@example.com",
      password: "super-secret",
    });

    await expect(store.readConfig()).resolves.toEqual({
      baseUrl: "https://docs.example.com",
      email: "jane@example.com",
      lastAuthenticatedAt: "2026-03-12T00:00:00.000Z",
    });
    await expect(store.readSession()).resolves.toEqual({
      token: "token-value",
      expiresAt: "2026-03-12T01:00:00.000Z",
      savedAt: "2026-03-12T00:00:00.000Z",
    });
    await expect(store.readCredentials()).resolves.toEqual({
      email: "jane@example.com",
      password: "super-secret",
    });
  });

  it("clears the saved session without touching credentials", async () => {
    const baseDir = await createTempDir();
    const store = new StateStore(baseDir);

    await store.writeSession({
      token: "token-value",
      expiresAt: null,
      savedAt: "2026-03-12T00:00:00.000Z",
    });
    await store.writeCredentials({
      email: "jane@example.com",
      password: "super-secret",
    });

    await store.clearSession();

    await expect(store.readSession()).resolves.toBeNull();
    await expect(store.readCredentials()).resolves.toEqual({
      email: "jane@example.com",
      password: "super-secret",
    });
  });
});

async function createTempDir(): Promise<string> {
  const dir = await mkdtemp(path.join(os.tmpdir(), "docmost-local-mcp-"));
  tempDirs.push(dir);
  return dir;
}
