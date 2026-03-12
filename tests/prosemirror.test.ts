import { describe, expect, it } from "vitest";

import { prosemirrorToMarkdown } from "../src/utils/prosemirror.js";

describe("prosemirrorToMarkdown", () => {
  it("renders headings, paragraphs, links, and code blocks", () => {
    const markdown = prosemirrorToMarkdown({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Overview" }],
        },
        {
          type: "paragraph",
          content: [
            { type: "text", text: "Visit " },
            {
              type: "text",
              text: "Docmost",
              marks: [{ type: "link", attrs: { href: "https://docmost.com" } }],
            },
          ],
        },
        {
          type: "codeBlock",
          attrs: { language: "ts" },
          content: [{ type: "text", text: "console.log('hello');" }],
        },
      ],
    });

    expect(markdown).toContain("## Overview");
    expect(markdown).toContain("Visit [Docmost](https://docmost.com)");
    expect(markdown).toContain("```ts");
    expect(markdown).toContain("console.log('hello');");
  });

  it("renders nested unordered lists and tables", () => {
    const markdown = prosemirrorToMarkdown({
      type: "doc",
      content: [
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "Parent" }],
                },
                {
                  type: "bulletList",
                  content: [
                    {
                      type: "listItem",
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "Child" }],
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
        {
          type: "table",
          content: [
            {
              type: "tableRow",
              content: [
                {
                  type: "tableHeader",
                  content: [{ type: "paragraph", content: [{ type: "text", text: "A" }] }],
                },
                {
                  type: "tableHeader",
                  content: [{ type: "paragraph", content: [{ type: "text", text: "B" }] }],
                },
              ],
            },
            {
              type: "tableRow",
              content: [
                {
                  type: "tableCell",
                  content: [{ type: "paragraph", content: [{ type: "text", text: "1" }] }],
                },
                {
                  type: "tableCell",
                  content: [{ type: "paragraph", content: [{ type: "text", text: "2" }] }],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(markdown).toContain("- Parent");
    expect(markdown).toContain("  - Child");
    expect(markdown).toContain("| A | B |");
    expect(markdown).toContain("| 1 | 2 |");
  });

  it("renders task lists with checked states", () => {
    const markdown = prosemirrorToMarkdown({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  attrs: { id: "bwjgrvbdamfz" },
                  content: [{ type: "text", text: "Monitoring " }],
                },
              ],
            },
          ],
        },
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  attrs: { id: "kmuomrhntdgh" },
                  content: [
                    {
                      type: "text",
                      text:
                        "Performance Logging should be more to test multiple theories , not just raw logging and then making theories from them",
                    },
                  ],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  attrs: { id: "kbtervxilajg" },
                  content: [
                    {
                      type: "text",
                      text: "Whims drive tasks completion rather than pre-planned flow",
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(markdown).toContain("- [ ] Monitoring");
    expect(markdown).toContain(
      "- [ ] Performance Logging should be more to test multiple theories , not just raw logging and then making theories from them",
    );
    expect(markdown).toContain("- [ ] Whims drive tasks completion rather than pre-planned flow");
  });
});
