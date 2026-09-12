import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { checkFiles } from "./validation.mjs";

// The root is fixed by the server, never supplied by the browser. Do not follow
// symbolic links into unrelated parts of the user's filesystem.
export async function readRepository(root) {
  const files = [];
  let bytes = 0;
  async function visit(directory, prefix = "") {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((a, b) => a.name.localeCompare(b.name));
    for (const entry of entries) {
      if (entry.name.startsWith(".") || entry.isSymbolicLink()) continue;
      const absolute = path.join(directory, entry.name);
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) await visit(absolute, relative);
      else if (entry.isFile() && /\.(yaml|yml|json)$/i.test(entry.name)) {
        const info = await stat(absolute);
        bytes += info.size;
        if (
          files.length >= 100 ||
          info.size > 4 * 1024 * 1024 ||
          bytes > 10 * 1024 * 1024
        )
          throw new Error(
            "repository 文件超过工作区限制（100 个文件，单文件 4 MiB，合计 10 MiB）。",
          );
        files.push({
          path: relative,
          source: await readFile(absolute, "utf8"),
        });
      }
    }
  }
  await visit(root);
  if (!files.length) throw new Error("repository 中没有 YAML 或 JSON 文件。");
  checkFiles(files);
  return { files };
}
