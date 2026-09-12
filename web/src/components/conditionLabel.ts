import { object, type Value } from "../model";

/** Display condition groups without changing their Boolean grouping. */
export function conditionLabel(value: Value | undefined, depth = 0): string {
  if (typeof value === "string") return value.trim() || "未配置条件";
  if (value == null) return "未配置条件";
  const record = object(value);
  const keys = Object.keys(record);
  if (depth < 12 && keys.length === 1) {
    const mode = keys[0];
    const children = record[mode];
    if (Array.isArray(children) && children.length) {
      const parts = children.map((child) => conditionLabel(child, depth + 1));
      if (mode === "not" && parts.length === 1) return `NOT (${parts[0]})`;
      if (mode === "all" || mode === "any") {
        if (parts.length === 1) return parts[0];
        return parts
          .map((part) => `(${part})`)
          .join(mode === "all" ? "\nAND " : "\nOR ");
      }
    }
  }
  return JSON.stringify(value);
}
