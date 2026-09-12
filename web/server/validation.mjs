import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, mkdir, writeFile, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

const execute = promisify(execFile);
export function checkFiles(files) {
  if (!Array.isArray(files) || files.length < 1 || files.length > 100) {
    throw new Error("请选择 1–100 个 CDL 文件。");
  }
  const names = new Set();
  let size = 0;
  for (const file of files) {
    if (
      !file ||
      typeof file.path !== "string" ||
      typeof file.source !== "string" ||
      !/^[\p{L}\p{N}_ .\-/]+\.(yaml|yml|json)$/iu.test(file.path) ||
      file.path.startsWith("/") ||
      file.path.split("/").some((p) => !p || p === "." || p === "..") ||
      names.has(file.path.toLowerCase())
    ) {
      throw new Error("文件名无效、路径越界或文件路径重复。");
    }
    names.add(file.path.toLowerCase());
    const bytes = Buffer.byteLength(file.source);
    size += bytes;
    if (bytes > 4 * 1024 * 1024 || size > 10 * 1024 * 1024)
      throw new Error("文件过大（单文件最多 4 MiB，合计 10 MiB）。");
  }
}

export async function validateFiles(files, binary) {
  checkFiles(files);
  const directory = await realpath(
    await mkdtemp(path.join(tmpdir(), "corint-studio-")),
  );
  try {
    await Promise.all(
      files.map(async (file) => {
        const target = path.join(directory, file.path);
        await mkdir(path.dirname(target), { recursive: true });
        await writeFile(target, file.source, "utf8");
      }),
    );
    let stdout,
      exitCode = 0;
    try {
      ({ stdout } = await execute(
        binary,
        [
          "validate",
          "--profile",
          "cdl-static-1",
          "--format",
          "json",
          "--root",
          directory,
          ".",
        ],
        { timeout: 30_000, maxBuffer: 8 * 1024 * 1024 },
      ));
    } catch (error) {
      if (error.code === "ENOENT")
        throw new Error(
          "未找到 Corint CLI。请先在仓库根目录运行 cargo build --locked -p corint-decision-cli，或设置 CORINT_CLI。",
        );
      if (![1, 2].includes(error.code) || !error.stdout)
        throw new Error("CLI 未完成校验，请检查 CLI 配置或稍后重试。");
      stdout = error.stdout;
      exitCode = error.code;
    }
    const report = JSON.parse(
      stdout.replaceAll(directory + "/", "").replaceAll(directory, "."),
    );
    return {
      ...report,
      valid: exitCode === 0 && report.valid === true,
      exit_code: exitCode,
    };
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}
