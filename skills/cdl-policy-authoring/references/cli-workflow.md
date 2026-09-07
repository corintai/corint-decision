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

典型输出是 `rule.yaml`、`ruleset.yaml`、`pipeline.yaml`、`registry.yaml`、`input-schema.yaml` 和 `behavior.yaml`；多资源策略可使用多个明确命名的文件。
先从 `tests/conformance/generation/` 读取四种资源模板，以及 `tests/conformance/cdl_core/` 的输入 Schema 与 `tests/conformance/cdl_core/behavior.yaml` 的行为 fixture，再写入策略目录。
这些名称是示例，不要求重命名用户已有文件。

## 编译和行为执行

下面以四个资源文件为例。将参数列表扩展为实际完整资源闭包，不使用 `*.yaml` 混入 Schema、用例、备份或其他 Registry。
先确认 `POLICY_DIR` 存在，报告选择本次运行的文件名；保留需要审查的旧报告。

```sh
set -- "$POLICY_DIR/rule.yaml" "$POLICY_DIR/ruleset.yaml" \
  "$POLICY_DIR/pipeline.yaml" "$POLICY_DIR/registry.yaml"

cdl validate --input-schema "$POLICY_DIR/input-schema.yaml" \
  --format json "$@" > "$POLICY_DIR/validation.json"
```

记录退出码并读取 `validation.json`。仅在编译通过后执行：

```sh
cdl test --input-schema "$POLICY_DIR/input-schema.yaml" \
  --cases "$POLICY_DIR/behavior.yaml" --format json "$@" \
  > "$POLICY_DIR/behavior-report.json"
```

`--format json` 仅控制 stdout，构建信息在 stderr。不要用 stderr 是否为空来判断策略是否通过。
退出码 `0` 表示命令通过，`1` 表示校验或行为失败，`2` 表示用法/读写错误；Cargo 自身构建失败另行报告。`--help` 和 `--version` 的成功不是验证证据。

验证报告要求 `valid: true`；行为报告还要求 `execution_checked: true`，`test_results.executed == total`、`passed == total`、`failed == 0`，并核对各用例 `passed` 和 `trace_parity`。
预期输入/执行错误的诊断可能出现在通过的用例中，不能只按诊断是否非空判断失败。

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
