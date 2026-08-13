import { useRef, useEffect, useCallback } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { oneDark } from "@codemirror/theme-one-dark";
import {
  detectLanguage,
  getLanguageExtension,
  languageDisplayName,
} from "./CodeMirrorAdapter";
import type { SupportedLanguage } from "./CodeMirrorAdapter";

interface CodeEditorProps {
  code: string;
  language?: SupportedLanguage;
  editable?: boolean;
  onChange?: (code: string) => void;
}

/**
 * CodeEditor — CodeMirror 6 based code editor component.
 * Per GOAL.md: "Code editor：CodeMirror 6"
 * Per AGENTS.md: "editors mount only when selected and editing"
 */
export function CodeEditor({
  code,
  language,
  editable = false,
  onChange,
}: CodeEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const detectedLang = language ?? detectLanguage(code);

  const createEditor = useCallback(() => {
    if (!containerRef.current) return;

    // Destroy existing editor
    if (viewRef.current) {
      viewRef.current.destroy();
      viewRef.current = null;
    }

    const langExtension = getLanguageExtension(detectedLang);

    const extensions = [
      lineNumbers(),
      highlightActiveLine(),
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      oneDark,
      EditorView.editable.of(editable),
      EditorView.theme({
        "&": { fontSize: "13px", maxHeight: "400px" },
        ".cm-content": { fontFamily: "'Cascadia Code', 'Fira Code', monospace" },
        ".cm-gutters": { backgroundColor: "#1a1a2e", border: "none" },
      }),
    ];

    if (langExtension) {
      extensions.push(langExtension);
    }

    if (onChange && editable) {
      extensions.push(
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChange(update.state.doc.toString());
          }
        }),
      );
    }

    const state = EditorState.create({ doc: code, extensions });
    viewRef.current = new EditorView({ state, parent: containerRef.current });
  }, [code, detectedLang, editable, onChange]);

  useEffect(() => {
    createEditor();
    return () => {
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, [createEditor]);

  return (
    <div className="code-editor-container">
      <div className="code-editor-header">
        <span className="code-lang-badge">{languageDisplayName(detectedLang)}</span>
      </div>
      <div ref={containerRef} className="code-editor-mount" />
    </div>
  );
}
