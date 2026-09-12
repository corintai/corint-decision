import { useEffect, useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  FileCode2,
  Folder,
  FolderOpen,
} from "lucide-react";

interface Resource {
  path: string;
  label: string;
}
interface Directory {
  name: string;
  path: string;
  directories: Map<string, Directory>;
  files: Resource[];
  count: number;
}

export default function RepositoryTree({
  files,
  activePath,
  search,
  onSelect,
}: {
  files: Resource[];
  activePath?: string;
  search: string;
  onSelect: (path: string) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const query = search.trim().toLowerCase();
  const root = useMemo(() => {
    const root: Directory = {
      name: "repository",
      path: "",
      directories: new Map(),
      files: [],
      count: 0,
    };
    for (const file of files) {
      if (query && !`${file.path} ${file.label}`.toLowerCase().includes(query))
        continue;
      let directory = root;
      directory.count++;
      const segments = file.path.split("/");
      for (const name of segments.slice(0, -1)) {
        const path = directory.path ? `${directory.path}/${name}` : name;
        let child = directory.directories.get(name);
        if (!child) {
          child = { name, path, directories: new Map(), files: [], count: 0 };
          directory.directories.set(name, child);
        }
        directory = child;
        directory.count++;
      }
      directory.files.push(file);
    }
    return root;
  }, [files, query]);

  // Reveal files selected by import, creation, or diagnostics in their real folder.
  useEffect(() => {
    if (!activePath) return;
    const segments = activePath.split("/");
    setCollapsed((previous) => {
      const next = new Set(previous);
      next.delete("");
      for (let i = 1; i < segments.length; i++)
        next.delete(segments.slice(0, i).join("/"));
      return next;
    });
  }, [activePath]);

  useEffect(() => {
    if (query) setCollapsed(new Set());
  }, [query]);

  function renderDirectory(directory: Directory, depth: number) {
    const open = !collapsed.has(directory.path);
    return (
      <li key={directory.path}>
        <button
          className="directory-item"
          title={directory.path || "repository"}
          aria-expanded={open}
          aria-label={`${open ? "折叠" : "展开"} ${directory.path || "repository"}`}
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={() =>
            setCollapsed((previous) => {
              const next = new Set(previous);
              if (next.has(directory.path)) next.delete(directory.path);
              else next.add(directory.path);
              return next;
            })
          }
        >
          {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
          {open ? <FolderOpen size={15} /> : <Folder size={15} />}
          <span className="directory-name">{directory.name}</span>
          <span className="count">{directory.count}</span>
        </button>
        {open && (
          <ul className="repository-children">
            {[...directory.directories.values()]
              .sort((a, b) => a.name.localeCompare(b.name))
              .map((child) => renderDirectory(child, depth + 1))}
            {[...directory.files]
              .sort((a, b) => a.path.localeCompare(b.path))
              .map((file) => (
                <li key={file.path}>
                  <button
                    title={file.path}
                    aria-current={activePath === file.path ? "page" : undefined}
                    className={`resource-item ${activePath === file.path ? "active" : ""}`}
                    style={{ paddingLeft: 8 + (depth + 1) * 14 + 19 }}
                    onClick={() => onSelect(file.path)}
                  >
                    <FileCode2 size={15} />
                    <span>{file.path.split("/").pop()}</span>
                  </button>
                </li>
              ))}
          </ul>
        )}
      </li>
    );
  }

  return (
    <nav className="resource-tree" aria-label="repository 目录">
      <ul className="repository-children">{renderDirectory(root, 0)}</ul>
      {query && root.count === 0 && (
        <p className="empty-search">没有匹配的资源</p>
      )}
    </nav>
  );
}
