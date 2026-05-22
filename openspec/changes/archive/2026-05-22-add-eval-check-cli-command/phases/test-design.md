# 测试设计: add-eval-check-cli-command

> **变更**: add-eval-check-cli-command
> **日期**: 2026-05-21
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | eval-check 核心逻辑：前置阶段门控检查、timestamp 顺序验证、当前阶段状态判定、回溯标记检测。这些纯函数从 CLI action handler 中提取，可独立测试 | vitest | 分支覆盖率 >= 90%，覆盖门控/顺序/回溯/状态判定的所有逻辑分支 |
| 集成测试 | eval-check CLI 命令的完整执行链路：参数解析、退出码、--json 输出格式与字段完整性、错误消息 | vitest + execa | 覆盖 CLI 入口到 stdout/stderr 输出及退出码的完整链路，每个 AC 验证退出码和输出内容 |
| 手动验证脚本 | 开发期间快速验证 eval-check 命令在模拟变更目录上的行为，以及未来技能集成时的端到端门控流程 | Node.js 脚本（无框架依赖） | 作为开发辅助工具，不纳入 CI 自动化套件 |

> **注**：vitest 需要作为 devDependency 添加到 `plugins/dev-team/bin/package.json`。建议使用 vitest v3.x，与 esbuild 原生兼容，无需额外编译配置。

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 门控通过 |
| AC-2 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 门控阻断并列出缺失阶段 |
| AC-3 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — timestamp 顺序验证 |
| AC-4 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts` | 集成测试 | 输出格式 — JSON 可解析 |
| AC-5 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts` | 集成测试 | 输出结构 — 必填字段存在且类型正确 |
| AC-6 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 回溯标记阻断 |
| AC-7 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 当前阶段状态（首次运行） |
| AC-8 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 当前阶段状态（重试） |
| AC-9 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 功能 — 当前阶段状态（已通过） |
| AC-10 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts` | 单元测试 | 边界 — 首个阶段无前置检查 |
| AC-11 | `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts` | 集成测试 | 错误处理 — 不存在的 change 名称 |

---

## 3. 测试策略

### 3.1 方法

采用分层测试策略，按隔离程度分为两个层级：

- **单元测试层**：将 eval-check 的核心业务逻辑从 CLI action handler 中提取为纯函数（接受 entries 数组和 phase 名称作为参数，返回结构化结果对象），在内存中构造不同状态的 eval entries 数据进行测试。所有文件系统调用（readEvalJson、resolveChangeDir）通过 vitest 的 `vi.mock()` 模拟。
- **集成测试层**：通过 execa 在子进程中执行 `node dev-team-bundle.js eval-check ...`，传入真实参数组合，验证 stdout/stderr 内容和进程退出码。测试前在临时目录中构造模拟的 eval.json 文件，通过 `--change` 指向该目录。

### 3.2 测试分类

- **单元测试** (`eval-check.unit.test.ts`)：
  - 测试 `checkPriorPhases(entries, priorPhases)` — 门控检查函数
  - 测试 `checkTimestampOrder(entries, priorPhases)` — 顺序验证函数
  - 测试 `checkBacktrack(entries, priorPhases)` — 回溯标记检测函数
  - 测试 `determinePhaseState(entries, currentPhase)` — 阶段状态判定函数
  - 测试 `buildEvalCheckResult(options)` — 结果组装函数
  - 不涉及文件系统，所有输入在测试中直接构造

- **集成测试** (`eval-check.integration.test.ts`)：
  - 在 `os.tmpdir()` 中创建临时变更目录结构（`phases/eval.json`）
  - 通过 execa 调用 `node ${bundlePath} eval-check --change <tmpDir> --phase <phase> [--json]`
  - 验证退出码（0 或 1）
  - 验证 stdout 内容（--json 模式下 JSON.parse 是否成功，字段完整性）
  - 验证 stderr 内容（错误消息的人类可读性）
  - 测试后清理临时目录

- **手动验证脚本** (`manual-eval-check.js`)：
  - 提供交互式或预设场景的 eval-check 调用
  - 用于开发期间快速迭代和未来技能集成的端到端流程验证
  - 不纳入 CI 自动化套件

### 3.3 模拟策略

| 组件 | 单元测试 | 集成测试 |
|------|----------|----------|
| `readEvalJson` | `vi.mock()` 返回内存中构造的 mock entries 数组 | 真实文件系统，在临时目录创建 eval.json |
| `resolveChangeDir` / `getPhasesDir` | `vi.mock()` 返回任意字符串，不实际访问文件系统 | 真实路径，指向临时目录 |
| `process.exit` | `vi.spyOn(process, 'exit').mockImplementation()` 捕获退出码 | 子进程自然退出，通过 `exitCode` 获取退出码 |
| `console.log` / `console.error` | `vi.spyOn(console, 'log/error').mockImplementation()` 捕获输出 | 子进程 stdout/stderr 通过 execa 捕获 |
| `process.argv` | 不适用（单元测试直接调用核心函数） | 通过 execa 参数传递，不 mock |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 空 eval.json（从未执行过任何评估） | entries=[], phase=03-dev-proposal | passed=false, missing=[01-requirements, 02-test-design], exit code 1 | `eval-check.unit.test.ts` |
| 首个阶段运行 eval-check | phase=01-requirements, entries 任意或为空 | prior_phases 为空, passed=true, exit code 0, 不检查任何前置阶段 | `eval-check.unit.test.ts` |
| 前置阶段 pass 时间戳完全相等 | 01-requirements.pass.ts=2026-05-21T10:00:00.000Z, 02-test-design.pass.ts=2026-05-21T10:00:00.000Z | 视为顺序正确（>= 约定），exit code 0 | `eval-check.unit.test.ts` |
| 前置阶段 pass 时间戳明显逆序 | 02-test-design.pass.ts=2026-05-21T09:00:00.000Z, 01-requirements.pass.ts=2026-05-21T10:00:00.000Z | passed=false, block_reasons 包含 timestamp ordering 错误描述, exit code 1 | `eval-check.unit.test.ts` |
| 前置阶段最新条目包含 backtrack_to（活跃回溯） | 01-requirements 最新条目: {verdict: "pass", backtrack_to: "01-requirements"} | passed=false, block_reasons 包含 "backtrack" 描述, exit code 1 | `eval-check.unit.test.ts` |
| 前置阶段非最新条目包含 backtrack_to（已修复的回溯） | 01-requirements 有 2 条: [条目1: backtrack_to="01-requirements", 条目2: backtrack_to=null]，条目2 timestamp 更新 | backtrack 已解决, passed=true, exit code 0 | `eval-check.unit.test.ts` |
| 当前阶段有多条条目，最新为 pass | 03-dev-proposal 有 3 条: [fail, fail, pass] | phase_state="passed", exit code 0 | `eval-check.unit.test.ts` |
| 当前阶段有多条条目，最新为 fail（重试场景） | 03-dev-proposal 有 2 条: [pass, fail] | phase_state="retry", exit code 0（仅报告，不阻断） | `eval-check.unit.test.ts` |
| 当前阶段无任何条目（首次执行） | entries 中不含当前阶段 phase 的条目 | phase_state="first_run", exit code 0（仅报告，不阻断） | `eval-check.unit.test.ts` |
| 五个前置阶段全部缺失 | phase=06-code-review, entries 为空 | missing=[01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen, 05-implementation], exit code 1 | `eval-check.unit.test.ts` |
| 非法 phase 名称 | --phase invalid-phase, entries 任意 | 不合法 phase 名称，输出有意义错误信息，exit code 1 | `eval-check.integration.test.ts` |
| 不存在的 change 名称 | --change non-existent-change --phase 01-requirements | getPhasesDir 返回不存在路径，readEvalJson 返回空数组（门控无前置阶段时通过），或在 resolve 阶段报错。验证退出码和错误消息有意义 | `eval-check.integration.test.ts` |
| eval.json schema_version 不匹配 | eval.json 中 schema_version="0.9"，与当前 SCHEMA_VERSION="1.0" 不匹配 | 输出警告信息但继续执行，不阻断 | `eval-check.unit.test.ts` |
| 仅有一个前置阶段缺失 | 01-requirements 有 pass, 02-test-design 无 pass, phase=03-dev-proposal | passed=false, missing=[02-test-design], exit code 1 | `eval-check.unit.test.ts` |
| --json 模式下同时输出人类可读信息和 JSON | --json 标志 + 所有前置阶段通过 | stdout 最后一行或整体为合法 JSON，包含 passed=true 等字段 | `eval-check.integration.test.ts` |

---

## 5. 测试数据

### 5.1 单元测试数据

单元测试使用内存中构造的 eval entries 数组，遵循 eval.json 的 schema 格式：

```typescript
// 通过的 eval entries（典型场景）
const passedEntries = [
  { phase: "01-requirements", timestamp: "2026-05-21T10:00:00.000Z", verdict: "pass",
    attempt: 1, report: "All requirements met", items: [], backtrack_to: null,
    schema_version: "1.0" },
  { phase: "02-test-design", timestamp: "2026-05-21T11:00:00.000Z", verdict: "pass",
    attempt: 1, report: "Test design approved", items: [], backtrack_to: null,
    schema_version: "1.0" },
];

// 包含 backtrack 的 entries
const backtrackEntries = [
  { phase: "01-requirements", timestamp: "2026-05-21T10:00:00.000Z", verdict: "pass",
    attempt: 1, report: "All requirements met", items: [], backtrack_to: "01-requirements",
    schema_version: "1.0" },
];

// 时间戳逆序的 entries
const outOfOrderEntries = [
  { phase: "02-test-design", timestamp: "2026-05-21T09:00:00.000Z", verdict: "pass",
    attempt: 1, report: "Test design", items: [], backtrack_to: null, schema_version: "1.0" },
  { phase: "01-requirements", timestamp: "2026-05-21T10:00:00.000Z", verdict: "pass",
    attempt: 1, report: "Requirements", items: [], backtrack_to: null, schema_version: "1.0" },
];
```

### 5.2 集成测试数据

集成测试在 `os.tmpdir()` 中创建临时目录结构：

```
<tmpdir>/eval-check-test-<random>/phases/eval.json
```

eval.json 内容根据测试场景动态生成，通过 `fs.writeFileSync` 写入。测试使用 `afterEach` / `afterAll` 清理临时目录。

### 5.3 阶段顺序定义

测试中使用的阶段顺序常量直接引用 `workflow.ts` 中的 `PHASES` 数组：

```
01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen,
05-implementation, 06-code-review, 07-acceptance
```

---

## 6. 不可测试项

| 项目 | 原因 |
|------|------|
| eval-check 被真实阶段技能调用时的门控效果 | 技能之间的集成涉及 Claude Agent 运行时决策，无法在确定性自动化测试中重现。通过手动验证脚本和技能开发期间的交互测试覆盖 |
| 时钟回拨或系统时间跳变导致的时间戳异常 | 时间戳由操作系统提供，超出 eval-check 命令的控制范围。单元测试已覆盖相同时间戳（>= 约定）的处理，极端时钟异常在真实场景中概率极低 |
| eval.json 文件被并发写入时的竞态条件 | eval.json 的写入和读取操作没有文件锁。这是现有 eval-log 命令的既有问题，不在本变更范围内解决。本变更不引入新的并发写入路径 |
| schema 升级后 eval-check 的兼容性 | eval-check 读取时检查 schema_version 并发出警告，但无法测试对未来 schema 的兼容性。schema 升级发生时需同步更新 eval-check 并补充对应的测试用例 |
