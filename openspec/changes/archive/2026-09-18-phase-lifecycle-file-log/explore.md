# 会话绑定释放:工作流终态清除 file inventory 归账绑定

日期:2026-09-18 · 状态:方案已定(A2),待 phase-proposal 收敛

## 问题

PostToolUse `record-files` hook 通过 `session_id → change` 持久注册表(`<tmpdir>/dev-team-hooks/<sha256(projectRoot)前16位>.json`)归账文件操作。绑定仅在 `phase_next` MCP 调用事件时建立/刷新(`record-files.ts:100-103`),**没有任何释放机制**:

- `session-registry.ts` 只有 `bindSession` / `lookupChange`,无 unbind;
- 24h TTL 只在 `bindSession` 内懒惰修剪,`lookupChange` 不做新鲜度检查;
- spec(`openspec/specs/workflow-file-inventory/spec.md:92-114`)明文"此后同一 session_id 下的工具事件 SHALL 查表归账",即绑定后全归账是现行规定。

### 两个泄漏窗口

```
turn1  /workflow 技能    phase_next ──绑定──▶ change-X     ✓ 正确
turn2  普通指令          绑定仍指向 X                      ✗ 中途泄漏
turn3  /workflow 继续    phase_next 刷新                   ✓
turn4  acceptance pass   done:true,无解绑                  ⚠
turn5  普通指令          绑定仍指向 X                      ✗ 完成后泄漏
turnN  手动 archive      目录被 mv → writeFileInventory 抛错被吞(副作用式释放,非设计)
```

**完成后泄漏(turn5)伤害最大**:本仓库习惯不自动 archive、完成后人工检查,检查期内同会话的顺手修改会污染 `files` 清单,而该清单是 implementation-evaluator 的核对范围(替代 git-based scope),污染 → "实现改了设计外文件"假阳性。事后补救仅 `change_files` MCP `op:"set"` 手工覆写。

## 已验证事实

- `phase_next` 终态返回 `{done: true, next_phase: null}`(`phase-next.ts:144-162`);`hasPhasePassed` 已 export(`phase-next.ts:207`),`getWorkflowType`/`getPhaseTable` 现成。
- `phase_log` 是写操作(`appendEntry` 落盘 eval 条目,`phase-log.ts:71-76`),返回 `{written, phase, attempt}`;插入点在 appendEntry 成功之后。
- 工作流技能协议:**每个 turn 以 `phase_next` 开头**(phase-implement SKILL.md:"At the start of this turn... Call phase_next"),绑定按 turn 刷新,清掉的绑定可自愈重绑。
- 平台语义(2026-09 查证 Claude Code hooks 文档):
  - PostToolUse 对 MCP 工具**透传 `tool_response`**(结构化对象;MCP 输出无 schema 校验,需防御性解析)→ A1 可行但不采用;
  - **Stop 钩子在用户中断(Esc)与 API 错误时不触发**(API 错误走 StopFailure)→ C 存在实锤的边缘泄漏窗口;
  - Stop 无 matcher;`session_id` 为所有 hook 事件的公共字段。

## 方案对比

| | A1: hook 读 tool_response | **A2: 终态清除(选定)** | B: 标记 MCP 调用 | C: Stop 按 turn 解绑 | D: lookup 加 TTL |
|---|---|---|---|---|---|
| 触发点 | phase_next 返回 done 时 hook 解绑 | phase_log 落盘 pass 后检测 all-passed,清除该 change **全部**绑定 | 技能末尾显式调"完成"标记工具 | 每 turn 结束解绑 | lookup 检查新鲜度 |
| 覆盖 完成后泄漏 | ✓ 仅当次 session | ✓ 该 change 所有 session | ✓ | ✓ | 部分 |
| 覆盖 中途泄漏 | ✗ | ✗ | ✗ | ✓ | 部分 |
| 改动面 | record-files.ts | phase-log.ts + session-registry.ts | 新 MCP 工具+全部技能+matcher | hooks 清单+构建+新子命令 | session-registry.ts 一处 |
| Cursor 端同享 | ✓ | ✓ 纯 MCP 层 | ✓ | ✗ 需 Cursor 支持 Stop | ✓ |
| 记录完整性风险 | 无 | 无 | 无 | 有(turn 分裂/后台 agent 跨 Stop 丢记录) | 有(长 turn 中途丢) |

## 决定:A2

理由:改动最小且全在 MCP 服务端(hook 清单/构建/技能零改动);平台无关;change 级清除强于 session 级(一并释放残留旧绑定);精确对齐"不自动 archive、all passed 即事实终点"的流程;done 后 backtrack 重跑由下一个 phase_next 自愈重绑,无副作用。

### 设计要点

1. `session-registry.ts` 新增 `unbindChange(projectRoot, change)`:读注册表 → 过滤掉 `entry.change === change` 的条目 → 重写;fail-open,不抛错。
2. `phase-log.ts`:`appendEntry` 成功后重读 entries,`getWorkflowType(change)` + `getPhaseTable` + `hasPhasePassed` 计算 all-passed;为真则 `unbindChange`。注意 `hasPhasePassed` 目前位于 `commands/phase-next.ts`,设计时考虑下沉到 lib(workflow 或 eval-json)避免 command 间互引。
3. 触发天然只在"完成流程的那条 pass/skip 落盘"时发生(fail 不计入 hasPhasePassed),无需额外条件。
4. 返回值保持 `{written, phase, attempt}` 不变(清绑定是 hook 缓存卫生,不进协议);或设计阶段再议是否加可观测字段。

### 残留缺口(明确接受)

- 中途泄漏(turn2)不解决——实际使用中工作流间歇顺手改代码较少见;若日后证实常见,二期评估 C(Stop 按 turn 解绑),需接受中断不触发 Stop 的边缘窗口与构建系统/Cursor 平台扩展成本。
- 错误终态(round_limit_exceeded / max_retries_exceeded)**不**清除:change 未完成,续跑时 phase_next 会重绑;彻底弃置 change 的场景罕见,接受残留(待 proposal 阶段最终拍板)。
- `openspec/specs/workflow-file-inventory/spec.md:92-114` 的"绑定后全归账"条款需同步修订,属正式 spec 变更 → 走 phase-proposal。

## 补充:完成 turn 的重绑竞态与双层设计(2026-09-18 二轮)

初始 A2 草案有实锤缺陷:技能协议规定每 turn 末尾必调 Verdict `phase_next`(返回 `done:true`),而 hook **只读 tool_input、无条件绑定**——该调用发生在最终 `phase_log` 清除之后,同 turn 内即把绑定绑回,清除被系统性击穿。

### 修正:两层防护

1. **hook 状态守卫**(record-files.ts):phase_next 事件改为状态条件式——读 eval store(`getWorkflowType`+`getPhaseTable`+`hasPhasePassed`,与 MCP server 同仓库直引;`hasPhasePassed` 需从 commands/phase-next.ts 下沉到 lib)——all-passed → `unbindChange`(幂等);否则 → `bindSession` 照旧。不依赖 `tool_response`,Cursor 同享。backtrack(stale → not all-passed)与错误终态自动落入"照常绑定"分支,自愈保留。
2. **phase_log 侧清除**(保留):all-passed 落盘即释放该 change 全部绑定——防"完成 turn 在 Verdict phase_next 之前被中断"的残留(hook 守卫管不到不曾发生的 ④')。

### 主/子代理写入

子代理工具事件与主代理同 session_id,归账无区别(`agent_type` 仅审计戳)。完成 turn 内:unbind 之前(含 executor 全部写)正常归账;unbind 之后的主 agent 写丢弃——规定流程该区间为 Verdict→Report 无写操作,临时写丢弃也符合"流程已完成"语义,`change_files` append 兜底。

### 残余窗口(接受)

逐中断点核对:
- **最终 phase_log 之后、Verdict phase_next 之前中断**:phase_log 层在该时刻已释放绑定,无泄漏。
- **最终 phase_log 之前中断**(evaluator 未落盘即被杀):all-passed 未达成,工作流本就未完成——turn 开头的绑定残留,与错误终态同一立场:绑定保留等待续跑,下一次 phase_next 走"not all-passed → 照常绑定"自愈。若用户此后不再续跑而发普通指令,写仍归账——与"中途泄漏"同类,属 A2 明确不解决的缺口。

## 三轮(2026-09-18):phase_start + 状态门控,取代 A2

新需求(记录每 phase 耗时)与归账问题合并审视后,方案升级:`phase_start` 显式开启 + **active_phase 状态门控**,A2 的 unbindChange/bind-guard 两层全部删除。

### 门控规则(record-files)

写入事件归账当且仅当:① session 绑定到 change(注册表机制不变,phase_next 与 phase_start 事件都刷新绑定);② 该 change 存在 `active_phase`;③ [待拍板] 事件 `agent_type` == running phase 的 `executor.agent_type`(主 agent 无 agent_type,天然被挡)。

效果:中途泄漏、完成后泄漏全堵在门②;完成 turn 的 Verdict phase_next 重绑变得无害(门已关),上轮的重绑竞态消失;backtrack/错误终态经新 phase_start 自愈。

### schema:增量,不重构 eval→phases

phases[] 重构的硬伤:round 语义依赖 append-only 序列(`computeRound` = anchor 窗口长度、20 轮上限、retry 计数、backtrack 最新条目判定),per-phase 记录表达不了"第几轮";且触碰已知敏感的 audit trail、引擎三消费者全改、刚做过 eval merge 迁移。

增量方案:`active_phase: {phase, attempt, start_at} | null` 新字段做门控与计时起点;`phase_log` 落盘条目时从 active_phase 盖章 `start_at`(既有 `timestamp` 即 end_at)。per-attempt 耗时 = timestamp − start_at,retry 独立计时,status 从 verdict/stale/skipped 推导,零迁移。

### 中断收口:UserPromptSubmit sweep

phase 生命周期单 turn,但用户中断时 Stop 不触发(已查证),active_phase 会遗留 running → 门常开。收口:UserPromptSubmit hook 在每条新用户消息前 sweep——绑定 change 存在遗留 active_phase 即关闭(end_at=now,移入 `interrupted[]` 留档;**不**写 eval 条目,避免烧 retry 配额)。Cursor 端口若不支持该事件则降级保留中断窗口。

### 协议/清单改动面

- MCP:新工具 `phase_start(change, phase)`(开 active_phase);`phase_log` 增盖章 start_at + 清 active_phase;`phase_next` 保持只读不动。
- hooks.canonical.json:PostToolUse matcher 增 `__MCP:phase_start__`(绑定刷新);新增 UserPromptSubmit 事件(sweep)。构建系统需支持新事件键。
- 技能协议:每个 phase 技能在 phase_next 返回 phase 后加一次 phase_start 调用(retry 重跑同样要 re-start——attempt 计时与门控都以 start 为准)。

### 待拍板

1. schema:增量(推荐)vs phases[] 重构。
2. 中断收口:UserPromptSubmit sweep(推荐)vs 接受窗口 vs TTL。
3. 主 agent 擅自写:门③硬门控不入账(推荐)vs 记录+source 标记 vs PreToolUse 直接 block。

## 四轮(2026-09-18):存储层重构——file_log 取代 files+source

主 agent 擅自写的决策:**记录+标记**,并顺势重构存储:workflow.json 不再区分 `files{written,deleted}` 与 `source` 戳,改为日志式 `file_log`,每条记录 `{op, scope(phase/workflow), path, at, agent?}`;**单轮 phase 内多次操作覆盖更新,跨 phase 生成新记录**。

### 结构(定稿,2026-09-18 拍板)

`file_log: [{op:'write'|'delete', scope:'<phase-id>'|'workflow', attempt:<n>?, path, at}]`:
- dedupe key = **(attempt, path)**:同 key 原位覆盖 op/scope/at(后写者胜,含 scope 翻转);跨 attempt/跨 phase 追加新记录。attempt 字段是按轮 key 的必然组成。
- scope 即标记(方案 a):executor 子代理写 → scope=phase id + attempt;主 agent 及非 executor 子代理在 phase 期间的写 → scope='workflow'(无 attempt)。agent 字段取消(executor 类型由 phase 表推导)。
- 净状态派生:按 log 顺序同 path 后条胜,等价现行 foldFileOps,4 个读方契约不变。
- 门②不变:'workflow' scope 仅在 running phase 期间产生,不是"随时都记"。
- hook 判定:绑定?→ active_phase?→ agent_type==executor.agent_type ? phase 记录 : workflow 记录。

### 留给 design 阶段

- change_files `append` 在 workflow scope 内按 path upsert 即可;
- change_files `set` 映射:"删除涉及 path 的全部记录 + 追加 workflow 记录"(会抹掉那些 path 的 phase 审计,与现行 set 覆盖净状态精神一致,design 拍板)。

### 波及面(已 grep 核实)

- 写路径重写:`modules/workflow/files/file-inventory.ts`、`record.ts`(fold 桶 → 按 key 覆盖/追加)。
- 读方契约不变(走派生视图 fold log→每 path 最后 op):`files-query.ts`(workflow_files)、`lib/c4-cross-ref.ts`(archi_check)、`commands/test-execution.ts`、`commands/test-resolve-paths.ts`。
- 手工通道 `commands/change-files.ts`:append→scope=workflow 记录;`set` 整体覆写语义需重新定义(替换整个 log?)。
- 引擎(phase-next/backtrack)零改动;spec(workflow-file-inventory)重写;fold 语义测试重做。
- 兼容:进行中 change 的旧 `files` 字段——沿用"legacy 须重建 change"的既有惯例,不做迁移。

### 收益

phase 维度的文件触达审计 + active_phase 的 start/end 耗时,拼成完整 phase 档案;门②③语义不变(hook 只在 running phase 期间入账,主 agent 写记录而非丢弃)。

## 测试要点(供 test-design 参考)

- 仿 `file-inventory-recording.test.ts` 端到端:phase_next 绑定 → 写入归账 → phase_log 落盘最终 pass → 同 session 再 Write → 走"未绑定 change,丢弃"路径(stderr 诊断)。
- **重绑竞态回归**:phase_log 最终 pass 之后再发 phase_next 事件(模拟 Verdict 调用,done:true)→ hook 走 all-passed 分支 → 绑定不得复活。
- done 后 backtrack → phase_next 重绑 → 归账恢复(自愈)。
- 多 session 绑同一 change:终态清除释放全部。
- 错误终态/未完成流程:不清除。
