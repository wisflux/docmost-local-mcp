import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import { chmod, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import type { StoredConfig, StoredCredentials, StoredSession } from "../types.js";

interface EncryptedPayload {
  iv: string;
  tag: string;
  ciphertext: string;
}

const DEFAULT_DIRNAME = ".docmost-local-mcp";

export class StateStore {
  readonly baseDir: string;
  private readonly configPath: string;
  private readonly sessionPath: string;
  private readonly credentialsPath: string;
  private readonly keyPath: string;

  constructor(baseDir = path.join(os.homedir(), DEFAULT_DIRNAME)) {
    this.baseDir = baseDir;
    this.configPath = path.join(this.baseDir, "config.json");
    this.sessionPath = path.join(this.baseDir, "session.json");
    this.credentialsPath = path.join(this.baseDir, "credentials.enc.json");
    this.keyPath = path.join(this.baseDir, "credentials.key");
  }

  async readConfig(): Promise<StoredConfig | null> {
    return this.readJsonFile<StoredConfig>(this.configPath);
  }

  async writeConfig(config: StoredConfig): Promise<void> {
    await this.writeJsonFile(this.configPath, config);
  }

  async readSession(): Promise<StoredSession | null> {
    return this.readJsonFile<StoredSession>(this.sessionPath);
  }

  async writeSession(session: StoredSession): Promise<void> {
    await this.writeJsonFile(this.sessionPath, session);
  }

  async clearSession(): Promise<void> {
    await rm(this.sessionPath, { force: true });
  }

  async readCredentials(): Promise<StoredCredentials | null> {
    const encrypted = await this.readJsonFile<EncryptedPayload>(this.credentialsPath);
    if (!encrypted) {
      return null;
    }

    const key = await this.getOrCreateEncryptionKey();
    const plaintext = this.decryptString(encrypted, key);
    return JSON.parse(plaintext) as StoredCredentials;
  }

  async writeCredentials(credentials: StoredCredentials): Promise<void> {
    const key = await this.getOrCreateEncryptionKey();
    const encrypted = this.encryptString(JSON.stringify(credentials), key);
    await this.writeJsonFile(this.credentialsPath, encrypted);
  }

  private async ensureBaseDir(): Promise<void> {
    await mkdir(this.baseDir, { recursive: true, mode: 0o700 });
    await chmod(this.baseDir, 0o700);
  }

  private async readJsonFile<T>(filePath: string): Promise<T | null> {
    try {
      const contents = await readFile(filePath, "utf8");
      return JSON.parse(contents) as T;
    } catch (error) {
      if (isMissingFile(error)) {
        return null;
      }

      throw error;
    }
  }

  private async writeJsonFile(filePath: string, value: unknown): Promise<void> {
    await this.ensureBaseDir();

    const tempPath = `${filePath}.tmp`;
    const contents = `${JSON.stringify(value, null, 2)}\n`;
    await writeFile(tempPath, contents, { encoding: "utf8", mode: 0o600 });
    await chmod(tempPath, 0o600);
    await rename(tempPath, filePath);
  }

  private async getOrCreateEncryptionKey(): Promise<Buffer> {
    await this.ensureBaseDir();

    try {
      const stored = await readFile(this.keyPath, "utf8");
      return Buffer.from(stored.trim(), "base64");
    } catch (error) {
      if (!isMissingFile(error)) {
        throw error;
      }
    }

    const key = randomBytes(32);
    await writeFile(this.keyPath, key.toString("base64"), { encoding: "utf8", mode: 0o600 });
    await chmod(this.keyPath, 0o600);
    return key;
  }

  private encryptString(plaintext: string, key: Buffer): EncryptedPayload {
    const iv = randomBytes(12);
    const cipher = createCipheriv("aes-256-gcm", key, iv);
    const ciphertext = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
    const tag = cipher.getAuthTag();

    return {
      iv: iv.toString("base64"),
      tag: tag.toString("base64"),
      ciphertext: ciphertext.toString("base64"),
    };
  }

  private decryptString(payload: EncryptedPayload, key: Buffer): string {
    const decipher = createDecipheriv(
      "aes-256-gcm",
      key,
      Buffer.from(payload.iv, "base64"),
    );
    decipher.setAuthTag(Buffer.from(payload.tag, "base64"));

    const plaintext = Buffer.concat([
      decipher.update(Buffer.from(payload.ciphertext, "base64")),
      decipher.final(),
    ]);

    return plaintext.toString("utf8");
  }
}

function isMissingFile(error: unknown): error is NodeJS.ErrnoException {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "ENOENT"
  );
}
