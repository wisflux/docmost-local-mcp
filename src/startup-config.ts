import type { StartupConfig } from "./types.js";

export function parseStartupConfig(
  argv = process.argv.slice(2),
  env: NodeJS.ProcessEnv = process.env,
): StartupConfig {
  let baseUrl = readBaseUrlFromEnv(env);

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];

    if (argument === "--base-url") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("Missing value for --base-url.");
      }

      baseUrl = value;
      index += 1;
      continue;
    }

    if (argument?.startsWith("--base-url=")) {
      baseUrl = argument.slice("--base-url=".length);
    }
  }

  const config: StartupConfig = {};
  if (baseUrl?.trim()) {
    config.baseUrl = normalizeBaseUrl(baseUrl);
  }

  return config;
}

export function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.trim().replace(/\/+$/, "");
}

function readBaseUrlFromEnv(env: NodeJS.ProcessEnv): string | undefined {
  const value = env.DOCMOST_BASE_URL?.trim();
  return value ? value : undefined;
}
