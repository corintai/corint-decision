import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, symlink, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { readRepository } from "./repository.mjs";

test("repository reads nested source files verbatim without following symlinks", async () => {
  const temporary = await mkdtemp(path.join(tmpdir(), "cdl-repository-test-"));
  const root = path.join(temporary, "repository");
  try {
    await mkdir(path.join(root, "rules", "payment"), { recursive: true });
    const source = "# Keep this comment\nrule:\n  id: payment\n";
    await writeFile(path.join(root, "rules/payment/check.yaml"), source);
    await writeFile(path.join(root, "registry.yaml"), "registry: []\n");
    await writeFile(path.join(root, "README.md"), "Documentation");
    await writeFile(path.join(temporary, "outside.yaml"), "outside: secret");
    await symlink(
      path.join(temporary, "outside.yaml"),
      path.join(root, "linked.yaml"),
    );
    await symlink(temporary, path.join(root, "linked-directory"));
    const { files } = await readRepository(root);
    assert.deepEqual(files, [
      { path: "registry.yaml", source: "registry: []\n" },
      { path: "rules/payment/check.yaml", source },
    ]);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});

test("repository reports an empty directory instead of loading demo content", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "cdl-repository-empty-"));
  try {
    await assert.rejects(readRepository(root), /没有 YAML 或 JSON/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
