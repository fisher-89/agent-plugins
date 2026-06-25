# 提案: 移除 node-test 覆盖率解析脚本

> **变更**: remove-parse-node-test-coverage-script
> **日期**: 2026-06-24
> **状态**: 草案

---

## 问题

插件中有一个独立的 Node.js 脚本 `plugins/dev-team/scripts/parse-node-test-coverage.mjs`（约 87 行），用于将 `node:test --experimental-test-coverage` 的 stdout 文本表格解析为 Istanbul 兼容的 `coverage-summary.json`。

该脚本作为 `node-test` 框架的 `coverage_cmd` 的一部分被硬编码在 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`（第 77 行）：

```
coverage_cmd: 'node --test --experimental-test-coverage 2>&1 | tee coverage/node-test-output.txt && node plugins/dev-team/scripts/parse-node-test-coverage.mjs coverage/node-test-output.txt coverage/coverage-summary.json'
```

这种设计存在两个问题：
1. 硬编码的相对路径 `plugins/dev-team/scripts/parse-node-test-coverage.mjs` 是脆弱的，在不同的工作目录上下文中容易失效
2. 框架配置与一个外部解析脚本耦合，破坏了框架配置的凝聚性（cohesion）——框架配置应该只描述如何运行测试，而不应该包含数据格式转换逻辑

---

## 提案

采用与 `go-cover` 框架相同的设计模式（方案 B）：将 `coverage_cmd` 简化为只运行测试命令，让执行器（agent）在步骤 4（Coverage parsing）中直接解析原始的文本表格输出。

### 具体变更

1. **简化 `coverage_cmd`**：将 `node-test` 的 `coverage_cmd` 改为 `node --test --experimental-test-coverage`，去除 pipe、`tee` 和外部解析脚本调用
2. **更改 `coverage_output`**：改为 `coverage/node-test-output.txt`（原始文本表格，而非解析后的 JSON）
3. **更改 `coverage_artifacts`**：改为 `['coverage/node-test-output.txt']`（与 `go-cover` 的 `coverage/func-summary.txt` 模式一致）
4. **更新执行器 agent**：在 `unit-test-executor.md` 步骤 4 中更新 `node-test` 条目，说明如何直接解析原始文本表格（regex 提取 `all files` 行），模式类似于 `go-cover` 读取 `func-summary.txt` 并提取 `total:` 行
5. **删除脚本及其测试**：删除 `parse-node-test-coverage.mjs` 和 `parse-node-test-coverage.test.mjs`
6. **更新测试**：所有引用旧 `coverage_cmd` 字符串的测试用例

### 关键约束

- 不新建 MCP 工具——仅简化框架配置中的 `coverage_cmd` 并更新执行器 agent 的解析说明
- 遵循 `go-cover` 模式：原始文本文件作为 `coverage_output`，LLM/Agent 在步骤 4 中直接解析
- 保持所有现有功能——覆盖率百分比（lines、branches、functions）必须仍然可提取

---

## 能力

### 修改的能力

- **test-get-framework-config** — 修改 `node-test` 框架的 `coverage_cmd`、`coverage_output` 和 `coverage_artifacts` 字段，移除对外部解析脚本的依赖
- **unit-test-executor** — 更新步骤 4（Coverage parsing）中 `node-test` 的覆盖率解析方式，从读取解析器生成的 Istanbul JSON 改为直接解析原始文本表格（与 `go-cover` 模式一致）

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` — 简化 `node-test` 框架的 `coverage_cmd`、更改 `coverage_output` 和 `coverage_artifacts`
- `plugins/dev-team/agents/unit-test-executor.md` — 更新步骤 4 中 `node-test` 覆盖率解析说明，改为直接解析原始文本表格
- `plugins/dev-team/scripts/parse-node-test-coverage.mjs` — **删除**（不再需要）

### 测试文件

- `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs` — **删除**（对应已删除的脚本）
- `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` — 更新 `node-test` 框架的期望值，移除对 `tee` 和 `parse-node-test-coverage.mjs` 的断言
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` — 更新 `node-test` plan 的期望值，移除对 `parse-node-test-coverage.mjs` 的断言

### 不要修改

- 其他测试框架的配置条目（jest、vitest、vite-plus、bun、rust、go、pytest）
- MCP 工具的 schema 定义文件（`test-get-framework-config.schema.ts`、`test-detect-frameworks.schema.ts`）
- `config.schema.ts` 及 `config.schema.test.ts`
- 其他 schema test 文件（`test-get-framework-config.schema.test.ts`、`test-detect-frameworks.schema.test.ts`）
- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` 及相关测试
- `plugins/dev-team\.claude-plugin/plugin.json`（不涉及功能增减，无需升级版本号——仅修改实现细节和删除脚本）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | node-test coverage_cmd 简化 | `coverage_cmd` 为 `node --test --experimental-test-coverage`，不含 `tee`、`parse-node-test-coverage.mjs` 或管道符 `|` |
| AC-2 | node-test coverage_output 更改 | `coverage_output` 为 `coverage/node-test-output.txt` |
| AC-3 | node-test coverage_artifacts 更改 | `coverage_artifacts` 为 `['coverage/node-test-output.txt']` |
| AC-4 | parse-node-test-coverage.mjs 删除 | 脚本文件 `parse-node-test-coverage.mjs` 及其测试文件 `parse-node-test-coverage.test.mjs` 不再存在于项目中 |
| AC-5 | executor agent 步骤 4 更新 | `unit-test-executor.md` 中 `node-test` 覆盖率解析描述改为直接解析原始文本表格，通过 regex 匹配 `all files` 行提取 lines/branches/functions |
| AC-6 | 测试文件更新后全部通过 | `test-get-framework-config.test.ts` 和 `test-detect-frameworks.test.ts` 中所有测试通过，不再引用旧的 `tee`/`parse-node-test-coverage.mjs` |
| AC-7 | 覆盖率百分比仍然可提取 | 从 `node-test-output.txt` 原始文本中通过 regex `all files` 行可正确提取 lines、branches、functions 三个维度的覆盖率百分比 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 解析原始文本表格时 regex 与 Node.js 版本输出格式不兼容 | 覆盖率解析失败，返回 0 | 中 | 遵循 `go-cover` 已有模式；在 executor agent 文档中明确列出期望的表格格式和 regex 说明；使用 `go-cover` 相同的错误处理策略（无法解析时设为 0） |
| 现有测试仍引用旧的 coverage_cmd 字符串 | 测试失败，CI 阻塞 | 高 | 明确列出所有需要更新的测试文件（test-get-framework-config.test.ts、test-detect-frameworks.test.ts）；在 proposal 中标记范围为强制更新 |
| 开发者不清楚 node-test 覆盖率需要在 executor 中解析 | 配置或运行 node-test 覆盖率时困惑 | 低 | executor agent 文档中明确说明每个 coverage_format 的解析方式，`node-test` 条目包含具体说明和 regex 示例 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 采用外部脚本还是让 LLM 直接解析原始输出 | 让 LLM 直接解析（方案 B） | 与 `go-cover` 框架设计模式一致，提高框架配置的凝聚性，消除硬编码路径的脆弱性 | 方案 A：保留脚本但修复路径问题——引入了新的推理阶段脚本，增加维护负担 |
| coverage_output 指向原始文件还是解析后 JSON | 指向原始文本文件 `coverage/node-test-output.txt` | 与 `go-cover` 的 `coverage/func-summary.txt` 模式一致；executor 负责解析无需中间格式转换 |

### 待决问题

- 无
