# 提案: agent-core-session-kernel

> **变更**: agent-core-session-kernel
> **日期**: 2026-10-01
> **状态**: draft

---

## 问题

### bug 1: SDK 引擎碎事件（用户可见——debug 页一轮回复碎成几十条 1-2 字多行）

- rig-core 0.42 流面不对称（`streaming/mod.rs` 枚举定义）：`Text` 只有增量变体（注释原话「Text **delta** emitted by the assistant」），完整聚合不在流内、在流末 `response.choice`；`Reasoning` / `ToolCall` 则有「增量 + 完整块」双变体
- `crates/infra/agent/src/sdk/normalize.rs` 把每个 `Text` 增量当完整块直接出 `Message` 事件（`ReasoningDelta` / `ToolCallDelta` 被正确跳过等完整块，唯独 text 等不到——流内根本没有完整 text 块）
- 传播链：SSE chunk（1-2 字）→ 每 delta 一个 `Message{blocks:[Text]}` 事件 → loop 盖 seq → store N 条碎事件入库 → adapter 1 事件 = 1 条 `evt-<seq>` UIMessage → timeline 1 UIMessage = 1 行 → 一轮回复碎成几十条碎行
- 追加毒化：碎事件让 resume 重建把每个 Message 事件折成一条独立 rig Message（`sdk/resume.rs` rebuild 无折叠），续会话时 provider 收到几十条微消息
- CLI 租户无此问题：claude stream-json 的 assistant 事件本就是完整消息——core 协议 `Message` 的语义事实上是「完整消息」，SDK 引擎把 provider 传输粒度（SSE delta）泄漏进了协议

### bug 2: SDK 链式续会话逐轮丢上下文（探索中发现，潜伏）

- `src/commands/exec/agent.rs` 的 `find_events_by_session`：`list_agent_runs()`（按 started_at 降序）上 `.find()` 命中**最新** run，取其事件作 resume 转录
- SDK 每 run 只落**本轮**事件（resume 重建的旧史不重发），链上各 run 同 session_id ⇒ 第三轮续会话的重建史 = 第二轮事件，**第一轮静默丢失**；链越长丢越多（只保最后一轮）
- CLI 免疫：`--resume` 由 claude 自持全量会话状态，转录重建是 SDK 独有路径

### 同根诊断

| bug | 根因 |
|---|---|
| 碎事件 | 协议无增量语义，且**传输粒度 = 落库粒度绑死**（tee 同一流既流出又落库） |
| 链式丢上下文 | **session 非一等公民**：转录 = 单 run 事件的拼凑，非会话全史 |

两者都是 core 形状的欠账——现状 core = 纯类型库（信封 + `AgentRunner` trait + 状态机，零 IO），编排（tee/落库/收敛）住在 `commands/exec`，session 只是 run 记录上的衍生字段（engine 铸的 session_id）——不是实现失误。重设计把两笔一起还。

---

## 提案

core/agent 从纯类型库升格为**会话域内核**（有状态，IO 一律经 port；「MUST NOT 认识 claude / 引擎」「MUST NOT 依赖 Tauri」禁令全部保留）：

- **运行面**：P1 引擎协议（参考 Actor，不照搬）——会话建立（injections：prompt + tools；ctx：workspace root、部分 config、可扩展；session 引用）、轮驱动（question）、终止（stop）、观察回流（Delta / Sealed / Notice / Raw / TurnDone）
- **治理面**：盖戳（seq/时间戳）、收敛状态机、停止注册（内存态）——现 `commands/exec` 编排（tee/落库/收敛）下沉进内核
- **持久面**：write-through——收到密封事件/返回值即经 store 原子操作落盘；查询 api 对账重导（从转录重建统计/索引）保留为纠偏机制，非主路径
- **查询面**：core 定契约，infra 实现（会话清单/聚合统计/转录重放）

**核心协议决策——传输粒度与落库粒度解耦**：

- `MessageDelta`（增量，ephemeral）：token 级，思考/回复可辨，只上传输面，**store 永不见**
- `Message`（密封，durable）：唯一落库与重放单元，密封粒度 = 引擎一次 assistant 回应（全部块收进恰一条密封 Message）；碎事件 bug 自然消解
- TurnDone（现 `RunResult` 演进）字段面（num_turns / duration_ms / usage / cost）= **统计唯一口径**，core 累计计算，引擎不得另立

**session 一等公民**：`SessionRecord`（core 铸 id、engine_session_id、配置快照、created_at/updated_at）+ 密封事件**直挂 session**（转录单表，run 退化为统计行）+ 转录 = 会话全史 = resume 重建源（链式丢上下文正解）。跨库张力（实例记录在全局库、session 在 workspace 库）以**配置快照**消解（禁跨库引用，实例改名/删除不伤历史会话）；聚合统计（轮数/累计墙钟/累计 token）MVP 从 runs 现算，不维护累计列。

**双引擎适配**（恢复归 infra，全自治）：

- SDK 引擎：流内 `Text` / `ReasoningDelta` → `MessageDelta`；轮末 `choice` → 密封 `Message`（碎事件消解）；resume 自读会话全史转录重建（丢上下文消解）
- CLI 引擎：行为零变化（resume 走 engine_handle 原生 `--resume`）；`--include-partial-messages` 增量翻译为可选后续，不进本期

**前端适配**：adapter 增 delta 路——delta → provisional 消息（correlator 键），密封到达以密封为准替换；`text-delta` 是 ai-sdk 原生词汇，前端增量路已就绪。调试页一轮回复呈现为单条连续增长的消息。

**切片**（已裁定）：②（session 一等公民 + 协议 delta/sealed 双层 + 查询 API + 编排下沉）**吸收**①（独立修碎事件），③（instance 装配升维 + 平台工具注入）仅预留接口位延后；链式 bug 直接进本 change 不做当前架构热修；「方案 A 轮末收口」热修作废。装配走最小组合根（resolve 下沉，不做 instance 驱动的完整装配 UX）。

---

## 能力

### 新增能力

- 无独立新能力。新关注点（会话域内核公共 API、P1 协议不变量与缺省降级、`SessionRecord` 模型与会话转录单表、会话查询与对账 API）以既有能力的 **ADDED requirement** 承载（见各 spec delta），不新开能力 spec。

### 修改的能力

- **desktop-agent-execution** — core/agent 升格会话域内核（纯度原则修订为「无直接 IO、经 port」）；事件词汇增 `MessageDelta` 与密封纪律、统计口径唯一化；`AgentRunner` trait 演进为 P1 引擎协议（四不变量 + 双 id 映射）；SDK 引擎 delta/密封交付与会话全史 resume（修两 bug）；落库演进为 write-through + 只落密封 + 转录挂 session；命令面/引擎门面解析/停止语义/调试页随内核重塑
- **desktop-workspace-store** — `SessionRecord` 模型与会话转录单表（session 键、run 统计行、配置快照）；会话查询与对账 API；双库布局 workspace 模型组增 `SessionRecord`；事件记录键位、run 来源归属与 explore 级联圈定随会话化平移
- **desktop-agent-chat-infra** — 适配层增 delta 路（provisional 消息 + 密封替换）；transport/hook 会话域重放装载与发送参数；ai-sdk 收口条款随信封演进更新
- **desktop-agent-management** — 运行发起解析单点自命令层**下沉至内核组合根**（默认 agent/显式 agent/sdk provider 组装/high 档/无默认 Err 语义不变；`runner_for` 与 `EngineConfig` 消费面零改动承诺保留）

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/agent/src/`（`event.rs` / `runner.rs` / `state.rs` / `lib.rs` + 新增 port、session、内核编排模块）—— 会话域内核化：协议双层词汇、P1 协议、治理面、持久面编排、查询契约
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` — 流项归一化双层化（Text/ReasoningDelta → delta；轮末密封）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` — 密封收口与流面交付改造
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` — 会话全史转录重建
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` / `cli/runner.rs` — 引擎协议实现适配（CLI 行为零回归）
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` / `store.rs` / `lib.rs` — `SessionRecord` 模型、转录单表、会话查询与对账 API
- `packages/desktop/src-tauri/src/commands/exec/agent.rs` — 编排下沉后重塑为内核薄包装（三件事纪律保留）
- `packages/desktop/src/lib/agent-adapter.ts` — delta 路（provisional + 密封替换）
- `packages/desktop/src/lib/agent-transport.ts` / `src/hooks/use-agent-chat.ts` — 会话域 invoke/流与重放装载
- `packages/desktop/src/views/agent/`（`agent-debug-view.tsx` / `components/agent-run-history.tsx` / `hooks/use-agent-run-history.ts`）— 调试页流式呈现与会话历史
- `packages/desktop/src/views/explores/hooks/use-explore-session.ts` — 探索链消费面语义等价切换
- `packages/desktop/src-tauri/src/bindings/` — 经 export-bindings 再生成（信封与命令面演进），非手改
- `packages/desktop/package.json` — 版本 bump（用户可见变更：碎行修复 + 续会话修复）

### 测试文件

- `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs` / `runner_test.rs` / `state_test.rs` + 新增内核/port 测试模块
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs` / `resume_test.rs` / `runner_test.rs` / `loop` 测试
- `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner_test.rs` / `jsonl_test.rs`（零回归护栏）
- `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` / `store_test.rs`
- `packages/desktop/src-tauri/src/commands/exec/agent_test.rs`
- `packages/desktop/src/lib/agent-adapter.test.ts` / `agent-transport.test.ts` / `src/hooks/use-agent-chat.test.ts`
- `packages/desktop/src/views/agent/` 与 `src/components/agent/` 既有 `*.test.tsx` 同步演进

### 删除文件

- 无整文件删除。函数级退役位：`find_events_by_session`（单 run 拼凑转录，由会话全史查询取代）；`commands/exec` 内 tee/落库/收敛编排体（下沉内核，`drive_agent_run` 编排形态随之退役）。

### 不要修改

- `openspec/explores/`（探索稿只读输入）
- `crates/infra/agent/src/sdk/policy.rs` / `sandbox.rs` / `tools.rs` 的权限三档、路径沙箱与工具面语义（本 change 不动权限面）
- `crates/infra/store` 的双库布局骨架、注入式打开、单进程约束、零迁移纪律（两库分组与注册表治理不变，仅模型组与会话域操作面演进）
- CLI 租户行为面（flag 组装 / jsonl 解析）：协议适配不重构，既有测试为回归护栏
- `tests/golden` 固定线面（golden wire contract，与本 change 无涉不触碰）
- `plugins/**`（桌面端变更不涉插件产物；`packages/desktop` 不受插件 bump 规则约束）
- `desktop-crate-layout` 能力 spec：本 change 不携带其 delta；port 落点若 design 裁定「core 直依 store crate」需追加该能力修订（见待决问题 1）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | SDK normalize 双层化 | 一轮含文本+思考的 SDK 回复：实时流呈 token 级增量（text/thinking 可辨），轮末恰一条密封 Message；调试页一轮回复为单条连续增长的消息，无碎行 |
| AC-2 | 转录挂 session 单表 | 密封事件记录以 session 为键直挂会话；每轮 assistant 产出恰一条密封 Message；store 中不存在任何 delta 记录 |
| AC-3 | resume 会话全史 | 第 N 轮续会话的重建史含全部前 N-1 轮 user/assistant 往返（第一轮不再丢失）；会话不存在或非 SDK 产出时显式启动失败 |
| AC-4 | 内核纯度 | core/agent 无对外部世界的直接 IO（文件/进程/网络），IO 全经 port 由 infra 实现；源码无 claude/引擎字样、无 Tauri 依赖；（port 落点裁定前）无 workspace 内 crate 依赖 |
| AC-5 | P1 四不变量 | 观察词汇含 delta 且通道有界背压；TurnDone 字段面为唯一统计口径且由 core 累计（引擎无第二口径）；engine_handle 经 spawn 响应或事件上报且 core 记双 id 映射；injections（会话级）与 question/ctx（轮级）分离 |
| AC-6 | write-through 持久化 | 密封事件/返回值到达即 store 原子操作落盘；查询对账重导可校正统计/索引；统计与查询附带字段缺席不报错（降级不违约） |
| AC-7 | 停止语义保留 | 会话活动轮可终止并收敛 `stopped`；对非运行中目标幂等忽略；停止后重放与实时一致；`RunHandle` 已验证竞态语义无回退 |
| AC-8 | 前端两路同构 | delta → provisional 消息、密封到达替换；重放（会话转录折叠）与实时收口归一后的状态同形状；未知事件仍 `data-raw` 透传不丢 |
| AC-9 | 命令面重塑 | 编排（tee/落库/收敛/盖戳）自 commands 下沉内核；命令体保持参数转换 → 调用 → 错误映射三件事；无默认 agent 时显式 `Err` 引导管理页；root 寻址与 `Result<T, String>` 模板保留 |
| AC-10 | CLI 租户零回归 | CLI 引擎 flag 组装 / jsonl 解析 / `--resume` 行为不变，既有 CLI 测试全绿 |
| AC-11 | 探索链语义等价 | explore 会话链续话、历史还原、记录级联删除经会话域语义等价工作（行为不回退） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 线格式演进（信封增 `MessageDelta`、TurnDone 口径）破坏前端与存量库读取 | 重放/呈现异常 | 中 | serde additive 演进 + bindings 再生成 + 适配层先收；存量数据处置见待决问题 2 |
| port 落点（core 定接口 vs core 直依 store）触碰 desktop-crate-layout「core MUST NOT 依赖 store」规则 | 布局 spec 需追加修订 | 中 | design 裁定落点并核对 crate 图；倾向 core 定 port、infra 实现（不新增 core→store 边） |
| SDK loop 改造波及工具执行/权限/沙箱 | 工具面回归 | 中 | 改造限于流面（normalize/密封收口），工具面与沙箱代码不动；normalize/resume fixture 密集回归 |
| 会话模型引入使存量 run/事件历史不可读 | 历史会话丢失 | 待裁定 | 零迁移冷启动有先例（旧单库惰性废弃）；design 提案、用户裁定后落 tasks |
| CLI/SDK 流面不对称持续（本期 CLI 无增量） | CLI 页面无 token 级流 | 低 | 协议已留位；将来 `--include-partial-messages` 翻译即可，无返工 |
| 前端 provisional 状态机（useChat reducer 上的替换语义）复杂化 | 呈现异常 | 中 | 适配层纯函数锁定 + 两路同构测试 + 密封收口时 setMessages 归一兜底 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 内核纯度 | 接受「无直接 IO、经 port」修订；claude/引擎/Tauri 禁令保留 | bug 消解需编排入内核；纯类型库形状下编排只能留 commands，两 bug 无协议层解 | 维持纯类型库（作废） |
| 密封消息归属 | 事件直挂 session（转录单表），run 退化为统计行 | 查询/重建简单优先于迁移省事 | 事件挂 run（session 查询 fold 链，迁移小但每查询重折） |
| 切片 | 单 change：②吸收①，③仅预留延后 | 落库半边结构性必然（密封完整消息是转录全史唯一可行 durable 单元）+ 传输半边本就是 run_session 核心交付 | ①独立成 change（作废）；三刀依次（过碎） |
| 链式丢上下文 bug | 直接进本 change，不做当前架构热修 | 两 bug 同根，热修属重复偿债 | 当前架构 fold 全链热修（作废） |
| 持久化纪律 | write-through + 查询对账纠偏 | 收到即原子落盘；重导为误差校正非主路径 | 批量 flush（丢窗口）/ 仅查询现算（写路径无锚） |
| 聚合统计 | MVP 从 runs 现算，不维护累计列 | 避免写放大；查询 API 是天然聚合点 | 累计列（写放大，将来不够再加） |

### 待决问题

- core ↔ store crate 落点：core 定 port 由 infra 实现，或 core 直依 store crate——与 desktop-crate-layout 依赖方向的关系由 design 裁定（倾向前者）
- 存量 run/事件数据处置：零迁移冷启动（援引旧单库惰性废弃先例）vs native_model 版本演进挂接——design 提案、用户裁定
- delta 配对键：`parent_tool_use_id` vs 显式 correlator
- seq 语义定稿：delta 是否占 seq（库内重放空洞容忍）或独立编号空间
- Stop 消息 vs 保留 `RunHandle` 薄包装（竞态语义已验证）；Spawn / 会话 actor 语义统一（CLI 每轮进程、SDK 常驻，actor 均为逻辑层）
- ctx 首版字段面；未知字段忽略的演进策略
- 命令面命名：`agent_start` 等保留语义升级 vs 更名会话域命令；旧 run 域查询命令的退役/收窄路径
- TurnDone 与 `RunResult` 线格式关系（更名 vs 别名）及 `AgentStartError` / `AgentRunParams` 的演进面

---
