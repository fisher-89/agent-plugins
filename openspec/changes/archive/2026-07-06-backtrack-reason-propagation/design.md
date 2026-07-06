# 设计: backtrack-reason-propagation

> **变更**: backtrack-reason-propagation
> **日期**: 2026-07-06

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Schema 模块 | 定义 `phaseLogSchema` 中的 `backtrack_reason` 字段及校验规则 | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | zod/v4 | TypeScript |
| Eval JSON 库 | `BuildEntryParams` 类型和 `buildEntry()` 函数支持 `backtrack_reason` 的写入 | `plugins/dev-team/bin/src/lib/eval-json.ts` | zod/v4, fs, path | TypeScript |
| Phase Log 命令 | `runPhaseLog()` 中增加 `backtrack_reason` 必填校验，写入 eval.json | `plugins/dev-team/bin/src/commands/phase-log.ts` | eval-json, schemas, change, change-config, workflow | TypeScript |
| Phase Next 命令 | `getLatestBacktrackInfo()` 读取回溯原因，`buildPhaseDef()` 拼接到 prompt 中 | `plugins/dev-team/bin/src/commands/phase-next.ts` | eval-json, schemas, change, change-config, workflow | TypeScript |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  本变更不涉及新增文件、配置变更和 HTTP API 设计，相关子节以 HTML 注释标注省略原因。
-->

### 新增文件

<!-- 本变更不涉及新增文件，所有修改均在现有文件内完成 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | `phaseLogSchema` 新增 `backtrack_reason` 字段；`phaseLogInputSchema` 的 `.pick()` 中包含 `backtrack_reason` | Schema 层定义字段结构，input schema 使其可通过 MCP 工具传入 |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | `BuildEntryParams` 增加 `backtrack_reason`；`buildEntry()` 将该字段写入 EvalEntry | 存储层将回溯原因持久化到 eval.json |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | `runPhaseLog()` 增加校验：`backtrack_to` 非空时 `backtrack_reason` 必填；`buildEntry()` 调用中传入 `backtrack_reason` | 校验层确保回溯原因不被遗漏，同时传递给存储层 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | `getLatestBacktrackTarget()` 重构为 `getLatestBacktrackInfo()`；`handleBacktrack()` 获取 reason 并传入 `buildPhaseResponse()`；`buildPhaseDef()` 将 reason 拼接到 planner 和 evaluator prompt | 读取层+Prompt 层将回溯原因传播到后续 agent 的 prompt 中 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runPhaseLog` | `plugins/dev-team/bin/src/commands/phase-log.ts` | 修改 | `(options: PhaseLogOptions) -> PhaseLogResult` | 新增 `backtrack_reason` 校验逻辑：当 `options.backtrack_to` 非空时，`options.backtrack_reason` 必填且不能为空字符串 |
| `runPhaseNext` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 修改 | `(options: PhaseNextOptions) -> PhaseNextResult` | 内部使用 `getLatestBacktrackInfo()` 替代 `getLatestBacktrackTarget()`，返回结果中包含带回溯原因的 prompt |
| `buildEntry` | `plugins/dev-team/bin/src/lib/eval-json.ts` | 修改 | `(params: BuildEntryParams) -> EvalEntry` | 将 `params.backtrack_reason` 写入 EvalEntry 对象（可选，默认 null） |
| `readEvalJson` | `plugins/dev-team/bin/src/lib/eval-json.ts` | 不变 | `(changeDir: string) -> EvalEntry[]` | 无签名变更；旧条目无 `backtrack_reason` 字段时解析不报错 |
| `getLatestBacktrackTarget` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 删除 | — | 重构为 `getLatestBacktrackInfo()`，此函数不再存在 |
| `getLatestBacktrackInfo` | `plugins/dev-team/bin/src/commands/phase-next.ts` | 新增 | `(entries: EvalEntry[]) -> { target: string \| string[] \| null, reason: string \| null }` | 替代 `getLatestBacktrackTarget()`，同时返回回溯目标和原因；无 `backtrack_reason` 字段时 reason 返回 null |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `phaseLogSchema` (Zod 对象) | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 修改 | 新增 `backtrack_reason: z.string().max(500).optional().nullable()` |
| `phaseLogInputSchema` (Zod 对象) | `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | 修改 | `.pick()` 列表新增 `backtrack_reason: true`，使其可被 MCP 工具输入接收 |
| `BuildEntryParams` | `plugins/dev-team/bin/src/lib/eval-json.ts` | 修改 | Pick 列表中新增 `'backtrack_reason'`，字段类型为 `string \| null \| undefined`（由 Zod schema 派生） |
| `EvalEntry` | `plugins/dev-team/bin/src/lib/eval-json.ts` | 自动更新 | 由 `z.infer<typeof phaseLogSchema>` 派生，自动继承 `backtrack_reason?: string \| null` |

### 配置

<!-- 本变更不涉及配置文件变更，无配置键新增或修改 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| EvalEntry (`eval.json` 条目) | `phase`, `attempt`, `verdict`, `report`, `checklist`, `backtrack_to`, **`backtrack_reason`** (新增), `skipped`, `timestamp`, `stale` | 由 `phaseLogSchema` 定义；`EvalEntry` 通过 `z.infer` 派生 | 追加写入或全量覆写至 `openspec/changes/<change>/eval.json`，JSON 数组格式 |

`backtrack_reason` 字段约束：
- **类型**: `string | null | undefined`
- **最大长度**: 500 字符（Zod `.max(500)` 校验）
- **存储条件**: `backtrack_to` 非空时要求必填；无回溯时不写入
- **向后兼容**: 旧条目无此字段时解析为 `undefined`，读取层统一转为 `null`

---

## 路由/API 设计

<!-- 本变更为纯 CLI 工具的内部逻辑变更，不涉及 HTTP API 端点 -->

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有变更为现有文件内修改，依赖链与当前一致（zod/v4, fs, path）

### 构建/测试依赖

- 无新增构建/测试依赖

---

## 待决问题

- 无（proposal.md 中无待决问题）
