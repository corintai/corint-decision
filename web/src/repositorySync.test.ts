import test from "node:test";
import assert from "node:assert/strict";
import { parseDraft, syncRepository } from "./repositorySync";

const files = [{ path: "rule.yaml", source: "current" }];
const old = [{ path: "rule.yaml", source: "old" }];
test("legacy cache and changed disk versions load current files while retaining a backup", () => {
  for (const revision of [undefined, "old-version"]) {
    const result = syncRepository(
      { files, revision: "new-version" },
      { files: old, revision },
    );
    assert.deepEqual(result.files, files);
    assert.deepEqual(result.backup, old);
    assert.equal(result.revision, "new-version");
  }
});
test("unchanged disk preserves edited drafts, including imports and deletions", () => {
  const edits = [{ path: "new.yaml", source: "unsaved policy" }];
  assert.deepEqual(
    syncRepository(
      { files, revision: "same" },
      { files: edits, revision: "same" },
    ),
    { files: edits, revision: "same", backup: null },
  );
});
test("fresh and identical legacy workspaces need no backup; corrupt drafts are ignored", () => {
  for (const draft of [null, { files }])
    assert.equal(
      syncRepository({ files, revision: "new" }, draft).backup,
      null,
    );
  assert.equal(parseDraft({ files: "invalid" }), null);
  assert.deepEqual(parseDraft({ version: 1, files, revision: 12 }), {
    files,
    revision: undefined,
  });
});
