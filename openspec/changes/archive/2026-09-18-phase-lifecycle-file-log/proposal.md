# 提案: phase-lifecycle-file-log

> **变更**: phase-lifecycle-file-log
> **日期**: 2026-09-18
> **状态**: draft(待 proposal 评审)

---

## 问题

两个问题在同一条归账管线上交汇,合并解决:

**1. 文件归账绑定只增不减,存在完成后泄漏。** PostToolUse `record-files` hook 经 `session_id → change` 持久注册表(`<tmpdir>/dev-team-hooks/<sha256(projectRoot)前16位>.json`)归账文件操作,绑定仅在 `phase_next` MCP 调用事件时建立/刷新(`record-files.ts` 的 `bindFromPhaseNextCall`),没有任何释放机制:`session-registry.ts` 只有 `bindSession` / `lookupChange`,24h TTL 仅在 bind 时懒惰修剪,`lookupChange` 不做新鲜度检查。工作流完成后(acceptance pass,`phase_next` 返回 `done:true`)绑定仍指向 change——本仓库惯例不自动 archive,人工检查期内同 session 的顺手修改会持续污染文件清单,而该清单正是 implementation-evaluator 的核对范围(已替代 git-based scope),污染直接产生"实现改了设计外文件"假阳性;事后补救仅有 `change_files` 手工覆写。中途泄漏(turn 内非工作流修改被归账)同源存在。

**2. phase 无耗时档案。** eval 条目只有落盘时刻 `timestamp`,没有起点,per-attempt 耗时无法计算;phase 档案缺"何时开始、跑了多久"。

早期 A2 方案(终态 unbind + hook all-passed 守卫)经三轮演进被否决:技能协议规定每 turn 末尾 Verdict 必调 `phase_next`(返回 `done:true`),而 hook 无条件绑定——该调用发生在最终 `phase_log` 清除之后,同 turn 内即把绑定绑回,清除被系统性击穿。取代方案为 `phase_start` 显式开启 + `active_phase` 状态门控(见提案),A2 的 `unbindChange` / bind-guard 两层全部废弃。

---

## 提案

以 `workflow.json` 新增 `active_phase` 运行态为轴心,做三件事:

1. **phase 生命周期**:新 MCP 工具 `phase_start(change, phase)` 显式开启运行态 `{phase, attempt, start_at}`;`phase_log` 落盘条目时从 active_phase 盖章 `start_at`(既有 `timestamp` 即 end_at)并清空 active_phase;UserPromptSubmit hook 在每条新用户消息前 sweep,关闭因用户中断遗留的 active_phase(移入 `interrupted[]` 留档,**不**写 eval 条目,避免烧 retry 配额;平台查证:Stop 钩子在中断与 API 错误时不触发,不能承担收口)。per-attempt 耗时 = `timestamp − start_at`,retry 独立计时,status 由 verdict/stale/skipped 推导,零迁移。

2. **归账门控**:record-files 对写事件按序判定——① session 已绑定(注册表机制不变,`phase_next` 与 `phase_start` 事件都刷新绑定);② 该 change 存在 `active_phase`;③ 事件 `agent_type` 等于运行 phase 的 `executor.agent_type`。③不匹配(主 agent / 非 executor 子代理)**不丢弃**,记录为 `workflow` scope。中途泄漏与完成后泄漏全堵在门②;完成 turn 的 Verdict `phase_next` 重绑因门②已关而变得无害,重绑竞态消失;backtrack / 错误终态经下一 turn 的 `phase_start` 自愈重开。

3. **存储重构**:workflow.json 的 `files{written,deleted}` + `source` 旁挂映射废除,改为日志式 `file_log: [{op:'write'|'delete', scope:'<phase-id>'|'workflow', attempt?, path, at}]`;dedupe key = (attempt, path),同 key 原位覆盖 op/scope/at(后写者胜,含 scope 翻转),跨 attempt / 跨 phase 追加新记录。净状态按 log 顺序派生(同 path 后条胜),4 个读方(`workflow_files`、`c4-cross-ref`、`test-execution`、`test-resolve-paths`)契约不变。executor 子代理写 → scope = phase id + attempt;主 agent / 非 executor 写 → scope = 'workflow'(无 attempt),scope 即来源标记,独立 `agent_type` 截取消除(executor 类型由 phase 表推导)。

schema 取**增量**(`active_phase` + `file_log` 并行字段),**不**重构 eval→phases:round 语义依赖 append-only 序列(computeRound = anchor 窗口长度、20 轮上限、retry 计数、backtrack 最新条目判定),per-phase 记录表达不了"第几轮",且 audit trail 与引擎三消费者刚经历 eval merge 迁移,触碰成本过高。引擎(`phase_next` / `backtrack`)零改动;进行中 change 沿用"legacy 须重建 change"惯例,不做迁移。

---

## 能力

### 新增能力

- **phase-lifecycle** — workflow.json 的 `active_phase` / `interrupted` 运行态、`phase_start` MCP 工具、`phase_log` 盖章清场集成、UserPromptSubmit sweep hook 与技能协议 `phase_start` 调用,为每个 attempt 提供起止耗时档案,并为文件归账提供门控状态。

### 修改的能力

- **workflow-file-inventory** — 存储由 `files` + `source` 重构为日志式 `file_log`(净状态改为派生视图);记录器增加门控(绑定 + active_phase)与 phase/workflow scope 路由;session 绑定刷新扩展到 `phase_start` 事件;"清单条目来源审计"要求取消,由 scope 标记取代。
- **change-files** — `append` 语义改为 workflow scope 按 path upsert(phase 审计记录保留);`set` 语义重定义为"删除涉及 path 的全部记录 + 追加 workflow 记录";`workflow_files` 查询输出形状不变但改读派生视图。

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/workflow.schema.ts` — 扩展 `active_phase`(对象或 null)、`interrupted[]`、`fileLogSchema`;`phase-log.schema.ts` 扩展可选 `start_at`
- `plugins/dev-team/bin/src/mcp.ts` — 注册 `phase_start` 工具
- `plugins/dev-team/bin/src/commands/phase-start.ts`(新)— `phase_start` 实现(校验 phase 属于工作流表,last-wins 写 active_phase)
- `plugins/dev-team/bin/src/commands/phase-log.ts` — 落盘条目盖章 `start_at` + 清 active_phase
- `plugins/dev-team/bin/src/lib/eval-json.ts` — `buildEntry` 支持可选 `start_at`
- `plugins/dev-team/bin/src/commands/record-files.ts` — 门控判定与 scope 路由;`phase_start` 事件识别并入绑定刷新(matcher 增 `__MCP:phase_start__`)
- `plugins/dev-team/bin/src/lib/workflow.ts` / `lib/` — `hasPhasePassed` 自 `commands/phase-next.ts` 下沉(供 MCP server 与 hook 同仓库直引);`executor.agent_type` 平台名解析辅助(门③)
- `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` — file_log 读写、(attempt, path) 键控覆盖/追加、派生净状态
- `plugins/dev-team/bin/src/modules/workflow/files/record.ts` — 归账管线改写 file_log(scope 由门控解析传入)
- `plugins/dev-team/bin/src/modules/workflow/files/files-query.ts` — 改读派生视图(输出形状不变)
- `plugins/dev-team/bin/src/commands/change-files.ts` — append/set 新语义委托
- `plugins/dev-team/bin/src/lib/c4-cross-ref.ts`、`commands/test-execution.ts`、`commands/test-resolve-paths.ts` — 读方切派生视图(契约不变)
- `plugins/dev-team/bin/src/hooks.ts` 与新子命令(如 `commands/sweep-phase.ts`)— UserPromptSubmit sweep
- `plugins/dev-team/hooks/hooks.canonical.json` — postToolUse matcher 增 `__MCP:phase_start__`;新增 userPromptSubmit 事件(claude 注册,cursor null)
- `plugins/dev-team/build/hooks-profile.ts` — canonical schema 增 `userPromptSubmit` 键与双平台包装
- `plugins/dev-team/skills/phase-*/SKILL.md`(8 个 phase 技能)— turn 协议在 phase_next 返回 phase 后增加一次 `phase_start`;`workflow-requirement` / `workflow-test-only` 如内嵌协议文本则同步
- `plugins/dev-team/package.json` — 版本号 bump(项目规则),并 rebuild 双平台产物

### 测试文件

- `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.test.ts`、`record.test.ts`、`files-query.test.ts` — fold 语义测试重做为 file_log 覆盖/追加与派生等价测试
- `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts` — 端到端重做:phase_next 绑定 → phase_start 开门 → 写入归账(phase/workflow scope)→ phase_log 落盘 → 同 session 再写被门②丢弃;**重绑竞态回归**(最终 pass 后再发 phase_next 事件,绑定不得复活归账);done 后 backtrack → phase_start 重开自愈;错误终态不开门
- `plugins/dev-team/bin/src/commands/phase-log.test.ts`、`phase-next.test.ts`(hasPhasePassed 下沉回归)、新增 `phase-start` 测试
- `plugins/dev-team/bin/src/commands/change-files.test.ts` — append/set 新语义
- `plugins/dev-team/build/hooks-profile.test.ts`、`build/__tests__/hooks-*` — userPromptSubmit 键与双平台产物
- `plugins/dev-team/bin/src/mcp.test.ts` — phase_start 注册与校验

### 删除文件

- 无(file_log 重构在既有文件内完成;不删除任何文件)

### 不要修改

- `commands/phase-next.ts` 的响应协议与 gate/round 逻辑(`hasPhasePassed` 允许原样下沉 lib,行为零改动)
- `commands/backtrack.ts` 的引擎行为(对 `file_log` 与 `active_phase` 保持中立)
- `lib/shell-file-ops.ts` 的 `extractFileOps` 三态提取
- `modules/workflow/files/gitignore.ts` 的过滤语义
- `commands/protect-files.ts` 的 PreToolUse 写保护
- 既有 eval 历史结构与 round 语义(append-only、computeRound、retry 计数、backtrack 判定)——`active_phase` / `file_log` 为并行新字段,不重构 eval→phases

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `schemas/workflow.schema.ts` / `phase-log.schema.ts` 扩展 | `workflowFileSchema` 接受 `active_phase`(对象或 null)与 `interrupted[]`;eval 条目接受可选 `start_at`;未知键保留行为不变(单测) |
| AC-2 | `commands/phase-start.ts` + `mcp.ts` 注册 | phase_start 校验 change 与 phase 后 last-wins 写入 active_phase,attempt 由 eval 历史推导且 retry 后递增;非法 phase 报错且 workflow.json 不变(单测) |
| AC-3 | `commands/phase-log.ts` 集成 | active_phase 匹配时条目盖章 start_at 并清空 active_phase;无/不匹配时条目无 start_at 且 active_phase 不动;输出 `{written, phase, attempt}` 形状不变(单测) |
| AC-4 | `commands/record-files.ts` 门控 | 四分支:未绑定丢弃(现状);绑定无 active_phase 丢弃 + stderr 诊断;绑定 + active + agent_type 匹配 → phase 记录;绑定 + active + 不匹配 → workflow 记录(单测) |
| AC-5 | 重绑竞态回归(端到端) | phase_log 最终 pass 之后再发 phase_next 事件(Verdict,done:true)→ 绑定刷新但后续写事件仍被门②丢弃,归账不复活;仿 `file-inventory-recording` 端到端 |
| AC-6 | `file-inventory.ts` / `record.ts` 重写 | dedupe key=(attempt, path):同 key 原位覆盖 op/scope/at(含 scope 翻转),跨 attempt/phase 追加;派生净状态与现行 `foldFileOps` 等价(等价测试重做通过) |
| AC-7 | 四读方适配 | `workflow_files` / `test-execution` / `test-resolve-paths` / `c4-cross-ref` 行为不变,改走派生视图;缺失 `file_log` 硬报错含重建指引,不回退 git(既有测试迁移通过) |
| AC-8 | `commands/change-files.ts` 新语义 | append → workflow scope 按 path upsert(phase 记录保留);set → 删除涉及 path 全部记录 + 追加 workflow 记录,未涉及 path 不动(单测) |
| AC-9 | 绑定刷新扩展 | phase_next 与 phase_start 事件均建立/刷新 `session_id → change` 绑定;注册表其余行为不变(单测) |
| AC-10 | UserPromptSubmit sweep | 绑定 change 有遗留 active_phase → 移入 interrupted[](end_at=now)并清空,eval 数组不变;无绑定 no-op;任何失败 exit 0;canonical 增 userPromptSubmit 键,双平台构建产物 claude 注册、cursor 不产出(单测 + 构建测试) |
| AC-11 | 技能协议 | 各 phase 技能 SKILL.md 含 phase_next → phase_start 序列,retry 重跑重新 phase_start(grep 技能文本核验) |
| AC-12 | 兼容与零改动 | 旧 files-only change 经各消费方硬报错;phase_next/backtrack 响应与 gate 行为零改动;extractFileOps / gitignore / 写保护行为不变(既有测试通过) |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 门③ agent_type 映射失配(phase 表存 `__CALL_AGENT:...__` token,hook 事件携带平台实际 agent 名) | executor 写被误标 workflow scope,审计粒度降级 | 中 | design 定义 token → 平台名解析;误判仅损粒度,净状态与门②不受影响 |
| revert(`git restore`)在 file_log 中的表达未定 | 派生净状态可能偏离现行 foldFileOps 等价性 | 低 | 以等价为约束 design 拍板;fold 等价测试兜底 |
| file_log 体积增长(长 change 多轮 retry / 多 phase) | workflow.json 变大、读放大 | 低 | 同 key 覆盖已抑制大部分;压缩策略不在本变更,必要时二期 |
| Cursor 不支持 UserPromptSubmit 事件 | 中断后 active_phase 遗留、门常开,窗口回到"完成后泄漏"形态 | 中 | 明确降级接受并文档标注;TTL 兜底为二期备选 |
| 多 session 并发绑同一 change | change 级 active_phase last-wins 互踩,门控/计时串账 | 低 | 实际单人使用;记为已知限制,design 可评估 per-session active_phase |
| 进行中 change 不迁移 | 旧 `files` 形态 change 需重建 | 确定 | 沿用既有"legacy 须重建 change"惯例 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 释放绑定机制(A2 终态 unbind / hook 守卫) | 废弃,改为 phase_start + active_phase 门控 | A2 被完成 turn 的 Verdict phase_next 重绑系统性击穿;门控天然免疫重绑,且改动全在 MCP 服务端与 canonical 清单,平台无关 | A1 hook 读 tool_response;B 完成标记工具;C Stop 按 turn 解绑;D lookup TTL |
| schema:增量 vs phases[] 重构 | 增量(active_phase + file_log 并行字段) | round 语义依赖 append-only 序列;audit trail 敏感;引擎三消费者刚迁移 | phases[] per-phase 重构 |
| 存储形态 | 日志式 file_log,净状态派生 | phase 级触达审计免费获得;同 key 覆盖控体积;4 读方走派生视图契约不变 | 保留 files + 旁挂 scope 映射 |
| 主 agent 擅自写 | 记录 + 标记(workflow scope) | 硬丢弃违背"只多记、不漏记危险操作"原则;标记保住净状态完整性且获得审计粒度 | 门③硬门控不入账;PreToolUse 直接 block |
| 中断收口 | UserPromptSubmit sweep | Stop 在用户中断与 API 错误时不触发(平台查证);每条新用户消息前收口最小且确定 | 接受窗口;TTL |
| 兼容策略 | 不迁移,legacy 须重建 | 沿用既有惯例;进行中 change 数量少 | 写 files→file_log 迁移器 |

### 待决问题

- revert(`git restore`)在 file_log 中的表达:增加 `op:'revert'` 记录 vs 移除该 path 的既有记录——design 拍板,约束为派生净状态与现行 `foldFileOps` 等价(revert 后 written/deleted 均不含该 path)
- 门③的 `__CALL_AGENT:...__` token → 当前平台实际 agent 名的解析机制与落点
- `change_files` set 的 log 级映射细节(倾向"删除涉及 path 全部记录 + 追加 workflow 记录",design 定实现形态)
- phase_log 在无 active_phase / phase 不匹配时的防御行为细节;phase_start 重入(retry)与 attempt 推导的边界
- 多 session 并发绑同一 change 时 change 级 active_phase 是否需要 per-session 化(design 权衡,当前接受 last-wins)

---
