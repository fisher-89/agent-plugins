# 任务: agent-core-session-kernel

> **变更**: agent-core-session-kernel
> **日期**: 2026-10-01
> **依据**: proposal.md（AC-1~11）+ design.md（变更清单与设计裁定）

---

任务按依赖排序：阶段一（core 协议与内核）→ 阶段二（store 模型与操作面）→ 阶段三（infra 组合根与双引擎）→ 阶段四（壳命令面与 bindings）→ 阶段五（前端适配）→ 阶段六（版本 bump 与静态守线）。测试编写 / 测试执行不在本列表（由 test-design / test-gen / test-execution 阶段承接）；所有文件路径见 design.md 变更清单。

## 阶段一：core/agent 会话域内核（协议、词汇、port、编排）

- [x] `event.rs`：`AgentEventKind` 新增 `MessageDelta { parent_tool_use_id: Option<String>, delta: AgentDelta }` 变体与 `AgentDelta`（Text/Thinking，serde 内部 tag camelCase）枚举；`RunResult` 更名 `TurnDone`（serde 线值 `runResult` → `turnDone`，字段面不变）；新增 `is_delta()` / `is_sealed()` 判别方法；`AgentEvent::stamp` 盖戳原语保留
- [x] `state.rs`：`RunStateMachine::apply` 匹配变体随 `TurnDone` 更名平移（收敛语义零变化：首收敛生效、stopped 独立分支、终态幂等）
- [x] 新增 `session.rs`：`SessionProvenance` / `SessionRow` / `NewSessionRow` / `SessionStats` / `SessionSummary`（`turns: Vec<TurnSummary>`，core 契约不引用 store 模型）/ `TurnSummary`（IPC 轮行 DTO：turn_id / session_id / status / 起止时间戳 / TurnDone 统计字段 / error，实时与重放两路同构）与 `ses-` 前缀会话 id 铸造；serde camelCase + specta derive，纯类型零 IO
- [x] `runner.rs`：P1 协议改造——新增 `SessionOpen` / `SessionInjections` / `SessionCtx` / `SessionRef` / `TurnQuestion` / `AgentSession`（观察通道载未盖戳 `AgentEventKind`）；`AgentRunner` trait `start` → `open_session`；`RunHandle` 与 `AgentStartError`（CliMissing / SpawnFailed / ConfigMissing）零改动；退役 `AgentRunParams` / `AgentRun` / `AgentEnvMode`
- [x] 新增 `port.rs`：`SessionSink`（create_session / begin_turn / append_sealed / finish_turn / bind_remote_session）与 `SessionQuery`（list_sessions / transcript / reconcile_stats）trait 契约 + `TurnOutcome` / `KernelOutput` 收口类型
- [x] 新增 `kernel.rs`：`StopRegistry`（session_id → RunHandle 内存表，register/request_stop/remove）与 `SessionKernel`——`begin_turn` 同步段（open 失败不落库 → New 建会话行 → 开轮行 → 停止登记 → 返回 `RunningTurn`）与 `drive` 泵（盖戳 seq/时间戳 → delta 只上输出回调、密封 write-through 失败 failed 收敛记因 → `bind_remote_session` 双 id 落库 → TurnDone 统计收口 → 除名 → `TurnFinished` 流出）
- [x] `lib.rs`：模块声明与 pub use 扩展（session / port / kernel）、退役类型出导出面、模块文档措辞同步；自查 `crates/core/agent/Cargo.toml` 零 workspace 内依赖、源码无 claude/引擎/Tauri 字样（layout 命名隔离扫描双禁令字面量同查）

## 阶段二：store 会话模型与操作面

- [x] `model.rs`：新增 `SessionRecord`（native_model id=7：core 铸 id 字符串主键、`engine_session_id`、`config_snapshot`、`source` / `source_ref`、`created_at` / `updated_at`）与 `SessionConfigSnapshot`（engine/model/permission 嵌套 struct，store 本地定型）；新增 `SessionEventRecord`（native_model id=8、serde_json 编解码：`hash64(session_id) << 64 | seq` 打包 u128 主键 + `session_id` 二级索引 + 密封 `AgentEvent` 嵌装，`event_key_serde` 定式复用，hash64 由既有 sha2 前 8 字节派生）
- [x] `model.rs`：`AgentRunRecord` native_model 版本 3→4 轮统计行化（字段面收敛为 id / `session_id: Option<String>` / status / 起止时间戳 / TurnDone 统计三字段 / error；`from_previous` 存量行 `session_id = None`、统计与时间戳保留；prompt/cwd/env/permission_mode/source/source_ref/parent_run_id 随版本退役）
- [x] `store.rs`：workspace 模型组注册 `SessionRecord` + `SessionEventRecord`、移除 `AgentEventRecord` 注册（全局组不动）；新增 `create_session` / `find_session` / `begin_agent_turn` / `append_session_events`（delta 防御性忽略）/ `finish_agent_turn` / `bind_session_remote` / `list_sessions`（来源过滤 + updated_at 降序 + 聚合现算：轮数/累计墙钟/累计 token 鸭子类型求和、缺席合法缺省、轮统计行随行返回）/ `list_session_events`（二级索引扫描 + seq 升序）/ `reconcile_session_stats`（显式对账重导，不改转录）
- [x] `store.rs`：`delete_explore_record` 级联圈定自 runs 平移至会话——按 `(source, source_ref)` 圈定归属会话，转录（二级索引扫描）+ 轮统计行 + 会话行同事务删除；`EXPLORE_RUN_SOURCE` 常量与圈定逻辑平移；退役 `begin_agent_run` / `finish_agent_run` / `list_agent_runs` / `list_agent_run_events` / `append_agent_run_events` / `restore_run_chain`
- [x] `lib.rs`：re-export 同步（`SessionRecord` / `SessionConfigSnapshot` / `SessionEventRecord` 出，退役 API 收）；双库布局骨架、注入式打开、零迁移纪律不动

## 阶段三：infra/agent 组合根与双引擎协议适配

- [x] `Cargo.toml`：新增 `store` workspace 内依赖
- [x] 新增 `store_port.rs`：`SessionSink` / `SessionQuery` 的 store 适配实现（`Arc<Store>` 持有；聚合现算与对账重导委托 store 操作面；core 契约类型 ↔ store 记录类型映射）
- [x] 新增 `compose.rs`：`compose_turn(stores, root, agent)` 组合根——解析语义自命令层原样平移（缺省解析默认 agent / 显式 agent / sdk provider 组装取 high 档 / 无默认 `Err` 引导管理页）；Continue 读会话行并按快照 engine 比对 + 提取 prior_handle（不存在 / 非同引擎 / 句柄缺失 → `Err`）；resume 装载缝以 `list_session_events` 全史闭合；装配 sink/query/门面/内核
- [x] `sdk/normalize.rs`：双层化——`Text` / `ReasoningDelta` / 完整 `Reasoning` → `MessageDelta`（text/thinking 可辨）；完整 `ToolCall` 不再立即出密封事件（返回 None，轮末收口）；`Unknown` → `Raw`；`Final` / `ToolCallDelta` 簿记项照旧 None
- [x] `sdk/loop.rs`：轮末 `choice` 收口恰一条密封 `Message`（Text/Thinking/ToolUse 全部块收进，role=assistant）；`RunResult` → `TurnDone` 组装（统计字段面唯一出口、cost_usd 恒 None）；ToolResult 密封 user 消息、空轮/轮数熔断/API 失败收敛语义保留；观察发送改载未盖戳 `AgentEventKind`；工具执行 / policy / sandbox 代码不动
- [x] `sdk/resume.rs`：`rebuild` 重建源语义平移为会话全史转录（顶层非 Raw 密封事件、tool 成对回灌不变）；退役 `owns_session` / `SESSION_PREFIX` 前缀校验（`sdk-` 前缀仅保留为 remote id 铸造格式）
- [x] `sdk/runner.rs`：`start` → open/ask 两段——open 阶段配置三件套校验（`ConfigMissing`）+ Continue 经装载缝取全史转录重建；ask 阶段泵 select（停止 drop future，不合成收敛）；每轮 `sdk-` remote id 铸造经 `RunStarted` / `TurnDone` session_id 上报
- [x] `cli/flags.rs`：`build_args` 入参自 `AgentRunParams` 平移为协议轮参数形状；flag 输出序列零变化（含 `--resume <id>` 尾追加规则与 Windows shim 约定）
- [x] `cli/jsonl.rs`：归一化产出变体 `RunResult` → `TurnDone`（含文档措辞）；行解析语义零变化
- [x] `cli/runner.rs`：`start` → open/ask 两段——open 无 IO（injections/ctx/session 引用留存）；ask 按既有流程 spawn + 逐行泵 + EOF 合成收敛（`TurnDone`）+ 树杀缝；`--resume` 走 prior_handle；行为零回归
- [x] `sdk/mod.rs`：模块文档 `RunResult` 措辞同步 `TurnDone`（更名 grep 归零）
- [x] `lib.rs`：挂载 compose / store_port 模块；`ResumeTranscript` 装载缝语义平移为会话全史转录（类型形状不变）；门面 `runner_for` / `EngineConfig` 零 diff 复核（签名与三字段结构不变）

## 阶段四：壳命令面与 bindings 再生成

- [x] `commands/exec/agent.rs`：重塑为薄包装——保留 `AgentRunMessage` IPC 信封（`Record` 臂载 `TurnSummary`）与 running/终态 `TurnSummary` 装配 helper（自 `TurnRequest` / `RunningTurn` / `TurnOutcome` 中性数据装配）；退役 `find_events_by_session` / `RunProvenance` / `RunSummary` / `resolve_agent_engine` / `ResolvedEngine` / `RunStopRegistry` / `start_agent_run*` / `drive_agent_run` / `running_record` 全量编排体
- [x] `commands/exec/mod.rs`：`agent_start` 参数面升级（`resume_session_id` / `parent_run_id` → `session_id`，返回 `Result<TurnSummary, String>`，编排调用改组合根 + 内核，后台转发任务流出 KernelOutput → Channel）；`agent_stop` 寻址键 run_id → session_id（root 参数与幂等忽略保留，State 换 `StopRegistry`）；新增 `agent_sessions` / `agent_session_transcript` 查询薄包装；退役 `agent_runs` / `agent_run_events` / `agent_run_chain`；blank root 守卫与 `Result<T, String>` 模板保留；命令注册宏同步
- [x] `main.rs`：托管状态 `RunStopRegistry` → `agent::StopRegistry` 挂载替换
- [x] `src/types/generated/bindings.ts` 再生成：`pnpm -C packages/desktop run bindings:export`（生成物非手改）——核对 `AgentEventKind`（`messageDelta` / `turnDone`）、`SessionRow` / `SessionSummary` / `TurnSummary` 域类型与四命令签名镜像一致、`AgentRunRecord` 出 IPC 镜像；`bindings/mod.rs` 无需手改

## 阶段五：前端适配（delta 路 + 会话域）

- [x] `lib/agent-adapter.ts`：`eventsToUIMessages` 密封-only（`messageDelta` 防御跳过）、`turnDone` → `data-run-result`；`eventToChunk` 增 delta 路（`messageDelta` → `text-delta` / `reasoning-delta`，稳定 part id）；新增导出 `provisionalMessageId(parentToolUseId)` 与 `deltaPartId(key, part)`；`runRecordToUIMessage` 入参演进 `TurnSummary`；密封 Message chunk 组保持 start+reset+整块（同键让位替换）；`data-raw` 透传与 seq / `parentToolUseId` 保真锁定不变
- [x] `lib/agent-transport.ts`：body 参数会话域化（`sessionId: string | null` 取代 `resumeSessionId` / `parentRunId`，读取校验函数同步）；流闭包内首 delta 簿记（配对键 → 已开件标记，首 delta 补发 start(provisional 键)+reset-step+part-start，后续 delta 直发累积 chunk）；密封/aux chunk 组装、Record 收尾、`reconnectToStream` null 不变
- [x] `hooks/use-agent-chat.ts`：会话域状态——重放装载改 `agentSessions`（取 updated_at 最新会话）+ `agentSessionTranscript` 全史转录；镜像只收密封事件（delta 不入镜像）；`sendMessage` body 收口 `sessionId`（当前会话 Continue，无会话 New）；early-resolve 轮行（`TurnSummary`）`sessionId` 回写当前会话；`stop` → `agentStop(root, sessionId)`；收口归一重建含 `TurnSummary` 轮行 `data-run-record` 部件交错（TurnDone 事件位与轮序对齐）；对外 `UseAgentChatState` 增 `session` 镜像、`chain` 平移为 `TurnSummary` 轮行列表
- [x] `views/explores/hooks/use-explore-session.ts`：会话寻址语义等价切换（source/sourceRef 经基建消费、Continue 续话、stop 透传）；对外返回形状（messages/chain/events/loading/running/error/send/stop）与 stance 拼接层次不变，行为不回退
- [x] `views/agent/hooks/use-agent-run-history.ts`：取数轨道改 `agentSessions(root, "debug", null)` 清单 + `agentSessionTranscript(root, sessionId)` 重放；显式触发、无轮询纪律保留
- [x] `views/agent/components/agent-run-history.tsx`：会话行列表（updated_at / 轮数 / 终态）+ 点开会话转录重放（密封-only `eventsToUIMessages`）；重放区限高滚动与空/加载/错误态保持
- [x] `views/agent/agent-debug-view.tsx`：历史区改会话历史接线；每跑重置（每跑 New 会话）与流式呈现参数传递保持；时间线组件零改动复用

## 阶段六：版本 bump 与静态守线（静态，不含测试执行）

- [x] `packages/desktop/package.json`：`version` 0.3.13 → 0.3.14（用户可见变更：碎行修复 + 续会话修复）
- [x] 静态检查：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（vp check + knip）通过
  - 实现阶段状态：server:check 全绿（cargo fmt + clippy 零告警）、knip 零报告、vp check 147 文件零告警零类型错误；共置 `*.test.ts(x)` 已随协议/命令/状态面演进机械迁移（fixtures 改 TurnSummary / SessionSummary、mocks 改 agent_sessions / agent_session_transcript、body 改 sessionId），前端全套件 685 用例全绿（test-gen 阶段按 test-design.md 再生为行为面完整覆盖）
- [x] 更名归零自查：`src-tauri` 产品源码 grep `RunResult` / `resume_session_id` / `AgentRunParams` / `owns_session` / `restore_run_chain` / `agent_run_chain` 零残留（退役面不含测试文件）
- [x] 纯度与边界自查：`crates/core/agent` 源码无 claude/引擎概念字样、无 Tauri 依赖、Cargo.toml 零 workspace 内依赖；产品源码不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名）；进程 spawn 仅存于 `cli/runner.rs`
- [x] 边界确认：`不要修改` 清单（权限三档 / sandbox / 工具面语义、双库布局骨架、CLI flag/jsonl 行为面、golden 线面、插件产物、探索稿只读）零触碰；`desktop-crate-layout` 能力无本变更 delta（port 落点裁定为 core 定 port，core→store 边未产生）

> 测试编写与测试执行由 test-design / test-gen / test-execution 阶段承接，本列表不携带任何测试任务或测试执行命令。
