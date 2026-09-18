# 设计: phase-lifecycle-file-log

> **变更**: phase-lifecycle-file-log
> **日期**: 2026-09-18

> **提案与规格同步状态**: proposal.md 与 specs/(phase-lifecycle、workflow-file-inventory、change-files)已随提案阶段写入并通过评估,本设计不再将其列为变更对象;变更清单仅覆盖实现文件。
>
> **本版修订(第 2 轮)**:① 修复任务排序违例——`schemas/phase-start.schema.ts` 的创建任务自 Phase 3 前移至 Phase 1(schema 层),使 `schemas/index.ts` 导出任务不再依赖后续阶段;② 依用户架构决策,phase 相关操作安置于 `modules/workflow/phase/`——原设计置于 `modules/workflow/files/` 的 `phase-state.ts` 迁至该目录,共享读写原语 `doc-io.ts` 上移至 `modules/workflow/` 模块根供两子目录共用;仅做本变更所需的最小迁移,模块层不依赖 `commands/`,并避免依赖 `lib/`(理由见核心设计决议·目录安置行)。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| workflow schema 层 | 扩展 `active_phase`(对象或 null)、`interrupted[]`、`file_log` 校验;eval 条目可选 `start_at`;移除 `files`/`source` 桶 schema | `plugins/dev-team/bin/src/schemas/workflow.schema.ts`、`phase-log.schema.ts`、`phase-start.schema.ts`(新)、`index.ts` | zod/v4 | zod looseObject(未知键保留) |
| phase 生命周期命令层(MCP / hook 适配) | 校验 change 与 phase 表归属、attempt 推导、stdin / session 适配,随后委托 module 状态操作;自身不持有 `active_phase` 读写逻辑(薄适配,与 `commands/change-files.ts` 同型) | `commands/phase-start.ts`(新)、`commands/phase-log.ts`、`commands/sweep-phase.ts`(新)、`hooks.ts`、`mcp.ts` | schemas、lib/change、lib/change-config、lib/eval-json、lib/session-registry、lib/workflow、modules/workflow | MCP tool(zod schema 校验)+ hook 子命令 |
| phase 状态操作(module 层) | `workflow.json` 的 `active_phase` / `interrupted` 读、写、清、归档(`readActivePhase` / `writeActivePhase` / `clearActivePhase` / `interruptActivePhase`),写纪律与既有清单层一致(文件必须已存在、schema 校验、未知键保留、2 空格缩进) | `modules/workflow/phase/phase-state.ts`(新)、`modules/workflow/doc-io.ts`(新,module 根共享读写原语) | schemas、doc-io、utils | read-modify-write JSON;零 lib、零 commands 依赖 |
| file_log 存储层 | 日志式文件清单:读 log、(scope 命名空间, attempt, path) 键控原位覆盖/追加、派生净状态、append/set 语义 | `modules/workflow/files/file-inventory.ts`(重写) | schemas、doc-io(module 根)、utils | 纯函数派生 + read-modify-write |
| 引擎共享辅助下沉 | `hasPhasePassed` 自 `commands/phase-next.ts` 原样下沉(供 MCP server 与 hook 同仓库直引);`extractAgentName` / `matchesExecutorAgent` 门③解析辅助 | `lib/workflow.ts`、`lib/eval-json.ts`、`commands/phase-next.ts` | schemas | 纯函数,行为零改动 |
| 归账管线 | 规范化 → 自污染排除 → gitignore 过滤 → 按门控解析出的 scope 写 log(签名由 `agentType` 上下文改为 `scope` 上下文) | `modules/workflow/files/record.ts` | file-inventory、gitignore | hook 管线(fail-open) |
| 记录器门控(hook 命令层) | stdin 解析、`phase_next` / `phase_start` 事件识别与绑定刷新、门①②③按序判定、scope 解析、错误吞并 exit 0 | `commands/record-files.ts` | session-registry、shell-file-ops、lib/workflow、modules/workflow | PostToolUse hook 协议适配 |
| 中断收口 sweep | UserPromptSubmit 事件:绑定 change 有遗留 `active_phase` → 以 `end_at` 移入 `interrupted[]` 并清空;不写 eval;任何失败 exit 0 | `commands/sweep-phase.ts`(新)、`hooks.ts` | session-registry、modules/workflow | UserPromptSubmit hook |
| hooks 清单与双平台构建 | PostToolUse matcher 增 `__MCP:phase_start__`;新增 `userPromptSubmit` 事件键;claude 产出无 matcher 事件、cursor 不产出 | `hooks/hooks.canonical.json`、`build/hooks-profile.ts` | apply-env-tokens | canonical JSON + 平台包装 |
| 派生净状态读方 | 四个读方改读派生视图,输出/行为契约不变;缺失 `file_log` 硬报错 | `modules/workflow/files/files-query.ts`、`lib/c4-cross-ref.ts`、`commands/test-execution.ts`、`commands/test-resolve-paths.ts` | modules/workflow | 只读消费 |
| 手工补录/初始化通道 | `change_files` append/set 委托新 log API;`change_create` 初始化 `file_log: []` | `commands/change-files.ts`、`commands/change-create.ts` | modules/workflow | MCP tool |
| 技能协议 | 每个 phase turn 在 `phase_next` 返回目标 phase 后、executor/evaluator 执行前调用一次 `phase_start`;retry/回溯 recall 重跑重新调用;Verdict 不调用 | `skills/phase-*/SKILL.md`(8 个)、`skills/workflow-requirement/SKILL.md`、`skills/workflow-test-only/SKILL.md` | MCP | 技能文本(构建期 token 替换) |

**保持不动的边界**(proposal「不要修改」):`phase-next.ts` 响应协议与 gate/round 逻辑(`hasPhasePassed` 仅原样下沉,行为零改动)、`backtrack.ts` 引擎(对 `file_log` / `active_phase` 中立)、`lib/shell-file-ops.ts` 的 `extractFileOps` 三态提取、`gitignore.ts` 过滤语义、`protect-files.ts` 写保护、既有 eval 历史 append-only 序列与 round 语义。`lib/session-registry.ts` 零改动(绑定刷新扩展只在 `record-files.ts` 识别层)。

---

## 核心设计决议

对 proposal「待决问题」逐条拍板:

| 待决问题 | 决议 | 理由 |
|----------|------|------|
| revert(`git restore`)在 file_log 中的表达 | 新增第三种 op 值 `op: 'revert'` 记录入 log;`deriveNetState` 规则:同 path 末条为 revert → written 与 deleted 均不含 | 保持 log append-only 与审计完整性;与现行 `foldFileOps` 的 revert 语义一一对应,等价性天然成立。对比"移除该 path 既有记录":需在归账热路径引入记录删除,破坏日志不可变语义 |
| 门③ `__CALL_AGENT:...__` token → 平台实际 agent 名解析 | 纯函数 bare-name 归一化:token `__CALL_AGENT:<id>__` 提取 `<id>`;事件 `agent_type` 剥离 `dev-team:` 与 `dev-team_` 前缀(claude/cursor 插件产物为 `dev-team:<id>`,cursor-home-image 产物为 `dev-team_<id>`,与 `build/env.ts` 前缀表一致);两者归一后按 bare id 相等判定。事件无 `agent_type` 或该 phase `executor: null` → 不匹配 → workflow scope | 运行时无需探测平台(build 期三种产物前缀被统一剥离),无平台探测的失效面;误判(异插件同名 agent)仅损 scope 粒度,不影响净状态与门②,与 proposal 风险表缓解一致。落点 `lib/workflow.ts`(phase 表同文件) |
| dedupe key 细化与 scope 翻转的物理形态 | key = **(scope 命名空间, attempt?, path)**:phase scope 按 `(phase-id, attempt, path)` 键控,workflow scope 按 `('workflow', path)` 键控(workflow 条目无 attempt,与 spec 键控定义一致)。同 key 原位覆盖 `op`/`scope`/`at`(保留原数组位置);跨 key(跨 attempt/跨 phase)追加末尾。phase ↔ workflow 翻转属跨 key:追加新记录,净状态由派生"同 path 后条胜"达成 workflow 覆盖——与 spec 场景「scope 翻转」的 THEN(派生净状态为 workflow)完全一致。说明:phase 记录携带 attempt 而 workflow 记录无 attempt,二者在键空间上不可能同 key,spec 示例中"原位覆盖"在派生视图意义上成立 | 键控定义由 spec 规定;物理原位覆盖仅发生在同 key 内,日志保留全部来源轨迹 |
| `change_files` set 的 log 级映射 | provided = `written ∪ deleted`;**删除 log 中涉及 provided path 的全部记录**(任意 scope/attempt/op),再按桶序在 log 末尾追加 workflow scope 记录(`written` 先、`deleted` 后;同 path 出现在两桶时 deleted 胜,与现行 fold 先 written 后 deleted 的顺序一致)。未涉及 path 的记录(含 phase 审计)逐字保留 | 精确落实 spec「删除涉及 path 全部记录 + 追加 workflow 记录」;末尾追加使派生后条胜自然生效,无需重排既有记录 |
| `change_files` append 的实现形态 | 对每个传入 path 追加 workflow scope 记录(`written` → op write,`deleted` → op delete);workflow 命名空间内同 path 已有记录则原位覆盖(去重);phase scope 审计记录不动;不应用 gitignore | 即 spec「workflow scope 按 path upsert、phase 记录保留」 |
| `phase_log` 盖章时序与防御行为 | 落盘**前**读 `active_phase`:`active_phase.phase === 本次 phase` 时 `buildEntry` 携带 `start_at = active_phase.start_at`,否则不携带且不改 `active_phase`;`appendEntry` 成功**后**清空 `active_phase`(置 null)。清场失败仅 stderr 诊断、不抛错(输出 `{written, phase, attempt}` 契约与"条目已落盘"事实不受损;遗留 `active_phase` 由下次 `phase_start` last-wins 或 sweep 回收,不会产生重复条目) | 若清场放在盖章前,appendEntry 失败会白丢运行态;若清场抛错,调用方重试 `phase_log` 会造成重复条目。吞错收尾无双录风险,interrupted[] 留档语义略失真但无消费方,接受 |
| `phase_start` 重入与 attempt 推导 | attempt = `computeAttempt(readEvalJson(changeDir), phase)`(该 phase 既有 eval 条目数 + 1,与 `phase_log` 无显式 attempt 时同规则);last-wins 覆盖既有 `active_phase`,可重入;同 turn 协议外重复调用 → attempt 不变、`start_at` 刷新,计时以最后一次为准 | 与 spec「retry 重跑在上一 fail 条目落盘后重新调用,attempt 自然递增」一致;复用既有 `computeAttempt`,无新推导逻辑 |
| 多 session 并发绑同一 change | 接受 change 级 `active_phase` last-wins(后开覆盖先开),记为已知限制;per-session 化留二期 | 实际单人使用;proposal 风险表已接受 |
| phase 目录安置与依赖方向(用户架构决策,本版新增) | phase 状态操作安置于 `modules/workflow/phase/`(`phase-state.ts`);`doc-io.ts` 上移 `modules/workflow/` 模块根供 `files/` 与 `phase/` 两子目录共用(避免 phase→files 兄弟耦合);仅迁移本变更所需——既有 `files/*` 四文件不搬移;`commands/phase-start.ts` 等命令仅作薄适配 | module 层不依赖 `commands/`(现状无此方向依赖,保持);`phase/` 尽量不依赖 `lib/`——`phase_start` 的校验(phase 表在 `lib/workflow`)与 attempt 推导(eval 历史在 `lib/eval-json`)是操作不可缺的输入,若整体下沉 module 反而引入 3 个 lib 依赖,故校验与推导留在命令层(既有命令本就依赖 lib,非新增依赖面),`phase-state.ts` 保持零 lib 依赖;现有唯一 modules→lib 边(`files-query.ts` → `lib/change` 的 `resolveChangeDir`,既有)不动。`hasPhasePassed` 下沉落点维持 proposal 指定的 `lib/workflow.ts`——它是 eval 序列语义而非 `active_phase` 运行态,纳入 `modules/workflow/phase/` 超出最小迁移 |

**`change_create` 必须同步初始化 `file_log`**(proposal 实现文件清单未列,系 spec 强制推导的必需补充):所有清单消费方对缺失 `file_log` 硬报错,若 `change_create` 仍写 `files`,新建 change 立即成为"legacy"。故初始化字段由 `files: { written: [], deleted: [] }` 改为 `file_log: []`,键序契约同步为 `workflow_type → created → file_log`。

**关键流程时序**(单 phase turn):

```
phase_next(只读, PostToolUse 事件刷新绑定)
→ phase_start(写 active_phase{phase, attempt, start_at}, 事件再次刷新绑定)
→ executor/evaluator 写文件(PostToolUse: 门①绑定 → 门②active_phase → 门③agent_type
   → scope = phase/workflow → file_log 覆盖/追加)
→ phase_log(条目盖 start_at;清空 active_phase → 门②关闭,后续写丢弃)
→ [中断] 下一条用户消息 UserPromptSubmit sweep → interrupted[] 留档 + 清空
→ [retry/回溯] 重新 phase_next → phase_start 自愈重开
```

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/phase-start.ts` | `phase_start` MCP 命令适配(薄):校验 change 与 phase 表归属,attempt 由 eval 历史推导(`lib/eval-json`),委托 `writeActivePhase` last-wins 写入 `active_phase` |
| `plugins/dev-team/bin/src/commands/sweep-phase.ts` | UserPromptSubmit hook 子命令:绑定 change 的遗留 `active_phase` 移入 `interrupted[]` 并清空;fail-open exit 0 |
| `plugins/dev-team/bin/src/schemas/phase-start.schema.ts` | `phase_start` 输入/输出 schema(`phaseStartInputSchema` / `phaseStartOutputSchema`) |
| `plugins/dev-team/bin/src/modules/workflow/phase/phase-state.ts` | `active_phase` / `interrupted` 的读、写、清、归档(module 内部实现,经 barrel 导出);仅依赖 schemas 与 `doc-io`,零 lib、零 commands 依赖 |
| `plugins/dev-team/bin/src/modules/workflow/doc-io.ts` | module 根共享读写原语:读取并校验 `workflow.json` 文档 / 原样保留未知键写回(2 空格缩进 + 末尾换行);供 `files/file-inventory.ts` 与 `phase/phase-state.ts` 两子目录复用,不经 barrel 导出 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | 移除 `workflowFilesSchema`(`files`/`source` 桶);新增 `activePhaseSchema`、`interruptedEntrySchema`、`fileLogSchema`;`workflowFileSchema` 增可选 `active_phase` / `interrupted` / `file_log` | 存储重构的 schema 面;legacy `files` 字段经 looseObject 落入未知键保留 |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` | `phaseLogSchema` 增可选 `start_at: z.iso.datetime()` | 仅落盘侧盖章;`phaseLogInputSchema` 的 pick 不含 `start_at`,输入面不变 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 导出 `phaseStartInputSchema` / `phaseStartOutputSchema` 及新类型;移除 `workflowFilesSchema` 导出 | 导出清单随 schema 增删同步 |
| `plugins/dev-team/bin/src/lib/workflow.ts` | `hasPhasePassed` 自 `commands/phase-next.ts` 原样下沉(参数改结构化条目类型,避免 eval-json 循环依赖);新增 `extractAgentName(agentRef: string): string` 与 `matchesExecutorAgent(eventAgentType, executorAgentType)` 门③解析辅助 | 供 hook 与 MCP server 同仓库直引;`phase_next` 引擎行为零改动 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 删除本地 `hasPhasePassed` 定义,改为自 `../lib/workflow` 导入并 re-export | 保持 `commands/phase-next` 既有导入路径(`change-list.ts` 不动)与行为零改动 |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | `BuildEntryParams` 与 `buildEntry` 支持可选 `start_at`(显式传入才写入条目) | 其余 eval 读写逻辑不动(迁移留后续 change) |
| `plugins/dev-team/bin/src/mcp.ts` | 注册 `phase_start` 工具(withResolvedProjectRoot 注入);刷新 `change_files` / `workflow_files` / `test_resolve_paths` / `archi_check` 描述中 `files` 相关措辞为 `file_log` 派生净状态 | 工具名遵循 `xx_yy` 命名空间规则 |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 落盘前读 `active_phase` 决定是否盖章 `start_at`;`appendEntry` 成功后清空 `active_phase`(匹配时),清场失败 stderr 吞错 | 输出 `{written, phase, attempt}` 形状不变 |
| `plugins/dev-team/bin/src/commands/record-files.ts` | `isPhaseStartCall` 识别并入绑定刷新;门②(`readActivePhase`)与门③(`getWorkflowType` + `getPhaseTable` + `matchesExecutorAgent`)按序判定;scope 解析为 `{ kind: 'phase', phase, attempt } \| { kind: 'workflow' }` 传入 `recordFileOps`;门②不满足丢弃 + stderr 诊断 | 失败语义不变:任何错误 stderr + exit 0;绑定/提取/吞错结构保留 |
| `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 重写:`FileLogEntry` / `FileOp`(去 `agentType`)/ `RecordScope` 类型;`readFileLog`(缺失 `file_log` 硬报错含重建指引)、`appendLogEntries`(键控覆盖/追加落盘)、`deriveNetState`(纯函数,与现行 `foldFileOps` 等价)、`appendWorkflowFiles`、`setWorkflowFiles`;移除 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `appendFileOps` / `setFileBuckets` | 读写纪律不变(文件必须已存在、未知键保留、2 空格缩进);IO 原语改用 `doc-io.ts` |
| `plugins/dev-team/bin/src/modules/workflow/files/record.ts` | `recordFileOps` 上下文由 `{ projectRoot, agentType? }` 改为 `{ projectRoot, scope: RecordScope }`;条目改为带 `scope`/`attempt`/`at` 的 `FileLogEntry` 经 `appendLogEntries` 落盘 | 规范化 → 排除 → gitignore 过滤的管线顺序不变(`op: 'revert'` 一并过滤) |
| `plugins/dev-team/bin/src/modules/workflow/files/files-query.ts` | 实现改为 `deriveNetState(readFileLog(changeDir))` | `getChangedFiles` 签名与输出形状不变 |
| `plugins/dev-team/bin/src/modules/workflow/index.ts` | barrel 导出更新:自 `./files/file-inventory` 增 `readFileLog` / `appendLogEntries` / `deriveNetState` / `appendWorkflowFiles` / `setWorkflowFiles`,自 `./phase/phase-state` 增 `readActivePhase` / `writeActivePhase` / `clearActivePhase` / `interruptActivePhase`,连同 `FileLogEntry` / `FileOp` / `RecordScope` / `ActivePhase` / `InterruptedPhase` 类型;移除旧桶形态导出 | 单一出口,无 re-export shim;`doc-io.ts` 不经 barrel |
| `plugins/dev-team/bin/src/commands/change-files.ts` | 委托改为 `appendWorkflowFiles` / `setWorkflowFiles` | 输入校验与输出形状不变 |
| `plugins/dev-team/bin/src/commands/change-create.ts` | 初始化字段 `files: { written: [], deleted: [] }` → `file_log: []`,键序注释同步 | 必需补充:缺失 `file_log` 即硬报错,否则新建 change 立即成 legacy |
| `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | 清单模式被查文件集改读 `deriveNetState(readFileLog(...)).written` | `staged` 保持现有硬报错别名语义;`files` 显式入参优先级不变;不执行 `git diff --cached` |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | `resolveMutationDiffFiles` 改读派生 written | 突变 scope 行为不变 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `modules === 'change'` 模式改读派生 written | 错误语义不变(硬报错、不回退 git) |
| `plugins/dev-team/bin/src/hooks.ts` | 注册 `sweep-phase` 子命令分发并导出 `runSweepPhase` | 其余子命令不动 |
| `plugins/dev-team/hooks/hooks.canonical.json` | postToolUse matchers(claude 与 cursor)追加 `__MCP:phase_start__`;新增 `userPromptSubmit` 事件项(`matchers.claude` 仅作存在标志、包装时省略 matcher 字段;`cursor: null`) | 指向 `sweep-phase` 子命令 |
| `plugins/dev-team/build/hooks-profile.ts` | canonical schema 增 `userPromptSubmit` 键;claude 包装产出 `UserPromptSubmit`(无 matcher 字段);cursor 包装不产出该事件(cursor 侧无 matcher 值即降级) | 既有 preToolUse / postToolUse / subagentStop 包装逻辑不动 |
| `plugins/dev-team/skills/phase-proposal/SKILL.md`、`plugins/dev-team/skills/phase-dev-design/SKILL.md`、`plugins/dev-team/skills/phase-test-design/SKILL.md`、`plugins/dev-team/skills/phase-implement/SKILL.md`、`plugins/dev-team/skills/phase-test-gen/SKILL.md`、`plugins/dev-team/skills/phase-test-execution/SKILL.md`、`plugins/dev-team/skills/phase-code-review/SKILL.md`、`plugins/dev-team/skills/phase-acceptance/SKILL.md` | 协议插入:gate `phase_next` 返回本 phase 后、executor/evaluator 执行前调用一次 `__MCP:phase_start__({change, phase: "<next_phase>"})`;retry 重跑与 backtrack recall 重进执行前重新调用;Verdict `phase_next` 不触发 | 8 个 phase 技能全覆盖;无 executor 的 phase(code-review / acceptance)同样调用,供计时档案 |
| `plugins/dev-team/skills/workflow-requirement/SKILL.md`、`plugins/dev-team/skills/workflow-test-only/SKILL.md` | LOOP 内 gate 通过后、`-- Run Executor if Exist --` 之前插入 `phase_start` 调用(每轮迭代一次;done/error 不调用) | 两技能内嵌编排协议,须同步 |
| `plugins/dev-team/package.json` | `version` 2.10.39 → 2.10.40 | 项目规则:改源后 bump 并 `pnpm -C plugins/dev-team run build` 重建 `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 产物(产物为生成物,不在变更清单) |

### 删除文件

<!-- 无删除文件:file_log 重构在既有文件内完成,旧桶形态 API 随 file-inventory.ts 重写移除,不产生文件级删除 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runPhaseStart` | `commands/phase-start.ts` | 新增 | `function runPhaseStart(options: PhaseStartOptions): PhaseStartResult` | 薄适配:校验 change 存在、`workflow.json` 合法、phase 属于该 `workflow_type` phase 表,attempt 由 `computeAttempt(readEvalJson(changeDir), phase)` 推导,委托 `writeActivePhase` last-wins 写入;非法 phase 报错且文件不变 |
| `runSweepPhase` | `commands/sweep-phase.ts` | 新增 | `function runSweepPhase(): void` | 读 stdin `session_id` → `lookupChange` → `interruptActivePhase`;任何失败仅 stderr、exit 0 |
| `readActivePhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `function readActivePhase(changeDir: string): ActivePhase \| null` | 字段缺失或 null 视为无运行态;文件缺失/非法抛错(由调用方吞错策略兜底) |
| `writeActivePhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `function writeActivePhase(changeDir: string, active: ActivePhase): void` | last-wins 覆盖,未知键保留 |
| `clearActivePhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `function clearActivePhase(changeDir: string): void` | 置 null,`interrupted` 不动(`phase_log` 清场用) |
| `interruptActivePhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `function interruptActivePhase(changeDir: string, endedAt?: string): boolean` | 有遗留 `active_phase` 时以 `end_at`(缺省 now)追加进 `interrupted[]` 并清空,返回是否发生归档;不写 eval(`sweep` 用) |
| `readFileLog` | `modules/workflow/files/file-inventory.ts` | 新增(取代 `readFileInventory`) | `function readFileLog(changeDir: string): FileLogEntry[]` | 缺失 `file_log` 硬报错,文案含"重建 change"指引,不回退 git |
| `appendLogEntries` | `modules/workflow/files/file-inventory.ts` | 新增(取代 `foldFileOps` + `writeFileInventory`) | `function appendLogEntries(changeDir: string, entries: FileLogEntry[]): FileLogEntry[]` | (scope 命名空间, attempt?, path) 键控原位覆盖/追加后落盘,返回落盘后的 log |
| `deriveNetState` | `modules/workflow/files/file-inventory.ts` | 新增 | `function deriveNetState(log: FileLogEntry[]): { written: string[]; deleted: string[] }` | 纯函数:同 path 后条胜;末条 write→written、delete→deleted、revert→两桶均不含;与现行 `foldFileOps` 等价 |
| `appendWorkflowFiles` | `modules/workflow/files/file-inventory.ts` | 新增(取代 `appendFileOps`) | `function appendWorkflowFiles(changeDir: string, paths: { written?: string[]; deleted?: string[] }): { written: string[]; deleted: string[] }` | workflow scope 按 path upsert,phase 审计记录保留,不过滤 gitignore |
| `setWorkflowFiles` | `modules/workflow/files/file-inventory.ts` | 新增(取代 `setFileBuckets`) | `function setWorkflowFiles(changeDir: string, paths: { written?: string[]; deleted?: string[] }): { written: string[]; deleted: string[] }` | 删除涉及 path 的全部记录 + 末尾追加 workflow 记录;未涉及 path 不动;不过滤 gitignore |
| `recordFileOps` | `modules/workflow/files/record.ts` | 修改 | `function recordFileOps(changeDir: string, ops: FileOp[], context: { projectRoot: string; scope: RecordScope }): void` | scope 由 hook 命令层门控解析后传入;管线顺序不变 |
| `extractAgentName` | `lib/workflow.ts` | 新增 | `function extractAgentName(agentRef: string): string` | `__CALL_AGENT:<id>__` 提取 `<id>`;剥离 `dev-team:` / `dev-team_` 前缀;其余原样返回 |
| `matchesExecutorAgent` | `lib/workflow.ts` | 新增 | `function matchesExecutorAgent(eventAgentType: string \| undefined, executorAgentType: string \| null \| undefined): boolean` | 门③:双方经 `extractAgentName` 归一后 bare id 相等才为 true;任一缺失为 false |
| `hasPhasePassed` | `lib/workflow.ts`(自 `commands/phase-next.ts` 下沉) | 修改(实现位置) | `function hasPhasePassed(entries: ReadonlyArray<{ phase: string; verdict: string; skipped?: boolean; stale?: boolean }>, phaseId: string): boolean` | 逻辑逐字不变,参数改结构化类型避免循环依赖;`commands/phase-next.ts` re-export 保持既有导入路径 |
| `buildEntry` | `lib/eval-json.ts` | 修改 | `function buildEntry(params: BuildEntryParams): EvalEntry`(`BuildEntryParams` 增可选 `start_at: string`) | `start_at` 显式传入才写入条目;`timestamp` 语义不变 |
| `getChangedFiles` | `modules/workflow/files/files-query.ts` | 修改(实现) | `function getChangedFiles(changeName: string, projectRoot: string): { written: string[]; deleted: string[] }` | 签名与输出形状不变,改读派生视图 |
| `runChangeFiles` | `commands/change-files.ts` | 修改(实现) | `function runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput` | 签名不变,委托新 log API |
| `runPhaseLog` | `commands/phase-log.ts` | 修改 | `function runPhaseLog(options: PhaseLogOptions): PhaseLogResult` | 签名与输出形状不变,集成盖章与清场 |
| `runRecordFiles` / `runProtectFiles` / `runStaticCheck` | `hooks.ts` | 修改(仅导出面) | `export { runProtectFiles, runRecordFiles, runSweepPhase, runStaticCheck }` | 增 `runSweepPhase`,其余不动 |
| `readFileInventory` / `foldFileOps` / `writeFileInventory` / `appendFileOps` / `setFileBuckets` | `modules/workflow/files/file-inventory.ts` | 移除 | — | 桶形态 API 由 log API 与派生视图取代,不留 shim |

<!-- 私有函数(规范化、排除判定、键控折叠内部实现等)不列入。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FileLogEntry` | `modules/workflow/files/file-inventory.ts` | 新增 | `{ op: 'write' \| 'delete' \| 'revert'; scope: string; attempt?: number; path: string; at: string }`;`scope` 为 phase id 或 `'workflow'`,`attempt` 仅 phase scope 携带 |
| `FileOp` | `modules/workflow/files/file-inventory.ts` | 修改 | `{ op: 'write' \| 'delete' \| 'revert'; path: string }`;移除 `agentType` 字段(来源由 scope 承载) |
| `RecordScope` | `modules/workflow/files/file-inventory.ts` | 新增 | `{ kind: 'phase'; phase: string; attempt: number } \| { kind: 'workflow' }`;门控解析结果,记录器与手工通道共用 |
| `ActivePhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `{ phase: string; attempt: number; start_at: string }`,对应 `activePhaseSchema` 推断 |
| `InterruptedPhase` | `modules/workflow/phase/phase-state.ts` | 新增 | `{ phase: string; attempt: number; start_at: string; end_at: string }`,对应 `interruptedEntrySchema` 推断 |
| `PhaseStartOptions` / `PhaseStartResult` | `schemas/phase-start.schema.ts` | 新增 | `z.input` / `z.output` 推断;输出 `{ started: true; phase: string; attempt: number; start_at: string }` |
| `FileInventory`(及 `workflowFilesSchema`) | `schemas/workflow.schema.ts` | 移除 | `files`/`source` 桶形态随存储重构废除;legacy 文件中的 `files` 键经 looseObject 未知键保留 |
| `EvalEntry` | `lib/eval-json.ts` | 修改(随 schema) | `phaseLogSchema` 增可选 `start_at` 后自动携带;消费方按可选读取 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `postToolUse[].matchers`(claude/cursor) | `plugins/dev-team/hooks/hooks.canonical.json` | 修改 | — | 追加 `__MCP:phase_start__` token(phase_start 调用事件刷新绑定) |
| `userPromptSubmit`(新事件键) | `plugins/dev-team/hooks/hooks.canonical.json` | 新增 | `[{ matchers: { claude: "*", cursor: null }, commandTemplate: "node \"__DEV_TEAM_ROOT__/bin/__BIN:hooks__\" sweep-phase" }]` | claude 包装产出无 matcher 的 `UserPromptSubmit`;cursor 不产出(中断窗口降级,明确接受) |
| `userPromptSubmit`(canonical schema 键) | `plugins/dev-team/build/hooks-profile.ts` | 新增 | `[]`(default) | `Array<{ matchers, commandTemplate }>`;claude 包装省略 matcher 字段 |
| `version` | `plugins/dev-team/package.json` | 修改 | `2.10.40` | 项目规则:改源后 bump 并重建双平台产物 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json`(总账,`change_create` 唯一创建) | `workflow_type`、`created`、`eval[]`(条目增可选 `start_at`,`timestamp` 即 end_at)、`active_phase`(运行态)、`interrupted[]`(中断留档)、`file_log[]`(日志式清单);legacy `files`/`source` 落入未知键保留 | `active_phase.phase` ∈ phase 表;`active_phase.attempt` 与 `eval` 同 phase 条目数 + 1 对齐;`file_log[].scope` ∈ phase id ∪ `'workflow'`,`attempt` 对应 `active_phase.attempt` | `openspec/changes/<name>/workflow.json`,2 空格缩进 + 末尾换行,写方必须文件已存在 |
| 派生净状态(虚拟视图,不落盘) | `written: string[]`、`deleted: string[]` | 由 `file_log` 按数组顺序"同 path 后条胜"派生;revert 净消除 | 无(每次读取现算) |
| session 注册表(不变) | `session_id → { change, updatedAt }`,24h TTL bind 时懒惰修剪 | `phase_next` / `phase_start` 事件建立/刷新;门①查询 | `<tmpdir>/dev-team-hooks/<sha256(projectRoot)前16位>.json` |
| `interrupted[]` 条目 | `{ phase, attempt, start_at, end_at }` | 由 sweep 从 `active_phase` 归档生成;仅留档,无消费方、不写 eval | `workflow.json.interrupted` |

---

## 路由 / API 设计

本变更无 HTTP API;API 面为 MCP 工具(hook 子命令见变更清单):

| 工具 | 方向 | 输入 | 输出 | 说明 |
|------|------|------|------|------|
| `phase_start` | 新增 | `{ change: string, phase: string(9 值枚举), project_root?: string }` | `{ started: true, phase: string, attempt: number, start_at: string }` | 非法 phase 报错且 `workflow.json` 逐字节不变;可重入 last-wins |
| `phase_log` | 行为扩展 | 不变(输入不含 `start_at`) | `{ written, phase, attempt }` 形状不变 | 匹配 `active_phase` 时盖章 `start_at` 并清场 |
| `change_files` | 语义重定义 | `{ change, op: "append"\|"set", written?, deleted? }` | `{ written, deleted }`(派生净状态) | append = workflow scope upsert;set = 删涉及 path 全部记录 + 追加 workflow 记录 |
| `workflow_files` | 实现替换 | `{ change, project_root? }` | `{ written, deleted }`(派生净状态,无审计明细) | 缺失 `file_log` 硬报错含重建指引 |
| `phase_next` / `backtrack` | 零改动 | 不变 | 不变 | 引擎只读;backtrack 对 `file_log` / `active_phase` 中立 |
| `archi_check` / `test_resolve_paths` | 实现替换 | 不变 | 不变 | 被查/突变文件集改读派生 written;`staged` 维持现有硬报错别名 |

---

## 依赖

### 运行时依赖

- `@modelcontextprotocol/sdk` — MCP server 与 `phase_start` 工具注册(既有)
- `zod/v4` — `active_phase` / `interrupted` / `file_log` / `start_at` schema 校验(既有)
- `node:fs` / `node:path` / `node:crypto` / `node:os` — `workflow.json` 读写、session 注册表(既有,零新增)

### 构建/测试依赖

- 无新增。`pnpm -C plugins/dev-team run build` 既有流水线承担 canonical hooks 清单的双平台包装与产物重建;测试依赖(vitest 等)不变

---

## 待决问题

- 提案待决问题已全部在上文「核心设计决议」拍板(revert 表达、门③解析、set 映射、盖章防御、重入边界、多 session),无阻塞实现的遗留决策。
- 已知限制(接受,不阻塞):Cursor 无 UserPromptSubmit → 中断后 `active_phase` 遗留、门常开至下一条消息由 MCP 侧收口或 `phase_start` 覆盖(TTL 兜底为二期备选);多 session 并发 last-wins 互踩;`file_log` 体积压缩策略二期;bare-name 归一化在异插件同名 agent 场景可能误判 phase scope(仅损粒度,不影响净状态与门②)。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `schemas/workflow.schema.ts`:`activePhaseSchema` / `interruptedEntrySchema` / `fileLogSchema`,looseObject 未知键保留;`phase-log.schema.ts` 可选 `start_at`(类型定义表) |
| AC-2 | `commands/phase-start.ts`(薄适配)`runPhaseStart` + `modules/workflow/phase/phase-state.ts` `writeActivePhase` + `mcp.ts` 注册:`computeAttempt` 推导 attempt、last-wins 覆盖、非法 phase 报错文件不变(公共函数表) |
| AC-3 | `commands/phase-log.ts`:匹配盖章 + `clearActivePhase`,不匹配不动;输出形状不变(核心设计决议·盖章时序) |
| AC-4 | `commands/record-files.ts` 四分支:未绑定丢弃 / 无 `active_phase` 丢弃 + stderr / 匹配 → phase 记录 / 不匹配 → workflow 记录(修改文件表 + 时序) |
| AC-5 | 重绑竞态回归:`phase_log` 清场后 Verdict `phase_next` 仅刷新绑定,门②已关写事件丢弃(sweep/门控设计,端到端由测试阶段覆盖) |
| AC-6 | `file-inventory.ts` 键控覆盖/追加 + `deriveNetState` 与现行 `foldFileOps` 等价(公共函数表 + 核心设计决议·dedupe key) |
| AC-7 | 四读方切派生视图契约不变;`readFileLog` 缺失 `file_log` 硬报错含重建指引,不回退 git(修改文件表) |
| AC-8 | `change-files.ts` 委托 `appendWorkflowFiles` / `setWorkflowFiles` 新语义(公共函数表 + 核心设计决议) |
| AC-9 | `record-files.ts` `isPhaseStartCall` 并入绑定刷新;`session-registry.ts` 不变(修改文件表) |
| AC-10 | `commands/sweep-phase.ts` + `hooks.ts` 分发 + `hooks.canonical.json` `userPromptSubmit` 键 + `build/hooks-profile.ts` 双平台包装(claude 产出、cursor 不产出)(新增/修改文件 + 配置表) |
| AC-11 | 8 个 `skills/phase-*/SKILL.md` 与 2 个 workflow 编排技能插入 `phase_next → phase_start` 序列,retry 重跑重新调用(修改文件表) |
| AC-12 | 旧 files-only change 经 `readFileLog` 硬报错;`phase_next` / `backtrack` 零改动;`extractFileOps` / gitignore / 写保护不动(保持不动的边界) |
