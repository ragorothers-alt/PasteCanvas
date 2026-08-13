import { describe, it, expect } from "vitest";
import {
  extractPlainText,
  createEmptyDocument,
  documentFromPlainText,
  isValidDocument,
} from "../adapters/rich-text/RichTextAdapter";
import type { RichTextDocument } from "../adapters/rich-text/RichTextAdapter";

describe("RichTextAdapter", () => {
  describe("extractPlainText", () => {
    it("extracts text from paragraphs", () => {
      const doc: RichTextDocument = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Hello world" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Second line" }],
          },
        ],
      };
      expect(extractPlainText(doc)).toBe("Hello world\nSecond line");
    });

    it("handles empty document", () => {
      const doc: RichTextDocument = { type: "doc", content: [] };
      expect(extractPlainText(doc)).toBe("");
    });

    it("handles document without content", () => {
      const doc: RichTextDocument = { type: "doc" };
      expect(extractPlainText(doc)).toBe("");
    });

    it("extracts nested text (bold/italic marks)", () => {
      const doc: RichTextDocument = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [
              { type: "text", text: "Normal " },
              { type: "text", text: "bold", marks: [{ type: "bold" }] },
              { type: "text", text: " text" },
            ],
          },
        ],
      };
      expect(extractPlainText(doc)).toBe("Normal bold text");
    });
  });

  describe("createEmptyDocument", () => {
    it("creates valid empty doc", () => {
      const doc = createEmptyDocument();
      expect(doc.type).toBe("doc");
      expect(doc.content).toHaveLength(1);
      expect(doc.content![0]!.type).toBe("paragraph");
      expect(isValidDocument(doc)).toBe(true);
    });
  });

  describe("documentFromPlainText", () => {
    it("converts single line", () => {
      const doc = documentFromPlainText("Hello");
      expect(doc.content).toHaveLength(1);
      expect(doc.content![0]!.content![0]!.text).toBe("Hello");
    });

    it("converts multiple lines to paragraphs", () => {
      const doc = documentFromPlainText("Line 1\nLine 2\nLine 3");
      expect(doc.content).toHaveLength(3);
      expect(doc.content![0]!.content![0]!.text).toBe("Line 1");
      expect(doc.content![2]!.content![0]!.text).toBe("Line 3");
    });

    it("handles empty string", () => {
      const doc = documentFromPlainText("");
      expect(doc.content).toHaveLength(1);
      expect(doc.content![0]!.content).toHaveLength(0);
    });

    it("round-trips through extractPlainText", () => {
      const original = "First paragraph\nSecond paragraph";
      const doc = documentFromPlainText(original);
      const extracted = extractPlainText(doc);
      expect(extracted).toBe(original);
    });
  });

  describe("isValidDocument", () => {
    it("validates correct document", () => {
      expect(isValidDocument({ type: "doc", content: [] })).toBe(true);
    });

    it("rejects null", () => {
      expect(isValidDocument(null)).toBe(false);
    });

    it("rejects non-object", () => {
      expect(isValidDocument("string")).toBe(false);
      expect(isValidDocument(42)).toBe(false);
    });

    it("rejects wrong type", () => {
      expect(isValidDocument({ type: "paragraph", content: [] })).toBe(false);
    });

    it("rejects missing content array", () => {
      expect(isValidDocument({ type: "doc" })).toBe(false);
      expect(isValidDocument({ type: "doc", content: "not array" })).toBe(false);
    });
  });
});
