# 任务: merge-eval-into-workflow

> **变更**: merge-eval-into-workflow
> **日期**: 2026-09-11

---

## 阶段 1: workflow.schema 与严格类型读取

- [x] 新增 `plugins/dev-team/bin/src/schemas/workflow.schema.ts`（不引入新依赖）：
  - `workflowTypeSchema = z.enum(['requirement', 'bug-fix', 'refactor', 'test-only'])`，describe 保持 `PGE workflow type`
  - `workflowEvalSchema = z.array(phaseLogSchema)`
  - `workflowFileSchema = z.looseObject({ workflow_type: workflowTypeSchema, created: z.string().regex(/^\d{4}-\d{2}-\d{2}$/).optional(), eval: workflowEvalSchema.optional() })`；`created` 的 describe 说明「`change_create` 写入 `YYYY-MM-DD`；缺失不视为非法」；未知键允许
  - `export type WorkflowFile = z.infer<typeof workflowFileSchema>`
- [x] 在 `plugins/dev-team/bin/src/schemas/index.ts` 导出 `workflowTypeSchema` / `workflowEvalSchema` / `workflowFileSchema` 与 `WorkflowFile` 类型
- [x] 修改 `plugins/dev-team/bin/src/schemas/change-create.schema.ts`：`workflow_type` 的 `z.enum([...])` 改为引用 `workflowTypeSchema`，值域与 describe 不变
- [x] 修改 `plugins/dev-team/bin/src/lib/change-config.ts`：
  - 删除 `DEFAULT_WORKFLOW_TYPE` 常量；文件内 MUST NOT 再出现 `'requirement'` 兜底字面量
  - `getWorkflowType(change: string): string` 按「文件必须存在 → `JSON.parse` → 根必须是对象（`isPlainObject`）→ `workflowFileSchema.parse` → 返回 `workflow_type`」实现；任一失败按 design.md「错误文案」抛错（含绝对路径与 `change_create` 指引）
  - 私有解析 / 读取辅助函数不 `export`（避免 knip 死代码与 test-only export）
- [x] `change-config.ts` 注释写明：`workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` 的前置文件；MUST NOT 再描述「缺省」「默认类型」语义

## 阶段 2: Eval 存储 IO 严格化

- [x] `plugins/dev-team/bin/src/lib/eval-json.ts` 的 `readEvalJson(changeDir: string): EvalEntry[]`：
  - 优先级不变：`workflow.json.eval` 为数组（含 `[]`）→ 用之；否则遗留 `eval.json`；否则 `[]`；MUST NOT 合并两源
  - `eval` 数组改经 `workflowEvalSchema.parse` 解析
  - `eval` 键存在但非数组 → 抛 `workflow.json.eval 必须是数组，但实际类型为 <typeof>`；遗留 `eval.json` 根非数组 → 保持现行抛错
- [x] `writeEvalJson(changeDir: string, entries: EvalEntry[]): void`：
  - 删除 `mkdirSync`、`DEFAULT_WORKFLOW_TYPE` 与缺省补齐 `workflow_type` / `created` 的分支
  - 文件不存在 → 抛错（文案含绝对路径 + `change_create` 指引）
  - 文件存在 → `parseWorkflowJson`（根必须是对象）+ `workflowFileSchema.parse` 校验，失败抛错且不写入
  - 设置 `doc.eval` 为传入的完整 `entries`，保留 `workflow_type` / `created` / 未知键；`JSON.stringify(doc, null, 2)` + 末尾换行写回
  - 成功后若同目录存在 `eval.json` 则 `unlinkSync`；MUST NOT 再 `writeFileSync` `eval.json`
- [x] `appendEntry(changeDir: string, entry: EvalEntry): void`：仍为 `readEvalJson` + push + `writeEvalJson`；删除「创建目录 / 文件」相关注释与实现假设（目录与文件 MUST 已存在）

## 阶段 3: 命令层错误传播与文案

- [x] `plugins/dev-team/bin/src/commands/phase-next.ts`：确认 `getWorkflowType(change)` 的抛错不被 try/catch 吞掉（缺文件 / 格式非法直接终止）；保留 `Failed to read workflow.json: <msg>` 包装与 `round_limit` 的 `workflow.json` 文案；保持只读（不调用 `writeEvalJson`、不删 `eval.json`）
- [x] `plugins/dev-team/bin/src/commands/backtrack.ts`：同 `phase-next`（类型解析抛错透出）；persist 仅 `writeEvalJson`；保留「`Phase "…" 没有评估条目…`」文案
- [x] `plugins/dev-team/bin/src/commands/phase-log.ts`：保留 `读取 workflow.json 失败` / `写入 workflow.json 失败` 包装；注释说明缺文件时内层错误已带 `change_create` 指引；调用面仍为 `readEvalJson` / `appendEntry`
- [x] `plugins/dev-team/bin/src/commands/change-list.ts`：`readWorkflowType` 改用 `workflowFileSchema.safeParse`（删除手工 `isPlainObject` / `typeof` 判断与随之不再使用的 import），任一失败返回 `null`；`KNOWN_ARTIFACTS` 不含 `eval.json`；解析失败 → `latest_phase: null`、`workflow_done: false`，change 仍出现在列表中
- [x] `plugins/dev-team/bin/src/commands/change-create.ts`：实现不变（只写 `{ workflow_type, created }`）；注释补契约：唯一创建者、MUST 写 `workflowTypeSchema` 内的合法类型、MUST NOT 写 `eval` 键、MUST NOT 创建 `eval.json`

## 阶段 4: MCP、Hook 与 Schema 文案

- [x] `plugins/dev-team/bin/src/hooks.ts`（本轮回溯修正）：复核 `loadPatterns` 的内置集合**只**有 `**/openspec/changes/**/workflow.json`（reason 提示 `phase_log` / `backtrack` / `change_create`，保留 `%s` / `%t`）与 `**/openspec/config.json`；文件内 MUST NOT 出现 `**/openspec/changes/**/eval.json` glob 或把 `eval.json` 当作受保护遗留文件的注释；`write_protection.files` 合并逻辑（含绝对路径判断与 `**/` 前缀补齐）保持不变。现存代码已符合，本任务是复核与保持，不得回退为「过渡期保护 `eval.json`」
- [x] `plugins/dev-team/bin/src/mcp.ts`：确认 `phase_log` / `backtrack` description 指向 `workflow.json`；`change_create` description 补「`workflow.json` 的唯一创建者；缺文件时读取方报错」；工具名与 input schema 不变
- [x] `plugins/dev-team/bin/src/schemas/backtrack.schema.ts`：注释指向 `workflow.json`，并补「缺文件 / 格式非法时 `getWorkflowType` 先抛错」；输入字段不变
- [x] `plugins/dev-team/bin/src/schemas/change-list.schema.ts`：`artifacts` / `latest_phase` 的 describe 去掉 `eval.json` 文件语义；`workflow_done` describe 补「`workflow.json` 缺失或格式非法时为 false」
- [x] `plugins/dev-team/bin/src/schemas/config/config.schema.ts`：确认 `write_protection.files[].glob` 的 describe 示例为 `openspec/changes/**/workflow.json`（本轮回溯只影响内置集合，用户配置形状不变）；无实质改动时保持原样，构建时经 `generateConfigJsonSchema` 刷新 `plugins/dev-team/bin/dev-team-config.schema.json`

## 阶段 5: Skill 与 Agent 文案

- [x] `plugins/dev-team/skills/phase-proposal/SKILL.md` 的 `### Confirm workflow type`：已存在 → 跳过并进入 Phase Check；不存在 → **停止**并指引（由用户手写 `{ "workflow_type": "<type>", "created": "<YYYY-MM-DD>" }`，或改走 `change_create` / `workflow-*` skill 新建），MUST NOT Write/Edit；删除「用 `__TOOL_ASK_USER__` 确认类型后调用 `change_create`」的旧分支，以及任何「缺省 `requirement` 足够继续门禁」类措辞
- [x] `plugins/dev-team/skills/workflow-requirement/SKILL.md` 与 `plugins/dev-team/skills/workflow-test-only/SKILL.md`：保留 `change_create` 调用与「Evaluator 经 `phase_log` 写入 `workflow.json`」约定；补「`workflow.json` 由 `change_create` 建立，是 `phase_next` / `backtrack` 的前置文件（不得手写）」
- [x] `plugins/dev-team/skills/openspec-archive-change/SKILL.md`：`artifacts` 示例不含 `eval.json`，完成性以 `workflow_done` 为准；补「`workflow_done: false` 亦可能因 `workflow.json` 缺失 / 格式非法，此时指引 `change_create`」
- [x] 确认 `plugins/dev-team/skills/phase-dev-design/SKILL.md`、`phase-test-design`、`phase-implement`、`phase-test-gen`、`phase-test-execution`、`phase-code-review`、`phase-acceptance` 的错误文案为「Evaluator 未通过 `phase_log` 写入 `workflow.json`（或 `phase_next.last_result` 未更新）」，MUST NOT 再把独立 `eval.json` 当作唯一合法存储
- [x] 确认 `plugins/dev-team/agents/` 下 9 个 `*-evaluator.md`（proposal / dev-design / test-design / implementation / test-gen / test-execution / code-review / acceptance / code-analyze）与 `test-execution-executor.md` 的文案为「经 `phase_log` 写入 `workflow.json`（`eval` 字段）」，且禁止 Write/Edit/Bash 改 `eval.json` / `workflow.json`；标题或步骤名若仍写「Append to eval.json」改为指向 `phase_log` / `workflow.json`
- [x] `plugins/dev-team/agents/test-design-planner.md`：关系标题示例中的 `CLI参数 → eval.json持久化` 改为 `CLI参数 → workflow.json持久化`（仅文案，不改 planner 行为）

## 阶段 6: 版本与产物

- [x] 按项目规则 patch bump `plugins/dev-team/package.json` 的 `version`（HEAD 为 `2.10.35`，暂存区已是 `2.10.36`；保持单一 patch bump，不在同一提交内叠加两次）
- [x] 在 `plugins/dev-team` 执行构建（`pnpm -C plugins/dev-team run build`），刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`
- [x] **不要**手改组装产物（`claude-plugins/`、`cursor-plugins/`、`cursor-home-image/`）与 `openspec/config.json`

## 阶段 7: test-execution 回溯修正（本轮回溯）

- [x] `plugins/dev-team/bin/src/lib/eval-json.ts` 的 `writeEvalJson`：在 `doc.eval = entries` 前加 `Array.isArray(entries)` 守卫；非数组（`null` / `undefined`）时抛 `workflow.json.eval 必须是数组，但实际类型为 <typeof>`，不写盘（test-design L86）
- [x] `plugins/dev-team/bin/src/lib/eval-json.ts` 的 `appendEntry`：在读取 / 追加前加空条目守卫（`entry === undefined || entry === null` 抛错）；**不得**改用 `phaseLogSchema.parse` 校验（Zod 会 strip 未知字段，破坏「entry 含多余未知字段时随数组写入」边界断言）；`push` 保持原样（test-design L105）
- [x] 回滚 10 个 `plugins/dev-team/skills/*/SKILL.md` 越界的调用形式改写：`__MCP:phase_next__({ change: "...", run_id: "..." })` → 恢复为 design L28 / `phase-skills` `workflow-orchestration` spec 的规范形式 `__MCP:phase_next__(change=<name>, run_id=<run_id>)`；本变更的 skill 文案改动（`workflow.json` 落点、错误文案、`phase-proposal` 缺文件停止）保留不动
- [x] 恢复 `plugins/dev-team/bin/__tests__/phase-next-run-id-skills/phase-next-run-id-skills.test.ts`（`git restore`，未改动内容），使 `run_id` / `[Round {gate.round}/20]` 契约重新有断言覆盖
- [x] 重新构建产物并跑全量测试：`plugins/dev-team` 下 `vp test` 1956/1956 通过（含 `writeEvalJson` / `appendEntry` 的空值用例与恢复的 skill 文案契约用例）
