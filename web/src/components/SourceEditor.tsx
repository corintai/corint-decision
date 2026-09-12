import { useLayoutEffect, useRef } from "react";
import { Annotation, EditorState, Transaction } from "@codemirror/state";
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  HighlightStyle,
  bracketMatching,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { yaml } from "@codemirror/lang-yaml";
import { tags } from "@lezer/highlight";
import { yamlScalarHighlighting } from "./yamlHighlighting";

const highlighting = HighlightStyle.define([
  { tag: tags.propertyName, color: "var(--syntax-key)" },
  { tag: [tags.string, tags.content], color: "var(--syntax-string)" },
  { tag: tags.number, color: "var(--syntax-number)" },
  { tag: [tags.bool, tags.null, tags.keyword], color: "var(--syntax-keyword)" },
  { tag: tags.comment, color: "var(--syntax-comment)", fontStyle: "italic" },
  {
    tag: [tags.meta, tags.labelName, tags.typeName],
    color: "var(--syntax-meta)",
  },
  {
    tag: [tags.punctuation, tags.operator],
    color: "var(--syntax-punctuation)",
  },
]);

export default function SourceEditor({
  source,
  onChange,
  error,
}: {
  source: string;
  onChange: (source: string) => void;
  error: string | null;
}) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const currentSource = useRef(source);
  const externalUpdate = useRef(Annotation.define<boolean>()).current;
  const changeHandler = useRef(onChange);

  useLayoutEffect(() => {
    changeHandler.current = onChange;
    currentSource.current = source;
  }, [onChange, source]);
  useLayoutEffect(() => {
    if (!host.current) return;
    const view = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: currentSource.current,
        extensions: [
          yaml(),
          syntaxHighlighting(highlighting),
          yamlScalarHighlighting,
          lineNumbers(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          drawSelection(),
          bracketMatching(),
          history(),
          indentUnit.of("  "),
          EditorState.tabSize.of(2),
          keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
          EditorView.contentAttributes.of({
            "aria-label": "YAML 源码",
            spellcheck: "false",
          }),
          EditorView.updateListener.of((update) => {
            if (
              update.docChanged &&
              !update.transactions.some((transaction) =>
                transaction.annotation(externalUpdate),
              )
            ) {
              changeHandler.current(update.state.doc.toString());
            }
          }),
        ],
      }),
    });
    editor.current = view;
    return () => {
      editor.current = null;
      view.destroy();
    };
  }, [externalUpdate]);

  // Form edits and toolbar undo must update the same document without echoing another edit.
  useLayoutEffect(() => {
    const view = editor.current;
    if (!view || view.state.doc.toString() === source) return;
    const { anchor, head } = view.state.selection.main;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: source },
      selection: {
        anchor: Math.min(anchor, source.length),
        head: Math.min(head, source.length),
      },
      annotations: [
        externalUpdate.of(true),
        Transaction.addToHistory.of(false),
      ],
    });
  }, [source, externalUpdate]);

  return (
    <div className="source-pane">
      <div className="source-caption">
        <span>YAML SOURCE</span>
        <span>UTF-8 · 2 spaces</span>
      </div>
      {error && (
        <div className="source-error" role="alert">
          {error}
        </div>
      )}
      <div className="code-editor" ref={host} />
    </div>
  );
}
