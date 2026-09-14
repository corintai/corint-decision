import { validWorkspace, type PolicyFile } from "./model";

export interface RepositoryWorkspace {
  files: PolicyFile[];
  revision: string;
}
export interface DraftWorkspace {
  files: PolicyFile[];
  revision?: string;
}
export function parseDraft(value: unknown): DraftWorkspace | null {
  if (!validWorkspace(value)) return null;
  const revision = (value as { revision?: unknown }).revision;
  return {
    files: value.files,
    revision: typeof revision === "string" ? revision : undefined,
  };
}
export async function fetchRepository(): Promise<RepositoryWorkspace> {
  const response = await fetch("/api/repository", { cache: "no-store" });
  const workspace = await response.json();
  if (!response.ok)
    throw new Error(workspace.error || "无法读取 repository 目录。");
  const revision: unknown = workspace?.revision;
  if (!validWorkspace(workspace) || typeof revision !== "string")
    throw new Error("repository 响应缺少文件版本，请重启 Web 服务。");
  return { files: workspace.files, revision };
}
// A revision describes the disk version on which a draft was based, not the
// draft's own contents. Keep edits only while that disk version still matches.
export function syncRepository(
  disk: RepositoryWorkspace,
  draft: DraftWorkspace | null,
) {
  if (draft?.revision === disk.revision)
    return { files: draft.files, revision: disk.revision, backup: null };
  const different =
    draft && JSON.stringify(draft.files) !== JSON.stringify(disk.files);
  return {
    files: disk.files,
    revision: disk.revision,
    backup: different ? draft.files : null,
  };
}
