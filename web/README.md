# CDL Studio

本地可视化 CDL 编辑应用。React + TypeScript 前端通过独立 Node 服务调用仓库的 `corint validate`，无需启动决策引擎或配置数据库。

视觉样式沿用 `corint-cognition` 的 WebUI 设计基线：`web/src/theme.css` 对齐其 `packages/webui/src/index.css` 与 `branding.css` 的品牌、面板、文字和流程图语义色；按钮、表单和分段视图切换沿用 `DesignButton`、`DesignInput`、`DesignSegmentedControl` 的外观。品牌图标使用同源 Corint 资产。支持浅色、深色与跟随系统主题，并保存选择。样式和资产存放于本应用，运行时不依赖相邻仓库。

## 启动

需要 Node.js 22.12+ 和 Rust 工具链。在仓库根目录执行：

```sh
cargo build --locked -p corint-decision-cli
cd web
npm ci
npm run dev
```

打开 <http://127.0.0.1:5173>。默认读取项目根目录 `repository/` 下的 YAML / YML / JSON 文件，按实际相对路径显示可折叠目录树；搜索文件、目录或资源名称时会展开匹配路径。之后优先恢复当前浏览器的 repository 草稿。原示例工作区的缓存单独保留，不会覆盖新工作区。读取目录不跟随符号链接。

可通过 `PORT=5174 npm run dev` 修改端口，通过 `CORINT_CLI=/absolute/path/to/corint npm run dev` 使用其他已确认版本的 CLI（包括自定义 Cargo target 目录）。服务只绑定本机地址。

## 编辑与校验

- **Pipeline**：点击节点编辑属性，拖动节点调整视图，连接端口修改 `entry`、`next` 或 Router 路由；可添加五类节点、删除节点、配置最终决策。改名会同步当前 Pipeline 中的节点引用；删除节点会将入边接至其下一节点或默认分支。节点位置仅保留在当前画布，不写入 CDL。
- **编辑方式**：仅 Pipeline 提供可视化流程图及节点、流程属性表单，字段来自 `CDL/schema/authoring.json`。Rule、Ruleset、Registry、Feature、List、Service 及其他文件直接使用 YAML 源码编辑；切换资源时自动显示对应编辑方式。
- **同文件多资源**：支持连续声明多个 Pipeline / Rule / Ruleset，也支持 `---` 分隔的多个资源文档和拆分文件；共享文件头的 version / import。文件包含多个 Pipeline 时可选择当前流程，编辑不会覆盖相邻资源。规则内部重复字段仍报错，CLI 检查所有资源和重复 ID。
- **源码同步**：YAML 编辑器支持适配明暗主题的语法高亮、行号、两空格缩进及键盘撤销。表单更改写回 YAML AST，保留未修改位置的注释与字段；结构重排或整块替换会重新序列化该块。错误 YAML 保留原文，修复后恢复可视化编辑。工作区撤销/重做保留最近 60 次操作。
- **文件导入**：支持多选 YAML / JSON，或导入文件夹保留相对路径。导入会合并工作区，同路径覆盖前确认。可编辑文件路径；修改路径后需同步相关 import。资源 ID 的跨文件引用通过校验检查，不自动改写。
- **导出**：导出当前 YAML，或导出包含所有文件原文和相对路径的 `workspace.cdl.json`。完整工作区可重新导入；它是编辑器工作区格式，不是可执行 CDL 包。浏览器草稿不会修改仓库文件，也不会发布策略。
- **校验**：将工作区写入临时目录，以 `cdl-static-1` 执行 `corint validate --format json --root <临时目录> .`，检查整个集合及跨文件引用，完成后清理目录。CLI 错误显示文件、字段、错误码，点击诊断可打开对应源码。编辑后旧结果标为过期。

校验不会运行策略、调用外部数据源或执行动作。目前页面未提供 `--input-schema` 参数；导入的输入 Schema 作为辅助文件跳过，输入字段类型仍标为未检查。完整检查范围以报告的 `references_checked`、`input_schema_checked`、`skipped_sources` 和 `unchecked` 为准。

每个工作区最多 100 个文件，单文件最多 4 MiB，校验内容合计最多 10 MiB。草稿存储受浏览器额度限制，保存失败时会显示提示，可立即导出保存。

## 构建与测试

```sh
npm run build
npm start
npm test
```

`npm start` 在同一端口提供 `dist/` 和校验 API。应用需保留当前仓库布局，以读取 repository 与 CLI；这是本地编辑服务，没有多用户身份、远程发布或生产激活功能。

测试涵盖 YAML 无损字段修改、错误输入保留、节点引用更新、七类模板、导入边界，以及真实 CLI 的成功、缺失依赖、语法错误和未安装场景；运行测试前先构建 CLI。
