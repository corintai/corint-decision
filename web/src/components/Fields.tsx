import { useId, useState } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import {
  defaultValue,
  definitions,
  object,
  resolveSchema,
  type Path,
  type Schema,
  type Value,
} from "../model";

export type Patch = (path: Path, value: Value | undefined) => void;
interface Props {
  value: Value | undefined;
  schema: Schema;
  path: Path;
  onPatch: Patch;
  label?: string;
  depth?: number;
}
const labels: Record<string, string> = {
  id: "标识 ID",
  name: "名称",
  description: "描述",
  score: "命中分数",
  when: "匹配条件",
  rules: "规则引用",
  conclusion: "结论 · 按顺序匹配",
  signal: "信号",
  result: "决策结果",
  reason: "原因说明",
  actions: "动作意图",
  entry: "入口节点",
  next: "下一节点",
  default: "默认分支",
  routes: "条件路由 · 按顺序匹配",
  decision: "最终决策",
  type: "类型",
  rule: "调用规则",
  ruleset: "调用规则集",
  pipeline: "调用流程",
  service: "调用服务",
  operation: "操作",
  expression: "计算表达式",
  datasource: "数据源",
  dimension: "维度",
  window: "时间窗口",
  initial_values: "初始名单",
  backend: "存储方式",
  base_url: "服务地址",
  operations: "服务操作",
};

function RawValue({
  value,
  path,
  onPatch,
}: Pick<Props, "value" | "path" | "onPatch">) {
  const [draft, setDraft] = useState(JSON.stringify(value ?? null, null, 2));
  const [error, setError] = useState("");
  return (
    <div className="raw-value">
      <textarea
        aria-label={`${path.join(".")} JSON`}
        value={draft}
        rows={5}
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
      />
      <button
        className="small-button"
        onClick={() => {
          try {
            onPatch(path, JSON.parse(draft));
            setError("");
          } catch {
            setError("请输入有效 JSON。");
          }
        }}
      >
        应用 JSON
      </button>
      {error && <small className="error-text">{error}</small>}
    </div>
  );
}

function Condition({
  value,
  path,
  onPatch,
  depth = 0,
}: Pick<Props, "value" | "path" | "onPatch" | "depth">) {
  if (depth > 12)
    return <RawValue value={value} path={path} onPatch={onPatch} />;
  const record = object(value);
  const keys = Object.keys(record);
  const mode =
    typeof value === "string"
      ? "expression"
      : keys.length === 1 &&
          ["all", "any", "not"].includes(keys[0]) &&
          Array.isArray(record[keys[0]])
        ? keys[0]
        : "advanced";
  return (
    <div className="condition-editor">
      <select
        aria-label={`${path.join(".")} 条件模式`}
        value={mode}
        onChange={(e) => {
          const next = e.target.value;
          if (next === "expression") {
            const parts = Array.isArray(record[mode])
              ? (record[mode] as Value[])
              : [];
            const expression =
              parts.length === 1 && typeof parts[0] === "string"
                ? parts[0]
                : "true";
            if (
              parts.length > 1 &&
              !window.confirm("转换为单个表达式将替换当前条件组，是否继续？")
            )
              return;
            onPatch(path, expression);
          } else onPatch(path, { [next]: [value ?? "true"] });
        }}
      >
        <option value="expression">表达式</option>
        <option value="all">全部满足 · AND</option>
        <option value="any">任一满足 · OR</option>
        <option value="not">取反 · NOT</option>
        {mode === "advanced" && <option value="advanced">完整条件对象</option>}
      </select>
      {mode === "expression" ? (
        <textarea
          aria-label={`${path.join(".")} 表达式`}
          spellCheck={false}
          rows={2}
          value={String(value ?? "")}
          onChange={(e) => onPatch(path, e.target.value)}
        />
      ) : mode === "advanced" ? (
        <RawValue
          key={JSON.stringify(value)}
          value={value}
          path={path}
          onPatch={onPatch}
        />
      ) : (
        <div className="condition-group">
          {(record[mode] as Value[]).map((item, index, items) => (
            <div className="condition-row" key={index}>
              <Condition
                depth={depth + 1}
                value={item}
                path={[...path, mode, index]}
                onPatch={onPatch}
              />
              {items.length > 1 && (
                <button
                  className="icon-button danger"
                  aria-label={`删除条件 ${index + 1}`}
                  onClick={() =>
                    onPatch(
                      [...path, mode],
                      items.filter((_, i) => i !== index),
                    )
                  }
                >
                  <Trash2 size={14} />
                </button>
              )}
            </div>
          ))}
          {mode !== "not" && (
            <button
              className="text-button"
              onClick={() =>
                onPatch([...path, mode], [...(record[mode] as Value[]), "true"])
              }
            >
              <Plus size={14} />
              添加条件
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export function Fields({
  value,
  schema: inputSchema,
  path,
  onPatch,
  label,
  depth = 0,
}: Props) {
  const id = useId();
  const schema = resolveSchema(inputSchema, value);
  const field = String(path.at(-1) || label || "");
  const record = object(value);
  const isCondition =
    inputSchema.$ref?.match(/\/(when|condition)$/) ||
    (field === "when" && typeof value !== "boolean");
  let content;
  if (depth > 14)
    content = <RawValue value={value} path={path} onPatch={onPatch} />;
  else if (isCondition)
    content = <Condition value={value ?? ""} path={path} onPatch={onPatch} />;
  else if (
    schema.enum ||
    schema.const !== undefined ||
    schema.type === "boolean" ||
    typeof value === "boolean"
  ) {
    const values =
      schema.enum ||
      (schema.const !== undefined ? [schema.const] : [true, false]);
    content = (
      <select
        id={id}
        value={String(value ?? "")}
        onChange={(e) =>
          onPatch(
            path,
            values.find((v) => String(v) === e.target.value) ?? e.target.value,
          )
        }
      >
        {!values.some((v) => v === value) && (
          <option value={String(value ?? "")}>
            {value === undefined ? "请选择" : String(value)}
          </option>
        )}
        {values.map((v) => (
          <option key={String(v)} value={String(v)}>
            {String(v)}
          </option>
        ))}
      </select>
    );
  } else if (schema.type === "array" || Array.isArray(value)) {
    const items = Array.isArray(value) ? value : [];
    content = (
      <div className="array-field">
        {items.map((item, index) => (
          <div className="array-item" key={index}>
            <div className="array-toolbar">
              <span>{String(index + 1).padStart(2, "0")}</span>
              <div>
                <button
                  className="icon-button"
                  aria-label={`上移 ${field} ${index + 1}`}
                  disabled={index === 0}
                  onClick={() => {
                    const next = [...items];
                    [next[index - 1], next[index]] = [
                      next[index],
                      next[index - 1],
                    ];
                    onPatch(path, next);
                  }}
                >
                  <ArrowUp size={13} />
                </button>
                <button
                  className="icon-button"
                  aria-label={`下移 ${field} ${index + 1}`}
                  disabled={index === items.length - 1}
                  onClick={() => {
                    const next = [...items];
                    [next[index + 1], next[index]] = [
                      next[index],
                      next[index + 1],
                    ];
                    onPatch(path, next);
                  }}
                >
                  <ArrowDown size={13} />
                </button>
                <button
                  className="icon-button danger"
                  aria-label={`删除 ${field} ${index + 1}`}
                  onClick={() =>
                    onPatch(
                      path,
                      items.filter((_, i) => i !== index),
                    )
                  }
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
            <Fields
              value={item}
              schema={schema.items || {}}
              path={[...path, index]}
              onPatch={onPatch}
              depth={depth + 1}
            />
          </div>
        ))}
        <button
          className="add-row"
          onClick={() => {
            const item =
              field === "decision"
                ? { when: "total_score >= 100", result: "review" }
                : field === "conclusion"
                  ? { when: "total_score >= 100", signal: "review" }
                  : defaultValue(schema.items || {});
            const next = [...items];
            const fallback = next.findIndex((v) => object(v).default === true);
            next.splice(fallback >= 0 ? fallback : next.length, 0, item);
            onPatch(path, next);
          }}
        >
          <Plus size={14} />
          添加{labels[field]?.split(" · ")[0] || "一项"}
        </button>
      </div>
    );
  } else if (
    schema.type === "object" ||
    (value !== null && typeof value === "object")
  ) {
    const properties = schema.properties || {};
    let required = schema.required || [];
    if (inputSchema.$ref?.endsWith("/step"))
      required = [
        ...required,
        ...(record.type === "router"
          ? ["routes", "default"]
          : record.type === "service"
            ? ["service", "operation", "next"]
            : [String(record.type), "next"]),
      ];
    const existing = [...new Set([...required, ...Object.keys(record)])];
    const missing = Object.keys(properties).filter(
      (key) => !existing.includes(key),
    );
    content = (
      <div className="object-fields">
        {existing.map((key) => (
          <div className="property" key={key}>
            <div className="property-heading">
              <label>
                {labels[key] || key}
                {required.includes(key) && <span className="required"> *</span>}
              </label>
              <code>{key}</code>
              {key in record && !required.includes(key) && (
                <button
                  className="icon-button remove-property"
                  aria-label={`移除字段 ${[...path, key].join(".")}`}
                  onClick={() => onPatch([...path, key], undefined)}
                >
                  <Trash2 size={12} />
                </button>
              )}
            </div>
            <Fields
              value={record[key]}
              schema={
                key === "type" && inputSchema.$ref?.endsWith("/feature")
                  ? definitions.feature.properties!.type
                  : properties[key] || {}
              }
              path={[...path, key]}
              onPatch={onPatch}
              depth={depth + 1}
            />
          </div>
        ))}
        {missing.length > 0 && (
          <select
            className="add-property"
            aria-label={`${path.join(".")} 添加字段`}
            value=""
            onChange={(e) => {
              if (e.target.value)
                onPatch(
                  [...path, e.target.value],
                  defaultValue(properties[e.target.value]),
                );
            }}
          >
            <option value="">＋ 添加可选字段</option>
            {missing.map((key) => (
              <option key={key} value={key}>
                {labels[key] || key} ({key})
              </option>
            ))}
          </select>
        )}
        {schema.additionalProperties !== false && (
          <MapEntry
            path={path}
            properties={record}
            itemSchema={
              typeof schema.additionalProperties === "object"
                ? schema.additionalProperties
                : {}
            }
            onPatch={onPatch}
          />
        )}
      </div>
    );
  } else if (
    schema.type === "number" ||
    schema.type === "integer" ||
    typeof value === "number"
  ) {
    content = (
      <input
        id={id}
        type="number"
        min={schema.minimum}
        max={schema.maximum}
        step={schema.type === "integer" ? 1 : "any"}
        value={value === undefined ? "" : String(value)}
        onChange={(e) => {
          if (e.target.value === "") onPatch(path, "");
          else if (Number.isFinite(e.target.valueAsNumber))
            onPatch(path, e.target.valueAsNumber);
        }}
      />
    );
  } else if (
    !schema.type &&
    !schema.$ref &&
    typeof value !== "string" &&
    value !== undefined
  )
    content = (
      <RawValue
        key={JSON.stringify(value)}
        value={value}
        path={path}
        onPatch={onPatch}
      />
    );
  else
    content =
      field === "description" ||
      field === "reason" ||
      field === "expression" ? (
        <textarea
          id={id}
          rows={3}
          value={String(value ?? "")}
          onChange={(e) => onPatch(path, e.target.value)}
        />
      ) : (
        <input
          id={id}
          type="text"
          value={String(value ?? "")}
          spellCheck={false}
          onChange={(e) => onPatch(path, e.target.value)}
        />
      );
  // The path stays visible to assistive technology even for anonymous array entries.
  return (
    <div className="field-control" aria-label={path.join(".")}>
      <label className="sr-only" htmlFor={id}>
        {path.join(".")}
      </label>
      {label && <h3>{label}</h3>}
      {content}
    </div>
  );
}
function MapEntry({
  path,
  properties,
  itemSchema,
  onPatch,
}: {
  path: Path;
  properties: ReturnType<typeof object>;
  itemSchema: Schema;
  onPatch: Patch;
}) {
  const [key, setKey] = useState("");
  return (
    <div className="map-entry">
      <input
        aria-label={`${path.join(".")} 新属性名`}
        value={key}
        placeholder="新属性名"
        onChange={(e) => setKey(e.target.value)}
      />
      <button
        className="icon-button"
        aria-label={`${path.join(".")} 添加属性`}
        disabled={!key.trim() || Object.hasOwn(properties, key.trim())}
        onClick={() => {
          onPatch([...path, key.trim()], defaultValue(itemSchema));
          setKey("");
        }}
      >
        <Plus size={15} />
      </button>
    </div>
  );
}
