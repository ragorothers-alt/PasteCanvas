import { describe, it, expect } from "vitest";
import {
  htmlToTiptap,
  hasRichContent,
  htmlToPlainText,
} from "../adapters/rich-text/HtmlToTiptap";

describe("HtmlToTiptap", () => {
  describe("htmlToTiptap", () => {
    it("converts simple paragraph", () => {
      const doc = htmlToTiptap("<p>Hello world</p>");
      expect(doc.type).toBe("doc");
      expect(doc.content).toHaveLength(1);
      const para = doc.content![0]!;
      expect(para.type).toBe("paragraph");
      expect(para.content![0]!.text).toBe("Hello world");
    });

    it("converts headings with levels", () => {
      const doc = htmlToTiptap("<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>");
      expect(doc.content).toHaveLength(3);
      expect(doc.content![0]!.type).toBe("heading");
      expect(doc.content![0]!.attrs?.level).toBe(1);
      expect(doc.content![1]!.attrs?.level).toBe(2);
      expect(doc.content![2]!.attrs?.level).toBe(3);
    });

    it("converts bold text", () => {
      const doc = htmlToTiptap("<p><strong>bold</strong> text</p>");
      const para = doc.content![0]!;
      expect(para.content).toHaveLength(2);
      const boldNode = para.content![0]!;
      expect(boldNode.text).toBe("bold");
      expect(boldNode.marks).toHaveLength(1);
      expect(boldNode.marks![0]!.type).toBe("bold");
    });

    it("converts italic text", () => {
      const doc = htmlToTiptap("<p><em>italic</em></p>");
      const textNode = doc.content![0]!.content![0]!;
      expect(textNode.marks![0]!.type).toBe("italic");
    });

    it("converts inline code", () => {
      const doc = htmlToTiptap("<p><code>const x = 5</code></p>");
      const textNode = doc.content![0]!.content![0]!;
      expect(textNode.marks![0]!.type).toBe("code");
    });

    it("converts links with href", () => {
      const doc = htmlToTiptap('<p><a href="https://example.com">link</a></p>');
      const textNode = doc.content![0]!.content![0]!;
      expect(textNode.marks![0]!.type).toBe("link");
      expect(textNode.marks![0]!.attrs?.href).toBe("https://example.com");
    });

    it("converts bullet lists", () => {
      const doc = htmlToTiptap("<ul><li>Item 1</li><li>Item 2</li></ul>");
      const list = doc.content![0]!;
      expect(list.type).toBe("bulletList");
      expect(list.content).toHaveLength(2);
      expect(list.content![0]!.type).toBe("listItem");
    });

    it("converts ordered lists", () => {
      const doc = htmlToTiptap("<ol><li>First</li><li>Second</li></ol>");
      expect(doc.content![0]!.type).toBe("orderedList");
    });

    it("converts code blocks", () => {
      const doc = htmlToTiptap("<pre>function hello() {}</pre>");
      const block = doc.content![0]!;
      expect(block.type).toBe("codeBlock");
      expect(block.content![0]!.text).toBe("function hello() {}");
    });

    it("handles empty input", () => {
      const doc = htmlToTiptap("");
      expect(doc.type).toBe("doc");
      expect(doc.content).toHaveLength(1);
      expect(doc.content![0]!.type).toBe("paragraph");
    });

    it("handles plain text without tags", () => {
      const doc = htmlToTiptap("Just plain text");
      const para = doc.content![0]!;
      expect(para.type).toBe("paragraph");
      expect(para.content![0]!.text).toBe("Just plain text");
    });

    it("decodes HTML entities", () => {
      const doc = htmlToTiptap("<p>&amp; &lt; &gt; &quot;</p>");
      expect(doc.content![0]!.content![0]!.text).toBe('& < > "');
    });
  });

  describe("sanitization", () => {
    it("removes script tags", () => {
      const doc = htmlToTiptap('<p>Hello</p><script>alert("xss")</script><p>World</p>');
      const allText = JSON.stringify(doc);
      expect(allText).not.toContain("script");
      expect(allText).not.toContain("alert");
    });

    it("removes event handlers", () => {
      const doc = htmlToTiptap('<p onclick="alert(1)">Click me</p>');
      const allText = JSON.stringify(doc);
      expect(allText).not.toContain("onclick");
      expect(allText).toContain("Click me");
    });

    it("neutralizes javascript: URLs", () => {
      const doc = htmlToTiptap('<p><a href="javascript:alert(1)">bad link</a></p>');
      const textNode = doc.content![0]!.content![0]!;
      const linkMark = textNode.marks?.[0];
      expect(linkMark?.attrs?.href).toBe("#");
    });
  });

  describe("hasRichContent", () => {
    it("detects rich content", () => {
      expect(hasRichContent("<p><strong>bold</strong></p>")).toBe(true);
      expect(hasRichContent("<h1>Title</h1>")).toBe(true);
      expect(hasRichContent("<ul><li>item</li></ul>")).toBe(true);
    });

    it("rejects plain text", () => {
      expect(hasRichContent("just plain text")).toBe(false);
      expect(hasRichContent("")).toBe(false);
    });
  });

  describe("htmlToPlainText", () => {
    it("extracts text from HTML", () => {
      expect(htmlToPlainText("<p>Hello <strong>world</strong></p>")).toBe("Hello world");
    });

    it("strips all tags", () => {
      expect(htmlToPlainText("<div><h1>Title</h1><p>Body</p></div>")).toBe("TitleBody");
    });

    it("decodes entities", () => {
      expect(htmlToPlainText("<p>&amp; &lt; &gt;</p>")).toBe("& < >");
    });
  });
});
