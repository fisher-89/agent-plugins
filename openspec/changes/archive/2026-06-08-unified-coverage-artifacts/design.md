# 设计: unified-coverage-artifacts

> **变更**: unified-coverage-artifacts
> **日期**: 2026-06-08
> **基于**: proposal.md, per-directory-test-execution/design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `FrameworkConfig` 接口扩展 | 新增 `coverage_artifacts: string[]`（声明要移动的产物 glob 列表）和 `coverage_cleanup: string[]`（声明要删除的目录/文件列表） | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | 无（TypeScript 接口） | TypeScript |
| `FRAMEWORK_REGISTRY` 填充 | 五个框架（jest/vitest/vite-plus/bun/rust）的注册表条目填充 `coverage_artifacts` 和 `coverage_cleanup` 值 | `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | `FrameworkConfig` 接口 | TypeScript |
| `PlanEntry` 接口扩展 | 新增 `coverage_artifacts` 和 `coverage_cleanup` 字段，plan 条目从框架注册表携带这些信息 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `FrameworkConfig`（通过 `runTestGetFrameworkConfig` 调用获取） | TypeScript |
| `testGetFrameworkConfigOutputSchema` 扩展 | 输出 Zod schema 新增 `coverage_artifacts` 和 `coverage_cleanup` 为可选字段（向后兼容） | `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts` | `zod/v4` | TypeScript / Zod |
| `testDetectFrameworksOutputSchema` 扩展 | plan 条目 Zod schema 新增 `coverage_artifacts` 和 `coverage_cleanup` 为可选字段（向后兼容） | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | `zod/v4` | TypeScript / Zod |
| `unit-test-executor` | Agent 指南文档，在步骤 2（覆盖率执行）和步骤 3（覆盖率解析）之间插入"移动覆盖率产物"步骤 | `plugins/dev-team/agents/unit-test-executor.md` | 无（文档） | Markdown |

### 组件图

```
test_get_framework_config (MCP Tool)
  |
  v
runTestGetFrameworkConfig()
  |-- 返回 FrameworkConfig { framework, test_cmd, coverage_cmd,
  |     coverage_format, coverage_output,
  |     coverage_artifacts, coverage_cleanup }   <-- 新增字段
  |
  v
runTestDetectFrameworks()
  |-- plan.push({
  |     directory, framework, coverage_cmd,
  |     coverage_format, coverage_output,
  |     coverage_artifacts,                 <-- 从注册表携带
  |     coverage_cleanup                    <-- 从注册表携带
  |   })
  |
  v
unit-test-executor (Agent)
  1. 框架检测（不变）
  2. 逐目录执行覆盖率命令（不变）
  3. 移动覆盖率产物（新增步骤）
     |-- 对每个 plan 条目：
     |     |-- 若 coverage_artifacts 非空：
     |     |     |-- 创建统一目标目录 reports/coverage/<framework>/
     |     |     |-- 按 glob 匹配并移动产物到目标目录
     |     |     |-- 更新 coverage_output 路径为统一位置
     |     |     |-- 删除 coverage_cleanup 中的原始目录
     |     |-- 若 coverage_artifacts 为空：跳过
     |     |-- 若移动失败：记录 findings，保留原始状态，不清除
  4. 覆盖率解析（根据更新后的 coverage_output 路径读取）
  5. 阈值判定（不变）
  6. 报告写入（html_report 路径指向统一位置）
```

---

## 数据流

### 流程描述

1. `runTestGetFrameworkConfig(framework)` 被调用。返回的 `FrameworkConfig` 对象新增 `coverage_artifacts` 和 `coverage_cleanup` 字段，值来自硬编码注册表。

2. `runTestDetectFrameworks` 内部在生成 plan 时，从 `runTestGetFrameworkConfig` 的返回值中一并提取 `coverage_artifacts` 和 `coverage_cleanup`，注入 `PlanEntry`。

3. Executor 收到 plan 后，遍历每个条目：
   - 3a. 在 `directory` 下执行 `coverage_cmd`，产物（如 `coverage/coverage-summary.json`、`coverage/index.html`）生成在框架工作目录中。
   - 3b. **新增步骤**：将产物从框架工作目录移动到统一位置 `openspec/changes/<change>/reports/coverage/<framework>/`。使用 `coverage_artifacts` 中的 glob 模式匹配要移动的文件。
   - 3c. 移动成功后，`coverage_output` 路径更新为指向统一位置下的文件（如 `reports/coverage/vitest/coverage-summary.json`）。
   - 3d. 删除 `coverage_cleanup` 中声明的原始目录（如 `coverage/`、`.nyc_output/`）。
   - 3e. 若移动失败（源文件不存在），保留原始状态，不清除，记录 findings 但不阻断流程。

4. 覆盖率解析器（`coverage-parser.ts`）从更新后的 `coverage_output` 路径读取文件。解析逻辑不变。

5. 报告写入时，`coverage_by_framework` 的 `html_report` 路径指向统一位置下的 HTML 报告。

### 统一目录结构

```
openspec/changes/<change>/
  reports/
    coverage/
      <framework-1>/
        coverage-summary.json
        index.html
        ...
      <framework-2>/
        coverage-summary.json
        index.html
        ...
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FrameworkConfig`（扩展） | `framework: string`（不变）<br>`test_cmd: string`（不变）<br>`coverage_cmd: string`（不变）<br>`coverage_format: "istanbul" \| "llvm-cov"`（不变）<br>`coverage_output: string`（不变）<br>**`coverage_artifacts: string[]`（新增）** — 要移动的产物 glob 列表<br>**`coverage_cleanup: string[]`（新增）** — 要删除的目录/文件列表 | 注册表中的每个条目对应一个框架的全部配置 | 硬编码在 `test-get-framework-config.ts` |
| `PlanEntry`（扩展） | `directory: string`（不变）<br>`framework: string`（不变）<br>`coverage_cmd: string`（不变）<br>`coverage_format: "istanbul" \| "llvm-cov"`（不变）<br>`coverage_output: string`（不变）<br>**`coverage_artifacts: string[]`（新增）**<br>**`coverage_cleanup: string[]`（新增）** | 每个 plan 条目对应一个框架配置，`coverage_artifacts/cleanup` 委托自 `FrameworkConfig` 注册表 | 不持久化（每次调用实时生成） |

### 各框架注册表配置

| 框架 | `coverage_format` | `coverage_output` | `coverage_artifacts` | `coverage_cleanup` |
|------|-------------------|-------------------|---------------------|-------------------|
| jest | istanbul | coverage/coverage-summary.json | ["coverage/**"] | ["coverage", ".nyc_output"] |
| vitest | istanbul | coverage/coverage-summary.json | ["coverage/**"] | ["coverage", ".nyc_output"] |
| vite-plus | istanbul | coverage/coverage-summary.json | ["coverage/**"] | ["coverage", ".nyc_output"] |
| bun | istanbul | coverage/coverage-summary.json | ["coverage/**"] | ["coverage"] |
| rust | llvm-cov | coverage/coverage-summary.json | ["coverage/**", "target/llvm-cov/**"] | ["coverage", "target/llvm-cov"] |

---

## 路由/API 设计

### MCP 工具: `test_get_framework_config`（输出 schema 扩展）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_get_framework_config` | 获取指定框架的命令配置（新增 `coverage_artifacts` 和 `coverage_cleanup` 可选字段） | `{ framework: string }`（不变） | `{ framework, test_cmd, coverage_cmd, coverage_format, coverage_output, **coverage_artifacts?: string[], coverage_cleanup?: string[]** }` | 无（本地 MCP） |

### MCP 工具: `test_detect_frameworks`（plan 条目 schema 扩展）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_detect_frameworks` | 检测文件所属测试框架（不变），plan 条目新增 `coverage_artifacts` 和 `coverage_cleanup` | `{ files?: string[], project_root?: string }`（不变） | `{ detected, frameworks, plan: { directory, framework, coverage_cmd, coverage_format, coverage_output, **coverage_artifacts?: string[], coverage_cleanup?: string[]** }[] }` | 无（本地 MCP） |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | `coverage_artifacts` 和 `coverage_cleanup` 在 Zod 输出 schema 中设为**可选**（`z.array(z.string()).optional()`） | 保持向后兼容：现有消费者在升级前不会传递这些字段，可选字段不会导致 schema 解析失败。Executor 在读取时检查空数组/undefined 以决定是否跳过移动步骤。 | **备选：设为必填字段**。被拒绝，这会导致旧版消费者在未升级 schema 时解析失败，违反向后兼容原则。 |
| D2 | 产物移动策略使用 glob 匹配 + 复制后删除，而非 `fs.rename` | Glob 匹配支持 `coverage/**` 等模式，可以处理目录下的多文件/嵌套结构。复制后删除比跨设备 `rename` 更安全（网络驱动器、容器挂载等场景不会因跨设备问题失败）。 | **备选：使用 `fs.rename` 直接移动目录**。被拒绝，`coverage_artifacts` 使用 glob 模式而非目录名，且 `rename` 在跨设备时抛出 EXDEV 错误，需要 fallback 到复制+删除，增加复杂度。 |
| D3 | 统一路径使用相对于变更目录的 `reports/coverage/<framework>/` | 与 `reports/unit-test-execution.json` 的存放位置一致，路径简洁。按框架名隔离避免并发覆盖。 | **备选：使用绝对路径或项目根目录相对路径**。被拒绝，相对于变更目录的路径在归档/迁移时更自然，所有报告产物都应放在变更目录下。 |
| D4 | 移动失败时保留原始状态，不清除，记录 findings 而非抛出异常 | 覆盖率数据是辅助信息，不应因产物移动失败而阻断整体测试报告生成。Executor 应具有容错性（非零 exit code 不阻断流程的设计原则在此延续）。 | **备选：移动失败时抛出异常中断流程**。被拒绝，提案明确要求移动失败时保留原始状态不阻断流程（风险缓解措施），与覆盖率命令失败不阻断测试报告的原则一致。 |
| D5 | `coverage_cleanup` 使用相对路径名（如 `"coverage"`、`".nyc_output"`）而非 glob | 清理目标通常是固定名称的临时目录或文件，使用简单名称比 glob 更安全和可预测。避免了误匹配的风险（如 `coverage` 目录名恰好与用户代码中的目录名冲突的概率极低）。 | **备选：`coverage_cleanup` 也使用 glob 模式**。被拒绝，过度设计。清理是精确删除已知的临时目录，glob 模式可能意外匹配用户文件。 |
| D6 | `coverage_artifacts` 使用 glob 模式（如 `"coverage/**"`）而非目录名 | Glob 模式更灵活，支持精确选择要移动的文件子集。`coverage/**` 明确表达"移动 coverage 目录下的所有内容"，比目录名语义更清晰。 | **备选：直接使用目录名**。被拒绝，不够精确——如果将来只需要移动部分文件（如只移动 JSON 报告不移动 HTML），glob 模式更有优势。 |

---

## 依赖

### 运行时依赖

- `zod/v4` — 已在项目中用于 schema 定义，用于扩展现有 schema 的可选字段
- `test-get-framework-config.ts` — 框架命令注册表，新增 `coverage_artifacts` 和 `coverage_cleanup` 字段
- `test-detect-frameworks.ts` — 生成 plan 条目时携带新的字段

### 构建/测试依赖

- `vite-plus/test` — 已有的测试框架，用于新增字段的单元测试
- `fs` (Node.js built-in) — 用于模拟场景中的文件系统操作（移动、删除）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 移动操作在覆盖率命令执行但未生成产物时失败 | Executor 流程中断，覆盖率数据丢失 | 低 | 移动前检查源路径是否存在；移动失败时记录错误到 findings 但不阻断流程，`coverage` 设为 null |
| 清理操作误删用户文件 | 项目目录中用户文件被意外删除 | 极低 | 仅清理在 `coverage_cleanup` 中声明的目录名，且仅在移动成功后执行；清理范围限制在各框架工作目录下 |
| 并发执行时不同框架的产物互相覆盖 | 覆盖率报告被错误数据污染 | 低 | 统一位置按框架名隔离（`reports/coverage/<framework>/`），天然互斥 |
| Windows 路径兼容性问题（反斜杠 vs 正斜杠） | 路径匹配失败或产物未正确移动 | 低 | 代码中使用 `path.posix` 风格或显式归一化处理；executor 指令强调使用正斜杠规则 |
| Executor Agent 对 glob 匹配的支持不足 | 移动步骤执行不正确 | 中 | Executor 指南提供具体的 glob 用法示例和 cp/mv 命令模板；基于 glob 的简单匹配由 shell 原生支持 |
| `coverage_artifacts` 为空数组时仍尝试移动 | 不必要的步骤执行 | 极低 | Executor 指南明确要求检查空数组后跳过移动步骤 |

---

## 迁移步骤

无迁移步骤。此变更为纯新增字段和新增执行步骤。所有变更向后兼容：

1. `FrameworkConfig` 接口新增 `coverage_artifacts` 和 `coverage_cleanup`——现有消费者不受影响
2. `PlanEntry` 接口新增对应字段——现有消费者忽略即可
3. Zod 输出 schema 新增可选字段——现有 schema 解析不受影响
4. `unit-test-executor.md` 新增步骤——仅影响新的 Agent 会话
5. `coverage-parser.ts` 和 `coverage-calculator.ts` 不变

---

## 待决问题

- 是否需要为移动步骤编写独立的 Node.js 脚本（如 `move-coverage-artifacts.ts`），还是由 Executor Agent 直接在指南中通过 shell 命令实现？当前设计倾向后者，以保持 Executor 的 Agent 驱动的灵活性。
- `coverage_artifacts` 的 glob 匹配是否需要支持否定模式（如 `!coverage/*.tmp`）？目前没有这个需求，各框架的产物目录结构是确定的。
