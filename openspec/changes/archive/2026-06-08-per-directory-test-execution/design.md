# 设计: per-directory-test-execution

> **变更**: per-directory-test-execution
> **日期**: 2026-06-08
> **基于**: proposal.md, specs/test-execution-diagnostics/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `deriveWorkingDirectory` | 从 glob 模式字符串推导出命令执行的工作目录（相对于项目根目录） | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 无（纯函数） | TypeScript |
| `runTestDetectFrameworks` | 检测文件所属测试框架；输出扩展 `plan` 字段，包含每个框架的执行计划（directory、coverage_cmd、coverage_format、coverage_output） | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `readConfig`、`normaliseFrameworks`、`runTestGetFrameworkConfig`（内部调用）、`deriveWorkingDirectory` | TypeScript |
| `testDetectFrameworksOutputSchema` | Zod schema 定义 `test_detect_frameworks` 工具的输出结构，新增 `plan` 数组字段 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | `zod/v4` | TypeScript / Zod |
| `unit-test-executor` | Agent 指南文档，定义 Executor 的执行流程；修改后删除独立 test_cmd 步骤和 test_get_framework_config 调用，改为直接从 `plan` 逐目录执行 coverage_cmd | `plugins/dev-team/agents/unit-test-executor.md` | 无（文档） | Markdown |

### 组件图

```
test_detect_frameworks (MCP Tool)
  |
  v
runTestDetectFrameworks()
  |-- normaliseFrameworks(config.test.frameworks)
  |-- for each mapping:
  |     |-- deriveWorkingDirectory(glob)        -- 推导工作目录
  |     |-- runTestGetFrameworkConfig(framework) -- 获取命令配置
  |     |-- 生成 plan 条目 {directory, framework, coverage_cmd, coverage_format, coverage_output}
  |-- for each file: matchGlob(file, glob)      -- 首匹配文件到框架
  |-- return { detected, frameworks, plan }
        |
        v
unit-test-executor (Agent)
  |-- 调用 test_detect_frameworks 获取检测结果 + plan
  |-- 遍历 plan 条目:
  |     |-- 在 directory 下执行 coverage_cmd
  |     |-- 按 coverage_format 解析覆盖率输出
  |-- 阈值判定、报告写入（不变）
```

---

## 数据流

### 流程描述

1. `test_detect_frameworks` 被调用（MCP 工具）。其内部调用 `readConfig` 读取 `openspec/config.json` 的 `test.frameworks` 配置。
2. `normaliseFrameworks` 将配置归一化为 `{glob, framework}[]` 数组——字符串简写展开为带默认 glob 的单条记录，空/未配置则返回空数组。
3. 对归一化后的每个 `{glob, framework}` 条目：
   - 调用 `deriveWorkingDirectory(glob)` 从 glob 模式推导工作目录。规则：取第一个通配符（`*`、`?`、`{`）之前的路径前缀。无通配符则返回整个 glob。通配符在起始位置返回 `"."`。
   - 调用 `runTestGetFrameworkConfig(framework)` 获取该框架的 `coverage_cmd`、`coverage_format`、`coverage_output`。
   - 组装为一条 `plan` 条目：`{directory, framework, coverage_cmd, coverage_format, coverage_output}`。
4. 按文件首匹配 glob 生成 `detected` 数组和 `frameworks` 列表（逻辑不变）。
5. 返回 `{ detected, frameworks, plan }`。
6. Executor 收到结果后，遍历 `plan` 数组：
   - 对每个条目，在其 `directory` 下执行 `coverage_cmd`。
   - 根据 `coverage_format` 解析覆盖率输出文件（`coverage_output` 相对于 `directory`）。
   - 不再单独执行 `test_cmd`，也不再调用 `test_get_framework_config`。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `TestDetectFrameworksResult` | `detected: {file, framework}[]`（不变）、`frameworks: string[]`（不变）、**`plan: PlanEntry[]`（新增）** | `plan` 基于 `test.frameworks` 配置逐条目生成；`detected` 基于文件首匹配生成 | 不持久化（每次调用实时生成） |
| `PlanEntry`（新增） | `directory: string` — 执行工作目录（相对项目根目录）<br>`framework: string` — 框架名称<br>`coverage_cmd: string` — 覆盖率命令（包含测试运行）<br>`coverage_format: "istanbul" \| "llvm-cov"` — 覆盖率输出格式<br>`coverage_output: string` — 覆盖率输出路径（相对 directory） | 每个条目对应 `test.frameworks` 中的一条配置，`coverage_cmd/format/output` 委托自 `test-get-framework-config` 注册表，`directory` 由 `deriveWorkingDirectory` 从 glob 推导 | 不持久化 |
| `FrameworkMapping`（不变） | `glob: string` — 测试文件模式<br>`framework: string` — 框架名称 | 存储在 `config.json` 的 `test.frameworks` 数组中 | `openspec/config.json` |

### `deriveWorkingDirectory` 关键路径示例

| glob 模式 | 第一个通配符位置 | 返回路径 |
|-----------|----------------|----------|
| `"plugins/dev-team/bin"` | 无通配符 | `"plugins/dev-team/bin"` |
| `"src/**/*.test.ts"` | 索引 3（`*`） | `"src"` |
| `"**/*.test.ts"` | 索引 0（`*`） | `"."` |
| `"{src,lib}/*.test.ts"` | 索引 0（`{`） | `"."` |
| `"tests/?nit/*.test.ts"` | 索引 6（`?`） | `"tests"` |
| `"packages/*/src/__tests__/*.test.ts"` | 索引 9（`*`） | `"packages"` |

---

## 路由/API 设计

### MCP 工具: `test_detect_frameworks`（输出 schema 变更）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_detect_frameworks` | 检测文件所属测试框架（不变），返回扩展后的执行计划（新增 `plan`） | `{ files?: string[], project_root?: string }`（不变） | `{ detected: {file, framework}[], frameworks: string[], **plan: {directory, framework, coverage_cmd, coverage_format, coverage_output}[]** }` | 无（本地 MCP） |

### MCP 工具: `test_get_framework_config`（不变）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_get_framework_config` | 获取指定框架的命令配置（保持不变，为其他消费者保留） | `{ framework: string }` | `{ framework, test_cmd, coverage_cmd, coverage_format, coverage_output }` | 无（本地 MCP） |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | `plan` 由 `runTestDetectFrameworks` 在调用时实时生成，而非在 config.json 中预计算 | 保证 `plan` 始终基于最新注册表（`test-get-framework-config`），注册表变更不需要更新配置文件。Executor 一次调用即可获取所有所需信息，消除信息差。 | **备选：在 config.json 中新增 `plan` 静态字段**。被拒绝，因为会导致配置与注册表的双重维护，增加不一致风险。 |
| D2 | `deriveWorkingDirectory` 识别 `*`、`**`、`?`、`{` 四种通配符，但不识别 `[`（字符类） | 提案明确指定了四种通配符，而这四种覆盖了实际项目中 99% 以上的 glob 模式。`[` 字符类在测试框架 glob 中极少使用。 | **备选：实现完整的 glob 解析（包括 `[` 等转义处理）**。被拒绝，增加复杂度且无实际用例。若将来需要，扩展规则影响范围极小。 |
| D3 | `plan` 数组的顺序与 `test.frameworks` 配置顺序一致 | 用户可以通过调整 `test.frameworks` 顺序来控制 `plan` 的执行顺序（重要时先执行）。实现简单直接。 | **备选：按框架名称字母序排序**。被拒绝，会丢失用户期望的执行顺序控制，且与 `test.frameworks` 首匹配语义不一致。 |
| D4 | Executor 不再单独调用 `test_get_framework_config` MCP 工具 | `runTestDetectFrameworks` 内部已有 `runTestGetFrameworkConfig` 的调用路径，直接在内部获取命令配置并注入 `plan`，避免 Executor 二次调用。减少 MCP 往返次数，简化 Agent 流程。 | **备选：保持 Executor 调用 `test_get_framework_config` 的模式**。被拒绝，虽可保持最小化修改，但增加了 Agent 的步骤复杂度和 MCP 通信开销。 |
| D5 | Executor 始终运行 `coverage_cmd`（包含测试），不再单独运行 `test_cmd` | 覆盖率命令本身包含测试执行全过程，单独运行 `test_cmd` 是重复的。合并为单步后执行时间减半。 | **备选：保留两阶段，通过检查输出决定是否跳过**。被拒绝，复杂度高且不可靠；`coverage_cmd` 已经是超集，无需冗余的 `test_cmd`。 |

---

## 依赖

### 运行时依赖

- `zod/v4` — 已在项目中用于 schema 定义，用于扩展 `testDetectFrameworksOutputSchema`
- `test-get-framework-config.ts` — 已有的框架命令注册表，`runTestDetectFrameworks` 内部调用其 `runTestGetFrameworkConfig` 和 `getDefaultGlobForFramework`

### 构建/测试依赖

- `vite-plus/test` — 已有的测试框架，用于 `deriveWorkingDirectory` 和 `plan` 的单元测试
- `vitest` 或等效 runner — 已有的测试运行环境

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `deriveWorkingDirectory` 对含嵌套通配符的复杂 glob 模式推导错误 | Executor 在错误目录下运行命令，导致测试失败或覆盖率数据丢失 | 低 | 实现时覆盖 AC-2 至 AC-5 所有通配符变体（`*`、`**`、`?`、`{}`）的单元测试；规则简单可预测（第一个通配符前的纯路径前缀） |
| Executor 从 `plan` 获取 `coverage_cmd` 后不再感知注册表的实时变化 | 无实际风险 | 极低 | `plan` 在每次 `test_detect_frameworks` 调用时实时生成，总是从当前注册表获取命令 |
| 现有消费者依赖 `test_detect_frameworks` 仅返回 `detected` 和 `frameworks` 字段 | 新增 `plan` 字段不影响现有消费者 | 极低 | `plan` 是新增的只读字段，不修改现有字段的含义或类型（AC-11）；所有现有测试继续通过 |
| unit-test-executor.md 指南更新后与旧版流程混淆 | Agent 可能继续使用旧流程 | 低 | 文档中明确标记步骤更改对照表；旧流程步骤 2/3/4 被删除而非保留注释 |

---

## 迁移步骤

无迁移步骤。此变更为纯新增（`plan` 字段）和修改（Executor 流程），不涉及数据迁移或配置格式变更。所有变更向后兼容：

1. `test_detect_frameworks` 输出新增 `plan` 字段——现有消费者忽略此字段仍能正常工作
2. `unit-test-executor.md` 更新流程——仅影响新的 Agent 会话
3. `test-get-framework-config.ts` 保持不变——保留供其他消费者使用
4. `config.json` 的 `test.frameworks` schema 不变

---

## 待决问题

- `deriveWorkingDirectory` 是否需要导出供其他模块使用？当前仅在 `runTestDetectFrameworks` 内部使用，保持 `export` 便于测试
- 覆盖率输出文件路径 `coverage_output` 应相对于 `directory` 还是项目根目录？设计上相对于 `directory`，因为命令在 `directory` 下执行，覆盖率工具默认输出相对于工作目录
