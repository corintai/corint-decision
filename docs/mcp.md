# MCP：AI 策略开发与诊断接口

`corint-mcp` 是独立的本地 MCP Server，支持 stdio 和 Streamable HTTP，供外部 AI 客户端读取 CDL、校验候选、运行行为样例、试算事件和比较策略版本。使用官方 Rust SDK `rmcp` 处理协议；业务语义复用现有 toolchain 和 DecisionEngine。

本期支持 macOS/Linux。它是本地开发入口，不连接生产 HTTP 服务，不提供公网 MCP、策略写入、发布重载或外部 MCP Client。Agent 用自身已授权的文件编辑工具修改候选，再调用本服务检查。MCP 本身无需 LLM API key、数据库或生产凭据。

## 同时启动 Decision 与 MCP

`scripts/` 目录提供 [start.sh](../scripts/start.sh)、[restart.sh](../scripts/restart.sh) 和 [stop.sh](../scripts/stop.sh)，需要 Python 3.9+、Rust/Cargo，以及构建 Decision Server 所需的 `protoc`。在仓库根目录运行：

```sh
./scripts/start.sh              # 编译并后台启动两个服务
./scripts/restart.sh            # 先编译成功，再优雅停止并启动两个服务
./scripts/stop.sh               # 优雅停止本脚本管理的服务
./scripts/start.sh --no-build   # 使用已构建的 debug 二进制
./scripts/restart.sh --release  # 构建并使用 release 二进制
./scripts/start.sh --only mcp   # 仅启动 MCP（已有独立 Decision 服务时适用）
```

三个脚本都可从其他工作目录调用。重复 start 不会重复创建进程；启动失败会回收本次已启动的进程，保留原先已运行的服务。PID 身份校验和文件锁避免误杀旧 PID 对应的其他进程及并发启动冲突。

`restart.sh` 还会恢复缺少 PID 文件的旧实例：必须属于当前用户，工作目录与本仓库一致，真实可执行文件位于当前 target 的 debug/release 目录，且启动参数匹配。确认只有一个实例后才纳入管理并优雅停止；其他项目、其他用户或身份不明确的进程不会被停止。多个实例存在歧义时，在停止任何服务之前报错。macOS 上发现旧进程使用系统 `lsof`，Linux 使用 `/proc`。

恢复旧实例时，将其运行时 `CORINT_*` 配置（排除脚本控制项与 demo 管理凭据）及 `DATABASE_URL` 保存在权限为 `600` 的 `.run/<service>.environment.json`，后续重启继续使用；当前 shell 显式设置的同名变量优先。这样旧版的数据库鉴权配置不会被本地生成的 token 替换。需要清除保留配置时，可移走对应 environment 文件再启动。配置值不打印到终端。无关进程占用端口仍会报错，并展示实际启动错误及日志位置。

Decision Server 继续读取原有 `config/server.yaml`、`.env` 和 `CORINT_*` 配置；显式的 `CORINT_CORE_CONFIG` / `CORINT_TENANT_CONFIG` 也按原入口处理。其端口以实际配置为准。若启动本地兼容模式，且未提供凭据/鉴权配置、也不存在 `.env`，脚本自动生成两个独立随机 token，保存在权限为 `600` 的 `.run/local-credentials.json`，重启复用，不打印 token。已有身份配置不会被替换。

MCP 后台使用 HTTP 模式，默认地址为 `http://127.0.0.1:8082/mcp`，健康检查为 `/health`。它仅允许 loopback 监听，启用 Host 和 Origin 校验；本期 HTTP 入口面向本机客户端，没有用户登录或多租户鉴权。原有 stdio 模式不变。

| 环境变量 | 默认值 / 用途 |
|---|---|
| `CORINT_MCP_CONFIG` | 显式策略目录配置；未设置且无 `config/mcp.json` 时自动读取 Decision Server 的文件仓库 |
| `CORINT_MCP_LISTEN` | `127.0.0.1:8082`；修改 MCP HTTP 监听地址 |
| `CORINT_RUN_DIR` | `.run/`；PID、锁、日志及本地开发凭据目录 |
| `CORINT_START_TIMEOUT` | `60` 秒；等待实际监听端口就绪 |
| `CORINT_STOP_TIMEOUT` | `180` 秒；优雅退出上限，超时保留进程并报错，不强制 SIGKILL |
| `CARGO_TARGET_DIR` | `target/`；构建产物位置 |

自定义这些环境变量后，后续操作应使用相同设置，尤其是 `CORINT_RUN_DIR`。`.run/decision.log` 和 `.run/mcp.log` 追加保存日志。`.run/` 及本地 `config/mcp.json` 已加入 Git 忽略项。默认通过 Decision Server 的只读 `--repository-info` 命令解析同一份有效配置（包括保留的运行环境），将文件仓库路径交给 MCP；不会回退到示例。自动模式即使 `--only mcp` 也会构建用于读取配置的 Decision 二进制，但不启动 Decision。数据库/API 仓库及 Core/多租户模式目前不支持自动发现，会明确报错；可显式配置本地候选目录。

也可单独运行 HTTP MCP：

```sh
./target/debug/corint-mcp --repository /absolute/path/to/policy-repository --http-listen 127.0.0.1:8082
```

支持 HTTP MCP 的客户端连接 `http://127.0.0.1:8082/mcp`，不再通过命令启动 stdio 子进程。

## 启动与连接

<!-- executable-example: mcp-repository-discovery -->

默认使用上面的启动脚本，自动读取 Decision Server 配置的策略仓库。若客户端需要 stdio 模式，可构建后直接指定实际策略仓库路径：

```sh
cargo build -p corint-decision-mcp --locked
./target/debug/corint-mcp --repository /absolute/path/to/policy-repository
```

服务通过 stdin/stdout 接收和返回 MCP 消息，启动后等待客户端连接是正常行为；stdout 不输出普通日志。客户端断开后进程退出。不要在协议流上合并 stderr。

在支持 stdio MCP 的客户端中填写可执行文件和参数。采用 `mcpServers` 配置格式的客户端可使用以下模板，将可执行文件路径和策略仓库路径替换为实际值：

```json
{
  "mcpServers": {
    "corint-decision": {
      "command": "/absolute/path/corint-decision/target/debug/corint-mcp",
      "args": [
        "--repository",
        "/absolute/path/to/policy-repository"
      ]
    }
  }
}
```

`command` 和参数是通用接入信息；配置文件位置及外层格式由具体客户端决定。二进制内嵌 CDL 文档，更新文档后需重新构建。

## 真实仓库读取

`--repository PATH` 每次工具调用和资源查询都重新读取指定仓库，自动发现 `pipelines/`、`rulesets/` 下的策略声明。`policy_id` 使用 `pipeline/<实际ID>` 或 `ruleset/<实际ID>`，避免不同资源类型的同名冲突；响应另含 `kind`、`resource_id`、仓库路径及 `source: repository`。新增、修改、删除策略无需重启 MCP；更改仓库路径需重启。

源码发现仅限 `rules/`、`rulesets/`、`pipelines/`、`features/`、`lists/`、`services/`，不读取鉴权或数据源配置。`get_policy` 与策略 Resource 使用现有 CDL 校验器解析引用，返回入口及已解析的依赖源码；静态错误由 `validate_policy` 报告。最多 4096 个资源文件、32 层目录，总源码上限 16 MiB；重复 ID、非法 YAML、符号链接或读取错误明确报错。资源 ID 支持最多 256 个 ASCII 字母、数字、`-`、`_`、`.`。

这是仓库当前源码视图，包含可能尚未发布的 Ruleset，`activation_status: not_checked` 不代表引擎已加载版本。自动模式不会合成输入 Schema 或 Registry，因此读取和静态校验可用，Core 试算/测试仍需显式目录配置中的完整 Core 源码和输入 Schema。

## 策略目录配置（可选的显式候选模式）

配置是本地操作者管理的 JSON；工具调用只接受已登记的策略 ID，不接受任意路径、Shell 命令或 URL。每个条目表示一个可读取的源码集合，可以登记同一业务的 baseline 和 candidate 两个 checkout：

```json
{
  "policies": [
    {
      "id": "payment-baseline",
      "description": "支付策略基线",
      "root": "../policies/baseline",
      "files": ["rule.yaml", "ruleset.yaml", "pipeline.yaml", "registry.yaml"],
      "input_schema": "input-schema.yaml",
      "cases": "behavior.yaml"
    },
    {
      "id": "payment-candidate",
      "description": "待验证的支付策略候选",
      "root": "../policies/candidate",
      "files": ["rule.yaml", "ruleset.yaml", "pipeline.yaml", "registry.yaml"],
      "input_schema": "input-schema.yaml",
      "cases": "behavior.yaml"
    }
  ]
}
```

- `root` 相对于配置文件所在目录解析，也支持绝对目录。配置修改后需重启服务；源码修改在下一次调用时重新读取。
- `files` 必须列出完整源码闭包，包括静态校验所需的引用和 import 文件。静态检查在这些文件的临时快照中进行，不自动扫描原始 root 下的其他文件。
- `input_schema` 可省略；省略时仅支持无输入类型检查的静态校验。执行工具必须提供现有模型 Schema，格式见[CLI 输入说明](cli.md)。
- `cases` 可省略；运行测试时需要通过 `cases_yaml` 提供完整行为套件。默认套件与临时套件的格式一致，见[行为测试](testing.md)。
- ID 限 1–80 个 ASCII 字母、数字、`-`、`_`。文件名限 ASCII 字母、数字、`-`、`_`、`.` 和路径分隔符 `/`，不得包含空段、`.`/`..` 路径段或绝对路径。

每个配置最多 64 个策略，每个策略最多 256 份源码；单文件最大 4 MiB，源码及输入 Schema 合计最大 16 MiB，行为套件和试算事件各限 1 MiB。读取拒绝文件路径中的符号链接和非普通文件。服务同时最多运行两个耗时操作，繁忙时返回可重试错误。

配置的根目录及其祖先由本地操作者控制。只登记允许向所连接 AI 客户端披露的文件；`get_policy` 和策略 Resource 返回原始源码和输入定义。工具不修改这些文件；静态校验使用的临时副本在结束时删除。

## 六个工具

| 工具 | 参数 | 返回内容 |
|---|---|---|
| `list_policies` | 无 | 策略 ID、说明、文件标签、输入 Schema/样例标签和 Resource URI；不声称策略有效 |
| `get_policy` | `policy_id` | 原始源码、输入 Schema、本次源码指纹；无效草稿也可读取 |
| `validate_policy` | `policy_id` | `cdl-static-1` 结构化报告，包括诊断、引用检查、未检查范围和源码指纹 |
| `test_policy` | `policy_id`、可选 `cases_yaml` | 每个样例的预期/实际结果、trace 一致性、通过/失败统计、源码与套件指纹 |
| `evaluate_decision` | `policy_id`、对象 `event`、可选布尔 `trace` | 真实 Core 引擎的结果与可选执行轨迹；标记 `mode: offline_trial`、`actions_executed: false` |
| `compare_policy_versions` | `baseline_policy_id`、`candidate_policy_id`、可选 `cases_yaml` | 同一组样例的确定性结果差异、变更样例数、双方完整测试报告及指纹 |

例如在显式候选模式中，为 `payment-baseline` 配置完整 Core 源码和包含 `amount` 字段的输入 Schema 后，可以试算：

```json
{"policy_id":"payment-baseline","event":{"amount":1001},"trace":true}
```

比较默认使用 **baseline 配置的套件**，不会读取 candidate 的套件；提供 `cases_yaml` 时，两边使用完全相同的调用方套件。比较忽略请求 ID 和耗时，覆盖决策、分数、规则命中、动作意图、步骤、局部结果和输入错误。比较成功表示完成了比较，候选是否满足预期应查看 `candidate.tests.failed`；不能把 `changed_cases: 0` 当作样例全部通过。

所有工具同时返回 `structuredContent` 和相同 JSON 的文本内容。静态校验失败、测试断言失败、源码读取或执行失败均设置 `isError: true`；保留已有 Core 诊断的错误码、阶段与位置。协议无法识别的方法或参数由 SDK 返回协议/参数错误。

`snapshot_sha256` 是本适配器对声明顺序下的源码标签、原始字节和输入 Schema 计算的内容身份，使用域前缀 `corint-mcp-snapshot-v1`。它不是 repository 的 `policy_sha256`，也不是版本发布或批准证明。`suite_sha256` 是本次套件原始 UTF-8 字节的 SHA-256。读取多个文件不提供跨文件事务；需要稳定比较时使用冻结 checkout，避免调用期间修改源码。

## Resources 与推荐工作流

`resources/list` 提供以下资源，`resources/read` 可读取其文本内容：

- `corint://cdl/overall` 以及 `rule`、`ruleset`、`pipeline`、`registry`、`feature`、`list`、`service`：内嵌语言规范。
- `corint://cdl/authoring-schema`、`corint://cdl/input-schema`、`corint://cdl/behavior-suite-schema`：编写及测试所需的格式定义。
- `corint://policies/{policy_id}`：与 `get_policy` 相同的完整源码与输入 Schema 快照。

推荐流程：列出策略 → 读取规范和候选 → Agent 编辑候选文件 → 静态校验并修复 → 运行独立样例 → 试算与比较 → 展示修改及证据。自然语言生成和解释由外部 Agent 完成，MCP Server 不自动调用模型或修改样例来使测试通过。

静态校验覆盖七类 CDL 资源，但执行工具遵守实验性的 `cdl-core-risk-draft-1` 契约，使用显式文件闭包，不解析 Core imports，也不启用 Feature/Service/List 外部客户端。需要 imports 的 Core 工程应先按[导入解析](resolution.md)的规范准备可执行源码。静态成功不代表 Core 可执行；行为样例通过不等于业务效果验证或发布批准。试算结果中的 actions 是动作意图，不会执行。

## 验证

```sh
cargo test -p corint-decision-mcp --locked
python3 tests/scripts/test_mcp_http.py
python3 tests/scripts/test_services.py
python3 tests/scripts/check_docs.py
```

[stdio 集成测试](../crates/corint-decision-mcp/tests/stdio.rs) 启动真实子进程，通过官方 SDK 客户端验证握手、工具/资源发现、静态与行为检查、事件试算、源码更新、版本差异、错误恢复和文件边界。该测试使用本地合成样例，不连接生产系统。
