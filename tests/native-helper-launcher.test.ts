import { describe, expect, it } from "vitest";

import { getBundledHelperPath } from "../src/auth/native-helper-launcher.js";

describe("getBundledHelperPath", () => {
  it("returns a path for supported platforms", () => {
    const path = getBundledHelperPath("darwin", "arm64");
    expect(path).toContain("helpers/darwin-arm64/docmost-auth-helper");
  });

  it("returns a path with .exe for Windows", () => {
    const path = getBundledHelperPath("win32", "x64");
    expect(path).toContain("helpers/win32-x64/docmost-auth-helper.exe");
  });

  it("returns null for unsupported platforms", () => {
    expect(getBundledHelperPath("freebsd", "x64")).toBeNull();
  });
});
