/**
 * Rich Text Adapter — wraps Tiptap/ProseMirror behind a stable interface.
 * Per AGENTS.md: "Rich text：Tiptap／ProseMirror"
 * Per AGENTS.md: "adapters/rich-text/" isolates editor-specific types.
 *
 * The adapter provides:
 * - JSON document serialization (Tiptap JSON format)
 * - Plain text extraction for search indexing
 * - HTML sanitization for display
 */

export interface RichTextDocument {
  type: string;
  content?: RichTextNode[];
  [key: string]: unknown;
}

export interface RichTextNode {
  type: string;
  content?: RichTextNode[];
  text?: string;
  marks?: Array<{ type: string; attrs?: Record<string, unknown> }>;
  attrs?: Record<string, unknown>;
}

/** Extracts plain text from a Tiptap JSON document for search indexing. */
export function extractPlainText(doc: RichTextDocument): string {
  if (!doc.content) return "";
  return doc.content
    .map((node) => extractNodeText(node))
    .filter(Boolean)
    .join("\n");
}

function extractNodeText(node: RichTextNode): string {
  if (node.text) return node.text;
  if (!node.content) return "";
  return node.content.map((child) => extractNodeText(child)).join("");
}

/** Creates an empty Tiptap document. */
export function createEmptyDocument(): RichTextDocument {
  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [],
      },
    ],
  };
}

/** Creates a document from plain text (for migration/initial content). */
export function documentFromPlainText(text: string): RichTextDocument {
  const paragraphs = text.split("\n").map((line) => ({
    type: "paragraph" as const,
    content: line ? [{ type: "text" as const, text: line }] : [],
  }));

  return {
    type: "doc",
    content: paragraphs.length > 0 ? paragraphs : [{ type: "paragraph", content: [] }],
  };
}

/** Validates that a value is a well-formed Tiptap document. */
export function isValidDocument(value: unknown): value is RichTextDocument {
  if (typeof value !== "object" || value === null) return false;
  const doc = value as Record<string, unknown>;
  return doc.type === "doc" && Array.isArray(doc.content);
}
