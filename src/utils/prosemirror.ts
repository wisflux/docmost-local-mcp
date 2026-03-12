type ProseMirrorNode = {
  type?: string;
  attrs?: Record<string, unknown>;
  text?: string;
  marks?: Array<{
    type?: string;
    attrs?: Record<string, unknown>;
  }>;
  content?: ProseMirrorNode[];
};

export function prosemirrorToMarkdown(content: unknown): string {
  if (!isDocumentNode(content)) {
    return "";
  }

  return convertNodes(content.content ?? []).trim();
}

function isDocumentNode(value: unknown): value is ProseMirrorNode {
  return typeof value === "object" && value !== null && (value as ProseMirrorNode).type === "doc";
}

function convertNodes(nodes: ProseMirrorNode[], indent = 0): string {
  const output: string[] = [];

  for (const node of nodes) {
    switch (node.type) {
      case "paragraph": {
        const text = extractText(node.content ?? []);
        if (text) {
          output.push(text, "");
        }
        break;
      }
      case "heading": {
        const level = Number(node.attrs?.level ?? 1);
        output.push(`${"#".repeat(level)} ${extractText(node.content ?? [])}`, "");
        break;
      }
      case "bulletList": {
        output.push(convertList(node.content ?? [], false, indent), "");
        break;
      }
      case "orderedList": {
        output.push(convertList(node.content ?? [], true, indent), "");
        break;
      }
      case "codeBlock": {
        const language = typeof node.attrs?.language === "string" ? node.attrs.language : "";
        output.push(`\`\`\`${language}`, extractText(node.content ?? []), "```", "");
        break;
      }
      case "blockquote": {
        const inner = convertNodes(node.content ?? []);
        const quoted = inner
          .split("\n")
          .filter(Boolean)
          .map((line) => `> ${line}`)
          .join("\n");
        if (quoted) {
          output.push(quoted, "");
        }
        break;
      }
      case "horizontalRule": {
        output.push("---", "");
        break;
      }
      case "table": {
        const table = convertTable(node);
        if (table) {
          output.push(table, "");
        }
        break;
      }
      case "image": {
        const src = typeof node.attrs?.src === "string" ? node.attrs.src : "";
        if (src) {
          const alt = typeof node.attrs?.alt === "string" ? node.attrs.alt : "image";
          output.push(`![${alt}](${src})`, "");
        }
        break;
      }
      case "embed": {
        const src = typeof node.attrs?.src === "string" ? node.attrs.src : "";
        if (src) {
          output.push(`[Embedded content](${src})`, "");
        }
        break;
      }
      default:
        break;
    }
  }

  return output.join("\n").trimEnd();
}

function extractText(content: ProseMirrorNode[]): string {
  const parts: string[] = [];

  for (const item of content) {
    if (item.type === "text") {
      let text = item.text ?? "";

      for (const mark of item.marks ?? []) {
        switch (mark.type) {
          case "bold":
            text = `**${text}**`;
            break;
          case "italic":
            text = `*${text}*`;
            break;
          case "code":
            text = `\`${text}\``;
            break;
          case "strike":
            text = `~~${text}~~`;
            break;
          case "link": {
            const href = typeof mark.attrs?.href === "string" ? mark.attrs.href : "";
            text = href ? `[${text}](${href})` : text;
            break;
          }
          default:
            break;
        }
      }

      parts.push(text);
      continue;
    }

    if (item.type === "hardBreak") {
      parts.push("\n");
      continue;
    }

    if (item.content?.length) {
      parts.push(extractText(item.content));
    }
  }

  return parts.join("");
}

function convertList(items: ProseMirrorNode[], ordered: boolean, indent: number): string {
  const output: string[] = [];
  const prefix = " ".repeat(indent);

  items.forEach((item, index) => {
    if (item.type !== "listItem") {
      return;
    }

    const itemContent = item.content ?? [];
    let firstParagraph = "";
    const nestedLists: ProseMirrorNode[] = [];

    for (const child of itemContent) {
      if (child.type === "paragraph" && !firstParagraph) {
        firstParagraph = extractText(child.content ?? []);
      } else if (child.type === "bulletList" || child.type === "orderedList") {
        nestedLists.push(child);
      }
    }

    const marker = ordered ? `${index + 1}. ` : "- ";
    output.push(`${prefix}${marker}${firstParagraph}`);

    for (const nestedList of nestedLists) {
      output.push(convertNodes([nestedList], indent + 2));
    }
  });

  return output.join("\n");
}

function convertTable(tableNode: ProseMirrorNode): string {
  const rows: string[][] = [];

  for (const row of tableNode.content ?? []) {
    if (row.type !== "tableRow") {
      continue;
    }

    const cells: string[] = [];
    for (const cell of row.content ?? []) {
      if (cell.type !== "tableCell" && cell.type !== "tableHeader") {
        continue;
      }

      const cellText = convertNodes(cell.content ?? []).replace(/\s+/g, " ").trim();
      cells.push(cellText);
    }

    if (cells.length > 0) {
      rows.push(cells);
    }
  }

  if (rows.length === 0) {
    return "";
  }

  const firstRow = rows[0];
  if (!firstRow) {
    return "";
  }

  const lines: string[] = [];
  lines.push(`| ${firstRow.join(" | ")} |`);
  lines.push(`| ${firstRow.map(() => "---").join(" | ")} |`);

  for (const row of rows.slice(1)) {
    lines.push(`| ${row.join(" | ")} |`);
  }

  return lines.join("\n");
}
