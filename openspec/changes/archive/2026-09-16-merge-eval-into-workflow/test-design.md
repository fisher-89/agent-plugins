# 测试设计: merge-eval-into-workflow

> **日期**: 2026-09-11

测试框架：`vite-plus`（`test_detect_frameworks`），断言库与运行器为 `vite-plus/test`（`describe` / `it` / `expect` / `vi`）。单元测试文件与被测源文件同目录同名（`*.test.ts`），路径由 `test_resolve_paths` 解析（范围 `plugins/dev-team/bin/src/**/*.{ts,tsx}`，排除 `bin/src/schemas/**`）。集成测试文件统一置于 `plugins/dev-team/bin/__tests__/<关系目录>/<关系目录>.test.ts`。

本文档描述的是**实现完成后的目标态**，不是当前磁盘状态：`plugins/dev-team/bin/src/lib/change-config.ts` 仍保留 `DEFAULT_WORKFLOW_TYPE` 兜底，`plugins/dev-team/bin/src/lib/eval-json.ts` 的 `writeEvalJson` 仍 `mkdirSync` 并以缺省元数据补建文件，`plugins/dev-team/bin/src/mcp.ts` 的 `change_create` description 仍缺「唯一创建者」契约；相关测试需按下文用例改造（含标注为「废弃」的旧断言）。`plugins/dev-team/bin/src/hooks.ts` 与 `plugins/dev-team/bin/src/commands/change-*.ts` 已接近目标态，但 `hooks.test.ts` 尚缺 `eval.json` 不再被内置规则 deny 的反转用例。

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | `phase_log` 成功后 change 目录存在 `workflow.json.eval` 数组新条目，且 **不** 创建/更新 `eval.json` | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`plugins/dev-team/bin/src/commands/phase-log.ts`、`phase_log / backtrack → writeEvalJson → workflow.json.eval` |
| AC-2 | 已有 `{ workflow_type, created }` 的文件在追加 eval 后两字段值不变，未知键仍在 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`phase_log / backtrack → writeEvalJson → workflow.json.eval` |
| AC-3 | 仅有旧 `eval.json`、`workflow.json` 无 `eval` 键时，`phase_next` / `change_list` 行为与合并前一致 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`plugins/dev-team/bin/src/commands/phase-next.ts`、`plugins/dev-team/bin/src/commands/change-list.ts`、`readEvalJson 优先级 → phase_next / change_list`、`旧条目兼容 → phase_next`、`backtrack → phase_next 回溯原因传播` |
| AC-4 | 对同时存在两文件的 change 执行 `phase_log` 或 `backtrack` 后，`eval.json` 不存在，条目在 `workflow.json.eval` | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`plugins/dev-team/bin/src/commands/phase-log.ts`、`plugins/dev-team/bin/src/commands/backtrack.ts`、`phase_log / backtrack → writeEvalJson → workflow.json.eval`、`phase_log 拒绝回溯字段 → backtrack 缺条目文案` |
| AC-5 | `workflow.json.eval` 为 `[]` 且磁盘上仍有旧 `eval.json` 时，读取结果为空数组（不合并两源） | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`readEvalJson 优先级 → phase_next / change_list` |
| AC-6 | 新建 change 只有 `workflow.json`（`workflow_type`+`created`），无 `eval` 键，无 `eval.json`；该工具是 `workflow.json` 的唯一创建者 | 单元测试、集成测试 | `plugins/dev-team/bin/src/commands/change-create.ts`、`change_create → 无 eval 键 → readEvalJson / phase_next` |
| AC-7 | 即使已有评估历史，`artifacts` 也不包含 `eval.json`；`latest_phase`/`workflow_done` 仍正确 | 单元测试、集成测试 | `plugins/dev-team/bin/src/commands/change-list.ts`、`readEvalJson 优先级 → phase_next / change_list` |
| AC-8 | Write/Edit/Bash/PowerShell/Shell/StrReplace 写 `openspec/changes/<name>/workflow.json` 被 deny；MCP 进程内 `fs` 写入仍成功 | 单元测试、集成测试 | `plugins/dev-team/bin/src/hooks.ts`、`PreToolUse deny → Node fs 写 workflow.json` |
| AC-9 | 内置 glob 仅为 `**/openspec/changes/**/workflow.json` 与 `**/openspec/config.json`；未配置 `write_protection` 时对 `openspec/changes/<name>/eval.json` 的 agent 写入**不再**产生内置 deny | 单元测试、集成测试 | `plugins/dev-team/bin/src/hooks.ts`、`PreToolUse deny → Node fs 写 workflow.json` |
| AC-10 | `phase_log` / `backtrack` 的 description 提及 `workflow.json`，不再把 `eval.json` 当作写入目标 | 单元测试 | `plugins/dev-team/bin/src/mcp.ts` |
| AC-11 | `phase-proposal` 无 Write `workflow.json` 步骤，缺文件时停止并指引（不手写）；evaluator/skill 文案指向 `phase_log` → `workflow.json` | 集成测试 | `Skill / agent 文案 → phase_log / workflow.json` |
| AC-12 | `workflow.json.eval` 为对象（非数组）时读取抛错；`change_list.workflow_done` 为 false 且不崩溃 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/eval-json.ts`、`plugins/dev-team/bin/src/commands/change-list.ts`、`readEvalJson 优先级 → phase_next / change_list` |
| AC-13 | `getWorkflowType()` 在文件缺失时抛错而**不再**返回 `"requirement"`；`phase_next` / `backtrack` 以可读错误终止（文案含绝对路径与 `change_create` 指引）；`phase_log` 同样失败且**不创建**任何文件 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/change-config.ts`、`plugins/dev-team/bin/src/commands/phase-next.ts`、`plugins/dev-team/bin/src/commands/backtrack.ts`、`plugins/dev-team/bin/src/lib/eval-json.ts`、`同一非法 workflow.json → 严格方抛错 / 容错方不崩溃` |
| AC-14 | JSON 非法、根非对象、`workflow_type` 缺失或非枚举、`eval` 非数组、`created` 非 `YYYY-MM-DD` 时 `getWorkflowType` / `writeEvalJson` 抛错且不写入；`change_list` 对同一文件返回 `workflow_done: false`、`latest_phase: null` 且不崩溃 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/change-config.ts`、`plugins/dev-team/bin/src/lib/eval-json.ts`、`plugins/dev-team/bin/src/commands/change-list.ts`、`同一非法 workflow.json → 严格方抛错 / 容错方不崩溃` |

---

## 单元测试

### plugins/dev-team/bin/src/lib/eval-json.ts -> plugins/dev-team/bin/src/lib/eval-json.test.ts

#### 待测功能

- `readEvalJson(changeDir: string): EvalEntry[]`: 权威读 `workflow.json.eval`（经 `workflowEvalSchema` 解析）；无该数组时只读回退遗留 `eval.json`；两者皆无返回 `[]`；`eval` 非数组 / 根非对象 / JSON 非法时抛错；禁止两源数组合并；文件缺失不抛错。
- `writeEvalJson(changeDir: string, entries: EvalEntry[]): void`: 要求 `workflow.json` **既存且通过 `workflowFileSchema`**，否则抛错且不写盘；把完整 `entries` 覆盖写入 `eval` 并保留 `workflow_type`/`created`/未知键；成功后 `unlinkSync` 遗留 `eval.json`；MUST NOT `mkdirSync`、MUST NOT 新建文件、MUST NOT 补写缺省元数据。
- `appendEntry(changeDir: string, entry: EvalEntry): void`: `readEvalJson` + push + `writeEvalJson`；不再探测 / 创建目录或文件。
- `buildEntry(params: BuildEntryParams): EvalEntry`: 纯内存组条，本变更不改。
- `computeAttempt(entries: EvalEntry[], phase: string, explicitAttempt?: number): number`: 纯内存，本变更不改。
- `markPhaseStale(entries: EvalEntry[], phaseId: string, workflowType: string): void`: 纯内存，本变更不改。
- `validateVerdict(verdict: string, skipped?: boolean): void`: 纯内存，本变更不改。
- `validateReportLength(report: string): void`: 纯内存，本变更不改。

`validateVerdict` / `buildEntry` / `markPhaseStale` 沿用既有用例（本变更不触碰其算法）。下列用例覆盖本变更改写的 IO 契约与「文件既存且合法」前置条件；夹具条目须符合 `phaseLogSchema`（含合法 `phase` 枚举）。使用 `fs.mkdtempSync` 真实文件系统，不 mock `fs`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| readEvalJson | 正向 | `workflow.json.eval` 含一条合法条目 → 返回该数组，且不读取同目录 `eval.json`（即使后者条目不同）（AC-1） | 新增 |
| readEvalJson | 正向 | `workflow.json` 无 `eval` 键、遗留 `eval.json` 为合法数组 → 返回遗留数组（AC-3） | 新增 |
| readEvalJson | 正向 | `workflow.json` 不存在、仅有合法 `eval.json` → 返回遗留数组（AC-3） | 新增 |
| readEvalJson | 正向 | `workflow.json` 仅有 `{ workflow_type, created }` 且无遗留文件 → 返回 `[]` | 新增 |
| readEvalJson | 正向 | `workflow.json.eval` 为 `[]` 且遗留 `eval.json` 非空 → 返回 `[]`，不合并遗留条目（AC-5） | 新增 |
| readEvalJson | 正向 | 两文件皆不存在（change 目录存在）→ 返回 `[]`，不抛错 | 新增 |
| readEvalJson | 异常 | `workflow.json.eval` 为对象 `{}` → 抛错，message 含 `workflow.json.eval 必须是数组`，不回退 `eval.json`（AC-12） | 新增 |
| readEvalJson | 异常 | `workflow.json.eval` 为 `null` / 字符串 / 数字 → 抛错，不回退 | 新增 |
| readEvalJson | 异常 | `workflow.json` 根为数组 → 抛错（根必须是对象），不回退 | 新增 |
| readEvalJson | 异常 | `workflow.json` 非法 JSON → 抛错，message 含解析失败语义，不回退 | 新增 |
| readEvalJson | 异常 | `eval` 数组元素不符合 `phaseLogSchema`（非法 `phase` 枚举 / 缺必填）→ 抛 Zod 校验错误 | 新增 |
| readEvalJson | 异常 | 走遗留路径且 `eval.json` 根为对象 → 抛错，message 含 `eval.json 根元素必须是数组` | 新增 |
| readEvalJson | 异常 | 遗留 `eval.json` 非法 JSON → 抛错 | 新增 |
| readEvalJson | 异常 | `changeDir` 为 `undefined` / `null`（None）→ 抛 TypeError 或路径错误，不得静默返回遗留数据 | 新增 |
| readEvalJson | 边界 | `changeDir` 为空串 `''` → 不崩溃；按「文件不存在」返回 `[]`，不得读到无关文件 | 新增 |
| readEvalJson | 边界 | `changeDir` 超长（>1000 chars）→ 不崩溃；文件不存在则 `[]` | 新增 |
| readEvalJson | 边界 | `changeDir` 含 `\n` / emoji 等特殊字符 → 不崩溃 | 新增 |
| readEvalJson | 边界 | `eval` 为单元素数组 → 返回长度 1 | 新增 |
| readEvalJson | 边界 | `eval` 为超大列表（>100 条合法条目）→ 返回等长数组且顺序不变 | 新增 |
| readEvalJson | 边界 | `workflow.json` 键为 `"Eval"` / `"entries"`（大小写或别名）而非 `"eval"` → 视为无权威数组，回退遗留 `eval.json` | 新增 |
| readEvalJson | 边界 | `eval` 键缺失（Optional None）且无遗留文件 → `[]` | 新增 |
| readEvalJson | 边界 | 含 `eval` 数组且含未知键（如 `note`）→ 返回条目数组，未知键不影响 | 新增 |
| writeEvalJson | 正向 | 已有 `{ workflow_type, created }` 写入一条 → 两字段值不变，`eval` 长度 1，不创建 `eval.json`（AC-1、AC-2） | 新增 |
| writeEvalJson | 正向 | 已有未知键 `"note": "x"` → 写回后 `"note"` 仍为 `"x"`（AC-2） | 新增 |
| writeEvalJson | 正向 | 同目录存在遗留 `eval.json` → 写成功后 `eval.json` 不存在，条目仅在 `workflow.json.eval`（AC-4） | 新增 |
| writeEvalJson | 正向 | 传入完整数组覆盖磁盘 `eval` 字段，不与遗留 `eval.json` 数组合并 | 新增 |
| writeEvalJson | 正向 | 序列化为 2 空格缩进且文件以换行结尾 | 新增 |
| writeEvalJson | 异常 | `workflow.json` 不存在 → 抛错，message 含绝对路径与 `change_create` 指引；目录与文件均未被创建（AC-13） | 新增 |
| writeEvalJson | 异常 | 已有 `workflow.json` 根为数组 → 抛错，文件内容不被改写，遗留 `eval.json`（若存在）不被删除（AC-14） | 新增 |
| writeEvalJson | 异常 | 已有 `workflow.json` 非法 JSON → 抛错，不删除遗留 `eval.json`（AC-14） | 新增 |
| writeEvalJson | 异常 | `workflow_type` 缺失 / 非枚举 / 非字符串 → 抛错且不写入（AC-14） | 新增 |
| writeEvalJson | 异常 | `created` 为 `2026/09/11`（非 `YYYY-MM-DD`）→ 抛错且不写入（AC-14） | 新增 |
| writeEvalJson | 异常 | 磁盘 `eval` 为对象 `{}`（经 `workflowFileSchema` 校验）→ 抛错且不写入 | 新增 |
| writeEvalJson | 异常 | `entries` 为 `undefined` / `null`（None）→ 抛错或写入失败，不得写成非数组 `eval` | 新增 |
| writeEvalJson | 异常 | `changeDir` 为 `undefined` / `null`（None）→ 抛 TypeError 或路径错误，不得写出 `eval.json` | 新增 |
| writeEvalJson | 边界 | `changeDir` 为空串 `''` → 不崩溃；文件不存在故抛错，不得把权威数组写到无关的 `eval.json` | 新增 |
| writeEvalJson | 边界 | `changeDir` 超长（>1000 chars）→ 不崩溃；不得再 `mkdirSync` 超长目录，抛路径错误即视为通过 | 新增 |
| writeEvalJson | 边界 | `changeDir` 含 `\n` / emoji 等特殊字符 → 不崩溃；按 `path.join` 处理，成功则只写该目录下 `workflow.json.eval` | 新增 |
| writeEvalJson | 边界 | `entries=[]`（空数组）→ `workflow.json.eval` 为 `[]`；遗留 `eval.json` 仍被删除（权威空数组优先，AC-5） | 新增 |
| writeEvalJson | 边界 | `entries` 单元素 → `eval.length===1` | 新增 |
| writeEvalJson | 边界 | `entries` 超大列表（>100 条）→ 写回长度与顺序一致 | 新增 |
| writeEvalJson | 边界 | 已有文件缺少 `created`（Optional None）→ 写成功且**不得**补写该键 | 新增 |
| writeEvalJson | 边界 | 同目录本无 `eval.json` → 写成功且不因 unlink 抛错 | 新增 |
| writeEvalJson | 边界 | 已有文件含多余字段（未知键）→ 全部保留 | 新增 |
| writeEvalJson | 废弃 | 文件缺失时以缺省 `workflow_type:"requirement"` + 当日 `created` 补建 `workflow.json` | 废弃 |
| writeEvalJson | 废弃 | `changeDir` 不存在时 `mkdirSync({ recursive: true })` 后写入成功 | 废弃 |
| writeEvalJson | 废弃 | 把 `entries` 根数组 `writeFileSync` 到 `eval.json` | 废弃 |
| appendEntry | 正向 | 仅有 `{ workflow_type, created }` 时追加一条 → `eval.length===1` 且不产生 `eval.json`（AC-1） | 新增 |
| appendEntry | 正向 | 仅有遗留 `eval.json` 时追加 → 新数组含旧条目+新条目，落在 `workflow.json.eval`，`eval.json` 被删除（AC-4） | 新增 |
| appendEntry | 正向 | 权威 `eval` 已有条目时再追加 → 旧条目保留，长度 +1 | 新增 |
| appendEntry | 异常 | `readEvalJson` 因 `eval` 非数组抛错 → `appendEntry` 向上抛出，不写盘 | 新增 |
| appendEntry | 异常 | `workflow.json` 缺失 → 抛错且不创建任何文件 / 目录（AC-13） | 新增 |
| appendEntry | 异常 | `entry` 为 `undefined` / `null`（None）→ 抛错 | 新增 |
| appendEntry | 异常 | `changeDir` 为 `undefined` / `null`（None）→ 抛 TypeError 或路径错误，不得创建 `eval.json` | 新增 |
| appendEntry | 边界 | `changeDir` 为空串 `''` → 不崩溃；按「文件不存在」抛错，不得写无关 `eval.json` | 新增 |
| appendEntry | 边界 | `changeDir` 超长（>1000 chars）→ 不崩溃；不得 `mkdir` 超长路径，抛 `ENAMETOOLONG` 即视为通过 | 新增 |
| appendEntry | 边界 | `changeDir` 含 `\n` / emoji 等特殊字符 → 不崩溃；成功则只在该目录追加 `workflow.json.eval` | 新增 |
| appendEntry | 边界 | 连续两次追加同一结构 → 长度为 2（非覆盖） | 新增 |
| appendEntry | 边界 | `entry` 含多余未知字段 → 随数组写入（JSON 原样保留可枚举字段） | 新增 |
| appendEntry | 废弃 | 首次追加（无文件）时创建 `workflow.json` 并带缺省元数据 | 废弃 |
| appendEntry | 废弃 | 不存在 `eval.json` 时创建独立 `eval.json` 数组文件 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实临时目录 | `fs.mkdtempSync` 创建 change 目录；`afterEach` `rmSync`；直接 `writeFileSync` 布置 `workflow.json` / `eval.json` | readEvalJson / writeEvalJson / appendEntry 全部路径 |
| `fs` 模块 | **不** `vi.mock('fs')`，以便覆盖「写后 unlink 遗留文件」「缺文件不建目录 / 不建文件」等真实文件系统契约 | IO 契约 |
| `Date` | 不再需要：缺省 `created` 补建分支已删除，`created` 一律由夹具显式给出 | — |

---

### plugins/dev-team/bin/src/lib/change-config.ts -> plugins/dev-team/bin/src/lib/change-config.test.ts

#### 待测功能

- `getWorkflowType(change: string): string`: 文件必须存在 → `JSON.parse` → 根必须是对象 → `workflowFileSchema.parse` → 返回 `workflow_type`；任一环节失败抛错（文案含绝对路径；缺文件另含 `change_create` 指引）；**不再**缺省 `"requirement"`；`eval` 数组与未知键不影响读取。

现有 `DEFAULT_WORKFLOW_TYPE` 兜底路径的用例（缺文件 / 缺字段 / 空串 → `"requirement"`）全部转为「废弃」，并以抛错断言替换。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getWorkflowType | 正向 | `{ workflow_type: "test-only", created: "2026-09-11", eval: [合法条目] }` → 返回 `"test-only"`，不因 `eval` 抛错 | 新增 |
| getWorkflowType | 正向 | `{ workflow_type: "requirement", eval: [] }` → 返回 `"requirement"` | 新增 |
| getWorkflowType | 正向 | 含未知键 `"note": "x"` → 忽略未知键，返回 `workflow_type` | 新增 |
| getWorkflowType | 正向 | 4 值枚举逐一（`requirement` / `bug-fix` / `refactor` / `test-only`）→ 原样返回（Enum 全覆盖） | 新增 |
| getWorkflowType | 异常 | `workflow.json` 不存在 → 抛错，message 含绝对路径与 `change_create` 指引，**不返回** `"requirement"`（AC-13） | 新增 |
| getWorkflowType | 异常 | 非法 JSON → 抛 `workflow.json 解析失败`（AC-14） | 新增 |
| getWorkflowType | 异常 | 根为数组（即便元素像 eval 条目）→ 抛错，message 含 `根元素必须是对象`（AC-14） | 新增 |
| getWorkflowType | 异常 | 根为 `null` / 标量（字符串 / 数字）→ 抛根类型错误（AC-14） | 新增 |
| getWorkflowType | 异常 | `{}`（缺 `workflow_type`）→ 抛格式非法，message 含字段路径 `workflow_type` 与绝对路径（AC-14） | 新增 |
| getWorkflowType | 异常 | `workflow_type: "unknown"` / `""`（空串）/ `123`（非字符串）→ 抛格式非法（AC-14） | 新增 |
| getWorkflowType | 异常 | `created: "2026/09/11"` → 抛格式非法（AC-14） | 新增 |
| getWorkflowType | 异常 | `eval: {}`（非数组）→ 抛格式非法（完整文件校验生效），不返回 `workflow_type`（AC-14） | 新增 |
| getWorkflowType | 异常 | `change` 为 `undefined` / `null`（None）→ 抛错 | 新增 |
| getWorkflowType | 边界 | 缺 `workflow_type` 但有 `eval` 数组 → 抛错（不再回退缺省） | 新增 |
| getWorkflowType | 边界 | `workflow_type` 超长（>1000 chars）→ 非枚举 → 抛格式非法（无「非空即采用」路径） | 新增 |
| getWorkflowType | 边界 | `created` 缺失（Optional None）→ 正常返回 `workflow_type`（`created` 可选） | 新增 |
| getWorkflowType | 边界 | `change` 为空串 `''` → 不崩溃；文件不存在则抛缺文件错误，不得读到其他 change 的 `workflow_type` | 新增 |
| getWorkflowType | 边界 | `change` 超长（>1000 chars）→ 不崩溃；缺文件则抛缺文件错误 | 新增 |
| getWorkflowType | 边界 | `change` 含 `\n` / emoji 等特殊字符 → 不崩溃；按 `getChangeDir` 拼路径 | 新增 |
| getWorkflowType | 废弃 | 文件缺失时返回 `"requirement"` | 废弃 |
| getWorkflowType | 废弃 | 缺 `workflow_type` 键时返回 `"requirement"` | 废弃 |
| getWorkflowType | 废弃 | `workflow_type` 为空串时返回 `"requirement"` | 废弃 |
| getWorkflowType | 废弃 | `eval` 为对象（非法形状）时**仍**返回 `workflow_type`（旧「本函数不校验 eval」断言） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs.existsSync` / `fs.readFileSync` | 沿用既有 `mockWorkflowJson`：`endsWith('workflow.json')` 返回夹具字符串；缺文件类用例让 `existsSync` 对该路径返回 `false` 并断言抛错 | getWorkflowType 全部 |
| `getChangeDir` | `vi.mock('./change')` 固定 `/tmp/test-change`（缺文件用例的 message 断言依赖该固定绝对路径） | 全部 |

---

### plugins/dev-team/bin/src/commands/phase-log.ts -> plugins/dev-team/bin/src/commands/phase-log.test.ts

#### 待测功能

- `runPhaseLog(options: PhaseLogOptions): PhaseLogResult`: 签名不变；经 `readEvalJson` + `appendEntry` 追加到 `workflow.json.eval`；读失败文案 `读取 workflow.json 失败:`、写失败文案 `写入 workflow.json 失败:`；MUST NOT 直接写 `eval.json`；缺 `workflow.json` 时内层错误（含 `change_create` 指引）随包装透出，且不新建任何文件。

既有无回溯参数、verdict 推导、`skipped`、report 超长、`appendEntry` 调用次数等用例保留。本变更补错误文案、严格失败路径与「不写 eval.json」契约。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runPhaseLog | 正向 | checklist 全 pass → 调用 `appendEntry` 一次，返回 `{ written: true, phase, attempt }`，不调用 `writeEvalJson`（仍只走 append）（AC-1） | 新增 |
| runPhaseLog | 正向 | 不把 `eval.json` 作为 `appendEntry` 之外的写入目标（无直接 `writeFileSync('eval.json')`） | 新增 |
| runPhaseLog | 正向 | `skipped=true` 且 checklist 全 pass → entry 含 `skipped:true`，`appendEntry` 仍被调用一次 | 新增 |
| runPhaseLog | 正向 | 同一 phase 已有一条 fail → `attempt` 为 2（既有 attempt 推导） | 新增 |
| runPhaseLog | 异常 | mock `readEvalJson` 抛错 → 抛 `读取 workflow.json 失败:` + 原因 | 新增 |
| runPhaseLog | 异常 | mock `appendEntry` 抛错（模拟缺 `workflow.json`）→ 抛 `写入 workflow.json 失败:`，message 含内层 `change_create` 指引，且未创建文件（AC-13） | 新增 |
| runPhaseLog | 异常 | `checklist` 含任一项 fail → verdict 为 `fail`（既有），`appendEntry` 入参 verdict 为 `fail` | 新增 |
| runPhaseLog | 异常 | `report` 501 字符 → 抛长度错误且不调用 `appendEntry` | 新增 |
| runPhaseLog | 异常 | `options` 缺 `checklist` 等必填（运行时传入残缺对象，None）→ 校验阶段抛错，不写盘 | 新增 |
| runPhaseLog | 边界 | `report` 空串 `''` → 通过长度校验并 append | 新增 |
| runPhaseLog | 边界 | `report` 恰 500 字符 → 通过；501 字符仍抛错 | 新增 |
| runPhaseLog | 边界 | `checklist=[]` 且未 skip → verdict 为 pass（every 空数组）并 append | 新增 |
| runPhaseLog | 边界 | `checklist` 单元素 pass → verdict 为 pass；`checklist` 超大列表（>100 项全 pass）→ 仍 append 一次 | 新增 |
| runPhaseLog | 边界 | `attempt` 显式传 `0` / `-1` → 抛「attempt 必须为正整数」，不写盘（`computeAttempt` 既有契约） | 新增 |
| runPhaseLog | 边界 | `phase` 非法枚举（如 `"integration-test"`）→ `buildEntry` 的 `phaseLogSchema.parse` 抛错，不写盘 | 新增 |
| runPhaseLog | 边界 | `change` 为空串 `''` → 不崩溃；读路径失败被包装为 `读取 workflow.json 失败:` | 新增 |
| runPhaseLog | 边界 | `change` 超长（>1000 chars）/ 含 emoji → 不崩溃，错误被包装而非进程退出 | 新增 |
| runPhaseLog | 废弃 | 读 / 写失败 message 匹配 `读取 eval.json 失败` / `写入 eval.json 失败` | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `../lib/eval-json` | 沿用 `vi.mock`：`readEvalJson` 默认 `[]`，`appendEntry` / `writeEvalJson` 为 `vi.fn`；异常用例改为 `mockImplementation` 抛错 | runPhaseLog |
| `getChangeDir` | mock 固定 change 目录 | 全部 |
| `fs` | 沿用部分 mock；本文件不测真实落盘（落盘见集成测试 `eval-store-persist`） | 既有路径 |

---

### plugins/dev-team/bin/src/commands/phase-next.ts -> plugins/dev-team/bin/src/commands/phase-next.test.ts

#### 待测功能

- `runPhaseNext(options: PhaseNextOptions): PhaseNextResult`: 签名不变；先经严格 `getWorkflowType(change)`（抛错**不得**被吞）；随后只读 `readEvalJson`；`anchor = entries.length`；读失败文案 `Failed to read workflow.json:`；`round_limit` 提示检查 `workflow.json` 中的 backtrack 记录；MUST NOT 调用 `writeEvalJson` / 删除 `eval.json`。
- `hasPhasePassed(entries: EvalEntry[], phaseId: string): boolean`: 算法不变（忽略 `stale`）。

既有的 session / round / 回溯 prompt / retry 用例保留。`next()` harness 的 fs mock 必须从「`endsWith('eval.json')` 提供条目」改为「`workflow.json` 对象携带 `eval`」，并保留仅有遗留 `eval.json` 的回退用例。`runPhaseNext — workflow.json default` 整段（缺文件 / 缺字段仍按 `requirement` 表）转为废弃并改写为抛错断言。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runPhaseNext | 正向 | `workflow.json.eval` 为 `[]` 且无遗留文件 → requirement 返回 `proposal`（first run，AC-3） | 新增 |
| runPhaseNext | 正向 | 仅有遗留 `eval.json`（proposal 非 stale pass）、`workflow.json` 无 `eval` 键 → `next_phase` 为 `dev-design`（与合并前一致，AC-3） | 新增 |
| runPhaseNext | 正向 | `workflow.json.eval` 为 `[]` 且遗留 `eval.json` 含 proposal pass → 仍为 first run `proposal`（不合并两源，AC-5） | 新增 |
| runPhaseNext | 正向 | `workflow.json.eval` 含 proposal 非 stale pass → `dev-design`，且 `last_result.phase==="proposal"` | 新增 |
| runPhaseNext | 正向 | 成功路径不调用 `writeEvalJson`，`workflow.json` / `eval.json` 内容调用前后字节级不变（只读） | 新增 |
| runPhaseNext | 异常 | `readEvalJson` 抛错（`eval` 为对象）→ message 匹配 `Failed to read workflow.json:`（AC-12） | 新增 |
| runPhaseNext | 异常 | `workflow.json` 缺失 → `getWorkflowType` 抛错直接终止，message 含绝对路径与 `change_create` 指引，**不**返回 `proposal`（AC-13） | 新增 |
| runPhaseNext | 异常 | `workflow.json` 根非对象 / JSON 非法 / `workflow_type` 非枚举 → 抛错终止，不回退 `requirement`（AC-14） | 新增 |
| runPhaseNext | 异常 | `created` 为 `2026/09/11` → 抛错终止（AC-14） | 新增 |
| runPhaseNext | 异常 | `change` 为空串 `''` → 抛 `Missing required parameter: change` | 新增 |
| runPhaseNext | 异常 | change 目录不存在 → 抛 `Change "..." does not exist` | 新增 |
| runPhaseNext | 边界 | `run_id` 为空 / 纯空白 → `missing_run_id`（既有） | 新增 |
| runPhaseNext | 边界 | round 超过 20 → `round_limit_exceeded`，message 含 `workflow.json` 且不含 `eval.json` | 新增 |
| runPhaseNext | 边界 | 生涯已有多条、窗口算法仍以 `entries.length` 为 anchor（条目来自 `workflow.json.eval`） | 新增 |
| runPhaseNext | 边界 | `workflow.json` 含未知键（如 `note`）→ 类型仍可解析并按 phase 表推进 | 新增 |
| runPhaseNext | 边界 | `workflow_type` 为 `test-only` → 使用 test-only 表（既有） | 新增 |
| runPhaseNext | 边界 | `created` 缺失（Optional None）→ 正常解析并按 phase 表推进 | 新增 |
| runPhaseNext | 边界 | `eval` 键缺失且无遗留文件（Optional None）→ first run `proposal` | 新增 |
| runPhaseNext | 废弃 | 读失败 message 为 `Failed to read eval.json:` | 废弃 |
| runPhaseNext | 废弃 | `round_limit` 文案为 `请检查 eval.json 中的 backtrack 记录` | 废弃 |
| runPhaseNext | 废弃 | `workflow.json` 缺失 / 缺 `workflow_type` 时按缺省 `requirement` 表返回 `proposal`（`runPhaseNext — workflow.json default` 整段） | 废弃 |
| runPhaseNext | 废弃 | fs mock 仅 `endsWith('eval.json')` 提供评估数组、且无权威 `eval` 对照 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs.existsSync` / `readFileSync` | `next()` harness 改为：change 目录返回 `true`；`workflow.json` 返回含 `workflow_type`（可含 `eval` 数组）的对象；仅「遗留回退」用例让 `eval.json` 返回数组 | runPhaseNext 存储来源 |
| `writeFileSync` / `unlinkSync` | `vi.spyOn` 断言评估存储路径未被调用（只读契约） | 只读用例 |
| `../lib/eval-json` | 仅用于 `readEvalJson` 抛错的包装文案用例（`mockImplementationOnce`）；其余用例走真实 `readEvalJson` + fs mock，避免双源优先级被跳过 | 读失败 / 优先级 |
| `getChangeDir` | 沿用既有固定 change 路径 mock | 工作流表 |

---

### plugins/dev-team/bin/src/commands/backtrack.ts -> plugins/dev-team/bin/src/commands/backtrack.test.ts

#### 待测功能

- `runBacktrack(options: BacktrackOptions): BacktrackResult`: 签名不变；`getWorkflowType` 的抛错（缺文件 / 格式非法）原样透出；`readEvalJson` + 原地改写最新条目 + `markPhaseStale` + `writeEvalJson`；读 / 写失败包装为 `读取 workflow.json 失败:` / `写入 workflow.json 失败:`；无条目文案 `Phase "…" 没有评估条目，无法设置回溯。请先执行该 phase 并记录评估结果。`；MUST NOT 直接写 `eval.json`。

既有目标合法性、stale 传播、幂等用例保留（仍 mock `readEvalJson` / `writeEvalJson`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runBacktrack | 正向 | 合法回溯 → 调用 `writeEvalJson` 一次且数组长度不变；不出现对 `eval.json` 的 `writeFileSync`（AC-4） | 新增 |
| runBacktrack | 正向 | 修改后最新条目 `backtrack_to` / `backtrack_reason` 值正确 | 新增 |
| runBacktrack | 正向 | 目标 phase 最新 pass 被标记 `stale:true` 并向下游传播（既有，条目来源改为 `workflow.json.eval`） | 新增 |
| runBacktrack | 正向 | 返回 `{ modified: true, phase, target }` | 新增 |
| runBacktrack | 正向 | 多 workflow 类型（`requirement` / `test-only`）均正常（既有） | 新增 |
| runBacktrack | 异常 | 该 phase 无任何评估条目 → 抛错，message 含 `没有评估条目`，**不含** `eval.json` | 新增 |
| runBacktrack | 异常 | mock `readEvalJson` 抛错 → `读取 workflow.json 失败:` + 原因 | 新增 |
| runBacktrack | 异常 | mock `writeEvalJson` 抛错 → `写入 workflow.json 失败:` + 原因 | 新增 |
| runBacktrack | 异常 | `getWorkflowType` 抛错（`workflow.json` 缺失）→ 原样透出，message 含 `change_create` 指引，不写盘（AC-13） | 新增 |
| runBacktrack | 异常 | `getWorkflowType` 抛错（JSON 非法 / 根非对象 / `workflow_type` 非枚举）→ 原样透出，不写盘（AC-14） | 新增 |
| runBacktrack | 异常 | `writeEvalJson` 因文件缺失拒绝（真实前置条件）→ 抛错且不创建文件（AC-13） | 新增 |
| runBacktrack | 异常 | `backtrack_to` 为未来 phase → 既有错误，不写盘 | 新增 |
| runBacktrack | 异常 | `backtrack_to` 不在 phase 表中 → 既有错误，不写盘 | 新增 |
| runBacktrack | 边界 | `backtrack_reason` 为空串 `''` → schema 允许（`max(500)` 无 min）时写出；若命令层拒绝则不写盘（以源码为准） | 新增 |
| runBacktrack | 边界 | `backtrack_reason` 恰 500 字符 → 通过；501 字符 → schema 拒绝 | 新增 |
| runBacktrack | 边界 | `backtrack_to` 为 `null` / `undefined`（None）→ schema `min(1)` 拒绝，不写盘 | 新增 |
| runBacktrack | 边界 | `phase` 为空串 `''` → schema `min(1)` 拒绝，不写盘 | 新增 |
| runBacktrack | 边界 | `backtrack_to` 等于当前 phase → 仍允许（既有） | 新增 |
| runBacktrack | 废弃 | 无条目错误匹配 `/没有 eval.json 条目/` | 废弃 |
| runBacktrack | 废弃 | 读失败匹配 `/读取 eval.json 失败/` | 废弃 |
| runBacktrack | 废弃 | 「写时自动补建 `workflow.json`」作为 backtrack 的成功前置（缺文件现在必须失败） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `../lib/eval-json` | `readEvalJson` / `writeEvalJson` 为 `vi.fn`，默认返回内存条目数组 | 正常 / 异常包装 |
| `../lib/change-config` | `vi.mock`：`getWorkflowType` 默认 `"requirement"` 或按用例切换；严格化用例改为 `mockImplementation` 抛错 | 目标合法性 / 严格透出 |
| `getChangeDir` | 固定 `/tmp/test-change` | 全部 |
| 真实 fs | 本文件不测 unlink 与「不建文件」；见集成测试 `eval-store-persist`、`同一非法 workflow.json → 严格方抛错 / 容错方不崩溃` | AC-4、AC-13 |

---

### plugins/dev-team/bin/src/commands/change-list.ts -> plugins/dev-team/bin/src/commands/change-list.test.ts

#### 待测功能

- `runChangeList(projectRoot: string): ChangeListResult`: 签名不变；`KNOWN_ARTIFACTS` 为 `proposal.md`、`design.md`、`tasks.md`、`test-design.md`（不含 `eval.json`，也不含 `workflow.json`）；`readWorkflowType` 改用 `workflowFileSchema.safeParse`（删除手工 `isPlainObject` / `typeof` 判断）；`latest_phase` / `workflow_done` 经 `readEvalJson`；`workflow.json` 缺失或校验失败 → `latest_phase: null`、`workflow_done: false`，该 change 仍出现在列表中，**不抛错**。

`writeChange` 夹具在本变更后：`evalEntries` 应写入 `workflow.json.eval`（经 `writeEvalJson`）；若同时传 `workflowJson`，必须把 `eval` 合并进同一对象，避免后写的 `workflow.json` 覆盖权威数组。非法形状继续用独立文件或残缺 `eval` 字段表达。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeList | 正向 | `workflow.json.eval` 非空且存在 `proposal.md` → `artifacts` 含 `proposal.md`，**不含** `eval.json`；`latest_phase.phase` 为 timestamp 最新条目（AC-7） | 新增 |
| runChangeList | 正向 | 目录仍有遗留 `eval.json` 文件 → `artifacts` 仍不含它；`latest_phase` 可经回退填出（AC-7） | 新增 |
| runChangeList | 正向 | 全 phase 非 stale pass 位于 `workflow.json.eval` → `workflow_done===true` | 新增 |
| runChangeList | 正向 | 4 值枚举逐一（`requirement` / `bug-fix` / `refactor` / `test-only`）→ 各自按对应 phase 表计算（Enum 全覆盖） | 新增 |
| runChangeList | 异常 | `workflow.json.eval` 为对象 `{}` → 不崩溃，该 change 仍出现，`workflow_done===false`，`latest_phase===null`（AC-12） | 新增 |
| runChangeList | 异常 | 遗留 `eval.json` 根非数组 → 同上吞错（不崩溃） | 新增 |
| runChangeList | 异常 | `workflow.json` 缺失 → `workflow_done===false`、`latest_phase===null`、不抛错（AC-13 容错面） | 新增 |
| runChangeList | 异常 | `workflow.json` JSON 非法 / 根为数组 / `workflow_type` 非枚举 → 同上（AC-14 容错面） | 新增 |
| runChangeList | 异常 | `created` 为 `2026/09/11` → `workflow_done===false`、不崩溃（AC-14 容错面） | 新增 |
| runChangeList | 异常 | `projectRoot` 为 `undefined` / `null`（None）→ 抛错或空列表，不崩溃进程 | 新增 |
| runChangeList | 边界 | `readEvalJson` 返回 `[]`（无 `eval` 键、无遗留文件）→ `workflow_done===false`，`latest_phase===null` | 新增 |
| runChangeList | 边界 | 仅有 `"eval": []` → `workflow_done===false`，`latest_phase===null` | 新增 |
| runChangeList | 边界 | `artifacts` 仅为已知四文件之子集；`workflow.json` 自身不列入 artifacts | 新增 |
| runChangeList | 边界 | `projectRoot` 空串 `''` / 超长（>1000 chars）/ 含 emoji → 既有 project_root 契约仍成立（空目录或无 changes 目录 → `count===0`） | 新增 |
| runChangeList | 边界 | `created` 缺失（Optional None）+ 未知键 → 正常计算 `workflow_done` | 新增 |
| runChangeList | 边界 | `openspec/changes` 目录不存在 → `changes: []`、`count===0`（既有） | 新增 |
| runChangeList | 废弃 | `KNOWN_ARTIFACTS` 含 `'eval.json'` 导致磁盘上有该文件时 artifacts 包含它 | 废弃 |
| runChangeList | 废弃 | 夹具把评估历史只写到 `eval.json` 且断言 artifacts 含 `eval.json` | 废弃 |
| runChangeList | 废弃 | `workflow_type` 为非 4 值枚举（如 `"unknown"` / `"custom"`）时仍按该字符串查 phase 表（现改为 `safeParse` 失败 → `null` / `false`） | 废弃 |
| runChangeList | 废弃 | `created` 格式非法时仍照常计算 `workflow_done`（现改为校验失败 → `false`） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实临时目录 | 沿用 `createTempProject` + `writeChange`；评估数据优先写 `workflow.json.eval`（`evalEntries` 与 `workflowJson` 同时给出时必须合并进同一对象） | 全部 |
| `writeEvalJson` | 夹具调用真实函数布置权威存储；非法形状改手写 JSON（`invalidEvalJson` 仍写遗留 `eval.json`） | AC-12 / 遗留回退 |

---

### plugins/dev-team/bin/src/commands/change-create.ts -> plugins/dev-team/bin/src/commands/change-create.test.ts

#### 待测功能

- `runChangeCreate(name: string, projectRoot: string, workflowType: string): { name: string; path: string }`: 签名与行为不变（只写 `{ workflow_type, created }`，已存在目录拒绝），仅补「唯一创建者 / MUST NOT 写 `eval` 键 / MUST NOT 创建 `eval.json`」契约注释；产出文件须能通过 `workflowFileSchema`（写方与读方同源）。

既有 kebab-case、长度、4 值 `workflow_type` 用例保留。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeCreate | 正向 | 新建成功 → `workflow.json` 仅有 `workflow_type` 与 `created`，`Object.prototype.hasOwnProperty.call(doc,'eval')===false`，`eval.json` 不存在（AC-6） | 新增 |
| runChangeCreate | 正向 | 4 值枚举逐一（`requirement` / `test-only` / `bug-fix` / `refactor`）→ 均无 `eval` 键、无 `eval.json`（Enum 全覆盖） | 新增 |
| runChangeCreate | 正向 | 产出文件可被 `workflowFileSchema` 解析（唯一创建者契约的读方视角） | 新增 |
| runChangeCreate | 正向 | `created` 匹配 `^\d{4}-\d{2}-\d{2}$` 且等于当日（既有断言保留） | 新增 |
| runChangeCreate | 异常 | 目录已存在 → 抛 `已存在`；既有 `workflow.json` 不出现新的 `eval` 键；不创建 `eval.json`（AC-6） | 新增 |
| runChangeCreate | 异常 | 非法 kebab-case / 超长 name（>128）→ 抛错且不创建目录，故无 `eval.json` | 新增 |
| runChangeCreate | 异常 | `name` 或 `projectRoot` 为 `undefined` / `null`（None）→ 抛错 | 新增 |
| runChangeCreate | 边界 | `name` 空串 `''` → kebab 校验失败，无文件 | 新增 |
| runChangeCreate | 边界 | `name` 恰 128 字符 → 创建成功且仍无 `eval` 键 | 新增 |
| runChangeCreate | 边界 | `name` 超长（>1000 chars）/ 含 emoji / 含 `\n` → kebab 校验失败，不创建目录 | 新增 |
| runChangeCreate | 边界 | `workflowType` 空串 `''` → 命令层原样写入该字符串（MCP 输入 schema 层已拒绝空串），断言不因此补 `eval` 键；严格化后该文件的 `getWorkflowType` 会抛错（边界，不作为合法态） | 新增 |
| runChangeCreate | 边界 | 紧凑 `JSON.stringify` + 末尾换行的现行格式保留；解析后键集合为 `workflow_type`、`created` | 新增 |
| runChangeCreate | 边界 | `projectRoot` 为空串 `''` → 不崩溃（递归 mkdir 的相对路径行为，既有） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实临时目录 | 沿用 `createTempProject` + 真实 `fs`（无 mock） | 全部 |

---

### plugins/dev-team/bin/src/hooks.ts -> plugins/dev-team/bin/src/hooks.test.ts

#### 待测功能

- `runProtectFiles(): void`: `loadPatterns` 内置集合 = `**/openspec/changes/**/workflow.json`（reason 含 `workflow.json` 并提示 `phase_log` / `backtrack` / `change_create`，支持 `%s` / `%t`）+ `**/openspec/config.json`；**MUST NOT** 含 `**/openspec/changes/**/eval.json`；用户 `write_protection.files` 合并逻辑不变（相对 glob 仍加 `**/` 前缀）。经 stdin 的 Write / Edit / StrReplace / Bash / Shell / PowerShell 命中内置 glob 即 deny；未配置 `write_protection` 时对 `eval.json` 不再产生内置 deny。
- `captureStderr()` / `runStaticCheck()` / `main()`: 本变更不改调度与静态检查。

既有 `workflow.json` 与 `config.json` 的 deny 矩阵保留（实现已到位，实现阶段仅复核），本变更加 `eval.json` 的反转用例（AC-9）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runProtectFiles | 正向 | Write `openspec/changes/test/workflow.json`、无 `write_protection` → `permissionDecision==="deny"`，reason 含 `workflow.json` 且提示 `phase_log` / `backtrack` / `change_create`（AC-8） | 新增 |
| runProtectFiles | 正向 | Edit / StrReplace 同一 `workflow.json` 路径 → deny，reason 含工具名 | 新增 |
| runProtectFiles | 正向 | Bash/Shell：`> ` / `>>` / `tee` / heredoc / `>|` / `>&` 写入 `openspec/changes/x/workflow.json` → deny | 新增 |
| runProtectFiles | 正向 | PowerShell：`Set-Content` / `Out-File` / `Add-Content` / `Export-Csv` / `Export-CliXml` / `Tee-Object` / `>` / `>>` / `*>` / `[System.IO.File]::WriteAllText` 写 `workflow.json` → deny | 新增 |
| runProtectFiles | 正向 | Write `openspec/changes/test/eval.json`、无 `write_protection` → 内置 glob **不单独** deny（输出 allow）（AC-9） | 新增 |
| runProtectFiles | 正向 | Bash `> openspec/changes/x/eval.json` / PowerShell `Set-Content ... eval.json` → allow（内置集合不含该文件）（AC-9） | 新增 |
| runProtectFiles | 正向 | Write `openspec/config.json` 仍 deny，reason 含 config.json 语义（AC-8） | 新增 |
| runProtectFiles | 正向 | Write `openspec/changes/test/proposal.md` → allow（内置 glob 不单独 deny） | 新增 |
| runProtectFiles | 正向 | `write_protection.files` 含 glob `openspec/changes/*/eval.json` → 同一 `eval.json` 路径 deny（用户可自行保护，与内置集合区分） | 新增 |
| runProtectFiles | 异常 | stdin 空串 / 无效 JSON / 缺 `tool_name` → fail-open allow（既有） | 新增 |
| runProtectFiles | 异常 | Write 缺 `file_path` 或 `file_path` 为 `null` / 数字（None / 非 str）→ allow | 新增 |
| runProtectFiles | 边界 | `file_path` 空串 `''` → allow | 新增 |
| runProtectFiles | 边界 | `file_path` 超长（>1000 chars）且以 `/workflow.json` 结尾并匹配 glob → deny | 新增 |
| runProtectFiles | 边界 | 路径含 emoji 的 change 名 + `workflow.json` → 仍按 glob 匹配 deny | 新增 |
| runProtectFiles | 边界 | 用户 `write_protection.files` 为空数组 → 内置 `workflow.json` / `config.json` 仍生效 | 新增 |
| runProtectFiles | 边界 | Bash `cat` / PowerShell `Get-Content` 只读 `workflow.json` → allow | 新增 |
| runProtectFiles | 边界 | 行首 `node` / `python` 写受保护路径仍豁免 allow（既有豁免不得因新 glob 取消） | 新增 |
| runProtectFiles | 边界 | reason 中 `%s` / `%t` 被替换，序列化可 `JSON.parse` | 新增 |
| runProtectFiles | 边界 | 未知工具名（如 `Read`）→ allow（fail-open，既有） | 新增 |
| runProtectFiles | 废弃 | Write `openspec/changes/test/eval.json` 在内置规则下 deny（旧「过渡期保护遗留文件」语义） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| stdin / `fs.readFileSync` | 沿用 hooks 测试 harness：`mockReadFileSync` 返回 tool_use JSON | runProtectFiles |
| `readConfig` | 默认 `{ schema: 'spec-driven' }`（无 `write_protection`）以锁内置集合；用户保护用例改为返回 `write_protection.files` | 内置集合 / 用户 glob |
| stdout | 捕获 `runProtectFiles` 打印的 JSON，断言 `permissionDecision` 与 reason | deny / allow |

---

### plugins/dev-team/bin/src/mcp.ts -> plugins/dev-team/bin/src/mcp.test.ts

#### 待测功能

- `connectToServer(transport: Transport): Promise<McpServer>`: 注册工具名与 input schema 不变；`phase_log` / `backtrack` 的 description 指向 `workflow.json`（不再把 `eval.json` 当作写入目标）；`change_create` description 补「`workflow.json` 的唯一创建者；缺文件时读取方报错」；`change_list` 在无评估条目 / `workflow.json` 缺失或非法时 `workflow_done===false` 且不崩溃。

`EXPECTED_TOOL_DESCRIPTIONS` 须与 `mcp.ts` 新字面量同步（当前测试仍写 `eval.json`），精确 `toBe` 断言保留。`writeDoneChange` 夹具的评估历史须写入 `workflow.json.eval`（同一对象合并），不得再写独立 `eval.json`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| connectToServer — description | 正向 | `listTools` 中 `phase_log.description` 与注册串字节级相等，且包含 `workflow.json`，不以 `eval.json` 为写入目标（AC-10） | 新增 |
| connectToServer — description | 正向 | `backtrack.description` 同上，提及 `workflow.json` 评估条目（AC-10） | 新增 |
| connectToServer — description | 正向 | `change_create.description` 含唯一创建者契约（缺文件时读取方报错 / `change_create` 是唯一创建者）（AC-10） | 新增 |
| connectToServer — description | 正向 | 工具名集合仍含 `phase_log` / `phase_next` / `backtrack` / `change_list` / `change_create`，无重命名 | 新增 |
| connectToServer — change_list | 正向 | 无评估条目（仅 `{ workflow_type, created }`）→ `workflow_done===false` 且不崩溃 | 新增 |
| connectToServer — change_list | 正向 | 全 phase pass 写在 `workflow.json.eval`（不再写独立 `eval.json`）→ `workflow_done===true` | 新增 |
| connectToServer — change_list | 正向 | `workflow.json` 缺失 → 该 change 仍列出且 `workflow_done===false`（AC-13 容错面） | 新增 |
| connectToServer — phase_log | 正向 | 经 MCP 调用 `phase_log`（临时项目 + 已有 `workflow.json`）→ 条目落 `workflow.json.eval`，目录内不产生 `eval.json`（AC-1） | 新增 |
| connectToServer | 异常 | `phase_log` / `backtrack` 缺必填字段 → Zod 拒参，与现行 input schema 一致 | 新增 |
| connectToServer | 异常 | `change_create` 传入非法枚举 `workflow_type` → Zod 拒参（引用 `workflowTypeSchema` 后值域不变） | 新增 |
| connectToServer | 异常 | `eval` 非法（对象）时 `change_list` 仍返回该 change 且 `workflow_done===false`（AC-12） | 新增 |
| connectToServer | 异常 | `workflow.json` 非法 JSON 时 `change_list` 不崩溃（AC-14 容错面） | 新增 |
| connectToServer | 边界 | `phase_log.description` 非空串；断言 `toContain('workflow.json')` | 新增 |
| connectToServer | 边界 | description 超长仍为单一字符串（无占位符残留） | 新增 |
| connectToServer | 边界 | 无评估条目的 `latest_phase` 为 `null` | 新增 |
| connectToServer | 边界 | `phase_log` 的 `tool_input` 含 `backtrack_to` → 被 schema 忽略 / 拒绝（既有行为保留） | 新增 |
| connectToServer | 废弃 | `EXPECTED_TOOL_DESCRIPTIONS.phase_log` 为 `… to eval.json …` | 废弃 |
| connectToServer | 废弃 | `EXPECTED_TOOL_DESCRIPTIONS.backtrack` 为 `… in eval.json …` | 废弃 |
| connectToServer | 废弃 | `writeDoneChange` 把评估数组写入独立 `eval.json` 作为 `workflow_done===true` 的唯一来源 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| InMemoryTransport + 命令 spy | 沿用既有 MCP 测试：`connectToServer` 真注册；handler 侧 spy `runPhaseLog` / `runBacktrack` 等 | description / 拒参 |
| 临时项目目录 | `setupTempProject`；评估历史写入 `workflow.json` 的 `eval` 字段（与 `workflowJson` 合并为同一对象） | workflow_done / phase_log 落盘 |
| `resolve` project root | `setResolvedRoot(dir)` | change_list 调用 |

---

## 集成测试

### phase_log / backtrack → writeEvalJson → workflow.json.eval → `plugins/dev-team/bin/__tests__/eval-store-persist/eval-store-persist.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 写入触发方：校验后经 `appendEntry` 追加 |
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 写入触发方：原地改写后经 `writeEvalJson` 持久化 |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 权威 IO：写 `workflow.json.eval`、保留元数据、成功后删除遗留 `eval.json` |

**关联AC**: AC-1, AC-2, AC-4, AC-13

**关系描述**:

命令层不直接 `writeFileSync` 评估文件，统一经 `appendEntry` / `writeEvalJson` 落盘。集成测试在真实临时 change 目录上调用 `runPhaseLog` 与 `runBacktrack`（不 mock `eval-json`），验证 CLI / MCP 共用入口把条目写入 `workflow.json.eval`、保留 `workflow_type` / `created` / 未知键、成功后删除遗留 `eval.json`，且过程中永不创建或更新 `eval.json` 作为权威文件；另外验证写入前置条件收紧后，缺 `workflow.json` 时命令失败且不补建文件。出错模式包括：只改了命令文案却仍写旧文件；写 `eval` 时覆盖元数据；写失败仍 unlink；双文件并存时合并数组而非覆盖；缺文件时旧 `mkdirSync` + 缺省元数据路径把 `workflow.json` 悄悄补出来。

#### 场景: phase_log 追加到已有元数据文件

前置为仅含 `{ workflow_type, created, note }` 的 `workflow.json`、无 `eval.json`。输入为合法 `runPhaseLog`（proposal / 全 pass checklist）。预期 `eval.length===1`、元数据与 `note` 不变、目录无 `eval.json`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | pass 追加后 `workflow.json.eval[0].verdict==="pass"` 且无 `eval.json`（AC-1、AC-2） | 新增 |
| 正向 | 同 phase 再 fail 一次 → 长度为 2，旧条目保留 | 新增 |
| 异常 | 预先写好的 `eval` 为对象 → 抛 `读取 workflow.json 失败`，文件未被改写成合法数组 | 新增 |
| 边界 | `checklist=[]` 未 skip → 仍追加一条 pass | 新增 |

<!-- 模块间真实调用，无跨进程 Mock -->

#### 场景: 双文件并存时写入迁移

前置为 `workflow.json` 无 `eval` 键，且遗留 `eval.json` 含至少一条合法条目。分别执行 `runPhaseLog` 与合法 `runBacktrack`。预期权威数组在 `workflow.json.eval`（log 为追加后的全集；backtrack 为改写后的等长数组），`eval.json` 不存在。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `phase_log` 后遗留文件删除，新条目在 `eval` 数组末尾（AC-4） | 新增 |
| 正向 | `backtrack` 后遗留文件删除，`eval` 长度不变且最新条目含 `backtrack_to`（AC-4） | 新增 |
| 异常 | 先把 `workflow.json` 改成非法 JSON 再 backtrack → 抛写 / 读错误，`eval.json` 仍在，`workflow.json` 内容未改写 | 新增 |
| 边界 | 权威已是 `[]` 时 `phase_log` → 结果只有新条目，不把旧 `eval.json` 内容合并进去再删除旧文件 | 新增 |

#### 场景: 缺 workflow.json 时写入失败且不补建

前置为 change 目录存在但**没有** `workflow.json`（模拟未经 `change_create` 的目录）。分别调用 `runPhaseLog` 与 `runBacktrack`。预期两者均抛错（文案含绝对路径与 `change_create` 指引），且目录内既不出现 `workflow.json` 也不出现 `eval.json`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | `runPhaseLog` 抛 `写入 workflow.json 失败` 且内层含 `change_create` 指引，目录条目数不变（AC-13） | 新增 |
| 异常 | `runBacktrack` 抛错且不创建文件（AC-13） | 新增 |
| 边界 | 目录本身不存在（未 `mkdir`）→ 抛错且不 `mkdirSync` 创建该目录（AC-13） | 新增 |
| 废弃 | 缺文件时 `writeEvalJson` 以缺省 `workflow_type:"requirement"` + 当日 `created` 补建成功 | 废弃 |

<!-- 全部真实 fs；无 Mock -->

---

### readEvalJson 优先级 → phase_next / change_list → `plugins/dev-team/bin/__tests__/eval-store-read-priority/eval-store-read-priority.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 读取优先级与校验（`workflow.json.eval` 优先，遗留回退，禁止合并） |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 只读消费者：`next_phase` / session anchor=`entries.length` |
| `plugins/dev-team/bin/src/commands/change-list.ts` | 只读消费者：`latest_phase` / `workflow_done` / `artifacts` |

**关联AC**: AC-3, AC-5, AC-7, AC-12

**关系描述**:

`phase_next` 与 `change_list` 都只通过 `readEvalJson` 取条目，自身不得迁移或删除文件。集成测试用真实磁盘组合「仅遗留文件 / 权威空数组 + 遗留非空 / 权威非数组 / 两文件皆无」验证只读行为与列表字段。出错模式包括：`phase_next` 误写盘或误删遗留文件；空 `eval` 仍合并遗留导致 `next_phase` 跳过 proposal；`change_list` 把遗留 `eval.json` 列进 `artifacts`；非法 `eval` 形状让列表抛错而非 `workflow_done=false`。

#### 场景: 仅遗留 eval.json 的只读兼容

前置为 `workflow.json` 只有 `workflow_type` / `created`（无 `eval` 键），`eval.json` 含 proposal 的非 stale pass。调用 `runPhaseNext` 与 `runChangeList`。预期与合并前一致：下一阶段为 `dev-design`；`latest_phase.phase==="proposal"`；`artifacts` 不含 `eval.json`；两文件内容调用前后字节级不变。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `phase_next` 返回 `dev-design` 且不写盘（AC-3） | 新增 |
| 正向 | `change_list` 填出 `latest_phase` 且 artifacts 无 `eval.json`（AC-7） | 新增 |
| 异常 | 遗留根为对象 → `phase_next` 抛 `Failed to read workflow.json`（包装链含根类型错误）；`change_list` 不抛，`workflow_done===false` | 新增 |
| 边界 | 两文件皆无（`workflow.json` 仅元数据、无遗留文件）→ `phase_next` 返回 first run `proposal`，`change_list.latest_phase===null`（AC-3） | 新增 |

#### 场景: 权威空数组忽略遗留文件

前置为 `workflow.json.eval=[]` 且 `eval.json` 含多条 pass。预期读取为空：`phase_next` 回到 first run（`proposal`）；`change_list.latest_phase===null` 且 `workflow_done===false`；磁盘两文件仍在（只读，不删除遗留）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `eval===[]` 挡住遗留非空数组（AC-5） | 新增 |
| 正向 | `phase_next` 不 `unlink` 遗留 `eval.json`（AC-3） | 新增 |
| 异常 | `eval` 为 `{}` → `phase_next` 抛错；`change_list` 该 change 仍在列表且 `workflow_done===false`（AC-12） | 新增 |
| 边界 | `eval===[]` 且无遗留文件 → 与「无评估条目」相同（`workflow_done===false`、`latest_phase===null`） | 新增 |

<!-- 真实 fs，无跨进程 Mock -->

---

### change_create → 无 eval 键 → readEvalJson / phase_next → `plugins/dev-team/bin/__tests__/eval-store-change-create/eval-store-change-create.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/change-create.ts` | 创建方：只写 `{ workflow_type, created }`，不写 `eval` 键、不建 `eval.json` |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 读取方：无 `eval` 键时回退遗留 `eval.json`，否则 `[]` |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 读取方：空历史 first run `proposal` |

**关联AC**: AC-6, AC-3, AC-13

**关系描述**:

`change_create` 若写入 `"eval": []`，会按读优先级挡住遗留 `eval.json`，使进行中 change 的历史被静默丢弃；若它补写缺省元数据，又会与「唯一创建者」契约冲突。集成测试在真实项目根调用 `runChangeCreate`，再 `readEvalJson` / `runPhaseNext`，确认新建 change 无 `eval` 键、无 `eval.json`、读取为 `[]`、下一 phase 为 `proposal`。第二场景用「手工创建无 `eval` 键的 `workflow.json` + 遗留 `eval.json`」对照（`change_create` 对已存在目录会拒绝，无法直接复现），验证读取走回退。出错模式是 create 写空数组导致回退失效，或 create 之后读方因缺元数据抛错。

#### 场景: 新建 change 无评估历史

前置为空白临时 `projectRoot`。输入 `runChangeCreate('my-change', root, 'requirement')`。预期目录内只有 `workflow.json` 两字段；`readEvalJson` 返回 `[]`；`runPhaseNext` 返回 `proposal`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 创建后无 `eval` 键、无 `eval.json`，`readEvalJson` 为 `[]`（AC-6） | 新增 |
| 正向 | 随后 `phase_next` 为 first run `proposal`（`getWorkflowType` 严格读取通过）（AC-13 反证） | 新增 |
| 异常 | 第二次 `change_create` 同名 → 抛已存在，不补 `eval`、不建 `eval.json` | 新增 |
| 边界 | 手工在已有无 `eval` 键文件旁放遗留 `eval.json` → `readEvalJson` 返回遗留数组（解释为何 create 不得写空 `eval`，AC-3） | 新增 |

<!-- 真实 fs，无跨进程 Mock -->

---

### 同一非法 workflow.json → 严格方抛错 / 容错方不崩溃 → `plugins/dev-team/bin/__tests__/eval-store-strict-workflow/eval-store-strict-workflow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/change-config.ts` | 严格类型读取：四步校验，任一失败抛错（含绝对路径与 `change_create` 指引） |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 严格消费者：`getWorkflowType` 抛错直接透出，不吞、不退化 |
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 严格消费者：类型解析 / 写前置失败即终止 |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 写入前置校验：文件缺失或格式非法即拒绝写入且不改写磁盘 |
| `plugins/dev-team/bin/src/commands/change-list.ts` | 容错消费者：同一非法文件返回 `workflow_done:false`、`latest_phase:null`，不崩溃 |

**关联AC**: AC-13, AC-14

**关系描述**:

本关系验证「一个非法文件、两种失败语义」的跨模块契约：读 / 写关键路径必须硬失败并给出可操作指引，而列表路径必须软失败以保持 change 可见。集成测试在同一个真实临时 change 目录上，对同一份缺失 / 非法的 `workflow.json` 依次调用各入口，断言彼此不矛盾。出错模式包括：`phase_next` / `backtrack` 把 `getWorkflowType` 的抛错 catch 掉后退化为缺省 `requirement`；`phase_log` 在缺文件时仍补建文件或目录；`change_list` 因同一文件抛错导致整个列表失败；错误文案丢失绝对路径或 `change_create` 指引。

#### 场景: workflow.json 缺失

前置为 change 目录存在但无 `workflow.json`。对同一目录依次调用 `runPhaseNext`、`runBacktrack`、`runPhaseLog`、`runChangeList`。预期前三者抛错且不创建任何文件，`runChangeList` 返回该 change（`workflow_done:false`、`latest_phase:null`）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `runChangeList` 仍列出该 change，`workflow_done===false`、`latest_phase===null`（AC-13 容错面） | 新增 |
| 异常 | `runPhaseNext` 抛错，message 含绝对路径与 `change_create` 指引，**不返回** `proposal`（AC-13） | 新增 |
| 异常 | `runBacktrack` 抛错，message 含绝对路径与 `change_create` 指引（AC-13） | 新增 |
| 异常 | `runPhaseLog` 抛错，且目录内未新增 `workflow.json` / `eval.json` / 子目录（AC-13） | 新增 |
| 边界 | 四次调用后目录内容与调用前完全一致（严格方只读失败，无副作用） | 新增 |

#### 场景: workflow.json 格式非法

前置为同一目录分别布置 5 种非法内容：非法 JSON、根为数组、`{}`（缺 `workflow_type`）、`workflow_type:"unknown"`、`created:"2026/09/11"`（以及 `eval: {}`）。每种内容下重复上述四入口断言。预期 `getWorkflowType` 族入口抛错（message 指出出错的字段路径），`writeEvalJson` 拒绝改写，`change_list` 软失败不崩溃。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 非法 JSON / 根为数组 → `runPhaseNext`、`runBacktrack` 抛错；磁盘文件内容未被改写（AC-14） | 新增 |
| 异常 | `workflow_type` 缺失 / 非枚举 → 同上，message 含字段路径 `workflow_type`（AC-14） | 新增 |
| 异常 | `created` 非 `YYYY-MM-DD` → 同上（AC-14） | 新增 |
| 异常 | `eval` 为对象 → `runPhaseLog` 抛错且不把 `eval` 改写成合法数组（AC-12、AC-14） | 新增 |
| 边界 | 同一非法文件下 `runChangeList` 均返回 `workflow_done===false`、`latest_phase===null`，change 仍出现（AC-14 容错面） | 新增 |
| 废弃 | 非法 / 缺字段时按缺省 `requirement` 继续推进工作流（`DEFAULT_WORKFLOW_TYPE` 兜底语义） | 废弃 |

<!-- 全部真实 fs；无 Mock -->

---

### PreToolUse deny → Node fs 写 workflow.json → `plugins/dev-team/bin/__tests__/eval-store-hook-vs-fs/eval-store-hook-vs-fs.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/hooks.ts` | PreToolUse：拦截 agent Write / Edit / Shell 对 `workflow.json` 的写入；对 `eval.json` 不再产生内置 deny |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | MCP 路径：进程内 `fs` 写 `workflow.json`、删除遗留 `eval.json` |
| `plugins/dev-team/bin/src/commands/change-create.ts` | MCP 路径：进程内创建 `workflow.json` |

**关联AC**: AC-8, AC-9

**关系描述**:

Hook 与 MCP 不共享写入通道：agent 工具调用经 `runProtectFiles`，MCP 经 Node `fs`。集成测试对同一相对路径分别走 stdin Write 与 `writeEvalJson` / `runChangeCreate`，证明 deny 不阻止进程内落盘；同时对 `eval.json` 断言「内置规则放行」与「MCP 侧迁移删除」并存，避免出现「agent 放行、MCP 却不再清理」的双源残留。出错模式是 hook glob 漏掉 `workflow.json`；内置集合误把 `eval.json` 继续 deny（AC-9 回退）；或误把 Node 写入也接到 PreToolUse（本仓库不会，用「同进程两入口」锁契约）。

#### 场景: 工具 deny 与 fs 成功并存

前置为 mock stdin 指向 `openspec/changes/demo/workflow.json`，同时在临时目录调用 `writeEvalJson` / `runChangeCreate`。预期 hook 输出 deny，临时目录文件写入成功。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | Write `workflow.json` deny，同时 `writeEvalJson` 在临时 change 目录写成功（AC-8） | 新增 |
| 正向 | `runChangeCreate` 仍能写 `workflow.json`（进程内 `fs` 不受 hook 限制）（AC-8） | 新增 |
| 异常 | Shell `> workflow.json` deny；不因此抛错影响随后的 `writeEvalJson` | 新增 |
| 边界 | StrReplace `workflow.json` deny；Edit 对照同样 deny | 新增 |

#### 场景: eval.json 内置豁免与 MCP 迁移删除并存

前置与上场景相同的临时 change 目录，含遗留 `eval.json` 与无 `eval` 键的 `workflow.json`。stdin 提交 Write `openspec/changes/demo/eval.json`，随后调用 `writeEvalJson`。预期 hook 不因内置规则 deny（agent 写入不再被拦截），而 MCP 侧仍完成迁移并删除遗留文件。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无 `write_protection` 时 Write `eval.json` → `permissionDecision!=="deny"`（AC-9） | 新增 |
| 正向 | 同批次再 `writeEvalJson` → 遗留 `eval.json` 被删除，条目在 `workflow.json.eval`（AC-4、AC-9） | 新增 |
| 异常 | `write_protection.files` 显式加入 `eval.json` glob → 同一 stdin 变为 deny（用户可自行保护） | 新增 |
| 边界 | `config.json` 同批仍 deny（内置集合未整体放宽）（AC-8） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| hook stdin / stdout | 沿用 `hooks.test.ts` 的 `mockReadFileSync` + 捕获 stdout；`readConfig` 无 `write_protection`（除用户保护用例） | PreToolUse deny / allow |
| 临时目录 fs | `writeEvalJson` / `runChangeCreate` 使用真实 `mkdtemp`，与 hook 输入路径解耦 | MCP fs 成功 |

---

### Skill / agent 文案 → phase_log / workflow.json → `plugins/dev-team/bin/__tests__/eval-store-skill-copy/eval-store-skill-copy.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/skills/phase-proposal/SKILL.md` | 禁止 Write `workflow.json`；缺文件时停止并二选一指引 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md`、`workflow-test-only/SKILL.md`、`openspec-archive-change/SKILL.md` | 前置文件说明、评估落盘文案、artifacts 示例 |
| `plugins/dev-team/agents/*-evaluator.md`、`test-execution-executor.md`、`test-design-planner.md` | evaluator 经 `phase_log` 写入 `workflow.json`；关系标题示例文件名 |

**关联AC**: AC-11

**关系描述**:

Skill / agent 是 agent 运行时的操作说明书，错误文件名会诱导 Write 被 hook deny，或在缺文件时依赖已删除的缺省类型继续门禁。本关系用只读 `fs.readFileSync` 做静态文案契约，不启动 agent。出错模式是只改了 MCP description 而 Markdown 仍写「Append to eval.json」或仍保留「缺文件时用 Write 创建 `workflow.json`」步骤。

#### 场景: 文案指向 phase_log 且禁止手写 JSON

读取 `plugins/dev-team` 下约定路径的 Markdown。预期 `phase-proposal` 的 Confirm workflow type 节不要求 Write `{ "workflow_type": "<choice>" }` 到 `workflow.json`，缺文件分支为「停止 + 二选一指引」；各 evaluator 的 `phase_log` 步骤提及 `workflow.json`（或评估记录位于该文件）；不得把独立文件 `eval.json` 当作唯一合法写入目标；归档 skill 的 artifacts 示例不含 `eval.json` 且说明 `workflow_done: false` 的两类成因。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `phase-proposal/SKILL.md` 无「Write `workflow.json`」步骤，且缺文件分支含停止 + 手写 / `change_create` 二选一指引（AC-11） | 新增 |
| 正向 | `test-execution-evaluator.md` 等 evaluator 提及 `phase_log` 与 `workflow.json`，不以「Append to eval.json」为步骤标题 | 新增 |
| 正向 | `workflow-requirement/SKILL.md`、`workflow-test-only/SKILL.md` 说明 `workflow.json` 由 `change_create` 建立且为 `phase_next` / `backtrack` 前置文件（AC-11） | 新增 |
| 正向 | `openspec-archive-change/SKILL.md` 完成性以 `workflow_done` 为准，并提示 `workflow_done: false` 可能源于 `workflow.json` 缺失 / 格式非法（指引 `change_create`） | 新增 |
| 异常 | 上述文件若仍出现以 `eval.json` 为写入目标的祈使句（Write / Append to eval.json）则失败 | 新增 |
| 边界 | `test-design-planner.md` 的关系标题示例为 `CLI参数 → workflow.json持久化`（不再出现 `eval.json持久化`） | 新增 |
| 废弃 | `phase-proposal/SKILL.md` 的「`__TOOL_ASK_USER__` 确认类型后调用 `change_create`」缺文件分支（对已存在目录必然失败） | 废弃 |

<!-- 只读扫描仓库内 Markdown，无跨进程 Mock -->

---

### backtrack → phase_next 回溯原因传播 → `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 写入回溯字段（原地改写最新条目） |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 读取窗口并拼接 planner / evaluator prompt |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 条目数组来源（本变更后 mock 不得再假设 `eval.json` 路径） |

**关联AC**: AC-3

**关系描述**:

既有集成测试验证 backtrack 后 `phase_next` 的 prompt 含回溯原因。本变更不改算法，但 mock 与注释中的 `eval.json` 文件语义需改为评估存储 / 读 `workflow.json.eval`。若继续按 `endsWith('eval.json')` 喂条目，合并后真实 `readEvalJson` 会读到空数组，导致假红或假绿。出错模式是 mock 漏改。

#### 场景: 更新存储来源后回溯原因仍传播

前置与既有相同（内存条目 + `workflow_type` mock）。执行 `runPhaseLog` / `runBacktrack` / `runPhaseNext`。预期 prompt 仍含 `⚠️ 回溯原因:`，且测试不再依赖向 `eval.json` 写盘。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | backtrack 后 planner / evaluator prompt 含原因（行为不变） | 新增 |
| 边界 | 多阶段回溯原因仍正确 | 新增 |
| 废弃 | 以磁盘 `eval.json` 作为该测试的写入断言目标 | 废弃 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `eval-json` 内存数组 | 沿用 `readEvalJson` / `writeEvalJson` / `appendEntry` 的 in-memory mock；`workflow.json` 仅提供 `workflow_type` | 回溯原因流 |

---

### 旧条目兼容 → phase_next → `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 解析无 `backtrack_reason` 的旧条目（来源改为 `workflow.json.eval` 或遗留文件） |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 兼容读取，不抛错 |

**关联AC**: AC-3

**关系描述**:

旧条目缺 `backtrack_reason` 时引擎不得抛错。本变更后夹具可放在 `workflow.json.eval` 或遗留 `eval.json`。出错模式是新存储路径下 schema 变严导致旧条目无法 parse，或夹具仍只靠 `endsWith('eval.json')` 提供数据。

#### 场景: 缺 backtrack_reason 的条目仍可读

旧条目可能省略可选字段 `backtrack_reason`。前置为内存数组或 `workflow.json.eval`（及对照遗留 `eval.json`）中存在缺该字段、其余字段符合 `phaseLogSchema` 的条目。输入为 `readEvalJson` 后再调用 `runPhaseNext`。预期解析成功且不抛错；prompt 不强制拼接回溯原因。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无 `backtrack_reason` 字段的条目经 `readEvalJson` 后 `phase_next` 不抛错 | 新增 |
| 正向 | 混合新旧条目时 prompt 不强制拼接原因 | 新增 |
| 边界 | 遗留 `eval.json` 回退路径下的旧条目同样可解析（AC-3） | 新增 |
| 废弃 | 仅通过 `endsWith('eval.json')` 提供旧条目且无权威 `eval` 对照 | 废弃 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 内存 / fs mock | 条目放到 `readEvalJson` 返回值或 `workflow.json.eval`；遗留回退作补充用例 | 兼容 |

---

### phase_log 拒绝回溯字段 → backtrack 缺条目文案 → `plugins/dev-team/bin/__tests__/phase_log workflow-aware backtrack rejection/phase_log workflow-aware backtrack rejection.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 不写回溯字段，只经 `appendEntry` 追加 |
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 无评估条目时拒绝回溯 |

**关联AC**: AC-1, AC-4

**关系描述**:

既有测试断言 `phase_log` 不改回溯状态、`backtrack` 在无条目时抛错；夹具与文案需按新存储与严格化后的错误消息更新（错误文案已从「没有 eval.json 条目」改为「没有评估条目」）。出错模式是只改了 `backtrack.ts` 源码、集成测试仍匹配旧正则导致假红。

#### 场景: 无评估条目时的拒绝文案

验证 `phase_log` 不写入回溯字段，以及无该 phase 评估条目时 `backtrack` 的拒绝文案。前置为内存 `mockEntries` 为空或不含目标 phase，且 `workflow.json` 存在合法元数据。先调用合法 `runPhaseLog`，再对无该 phase 条目的 change 调用 `runBacktrack`。预期 log 成功且 entry 无 `backtrack_to`；backtrack 抛错且 message 含 `没有评估条目`、不含 `eval.json`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `phase_log` 成功且 entry 无 `backtrack_to`（AC-1） | 新增 |
| 异常 | 无该 phase 条目时 `backtrack` 抛错，message 含 `没有评估条目`、不含 `eval.json`（AC-4） | 新增 |
| 边界 | `workflow.json` 缺失时 `backtrack` 先因类型读取抛错（早于「无条目」判定）（AC-13） | 新增 |
| 废弃 | `toThrow(/没有 eval.json 条目/)` | 废弃 |
| 废弃 | 夹具只向独立 `eval.json` 写条目以驱动本关系 | 废弃 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 内存 `mockEntries` | 沿用对 `readEvalJson` / `appendEntry` / `writeEvalJson` 的 mock；`workflow.json` 提供 `workflow_type` | 拒绝文案 |

---

## 不可测试项

- `plugins/dev-team/bin/src/schemas/workflow.schema.ts` — **原因**: `test_resolve_paths` 返回 `Not in test config scope`（`openspec/config.json` 的 `excludes` 含 `bin/src/schemas/**/*`）。schema 形状经全部消费入口间接覆盖：`workflow_type` 值域与 `created` 格式由 `change-config.test.ts`、`eval-json.test.ts` 的读写契约断言覆盖；合法产出由 `change-create.test.ts` 覆盖。
- `plugins/dev-team/bin/src/schemas/index.ts` — **原因**: 同上不在测试配置范围内。新增导出的可用性由消费方（`change-config.ts`、`eval-json.ts`、`change-create.schema.ts`）的编译期引用与静态分析保证，无独立运行时断言。
- `plugins/dev-team/bin/src/schemas/change-create.schema.ts` — **原因**: 同上。仅 `workflow_type` 的枚举改为引用 `workflowTypeSchema`（值域与 describe 不变），行为由 `mcp.test.ts` 的输入 schema 用例与 `change-create.test.ts` 覆盖。
- `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` — **原因**: 同上。输入字段不变，仅注释补「缺文件 / 格式非法时 `getWorkflowType` 先抛错」；该行为由 `backtrack.test.ts` 与集成测试 `同一非法 workflow.json → 严格方抛错 / 容错方不崩溃` 覆盖。
- `plugins/dev-team/bin/src/schemas/change-list.schema.ts` — **原因**: 同上。`artifacts` / `latest_phase` / `workflow_done` 的 describe 属文档字符串，运行时行为由 `change-list.test.ts` / `mcp.test.ts` 覆盖。
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — **原因**: 同上。`write_protection.files[].glob` 的 describe 示例字符串已为 `openspec/changes/**/workflow.json`（形状不变），内置 glob 行为由 `hooks.test.ts` 覆盖；`plugins/dev-team/bin/dev-team-config.schema.json` 为构建生成物。
- `plugins/dev-team/package.json` 的 `version` patch bump — **原因**: 发布流程字段，无自动化断言；组装产物在插件构建步骤中刷新。
- `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 组装产物 — **原因**: 提案与设计明确由 `plugins/dev-team` 构建刷新，不得手改，不在本变更测试范围。
- `openspec/changes/archive/**` 中的历史 `eval.json` / `workflow.json` — **原因**: 提案明确不批量迁移、不补写元数据，只读文物。
- Hook 与 MCP 分属两个操作系统进程（agent 工具调用被 deny、MCP 进程内 `fs` 放行）— **原因**: 无法在本仓库无 agent 宿主的情况下启动真实 PreToolUse 管道包住 MCP server；集成测试 `eval-store-hook-vs-fs` 在同进程分别调用 `runProtectFiles` 与 `writeEvalJson` / `runChangeCreate` 近似该契约。
- `plugins/dev-team/bin/src/mcp.ts` 的 stdio `require.main` 引导 — **原因**: 源码标明由主程序行使；`connectToServer` + `InMemoryTransport` 已覆盖注册、description 与 handler 行为。
- `eval-check` CLI 注册 — **原因**: `cli.ts` 不注册该命令；归档完成性继续以 `change_list.workflow_done` 为准（由 `change-list.test.ts` / `mcp.test.ts` 覆盖）。
