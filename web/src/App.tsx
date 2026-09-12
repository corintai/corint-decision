import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import {
  Braces,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Code2,
  Download,
  FileCode2,
  FolderOpen,
  GitBranch,
  LayoutGrid,
  LoaderCircle,
  PanelRightClose,
  Plus,
  Redo2,
  Search,
  ShieldCheck,
  Trash2,
  Undo2,
  Upload,
  X,
} from "lucide-react";
import type { Connection } from "@xyflow/react";
import { Fields, type Patch } from "./components/Fields";
import ThemeControl from "./components/ThemeControl";
import RepositoryTree from "./components/RepositoryTree";
import {
  definitions,
  detectKind,
  kindNames,
  kinds,
  object,
  parseSource,
  patchSource,
  pipelineSteps,
  renameStep,
  template,
  titles,
  validWorkspace,
  type Kind,
  type ObjectValue,
  type PolicyFile,
  type Value,
} from "./model";

// Keep earlier example-workspace drafts separate from the real repository.
const STORAGE_KEY = "corint.cdl-studio.repository.v1";
const PipelineCanvas = lazy(() => import("./components/PipelineCanvas"));
const SourceEditor = lazy(() => import("./components/SourceEditor"));
interface History {
  files: PolicyFile[];
  past: PolicyFile[][];
  future: PolicyFile[][];
}
type Action =
  | { type: "set"; files: PolicyFile[]; initial?: boolean }
  | { type: "undo" | "redo" };
function historyReducer(state: History, action: Action): History {
  if (action.type === "set")
    return {
      files: action.files,
      past: action.initial ? [] : [...state.past.slice(-59), state.files],
      future: [],
    };
  if (action.type === "undo" && state.past.length)
    return {
      files: state.past.at(-1)!,
      past: state.past.slice(0, -1),
      future: [state.files, ...state.future],
    };
  if (action.type === "redo" && state.future.length)
    return {
      files: state.future[0],
      past: [...state.past, state.files],
      future: state.future.slice(1),
    };
  return state;
}
interface Diagnostic {
  code?: string;
  message?: string;
  source?: string;
  field_path?: string;
  stage?: string;
  line?: number;
  column?: number;
}
interface Report {
  valid: boolean;
  diagnostics?: Diagnostic[];
  references_checked?: boolean;
  input_schema_checked?: boolean;
  execution_checked?: boolean;
  unchecked?: unknown;
  skipped_sources?: unknown[];
}
function download(name: string, content: string, type: string) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export default function App() {
  const [history, dispatch] = useReducer(historyReducer, {
    files: [],
    past: [],
    future: [],
  });
  const { files } = history;
  const [activePath, setActivePath] = useState("registry.yaml");
  const [pipelineIndex, setPipelineIndex] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [view, setView] = useState<"visual" | "source">("visual");
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<{
    value: Report;
    snapshot: string;
  } | null>(null);
  const [showReport, setShowReport] = useState(false);
  const [showNew, setShowNew] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [newKind, setNewKind] = useState<Kind>("rule");
  const [newId, setNewId] = useState("");
  const [newError, setNewError] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  const directoryInput = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const active = files.find((file) => file.path === activePath) || files[0];
  const parsed = useMemo(
    () => parseSource(active?.source || "", pipelineIndex),
    [active?.source, pipelineIndex],
  );
  const kind = detectKind(parsed.data);
  const data = parsed.data || {};
  const pipeline = object(data.pipeline);
  const steps = useMemo(() => pipelineSteps(parsed.data || {}), [parsed.data]);
  const selectedIndex = steps?.findIndex((step) => step.id === selected) ?? -1;
  const selectedStep = selectedIndex >= 0 ? steps![selectedIndex] : null;
  const sourceMode = kind !== "pipeline" || view === "source";
  const resource = object(
    ["pipeline", "rule", "ruleset", "features", "registry"].includes(kind)
      ? data[kind]
      : data,
  );
  const label = String(
    resource.name ||
      resource.id ||
      (kind === "registry"
        ? "策略入口注册"
        : active?.path.split("/").pop() || "CDL 工作区"),
  );
  const snapshot = useMemo(() => JSON.stringify(files), [files]);
  const stale = !!report && report.snapshot !== snapshot;
  const summaries = useMemo(
    () =>
      files.map((file) => {
        const data = parseSource(file.source).data;
        const kind = detectKind(data);
        const value = object(data?.[kind]);
        return {
          ...file,
          kind,
          label: String(value.name || value.id || file.path.split("/").pop()),
        };
      }),
    [files],
  );

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const saved = localStorage.getItem(STORAGE_KEY);
        if (saved) {
          const workspace: unknown = JSON.parse(saved);
          if (validWorkspace(workspace)) {
            if (!cancelled) {
              dispatch({ type: "set", files: workspace.files, initial: true });
              setLoading(false);
            }
            return;
          }
        }
      } catch {
        /* A corrupt/unavailable local cache must not prevent opening the editor. */
      }
      try {
        const response = await fetch("/api/repository");
        if (!response.ok) {
          const result = await response.json();
          throw new Error(result.error || "无法读取 repository 目录。");
        }
        const workspace: unknown = await response.json();
        if (!validWorkspace(workspace))
          throw new Error("repository 文件格式错误。");
        if (!cancelled)
          dispatch({ type: "set", files: workspace.files, initial: true });
      } catch (error) {
        if (!cancelled) setError((error as Error).message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);
  useEffect(() => {
    if (loading || !files.length) return;
    setSaved(false);
    const timer = setTimeout(() => {
      try {
        localStorage.setItem(
          STORAGE_KEY,
          JSON.stringify({ version: 1, files }),
        );
        setSaved(true);
      } catch {
        setError("本机草稿保存失败，请导出工作区以保留更改。");
      }
    }, 350);
    return () => clearTimeout(timer);
  }, [files, loading]);
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (!saved && files.length) e.preventDefault();
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [saved, files.length]);
  useEffect(() => {
    if (showNew) dialogRef.current?.showModal();
    else dialogRef.current?.close();
  }, [showNew]);

  function selectFile(path: string) {
    setActivePath(path);
    setPipelineIndex(0);
    setSelected(null);
  }
  function updateSource(source: string) {
    if (active)
      dispatch({
        type: "set",
        files: files.map((file) =>
          file.path === active.path ? { ...file, source } : file,
        ),
      });
  }
  const patch: Patch = (path, value) => {
    try {
      if (active)
        updateSource(patchSource(active.source, path, value, pipelineIndex));
    } catch (error) {
      setError((error as Error).message);
    }
  };
  const stepPatch: Patch = (path, value) => {
    if (
      path.at(-1) === "id" &&
      path.length === 5 &&
      selectedIndex >= 0 &&
      typeof value === "string"
    ) {
      if (
        steps?.some(
          (step, index) => index !== selectedIndex && step.id === value,
        ) ||
        ["end", "__entry"].includes(value)
      ) {
        setError("节点 ID 已存在或为保留名称。");
        return;
      }
      patch(["pipeline"], renameStep(pipeline, selectedIndex, value));
      setSelected(value);
    } else patch(path, value);
  };
  async function validate() {
    setBusy(true);
    setReport(null);
    setError("");
    setShowReport(true);
    const current = snapshot;
    try {
      const response = await fetch("/api/validate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ files }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error || "静态校验失败。");
      setReport({ value: result, snapshot: current });
    } catch (error) {
      setError((error as Error).message);
    } finally {
      setBusy(false);
    }
  }
  async function importFiles(list: FileList | null, directory = false) {
    if (!list?.length) return;
    try {
      const selectedFiles = Array.from(list).filter((file) =>
        /\.(yaml|yml|json)$/i.test(file.name),
      );
      if (!selectedFiles.length)
        throw new Error("请选择 YAML / JSON 文件或包含这些文件的文件夹。");
      if (
        selectedFiles.length > 100 ||
        selectedFiles.some((f) => f.size > 4 * 1024 * 1024) ||
        selectedFiles.reduce((n, f) => n + f.size, 0) > 10 * 1024 * 1024
      )
        throw new Error("最多导入 100 个文件，单文件 4 MiB，合计 10 MiB。");
      const imported = await Promise.all(
        selectedFiles.map(async (file) => ({
          path: directory
            ? file.webkitRelativePath.split("/").slice(1).join("/")
            : file.name,
          source: await file.text(),
        })),
      );
      if (imported.length === 1 && /\.json$/i.test(imported[0].path)) {
        const value: unknown = JSON.parse(imported[0].source);
        if (validWorkspace(value)) {
          if (
            files.length &&
            !window.confirm("用导入的工作区替换当前草稿？可以通过撤销恢复。")
          )
            return;
          dispatch({ type: "set", files: value.files });
          selectFile(value.files[0].path);
          setError("");
          return;
        }
      }
      const duplicates = imported.filter((file) =>
        files.some((existing) => existing.path === file.path),
      );
      if (
        duplicates.length &&
        !window.confirm(
          `覆盖 ${duplicates.length} 个同名文件？可以通过撤销恢复。`,
        )
      )
        return;
      const merged = [
        ...files.filter(
          (file) => !imported.some((item) => item.path === file.path),
        ),
        ...imported,
      ];
      if (!validWorkspace({ files: merged }))
        throw new Error("工作区文件过多或存在重复路径。");
      dispatch({ type: "set", files: merged });
      selectFile(imported[0].path);
      setError("");
    } catch (error) {
      setError((error as Error).message);
    } finally {
      if (fileInput.current) fileInput.current.value = "";
      if (directoryInput.current) directoryInput.current.value = "";
    }
  }
  function createResource() {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(newId) || newId === "end") {
      setNewError("请输入字母或下划线开头的有效 ID。");
      return;
    }
    const created = template(newKind, newId);
    if (files.some((file) => file.path === created.path)) {
      setNewError("文件已存在，请使用其他 ID。");
      return;
    }
    if (files.length >= 100) {
      setNewError("一个工作区最多包含 100 个文件。");
      return;
    }
    dispatch({ type: "set", files: [...files, created] });
    selectFile(created.path);
    setShowNew(false);
    setView("visual");
  }
  function addStep(type: string) {
    if (!steps) return;
    let index = 1;
    while (steps.some((step) => step.id === `${type}_${index}`)) index++;
    const id = `${type}_${index}`;
    const step: ObjectValue = {
      id,
      name: `新${type === "router" ? "条件分支" : "执行节点"}`,
      type,
    };
    if (type === "router") {
      step.routes = [{ when: "total_score >= 100", next: "end" }];
      step.default = "end";
    } else {
      step.next = "end";
      step[type] =
        type === "rule"
          ? "blocked"
          : type === "ruleset"
            ? "payment"
            : type === "service"
              ? "customer_risk"
              : "";
      if (type === "service") step.operation = "assess";
    }
    const next = structuredClone(pipeline);
    const wrappers = next.steps as ObjectValue[];
    // Insert after a selected call, otherwise append to an existing terminal call.
    const predecessor =
      selectedStep && selectedStep.type !== "router"
        ? object(wrappers[selectedIndex].step)
        : [...wrappers]
            .reverse()
            .map((w) => object(w.step))
            .find((s) => s.next === "end");
    if (predecessor) {
      if (type !== "router") step.next = predecessor.next;
      else {
        step.default = predecessor.next;
        object((step.routes as Value[])[0]).next = predecessor.next;
      }
      predecessor.next = id;
    } else if (!steps.length) next.entry = id;
    wrappers.push({ step });
    patch(["pipeline"], next);
    setSelected(id);
  }
  function connect(connection: Connection) {
    if (connection.source === "__entry") {
      patch(["pipeline", "entry"], connection.target);
      return;
    }
    const index =
      steps?.findIndex((step) => step.id === connection.source) ?? -1;
    if (index < 0) return;
    const path: (string | number)[] = ["pipeline", "steps", index, "step"];
    if (steps![index].type === "router")
      path.push(
        ...(connection.sourceHandle === "default"
          ? ["default"]
          : ["routes", Number(connection.sourceHandle), "next"]),
      );
    else path.push("next");
    patch(path, connection.target);
  }
  function deleteStep() {
    if (!selectedStep || !steps) return;
    if (
      !window.confirm(
        `删除节点「${selectedStep.name}」？指向它的连线将接到其下一节点或默认分支。`,
      )
    )
      return;
    const next = renameStep(
      pipeline,
      selectedIndex,
      String(selectedStep.next || selectedStep.default || "end"),
    );
    (next.steps as Value[]).splice(selectedIndex, 1);
    patch(["pipeline"], next);
    setSelected(null);
  }
  return (
    <div className="studio">
      <aside className="sidebar">
        <a className="brand" href="/">
          <img className="brand-mark" src="/corint-icon.png" alt="" />
          <span>
            Corint<span className="brand-subtitle">CDL Studio</span>
          </span>
        </a>
        <div className="workspace-card">
          <span className="workspace-icon">
            <FolderOpen size={18} />
          </span>
          <div>
            <strong>CDL 工作区</strong>
            <small>本地策略编辑</small>
          </div>
          <span className="local-badge">LOCAL</span>
        </div>
        <div className="sidebar-heading">
          <span>资源管理</span>
          <button
            className="icon-button"
            aria-label="新建资源"
            onClick={() => {
              setNewId("");
              setNewError("");
              setShowNew(true);
            }}
          >
            <Plus size={17} />
          </button>
        </div>
        <div className="search">
          <Search size={15} />
          <input
            aria-label="搜索资源"
            placeholder="搜索文件、目录或名称…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <RepositoryTree
          files={summaries}
          activePath={active?.path}
          search={search}
          onSelect={selectFile}
        />
        <ThemeControl />
        <div className="sidebar-bottom">
          <div>
            <span className="status-dot" />
            {files.length} 个资源文件<span>CDL</span>
          </div>
          <small>从规则，到每一次可信决策。</small>
        </div>
      </aside>
      <main className="main">
        <header className="topbar">
          <div className="breadcrumb">
            工作区
            <ChevronRight size={14} />
            <span>{kindNames[kind]}</span>
            <ChevronRight size={14} />
            <strong>{active?.path.split("/").pop() || "欢迎"}</strong>
          </div>
          <div className="header-actions">
            <span className="save-status">
              {saved ? <Check size={13} /> : <Circle size={11} />}
              {saved ? "草稿已保存至本机" : "本机草稿"}
            </span>
            <button
              className="button"
              onClick={() => fileInput.current?.click()}
            >
              <Upload size={15} />
              导入
            </button>
            <div className="export-wrap">
              <button
                className="button"
                disabled={!active}
                aria-expanded={showExport}
                onClick={() => setShowExport(!showExport)}
              >
                <Download size={15} />
                导出
                <ChevronDown size={12} />
              </button>
              {showExport && (
                <div className="export-menu">
                  <button
                    onClick={() => {
                      download(
                        active.path.split("/").pop()!,
                        active.source,
                        "text/yaml;charset=utf-8",
                      );
                      setShowExport(false);
                    }}
                  >
                    当前 YAML 文件
                  </button>
                  <button
                    onClick={() => {
                      download(
                        "workspace.cdl.json",
                        JSON.stringify({ version: 1, files }, null, 2),
                        "application/json",
                      );
                      setShowExport(false);
                    }}
                  >
                    完整工作区（可重新导入）
                  </button>
                </div>
              )}
            </div>
            <button
              className="button primary"
              disabled={busy || !files.length}
              onClick={() => void validate()}
            >
              {busy ? (
                <LoaderCircle size={15} className="spin" />
              ) : (
                <ShieldCheck size={15} />
              )}
              {busy ? "校验中…" : "校验 CDL"}
            </button>
          </div>
        </header>
        {error && (
          <div className="app-error" role="alert">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="关闭错误提示"
              onClick={() => setError("")}
            >
              <X size={16} />
            </button>
          </div>
        )}
        <section className="page-heading">
          <div>
            <div className="title-line">
              <h1>{label}</h1>
              <span className="draft-badge">草稿</span>
            </div>
            <p>
              {kind === "pipeline"
                ? "编排决策流程，让每一步逻辑清晰可见。"
                : `使用 YAML 源码编辑${titles[kind]}。`}
            </p>
          </div>
          <button
            className="text-button"
            onClick={() => {
              directoryInput.current?.setAttribute("webkitdirectory", "");
              directoryInput.current?.click();
            }}
          >
            <FolderOpen size={15} />
            导入文件夹
          </button>
        </section>
        <div className="editor-toolbar">
          <div className="view-tabs">
            {kind === "pipeline" && (
              <button
                className={view === "visual" ? "selected" : ""}
                onClick={() => setView("visual")}
              >
                <LayoutGrid size={15} />
                可视化编辑
              </button>
            )}
            <button
              className={sourceMode ? "selected" : ""}
              onClick={() => setView("source")}
            >
              <Code2 size={16} />
              YAML 源码
            </button>
          </div>
          {parsed.pipelines.length > 1 && (
            <select
              className="pipeline-selector"
              aria-label="当前 Pipeline"
              value={Math.min(pipelineIndex, parsed.pipelines.length - 1)}
              onChange={(e) => {
                setPipelineIndex(Number(e.target.value));
                setSelected(null);
              }}
            >
              {parsed.pipelines.map((resource, index) => (
                <option key={index} value={index}>
                  {String(
                    object(resource.data.pipeline).name ||
                      object(resource.data.pipeline).id ||
                      `Pipeline ${index + 1}`,
                  )}
                </option>
              ))}
            </select>
          )}
          <div className="canvas-tools">
            <button
              className="icon-button"
              aria-label="撤销"
              disabled={!history.past.length}
              onClick={() => {
                dispatch({ type: "undo" });
                setSelected(null);
              }}
            >
              <Undo2 size={16} />
            </button>
            <button
              className="icon-button"
              aria-label="重做"
              disabled={!history.future.length}
              onClick={() => {
                dispatch({ type: "redo" });
                setSelected(null);
              }}
            >
              <Redo2 size={16} />
            </button>
            <span className="toolbar-divider" />
            {kind === "pipeline" && view === "visual" && steps && (
              <select
                aria-label="添加流程节点"
                value=""
                onChange={(e) => {
                  if (e.target.value) addStep(e.target.value);
                }}
              >
                <option value="">＋ 添加节点</option>
                <option value="rule">Rule · 规则</option>
                <option value="ruleset">Ruleset · 规则集</option>
                <option value="router">Router · 条件分支</option>
                <option value="pipeline">Pipeline · 子流程</option>
                <option value="service">Service · 服务</option>
              </select>
            )}
            <button
              className={`text-button validation-toggle ${report && !stale && report.value.valid ? "success-text" : ""}`}
              onClick={() => setShowReport(!showReport)}
            >
              {report && !stale && report.value.valid ? (
                <CheckCircle2 size={15} />
              ) : (
                <Circle size={13} />
              )}
              {stale
                ? "校验结果已过期"
                : report
                  ? report.value.valid
                    ? "静态校验通过"
                    : "存在校验问题"
                  : "尚未校验"}
            </button>
          </div>
        </div>
        <div className="editor-layout">
          <section className={`editor-main ${sourceMode ? "source-mode" : ""}`}>
            {loading ? (
              <div className="empty-state">
                <LoaderCircle className="spin" />
                <h2>正在打开工作区…</h2>
              </div>
            ) : !active ? (
              <div className="empty-state">
                <Braces size={40} />
                <h2>创建你的第一份 CDL</h2>
                <button
                  className="button primary"
                  onClick={() => setShowNew(true)}
                >
                  新建资源
                </button>
              </div>
            ) : sourceMode ? (
              <Suspense
                fallback={
                  <div className="empty-state">正在加载源码编辑器…</div>
                }
              >
                <SourceEditor
                  key={active.path}
                  source={active.source}
                  onChange={updateSource}
                  error={parsed.error}
                />
              </Suspense>
            ) : parsed.error ? (
              <div className="empty-state">
                <Code2 size={36} />
                <h2>先修复 YAML 语法</h2>
                <p>{parsed.error}</p>
                <button className="button" onClick={() => setView("source")}>
                  打开源码编辑器
                </button>
              </div>
            ) : kind === "pipeline" && steps ? (
              <>
                <div className="canvas-caption">
                  <GitBranch size={14} />
                  <span>{steps.length} 个节点</span>
                  <span>实线：主流程 / 条件 · 虚线：默认分支</span>
                </div>
                <Suspense
                  fallback={<div className="empty-state">正在加载流程图…</div>}
                >
                  <PipelineCanvas
                    key={`${active.path}/${pipelineIndex}`}
                    pipeline={pipeline}
                    steps={steps}
                    selected={selected}
                    onSelect={setSelected}
                    onConnect={connect}
                  />
                </Suspense>
              </>
            ) : (
              <div className="empty-state">
                <GitBranch size={36} />
                <h2>暂时无法绘制流程图</h2>
                <p>请在 YAML 中检查 steps 结构、节点 ID 和类型。</p>
                <button className="button" onClick={() => setView("source")}>
                  编辑 YAML
                </button>
              </div>
            )}
          </section>
          <aside className="inspector">
            <div className="inspector-heading">
              <span>
                {selectedStep
                  ? "节点属性"
                  : selected === "end"
                    ? "最终决策"
                    : "资源属性"}
              </span>
              {selected && (
                <button
                  className="icon-button"
                  aria-label="取消节点选择"
                  onClick={() => setSelected(null)}
                >
                  <PanelRightClose size={16} />
                </button>
              )}
            </div>
            <div className="inspector-body">
              {active && parsed.data && kind === "pipeline" ? (
                <>
                  {selectedStep ? (
                    <>
                      <div className="inspector-kind">
                        <span className="selection-dot" />
                        {selectedStep.type.toUpperCase()}
                        <code>{selectedStep.id}</code>
                      </div>
                      <Fields
                        key={`${active.path}/${selectedIndex}`}
                        value={selectedStep}
                        schema={{ $ref: "#/definitions/step" }}
                        path={["pipeline", "steps", selectedIndex, "step"]}
                        onPatch={stepPatch}
                      />
                      <button
                        className="button danger delete-node"
                        onClick={deleteStep}
                      >
                        <Trash2 size={14} />
                        删除节点
                      </button>
                    </>
                  ) : selected === "end" ? (
                    <Fields
                      value={pipeline.decision}
                      schema={definitions.pipeline.properties!.decision}
                      path={["pipeline", "decision"]}
                      onPatch={patch}
                    />
                  ) : (
                    <>
                      <div className="inspector-kind">
                        <span className="selection-dot" />
                        PIPELINE
                      </div>
                      <Fields
                        value={Object.fromEntries(
                          Object.entries(pipeline).filter(
                            ([key]) => key !== "steps" && key !== "decision",
                          ),
                        )}
                        schema={{
                          ...definitions.pipeline,
                          properties: Object.fromEntries(
                            Object.entries(
                              definitions.pipeline.properties!,
                            ).filter(
                              ([key]) => key !== "steps" && key !== "decision",
                            ),
                          ),
                          required: ["id", "name", "entry"],
                        }}
                        path={["pipeline"]}
                        onPatch={patch}
                      />
                      <button
                        className="decision-link"
                        onClick={() => setSelected("end")}
                      >
                        <ShieldCheck size={17} />
                        <span>
                          配置最终决策
                          <small>
                            {Array.isArray(pipeline.decision)
                              ? pipeline.decision.length
                              : 0}{" "}
                            条决策分支
                          </small>
                        </span>
                        <ChevronRight size={16} />
                      </button>
                    </>
                  )}
                </>
              ) : (
                <div className="resource-info">
                  <span className="info-icon">
                    <FileCode2 size={26} />
                  </span>
                  <h3>{kindNames[kind]} 资源</h3>
                  <p>直接编辑 YAML 源码，更改自动保存为本机草稿。</p>
                  <dl>
                    <dt>文件路径</dt>
                    <dd>{active?.path || "—"}</dd>
                    <dt>保存位置</dt>
                    <dd>当前浏览器 · 本机草稿</dd>
                    <dt>静态校验</dt>
                    <dd>整个工作区及资源引用</dd>
                  </dl>
                </div>
              )}
              {active && (
                <div className="file-operations">
                  <label htmlFor="file-path">文件路径</label>
                  <input
                    id="file-path"
                    key={active.path}
                    defaultValue={active.path}
                    onBlur={(e) => {
                      const nextPath = e.target.value.trim();
                      if (nextPath === active.path) return;
                      if (
                        !/^[\p{L}\p{N}_ .\-/]+\.(yaml|yml|json)$/iu.test(
                          nextPath,
                        ) ||
                        nextPath.startsWith("/") ||
                        nextPath
                          .split("/")
                          .some((p) => !p || p === "." || p === "..") ||
                        files.some((f) => f.path === nextPath)
                      ) {
                        setError("文件路径无效或已存在。");
                        e.target.value = active.path;
                        return;
                      }
                      dispatch({
                        type: "set",
                        files: files.map((f) =>
                          f.path === active.path ? { ...f, path: nextPath } : f,
                        ),
                      });
                      selectFile(nextPath);
                    }}
                  />
                  <button
                    className="text-button danger"
                    disabled={files.length < 2}
                    onClick={() => {
                      if (
                        window.confirm(
                          `从工作区移除 ${active.path}？可通过撤销恢复。`,
                        )
                      ) {
                        dispatch({
                          type: "set",
                          files: files.filter((f) => f.path !== active.path),
                        });
                        setSelected(null);
                      }
                    }}
                  >
                    <Trash2 size={13} />
                    移除文件
                  </button>
                </div>
              )}
            </div>
            <div className="inspector-note">
              <ShieldCheck size={17} />
              <p>
                校验检查语法与资源引用。
                <br />
                不会执行策略或调用外部服务。
              </p>
            </div>
          </aside>
        </div>
        {showReport && (
          <section
            className={`validation-panel ${report?.value.valid && !stale ? "valid" : ""}`}
            aria-label="校验结果"
          >
            <div className="validation-heading">
              <strong>
                <ShieldCheck size={16} />
                静态校验
              </strong>
              <code>cdl-static-1</code>
              <span>
                {busy
                  ? "正在调用 Corint CLI…"
                  : stale
                    ? "文件已修改，请重新校验"
                    : report?.value.valid
                      ? "通过"
                      : report
                        ? "未通过"
                        : "点击「校验 CDL」开始"}
              </span>
              <button
                className="icon-button"
                aria-label="收起校验结果"
                onClick={() => setShowReport(false)}
              >
                <X size={15} />
              </button>
            </div>
            {report && (
              <div className="validation-content">
                <div className="validation-scope">
                  <span>
                    引用检查：
                    {report.value.references_checked ? "已完成" : "未完成"}
                  </span>
                  <span>
                    输入字段类型：
                    {report.value.input_schema_checked ? "已检查" : "未检查"}
                  </span>
                  <span>
                    策略执行：
                    {report.value.execution_checked ? "已检查" : "未执行"}
                  </span>
                  <span>
                    跳过辅助文件：{report.value.skipped_sources?.length || 0}
                  </span>
                </div>
                {report.value.diagnostics?.map((diagnostic, index) => (
                  <button
                    className="diagnostic"
                    key={index}
                    onClick={() => {
                      const match = files.find(
                        (file) =>
                          diagnostic.source?.replace(/^\.\//, "") === file.path,
                      );
                      if (match) {
                        selectFile(match.path);
                        setView("source");
                      }
                    }}
                  >
                    <code>{diagnostic.code}</code>
                    <span>
                      {diagnostic.message}
                      <small>
                        {diagnostic.source} {diagnostic.field_path}
                        {diagnostic.line
                          ? ` · ${diagnostic.line}:${diagnostic.column || 1}`
                          : ""}
                      </small>
                    </span>
                  </button>
                ))}
                <details>
                  <summary>查看完整校验报告</summary>
                  <pre>{JSON.stringify(report.value, null, 2)}</pre>
                </details>
              </div>
            )}
          </section>
        )}
        <footer className="statusbar">
          <span>
            <span className="status-dot" />
            CDL Studio{" "}
            <span className="statusbar-muted">/ 可视化策略工作台</span>
          </span>
          <span>
            {active?.source.split("\n").length || 0} 行{" "}
            <span className="statusbar-muted">UTF-8</span>
            <span className="statusbar-muted">v0.1.0</span>
          </span>
        </footer>
      </main>
      <input
        type="file"
        ref={fileInput}
        accept=".yaml,.yml,.json"
        multiple
        hidden
        onChange={(e) => void importFiles(e.target.files)}
      />
      <input
        type="file"
        ref={directoryInput}
        multiple
        hidden
        onChange={(e) => void importFiles(e.target.files, true)}
      />
      <dialog
        ref={dialogRef}
        className="new-dialog"
        onCancel={() => setShowNew(false)}
        onClose={() => setShowNew(false)}
      >
        <form
          onSubmit={(e) => {
            e.preventDefault();
            createResource();
          }}
        >
          <div className="dialog-heading">
            <div>
              <span className="eyebrow">CREATE RESOURCE</span>
              <h2>新建 CDL 资源</h2>
            </div>
            <button
              type="button"
              className="icon-button"
              aria-label="关闭新建窗口"
              onClick={() => setShowNew(false)}
            >
              <X size={19} />
            </button>
          </div>
          <label htmlFor="resource-kind">资源类型</label>
          <select
            id="resource-kind"
            value={newKind}
            onChange={(e) => setNewKind(e.target.value as Kind)}
          >
            {kinds.map((kind) => (
              <option key={kind} value={kind}>
                {kindNames[kind]} · {titles[kind]}
              </option>
            ))}
          </select>
          <label htmlFor="resource-id">资源 ID / 文件名</label>
          <input
            id="resource-id"
            autoFocus
            value={newId}
            placeholder="例如 payment_risk"
            onChange={(e) => {
              setNewId(e.target.value);
              setNewError("");
            }}
          />
          <p>创建可编辑模板后，请调整业务条件和资源引用，再校验工作区。</p>
          {newError && (
            <p className="error-text" role="alert">
              {newError}
            </p>
          )}
          <div className="dialog-actions">
            <button
              type="button"
              className="button"
              onClick={() => setShowNew(false)}
            >
              取消
            </button>
            <button className="button primary" type="submit">
              <Plus size={15} />
              创建资源
            </button>
          </div>
        </form>
      </dialog>
    </div>
  );
}
