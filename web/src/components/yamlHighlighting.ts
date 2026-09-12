import type { EditorState } from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";
import {
  Decoration,
  ViewPlugin,
  type DecorationSet,
  type EditorView,
  type ViewUpdate,
} from "@codemirror/view";

// The YAML grammar tags all plain scalars as content. Classify only parsed value
// nodes, so numbers inside comments, quoted strings, keys or CDL expressions stay intact.
export function scalarMarks(
  state: EditorState,
  from = 0,
  to = state.doc.length,
) {
  const marks: { from: number; to: number; className: string }[] = [];
  syntaxTree(state).iterate({
    from,
    to,
    enter(node) {
      if (node.name !== "Literal" || node.node.parent?.name === "Key") return;
      const value = state.sliceDoc(node.from, node.to);
      if (/^(?:true|false|null|~)$/i.test(value)) {
        marks.push({
          from: node.from,
          to: node.to,
          className: "cm-yaml-keyword",
        });
      } else if (
        /^[+-]?(?:0x[\da-f_]+|0o[0-7_]+|(?:\d[\d_]*(?:\.[\d_]*)?|\.\d[\d_]*)(?:e[+-]?\d+)?|\.inf|\.nan)$/i.test(
          value,
        )
      ) {
        marks.push({
          from: node.from,
          to: node.to,
          className: "cm-yaml-number",
        });
      }
    },
  });
  return marks;
}

function decorations(view: EditorView): DecorationSet {
  const marks = view.visibleRanges.flatMap((range) =>
    scalarMarks(view.state, range.from, range.to),
  );
  return Decoration.set(
    marks.map((mark) =>
      Decoration.mark({ class: mark.className }).range(mark.from, mark.to),
    ),
    true,
  );
}

export const yamlScalarHighlighting = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorations(view);
    }
    update(update: ViewUpdate) {
      if (
        update.docChanged ||
        update.viewportChanged ||
        syntaxTree(update.startState) !== syntaxTree(update.state)
      ) {
        this.decorations = decorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
