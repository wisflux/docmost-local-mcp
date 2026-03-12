export interface StoredConfig {
  baseUrl: string;
  email: string;
  lastAuthenticatedAt: string;
}

export interface StoredSession {
  token: string;
  expiresAt: string | null;
  savedAt: string;
}

export interface StoredCredentials {
  email: string;
  password: string;
}

export interface AuthenticatedSession {
  baseUrl: string;
  email: string;
  token: string;
  expiresAt: string | null;
}

export interface StartupConfig {
  baseUrl?: string;
}

export interface LoginInput {
  baseUrl: string;
  email: string;
  password: string;
}

export interface AuthWindowSession {
  loginUrl: string;
  successUrl: string;
  fallbackUrl: string;
  windowTitle: string;
  windowWidth: number;
  windowHeight: number;
}

export interface DocmostSpace {
  id: string;
  name: string;
  slug: string;
  description?: string | null;
  memberCount?: number | null;
}

export interface DocmostSearchResult {
  id?: string;
  slugId: string;
  title: string;
  highlight?: string;
  icon?: string;
  space?: {
    id?: string;
    name?: string;
  };
}

export interface DocmostPage {
  title: string;
  icon?: string;
  updatedAt?: string;
  space?: {
    name?: string;
  };
  creator?: {
    name?: string;
  };
  content?: Record<string, unknown>;
}
