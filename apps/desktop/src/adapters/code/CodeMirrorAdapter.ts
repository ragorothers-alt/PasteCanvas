/**
 * CodeMirror Adapter — CodeMirror-specific extension configuration.
 * Per GOAL.md: "Code editor：CodeMirror 6"
 * Per AGENTS.md: adapters isolate editor-specific types from the rest of the app.
 * Pure detection/display helpers live in languageDetect.ts (no CodeMirror deps);
 * they are re-exported here for backwards compatibility.
 */
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import type { Extension } from "@codemirror/state";
import type { SupportedLanguage } from "./languageDetect";

export type { SupportedLanguage };
export { detectLanguage, languageDisplayName, SUPPORTED_LANGUAGES } from "./languageDetect";

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
