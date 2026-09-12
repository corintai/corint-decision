import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { checkFiles, validateFiles } from "./validation.mjs";
const binary =
  process.env.CORINT_CLI ||
  fileURLToPath(new URL("../../target/debug/corint", import.meta.url));
const examples = [
  "registry.yaml",
  "rules/blocked.yaml",
  "rulesets/payment.yaml",
  "pipelines/payment.yaml",
  "features/payment.yaml",
  "lists/blocked.yaml",
  "services/risk.yaml",
];
const files = await Promise.all(
  examples.map(async (path) => ({
    path,
    source: await readFile(
      new URL(`../../tests/conformance/cdl_authoring/${path}`, import.meta.url),
      "utf8",
    ),
  })),
);

test("validation rejects traversal, duplicate paths, oversized input and malformed requests", () => {
  for (const path of [
    "../outside.yaml",
    "/tmp/out.yaml",
    "a/../../out.yaml",
    "a\\b.yaml",
    "a//b.yaml",
    "./a.yaml",
  ])
    assert.throws(() => checkFiles([{ path, source: "" }]));
  assert.throws(() => checkFiles([]));
  assert.throws(() =>
    checkFiles([
      { path: "a.yaml", source: "" },
      { path: "A.yaml", source: "" },
    ]),
  );
  assert.throws(() =>
    checkFiles([{ path: "a.yaml", source: "x".repeat(4 * 1024 * 1024 + 1) }]),
  );
  assert.throws(() => checkFiles([{ path: "a.yaml", source: 5 }]));
  assert.doesNotThrow(() =>
    checkFiles([{ path: "规则/payment.yaml", source: "" }]),
  );
});
test("real CLI checks all seven resources and references without executing", async () => {
  const report = await validateFiles(files, binary);
  assert.equal(report.valid, true);
  assert.equal(report.exit_code, 0);
  assert.equal(report.references_checked, true);
  assert.equal(report.execution_checked, false);
  assert.equal(report.input_schema_checked, false);
  assert.equal(report.sources.length, 7);
  assert.deepEqual([...report.sources].sort(), [...examples].sort());
  assert.ok(
    report.sources.every((source) => !source.includes("corint-studio-")),
  );
});
test("missing dependency yields actionable CLI diagnostics", async () => {
  const report = await validateFiles(
    files.filter((file) => file.path !== "rules/blocked.yaml"),
    binary,
  );
  assert.equal(report.valid, false);
  assert.equal(report.exit_code, 1);
  assert.ok(
    report.diagnostics.some((d) => d.code === "E_UNRESOLVED_REFERENCE"),
  );
});
test("syntax errors retain CLI source locations", async () => {
  const report = await validateFiles(
    [{ path: "rules/broken.yaml", source: "rule: [" }],
    binary,
  );
  assert.equal(report.valid, false);
  assert.ok(report.diagnostics.length > 0);
  assert.ok(report.diagnostics.some((d) => d.source?.includes("broken.yaml")));
  assert.ok(report.diagnostics.some((d) => d.source === "rules/broken.yaml"));
});
test("missing CLI produces setup instructions instead of a successful report", async () => {
  await assert.rejects(
    validateFiles(files, "/nonexistent/corint-studio-cli"),
    /cargo build/,
  );
});
