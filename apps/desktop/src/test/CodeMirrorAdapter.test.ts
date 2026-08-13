import { describe, it, expect } from "vitest";
import {
  detectLanguage,
  languageDisplayName,
  SUPPORTED_LANGUAGES,
} from "../adapters/code/CodeMirrorAdapter";

describe("CodeMirrorAdapter", () => {
  describe("detectLanguage", () => {
    it("detects Rust code", () => {
      expect(detectLanguage("fn main() {\n    println!(\"hello\");\n}")).toBe("rust");
      expect(detectLanguage("use std::io;")).toBe("rust");
      expect(detectLanguage("pub struct Item { id: u64 }")).toBe("rust");
      expect(detectLanguage("impl Display for Item { }")).toBe("rust");
      expect(detectLanguage("let mut x = 5;")).toBe("rust");
    });

    it("detects Python code", () => {
      expect(detectLanguage("def hello():\n    print('hi')")).toBe("python");
      expect(detectLanguage("import os")).toBe("python");
      expect(detectLanguage("from pathlib import Path")).toBe("python");
      expect(detectLanguage("class MyClass:\n    pass")).toBe("python");
    });

    it("detects JavaScript code", () => {
      expect(detectLanguage("const x = 5;")).toBe("javascript");
      expect(detectLanguage("function hello() { return 1; }")).toBe("javascript");
      expect(detectLanguage("console.log('hi')")).toBe("javascript");
      expect(detectLanguage("const fn = () => 42;")).toBe("javascript");
    });

    it("detects TypeScript code", () => {
      expect(detectLanguage("const x: string = 'hello';")).toBe("typescript");
      expect(detectLanguage("interface Foo { bar: number }")).toBe("typescript");
      expect(detectLanguage("export type Result<T> = { ok: T }")).toBe("typescript");
    });

    it("returns plaintext for unknown code", () => {
      expect(detectLanguage("just some plain text")).toBe("plaintext");
      expect(detectLanguage("")).toBe("plaintext");
      expect(detectLanguage("SELECT * FROM users")).toBe("plaintext");
    });
  });

  describe("languageDisplayName", () => {
    it("returns friendly names", () => {
      expect(languageDisplayName("javascript")).toBe("JavaScript");
      expect(languageDisplayName("typescript")).toBe("TypeScript");
      expect(languageDisplayName("python")).toBe("Python");
      expect(languageDisplayName("rust")).toBe("Rust");
      expect(languageDisplayName("plaintext")).toBe("Plain Text");
    });
  });

  describe("SUPPORTED_LANGUAGES", () => {
    it("contains all expected languages", () => {
      expect(SUPPORTED_LANGUAGES).toContain("javascript");
      expect(SUPPORTED_LANGUAGES).toContain("typescript");
      expect(SUPPORTED_LANGUAGES).toContain("python");
      expect(SUPPORTED_LANGUAGES).toContain("rust");
      expect(SUPPORTED_LANGUAGES).toContain("plaintext");
      expect(SUPPORTED_LANGUAGES).toHaveLength(5);
    });
  });
});
