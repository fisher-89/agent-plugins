# 设计: simplify-test-report-schema

> **变更**: simplify-test-report-schema
> **日期**: 2026-06-18
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `FRAMEWORK_REGISTRY` | 为各测试框架配置 JSON-only 覆盖率命令与产物路径 | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | 无（硬编码注册表） | TypeScript |
| `unit-test-executor` | 执行测试、解析覆盖率、写入嵌套 `coverage` 对象与合并后的 `test_cases[]` | `plugins/dev-team/agents/unit-test-executor.md` | `test_detect_frameworks` MCP、`config_get` | Markdown（Agent 指南） |
| `unit-test-evaluator` | 校验新 schema、读取 `coverage.pass`、从失败 `test_cases` 驱动决策树 | `plugins/dev-team/agents/unit-test-evaluator.md` | `phase_log` MCP | Markdown（Agent 指南） |
| `unit-test-execution.json` | 06-unit-test 阶段的结构化执行报告（schema 简化后的单一契约） | `openspec/changes/<change>/reports/unit-test-execution.json` | Executor 写入、Evaluator 读取 | JSON |

### 组件图

```
test_detect_frameworks (MCP)
  |
  |-- plan[].coverage_cmd        <-- JSON-only reporter 参数（来自 FRAMEWORK_REGISTRY）
  |-- plan[].coverage_artifacts <-- 仅 JSON 摘要路径
  |
  v
unit-test-executor (Agent)
  1. 框架检测 & 执行 plan script（不变）
  2. 移动 JSON 摘要到 reports/coverage/<framework>/（不再移动 HTML）
  3. 解析覆盖率 → coverage.by_framework[].measured
  4. 加权平均 + overrides → coverage.measured / coverage.pass
  5. 提取 test_cases[]，失败条目附加 error 字段（无 failures[]）
  6. 写入 unit-test-execution.json
       |-- coverage: CoverageObject | null
       |-- test_cases: TestCase[]
       |-- integration_test.test_cases（同上，无 failures[]）
  |
  v
unit-test-evaluator (Agent)
  Step 1: 校验 coverage 为 object|null，test_cases 含失败详情
  Step 3: coverage.pass === true 或 coverage === null → 通过
  Step 4: test_cases.filter(c => c.status === "failed") → 决策树
  Step 5: findings 引用 coverage.measured / coverage.by_framework（无 HTML 路径）
  |
  v
phase_log → eval.json
```

---

## 数据流

### 流程描述

1. **框架配置下发**：`runTestGetFrameworkConfig` 从 `FRAMEWORK_REGISTRY` 返回各框架的 `coverage_cmd`（含 JSON reporter 参数）和收窄后的 `coverage_artifacts`（仅 `coverage/coverage-summary.json`）。`test_detect_frameworks` 将上述配置注入 plan 条目，Executor 不直接调用 `test_get_framework_config`。

2. **覆盖率执行与产物移动**：Executor 在各 `directory` 下执行 `script`（内含 JSON-only coverage 命令）。成功后仅移动 `coverage_artifacts` 中声明的 JSON 摘要文件到 `reports/coverage/<framework>/coverage-summary.json`，随后按 `coverage_cleanup` 清理临时 `coverage/` 等。不再复制、记录或引用 `index.html` 等 HTML 报告。

3. **覆盖率聚合写入**：Executor 从 JSON 摘要解析各框架 `measured`，按源文件数加权平均写入 `coverage.measured`；从 `config.json` 读取阈值与 overrides 写入 `coverage.thresholds` / `coverage.overrides`；ALL 判定结果写入 `coverage.pass`。若所有框架覆盖率均未生成，顶层 `"coverage": null`（不含 `pass: false` 的空对象）。

4. **失败详情合并**：测试输出解析时，对 `status: "failed"` 的 `test_cases[]` 条目直接附加 `line`、`error_type`、`error_message`、`stack_trace`（及可选 `design_ref`）。顶层与 `integration_test` 均不再写入 `failures[]`。

5. **Evaluator 消费**：Evaluator 读取报告后，U1 校验新 schema；U3 使用 `coverage.pass` 或 `coverage === null`；决策树遍历 `test_cases.filter(c => c.status === "failed")` 读取 `error_type`、`file`、`line`；findings 模板使用 `coverage.measured` 与 `coverage.by_framework[].measured`。

### 数据模型

#### 顶层报告字段

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `UnitTestReport` | `phase`, `command`, `timestamp`, `total`, `passed`, `failed`, `skipped`, `duration_seconds`, `test_cases`, `coverage`, `integration_test`, `findings?` | 1 份报告对应 1 次 06-unit-test 执行 | `reports/unit-test-execution.json` |

#### `TestCase`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | yes | 用例名称 |
| `file` | string | yes | 测试文件路径 |
| `duration_ms` | number | yes | 执行时长（毫秒） |
| `status` | `"passed"` \| `"failed"` \| `"skipped"` | yes | 用例状态 |
| `line` | number | when failed | 失败行号 |
| `error_type` | string | when failed | 错误类型 |
| `error_message` | string | when failed | 错误消息 |
| `stack_trace` | string | when failed | 堆栈跟踪 |
| `design_ref` | string | optional | test-design 引用 |

#### `CoverageObject`（`coverage` 非 null 时）

| 字段 | 类型 | 说明 |
|------|------|------|
| `pass` | boolean | 全局 + 所有 overrides ALL 达标 |
| `measured` | `{lines, branches, functions}` | 加权平均测量值 |
| `thresholds` | `{lines, branches, functions}` | 门禁阈值 |
| `by_framework` | `{framework, measured}[]` | 各框架详情（无 `html_report`） |
| `overrides` | `{glob, thresholds, measured, pass}[]` | overrides 校验结果 |

#### 旧字段 → 新路径映射

| 旧字段 | 新路径 / 处置 |
|--------|---------------|
| `coverage: {lines, branches, functions}` | `coverage.measured` |
| `coverage_thresholds` | `coverage.thresholds` |
| `coverage_pass` | `coverage.pass` |
| `coverage_by_framework` | `coverage.by_framework`（条目内 `coverage` → `measured`） |
| `coverage_overrides` | `coverage.overrides`（条目内 `coverage` → `measured`） |
| `html_reports` | 删除 |
| `coverage_by_framework[].html_report` | 删除 |
| `failures[]` | `test_cases[]` where `status === "failed"` |
| `integration_test.failures[]` | `integration_test.test_cases[]` 失败条目 |

#### 统一覆盖率产物结构

```
openspec/changes/<change>/
  reports/
    unit-test-execution.json
    coverage/
      <framework>/
        coverage-summary.json    # 唯一保留的覆盖率产物
```

---

## 路由/API 设计

本变更不涉及 HTTP API。以下为 MCP 工具输出变更：

### MCP 工具: `test_get_framework_config`（注册表值变更）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_get_framework_config` | 返回框架命令配置（`coverage_cmd` 改为 JSON-only；`coverage_artifacts` 收窄） | `{ framework: string }` | `{ framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup }` | 无（本地 MCP） |

### 各框架 `coverage_cmd` / `coverage_artifacts` 目标值

| 框架 | `coverage_cmd` | `coverage_artifacts` |
|------|----------------|---------------------|
| jest | `npx jest --coverage --coverageReporters=json-summary` | `["coverage/coverage-summary.json"]` |
| vitest | `npx vitest run --coverage --coverage.reporter=json-summary` | `["coverage/coverage-summary.json"]` |
| vite-plus | `vp test --coverage --coverage.reporter=json-summary` | `["coverage/coverage-summary.json"]` |
| bun | `bun test --coverage --coverageReporters=json-summary` | `["coverage/coverage-summary.json"]` |
| rust | `cargo llvm-cov --json` | `["coverage/coverage-summary.json"]` |

`coverage_cleanup` 保持不变（仍清理整个 `coverage/` 及框架特定临时目录取）。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 6 个扁平 coverage 字段合并为单一嵌套 `coverage` 对象 | 消除 `coverage` 同名歧义（测量值 vs 按框架测量值）；配置（thresholds）与判定（pass）与测量（measured）语义分组清晰 | **保留扁平字段**：改动面小，但命名冲突与文档维护成本高，已拒绝 |
| D2 | 删除 `failures[]`，错误详情并入 `test_cases[]` 失败条目 | 消除 `name`/`file` 重复；Evaluator 只需遍历单一数组 | **保留 failures 作为索引**：仍冗余，且 Executor 需维护两份数据一致性，已拒绝 |
| D3 | 覆盖率产物仅保留 JSON 摘要，删除 HTML 相关字段 | HTML 对自动化链路无用；缩小产物移动范围，简化 Executor 步骤 | **继续生成 HTML 但不写入报告**：仍产生无用文件与移动开销，已拒绝 |
| D4 | 一次性切换，不保留 Evaluator 双读兼容 | 变更范围限于 agent 文档与注册表；双读增加长期维护负担 | **Evaluator 同时支持新旧 schema**：复杂度高且延迟清理，已拒绝 |
| D5 | `command` 保持单字符串，多命令数组留待后续 | 与本次 schema 简化正交；避免扩大变更范围 | **同步改为命令数组**：需改 Executor/Evaluator 及更多 spec，超出本次 scope，已拒绝 |
| D6 | rust 使用 `cargo llvm-cov --json` 替代 `--all --coverage` | llvm-cov 原生 JSON 输出即机器可解析格式，与 JSON-only 策略一致 | **保留 HTML/text 输出再转换**：多一步解析且产物仍含 HTML，已拒绝 |

---

## 依赖

### 运行时依赖

- `test_detect_frameworks` MCP — 提供含 JSON-only `coverage_cmd` 的 plan 条目
- `config_get` MCP — Executor 读取 `test.coverage.thresholds` 与 `test.coverage.overrides`
- `phase_log` MCP — Evaluator 写入 eval 结果

### 构建/测试依赖

- `plugins/dev-team/bin` — TypeScript 编译与单元测试（`test-get-framework-config.test.ts`）
- 各框架 JSON reporter — vitest/jest `json-summary`、bun `json-summary`、cargo `llvm-cov --json`

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 进行中的变更仍用旧 schema 写报告 | Evaluator U1 失败 | 中 | 一次性切换；实施完成后 grep 确认无旧字段引用 |
| Evaluator 遗漏旧字段引用 | 误判或 findings 缺失 | 低 | AC-5/AC-8 验收；tasks 阶段全文搜索 `coverage_pass`、`html_reports`、`failures` |
| `integration_test` 与顶层 test_cases 结构不一致 | 集成失败诊断遗漏 | 低 | Executor 文档中 integration_test 示例与顶层对齐 |
| 嵌套 `coverage` 与旧顶层 `coverage` 同名混淆 | 实施者误用旧语义 | 中 | design 提供字段映射表；Executor 示例完整展示新结构 |
| rust `llvm-cov --json` 输出路径与 istanbul 不同 | 解析失败 | 低 | 保持 `coverage_output: coverage/coverage-summary.json`；Executor 按 `coverage_format` 分支解析；移动步骤仅处理 JSON 摘要 |

---

## 迁移步骤

1. 更新 `FRAMEWORK_REGISTRY`：`coverage_cmd` 加 JSON reporter 参数；`coverage_artifacts` 收窄为 JSON 摘要路径。
2. 更新 `test-get-framework-config.test.ts` 中 `EXPECTED_CONFIGS` 与相关断言。
3. 更新 `unit-test-executor.md`：Process 步骤 2/3/5/6/7 移除 HTML 引用；报告 JSON 模板改为嵌套 `coverage`；失败详情写入 `test_cases`；no-coverage 示例仅 `"coverage": null`。
4. 更新 `unit-test-evaluator.md`：U1/U3、Step 1/3/4/5 字段路径与 findings 模板。
5. 运行 `pnpm run -C ./plugins/dev-team/bin check` 与单元测试。
6. 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号。
7. 归档变更时同步更新 `openspec/specs/test-execution-diagnostics/spec.md` 与 `openspec/specs/phase-agents/spec.md`（不在本次 tasks 实施范围内，由 archive 流程处理）。

---

## 待决问题

- 无。proposal 已明确 `command` 字段、integration-test 独立 agent、检测逻辑与加权算法均不在本次变更范围。
