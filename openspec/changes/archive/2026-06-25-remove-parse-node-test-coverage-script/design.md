# 设计: remove-parse-node-test-coverage-script

> **变更**: remove-parse-node-test-coverage-script
> **日期**: 2026-06-24
> **基于**: proposal.md, specs/

---

## 架构组件

| 组件 | 变更类型 | 文件位置 | 职责 | 依赖 | 技术 |
|------|---------|----------|------|------|------|
| `FRAMEWORK_REGISTRY` node-test 条目 | **修改** | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | 简化 `coverage_cmd` 为直接运行 `node --test --experimental-test-coverage`；`coverage_output` 指向原始文本表格；`coverage_artifacts` 对应更新 | Node.js `--experimental-test-coverage` 运行时特性；`node-test` 测试框架自身；executor agent 作为消费者 | TypeScript |
| `unit-test-executor.md` 步骤 4 | **修改** | `plugins/dev-team/agents/unit-test-executor.md` | `node-test` 覆盖率解析改为 regex 直接提取 raw text table 的 `all files` 行，同 `go-cover` 模式 | `framework_registry` 输出的 `coverage_output` 路径和 `coverage_format: "node-test"`；Node.js 原始文本表格格式稳定性 | Markdown (Agent 指南) |
| `parse-node-test-coverage.mjs` | **删除** | `plugins/dev-team/scripts/parse-node-test-coverage.mjs` | 不再需要：解析逻辑迁入 executor agent | 无（已删除） | JavaScript (ESM) |
| `parse-node-test-coverage.test.mjs` | **删除** | `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` | 对应已删除的脚本 | `parse-node-test-coverage.mjs`（随依赖被删除） | JavaScript (ESM) |

### 不修改的组件

| 组件 | 理由 |
|------|------|
| 其他七框架注册表条目 (jest/vitest/vite-plus/bun/rust/go/pytest) | scope 仅限于 node-test |
| MCP 输出 schema (`test-get-framework-config.schema.ts`, `test-detect-frameworks.schema.ts`) | `coverage_format: "node-test"` 枚举值不变，schema 无需修改 |
| `config.schema.ts` | 框架名枚举不受影响 |
| `dev-team-mcp.cjs` | MCP 注册与分发不变 |
| `plugin.json` | 不涉及功能增减，无需升级版本号 |

### 组件图

```
Before:
  coverage_cmd → node --test --experimental-test-coverage
                  | tee coverage/node-test-output.txt
                  && node parse-node-test-coverage.mjs ...
                                                    |
                                                    v
                              parse-node-test-coverage.mjs (87 行)
                              reads:  coverage/node-test-output.txt  (raw text table)
                              writes: coverage/coverage-summary.json (istanbul JSON)
                                                    |
                                                    v
                              unit-test-executor.md step 4:
                                reads coverage-summary.json via JSON parse

After (方案B):
  coverage_cmd → node --test --experimental-test-coverage
                                                    |
                                                    v
                              coverage/node-test-output.txt (raw text table, stdout)
                              coverage_artifacts: [coverage/node-test-output.txt]
                                                    |
                                                    v
                              unit-test-executor.md step 4:
                                regex match "all files" row on raw text table
                                extract % Lines → lines, % Branch → branches, % Funcs → functions
```

---

## 数据流

### 流程描述

1. **框架配置查找**：`runTestGetFrameworkConfig({framework: "node-test"})` 返回简化后的 `FrameworkConfig`：
   - `coverage_cmd`: `"node --test --experimental-test-coverage"`（无 pipe/tee/parser）
   - `coverage_output`: `"coverage/node-test-output.txt"`（原始文本表格）
   - `coverage_artifacts`: `["coverage/node-test-output.txt"]`

2. **Plan 生成**：`test_detect_frameworks` 将注册表配置传播到 plan 条目，`generateScript` 以简化后的 `coverage_cmd` 组装 bash script。

3. **执行与产物移动**：Executor 步骤 2 运行 script，node:test 将覆盖率文本表格输出到 `coverage/node-test-output.txt`。步骤 3 将该文件移动到 `reports/coverage/node-test/`。

4. **覆盖率解析**：Executor 步骤 4 读取移动后的 `coverage/node-test-output.txt`，使用 regex 匹配 `all files` 行提取三个维度的覆盖率百分比。

5. **聚合与门控**：后续步骤不变，`coverage.measured` 结构不变（三个非 null `number` 值）。

### node-test 配置对比

| 字段 | 变更前 | 变更后 |
|------|--------|--------|
| `coverage_cmd` | `node --test --experimental-test-coverage 2>&1 \| tee coverage/node-test-output.txt && node plugins/dev-team/scripts/parse-node-test-coverage.mjs coverage/node-test-output.txt coverage/coverage-summary.json` | `node --test --experimental-test-coverage` |
| `coverage_format` | `node-test` （不变） | `node-test` |
| `coverage_output` | `coverage/coverage-summary.json` | `coverage/node-test-output.txt` |
| `coverage_artifacts` | `["coverage/coverage-summary.json"]` | `["coverage/node-test-output.txt"]` |
| `coverage_cleanup` | `["coverage"]` （不变） | `["coverage"]` |

### 数据模型

| 模型 | 字段 | 变更 |
|------|------|------|
| `FrameworkConfig`（node-test 条目） | `coverage_cmd`, `coverage_output`, `coverage_artifacts` | 三个字段值变更，类型不变 |
| `coverage/coverage-summary.json`（istanbul JSON） | 不再为 node-test 生成 | 产物类型从 JSON 变为原始文本文件 |
| `coverage/node-test-output.txt` | 原始文本表格 | 新增为 node-test coverage 产物 |

### 各框架解析模式对比

| coverage_format | 源文件 | 提取方式 | 与 go-cover 模式一致性 |
|----------------|--------|----------|----------------------|
| `go-cover` | `coverage/func-summary.txt` | regex 匹配 `total:` 行 → `lines`；`branches/functions` = null | 基线模式 |
| `node-test`（变更后） | `coverage/node-test-output.txt` | regex 匹配 `all files` 行 → `lines`/`branches`/`functions` | 同一模式：原始文本 + regex 提取 |
| `node-test`（变更前） | `coverage/coverage-summary.json` | JSON parse: `total.lines.pct`/`branches.pct`/`functions.pct` | 依赖外部 parser 脚本 |

---

## 路由/API 设计

无新增或修改的 MCP 工具。Schema 不变。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 采用方案B：移除独立解析脚本，由 executor agent 直接解析原始文本表格 | 与 `go-cover` 框架设计模式一致，提高框架配置的凝聚性，消除硬编码路径的脆弱性 | **方案A**：保留脚本但修复路径问题——引入了新的推理阶段脚本，增加维护负担 |
| D2 | `coverage_output` 指向原始文本文件 `coverage/node-test-output.txt` 而非解析后 JSON | 与 `go-cover` 的 `coverage/func-summary.txt` 模式一致；executor 负责解析无需中间格式转换 | 保留 JSON 路径——仍需要脚本转化 |
| D3 | `coverage_cleanup` 保留 `["coverage"]` 不变 | 清理范围不变：`node --test --experimental-test-coverage` 仍生成 `coverage/` 目录 | 改为空列表——但 `coverage/` 目录仍需清理 |
| D4 | 不修改 plugin.json 版本号 | 本变更不涉及功能增减，仅实现细节修改和删除脚本 | 更新版本号——违背 CLAUDE.md "upgrade after changing codes" 意图 |
| D5 | 不修改 MCP schema 文件 | `coverage_format: "node-test"` 枚举值未变；schema 只描述输出格式标识符，不关心解析方式 | 修改 schema——无需变更的额外噪音 |

---

## 依赖

### 运行时依赖

- Node.js `--experimental-test-coverage` — 输出原始文本表格格式（no change）
- `go-cover` 解析模式已在 executor agent 中实现，作为 `node-test` 新模式的参考

### 构建/测试依赖

- TypeScript / `pnpm run -C plugins/dev-team/bin check` — 修改后类型检查
- `test-get-framework-config.test.ts` — 更新 `node-test` 期望值
- `test-detect-frameworks.test.ts` — 更新 `node-test` plan 断言

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| regex 与 Node.js 版本输出格式不兼容 | node-test 覆盖率解析失败，返回 0 | 中 | 遵循 `go-cover` 已有模式；executor agent 文档中明确列出表格格式和 regex 示例；无法解析时设为 0 |
| 测试仍引用旧 coverage_cmd 字符串 | 测试失败 | 低 | 本设计已穷举所有引用文件；tasks.md 中列出全部待更新测试 |
| 开发者不清楚 node-test 覆盖率在 executor 中解析 | 配置时困惑 | 低 | executor agent 文档中 `node-test` 条目包含具体说明和 regex 示例 |

---

## 待决问题

- 无
