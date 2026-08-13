/**
 * CodeMirror Adapter — language detection and extension configuration.
 * Per GOAL.md: "Code editor：CodeMirror 6"
 * Per AGENTS.md: adapters isolate editor-specific types from the rest of the app.
 */
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import type { Extension } from "@codemirror/state";

export type SupportedLanguage =
  | "javascript"
  | "typescript"
  | "python"
  | "rust"
  | "plaintext";

/** Detects programming language from content heuristics. */
export function detectLanguage(code: string): SupportedLanguage {
  const trimmed = code.trimStart();

  // Rust indicators
  if (
    trimmed.startsWith("fn ") ||
    trimmed.startsWith("use ") ||
    trimmed.startsWith("pub ") ||
    trimmed.startsWith("impl ") ||
    trimmed.startsWith("struct ") ||
    trimmed.startsWith("enum ") ||
    trimmed.includes("-> ") ||
    trimmed.includes("let mut ")
  ) {
    return "rust";
  }

  // Python indicators
  if (
    trimmed.startsWith("def ") ||
    trimmed.startsWith("import ") ||
    trimmed.startsWith("from ") ||
    trimmed.startsWith("class ") ||
    trimmed.includes(":\n") ||
    trimmed.startsWith("print(")
  ) {
    return "python";
  }

  // JavaScript/TypeScript indicators
  if (
    trimmed.startsWith("const ") ||
    trimmed.startsWith("let ") ||
    trimmed.startsWith("var ") ||
    trimmed.startsWith("function ") ||
    trimmed.startsWith("export ") ||
    trimmed.startsWith("import ") ||
    trimmed.startsWith("interface ") ||
    trimmed.startsWith("type ") ||
    trimmed.includes("=>") ||
    trimmed.includes("console.log")
  ) {
    // Distinguish TS from JS
    if (
      trimmed.includes(": string") ||
      trimmed.includes(": number") ||
      trimmed.includes("interface ") ||
      trimmed.includes("type ") ||
      trimmed.includes("<T>")
    ) {
      return "typescript";
    }
    return "javascript";
  }

  return "plaintext";
}

/** Returns the CodeMirror language extension for a given language. */
export function getLanguageExtension(lang: SupportedLanguage): Extension | null {
  switch (lang) {
    case "javascript":
      return javascript();
    case "typescript":
      return javascript({ typescript: true });
    case "python":
      return python();
    case "rust":
      return rust();
    case "plaintext":
      return null;
  }
}

/** Returns a display-friendly language name. */
export function languageDisplayName(lang: SupportedLanguage): string {
  switch (lang) {
    case "javascript":
      return "JavaScript";
    case "typescript":
      return "TypeScript";
    case "python":
      return "Python";
    case "rust":
      return "Rust";
    case "plaintext":
      return "Plain Text";
  }
}

/** All supported languages for the language selector. */
export const SUPPORTED_LANGUAGES: SupportedLanguage[] = [
  "javascript",
  "typescript",
  "python",
  "rust",
  "plaintext",
];
