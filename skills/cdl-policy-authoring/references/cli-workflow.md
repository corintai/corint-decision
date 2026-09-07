# CLI 操作流程

以下命令使用已确认的 CDL 仓库和用户选择的输出目录。先替换示例绝对路径；不要改写仓库中的公共 fixture。
命令使用当前检出的源码构建 CLI，避免 PATH 中旧版 `corint` 与新规范不一致。不要求全局安装或运行服务。

## 准备资源与命令

```sh
CDL_REPO="/absolute/path/to/corint-decision"
POLICY_DIR="/absolute/path/to/policy"

cdl() {
  CARGO_INCREMENTAL=0 cargo run --quiet --locked \
    --manifest-path "$CDL_REPO/Cargo.toml" \
    -p corint-decision-cli --bin corint -- "$@"
}

cdl --help
cdl --version
```

如依赖已经缓存，可在 `cargo run` 参数中加 `--offline`；依赖未缓存时不要把构建失败当作 CDL 校验失败。
同一工作区的 Cargo 命令串行运行。仓库或目标明确提供了经过版本确认的 CLI 时，也可直接使用该程序。

## 默认静态校验

单独修改一个资源时，不要求 Registry、输入 Schema 或行为用例：

```sh
cdl validate --format json "$POLICY_DIR/features/payment.yaml" \
  > "$POLICY_DIR/validation.json"
```

仓库采用 `rules/`、`rulesets/`、`pipelines/`、`features/`、`lists/`、`services/` 和根目录 Registry 时，扫描资源目录并检查引用：

```sh
cdl validate --root "$POLICY_DIR" --format json \
  > "$POLICY_DIR/validation.json"
```

有输入 Schema 时额外加 `--input-schema "$POLICY_DIR/input-schema.yaml"`。Schema 路径始终相对当前目录解析，不相对 `--root`。
`--root DIR FILE...` 则仅加载指定的根相对文件及其传递 imports；要求这一集合包含所有引用。其他目录布局显式列出资源文件，不使用 `*.yaml` 混入 Schema、用例或备份。
静态 imports 支持 rules/rulesets/pipelines/features/lists/services，路径相对 root；单文件含 import 时也必须给出 root。

读取报告和退出码。`valid: true` 且退出码 `0` 才是通过；`1` 是校验失败，`2` 是用法/文件错误。`--format json` 只控制程序 stdout；Cargo 构建信息可能在 stderr，构建失败另行报告。`--help`/`--version` 成功不是验证证据。

按诊断的 `source`、`field_path`、`stage`、`code` 修复，再次执行。记录 `references_checked`、`input_schema_checked` 与 `unchecked`，不把 `execution_checked: false` 当作静态校验失败。
完整命令契约见[CLI 文档](../../../docs/cli.md)。

## 按需执行 Core 编译和行为测试

用户要求验证 Core 可执行性或预期行为时，准备完整 Core 闭包与输入 Schema，行为测试另需独立预期用例。这里的文件列表必须符合严格 Core，不能包含 Feature/List/Service。

```sh
set -- "$POLICY_DIR/rule.yaml" "$POLICY_DIR/ruleset.yaml" \
  "$POLICY_DIR/pipeline.yaml" "$POLICY_DIR/registry.yaml"

cdl validate --profile cdl-core-risk-draft-1 \
  --input-schema "$POLICY_DIR/input-schema.yaml" --format json "$@"

cdl test --input-schema "$POLICY_DIR/input-schema.yaml" \
  --cases "$POLICY_DIR/behavior.yaml" --format json "$@" \
  > "$POLICY_DIR/behavior-report.json"
```

仅在 Core 编译通过后执行行为测试。行为报告要求 `valid: true`、`execution_checked: true`、`test_results.executed == total`、`passed == total`、`failed == 0`，并检查各用例 `passed` 和 `trace_parity`。
预期运行错误可能出现在通过的用例中，不能仅按诊断是否为空判断行为失败。

## 编写用例时的区别

以仓库 [testing.md](../../../docs/testing.md) 和 [test-suite.json](../../../docs/contracts/schema/test-suite.json) 为准：

- 行为文件有自己的版本与 Profile，不使用资源的语言版本作为测试格式版本。
- 用例输入为 `input: {event: {...}}`。成功断言包含 `pipeline_id`、`score`、`signal`、`actions`、`triggered_rules`、`steps`、`calls`、`local_results`；只有 `explanation` 可省略。
- `expect` 与 `expect_error` 二选一。`expect_error` 只能使用测试 Schema 接受的运行阶段/错误码；资源语法、引用或类型编译错误通过 `validate` 单独验证。
- 数组顺序和局部结果键集需要完整匹配；不能只断言最终信号。嵌套调用的父级局部结果不包含子级内部调用结果。
- 测试输入的 `event` 包装与 `record --event` 所读的裸事件对象不同。只有需要回放时才读取[回放契约](../../../docs/replay.md)。

先从需求推导预期值，再运行 CLI。断言不一致时对照每条规则的贡献、分支优先级和执行路径，不能把 `actual` 整体复制成 `expect`。

## 可选的交付方式

只在用户需要相应产物时采用：

- **绑定证据的源码包**：读取[包契约](../../../docs/packages.md)，使用 `cdl build --input-schema "$POLICY_DIR/input-schema.yaml" --cases "$POLICY_DIR/behavior.yaml" --output "$POLICY_DIR/policy.core-package.json" --format json "$@"`。输出路径必须不存在；`build` 会重新测试，`verify --package ... --cases ...` 检查绑定并重新执行用例。
- **模块化创作目录**：读取[解析契约](../../../docs/resolution.md)。`resolve` 只解析、冻结并编译；随后 `import --bundle ... --cases ... --output ...` 重新测试并生成源码包。冻结 bundle 是 JSON 容器，不能把它当作 CDL 资源文件传给 `validate`。
- **声明的目标兼容性**：读取[公共契约](../../../docs/contracts/README.md)，使用 `check-target --input-schema ... --context ... --target ... --format json FILE...`。context 和 target 必须来自明确的业务/部署声明，不能编造 capability、revision 或身份来通过检查；该命令不连接目标，也不替代行为测试。
- **发布或服务激活**：这是独立工作流，读取[Core 服务契约](../../../docs/contracts/core-server.md)并遵守已有授权。编写任务不自动启动服务或操作部署。

这些产物与报告都不能代替业务效果评估或授予发布权限。
