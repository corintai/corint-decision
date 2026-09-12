import test from "node:test";
import assert from "node:assert/strict";
import {
  parseSource,
  patchSource,
  detectKind,
  pipelineSteps,
  renameStep,
  template,
  validWorkspace,
  type ObjectValue,
} from "./model";

test("field edits preserve unedited YAML comments, imports and unknown extension fields", () => {
  const source =
    '# customer policy\nversion: "0.1"\nimport:\n  rules: [rules/other.yaml]\nrule:\n  id: risk\n  name: Risk\n  score: 10 # score explanation\n  when: event.amount > 10\n  metadata:\n    owner: payments\n';
  const result = patchSource(source, ["rule", "score"], 50);
  assert.match(result, /# customer policy/);
  assert.match(result, /score: 50 # score explanation/);
  assert.deepEqual(parseSource(result).data?.import, {
    rules: ["rules/other.yaml"],
  });
  assert.deepEqual((parseSource(result).data?.rule as ObjectValue).metadata, {
    owner: "payments",
  });
});

test("invalid YAML never silently replaces the editor document", () => {
  assert.ok(parseSource("rule: [").error);
  assert.ok(parseSource("rule: {id: a, id: b}").error);
  assert.ok(parseSource("[]").error);
  assert.throws(() => patchSource("rule: [", ["rule", "id"], "new"));
});

test("renaming a step updates entry, call targets and all router targets", () => {
  const pipeline: ObjectValue = {
    entry: "check",
    steps: [
      {
        step: {
          id: "check",
          name: "Check",
          type: "rule",
          rule: "risk",
          next: "router",
        },
      },
      {
        step: {
          id: "router",
          name: "Route",
          type: "router",
          default: "check",
          routes: [{ when: "true", next: "check" }],
        },
      },
    ],
  };
  const result = renameStep(pipeline, 0, "assess");
  assert.equal(result.entry, "assess");
  const steps = pipelineSteps({ pipeline: result })!;
  assert.equal(steps[1].default, "assess");
  assert.deepEqual(steps[1].routes, [{ when: "true", next: "assess" }]);
  assert.equal(pipeline.entry, "check");
});

test("malformed or duplicate flow nodes fall back to editable source/form", () => {
  assert.equal(pipelineSteps({ pipeline: { steps: null } }), null);
  assert.equal(pipelineSteps({ pipeline: { steps: [{ step: null }] } }), null);
  assert.equal(
    pipelineSteps({
      pipeline: {
        steps: [
          { step: { id: "a", type: "rule" } },
          { step: { id: "a", type: "router" } },
        ],
      },
    }),
    null,
  );
});

test("all seven templates have the expected resource identity and block rule lists", () => {
  for (const kind of [
    "rule",
    "ruleset",
    "pipeline",
    "registry",
    "features",
    "list",
    "service",
  ] as const) {
    const file = template(kind, "new_resource");
    assert.equal(parseSource(file.source).error, null);
    assert.equal(detectKind(parseSource(file.source).data), kind);
    if (kind === "ruleset") assert.match(file.source, /rules:\n\s+- blocked/);
  }
});

test("imported workspace shape and duplicate paths are checked before replacement", () => {
  assert.equal(
    validWorkspace({ files: [{ path: "a.yaml", source: "rule: {}" }] }),
    true,
  );
  assert.equal(validWorkspace({ files: [{ path: "a.yaml" }] }), false);
  assert.equal(
    validWorkspace({
      files: [
        { path: "a.yaml", source: "" },
        { path: "a.yaml", source: "" },
      ],
    }),
    false,
  );
});

test("co-located resource declarations and document streams retain every resource", () => {
  for (const separator of ["\n", "\n---\n"]) {
    const source = [
      "version: '0.1'\npipeline:\n  id: payment\n  entry: check\n  steps: []\n  decision: []\n",
      "rule:\n  id: first\n  name: First\n  when: 'true'\n  score: 10 # keep first\n",
      "rule:\n  id: second\n  when: 'true'\n  score: 20 # keep second\n",
      "ruleset:\n  id: combined\n  rules:\n    - first\n    - second\n",
    ].join(separator);
    const parsed = parseSource(source);
    assert.equal(parsed.error, null);
    assert.equal(parsed.resources.length, 4);
    assert.equal(parsed.pipelines.length, 1);
    const patched = patchSource(source, ["pipeline", "entry"], "assess");
    assert.equal(
      parseSource(patched).data?.pipeline &&
        (parseSource(patched).data!.pipeline as ObjectValue).entry,
      "assess",
    );
    assert.deepEqual(
      parseSource(patched)
        .resources.slice(1)
        .map((r) => r.data),
      parsed.resources.slice(1).map((r) => r.data),
    );
    assert.match(patched, /# keep first/);
    assert.match(patched, /# keep second/);
  }
});

test("editing the second Pipeline does not overwrite the first or its neighboring Rule", () => {
  const source =
    "# shared header\nversion: '0.1'\npipeline: {id: first, name: First, entry: end, steps: []}\n# neighbor\nrule: {id: risk, when: 'true', score: 5}\npipeline: {id: second, name: Second, entry: end, steps: []}\n";
  const parsed = parseSource(source, 1);
  assert.equal(parsed.error, null);
  assert.equal((parsed.data?.pipeline as ObjectValue).id, "second");
  const patched = patchSource(source, ["pipeline", "name"], "Edited second", 1);
  const result = parseSource(patched, 1);
  assert.equal(
    (result.pipelines[0].data.pipeline as ObjectValue).name,
    "First",
  );
  assert.equal(
    (result.pipelines[1].data.pipeline as ObjectValue).name,
    "Edited second",
  );
  assert.match(patched, /# neighbor/);
  assert.deepEqual(result.resources[1].data, parsed.resources[1].data);
});

test("resource bundles keep nested duplicate and conflicting header checks", () => {
  for (const source of [
    "version: '0.1'\nversion: '0.1'\nrule: {id: a}",
    "rule: {id: a, score: 1, score: 2}",
    "pipeline:\n  id: a\n  metadata: {owner: a, owner: b}",
    "import: {rules: [a.yaml]}\n---\nimport: {rules: [b.yaml]}\nrule: {id: a}",
    "rule: &loop {id: a, metadata: {loop: *loop}}",
  ])
    assert.ok(parseSource(source).error, source);
  const parsed = parseSource(
    "version: '0.1'\nimport: {rules: [rules/other.yaml]}\n---\nrule: {id: a}\n---\npipeline: {id: p}",
  );
  assert.equal(parsed.error, null);
  assert.deepEqual(parsed.data?.import, { rules: ["rules/other.yaml"] });
});

test("document-local versions and cross-resource anchors survive normalization", () => {
  const source =
    "version: '0.1'\nrule: {id: first, when: &condition 'event.amount > 1', score: 1}\nrule: {id: second, when: *condition, score: 2}\n---\nversion: '0.2'\nfeatures: [{name: amount, type: expression, expression: event.amount}]\n";
  const parsed = parseSource(source);
  assert.equal(parsed.error, null);
  assert.equal(parsed.resources.length, 3);
  assert.equal(
    (parsed.resources[1].data.rule as ObjectValue).when,
    "event.amount > 1",
  );
  assert.equal(parsed.resources[2].data.version, "0.2");
});
