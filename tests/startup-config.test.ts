import { describe, expect, it } from "vitest";

import { normalizeBaseUrl, parseStartupConfig } from "../src/startup-config.js";

describe("parseStartupConfig", () => {
  it("reads the base URL from CLI arguments", () => {
    expect(parseStartupConfig(["--base-url", "https://docs.example.com/"])).toEqual({
      baseUrl: "https://docs.example.com",
    });
  });

  it("supports inline CLI argument syntax", () => {
    expect(parseStartupConfig(["--base-url=https://docs.example.com/"])).toEqual({
      baseUrl: "https://docs.example.com",
    });
  });

  it("falls back to the environment when no CLI argument is provided", () => {
    expect(parseStartupConfig([], { DOCMOST_BASE_URL: "https://env.example.com/" })).toEqual({
      baseUrl: "https://env.example.com",
    });
  });

  it("throws when the base URL flag is missing a value", () => {
    expect(() => parseStartupConfig(["--base-url"])).toThrow("Missing value for --base-url.");
  });
});

describe("normalizeBaseUrl", () => {
  it("removes trailing slashes", () => {
    expect(normalizeBaseUrl("https://docs.example.com///")).toBe("https://docs.example.com");
  });
});
