import { useEffect, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

type Theme = "light" | "dark" | "system";
const STORAGE_KEY = "corint.cdl-studio.theme.v1";
const options = [
  { value: "light", label: "浅色主题", Icon: Sun },
  { value: "dark", label: "深色主题", Icon: Moon },
  { value: "system", label: "跟随系统主题", Icon: Monitor },
] as const;

function readTheme(): Theme {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    /* Theme selection also works when browser storage is unavailable. */
  }
  return "system";
}

function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme =
    theme === "system"
      ? matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
}

export function initializeTheme() {
  applyTheme(readTheme());
}

export default function ThemeControl() {
  const [theme, setTheme] = useState<Theme>(readTheme);
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const sync = () => applyTheme(theme);
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, [theme]);
  return (
    <div className="theme-control" role="group" aria-label="界面主题">
      {options.map(({ value, label, Icon }) => (
        <button
          key={value}
          className="icon-button"
          aria-label={label}
          title={label}
          aria-pressed={theme === value}
          onClick={() => {
            setTheme(value);
            try {
              localStorage.setItem(STORAGE_KEY, value);
            } catch {
              /* Keep the current session usable. */
            }
          }}
        >
          <Icon size={16} strokeWidth={1.8} />
        </button>
      ))}
    </div>
  );
}
