import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import * as z from "zod/v4";

import { AuthManager } from "./auth/auth-manager.js";
import { DocmostClient } from "./docmost-client.js";
import type { StartupConfig } from "./types.js";
import { prosemirrorToMarkdown } from "./utils/prosemirror.js";

export function createServer(startupConfig: StartupConfig = {}): McpServer {
  const authManager = new AuthManager(startupConfig);
  const client = new DocmostClient(authManager);
  const server = new McpServer({
    name: "docmost-local-mcp",
    version: "0.1.0",
  });

  server.registerTool(
    "list_spaces",
    {
      title: "List Docmost Spaces",
      description:
        "List all available documentation spaces in Docmost, including names, slugs, and IDs.",
      annotations: {
        readOnlyHint: true,
      },
    },
    async () => {
      const spaces = await client.listSpaces();

      if (spaces.length === 0) {
        return textResult("No Docmost spaces were found.");
      }

      const lines = [
        "## Available Documentation Spaces",
        "",
        "| Name | Slug | ID |",
        "| --- | --- | --- |",
      ];

      for (const space of spaces) {
        lines.push(`| ${space.name} | ${space.slug} | ${space.id} |`);
      }

      lines.push("", `Total spaces: ${spaces.length}`);
      return textResult(lines.join("\n"));
    },
  );

  server.registerTool(
    "search_docs",
    {
      title: "Search Docmost",
      description:
        "Search Docmost documentation and optionally filter results by a space ID from list_spaces.",
      inputSchema: {
        query: z.string().min(1).describe("Full-text query to search for."),
        space_id: z
          .string()
          .optional()
          .describe("Optional Docmost space ID to scope the search."),
      },
      annotations: {
        readOnlyHint: true,
      },
    },
    async ({ query, space_id }) => {
      const results = await client.searchDocs(query, space_id);

      if (results.length === 0) {
        return textResult(`No Docmost results were found for "${query}".`);
      }

      const lines = [`## Search Results for "${query}"`, ""];
      const topResults = results.slice(0, 5);

      topResults.forEach((result, index) => {
        const spaceName = result.space?.name ?? "Unknown";
        const preview = sanitizeHighlight(result.highlight);
        const icon = result.icon ? `${result.icon} ` : "";

        lines.push(`### ${index + 1}. ${icon}${result.title}`);
        lines.push(`- Space: ${spaceName}`);
        lines.push(`- Slug ID: \`${result.slugId}\``);
        if (preview) {
          lines.push(`- Preview: ${preview}`);
        }
        lines.push("");
      });

      lines.push(`Showing ${topResults.length} of ${results.length} results.`);
      lines.push("Use `get_page` with a slug ID to retrieve the full page.");
      return textResult(lines.join("\n"));
    },
  );

  server.registerTool(
    "get_page",
    {
      title: "Get Docmost Page",
      description: "Fetch a Docmost page by slug ID and return its content as Markdown.",
      inputSchema: {
        slug_id: z.string().min(1).describe("The page slug ID returned from search_docs."),
      },
      annotations: {
        readOnlyHint: true,
      },
    },
    async ({ slug_id }) => {
      const page = await client.getPage(slug_id);

      if (!page) {
        return textResult(`No Docmost page was found for slug ID "${slug_id}".`);
      }

      const lines = [
        `# ${page.icon ? `${page.icon} ` : ""}${page.title}`,
        "",
        `Space: ${page.space?.name ?? "Unknown"}`,
        `Author: ${page.creator?.name ?? "Unknown"}`,
        `Last updated: ${page.updatedAt?.slice(0, 10) ?? "Unknown"}`,
        "",
        "---",
        "",
        prosemirrorToMarkdown(page.content ?? {}),
      ];

      return textResult(lines.join("\n").trim());
    },
  );

  return server;
}

function textResult(text: string) {
  return {
    content: [{ type: "text" as const, text }],
  };
}

function sanitizeHighlight(value?: string): string {
  if (!value) {
    return "";
  }

  return value.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();
}
