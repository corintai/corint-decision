import express from "express";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { checkFiles, validateFiles } from "./validation.mjs";
import { readRepository } from "./repository.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const webRoot = path.join(root, "web");
const port = Number(process.env.PORT || 5173);
const binary = process.env.CORINT_CLI || path.join(root, "target/debug/corint");
const app = express();
const httpServer = createServer(app);
app.disable("x-powered-by");
// Local authoring service: reject foreign origins and DNS rebinding hosts.
app.use((req, res, next) => {
  if (!["127.0.0.1", "localhost", "[::1]"].includes(req.hostname))
    return res.status(403).json({ error: "仅允许本机访问。" });
  if (
    req.headers.origin &&
    ![`http://localhost:${port}`, `http://127.0.0.1:${port}`].includes(
      req.headers.origin,
    )
  ) {
    return res.status(403).json({ error: "不允许跨站访问。" });
  }
  next();
});
app.use("/api", express.json({ limit: "12mb" }));
app.get("/api/repository", async (_req, res) => {
  try {
    res
      .set("Cache-Control", "no-store")
      .json(await readRepository(path.join(root, "repository")));
  } catch (error) {
    res.status(500).json({
      error:
        error.code === "ENOENT"
          ? "找不到项目根目录下的 repository 目录。"
          : error.message,
    });
  }
});
const examples = [
  "registry.yaml",
  "rules/blocked.yaml",
  "rulesets/payment.yaml",
  "pipelines/payment.yaml",
  "features/payment.yaml",
  "lists/blocked.yaml",
  "services/risk.yaml",
];
app.get("/api/examples", async (_req, res) => {
  const files = await Promise.all(
    examples.map(async (name) => ({
      path: name,
      source: await readFile(
        path.join(root, "tests/conformance/cdl_authoring", name),
        "utf8",
      ),
    })),
  );
  res.json({ files });
});
let validating = false;
app.post("/api/validate", async (req, res) => {
  try {
    checkFiles(req.body?.files);
  } catch (error) {
    return res.status(400).json({ error: error.message });
  }
  if (validating)
    return res.status(429).json({ error: "校验正在进行，请稍后重试。" });
  validating = true;
  try {
    res.json(await validateFiles(req.body.files, binary));
  } catch (error) {
    res.status(503).json({ error: error.message });
  } finally {
    validating = false;
  }
});
app.use("/api", (_req, res) => res.status(404).json({ error: "接口不存在。" }));
if (process.argv.includes("--production")) {
  app.use(express.static(path.join(webRoot, "dist")));
  app.get("/{*path}", (_req, res) =>
    res.sendFile(path.join(webRoot, "dist/index.html")),
  );
} else {
  const { createServer } = await import("vite");
  const vite = await createServer({
    root: webRoot,
    server: { middlewareMode: true, hmr: { server: httpServer } },
  });
  app.use(vite.middlewares);
}
app.use((error, _req, res, _next) =>
  res.status(error.status || 500).json({
    error: error.status === 413 ? "请求体过大。" : "请求失败，请检查文件内容。",
  }),
);
httpServer.listen(port, "127.0.0.1", () =>
  console.log(`CDL Studio: http://127.0.0.1:${port}`),
);
