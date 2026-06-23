# 设计: add-node-go-pytest-frameworks

> **变更**: add-node-go-pytest-frameworks
> **日期**: 2026-06-23
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `testFrameworkSchema` | 扩展有效框架名枚举，接受 `node-test`、`go`、`pytest` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | `zod/v4` | TypeScript / Zod |
| `FRAMEWORK_REGISTRY` | 硬编码八框架命令配置（含新三条的 `test_cmd`、`coverage_cmd`、`coverage_format`、`default_glob` 等） | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | `TestFrameworks` 类型 | TypeScript |
| `parse-node-test-coverage.mjs` | 将 node:test `--experimental-test-coverage` stdout 文本表格转为 istanbul 兼容 `coverage-summary.json` | `plugins/dev-team/scripts/parse-node-test-coverage.mjs` | Node.js `fs` | JavaScript (ESM) |
| MCP 输出 Schema | 扩展 `coverage_format` 枚举为五值；`testFrameworkSchema` 自动继承新框架名 | `test-get-framework-config.schema.ts`、`test-detect-frameworks.schema.ts` | `testFrameworkSchema` | TypeScript / Zod |
| `PlanEntry` / plan 生成 | 从注册表携带新框架配置生成 `plan` 条目（含两步 `coverage_cmd`） | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `runTestGetFrameworkConfig`、`generateScript` | TypeScript |
| `unit-test-executor` | 新增三种框架测试输出解析、三种覆盖率格式解析路径、nullable 维度加权平均与门控 | `plugins/dev-team/agents/unit-test-executor.md` | MCP `test_detect_frameworks` | Markdown (Agent 指南) |
| `unit-test-evaluator` | 接受 nullable `measured` 维度；findings 标注 "N/A (框架不支持)" | `plugins/dev-team/agents/unit-test-evaluator.md` | 执行报告 JSON | Markdown (Agent 指南) |
| `detect_test_framework` | 检测 Go 项目（`go.mod`）并返回 `("go", "go test ./...")` | `plugins/dev-team/utils/test-framework.py` | 文件系统扫描 | Python |
| `run_tests` | 新增 `go`、`node-test` 运行器及输出解析 | `plugins/dev-team/utils/test-runner.py` | `test-framework.py` | Python |

### 组件图

```
openspec/config.json
  |  test.frameworks: "go" | "node-test" | "pytest" | [{glob, framework}]
  v
testFrameworkSchema (Zod) ──► config 验证通过
  |
  v
test_detect_frameworks (MCP)
  |-- matchGlob / scanProjectFiles
  |-- runTestGetFrameworkConfig() ──► FRAMEWORK_REGISTRY (8 entries)
  |-- generateScript() ──► bash script (cd, rm -rf, coverage_cmd)
  v
plan[]: { directory, framework, coverage_cmd, coverage_format,
          coverage_output, coverage_artifacts, coverage_cleanup, script }
  |
  v
unit-test-executor (Agent)
  |-- Step 2: 执行 script，解析测试输出 (node-test / go / pytest 规则)
  |-- Step 3: 移动 coverage_artifacts → reports/coverage/<framework>/
  |-- Step 4: 按 coverage_format 解析 measured (含 null 维度)
  |     |-- istanbul / node-test → coverage-summary.json
  |     |-- llvm-cov → JSON totals
  |     |-- go-cover → func-summary.txt (lines only; branches/functions = null)
  |     |-- coverage-py → coverage.json (lines + branches; functions = null)
  |-- Step 5: 加权平均 (null 不参与) + coverage.pass (null 跳过比较)
  v
reports/unit-test-execution.json
  |
  v
unit-test-evaluator (Agent)
  |-- U1/U3: 接受 measured.branches/functions 为 null
  |-- findings: null 维度标注 "N/A (框架不支持)"
```

---

## 数据流

### 流程描述

1. **配置验证**：用户在 `openspec/config.json` 中声明 `test.frameworks` 为 `"node-test"`、`"go"` 或 `"pytest"`（或含这些框架的 glob 映射数组）。`testFrameworkSchema` 验证通过后，MCP 工具可识别这些框架名。

2. **框架检测与 plan 生成**：`test_detect_frameworks` 读取 config，归一化字符串框架名为 `{glob, framework}` 条目（glob 来自注册表 `default_glob`），调用 `runTestGetFrameworkConfig` 填充 plan。对于 `node-test`，`coverage_cmd` 为两步 shell 命令：先 `tee` stdout 到中间文件，再调用 `parse-node-test-coverage.mjs` 生成 JSON 摘要。

3. **测试与覆盖率执行**：Executor 对每个 plan 条目运行 `script`（bash，`set -e`）。脚本在工作目录下清理历史产物、执行 `coverage_cmd`（覆盖率命令已包含测试运行）。从 stdout 解析各框架的用例结果。

4. **产物移动**：成功执行的框架按 `coverage_artifacts` 将 JSON/文本摘要复制到 `openspec/changes/<change>/reports/coverage/<framework>/`，更新解析路径，按 `coverage_cleanup` 清理工作目录临时文件。`node-test` 的中间 stdout 文件（`coverage/node-test-output.txt`）列入 `coverage_cleanup`，不进入 `coverage_artifacts`。

5. **覆盖率解析**：按 `coverage_format` 读取统一位置的产物，提取三维度 measured 值。不支持的原生维度设为 `null`（非 `0`）。解析失败时该框架三维度均设为 `0` 并记录 findings。

6. **聚合与门控**：多框架时按源文件数加权平均，null 维度不参与分子分母。`coverage.pass` 对非 null 维度做 ALL 比较，null 维度视为自动通过。overrides 分组同样跳过 null 维度。

7. **评估**：Evaluator 读取报告，U3 以 `coverage.pass` 为准；findings 中对 null 维度标注 "N/A (框架不支持)"。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FrameworkConfig` | `framework`, `test_cmd`, `coverage_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`, `default_glob` | 注册表 8 条目，每框架一条 | 硬编码于 `test-get-framework-config.ts` |
| `CoverageMeasured` | `{ lines: number, branches: number \| null, functions: number \| null }` | 用于 `coverage.measured`、`by_framework[].measured`、`overrides[].measured` | 写入 `unit-test-execution.json` |
| `PlanEntry` | 同现有 plan 字段；`coverage_format` 扩展为五值枚举 | 委托自 `FrameworkConfig` | 每次 MCP 调用实时生成 |
| node:test 中间产物 | `coverage/node-test-output.txt`（stdout tee） | Step 1 输出 → parser 输入 | 工作目录临时文件，cleanup 删除 |
| node:test 最终产物 | `coverage/coverage-summary.json`（istanbul 兼容） | parser 输出 → 移动至统一目录 | `reports/coverage/node-test/coverage-summary.json` |

### 各框架注册表配置（新增三条）

| 框架 | `test_cmd` | `coverage_format` | `coverage_output` | `coverage_artifacts` | `coverage_cleanup` | `default_glob` |
|------|------------|-------------------|-------------------|---------------------|-------------------|----------------|
| node-test | `node --test` | `node-test` | `coverage/coverage-summary.json` | `["coverage/coverage-summary.json"]` | `["coverage"]` | `**/*.test.{mjs,js,cjs}` |
| go | `go test ./...` | `go-cover` | `coverage/func-summary.txt` | `["coverage/func-summary.txt"]` | `["coverage", "coverage.out"]` | `**/*_test.go` |
| pytest | `pytest -v` | `coverage-py` | `coverage.json` | `["coverage.json"]` | `[".coverage", "htmlcov"]` | `**/test_*.py` |

### Nullable 维度映射

| `coverage_format` | `lines` | `branches` | `functions` |
|-------------------|---------|------------|-------------|
| istanbul | number | number | number |
| llvm-cov | number | number | number |
| node-test | number | number | number |
| go-cover | number（从 `total:` 行） | null | null |
| coverage-py | number（`totals.percent_covered`） | number（`totals.percent_covered_branches`，若存在） | null |

---

## 路由/API 设计

### MCP 工具: `test_get_framework_config`

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_get_framework_config` | 查询框架命令配置 | `{ framework: TestFrameworks, project_root?: string }` | `{ framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts?, coverage_cleanup? }`；`coverage_format` 为 `'istanbul' \| 'llvm-cov' \| 'node-test' \| 'go-cover' \| 'coverage-py'` | 无（本地 MCP） |

未知框架名返回错误，错误信息列出全部八框架。

### MCP 工具: `test_detect_frameworks`

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_detect_frameworks` | 检测文件框架并生成执行 plan | `{ files?: string[], project_root?: string }` | `{ detected, frameworks, plan[] }`；plan 条目 `coverage_format` 同上五值枚举 | 无（本地 MCP） |

`node-test` plan 的 `coverage_cmd` 示例：

```bash
node --test --experimental-test-coverage 2>&1 | tee coverage/node-test-output.txt && node plugins/dev-team/scripts/parse-node-test-coverage.mjs coverage/node-test-output.txt coverage/coverage-summary.json
```

解析器路径使用相对于**项目根目录**的 `plugins/dev-team/scripts/...`，与插件内其他脚本引用惯例一致（实现阶段若 MCP roots 可解析插件根目录，可改为更稳健的路径解析，见待决问题）。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | node:test 覆盖率采用**两步脚本**（native coverage + `parse-node-test-coverage.mjs`） | Node 无 JSON reporter；实验性 API 的 stdout 文本表格可稳定解析为 istanbul 兼容 JSON，复用现有 istanbul 读取路径 | **A**：仅报告 lines，忽略 branches/functions——信息不完整；**B**：引入 c8/istanbul 第三方——增加依赖，偏离内置测试栈 |
| D2 | 缺失覆盖率维度使用 **`null`**（非 `0`、非省略字段） | 语义清晰：`null` = 框架不支持，与真实 0% 覆盖率区分；门控可明确"跳过比较" | **A**：省略字段——schema 与 TypeScript 类型不一致；**B**：`-1` 哨兵值——非自解释，易与合法百分比混淆 |
| D3 | 框架命名 **`node-test`**（连字符，与 `vite-plus` 一致） | 与现有注册表命名风格统一 | `nodetest`、`node_test` |
| D4 | Go 覆盖率格式 **`go-cover`** + 文本 `func-summary.txt` | 使用 Go 原生 `go test -coverprofile` + `go tool cover -func`，无 JSON summary | cargo-llvm-cov 等 Rust 工具链方案不适用于 Go |
| D5 | pytest 覆盖率格式 **`coverage-py`** + `coverage.json` | pytest-cov 标准 JSON 输出，与 istanbul 路径分离便于解析规则独立 | XML/HTML reporter——已废弃 HTML-only 路径，XML 需额外解析 |
| D6 | null 维度**不参与加权平均、不参与阈值比较** | 避免 Go-only 项目因 branches/functions 缺失而误报 fail；多框架混合时分母仅含提供该维度的框架 | 将 null 视为 0 参与计算——会拉低 overall 并误触发门控 |
| D7 | node-test **`coverage_artifacts` 仅含最终 JSON**，中间 stdout 列入 **`coverage_cleanup`** | 统一目录只需 istanbul 兼容摘要；中间文件为解析临时产物 | 同时移动 stdout 文件——增加噪音，Evaluator 不使用 |
| D8 | 覆盖率命令失败时该框架 **measured 三维度均为 0**（非 null） | 与现有五框架行为一致，表示"执行了但失败/无数据"；null 保留给"框架不支持该维度" | 失败时也设为 null——与"不支持"语义混淆 |
| D9 | **`parse-node-test-coverage.mjs` 置于 `plugins/dev-team/scripts/`** | 与插件代码同仓、可被 `coverage_cmd` shell 直接引用；独立脚本便于 fixture 验证与 API 变更隔离 | 内联于 TypeScript MCP 命令——bash 中难以调用；嵌入 Agent 提示——不可脚本化、不可单测 |

---

## 依赖

### 运行时依赖

- **Node.js**（含 `--experimental-test-coverage`）— node:test 覆盖率 Step 1
- **Go toolchain**（`go test`、`go tool cover`）— Go 项目测试与覆盖率
- **pytest + pytest-cov** — Python 覆盖率 JSON 输出（缺失时 Executor 捕获失败、记录 findings，不阻塞报告）
- **zod/v4** — config 与 MCP 输出 schema
- **现有 `matchGlob` / `scanProjectFiles`** — 框架检测逻辑不变

### 构建/测试依赖

- **TypeScript / `pnpm run -C plugins/dev-team/bin check`** — 类型与 schema 一致性
- **plugin.json 版本号** — 代码变更后按 CLAUDE.md 规则升级

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `--experimental-test-coverage` 输出格式变更 | node-test 解析失败 | 中 | 独立 parser + fixture；失败时 measured 设为 0 并记录 findings |
| Go `go tool cover -func` 版本差异 | lines 提取失败 | 低 | 匹配 `total:` 行；失败时 lines=0 |
| pytest-cov 未安装 | 覆盖率命令非零退出 | 中 | Executor 容错；findings 注明依赖 |
| null 与 ALL 门控逻辑冲突 | 误报 pass/fail | 低 | 明确规则写入 executor/evaluator；spec 场景覆盖 Go-only / 混合项目 |
| 多框架加权分母变化 | overall 数值偏差 | 低 | 仅非 null 框架参与该维度；全 null 时 overall 也为 null |
| Windows 下 bash `tee` / 管道 | node-test 两步脚本失败 | 中 | Executor 在 Git Bash/WSL 环境执行；失败走 findings + measured=0 |

---

## 待决问题

- **`parse-node-test-coverage.mjs` 路径解析**：当前设计使用项目根相对路径 `plugins/dev-team/scripts/...`；若项目在 monorepo 子目录且无该路径，需在实现阶段确认是否通过 MCP `roots/list` 解析插件安装根目录（参考 `use-mcp-roots-list` 变更惯例）。
- **node:test stdout 表格格式样本**：实现 parser 前需收集 Node 20+ 实际 `--experimental-test-coverage` 输出 fixture，确认 lines/branches/functions 列名与分隔符。
- **Python `test-framework.py` 检测顺序**：Go 检测（`go.mod`）应置于 Node `package.json` 之前还是之后——当前提案要求独立 Go 仓库优先检测 `go.mod`；含 `go.mod` 且无 `package.json` 的纯 Go 项目为典型场景。
