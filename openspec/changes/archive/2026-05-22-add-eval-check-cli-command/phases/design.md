# 设计: add-eval-check-cli-command

> **变更**: add-eval-check-cli-command
> **日期**: 2026-05-22
> **基于**: proposal.md, test-design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `eval-check.ts` | 实现 `eval-check` CLI 子命令的参数定义、参数验证、变更目录存在性验证、调用核心逻辑函数、格式化输出 | `plugins/dev-team/bin/src/commands/eval-check.ts` | `eval-json.ts`（`readEvalJson`）、`workflow.ts`（`getPriorPhases`、`getPhaseIndex`）、`change.ts`（`getPhasesDir`）、`fs`（`existsSync`） | TypeScript, cac |
| `index.ts`（修改） | 注册 `eval-check` 子命令到 CAC CLI 实例 | `plugins/dev-team/bin/src/index.ts` | `eval-check.ts` | TypeScript, cac |
| `eval-json.ts`（已有，不修改） | 提供 `readEvalJson()` 读取 eval entries，`checkGate()` 前置阶段门控检查 | `plugins/dev-team/bin/src/lib/eval-json.ts` | fs, path | TypeScript |
| `workflow.ts`（已有，不修改） | 提供 `PHASES` 阶段顺序列表、`getPriorPhases()`、`getPhaseIndex()` | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| `change.ts`（已有，不修改） | 提供 `getPhasesDir()` 解析变更目录路径 | `plugins/dev-team/bin/src/lib/change.ts` | path | TypeScript |
| `eval-check.unit.test.ts` | 对核心逻辑函数（门控检查、timestamp 顺序验证、回溯检测、阶段状态判定、结果组装）进行单元测试 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | vitest, `eval-check.ts`（核心函数） | TypeScript, vitest |
| `eval-check.integration.test.ts` | 对 CLI 命令的完整链路（参数解析、退出码、JSON 输出格式）进行集成测试 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts` | vitest, execa, fs, os, path | TypeScript, vitest |

### 组件图

```
用户 / 技能脚本
    |
    | 调用 dev-team eval-check --change <name> --phase <phase> [--json]
    v
index.ts (main)
    |
    | CAC 路由到 eval-check 子命令
    v
eval-check.ts (registerEvalCheckCommand)
    |
    |--- 参数验证 (--change, --phase 必填)
    |--- getPhasesDir(changeName)  [change.ts]
    |--- phases 目录存在性验证 (fs.existsSync) | 不存在则 stderr 报错 + process.exit(1)
    |--- readEvalJson(phasesDir)   [eval-json.ts]
    |--- getPriorPhases(phase)     [workflow.ts]
    |
    |--- 核心检查函数 (纯函数，同一文件):
    |     - checkPriorPhases(entries, priorPhases)   -- 门控检查
    |     - checkTimestampOrder(entries, priorPhases) -- 顺序检查
    |     - checkBacktrack(entries, priorPhases)      -- 回溯检测
    |     - determinePhaseState(entries, phase)       -- 阶段状态判定
    |     - buildEvalCheckResult(...)                  -- 结果组装
    |
    |--- 输出格式 (JSON 或人类可读文本)
    |--- process.exit(0) 或 process.exit(1)
    v
stdout / stderr / 退出码
```

---

## 数据流

### 流程描述

1. **参数解析**：CLI action handler 从 CAC 解析 `--change`、`--phase`、`--json` 参数。验证 `--change` 和 `--phase` 非空，验证 `--phase` 值是否在合法阶段列表中（通过 `getPhaseIndex()` 返回非 -1）。
2. **路径解析**：调用 `getPhasesDir(changeName)` 获取变更的 phases 目录绝对路径。
3. **变更目录存在性验证**：调用 `fs.existsSync(phasesDir)` 验证 phases 目录是否存在。若目录不存在（说明变更名称无效），输出错误消息到 stderr 并 `process.exit(1)`。
4. **读取 eval.json**：调用 `readEvalJson(phasesDir)` 读取 eval entries 数组。若文件不存在或为空，返回空数组 `[]`。
5. **前置阶段识别**：调用 `getPriorPhases(phase)` 获取当前阶段的所有前置阶段列表。若为首个阶段（01-requirements），前置阶段列表为空。
6. **核心检查**：依次执行四项检查，每项检查不阻断后续检查（收集所有问题）：
   - **门控检查**：`checkPriorPhases(entries, priorPhases)` — 对每个前置阶段，检查 entries 中是否至少有一条 `verdict === "pass"` 的记录。
   - **Timestamp 顺序检查**：`checkTimestampOrder(entries, priorPhases)` — 收集每个前置阶段的最新 pass 记录的 timestamp，验证 timestamp 随阶段顺序单调递增（使用 `>=` 比较）。
   - **回溯标记检测**：`checkBacktrack(entries, priorPhases)` — 对每个前置阶段，按 timestamp 降序取最新条目，检查其 `backtrack_to` 字段是否为非 null 值。
   - **阶段状态判定**：`determinePhaseState(entries, phase)` — 取当前阶段的最新条目（按 timestamp 降序），根据 verdict 判定：无条目为 `"first_run"`、最新 verdict 为 `"fail"` 为 `"retry"`、最新 verdict 为 `"pass"` 为 `"passed"`。
7. **结果组装**：`buildEvalCheckResult(...)` 将所有检查结果和阶段状态组装为结构化结果对象。
8. **输出与退出**：
   - `--json` 模式：`console.log(JSON.stringify(result))`。
   - 非 JSON 模式：根据 `passed` 值输出人类可读的通过/阻断信息，阻断时列出所有 `block_reasons`。
   - 退出码：`passed === true` 时 `process.exit(0)`，否则 `process.exit(1)`。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `EvalCheckResult` | `passed: boolean` — 是否所有前置阶段门控通过且无阻断条件 | 聚合 `Details` 和 `PhaseState` | 运行时对象，输出到 stdout |
| | `phase: string` — 当前检查的阶段 ID | | |
| | `prior_phases: string[]` — 前置阶段 ID 列表 | | |
| | `block_reasons: string[]` — 阻断原因列表（为空表示无阻断） | | |
| | `phase_state: "first_run" \| "retry" \| "passed"` — 当前阶段状态 | | |
| | `details: Details` — 各项检查的详细结果 | | |
| `Details` | `prior_phase_gate: { passed: boolean; missing: string[] }` | 引用 `GateResult` 结构 | 运行时对象，嵌套在 `EvalCheckResult` 中 |
| | `timestamp_order: { passed: boolean; order_valid: boolean }` | | |
| | `backtrack: { passed: boolean; active_backtrack_phases: string[] }` | | |
| `GateResult`（已有） | `passed: boolean; missing: string[]` | 由 `eval-json.ts` 的 `checkGate()` 已定义 | 运行时对象 |
| `eval.json`（已有） | `phase: string, timestamp: string, verdict: string, attempt: number, report: string, items: Item[], backtrack_to: string \| null, schema_version: string` | 数组中的每个元素是一条 eval entry | 文件系统，`<change>/phases/eval.json` |

---

## 路由/API 设计

### CLI 命令

| 命令 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `eval-check` | `dev-team eval-check` | 验证当前阶段的所有前置阶段是否已通过评估，检查顺序性和回溯标记，报告当前阶段状态 | `--change <name>`（必填）、`--phase <phase>`（必填）、`--json`（可选） | JSON 或人类可读文本到 stdout；错误消息到 stderr | 无 |

### 参数详情

| 参数 | 类型 | 必填 | 默认值 | 描述 |
|------|------|------|--------|------|
| `--change <name>` | string | 是 | — | 变更名称，对应 `openspec/changes/<name>` |
| `--phase <phase>` | string | 是 | — | 阶段标识符（例如 `03-dev-proposal`）。必须是 `workflow.ts` 中 `PHASES` 数组的合法值 |
| `--json` | boolean | 否 | `false` | 输出结构化的 JSON 结果而非人类可读文本 |

### 退出码

| 退出码 | 含义 |
|--------|------|
| 0 | 门控通过 — 所有前置阶段已通过，顺序正确，无活跃 backtrack，可以继续 |
| 1 | 阻断 — 至少一个前置阶段未通过，或顺序错误，或有活跃 backtrack，或参数错误 |

### JSON 输出格式（`--json` 模式）

```json
{
  "passed": true,
  "phase": "03-dev-proposal",
  "prior_phases": ["01-requirements", "02-test-design"],
  "block_reasons": [],
  "phase_state": "first_run",
  "details": {
    "prior_phase_gate": { "passed": true, "missing": [] },
    "timestamp_order": { "passed": true, "order_valid": true },
    "backtrack": { "passed": true, "active_backtrack_phases": [] }
  }
}
```

### 阻断场景示例（`--json` 模式）

```json
{
  "passed": false,
  "phase": "03-dev-proposal",
  "prior_phases": ["01-requirements", "02-test-design"],
  "block_reasons": [
    "前置阶段门控未通过: 缺少 [02-test-design] 的 pass 记录",
    "前置阶段 pass 记录时间戳未按阶段顺序单调递增: 02-test-design (09:00) 早于 01-requirements (10:00)",
    "前置阶段 01-requirements 存在活跃 backtrack 标记 (backtrack_to: 01-requirements)"
  ],
  "phase_state": "first_run",
  "details": {
    "prior_phase_gate": { "passed": false, "missing": ["02-test-design"] },
    "timestamp_order": { "passed": false, "order_valid": false },
    "backtrack": { "passed": false, "active_backtrack_phases": ["01-requirements"] }
  }
}
```

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **TypeScript CLI 子命令**（方案 A） | 与现有的 `eval-log` 共享同一代码库，复用 `eval-json.ts` 中的 `readEvalJson()`、`checkGate()` 和 `workflow.ts` 中的 `getPriorPhases()`，避免逻辑重复。与当前 CLI 架构风格一致 | **方案 B（Python 脚本）**：需重写所有读取和验证逻辑，无法复用 TypeScript lib，导致阶段顺序定义在 Python 和 TypeScript 间重复维护，代码库碎片化。**方案 C（内联到技能）**：检查逻辑在 7 个技能中重复 7 次，没有统一输出格式，不可独立测试 |
| D2 | **核心逻辑提取为纯函数** | `checkPriorPhases`、`checkTimestampOrder`、`checkBacktrack`、`determinePhaseState` 作为纯函数从 CLI action handler 中分离，接受 entries 数组和阶段名称作为参数，不依赖文件系统。可直接在单元测试中构造内存数据测试，覆盖所有逻辑分支 | 将逻辑全部内联在 action handler 中会导致单元测试需要模拟整个 CLI 执行环境，测试编写和维护成本高 |
| D3 | **Timestamp 顺序比较使用 `>=` 而非 `>`** | 同一毫秒内完成的两个阶段评估 timestamp 可能完全相同，使用 `>` 会错误地将相同 timestamp 判定为逆序。`>=` 约定认为相同 timestamp 的顺序可接受 | 使用 `>` 严格比较会导致毫秒级并发写入的场景下误阻断，破坏 `>=` 约定后用户需手动调整 timestamp |
| D4 | **每次调用均重新读取 eval.json，不引入缓存** | eval-check 的调用频率低（每个阶段技能启动时调用一次），直接读取文件系统保证始终使用最新数据。引入缓存会增加复杂性且收益极低 | 内存缓存或文件监听机制：复杂度高，且 eval-check 的调用模式是低频的，缓存带来的性能提升无意义 |
| D5 | **收集所有阻断原因而非短路返回** | 即使门控检查失败，仍继续执行 timestamp 顺序检查和回溯检测，在单次调用中报告所有问题。帮助用户一次性了解全部阻断原因，而非逐个修复反复运行 | 短路返回：发现第一个阻断原因就停止，用户需反复运行多次才能修复所有问题，开发效率低 |
| D6 | **phase_state 不作为阻断条件** | 当前阶段的状态（首次运行/重试/已通过）是信息性报告，不影响是否可继续执行。即使当前阶段已通过，eval-check 仍返回退出码 0 | 将 phase_state 纳入门控逻辑（如已通过则阻断）会导致技能的调用逻辑复杂化，且无法支持重跑已通过阶段的场景 |
| D7 | **在 action handler 中执行变更目录存在性验证** | `getPhasesDir()` 仅做路径字符串拼接，不访问文件系统。若 `--change` 指向不存在的变更名称，对首个阶段调用时 `priorPhases` 为空、`readEvalJson` 返回空数组，所有检查通过但验证逻辑错误。在 action handler 中显式调用 `fs.existsSync(phasesDir)` 可确保在第一个数据访问点就发现并报告无效变更名，与 `getPhaseIndex()` 验证阶段名的模式一致 | **方案 B（在 change.ts 中内建验证）**：修改 `getPhasesDir()` 使其在目录不存在时抛出异常。但 `change.ts` 目前是纯路径计算函数，引入副作用偏离了其单一职责，该行为对其他调用方（如 `eval-log`）可能不期望。**方案 C（在 readEvalJson 中验证）**：修改 `eval-json.ts` 的 `readEvalJson()` 在目录不存在时报错。但 `readEvalJson` 已有空数组返回行为（文件不存在时返回 `[]`），引入副作用破坏其契约的一致性 |

---

## 依赖

### 运行时依赖

- `cac@^6.7.14` — 已有依赖，用于 CLI 参数解析和子命令注册

### 测试依赖（新增）

- `vitest@^3.x` — 单元测试和集成测试框架，与 esbuild 原生兼容，无需额外编译配置
- `execa` — 集成测试中用于在子进程执行 CLI 命令（或使用 Node.js 内置 `child_process`）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 并发写入导致相同 timestamp | 顺序检查可能误判 | 低 | 使用 `>=` 比较，相同 timestamp 视为顺序正确；在文档中记录此约定 |
| 已有变更的 eval.json timestamp 不单调递增 | 已有工作流的变更被阻断 | 中 | eval-check 输出明确指明 timestamp 异常阶段；可通过手动调整 eval.json 中对应 timestamp 修复 |
| 不存在的 change 名称传入 | `getPhasesDir` 返回有效路径但目录实际不存在，首个阶段场景下所有检查跳过，命令错误地返回退出码 0 | 低 | action handler 中调用 `fs.existsSync(phasesDir)` 验证目录存在性，不存在时 stderr 输出有意义的错误消息并退出码 1 |
| 非法 phase 名称传入 | 门控检查跳过或误判 | 低 | 通过 `getPhaseIndex()` 验证 phase 合法性，非法名称输出错误信息并退出码 1 |
| eval.json schema 版本不兼容 | 命令失效或误判 | 低 | 读取时检查 `schema_version`，不匹配时输出警告但不阻断；schema 升级时同步更新 |

---

## 迁移步骤

1. 创建 `plugins/dev-team/bin/src/commands/eval-check.ts`，包含核心检查函数和 CLI action handler
2. 在 `plugins/dev-team/bin/src/index.ts` 中导入并调用 `registerEvalCheckCommand(cli)`
3. 添加 `vitest` 到 `plugins/dev-team/bin/package.json` 的 devDependencies
4. 创建单元测试 `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts`
5. 创建集成测试 `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts`
6. 在 `plugins/dev-team/bin/package.json` 的 scripts 中添加 `"test": "vitest run"` 和 `"test:watch": "vitest"`
7. 运行测试套件验证所有 AC 覆盖

---

## 待决问题

- 暂无疑问
