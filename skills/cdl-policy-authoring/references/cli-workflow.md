# CLI 操作流程

以下命令使用已确认的 CDL 仓库和用户选择的策略目录。先替换示例绝对路径；不要改写仓库中的公共 fixture。命令直接在终端执行，不另建 `verify.sh` 等脚本；默认只交付必要的 CDL 资源文件。
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
cdl validate --format json "$POLICY_DIR/features/payment.yaml"
```

可以直接传入多个文件、目录，或混合路径。目录会递归扫描所有层级的 `.yaml`、`.yml`、`.json` 文件，不要求特定目录布局；重复路径只加载一次：

```sh
cdl validate "$POLICY_DIR" --format json
cdl validate "$POLICY_DIR/rules/blocked.yaml" "$POLICY_DIR/features" --format json
```

默认只校验选择范围，import 声明只检查语法，不跟随加载。需要额外解析 imports 并验证引用时使用 `--root DIR PATH...`；路径相对 root，集合需要包含所有引用。
`--root DIR` 不传路径时保留旧的仓库扫描方式，仅扫描标准资源目录和根 Registry。
已有输入 Schema 时额外加 `--input-schema /absolute/path/to/existing-input-schema.yaml`；Schema 路径始终相对当前目录解析，不相对 root。没有时省略该参数，不为完成默认校验新建 Schema。

目录扫描会识别输入 Schema、行为用例及已知验证/分析报告，并在 `skipped_sources` 列出跳过项；这些文件未作为 CDL 校验。未知文档结构或 YAML 解析失败仍会报错；不能仅靠目录名忽略疑似 CDL。显式指定的辅助文件会报告 `E_NOT_CDL`。扫描到 Schema 不自动开启字段检查，仍需 `--input-schema`。

默认直接读取 stdout，不保存 JSON 报告或日志。只有用户明确要求保存报告时才写入指定位置，并使其位于扫描目录之外，避免 shell 先创建的空文件被读到。

读取报告和退出码。`valid: true` 且退出码 `0` 才是通过；`1` 是校验失败，`2` 是用法/文件错误。`--format json` 只控制程序 stdout；Cargo 构建信息可能在 stderr，构建失败另行报告。`--help`/`--version` 成功不是验证证据。

按诊断的 `source`、`field_path`、`stage`、`code` 修复，再次执行。在回复中说明 `references_checked`、`input_schema_checked` 与 `unchecked`，不另建说明文件，不把 `execution_checked: false` 当作静态校验失败。
完整命令契约见[CLI 文档](../../../docs/cli.md)。

## 按需执行 Core 编译和行为测试

本节只适用于用户明确要求或此前已明确授权的额外验证，不因编写或修改策略而自动执行。Core 编译需要完整 Core 闭包与输入 Schema，行为测试另需独立预期用例。下面的 `INPUT_SCHEMA` 和 `CASES_FILE` 指向用户提供或另行明确要求编写的文件；不默认在策略目录新建 Schema、用例或报告。资源列表必须符合严格 Core，不能包含 Feature/List/Service。

```sh
set -- "$POLICY_DIR/rule.yaml" "$POLICY_DIR/ruleset.yaml" \
  "$POLICY_DIR/pipeline.yaml" "$POLICY_DIR/registry.yaml"

cdl validate --profile cdl-core-risk-draft-1 \
  --input-schema "$INPUT_SCHEMA" --format json "$@"

cdl test --input-schema "$INPUT_SCHEMA" \
  --cases "$CASES_FILE" --format json "$@"
```

仅在 Core 编译通过后执行行为测试。行为报告要求 `valid: true`、`execution_checked: true`、`test_results.executed == total`、`passed == total`、`failed == 0`，并检查各用例 `passed` 和 `trace_parity`。
预期运行错误可能出现在通过的用例中，不能仅按诊断是否为空判断行为失败。

## 明确要求编写用例时的区别

以仓库 [testing.md](../../../docs/testing.md) 和 [test-suite.json](../../../docs/contracts/schema/test-suite.json) 为准：

- 行为文件有自己的版本与 Profile，不使用资源的语言版本作为测试格式版本。
- 用例输入为 `input: {event: {...}}`。成功断言包含 `pipeline_id`、`score`、`signal`、`actions`、`triggered_rules`、`steps`、`calls`、`local_results`；只有 `explanation` 可省略。
- `expect` 与 `expect_error` 二选一。`expect_error` 只能使用测试 Schema 接受的运行阶段/错误码；资源语法、引用或类型编译错误通过 `validate` 单独验证。
- 数组顺序和局部结果键集需要完整匹配；不能只断言最终信号。嵌套调用的父级局部结果不包含子级内部调用结果。
- 测试输入的 `event` 包装与 `record --event` 所读的裸事件对象不同。只有需要回放时才读取[回放契约](../../../docs/replay.md)。

先从需求推导预期值，再运行 CLI。断言不一致时对照每条规则的贡献、分支优先级和执行路径，不能把 `actual` 整体复制成 `expect`。

## 可选的交付方式

只在用户明确要求或此前已明确授权相应产物时采用：

- **绑定证据的源码包**：读取[包契约](../../../docs/packages.md)，使用 `cdl build --input-schema "$INPUT_SCHEMA" --cases "$CASES_FILE" --output "$PACKAGE_OUTPUT" --format json "$@"`。`PACKAGE_OUTPUT` 为这项额外交付选定的位置，输出路径必须不存在；`build` 会重新测试，`verify --package ... --cases ...` 检查绑定并重新执行用例。
- **模块化创作目录**：读取[解析契约](../../../docs/resolution.md)。`resolve` 只解析、冻结并编译；随后 `import --bundle ... --cases ... --output ...` 重新测试并生成源码包。冻结 bundle 是 JSON 容器，不能把它当作 CDL 资源文件传给 `validate`。
- **声明的目标兼容性**：读取[公共契约](../../../docs/contracts/README.md)，使用 `check-target --input-schema ... --context ... --target ... --format json FILE...`。context 和 target 必须来自明确的业务/部署声明，不能编造 capability、revision 或身份来通过检查；该命令不连接目标，也不替代行为测试。
- **发布或服务激活**：这是独立工作流，读取[Core 服务契约](../../../docs/contracts/core-server.md)并遵守已有授权。编写任务不自动启动服务或操作部署。

这些产物与报告都不能代替业务效果评估或授予发布权限。
