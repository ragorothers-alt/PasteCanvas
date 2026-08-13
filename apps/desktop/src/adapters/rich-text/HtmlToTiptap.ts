/**
 * HTML to Tiptap JSON document converter.
 * Converts clipboard HTML content to Tiptap's ProseMirror JSON format.
 * Per GOAL.md: "Rich text：Tiptap／ProseMirror"
 * Per AGENTS.md: "HTML、SVG 與 rich text 一律視為不可信輸入"
 *
 * Security: strips script tags, event handlers, and dangerous attributes.
 */

import type { RichTextDocument, RichTextNode } from "./RichTextAdapter";

/**
 * Converts an HTML string to a Tiptap JSON document.
 * Sanitizes dangerous content (scripts, event handlers).
 */
export function htmlToTiptap(html: string): RichTextDocument {
  const sanitized = sanitizeHtml(html);
  const content = parseHtmlToNodes(sanitized);

  return {
    type: "doc",
    content: content.length > 0 ? content : [{ type: "paragraph", content: [] }],
  };
}

/** Removes dangerous HTML elements and attributes. */
function sanitizeHtml(html: string): string {
  let result = html;

  // Remove script/style/iframe/object/embed elements entirely
  result = result.replace(/<(script|style|iframe|object|embed|form|input|button)[^>]*>[\s\S]*?<\/\1>/gi, "");
  result = result.replace(/<(script|style|iframe|object|embed|form|input|button)[^>]*\/?>/gi, "");

  // Remove event handler attributes (on*)
  result = result.replace(/\s+on\w+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, "");

  // Remove javascript: URLs
  result = result.replace(/href\s*=\s*(?:"javascript:[^"]*"|'javascript:[^']*')/gi, 'href="#"');

  // Remove data: URLs (potential XSS vector)
  result = result.replace(/src\s*=\s*(?:"data:[^"]*"|'data:[^']*')/gi, "");

  return result;
}

/** Parses sanitized HTML into Tiptap node array. */
function parseHtmlToNodes(html: string): RichTextNode[] {
  const nodes: RichTextNode[] = [];

  // Extract block-level elements
  const blockRegex = /<(h[1-6]|p|ul|ol|pre|blockquote|div|hr)([^>]*)>([\s\S]*?)<\/\1>|<(hr)\s*\/?>/gi;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = blockRegex.exec(html)) !== null) {
    // Handle text between blocks
    const betweenText = html.slice(lastIndex, match.index).trim();
    if (betweenText) {
      const textContent = stripHtml(betweenText).trim();
      if (textContent) {
        nodes.push({
          type: "paragraph",
          content: parseInlineContent(betweenText),
        });
      }
    }

    const tag = (match[1] || match[4] || "").toLowerCase();
    const innerHtml = match[3] || "";

    switch (tag) {
      case "h1":
      case "h2":
      case "h3":
      case "h4":
      case "h5":
      case "h6":
        nodes.push({
          type: "heading",
          attrs: { level: parseInt(tag[1] ?? "1") },
          content: parseInlineContent(innerHtml),
        });
        break;

      case "p":
      case "div":
        nodes.push({
          type: "paragraph",
          content: parseInlineContent(innerHtml),
        });
        break;

      case "ul":
        nodes.push({
          type: "bulletList",
          content: parseListItems(innerHtml),
        });
        break;

      case "ol":
        nodes.push({
          type: "orderedList",
          content: parseListItems(innerHtml),
        });
        break;

      case "pre":
        nodes.push({
          type: "codeBlock",
          content: [{ type: "text", text: stripHtml(innerHtml) }],
        });
        break;

      case "blockquote": {
        const innerNodes = parseInlineContent(innerHtml);
        nodes.push({
          type: "blockquote",
          content: innerNodes.length > 0
            ? [{ type: "paragraph", content: innerNodes }]
            : [{ type: "paragraph", content: [] }],
        });
        break;
      }

      case "hr":
        nodes.push({ type: "horizontalRule" });
        break;
    }

    lastIndex = match.index + match[0].length;
  }

  // Handle remaining text after last block
  const remainingText = html.slice(lastIndex).trim();
  if (remainingText) {
    const textContent = stripHtml(remainingText).trim();
    if (textContent) {
      nodes.push({
        type: "paragraph",
        content: parseInlineContent(remainingText),
      });
    }
  }

  return nodes;
}

/** Parses inline content (text with marks like bold, italic, code, links). */
function parseInlineContent(html: string): RichTextNode[] {
  const nodes: RichTextNode[] = [];
  const inlineRegex = /<(strong|b|em|i|code|a|span)([^>]*)>([\s\S]*?)<\/\1>/gi;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = inlineRegex.exec(html)) !== null) {
    // Handle text before this inline element
    const beforeText = html.slice(lastIndex, match.index);
    if (beforeText) {
      const text = stripHtml(beforeText);
      if (text) {
        nodes.push({ type: "text", text });
      }
    }

    const tag = (match[1] ?? "").toLowerCase();
    const attrs = match[2] ?? "";
    const inner = match[3] ?? "";
    const text = stripHtml(inner);

    if (text) {
      const marks: Array<{ type: string; attrs?: Record<string, unknown> }> = [];

      switch (tag) {
        case "strong":
        case "b":
          marks.push({ type: "bold" });
          break;
        case "em":
        case "i":
          marks.push({ type: "italic" });
          break;
        case "code":
          marks.push({ type: "code" });
          break;
        case "a": {
          const hrefMatch = attrs.match(/href\s*=\s*(?:"([^"]*)"|'([^']*)')/i);
          const href = hrefMatch?.[1] || hrefMatch?.[2] || "#";
          marks.push({ type: "link", attrs: { href } });
          break;
        }
      }

      const node: RichTextNode = { type: "text", text };
      if (marks.length > 0) {
        node.marks = marks;
      }
      nodes.push(node);
    }

    lastIndex = match.index + match[0].length;
  }

  // Handle remaining text after last inline element
  const remainingText = html.slice(lastIndex);
  if (remainingText) {
    const text = stripHtml(remainingText);
    if (text) {
      nodes.push({ type: "text", text });
    }
  }

  return nodes;
}

/** Parses list items from ul/ol content. */
function parseListItems(html: string): RichTextNode[] {
  const items: RichTextNode[] = [];
  const liRegex = /<li[^>]*>([\s\S]*?)<\/li>/gi;
  let match: RegExpExecArray | null;

  while ((match = liRegex.exec(html)) !== null) {
    const inner = match[1] ?? "";
    items.push({
      type: "listItem",
      content: [{
        type: "paragraph",
        content: parseInlineContent(inner),
      }],
    });
  }

  return items.length > 0 ? items : [{
    type: "listItem",
    content: [{ type: "paragraph", content: [] }],
  }];
}

/** Strips all HTML tags, returning plain text. */
function stripHtml(html: string): string {
  return html
    .replace(/<[^>]*>/g, "")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, " ");
}

/** Checks if HTML content has meaningful rich text (not just plain text). */
export function hasRichContent(html: string): boolean {
  const sanitized = sanitizeHtml(html);
  const tagRegex = /<(?!br\s*\/?)[a-z][^>]*>/i;
  return tagRegex.test(sanitized);
}

/** Extracts plain text from HTML (for search indexing). */
export function htmlToPlainText(html: string): string {
  return stripHtml(sanitizeHtml(html)).trim();
}
