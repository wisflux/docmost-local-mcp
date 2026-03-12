import type {
  AuthenticatedSession,
  LoginInput,
  StartupConfig,
  StoredConfig,
  StoredSession,
} from "../types.js";
import { normalizeBaseUrl } from "../startup-config.js";
import { StateStore } from "../storage/state-store.js";
import { debugLog } from "../utils/debug.js";
import { LocalAuthServer } from "./local-auth-server.js";
import { launchAuthWindow } from "./native-helper-launcher.js";

const REFRESH_WINDOW_MS = 2 * 60 * 1000;

interface AuthManagerOptions extends StartupConfig {
  store?: StateStore;
}

export class AuthManager {
  private readonly store: StateStore;
  private readonly configuredBaseUrl: string | undefined;

  constructor(options: AuthManagerOptions = {}) {
    this.store = options.store ?? new StateStore();
    this.configuredBaseUrl = options.baseUrl ? normalizeBaseUrl(options.baseUrl) : undefined;
  }

  async getAuthenticatedSession(): Promise<AuthenticatedSession> {
    const config = await this.store.readConfig();
    const session = await this.store.readSession();
    const preferredBaseUrl = this.getPreferredBaseUrl(config);

    if (config && session && preferredBaseUrl === config.baseUrl && !isSessionExpiring(session)) {
      debugLog("auth", "Using saved session", {
        baseUrl: config.baseUrl,
        email: config.email,
        expiresAt: session.expiresAt,
      });
      return toAuthenticatedSession(config, session);
    }

    debugLog("auth", "Saved session missing or expiring; reauthenticating", {
      hasConfig: Boolean(config),
      hasSession: Boolean(session),
      expiresAt: session?.expiresAt ?? null,
    });
    return this.reauthenticate();
  }

  async reauthenticate(): Promise<AuthenticatedSession> {
    const config = await this.store.readConfig();
    const credentials = await this.store.readCredentials();
    const preferredBaseUrl = this.getPreferredBaseUrl(config);

    if (preferredBaseUrl && credentials) {
      debugLog("auth", "Reauthenticating with saved encrypted credentials", {
        baseUrl: preferredBaseUrl,
        email: credentials.email,
      });
      return this.login({
        baseUrl: preferredBaseUrl,
        email: credentials.email,
        password: credentials.password,
      });
    }

    debugLog("auth", "No reusable credentials available; starting interactive authentication", {
      hasConfig: Boolean(config),
      hasCredentials: Boolean(credentials),
      configuredBaseUrl: this.configuredBaseUrl ?? null,
    });
    return this.promptForLogin(config ?? undefined);
  }

  async login(input: LoginInput): Promise<AuthenticatedSession> {
    const baseUrl = normalizeBaseUrl(input.baseUrl);
    debugLog("auth", "Starting Docmost login", {
      baseUrl,
      email: input.email,
    });
    const response = await fetch(`${baseUrl}/api/auth/login`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify({
        email: input.email,
        password: input.password,
      }),
    });

    debugLog("auth", "Docmost login response received", {
      status: response.status,
      ok: response.ok,
    });

    if (!response.ok) {
      const details = await safeReadResponseText(response);
      throw new Error(`Docmost login failed (${response.status}). ${details}`.trim());
    }

    const token = readAuthTokenFromHeaders(response.headers);
    if (!token) {
      debugLog("auth", "Login response missing authToken cookie");
      throw new Error("Docmost login succeeded but no authToken cookie was returned.");
    }

    const expiresAt = getJwtExpiryIso(token);
    const now = new Date().toISOString();
    debugLog("auth", "Extracted auth token from login response", {
      expiresAt,
      tokenPreview: `${token.slice(0, 12)}...`,
    });

    await this.store.writeConfig({
      baseUrl,
      email: input.email,
      lastAuthenticatedAt: now,
    });
    await this.store.writeSession({
      token,
      expiresAt,
      savedAt: now,
    });
    await this.store.writeCredentials({
      email: input.email,
      password: input.password,
    });

    debugLog("auth", "Saved config, session, and encrypted credentials", {
      baseUrl,
      email: input.email,
      expiresAt,
    });

    return {
      baseUrl,
      email: input.email,
      token,
      expiresAt,
    };
  }

  private async promptForLogin(config?: StoredConfig): Promise<AuthenticatedSession> {
    const defaults: { baseUrl?: string; email?: string; baseUrlReadonly?: boolean } = {};
    const preferredBaseUrl = this.getPreferredBaseUrl(config);

    if (preferredBaseUrl) {
      defaults.baseUrl = preferredBaseUrl;
      defaults.baseUrlReadonly = Boolean(this.configuredBaseUrl);
    }
    if (config?.email) {
      defaults.email = config.email;
    }

    const authServer = new LocalAuthServer({
      defaults,
      onSubmit: async (input) => {
        await this.login(input);
      },
    });

    const authSession = await authServer.start();
    const authWindow = await launchAuthWindow(authSession);

    debugLog("auth", "Waiting for interactive authentication", {
      mode: authWindow.mode,
      loginUrl: authSession.loginUrl,
    });

    try {
      await waitForAuthenticationCompletion(authServer, authWindow.waitForExit);
      const refreshedConfig = await this.store.readConfig();
      const refreshedSession = await this.store.readSession();

      if (!refreshedConfig || !refreshedSession) {
        throw new Error("Authentication completed, but no session was saved.");
      }

      return toAuthenticatedSession(refreshedConfig, refreshedSession);
    } finally {
      authWindow.close();
      await authServer.stop();
    }
  }

  private getPreferredBaseUrl(config?: StoredConfig | null): string | undefined {
    return this.configuredBaseUrl ?? config?.baseUrl;
  }
}

function toAuthenticatedSession(
  config: StoredConfig,
  session: StoredSession,
): AuthenticatedSession {
  return {
    baseUrl: config.baseUrl,
    email: config.email,
    token: session.token,
    expiresAt: session.expiresAt,
  };
}

function isSessionExpiring(session: StoredSession): boolean {
  if (!session.expiresAt) {
    return false;
  }

  return new Date(session.expiresAt).getTime() - Date.now() <= REFRESH_WINDOW_MS;
}

function readAuthTokenFromHeaders(headers: Headers): string | null {
  const headerBag = headers as Headers & {
    getSetCookie?: () => string[];
  };

  const cookies =
    typeof headerBag.getSetCookie === "function"
      ? headerBag.getSetCookie()
      : headers.get("set-cookie")
        ? [headers.get("set-cookie") as string]
        : [];

  for (const cookie of cookies) {
    const match = /(?:^|,\s*)authToken=([^;]+)/.exec(cookie);
    if (match?.[1]) {
      return decodeURIComponent(match[1]);
    }
  }

  return null;
}

function getJwtExpiryIso(token: string): string | null {
  try {
    const payloadPart = token.split(".")[1];
    if (!payloadPart) {
      return null;
    }

    const payload = JSON.parse(Buffer.from(payloadPart, "base64url").toString("utf8")) as {
      exp?: number;
    };

    if (!payload.exp) {
      return null;
    }

    return new Date(payload.exp * 1000).toISOString();
  } catch {
    return null;
  }
}

async function safeReadResponseText(response: Response): Promise<string> {
  try {
    const text = (await response.text()).trim();
    return text ? `Response: ${text}` : "";
  } catch {
    return "";
  }
}

async function waitForAuthenticationCompletion(
  authServer: LocalAuthServer,
  helperExit?: Promise<number | null>,
): Promise<void> {
  const completion = authServer.waitForCompletion();
  if (!helperExit) {
    await completion;
    return;
  }

  await Promise.race([
    completion,
    helperExit.then(async (code) => {
      if (code === 0) {
        await completion;
        return;
      }

      if (code === 2) {
        throw new Error("The Docmost sign-in window was closed before authentication completed.");
      }

      throw new Error(
        `The Docmost sign-in window exited unexpectedly${code === null ? "." : ` (code ${code}).`}`,
      );
    }),
  ]);
}
