import { describe, expect, it } from "vitest";

import { getHelperPackageName } from "../src/auth/native-helper-launcher.js";

describe("getHelperPackageName", () => {
  it("maps supported platforms to package names", () => {
    expect(getHelperPackageName("darwin", "arm64")).toBe(
      "@docmost-local-mcp/auth-helper-darwin-arm64",
    );
    expect(getHelperPackageName("win32", "x64")).toBe("@docmost-local-mcp/auth-helper-win32-x64");
    expect(getHelperPackageName("linux", "x64")).toBe("@docmost-local-mcp/auth-helper-linux-x64");
  });

  it("returns null for unsupported platforms", () => {
    expect(getHelperPackageName("freebsd", "x64")).toBeNull();
  });
});
