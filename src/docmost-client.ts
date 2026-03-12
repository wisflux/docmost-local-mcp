import type { AuthenticatedSession, DocmostPage, DocmostSearchResult, DocmostSpace } from "./types.js";
import { AuthManager } from "./auth/auth-manager.js";
import { debugLog } from "./utils/debug.js";

interface ApiEnvelope<T> {
  data?: T;
}

interface ItemCollection<T> {
  items?: T[];
}

export class DocmostClient {
  private readonly authManager: AuthManager;

  constructor(authManager = new AuthManager()) {
    this.authManager = authManager;
  }

  async listSpaces(): Promise<DocmostSpace[]> {
    const result = await this.request<{ items?: DocmostSpace[] }>("/api/spaces", {
      page: 1,
      limit: 100,
    });

    return result.items ?? [];
  }

  async searchDocs(query: string, spaceId?: string): Promise<DocmostSearchResult[]> {
    const payload: { query: string; spaceId?: string } = { query };
    if (spaceId) {
      payload.spaceId = spaceId;
    }

    const result = await this.request<DocmostSearchResult[] | ItemCollection<DocmostSearchResult>>(
      "/api/search",
      payload,
    );
    return normalizeListResult(result);
  }

  async getPage(slugId: string): Promise<DocmostPage | null> {
    const result = await this.request<DocmostPage | null>("/api/pages/info", {
      pageId: slugId,
    });

    return result;
  }

  private async request<T>(
    endpoint: string,
    payload: Record<string, unknown>,
    retryOnUnauthorized = true,
  ): Promise<T> {
    const session = await this.authManager.getAuthenticatedSession();
    return this.performRequest<T>(session, endpoint, payload, retryOnUnauthorized);
  }

  private async performRequest<T>(
    session: AuthenticatedSession,
    endpoint: string,
    payload: Record<string, unknown>,
    retryOnUnauthorized: boolean,
  ): Promise<T> {
    debugLog("api", "Calling Docmost API", {
      endpoint,
      baseUrl: session.baseUrl,
      payload,
      retryOnUnauthorized,
    });
    const response = await fetch(`${session.baseUrl}${endpoint}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify(payload),
    });

    debugLog("api", "Docmost API response received", {
      endpoint,
      status: response.status,
      ok: response.ok,
    });

    if (response.status === 401 && retryOnUnauthorized) {
      debugLog("api", "Received 401 from Docmost API; retrying after reauthentication", {
        endpoint,
      });
      const refreshedSession = await this.authManager.reauthenticate();
      return this.performRequest(refreshedSession, endpoint, payload, false);
    }

    if (!response.ok) {
      const details = await safeReadResponseText(response);
      throw new Error(`Docmost API request failed (${response.status}). ${details}`.trim());
    }

    const json = (await response.json()) as ApiEnvelope<T>;
    debugLog("api", "Parsed Docmost API response body", {
      endpoint,
      hasData: json.data !== undefined,
      dataType: Array.isArray(json.data) ? "array" : typeof json.data,
    });
    return (json.data ?? null) as T;
  }
}

export function normalizeListResult<T>(result: T[] | ItemCollection<T> | null): T[] {
  if (Array.isArray(result)) {
    return result;
  }

  return result?.items ?? [];
}

async function safeReadResponseText(response: Response): Promise<string> {
  try {
    const text = (await response.text()).trim();
    return text ? `Response: ${text}` : "";
  } catch {
    return "";
  }
}
