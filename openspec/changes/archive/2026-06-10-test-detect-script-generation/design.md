# 设计: test-detect-script-generation

> **变更**: test-detect-script-generation
> **日期**: 2026-06-09
> **基于**: proposal.md, specs/test-execution-diagnostics/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `PlanEntry` 接口扩展 | 新增 `script: string` 必填字段，保存完整的 bash 执行脚本 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 无（TypeScript 接口） | TypeScript |
| `generateScript()` 函数 | 纯函数，根据 `directory`、`coverage_cleanup`、`coverage_cmd` 生成包含 shebang、set -e、可选的 cd、rm -rf 清理行、覆盖率命令的 bash 脚本 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 无（纯函数，无外部依赖） | TypeScript |
| `runTestDetectFrameworks()` 修改 | 在生成 plan 条目时调用 `generateScript()` 填充 `script` 字段 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `generateScript()`（同文件导出） | TypeScript |
| `testDetectFrameworksOutputSchema` 扩展 | plan 条目 Zod schema 新增 `script: z.string()` 必填字段 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | `zod/v4` | TypeScript / Zod |

### 组件图

```
runTestDetectFrameworks()
  |-- 对每个 test.frameworks 条目：
  |     |-- deriveWorkingDirectory(glob)        -> directory
  |     |-- runTestGetFrameworkConfig(framework)  -> { coverage_cmd, coverage_cleanup, ... }
  |     |-- generateScript({                      <-- 新增步骤
  |     |       directory,
  |     |       coverage_cleanup,
  |     |       coverage_cmd
  |     |     })                                 -> script
  |     |-- plan.push({
  |     |     directory, framework, coverage_cmd,
  |     |     coverage_format, coverage_output,
  |     |     coverage_artifacts, coverage_cleanup,
  |     |     script                              <-- 新增字段
  |     |   })
  |
  v
test_detect_frameworks MCP tool output
  |-- plan[].script   <-- 新字段，完整 bash 脚本

generateScript() 纯函数
  |-- 输入: { directory: string, coverage_cleanup: string[], coverage_cmd: string }
  |-- 输出: string (bash 脚本)
  |-- 副作用: 无（确定性纯函数）
```

---

## 数据流

### 流程描述

1. **`runTestDetectFrameworks()`** 遍历 `test.frameworks` 配置，对每个条目依次调用：
   - `deriveWorkingDirectory(glob)` 推导 `directory`
   - `runTestGetFrameworkConfig(framework)` 获取 `coverage_cmd`、`coverage_cleanup` 等配置
   - **新步骤**: `generateScript({ directory, coverage_cleanup, coverage_cmd })` 生成 `script`

2. **`generateScript()` 内部逻辑**:
   - 第一行始终为 `#!/bin/bash`
   - 第二行始终为 `set -e`
   - 如果 `directory !== "."`，插入 `cd <directory>` 行
   - 对 `coverage_cleanup` 数组中每个元素，插入 `rm -rf <item>` 行（保持顺序）
   - 如果 `coverage_cleanup` 为空数组，跳过 `rm -rf` 行
   - 最后一行始终为 `coverage_cmd`
   - 行之间使用 `\n` 分隔，末尾包含一个换行符

3. **Executor Agent** 收到包含 `script` 的 plan 条目后，可直接使用 `script` 字段作为 bash 执行脚本，无需手动拼装。本变更不改变 Executor 如何使用该脚本。

### 脚本生成示例

| 框架 | directory | coverage_cleanup | coverage_cmd | 生成的 script |
|------|-----------|------------------|-------------|---------------|
| vitest (glob: `**/*.test.ts`) | `.` | `["coverage", ".nyc_output"]` | `npx vitest run --coverage` | `#!/bin/bash\nset -e\nrm -rf coverage\nrm -rf .nyc_output\nnpx vitest run --coverage\n` |
| vite-plus (glob: `plugins/dev-team/bin`) | `plugins/dev-team/bin` | `["coverage", ".nyc_output"]` | `vp test --coverage` | `#!/bin/bash\nset -e\ncd plugins/dev-team/bin\nrm -rf coverage\nrm -rf .nyc_output\nvp test --coverage\n` |
| rust (glob: `**/tests/**/*.rs`) | `.` | `["coverage", "target/llvm-cov"]` | `cargo llvm-cov --all --coverage` | `#!/bin/bash\nset -e\nrm -rf coverage\nrm -rf target/llvm-cov\ncargo llvm-cov --all --coverage\n` |
| bun（清理较少） | `.` | `["coverage"]` | `bun test --coverage` | `#!/bin/bash\nset -e\nrm -rf coverage\nbun test --coverage\n` |
| coverage_cleanup 为空 | `.` | `[]` | `npx jest --coverage` | `#!/bin/bash\nset -e\nnpx jest --coverage\n` |

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `PlanEntry`（扩展） | `directory: string`（不变）<br>`framework: string`（不变）<br>`coverage_cmd: string`（不变）<br>`coverage_format: "istanbul" \| "llvm-cov"`（不变）<br>`coverage_output: string`（不变）<br>`coverage_artifacts: string[]`（不变）<br>`coverage_cleanup: string[]`（不变）<br>**`script: string`（新增）** — 完整的 bash 执行脚本 | 每个 plan 条目的 `script` 由 `generateScript()` 根据同一条目的 `directory`、`coverage_cleanup`、`coverage_cmd` 生成 | 不持久化（每次调用实时生成） |
| `generateScript()` 输出 | `string` — bash 脚本内容 | 输入参数决定输出：`{ directory, coverage_cleanup, coverage_cmd }` -> `string` | 不持久化（纯函数，立即返回） |

---

## 路由/API 设计

### MCP 工具: `test_detect_frameworks`（输出 schema 扩展）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_detect_frameworks` | 检测文件所属测试框架，plan 条目新增必填 `script` 字段 | `{ files?: string[], project_root?: string }`（不变） | `{ detected, frameworks, plan: { directory, framework, coverage_cmd, coverage_format, coverage_output, coverage_artifacts?, coverage_cleanup?, **script: string** }[] }` | 无（本地 MCP） |

输入 schema（`testDetectFrameworksInputSchema`）不变。输出 schema（`testDetectFrameworksOutputSchema`）的 plan 条目新增必填字段：

```typescript
// 新增字段定义
script: z.string().describe('Bash execution script with shebang, set -e, cd, rm -rf, and coverage command')
```

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **`generateScript()` 作为独立导出的纯函数** | 脚本生成逻辑是确定性的纯函数，独立导出便于单元测试和未来复用。不依赖文件系统或外部状态，相同输入始终产生相同输出。 | **备选：内联在 `runTestDetectFrameworks` 中**。被拒绝，内联导致无法单独测试脚本生成逻辑，且违反单一职责原则。 |
| D2 | **`script` 在 Zod 输出 schema 中设为必填（`z.string()` 非可选）** | 新的 plan 条目必须始终包含 `script` 字段。这是向前变更（non-backward-compatible），但由 Executor 消费的 plan 数据不应存在缺失 script 的场景。变更范围明确且一次性完成。 | **备选：设为可选字段 `z.string().optional()`**。被拒绝，脚本生成是本变更的核心意图，可选字段允许遗漏且无法通过 schema 强制保证。消费者（Executor）预期间 script 始终存在。 |
| D3 | **脚本模板固定为 `#!/bin/bash` + `set -e` + `[cd]` + `[rm -rf ...]` + `<coverage_cmd>`** | 采用业界标准的 bash 安全实践。`set -e` 确保任何命令失败时脚本立即退出，`cd` 仅在需要时生成，保持脚本简洁。所有行使用 `\n` 分隔，末尾包含换行符。 | **备选：使用 `set -euo pipefail`**。被拒绝，`set -u` 可能因未初始化变量导致不必要的中断，`set -o pipefail` 在部分 shell 中行为不一致。`set -e` 在安全性和兼容性之间取得平衡。 |
| D4 | **仅在 `directory !== "."` 时生成 `cd` 行** | `cd .` 是空操作，包含它会产生多余的 shell 调用并可能引起误解。跳过 `cd .` 使脚本更干净。 | **备选：始终生成 `cd <directory>` 行**。被拒绝，当 directory 为 `"."` 时，`cd .` 不改变当前目录但增加噪音，且可能让读者误以为脚本会在子目录执行。 |
| D5 | **`coverage_cleanup` 为空数组时跳过所有 `rm -rf` 行** | 避免生成空的 `rm -rf`（语法错误）或不必要的删除操作。bun 框架只需要删除 `coverage` 一个目录，而其他框架需要两个。通过判断空数组精确控制清理步骤。 | **备选：始终生成至少一个 `rm -rf` 占位**。被拒绝，占位行会执行不必要的 `rm -rf`（无参数时语法错误），必须在运行时动态判断，增加复杂性。 |
| D6 | **仅支持 bash 脚本生成，不支持 PowerShell** | 项目开发和运行环境以 Unix/macOS 为主（Node.js 工具链），使用 WSL 的 Windows 用户也能执行 bash。PowerShell 脚本生成涉及不同的语法和变量引用方式，超出当前变更范围。 | **备选：同时支持 bash 和 PowerShell 生成**。被拒绝，需要额外增加分支逻辑、两套模板和测试，当前没有 Windows 原生执行的需求。PowerShell 支持可延迟到后续变更。 |

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。`generateScript()` 为纯函数，依赖的数据全部来自已有的 `PlanEntry` 字段（`directory`、`coverage_cleanup`、`coverage_cmd`），这些字段由 `runTestGetFrameworkConfig()` 从 `FRAMEWORK_REGISTRY` 加载。

### 构建/测试依赖

- `vite-plus/test` — 已有的测试框架，用于 `generateScript()` 的单元测试
- `zod/v4` — 已在项目中用于 schema 定义，用于新增 `script` 必填字段的验证测试

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 脚本模板与 Executor 期望的执行方式不一致 | Executor 使用生成的脚本但执行结果不符合预期 | 低 | `generateScript()` 使用的 `coverage_cmd` 和 `coverage_cleanup` 与 Executor 原本手动拼装的数据完全一致（均来自 FRAMEWORK_REGISTRY），无行为差异 |
| `cd <directory>` 在目录不存在时导致脚本失败 | 脚本以非零退出码终止 | 低 | 脚本使用 `set -e` 确保失败显式暴露，与预期行为一致；目录不存在属于配置错误，应尽早暴露 |
| `rm -rf` 删除超出预期的文件 | 意外删除用户文件 | 低 | 清理路径来自 FRAMEWORK_REGISTRY 的硬编码配置（`coverage`、`.nyc_output`、`target/llvm-cov`），经过审核的已知临时目录名 |
| 仅支持 bash 限制 Windows 用户体验 | Windows 用户无法直接执行生成的脚本 | 中 | 明确设计决策仅支持 bash；Windows 用户可使用 WSL 或 Git Bash 执行。PowerShell 支持可延迟到后续变更 |
| 现有 plan 消费者未升级 schema 导致解析错误 | `script` 为必填字段，旧版消费者在解析包含 script 的输出时不会失败（passthrough），但旧版生产者生成的 plan 缺少 script 会导致验证失败 | 低 | `runTestDetectFrameworks` 是唯一的 plan 生产者，本次变更同时修改生产端。所有 plan 都包含 script 字段，schema 验证通过。 |

---

## 迁移步骤

无迁移步骤。本变更一次性完成所有修改：

1. `PlanEntry` 接口新增 `script: string` 必填字段——现有消费者忽略额外的字段即可（TypeScript 编译检查要求实现新字段）
2. `testDetectFrameworksOutputSchema` 新增 `script: z.string()` 必填字段——`runTestDetectFrameworks` 同时修改确保输出符合新 schema
3. `generateScript()` 纯函数新增——不影响现有功能
4. 已有测试（`test-detect-frameworks.test.ts` 和 schema 测试）补充新字段验证——无回归风险

---

## 待决问题

- 未来是否需要在 `generateScript()` 中添加框架特定的预处理步骤（如 rust 框架需要在 `cargo llvm-cov` 前执行 `cargo test` 的准备工作）？目前 `coverage_cmd` 已包含完整的命令，无需额外步骤。
- 是否需要在 `generateScript()` 中添加超时控制或重试逻辑？目前不需要，超时由 Executor 在脚本调用层级控制，不归脚本本身负责。
