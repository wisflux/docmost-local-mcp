import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import type { AuthWindowSession, LoginInput } from "../types.js";
import { debugLog } from "../utils/debug.js";

interface LocalAuthDefaults {
  baseUrl?: string;
  email?: string;
  baseUrlReadonly?: boolean;
}

interface LocalAuthServerOptions {
  defaults?: LocalAuthDefaults;
  onSubmit: (input: LoginInput) => Promise<void>;
  timeoutMs?: number;
}

export class LocalAuthServer {
  private readonly defaults: LocalAuthDefaults;
  private readonly onSubmit: (input: LoginInput) => Promise<void>;
  private readonly timeoutMs: number;
  private server: Server | null = null;
  private completion: Promise<void> | null = null;
  private resolveCompletion: (() => void) | undefined;
  private rejectCompletion: ((error: Error) => void) | undefined;
  private timeoutHandle: NodeJS.Timeout | undefined;
  private settled = false;

  constructor(options: LocalAuthServerOptions) {
    this.defaults = options.defaults ?? {};
    this.onSubmit = options.onSubmit;
    this.timeoutMs = options.timeoutMs ?? 5 * 60 * 1000;
  }

  async start(): Promise<AuthWindowSession> {
    if (this.server) {
      throw new Error("Local auth server is already running.");
    }

    this.server = createServer((request, response) => {
      void this.handleRequest(request, response);
    });
    this.completion = new Promise<void>((resolve, reject) => {
      this.resolveCompletion = resolve;
      this.rejectCompletion = reject;
    });
    this.timeoutHandle = setTimeout(() => {
      void this.finish(new Error("Timed out waiting for Docmost sign-in to complete."));
    }, this.timeoutMs);

    await new Promise<void>((resolve, reject) => {
      this.server?.listen(0, "127.0.0.1", () => resolve());
      this.server?.once("error", reject);
    });

    const address = this.server.address();
    if (!address || typeof address === "string") {
      await this.stop();
      throw new Error("Failed to start local auth server.");
    }

    const url = `http://127.0.0.1:${(address as AddressInfo).port}`;
    debugLog("local-auth", "Local auth page ready", { url, defaults: this.defaults });

    return {
      loginUrl: `${url}/login`,
      successUrl: `${url}/success`,
      fallbackUrl: `${url}/login`,
      windowTitle: "Docmost Sign In",
      windowWidth: 500,
      windowHeight: 680,
    };
  }

  waitForCompletion(): Promise<void> {
    if (!this.completion) {
      throw new Error("Local auth server has not been started.");
    }

    return this.completion;
  }

  async stop(): Promise<void> {
    if (this.timeoutHandle) {
      clearTimeout(this.timeoutHandle);
      this.timeoutHandle = undefined;
    }

    const server = this.server;
    this.server = null;
    if (!server) {
      return;
    }

    await new Promise<void>((resolve) => {
      server.close(() => resolve());
    });
  }

  private async handleRequest(request: IncomingMessage, response: ServerResponse): Promise<void> {
    debugLog("local-auth", "Incoming request", {
      method: request.method,
      url: request.url,
    });

    const url = new URL(request.url ?? "/", "http://127.0.0.1");

    if (request.method === "GET" && url.pathname === "/") {
      response.writeHead(302, { location: "/login" });
      response.end();
      return;
    }

    if (request.method === "GET" && url.pathname === "/login") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(renderLoginHtml(this.defaults));
      return;
    }

    if (request.method === "GET" && url.pathname === "/success") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(renderSuccessHtml());
      await this.finish();
      return;
    }

    if (request.method === "POST" && url.pathname === "/auth") {
      try {
        const body = await readRequestBody(request);
        const parsed = parseLoginInput(body, this.defaults);
        debugLog("local-auth", "Received auth form submission", {
          baseUrl: parsed.baseUrl,
          email: parsed.email,
        });
        await this.onSubmit(parsed);

        response.writeHead(200, { "content-type": "application/json; charset=utf-8" });
        response.end(JSON.stringify({ ok: true, redirectUrl: "/success" }));
        debugLog("local-auth", "Auth submission succeeded");
      } catch (error) {
        debugLog("local-auth", "Auth submission failed", {
          error: toErrorMessage(error),
        });
        response.writeHead(400, { "content-type": "application/json; charset=utf-8" });
        response.end(JSON.stringify({ ok: false, error: toErrorMessage(error) }));
      }
      return;
    }

    response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
    response.end("Not found");
  }

  private async finish(error?: Error): Promise<void> {
    if (this.settled) {
      return;
    }

    this.settled = true;
    const resolve = this.resolveCompletion;
    const reject = this.rejectCompletion;
    this.resolveCompletion = undefined;
    this.rejectCompletion = undefined;

    await this.stop();

    if (error) {
      reject?.(error);
      return;
    }

    resolve?.();
  }
}

async function readRequestBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];

  for await (const chunk of request) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  return Buffer.concat(chunks).toString("utf8");
}

function parseLoginInput(rawBody: string, defaults: LocalAuthDefaults): LoginInput {
  const data = JSON.parse(rawBody) as Partial<LoginInput>;
  const baseUrl = data.baseUrl?.trim() || defaults.baseUrl?.trim();
  const email = data.email?.trim();
  const password = data.password;

  if (!baseUrl || !email || !password) {
    throw new Error("Base URL, email, and password are required.");
  }

  return {
    baseUrl,
    email,
    password,
  };
}

function renderSuccessHtml(): string {
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Docmost MCP</title>
    <style>
      :root { color-scheme: dark; }
      body {
        margin: 0; min-height: 100vh; display: grid; place-items: center;
        background: linear-gradient(180deg, #0b1020 0%, #080c18 100%);
        font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        color: #aab4d6;
      }
      .msg { text-align: center; max-width: 460px; line-height: 1.6; }
      h2 { color: #f4f7ff; margin-bottom: 8px; }
      a { color: #7dd3fc; }
    </style>
  </head>
  <body>
    <div class="msg">
      <h2>Authentication Succeeded</h2>
      <p>This window can close now.</p>
    </div>
    <script>
      setTimeout(() => {
        try { window.close(); } catch {}
      }, 400);
    </script>
  </body>
</html>`;
}

function renderLoginHtml(defaults: LocalAuthDefaults): string {
  const baseUrl = escapeHtml(defaults.baseUrl ?? "");
  const email = escapeHtml(defaults.email ?? "");
  const readonlyBaseUrl = defaults.baseUrlReadonly ? "readonly" : "";
  const baseUrlHint = defaults.baseUrlReadonly
    ? "Configured by the MCP server startup options."
    : "Use the full Docmost URL, for example https://docs.example.com.";

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Docmost MCP Sign In</title>
    <style>
      :root {
        color-scheme: dark;
        --bg: #0b1020;
        --panel: #141b34;
        --panel-border: #2d3763;
        --text: #f4f7ff;
        --muted: #aab4d6;
        --accent: #7dd3fc;
        --accent-strong: #38bdf8;
        --danger: #fca5a5;
      }
      * { box-sizing: border-box; }
      body {
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        background:
          radial-gradient(circle at top, rgba(56, 189, 248, 0.18), transparent 35%),
          linear-gradient(180deg, #0b1020 0%, #080c18 100%);
        font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        color: var(--text);
      }
      .card {
        width: min(92vw, 480px);
        padding: 28px;
        border-radius: 20px;
        border: 1px solid var(--panel-border);
        background: rgba(20, 27, 52, 0.92);
        box-shadow: 0 18px 45px rgba(0, 0, 0, 0.32);
      }
      h1 { margin: 0 0 12px; font-size: 1.6rem; }
      p { margin: 0 0 16px; color: var(--muted); line-height: 1.5; font-size: 0.9rem; }
      label { display: block; margin-bottom: 12px; font-size: 0.92rem; }
      input {
        width: 100%; margin-top: 5px; padding: 10px 12px; border-radius: 10px;
        border: 1px solid #3b4a85; background: #0b1227; color: var(--text); font-size: 0.92rem;
      }
      button {
        width: 100%; margin-top: 8px; padding: 12px 14px; border: 0; border-radius: 10px;
        background: linear-gradient(135deg, var(--accent), var(--accent-strong));
        color: #04101a; font-weight: 700; cursor: pointer;
      }
      button:disabled { opacity: 0.7; cursor: progress; }
      .status { min-height: 22px; margin-top: 14px; color: var(--muted); }
      .status.error { color: var(--danger); }
      .status.success { color: #86efac; }
    </style>
  </head>
  <body>
    <main class="card">
      <h1>Sign in to Docmost</h1>
      <p>
        Credentials are sent only to the local MCP process, which then signs in to your Docmost instance.
      </p>
      <form id="login-form">
        <label>
          Docmost Base URL
          <input id="baseUrl" name="baseUrl" value="${baseUrl}" placeholder="https://docs.example.com" ${readonlyBaseUrl} required />
        </label>
        <p>${baseUrlHint}</p>
        <label>
          Email
          <input id="email" name="email" type="email" value="${email}" placeholder="you@example.com" required />
        </label>
        <label>
          Password
          <input id="password" name="password" type="password" placeholder="Your Docmost password" required />
        </label>
        <button id="submit-button" type="submit">Authenticate</button>
        <div id="status" class="status" role="status"></div>
      </form>
    </main>
    <script>
      const form = document.getElementById("login-form");
      const status = document.getElementById("status");
      const submitButton = document.getElementById("submit-button");

      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        submitButton.disabled = true;
        status.className = "status";
        status.textContent = "Signing in...";

        const payload = {
          baseUrl: document.getElementById("baseUrl").value,
          email: document.getElementById("email").value,
          password: document.getElementById("password").value
        };

        try {
          const response = await fetch("/auth", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(payload)
          });
          const result = await response.json();

          if (!response.ok || !result.ok) {
            throw new Error(result.error || "Authentication failed");
          }

          status.className = "status success";
          status.textContent = "Authenticated. Finishing sign-in...";
          window.location.assign(result.redirectUrl || "/success");
        } catch (error) {
          status.className = "status error";
          status.textContent = error instanceof Error ? error.message : String(error);
          submitButton.disabled = false;
        }
      });
    </script>
  </body>
</html>`;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
