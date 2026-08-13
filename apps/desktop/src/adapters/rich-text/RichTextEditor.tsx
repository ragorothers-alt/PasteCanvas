import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useCallback, useEffect } from "react";
import type { RichTextDocument } from "../../adapters/rich-text/RichTextAdapter";

interface RichTextEditorProps {
  content: RichTextDocument | null;
  editable?: boolean;
  onChange?: (doc: RichTextDocument) => void;
  placeholder?: string;
}

/**
 * RichTextEditor — Tiptap-based rich text editor component.
 * Per AGENTS.md: "editors mount only when selected and editing"
 * Per AGENTS.md: "Editors save through explicit command or debounced application mutation"
 */
export function RichTextEditor({
  content,
  editable = true,
  onChange,
  placeholder = "Start typing…",
}: RichTextEditorProps) {
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
        codeBlock: { HTMLAttributes: { class: "code-block" } },
      }),
    ],
    content: content ?? undefined,
    editable,
    editorProps: {
      attributes: {
        class: "rich-text-editor",
        "data-placeholder": placeholder,
      },
    },
    onUpdate: ({ editor: ed }) => {
      if (onChange) {
        const json = ed.getJSON() as RichTextDocument;
        onChange(json);
      }
    },
  });

  // Update content when prop changes externally
  useEffect(() => {
    if (editor && content && !editor.isFocused) {
      editor.commands.setContent(content);
    }
  }, [editor, content]);

  const handleBold = useCallback(() => {
    editor?.chain().focus().toggleBold().run();
  }, [editor]);

  const handleItalic = useCallback(() => {
    editor?.chain().focus().toggleItalic().run();
  }, [editor]);

  const handleCode = useCallback(() => {
    editor?.chain().focus().toggleCode().run();
  }, [editor]);

  if (!editor) return null;

  return (
    <div className="rich-text-container">
      {editable && (
        <div className="rich-text-toolbar">
          <button
            className={`toolbar-btn ${editor.isActive("bold") ? "active" : ""}`}
            onClick={handleBold}
            title="Bold (Ctrl+B)"
          >
            <strong>B</strong>
          </button>
          <button
            className={`toolbar-btn ${editor.isActive("italic") ? "active" : ""}`}
            onClick={handleItalic}
            title="Italic (Ctrl+I)"
          >
            <em>I</em>
          </button>
          <button
            className={`toolbar-btn ${editor.isActive("code") ? "active" : ""}`}
            onClick={handleCode}
            title="Code"
          >
            {"</>"}
          </button>
          <button
            className={`toolbar-btn ${editor.isActive("heading", { level: 1 }) ? "active" : ""}`}
            onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
            title="Heading 1"
          >
            H1
          </button>
          <button
            className={`toolbar-btn ${editor.isActive("bulletList") ? "active" : ""}`}
            onClick={() => editor.chain().focus().toggleBulletList().run()}
            title="Bullet List"
          >
            •
          </button>
        </div>
      )}
      <EditorContent editor={editor} />
    </div>
  );
}
