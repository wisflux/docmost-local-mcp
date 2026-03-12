import { describe, expect, it } from "vitest";

import { normalizeListResult } from "../src/docmost-client.js";

describe("normalizeListResult", () => {
  it("returns array results unchanged", () => {
    expect(normalizeListResult([{ id: 1 }, { id: 2 }])).toEqual([{ id: 1 }, { id: 2 }]);
  });

  it("extracts items arrays from wrapped Docmost responses", () => {
    expect(normalizeListResult({ items: [{ id: 1 }] })).toEqual([{ id: 1 }]);
  });

  it("returns an empty array for null or empty item collections", () => {
    expect(normalizeListResult(null)).toEqual([]);
    expect(normalizeListResult({})).toEqual([]);
  });
});
