"use client";

import { RichTextEditor } from "@editorcn/editor";
import "@editorcn/editor/style.css";
import { Markdown } from "@tiptap/markdown";
import { useEditor } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";
import { useEffect, useRef } from "react";
import { useLocale } from "@/lib/locale-provider";
import type { MarkdownEditorProps } from "./editor";

const extensions = [
  StarterKit.configure({ link: { openOnClick: false }, underline: false }),
  Markdown,
];

export function RichEditor({ id, label, value, disabled, onChange }: MarkdownEditorProps) {
  const { messages: { markdownEditor: copy } } = useLocale();
  const lastValue = useRef(value);
  const editor = useEditor({
    extensions,
    immediatelyRender: false,
    content: value,
    contentType: "markdown",
    editable: !disabled,
    editorProps: {
      attributes: { id, role: "textbox", "aria-label": label, "aria-multiline": "true" },
    },
    onUpdate: ({ editor }) => {
      const markdown = editor.isEmpty ? "" : editor.getMarkdown();
      lastValue.current = markdown;
      onChange(markdown);
    },
  });

  useEffect(() => {
    if (!editor || lastValue.current === value) return;
    editor.commands.setContent(value, { contentType: "markdown", emitUpdate: false });
    lastValue.current = value;
  }, [editor, value]);

  useEffect(() => { editor?.setEditable(!disabled, false); }, [disabled, editor]);

  if (!editor) return <p role="status">{copy.loading}</p>;
  return (
    <RichTextEditor editor={editor} editable={!disabled} labels={copy.toolbar}>
      <RichTextEditor.Toolbar>
        <RichTextEditor.ControlsGroup>
          <RichTextEditor.Bold />
          <RichTextEditor.Italic />
          <RichTextEditor.H2 />
          <RichTextEditor.BulletList />
          <RichTextEditor.OrderedList />
          <RichTextEditor.Blockquote />
        </RichTextEditor.ControlsGroup>
        <RichTextEditor.ControlsGroup>
          <RichTextEditor.Link />
          <RichTextEditor.Unlink />
        </RichTextEditor.ControlsGroup>
      </RichTextEditor.Toolbar>
      <RichTextEditor.Content className="min-h-56" />
    </RichTextEditor>
  );
}
