# 任务: phase-lifecycle-file-log

> 依赖顺序:schema 与共享层 → workflow module 存储层(`files/` file_log + `phase/` 状态操作) → phase 生命周期命令 → 门控与读方 → hooks 清单与构建 → 技能协议 → 版本与产物。测试用例与测试执行由独立工作流阶段承担,此处不含测试任务。

## Phase 1: schema 与共享层

- [x] `plugins/dev-team/bin/src/schemas/workflow.schema.ts`:移除 `workflowFilesSchema`(`files`/`source` 桶);新增 `activePhaseSchema`、`interruptedEntrySchema`、`fileLogSchema`(`op: 'write'|'delete'|'revert'`、`scope`、可选 `attempt`、`path`、`at`);`workflowFileSchema` 增可选 `active_phase`(对象或 null)、`interrupted[]`、`file_log`,保持 looseObject 未知键保留
- [x] `plugins/dev-team/bin/src/schemas/phase-log.schema.ts`:`phaseLogSchema` 增可选 `start_at: z.iso.datetime()`;`phaseLogInputSchema` 的 pick 不含 `start_at`(输入面不变)
- [x] 新增 `plugins/dev-team/bin/src/schemas/phase-start.schema.ts`:`phaseStartInputSchema`(`change: string`、`phase` 为 phaseId 枚举、`project_root` 可选)与 `phaseStartOutputSchema`(`{ started: true, phase, attempt, start_at }`)
- [x] `plugins/dev-team/bin/src/schemas/index.ts`:导出 `phaseStartInputSchema` / `phaseStartOutputSchema` 与新增 schema,移除 `workflowFilesSchema` 导出(依赖上一条目的 `phase-start.schema.ts`,同层内先建后导出)
- [x] `plugins/dev-team/bin/src/lib/workflow.ts`:`hasPhasePassed` 自 `commands/phase-next.ts` 原样下沉(参数改结构化条目类型 `{ phase; verdict; skipped?; stale? }`,避免 eval-json 循环依赖);新增 `extractAgentName(agentRef: string): string`(`__CALL_AGENT:<id>__` 提取、剥离 `dev-team:` / `dev-team_` 前缀)与 `matchesExecutorAgent(eventAgentType: string | undefined, executorAgentType: string | null | undefined): boolean`
- [x] `plugins/dev-team/bin/src/commands/phase-next.ts`:删除本地 `hasPhasePassed` 定义,改为自 `../lib/workflow` 导入并 re-export,内部调用同步换源;gate/round/响应协议零改动
- [x] `plugins/dev-team/bin/src/lib/eval-json.ts`:`BuildEntryParams` 增可选 `start_at: string`,`buildEntry` 显式传入才写入条目;其余 eval 逻辑不动

## Phase 2: workflow module 存储层(file_log + phase 状态操作)

- [x] 新增 `plugins/dev-team/bin/src/modules/workflow/doc-io.ts`(module 根):共享读写原语——`workflow.json` 存在性检查、JSON 解析、plain-object 根判定、`workflowFileSchema` 校验、未知键保留写回(2 空格缩进 + 末尾换行);供 `files/file-inventory.ts` 与 `phase/phase-state.ts` 两子目录复用(避免 phase→files 兄弟耦合);不经 barrel 导出
- [x] 重写 `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts`:类型 `FileLogEntry` / `FileOp`(去 `agentType`)/ `RecordScope`;`readFileLog`(缺失 `file_log` 硬报错含"重建 change"指引,不回退 git)、`appendLogEntries`((scope 命名空间, attempt?, path) 键控同 key 原位覆盖 op/scope/at、跨 key 末尾追加)、`deriveNetState`(纯函数,同 path 后条胜,revert 两桶净消除,与现行 `foldFileOps` 语义等价)、`appendWorkflowFiles`(workflow scope 按 path upsert,phase 记录保留)、`setWorkflowFiles`(删涉及 path 全部记录 + 末尾追加 workflow 记录,未涉及 path 不动);移除 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `appendFileOps` / `setFileBuckets`
- [x] `plugins/dev-team/bin/src/modules/workflow/files/record.ts`:`recordFileOps(changeDir, ops, context: { projectRoot, scope: RecordScope })` — 上下文由 `agentType` 改为 `scope`;条目改为带 `scope`/`attempt`/`at` 的 `FileLogEntry` 经 `appendLogEntries` 落盘;规范化 → 自污染排除 → gitignore 过滤(含 revert op)顺序不变
- [x] 新增 `plugins/dev-team/bin/src/modules/workflow/phase/phase-state.ts`(phase 状态操作,module 层):`readActivePhase(changeDir): ActivePhase | null`、`writeActivePhase(changeDir, active)`、`clearActivePhase(changeDir)`、`interruptActivePhase(changeDir, endedAt?): boolean`(有遗留则入 `interrupted[]` 并清空,不写 eval);读写纪律与 `../doc-io.ts` 一致;仅依赖 schemas 与 doc-io——零 lib、零 commands 依赖(校验与 attempt 推导留在命令层)
- [x] `plugins/dev-team/bin/src/modules/workflow/index.ts`:barrel 导出更新——增 `readFileLog`、`appendLogEntries`、`deriveNetState`、`appendWorkflowFiles`、`setWorkflowFiles`、`readActivePhase`、`writeActivePhase`、`clearActivePhase`、`interruptActivePhase` 与 `FileLogEntry` / `FileOp` / `RecordScope` / `ActivePhase` / `InterruptedPhase` 类型;移除旧桶形态导出,无 re-export shim

## Phase 3: phase 生命周期命令与 MCP 注册

- [x] 新增 `plugins/dev-team/bin/src/commands/phase-start.ts`(薄适配,依赖 Phase 1 的 `schemas/phase-start.schema.ts` 与 Phase 2 的 `phase-state.ts`,不内联 `active_phase` 读写逻辑):`runPhaseStart(options)` — `resolveChangeDir`(`lib/change`)+ `getWorkflowType`(`lib/change-config`)校验 change,`getPhaseTable`(`lib/workflow`)校验 phase 表归属(非法报错且文件不变);attempt = `computeAttempt(readEvalJson(changeDir), phase)`;`writeActivePhase` last-wins 覆盖;返回 `{ started: true, phase, attempt, start_at }`
- [x] `plugins/dev-team/bin/src/mcp.ts`:注册 `phase_start` 工具(`withResolvedProjectRoot` 注入);同步刷新 `change_files` / `workflow_files` / `test_resolve_paths` / `archi_check` 描述中 `files` 措辞为 `file_log` 派生净状态
- [x] `plugins/dev-team/bin/src/commands/phase-log.ts`:`appendEntry` 前读 `readActivePhase` — 匹配本次 phase 时 `buildEntry` 携带 `start_at`;`appendEntry` 成功后 `clearActivePhase`(清场失败仅 stderr 诊断不抛错);不匹配时条目无 `start_at` 且 `active_phase` 不动;输出 `{ written, phase, attempt }` 形状不变
- [x] 新增 `plugins/dev-team/bin/src/commands/sweep-phase.ts`:`runSweepPhase()` — 读 stdin `session_id` → `lookupChange` → 绑定且 `readActivePhase` 非空时 `interruptActivePhase`;未绑定 no-op;任何失败仅 stderr、exit 0,不阻塞用户消息
- [x] `plugins/dev-team/bin/src/hooks.ts`:注册 `sweep-phase` 子命令分发,导出面增 `runSweepPhase`

## Phase 4: 记录器门控与读方切换

- [x] `plugins/dev-team/bin/src/commands/record-files.ts`:`isPhaseStartCall`(`mcp__` 前缀 + `phase_start` 后缀)并入 `bindFromPhaseNextCall` 同路绑定刷新;写事件按序门控——门① session 绑定(未绑定丢弃,现状)→ 门② `readActivePhase`(缺失/null 丢弃 + stderr 诊断)→ 门③ `getWorkflowType` + `getPhaseTable` + `matchesExecutorAgent` 解析 scope(phase 匹配携带 attempt / 不匹配 → workflow);scope 作为 `recordFileOps` 上下文传入;错误吞并 exit 0 语义不变
- [x] `plugins/dev-team/bin/src/modules/workflow/files/files-query.ts`:`getChangedFiles` 实现改为 `deriveNetState(readFileLog(changeDir))`,签名与输出形状不变
- [x] `plugins/dev-team/bin/src/lib/c4-cross-ref.ts`:清单模式被查文件集改读派生 written(经 `readFileLog` + `deriveNetState`);`files` 显式入参优先、`staged` 维持硬报错别名、不执行 `git diff --cached`
- [x] `plugins/dev-team/bin/src/commands/test-execution.ts`:`resolveMutationDiffFiles` 改读派生 written,突变 scope 行为不变
- [x] `plugins/dev-team/bin/src/commands/test-resolve-paths.ts`:`modules === 'change'` 模式改读派生 written,错误语义不变(硬报错、不回退 git)
- [x] `plugins/dev-team/bin/src/commands/change-files.ts`:委托改为 `appendWorkflowFiles` / `setWorkflowFiles`,输入校验与输出形状不变
- [x] `plugins/dev-team/bin/src/commands/change-create.ts`:初始化字段 `files: { written: [], deleted: [] }` 改为 `file_log: []`,键序契约 `workflow_type → created → file_log` 及注释同步

## Phase 5: hooks 清单与双平台构建

- [x] `plugins/dev-team/hooks/hooks.canonical.json`:postToolUse matchers(claude 与 cursor)追加 `__MCP:phase_start__`;新增 `userPromptSubmit` 事件项(`matchers: { claude: "*", cursor: null }`,`commandTemplate` 指向 `sweep-phase` 子命令)
- [x] `plugins/dev-team/build/hooks-profile.ts`:canonical schema 增 `userPromptSubmit` 键(`Array<{ matchers, commandTemplate }>`,default `[]`);claude 包装产出 `UserPromptSubmit`(省略 matcher 字段);cursor 包装不产出该事件;既有 preToolUse / postToolUse / subagentStop 包装逻辑不动

## Phase 6: 技能协议

- [x] `plugins/dev-team/skills/phase-proposal/SKILL.md`、`plugins/dev-team/skills/phase-dev-design/SKILL.md`、`plugins/dev-team/skills/phase-test-design/SKILL.md`、`plugins/dev-team/skills/phase-implement/SKILL.md`、`plugins/dev-team/skills/phase-test-gen/SKILL.md`、`plugins/dev-team/skills/phase-test-execution/SKILL.md`、`plugins/dev-team/skills/phase-code-review/SKILL.md`、`plugins/dev-team/skills/phase-acceptance/SKILL.md`(8 个):gate `phase_next` 返回本 phase 后、executor/evaluator 执行前插入一次 `__MCP:phase_start__({change: "<change-name>", phase: "<next_phase>"})`;retry 重跑与 backtrack recall 重进执行前重新调用;Verdict `phase_next` 不触发;无 executor 的 phase(code-review / acceptance)同样调用
- [x] `plugins/dev-team/skills/workflow-requirement/SKILL.md` 与 `plugins/dev-team/skills/workflow-test-only/SKILL.md`:LOOP 内 gate 通过后、`-- Run Executor if Exist --` 之前插入 `phase_start` 调用(每轮迭代一次;`gate.error` / `gate.done` 路径不调用)

## Phase 7: 版本与产物

- [x] `plugins/dev-team/package.json`:`version` 2.10.39 → 2.10.40
- [x] 运行 `pnpm -C plugins/dev-team run build` 重建双平台产物(`claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`),确认产物内 hooks 清单含 `UserPromptSubmit`(claude)且 cursor 产物不含、技能文本含 `phase_start`
