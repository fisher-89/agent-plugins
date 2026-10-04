# desktop change 会话可见性（记录 / 查询 / 节点查看会话）

日期：2026-10-03（探索轮）

## 背景与诉求

desktop 执行 change（change-flow run）时：

1. 同 agent debug 一样记录会话信息；
2. infra/agent 能根据会话 id 显示进行中或已完成的会话；
3. 页面中节点增加「查看会话」功能。

## 探底结论：现状盘点

### 已经成立的（比诉求字面预期多）

**① 会话记录已在发生**——两条轨道同一条内核链路，write-through 落库同构：

```
             SessionKernel（治理 + write-through）
             create_session / begin_turn / append_sealed / finish_turn
                          │ SessionSink (StoreSink)
            ┌─────────────┴─────────────┐
            ▼                           ▼
  agent debug（/agent）        change-flow run（/changes/:name）
  agent_start 命令             KernelWorkerPort::execute
  source = "debug"             source = "change"
  source_ref = None            source_ref = "<change>/<phase>/<role>/<attempt>"
```

证据链：

- `change_flow_start` → `compose_turn`（`StoreSink` 注入内核，`infra/agent/compose.rs:52`）
- worker 每轮 `composed.begin(..., turn.provenance)`（`infra/agent/worker.rs:55`）
- provenance 由 walker 组装 `SOURCE_CHANGE`（`core/orchestration/walker.rs:33, 811-820`）
- 轮行终态（running/completed/failed/stopped）、密封事件、聚合统计全部落 workspace 库

**② 查询面已有两员**（`commands/exec/mod.rs:135-162`，走 infra/agent `session_query`）：

- `agent_sessions(root, source, source_ref)` — 按来源过滤的会话清单（含轮行 → 可推导 running/completed）
- `agent_session_transcript(root, session_id)` — 按会话 id 重放全史密封转录

**③ change 详情页已有「节点→会话」通路**：点运行节点 / eval 节点 → DetailDrawer →
`SessionTranscriptPanel` → `useSessionTranscript` 按 sourceRef 反查 + 重放 + 实时事件
seq 归并 + running 态推导。

### 真实缺口

| # | 缺口 | 细节 |
|---|------|------|
| 1 | 无「按 id 单查会话行+状态」面 | `agent_sessions` 只支持 source/source_ref 精确匹配；`agent_session_transcript` 只回事件流不回会话行/状态。store 有 `find_session` 底座，core `SessionQuery` 契约无单查面 |
| 2 | change 会话在所有清单 UI 不可见 | `/agent` 历史区硬编码 `source='debug'`（`use-agent-run-history.ts:47`） |
| 3 | 节点↔会话链接靠约定反查，非记录在案的 id | workflow.json 的 PhaseLog 不记 session id；run 结束图回落后 eval 节点联动靠 `<change>/<phase>/<role>/<attempt>` 定式反查，同 ref 多会话取「最近一条」（`use-session-transcript.ts:69`）；**decision 会话历史不可达**（eval 节点只联动 executor+evaluator，`detail-drawer.tsx:71`） |

## 拍板结论（本轮决策）

- **Q1 → A：增强现有抽屉**。roleRefs 补 decision 会话 tab + WorkerAgent 节点加显式
  「查看会话」入口；不建独立会话 route。
- **Q2 → A：`/agent` debug 页历史区加 source 筛选**（debug / change / 全部），复用
  `AgentRunHistory` 整套。
- **Q3 → B：session id 写进 workflow.json**（持久节点↔会话链接，弃 sourceRef 反查约定）。
  ⚠️ 这是对「golden wire contract 冻结」既定约束的**显式突破**——v1 wire format 变更，
  design 阶段需处理 golden 夹具对账（新代际 fixture 或显式重生成），并保持
  `#[serde(default)]` 向后兼容旧文件。
- **Q4 → 做：新增 `session_detail(root, session_id)` 单查命令**（回 row + stats + turns，
  状态可自轮行推导），core `SessionQuery` 契约加单查面，命令薄包装。

## Q3B 落点草图（design 阶段展开）

- `PhaseLog`（`core/workflow/src/model/workflow.rs:77`）加可选字段承载本 attempt 的
  session id（executor / evaluator / **decision 三槽位**；形态待 design 定：扁平三字段
  vs 结构体）。
- 写面：`PhaseLogInput`（`core/workflow/src/write/phase_log.rs:17`）随行携带 session id；
  walker `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id`
  取值传入。
- 读面：detail DTO（queries 层）暴露 eval attempt 的 session id（DTO 纯 derive 零字段
  属性的既定口径不变）；前端 bindings 再生成。
- 前端联动：抽屉转录面板优先用记录在案的 session id 直查（`session_detail` +
  `agent_session_transcript`），sourceRef 反查退居兜底（旧数据）。

## 遗留开放点（进 proposal / design 前需收口）

1. ~~decision 会话的持久挂点~~ **已拍板（第二轮）**：随 phase_log 条目增加第三槽位
   （decision 与 executor / evaluator 同驻一条目）。派生设计点：decision 会话产生于
   触发死锁的 fail 条目落账**之后**（max_retries_exceeded → 决策 → backtrack/retry），
   槽位写入时机（backtrack 写挂 vs 显式 amend 写面操作 vs 随下一 attempt 条目）留
   design 定。
2. **golden 对账方式**：加字段后旧 golden 夹具如何处置（冻结突破的执行细节）。
3. **进行中会话的清单时效**：列表为查询时刻快照（显式刷新模式，与 debug 页一致），
   不做推送——按现状接受，不在本 change 范围。
