# 任务: agent-component-ai-sdk

> 依赖顺序：Rust 终止与提前 resolve → 前端基建（适配层 / transport / hook）→ 展示组件族 → 页面接线 → 删除清理 → 守线。
> 测试编写与执行不在本清单（归 test-design / test-gen / test-execution 阶段承接）；被删组件的既有用例随迁由测试阶段处理。
>
> **回溯修订注记**：阶段一至六为首轮实现已完成项（保留存档）；阶段七为 implement 通过后回溯追加的修订任务，仅覆盖两条新增需求（对话透镜过滤 system 消息 + explore/debug 区域滚动）及其守线，起始均为未完成。阶段一至六任务不因本回溯重开。

## 阶段一：Rust 终止与提前 resolve（契约 → 租户 → 编排 → 命令面）

- [x] `crates/core/agent/src/state.rs`：`AgentRunState` 增 `Stopped` 变体；`RunStateMachine::stop(&mut self) -> AgentRunState` 显式收敛分支（Running → Stopped，已收敛幂等原样返回）；模块文档更新为 running → completed | failed | stopped
- [x] `crates/core/agent/src/runner.rs`：`RunHandle` 挂点填实——终止信号句柄（`request_stop` / `stop_requested` / `wait_requested`，`Notify` + `AtomicBool` 实现，Clone + Default）；保持逻辑面零进程类型（kill 机制归租户）
- [x] `crates/infra/agent/src/runner.rs`：泵任务改为 `select` 行流 vs `RunHandle` 停止信号——信号到达即进程树击杀（Windows `taskkill /PID <child_pid> /T /F` 覆盖 `cmd /C` 整树，非 Windows `child.start_kill()`；尽力语义、静默不阻塞）；停止路径跳过 `error_process_exit` result 合成；`pump_lines` 测试缝签名随迁（`discover` / `flags` / `jsonl` 零改动）
- [x] `src-tauri/src/commands/exec/agent.rs`：新增 `AgentRunMessage` IPC 信封（tag `ipc`，Event / Record 双变体）与 `STATUS_STOPPED` 受控常量；`RunStopRegistry`（`Mutex<HashMap<i64, RunHandle>>`，登记 / 查询 / 终态除名）
- [x] `src-tauri/src/commands/exec/agent.rs`：编排拆分——`start_agent_run`（组装 runner → `runner.start` → `begin_agent_run` 落 running 行 → 注册停止句柄 → spawn 后台任务 → 返回 running 记录）与 `drive_agent_run`（tee 双 sink 循环保持 store 逐事件单事务 + Channel 发送；EOF 收敛：状态机已收敛以状态机为准，否则 stop 请求 → `machine.stop()` 收敛 `stopped`，兜底 failed 记因；`finish_agent_run` → 注册表除名 → Channel 流出 `Record`）；store 写失败路径（`abort_with_store_failure`）同样流 Record 后再除名；移除旧 `run_agent` / `run_agent_with` 阻塞形态
- [x] `src-tauri/src/commands/exec/mod.rs`：`agent_start` 改经 `start_agent_run`（`on_event` 参数类型升为 `Channel<AgentRunMessage>`，IPC 参数面不变，语义演进为提前 resolve）；新增 `agent_stop` 命令（`State<'_, RunStopRegistry>` + `run_id: i64`，miss / 已终态幂等 `Ok`）；模块文档同步契约演进
- [x] `src-tauri/src/main.rs`：`invoke_handler` 注册 `commands::exec::agent_stop`；`app.manage(RunStopRegistry::default())`（与 `WatchRegistry` 同型）
- [x] `crates/infra/store/src/model.rs`：`AgentRunRecord.status` 字段注释受控清单扩展 `stopped`（仅 doc 注释，v1 历史结构与 schema 零变更）

## 阶段二：前端基建收口（依赖 → 适配层 → transport → hook）

- [x] `packages/desktop/package.json`：`dependencies` 新增 `ai`（v7 主版本锚定，精确版本以接入时 stable v7 为准）
- [x] `src/types/dto.ts`：`AgentRunStatus` union 增 `'stopped'`
- [x] `src/lib/agent-adapter.ts`（新）：`AgentUIMessage` / `AgentMessageMetadata`（seq、`parentToolUseId`）/ `AgentDataParts`（五 `data-*` 部件）类型；`eventsToUIMessages`（重放折叠：message 事件 → 气泡消息、非 message → system 角色 data 部件消息、工具同 id 跨消息配对、无主 result 合成占位、未知事件 `data-raw` 透传）；`eventToChunk`（单事件 chunk 组：text / reasoning / tool-input-available / tool-output-available / data 部件）；`runRecordToUIMessage`（终态同构部件消息）；与 `eventToChunk` 语义等价的不变量由实现自检（两路同一状态形状）
- [x] `src/lib/agent-transport.ts`（新）：`AgentRunMessage` TS 镜像；`TauriAgentTransport implements ChatTransport<AgentUIMessage>`——`sendMessages` 经 `options.body` 取链参数（root / permissionMode / resumeSessionId / parentRunId / source / sourceRef）+ 末条用户文本组装 `invoke("agent_start")`；`Channel<AgentRunMessage>` 消息逐条转 chunk，Record → `data-run-record` chunk + finish + 关流；early-resolve 返回的 running 记录经 `onRecord` 透传；`reconnectToStream` 恒 `null`；`trigger !== 'submit-message'` 显式不支持
- [x] `src/hooks/use-agent-chat.ts`（新）：`useChat`（ai v7）+ transport（tee 回调经 ref 稳定注入）；重放装载（`agent_run_chain` + `agent_run_events` → `eventsToUIMessages` + `runRecordToUIMessage` 逐 run 交错 → `setMessages`，sourceRef 为 null 跳过）；发送组装（链尾取最后一条非 running 记录的 `sessionId` / `id`，body 穿透；运行中重复发送忽略）；`stop` → `invoke("agent_stop", { runId: currentRunId })`，不调 `chat.stop()` 截断（终态部件与 finish 由后端闭流推入）；`events` / `chain`（running 追加、终态按 id 替换）/ `currentRunId` / `error` / `loading` / `reset` 面

## 阶段三：展示组件族（共享块 → 注册表 → 双透镜 → composer → 导出面）

- [x] `src/components/agent/agent-blocks.tsx`（新）：双透镜共享块渲染件自 `explore-conversation.tsx` 收编——TextBlock（markdown GFM）/ ThinkingBlock（折叠）/ ToolPairCard（input + output 成对，isError 高亮）/ ResultCard（轮数 / 成本 / 时长 / session）/ CopyValue / `data-run-record` 静默状态行；沿用既有 `block-*` / `event-*` data-testid
- [x] `src/components/agent/tool-cards.tsx`（新）：`AgentToolCard` 类型 + `AskUserQuestionCard` 收编（问题/选项静态可读，宽松解析回退 JSON）+ `DEFAULT_TOOL_CARDS` 注册表（name 命中特化卡、miss 回退 ToolPairCard）
- [x] `src/components/agent/agent-messages.tsx`：骨架填实为对话透镜——`AgentUIMessage[]` 渲染（气泡 user 靠右 / assistant、块经共享渲染件、data 部件 → 辅助行 / 汇总卡 / raw / record 行）、message-scroller 组合保持、loading / running / empty 态与 testid 保留
- [x] `src/components/agent/agent-timeline.tsx`（新）：保真透镜——按数组序（seq 保序）呈现、message 部件展开式渲染（含 tool input/output 对读）、子代理消息按 `metadata.parentToolUseId` 归因到父工具卡下（`subagent-group`）、raw 原文透传、result 汇总卡可复制（`result-*` / `copy-value` testid 沿用）
- [x] `src/components/agent/agent-input.tsx`：空文件填实 composer——prompt 输入 / permission-mode 三档下拉（默认 bypassPermissions，沿 ModeSelect 语义）/ 发送（运行中禁发）/ 停止入口（运行中呈现，触发 `onStop`）；`idPrefix` 驱动 id 与 testid
- [x] `src/components/agent/index.tsx`：导出面扩充（`AgentMessages` / `AgentTimeline` / `AgentInput` / `DEFAULT_TOOL_CARDS`），只导出被消费符号（knip 纪律）

## 阶段四：页面接线

- [x] `src/views/explores/hooks/use-explore-session.ts`：内部改走 `use-agent-chat`（`source="explore"`、`sourceRef=String(record.id)`、`root`）；`send` 只做 `buildExplorePrompt` stance 拼接后委托 `sendMessage`；对外暴露 `messages` / `loading` / `running` / `error` / `send` / `stop`（类型经 `ReturnType` 推导，不 import `ai`）；删除本地 Channel / invoke / 链还原实现（`loadChain` / `useChainReplay`）
- [x] `src/views/explores/components/explore-composer.tsx`：改为 `AgentInput` 薄适配——保留探索页 placeholder / label / `explore-*` testid 前缀，接出 `onStop`
- [x] `src/views/explores/explore-detail-view.tsx`：`ExploreConversation` → `AgentMessages`；composer 停止入口接线（`session.stop`）；双栏布局、预览、watch 生命周期、run 终态（running true→false）定点重读文档不变
- [x] `src/views/agent/agent-debug-view.tsx`：改走 `use-agent-chat`（`source="debug"`、`sourceRef=null`，每次发起先 `reset()`）；`AgentEventTimeline` → `AgentTimeline`（消费 chat messages）；`AgentRawStream` 改消费 chat 的 `events` 镜像；运行中呈现停止入口（页面级 chrome 按钮，调 `stop`）；run 表单 / 历史列表 / JSONL 开关外圈保留
- [x] `src/views/agent/components/agent-run-history.tsx`：重放区 `AgentEventTimeline` → `AgentTimeline`（`eventsToUIMessages(state.events)` 转换）；`statusLabel` 增 `stopped`（已停止）

## 阶段五：删除与清理

- [x] 删除 `src/views/explores/components/explore-conversation.tsx` 与 `explore-conversation.test.tsx`（渲染已收编组件族，用例随迁）
- [x] 删除 `src/views/agent/components/agent-event-timeline.tsx` 与 `agent-event-timeline.test.tsx`（由 `agent-timeline.tsx` 取代，用例随迁）
- [x] 删除 `src/views/agent/hooks/use-agent-run.ts` 与 `use-agent-run.test.ts`（由 `use-agent-chat` 取代）
- [x] 全局检索确认被删符号（`ExploreConversation` / `AgentEventTimeline` / `useAgentRun` / `collectResults`）在 `src/` 内零引用残留

## 阶段六：守线（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零告警——含删除目标零残留、新导出均有消费方（无 test-only 导出）
- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零告警
- [x] AC-8 静态核对：`from 'ai'` import 仅现于 `src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/use-agent-chat.ts`、`src/components/agent/` 四处；`views/` 与 `src-tauri/` 零命中
- [x] 边界静态核对（零 diff 确认）：`AgentEvent` 信封五变体与序列化未动；`agent-cli` 的 `flags.rs` / `jsonl.rs` 未动；store schema（`user_agent_runs` / `user_agent_run_events` 模型定义）未动；Rust 侧新增注释中的能力 spec 指针按「能力名 + `specs/<capability>/spec.md` + 路径相对域根」定式书写（命名隔离字面量约束）
- [x] 验收边界声明：测试执行归 test-execution 阶段承接，本阶段以静态检查守线

## 阶段七：回溯修订——对话透镜 system 过滤（仅覆盖新增/修订范围）

- [x] `src/components/agent/agent-messages.tsx`：渲染入口按 `role !== 'system'` 过滤消息（run 启动 / system 通知 / runResult / raw / record 的 system 载体消息整条不渲染；`props.messages` 入参仍接受全量）；删除 `MessageView` 的 system / onlyData 直出分支与 `InfoRow` / `RawView` 局部渲染件；`PartView` 收窄为 text / reasoning / tool（气泡内呈现）；empty / loading 判定改基于过滤后列表；模块文档同步呈现口径
- [x] `src/components/agent/agent-timeline.tsx`：section 增 `flex min-h-0 flex-1 flex-col`；消息体由裸堆叠改为 message-scroller 组合（`MessageScrollerProvider autoScroll defaultScrollPosition="end"` → `MessageScroller min-h-0 flex-1` → Viewport / Content，`topLevel` 消息逐条 `MessageScrollerItem` 包裹，`MessageScrollerButton direction="end"`）——流式跟随 + 内部滚动；子代理归因嵌套（`subagent-group`）随父项滚动；data 部件全量呈现与既有 testid 不变
- [x] `src/views/agent/components/agent-run-history.tsx`：`replay-area` 容器限高内部滚动（`max-h-96 overflow-y-auto`），重放 `AgentTimeline`（此时处普通块容器、自然高）在限高容器内滚动；run 列表区不动

## 阶段八：回溯修订——区域滚动接线（仅覆盖新增/修订范围）

- [x] `src/app.tsx`：路由内容包裹层 `mx-auto w-full flex-1 px-4 py-4` 增 `flex min-h-0 flex-col`——壳层滚动框架；其余路由页（change / db / explore 清单等）根节点为 flex-grow 0 的内容自适应 item，呈现不变（静态核对确认，不逐页改样式）
- [x] `src/views/explores/explore-detail-view.tsx`：根节点 `flex min-h-0 flex-col` 增 `flex-1`，接入壳层 flex 链（左栏 `AgentMessages` 已 flex-1 + message-scroller，链条补全即得「撑满剩余区域 + 超出内部滚动」）；双栏 / 预览 / watch 生命周期不动
- [x] `src/views/agent/agent-debug-view.tsx`：根节点 `<div>` 改 `flex min-h-0 flex-1 flex-col`；`AgentRunForm` / `RunErrorBanner` / `StreamToggle` / `AgentRunHistory` 保持自然高；流视图区（`AgentTimeline` / `AgentRawStream` 条件二选一）作为 `flex-1` 子项填充剩余区域
- [x] `src/views/agent/components/agent-raw-stream.tsx`：section 改 `flex min-h-0 flex-1 flex-col`；事件 `<pre>` 列表包 `min-h-0 flex-1 overflow-y-auto`（调试 dump 不做跟随）；empty 态与 testid 不变

## 阶段九：回溯修订守线（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零告警——含 `agent-messages.tsx` 删除分支与局部渲染件零残留、`agent-blocks.tsx` 导出面无孤儿（`ResultCard` / `RunRecordRow` 仍由 `agent-timeline.tsx` 消费）、无 test-only 导出（随迁微差：`RawView` 自 agent-messages 删除后 `AgentRawData` 导出转由 `agent-timeline.tsx` 的局部 `RawView` 消费，适配层零触碰）
- [x] 边界静态核对（零 diff 确认）：本回溯不触碰 `src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/use-agent-chat.ts`、`src/types/dto.ts` 与 `src-tauri/`（两条需求均为呈现层 / 布局层）；`from 'ai'` import 面不扩大
- [x] 验收边界声明：测试执行归 test-execution 阶段承接，本阶段以静态检查守线；对话区辅助行断言的用例基线收窄（如涉及）归测试阶段处理
