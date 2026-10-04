# 提案: desktop-change-session-visibility

> **变更**: desktop-change-session-visibility
> **日期**: 2026-10-03
> **状态**: draft

---

## 问题

desktop 执行 change（change-flow run）时，用户对会话的可见性存在三个真实缺口（探索轮盘点，会话记录本身已成立——两条轨道同一条内核链路，`source="change"` 的会话随 run 落库同构）：

1. **无「按 id 单查会话」面**：core `SessionQuery` 契约只有清单（source/source_ref 过滤）与转录重放两员；store 虽有 `find_session` 底座，命令面无 `session_detail(root, session_id)` 单查入口——「根据会话 id 显示进行中或已完成会话」的诉求没有直接承载面。
2. **change 会话在清单 UI 不可见**：Agent 调试页历史区硬编码 `source='debug'`（`use-agent-run-history.ts`），change 产生的会话在任何清单界面都看不到。
3. **节点↔会话链接靠约定反查、非记录在案**：workflow.json 的 eval 条目不记 session id；抽屉转录联动靠 `<change>/<phase>/<role>/<attempt>` sourceRef 定式反查（同 ref 多会话取最近一条），且 decision 会话完全不可达（eval 节点只联动 executor + evaluator 双会话）——run 结束后 decision 会话无法从页面找回。

---

## 提案

四项拍板（探索轮决策），全部为既有能力的增量增强，无新增顶层能力：

1. **新增 `session_detail(root, session_id)` 单查命令**：core `SessionQuery` 契约补单查方法（回会话行 + 聚合统计 + 轮行，运行状态自轮行推导），命令层薄包装；store `find_session` 底座已存在，零 store 变更。
2. **Agent 调试页历史区加来源筛选**：debug / change / 全部三态，复用 `AgentRunHistory` 整套取数与重放；默认 debug（现状不变），仍为显式刷新模式。
3. **session id 写进 workflow.json eval 条目**（持久节点↔会话链接，弃纯 sourceRef 反查约定）：eval 条目增加 executor / evaluator / decision 三槽位的可选会话 id 字段；`phase_log` 写面随行落账，walker 从 `WorkerTurnOutcome.session_id` 取值传入；detail DTO 暴露槽位；前端转录联动优先按记录在案的 session id 直查、sourceRef 反查退居旧数据兜底。**此为「golden wire contract 冻结」的显式突破**（`backtrack_to` / `backtrack_reason` 先例同型）：serde `#[serde(default)]` 读兼容旧文件，golden 快照按既有显式再生成流程对账。
4. **增强详情抽屉**：eval 节点会话联动补 decision 第三 tab；WorkerAgent 节点提供显式「查看会话」入口；不建独立会话 route（单一交互入口架构不变）。

不在范围：进行中会话的清单推送时效（列表为查询时刻快照，沿 debug 页显式刷新口径，探索轮已明确接受）。

---

## 能力

### 新增能力

- 无（全部为既有能力的增量增强）

### 修改的能力

- **desktop-agent-execution** — `SessionQuery` 契约补单查面 + `session_detail` 命令薄包装；Agent 调试页历史区加 source 筛选
- **desktop-workflow-write-face** — eval 条目新增会话槽位字段（schema 显式演进，突破「零新字段」约束）；`PhaseLogInput` 随行携带会话槽位
- **desktop-change-queries** — 详情聚合的 attempt 记录暴露三槽位会话 id（缺字段降级 null）
- **desktop-change-flow-view** — 抽屉转录联动升级：记录 id 直查优先 + sourceRef 兜底、eval 节点补 decision 会话 tab、WorkerAgent 节点显式「查看会话」入口
- **desktop-corpus-regression** — 语料补含会话槽位的新代际样本；golden 快照按显式流程再生成对账

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/agent/src/port.rs` — `SessionQuery` trait 增单查方法（`find_session_detail` 形状，命名 design 定稿）
- `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs` — 单查实现（`find_session` 底座 + 轮行推导状态）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — `session_detail(root, session_id)` 命令薄包装；`src/lib.rs`（builder 注册处）同步注册
- `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` — `PhaseLog` 增三槽位可选字段（`#[serde(default)]`，形态：扁平三字段 vs 结构体由 design 定稿）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` — `PhaseLogInput` 增会话槽位随行 + 落账写入（显式在位才写，沿 skipped / start_at 扩展字段先例）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值传入；decision 路径会话 id 传递（写挂时机 design 定稿）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `AttemptRecord` DTO 暴露三槽位会话 id（纯 derive，零字段属性口径不变）
- `packages/desktop/src/types/generated/bindings.ts` — 经 export-bindings 再生成
- `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` — source 筛选参数化（去硬编码 `'debug'`）
- `packages/desktop/src/views/agent/agent-debug-view.tsx`（或历史区组件落点）— 来源筛选 UI（三态切换）
- `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` — 记录 id 直查优先 + sourceRef 兜底
- `packages/desktop/src/views/changes/flow/detail-drawer.tsx` — `selectionRoleRefs` 补 decision；「查看会话」入口接线
- `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` — 三会话 tab（executor / evaluator / decision）
- `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/`、`tests/golden/` — 新代际样本收录 + golden 快照显式再生成
- `packages/desktop/package.json` — version 0.4.3 → 0.4.4（**归档时执行**，见 AC-11）

### 测试文件

- `packages/desktop/src-tauri/crates/core/agent/src/port_test.rs` — 单查契约测试
- `packages/desktop/src-tauri/crates/infra/agent/src/store_port_test.rs` — 单查实现测试（含跨 workspace 库隔离）
- `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` — `session_detail` 薄包装测试
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs` — 会话槽位落账 + 旧文件读兼容
- `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` — 新代际样本覆盖 + golden 对账
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` — DTO 槽位暴露与降级
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` — session id 随 phase log 传递
- `packages/desktop/src/views/agent/`、`packages/desktop/src/views/changes/` 对应 hook / 组件测试（筛选、直查优先、decision tab、查看会话入口；data-testid 挂钩）

### 删除文件

- 无

### 不要修改

- `packages/desktop/src/views/changes/flow/run-state.ts` — 无新事件词汇，reducer 与状态模型零触点
- `AgentTimeline` / `agent-adapter.ts` / `use-agent-chat.ts`（desktop-agent-chat-infra 渲染与传输基建）— 复用，不建第二套时间线
- `packages/desktop/src-tauri/crates/infra/store/` — `find_session` 底座已存在，store schema 零变更
- 既有 golden fixtures 的样本文件内容（`v0-a` … `v2-b` 等）— 仅 golden 快照按显式流程再生成，样本不改写
- 插件侧 TS 写路径（skill / MCP 轨道）— 冻结不改
- `PIPELINE_PHASES`、9 列布局、时间序边推导规则 — 图结构不变
- desktop 端既有命令签名（`agent_sessions` / `agent_session_transcript`）— 增量新增，不收窄

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `SessionQuery` 单查面 | core `SessionQuery` trait 增单查方法；实现按 session id 返回会话行 + 聚合统计 + 轮行，运行状态可自轮行推导（有 running 轮行即 running）；不存在 id 显式 `Err`、blank root 空结果；port 测试与 store_port 测试覆盖（含跨 workspace 库隔离） |
| AC-2 | `session_detail` 命令 | `session_detail(root, session_id)` 经 builder 注册、bindings 再生成；命令体三件事纪律（参数转换 → 调用 → 错误映射）；mod_test 覆盖 |
| AC-3 | 调试页来源筛选 | Agent 调试页历史区提供 debug / change / 全部三态筛选，默认 debug；切换后按 `agentSessions(root, source或null, null)` 重查，显式刷新模式不变（无轮询无订阅）；hook 测试覆盖 |
| AC-4 | eval 条目会话槽位 | `PhaseLog` 模型增 executor / evaluator / decision 三槽位可选字段；`phase_log` 写面随 `PhaseLogInput` 落账（显式在位才写）；旧 workflow.json（无槽位字段）读解析不报错；serde 写出仍可被插件侧解析面读取（fixture 对照绿） |
| AC-5 | walker 传递会话 id | `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值随输入落账；decision 会话收口后其 id 记录在案（写挂时机按 design 定稿实现）；walker_test 覆盖 |
| AC-6 | 详情 DTO 暴露 | `AttemptRecord` 暴露三槽位会话 id；无槽位的旧条目三值均 null 不报错；bindings 再生成一致（diff 守卫绿）；detail_test 覆盖 |
| AC-7 | 抽屉 decision 联动 | eval 节点抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位无记录时该 tab 呈空态，MUST NOT 虚构会话或误挂他 attempt 会话；组件测试覆盖 |
| AC-8 | 记录 id 直查优先 + 查看会话入口 | 有槽位 id 时转录面板按 session id 直查（`session_detail` + `agent_session_transcript`），无槽位回退 sourceRef 反查（旧数据兜底）；WorkerAgent 节点提供显式「查看会话」入口且打开同一抽屉转录联动；hook / 组件测试覆盖 |
| AC-9 | 语料与 golden 对账 | fixtures 增至少一个含会话槽位字段的 workflow.json 样本；golden 快照经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成后复核一致，diff 人工确认仅限新增槽位键；既有样本文件内容零改写；corpus 测试绿 |
| AC-10 | 全管线回归 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `vp test`、`cargo test --workspace` 全绿，无新增豁免条目 |
| AC-11 | 版本交付（归档时执行） | 归档提交将 `packages/desktop/package.json` version 0.4.3 → 0.4.4（用户可见变更）；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| DTO 新键触发全量 golden 快照 diff（线面 null 不省略） | corpus 回归测试全红，需逐 fixture 确认 | 高（必然发生） | 沿 corpus-regression 既有显式再生成流程；diff 人工确认清单预期「仅新增槽位键、其余逐字节不变」，作为 AC-9 证据留痕 |
| 插件侧解析面对 eval 新字段的行为未实测（若 strict 拒绝未知键则插件读路径炸） | 双写并存期插件轨道读 desktop 写出的 workflow.json 失败 | 低 | `backtrack_to` / `backtrack_reason` 先例同型（旧条目兼容测试在案）；AC-4 以 fixture 对照验收「serde 写出可被插件解析面读取」；风险如坐实则回退为仅 desktop 读取、插件侧兼容另立 change |
| decision 槽位写入时机复杂（decision 会话产生于 fail 条目落账之后） | decision 会话 id 可能晚于其宿主条目存在，落账路径多一分支 | 中 | 三方案（backtrack 写挂 / 显式 amend 写面操作 / 随下一 attempt 条目）留 design 定稿；保底口径：decision 联动在槽位缺席时呈空态不虚构 |
| 新代际语料样本来源未定（fixtures 收录自归档 change，含槽位样本需先有落账实例） | AC-9 的样本收录可能阻塞或需手工构造 | 中 | design 定稿：手工构造最小样本 vs 随首个含槽位归档 change 收录；不阻塞其余 AC |
| `session_detail` 不存在 id 的语义选择（Err vs 空态）与消费方预期不符 | 前端兜底逻辑返工 | 低 | 拍板 Err（单查语义：查无此 id 是调用方错误，与清单空态区分）；备选已记录于决策表 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| Q1: 节点会话查看建独立会话 route 还是增强抽屉？ | 增强现有抽屉（roleRefs 补 decision + WorkerAgent 节点显式「查看会话」入口） | 单一交互入口是既有架构（列头与节点共用一抽屉），route 会引入第二套交互面 | 独立 `/changes/:name/sessions/:id` route |
| Q2: change 会话如何在清单 UI 可见？ | `/agent` 调试页历史区加 source 筛选（debug / change / 全部），复用 `AgentRunHistory` 整套 | 取数、重放、刷新模型零新概念；默认 debug 保持现状不惊扰调试页既有用户 | change 详情页另建会话清单面板（范围膨胀） |
| Q3: 节点↔会话链接靠 sourceRef 反查约定还是记录在案 id？ | session id 写进 workflow.json eval 条目（三槽位），前端直查优先、反查兜底 | 持久链接消解「同 ref 多会话取最近一条」的歧义与 decision 不可达；`backtrack_to` / `backtrack_reason` schema 演进先例同型 | 维持纯反查约定（decision 永久不可达，歧义留存） |
| Q4: 按会话 id 单查会话信息走什么面？ | 新增 `session_detail(root, session_id)` 命令 + core `SessionQuery` 单查方法，命令薄包装 | store `find_session` 底座已在，补齐「根据 id 显示进行中/已完成会话」的正脸；清单命令保持 source/source_ref 语义不变 | 复用 `agent_sessions` 加 id 参数（清单语义被单查污染） |
| golden wire contract 冻结如何突破？ | 显式突破：serde `#[serde(default)]` 读兼容 + golden 按既有 `DESKTOP_GOLDEN_REWRITE=1` 流程显式再生成、diff 人工确认 | 冻结的动机（防静默劣化）由「显式行为」条款继续守护；写面 schema 演进本就是该流程的预期场景 | 不动 workflow.json，继续反查约定（放弃 Q3B 收益） |
| `session_detail` 查无此 id 的语义？ | 显式 `Err`（严格语义，与写面「查无此相位显式 Err」同文化） | 单查与清单空态语义区分；调用方 bug（悬挂 id）早暴露 | 返回空态（与清单语义同形，需前端再辨「无会话」与「查失败」） |

### 待决问题

- 会话槽位字段形态：eval 条目内扁平三字段（`executor_session_id` / `evaluator_session_id` / `decision_session_id`）vs 结构体聚合——design 定稿（含 JSON 键名与 serde alias 口径）
- decision 槽位写入时机：backtrack 写挂 vs 显式 amend 写面操作 vs 随下一 attempt 条目——design 定稿（decision 会话产生于 fail 条目落账之后的时序约束在案）
- golden diff 人工确认清单的验收形态（预期逐 fixture 仅新增槽位键）——design 阶段产出
- 新代际语料样本来源（手工构造 vs 随首个含槽位归档 change 收录）——design 定稿

---
