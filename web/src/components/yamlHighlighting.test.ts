import test from "node:test";
import assert from "node:assert/strict";
import { EditorState } from "@codemirror/state";
import { yaml } from "@codemirror/lang-yaml";
import { scalarMarks } from "./yamlHighlighting";

function markedValues(doc: string) {
  const state = EditorState.create({ doc, extensions: [yaml()] });
  return scalarMarks(state).map((mark) => [
    state.sliceDoc(mark.from, mark.to),
    mark.className,
  ]);
}

test("YAML scalar highlighting distinguishes values from keys, strings, comments and CDL expressions", () => {
  assert.deepEqual(
    markedValues(
      'score: -50\ndefault: true\nfallback: null\nratio: 1.5e2\nquoted: "true"\ncondition: event.amount > 100\n# score: 200\n123: plain text\n',
    ),
    [
      ["-50", "cm-yaml-number"],
      ["true", "cm-yaml-keyword"],
      ["null", "cm-yaml-keyword"],
      ["1.5e2", "cm-yaml-number"],
    ],
  );
});

test("incomplete YAML remains highlightable without treating block-string contents as scalars", () => {
  assert.deepEqual(
    markedValues("description: |\n  true\n  123\nscore: 100\nwhen: ["),
    [["100", "cm-yaml-number"]],
  );
});
