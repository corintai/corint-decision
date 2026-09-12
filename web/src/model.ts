import {
  isMap,
  isScalar,
  parseAllDocuments,
  stringify,
  visit,
  type YAMLMap,
} from "yaml";
import authoringSchema from "../../CDL/schema/authoring.json";

export type Value =
  null | boolean | number | string | Value[] | { [key: string]: Value };
export type ObjectValue = { [key: string]: Value };
export type Path = (string | number)[];
export interface PolicyFile {
  path: string;
  source: string;
}
export type Kind =
  | "pipeline"
  | "rule"
  | "ruleset"
  | "registry"
  | "features"
  | "list"
  | "service"
  | "unknown";
export const kinds: Kind[] = [
  "pipeline",
  "rule",
  "ruleset",
  "registry",
  "features",
  "list",
  "service",
];
export const titles: Record<Kind, string> = {
  pipeline: "流程",
  rule: "规则",
  ruleset: "规则集",
  registry: "入口注册",
  features: "特征",
  list: "名单",
  service: "服务",
  unknown: "其他文件",
};
export const kindNames: Record<Kind, string> = {
  pipeline: "Pipeline",
  rule: "Rule",
  ruleset: "Ruleset",
  registry: "Registry",
  features: "Feature",
  list: "List",
  service: "Service",
  unknown: "YAML",
};
export interface Schema {
  $ref?: string;
  type?: string;
  const?: Value;
  enum?: Value[];
  properties?: Record<string, Schema>;
  required?: string[];
  items?: Schema;
  oneOf?: Schema[];
  anyOf?: Schema[];
  allOf?: { if?: Schema; then?: Schema }[];
  additionalProperties?: boolean | Schema;
  minimum?: number;
  maximum?: number;
  description?: string;
}
export const definitions = authoringSchema.definitions as unknown as Record<
  string,
  Schema
>;
export const object = (value: Value | undefined): ObjectValue =>
  value && typeof value === "object" && !Array.isArray(value) ? value : {};
export function resolveSchema(input: Schema, value?: Value): Schema {
  let schema = input.$ref
    ? definitions[input.$ref.split("/").pop()!] || {}
    : input;
  if (input.$ref?.endsWith("/feature"))
    schema =
      definitions[`${object(value).type || "expression"}Feature`] || schema;
  return schema;
}
const resourceKeys = new Set([
  "rule",
  "ruleset",
  "pipeline",
  "registry",
  "features",
  "lists",
]);
export function parseSource(source: string, pipelineIndex = 0) {
  try {
    const documents = parseAllDocuments(source, { uniqueKeys: false });
    const resources: {
      data: ObjectValue;
      documentIndex: number;
      itemIndex: number;
      kind: Kind;
    }[] = [];
    let shared: ObjectValue = {};
    documents.forEach((document, documentIndex) => {
      if (document.errors.length) throw new Error(document.errors[0].message);
      if (!isMap(document.contents))
        throw new Error("CDL 顶层必须是一个对象。");
      const root = document.contents;
      visit(document, {
        Map(_, node) {
          const seen = new Set<string>();
          for (const item of node.items) {
            const key = isScalar(item.key)
              ? String(item.key.value)
              : String(item.key);
            if (seen.has(key) && !(node === root && resourceKeys.has(key)))
              throw new Error(`重复字段「${key}」：只有顶层资源声明可以重复。`);
            seen.add(key);
          }
        },
      });
      const keyAt = (index: number) => String(root.items[index].key);
      const declarations = root.items
        .map((_, index) => index)
        .filter((index) => resourceKeys.has(keyAt(index)));
      // Rename only the clone's root keys for decoding. Every declaration and
      // anchor remains present, so aliases can refer to earlier resources.
      const copy = document.clone();
      const copyRoot = copy.contents as YAMLMap;
      copyRoot.items.forEach((pair, index) => {
        pair.key = copy.createNode(String(index));
      });
      const values = copy.toJS({ maxAliasCount: 100 }) as Record<string, Value>;
      JSON.stringify(values);
      const common = Object.fromEntries(
        root.items
          .map((_, index) => [keyAt(index), values[String(index)]])
          .filter(([key]) => !resourceKeys.has(String(key))),
      ) as ObjectValue;
      const candidates = declarations.length ? declarations : [-1];
      for (const itemIndex of candidates) {
        const data: ObjectValue =
          itemIndex < 0
            ? { ...common }
            : { ...common, [keyAt(itemIndex)]: values[String(itemIndex)] };
        const explicitVersion = "version" in data;
        if (documentIndex === 0) {
          shared = Object.fromEntries(
            Object.entries(data).filter(
              ([key]) => key === "version" || key === "import",
            ),
          );
        } else {
          for (const [key, value] of Object.entries(shared)) {
            if (key in data && key === "import")
              throw new Error(`文件头与资源中的 ${key} 冲突。`);
            if (!(key in data)) data[key] = value;
          }
        }
        if (
          documentIndex === 0 &&
          !declarations.length &&
          Object.keys(data).length &&
          Object.keys(data).every(
            (key) => key === "version" || key === "import",
          )
        )
          continue;
        if (
          (itemIndex >= 0 && keyAt(itemIndex) === "lists") ||
          (documentIndex > 0 &&
            !explicitVersion &&
            ("base_url" in data || "backend" in data || "datasource" in data))
        )
          delete data.version;
        resources.push({
          data,
          documentIndex,
          itemIndex,
          kind: detectKind(data),
        });
      }
    });
    if (!resources.length) throw new Error("文件中没有 CDL 资源。");
    const pipelines = resources.filter(
      (resource) => resource.kind === "pipeline",
    );
    const selected = pipelines[pipelineIndex] || pipelines[0] || resources[0];
    return {
      document: documents[selected.documentIndex],
      documents,
      resources,
      pipelines,
      selected,
      data: selected.data,
      error: null,
    };
  } catch (error) {
    return {
      document: null,
      documents: [],
      resources: [],
      pipelines: [],
      selected: null,
      data: null,
      error: (error as Error).message,
    };
  }
}
export function detectKind(data: ObjectValue | null): Kind {
  if (!data) return "unknown";
  for (const kind of [
    "pipeline",
    "rule",
    "ruleset",
    "registry",
    "features",
  ] as Kind[])
    if (kind in data) return kind;
  if (
    "lists" in data ||
    ("id" in data && ("backend" in data || "datasource" in data))
  )
    return "list";
  if ("operations" in data || "base_url" in data) return "service";
  return "unknown";
}
export function patchSource(
  source: string,
  path: Path,
  value: Value | undefined,
  pipelineIndex = 0,
): string {
  const { document, documents, selected, error } = parseSource(
    source,
    pipelineIndex,
  );
  if (!document || !selected || !isMap(document.contents))
    throw new Error(error!);
  if (!path.length) {
    if (
      documents.length > 1 ||
      document.contents.items.filter((pair) =>
        resourceKeys.has(String(pair.key)),
      ).length > 1
    )
      throw new Error("请通过资源字段修改，避免替换同文件中的其他资源。");
    return stringify(value, { lineWidth: 0 });
  }
  const pair = (document.contents as YAMLMap).items[selected.itemIndex];
  if (pair && String(pair.key) === path[0]) {
    if (path.length === 1) {
      if (value === undefined)
        document.contents.items.splice(selected.itemIndex, 1);
      else pair.value = document.createNode(value);
    } else {
      if (!isMap(pair.value)) throw new Error("当前资源不是可编辑的对象。");
      if (value === undefined) pair.value.deleteIn(path.slice(1));
      else pair.value.setIn(path.slice(1), value);
    }
  } else {
    if (value === undefined) document.deleteIn(path);
    else document.setIn(path, value);
  }
  return documents
    .map((doc, index) => {
      if (index > 0) doc.directives.docStart = true;
      return doc.toString({ lineWidth: 0 });
    })
    .join("");
}
export function defaultValue(input: Schema, depth = 0): Value {
  const schema = resolveSchema(input);
  if (schema.const !== undefined) return schema.const;
  if (schema.enum) return schema.enum[0];
  if (depth > 5) return "";
  if (input.$ref?.endsWith("/feature"))
    return {
      name: "new_feature",
      type: "expression",
      expression: "event.amount",
    };
  if (schema.type === "object")
    return Object.fromEntries(
      (schema.required || []).map((key) => [
        key,
        defaultValue(schema.properties?.[key] || {}, depth + 1),
      ]),
    );
  if (schema.type === "array") return [];
  if (schema.type === "boolean") return false;
  if (schema.type === "integer" || schema.type === "number")
    return schema.minimum || 0;
  return "";
}
export function template(kind: Kind, id: string): PolicyFile {
  const values: Record<Kind, Value> = {
    rule: {
      version: "0.1",
      rule: {
        id,
        name: "新规则",
        when: { all: ["event.amount > 1000"] },
        score: 50,
      },
    },
    ruleset: {
      version: "0.1",
      ruleset: {
        id,
        name: "新规则集",
        rules: ["blocked"],
        conclusion: [
          { when: "total_score >= 100", signal: "review" },
          { default: true, signal: "pass" },
        ],
      },
    },
    pipeline: {
      version: "0.1",
      pipeline: {
        id,
        name: "新流程",
        entry: "check",
        steps: [
          {
            step: {
              id: "check",
              name: "规则检查",
              type: "ruleset",
              ruleset: "payment",
              next: "end",
            },
          },
        ],
        decision: [{ default: true, result: "approve" }],
      },
    },
    registry: {
      version: "0.1",
      registry: [{ pipeline: "payment_pipeline", when: "true" }],
    },
    features: {
      version: "0.2",
      features: [{ name: id, type: "expression", expression: "event.amount" }],
    },
    list: { id, backend: "memory", initial_values: [] },
    service: {
      name: id,
      base_url: "https://example.invalid",
      operations: { assess: { method: "POST", path: "/assess" } },
    },
    unknown: {},
  };
  const folders: Record<Kind, string> = {
    pipeline: "pipelines",
    rule: "rules",
    ruleset: "rulesets",
    registry: "",
    features: "features",
    list: "lists",
    service: "services",
    unknown: "",
  };
  return {
    path: `${folders[kind] ? folders[kind] + "/" : ""}${id}.yaml`,
    source: stringify(values[kind], { lineWidth: 0 }),
  };
}
export interface Step extends ObjectValue {
  id: string;
  name: string;
  type: string;
}
export function pipelineSteps(data: ObjectValue): Step[] | null {
  const pipeline = object(data.pipeline);
  if (!Array.isArray(pipeline.steps)) return null;
  const steps = pipeline.steps.map((wrapper) => object(object(wrapper).step));
  if (
    steps.some(
      (step) => typeof step.id !== "string" || typeof step.type !== "string",
    ) ||
    new Set(steps.map((s) => s.id)).size !== steps.length ||
    steps.some((s) => s.id === "end" || s.id === "__entry")
  )
    return null;
  return steps as Step[];
}
export function renameStep(
  pipeline: ObjectValue,
  index: number,
  newId: string,
): ObjectValue {
  const result = structuredClone(pipeline);
  const steps = result.steps as ObjectValue[];
  const previous = object(steps[index].step).id;
  object(steps[index].step).id = newId;
  if (result.entry === previous) result.entry = newId;
  for (const wrapper of steps) {
    const step = object(wrapper.step);
    for (const key of ["next", "default"])
      if (step[key] === previous) step[key] = newId;
    if (Array.isArray(step.routes))
      for (const route of step.routes)
        if (object(route).next === previous) object(route).next = newId;
  }
  return result;
}
export function validWorkspace(
  value: unknown,
): value is { files: PolicyFile[] } {
  if (!value || typeof value !== "object" || !("files" in value)) return false;
  const files = value.files;
  return (
    Array.isArray(files) &&
    files.length > 0 &&
    files.length <= 100 &&
    files.every(
      (f) =>
        f &&
        typeof f.path === "string" &&
        /^[\p{L}\p{N}_ .\-/]+\.(yaml|yml|json)$/iu.test(f.path) &&
        !f.path.startsWith("/") &&
        !f.path
          .split("/")
          .some((part: string) => !part || part === "." || part === "..") &&
        typeof f.source === "string" &&
        f.source.length <= 4 * 1024 * 1024,
    ) &&
    new Set(files.map((f) => f.path.toLowerCase())).size === files.length
  );
}
