# 任务: agent-sdk-rig-tenant

> 依据 `design.md`（v2 回溯修订）变更清单展开；文件边界以清单为准。
> proposal.md 与 specs 增量已随提案阶段同步，不在本任务列表。
> 测试编写 / 测试执行分别由 test-design / test-gen / test-execution 阶段承接，本列表不包含
> （断言 v1「缺省 cli」语义的既有壳层 / 前端测试修订亦归 test 阶段）。
> 阶段一至七为第一轮 implement 已完成项（存档保留，不得回退）；阶段八至十为 v2 回溯修订
> （引擎选择语义：debug-only 选择、后端默认 agent 硬编码 rig、缺席透传）新增范围，
> 仅覆盖新增 / 修订部分，不重复第一轮任务。

---

## 阶段一：crate 改名与 cli/ 平移（AC-1）

- [x] `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`：包名 `agent-cli` → `agent-runtime`，注释改写为「agent 边界的引擎门面与双引擎实现」；新增 `rig-core = { workspace = true }` 与 `glob = { workspace = true }` 依赖行（rig-core 采 workspace 收敛的 `default-features = false` + `["reqwest", "native-tls"]`；glob 为 rig 传递依赖同款 0.3，直连零新增足迹）
- [x] `packages/desktop/src-tauri/Cargo.toml`：workspace.dependencies 键 `agent-cli` → `agent-runtime`（目录名与 members 行不变）；新增 `agent-runtime = { path = "crates/infra/agent" }` 收敛条目、`rig-core = { version = "=0.42.0", default-features = false, features = ["reqwest", "native-tls"] }` pin 条目与 `glob = "0.3"` 条目；根包 `[dependencies]` 行 `agent-cli` → `agent-runtime`
- [x] `crates/infra/agent/src/cli/`：新增 `cli/mod.rs`（模块声明 + re-export）；既有 `discover.rs` / `flags.rs` / `jsonl.rs` / `runner.rs` 平移为 `cli/discover.rs` / `cli/flags.rs` / `cli/jsonl.rs` / `cli/runner.rs`（内容零改动，移动非删除；既有测试模块声明随目录同步平移，不改动内容）
- [x] `crates/infra/agent/src/lib.rs`：门面骨架重构——crate 文档改写（引擎门面 + `EngineKind` 参数选择）；`mod cli;` 挂载并保持 `discover` / `build_args` / `normalize_line` / `ClaudeCliRunner` 对外形状（经 `cli` 前缀 re-export）；`EngineKind` 枚举（serde/specta camelCase）、`ResumeTranscript` 类型别名、`EngineFacade` 结构体（`new()` + `runner_for`，本阶段先落 `EngineKind::Cli` 臂，`Sdk` 臂落位于阶段四）
- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：`use agent_cli::ClaudeCliRunner` → `use agent_runtime::ClaudeCliRunner`（本阶段仅改 import 键，runner 组装改造落位于阶段五）

## 阶段二：core 契约加法变体（AC-2）

- [x] `crates/core/agent/src/runner.rs`：`AgentStartError` 增中性变体 `ConfigMissing(String)`（承接 SDK 侧 key 未配 / 模型缺失 / 会话缺失 / 非 SDK 会话），`Display` 同步分支（「配置缺失: {msg}」）；命名与文档注释保持引擎中立（不出现 sdk / engine / rig / openai 字样）；trait / `AgentEvent` / `AgentRunParams` / `RunHandle` / 状态机零触碰

## 阶段三：sdk/ 纯函数层（AC-5 / AC-6 基础）

- [x] `crates/infra/agent/src/sdk/mod.rs`：新增 `sdk/` 引擎模块声明（本阶段挂 config / policy / sandbox / tools，其余落位于阶段四）
- [x] `crates/infra/agent/src/sdk/config.rs`：`EngineConfig { api_key, base_url, model }`（三 `String` 字段）+ `from_hardcoded_slot()` 硬编码预留位构造（空缺省值 + 「手填真机验证 / 换构造源时消费面零改动」注释留痕）；门面 `lib.rs` re-export `EngineConfig`
- [x] `crates/infra/agent/src/sdk/policy.rs`：静态权限三档纯决策表 `allows(mode: AgentPermissionMode, tool: &str) -> bool`（Default 只读 / AcceptEdits +write/edit / BypassPermissions 全放行；bash 拒绝语义为将来预留档位）
- [x] `crates/infra/agent/src/sdk/sandbox.rs`：路径沙箱纯函数——入参原始路径 canonicalize 后做 workspace root（canonical）前缀校验，返回规范化路径或拒绝原因；`..\` 相对逃逸与符号链接绕过统一拦截，root 内合法路径放行；Windows `\\?\` 前缀两侧一致化处理
- [x] `crates/infra/agent/src/sdk/tools.rs`：六工具面——read / grep / glob / ls / write / edit 各自的 `ToolDefinition`（名称 / 描述 / JSON schema 入参）与异步执行体（grep = 行级子串匹配、glob = glob 模式、write = 覆写、edit = 精确串替换；bash 不进工具面）；执行体不内嵌 policy / sandbox（由 loop 统一检查）

## 阶段四：sdk/ loop 与 runner（AC-4 / AC-7 / AC-8）

- [x] `crates/infra/agent/src/sdk/normalize.rs`：rig 归一化流项 / 聚合消息 → `AgentEvent` 纯函数——assistant text → Text 块、`reasoning` → Thinking 块、tool_calls → ToolUse 块、未知流项 → `Raw{event_type:"sdk_stream", raw_json}` 透传；seq 由调用方单调传入（`AgentEvent::stamp`）
- [x] `crates/infra/agent/src/sdk/resume.rs`：store 转录 → rig 对话历史重建纯函数——仅取顶层（`parent_tool_use_id = None`）非 Raw 事件，RunStarted 忽略、Message user/assistant 重建、ToolUse/ToolResult 成对回灌；`sdk-` 前缀归属校验（非前缀 → 拒绝）；重建空历史视同会话缺失
- [x] `crates/infra/agent/src/sdk/loop.rs`：手搓多轮 agent loop——`RunStarted`（model = `EngineConfig.model`、tools = 六工具名、`mcp_servers: []`）→ 逐轮 `CompletionRequest`（chat_history + 工具 definitions）→ rig `stream()` 归一化流 → normalize 出事件 → 轮末聚合回灌；tool_calls 逐个 policy 检查（拒绝 → `SystemNotice{subtype:"permission_denied"}` + is_error ToolResult 同 id）→ sandbox 校验（拒绝 → `SystemNotice{subtype:"sandbox_denied"}` + is_error ToolResult）→ 执行出 ToolResult → 续轮；无 tool_calls 轮 → `RunResult{is_error:false, num_turns, duration_ms, cost_usd: None, usage, session_id}`；API 错误/重试经 `SystemNotice{subtype:"api_retry"/"api_error"}` + `RunResult{is_error:true}` 流出
- [x] `crates/infra/agent/src/sdk/runner.rs`：`SdkRunner` + `AgentRunner` 实现——start 校验 `EngineConfig` 非空（缺 → `AgentStartError::ConfigMissing`）→ `resume_session_id` 经 `ResumeTranscript` 解析（loader Err / None / 非 sdk- 前缀 → `ConfigMissing`，不产生 run 记录）→ rig client 组装（openai completions API + 自定义 base_url + model）→ 有界 mpsc（容量 256）+ `tokio::select!{ 流项, handle.wait_requested() }` 泵任务（停止即中止流读取、不合成 RunResult；与 CLI `pump_lines` 同构）→ 生成 `sdk-` 前缀 session id
- [x] `crates/infra/agent/src/lib.rs`：门面补 `EngineKind::Sdk` 臂（`Box::new(SdkRunner::new(engine_cfg, resume.clone()))`）与 `EngineFacade::with_resume_transcript(loader)`；`sdk/mod.rs` 补挂 normalize / resume / loop / runner

## 阶段五：壳层接线（AC-3）

- [x] `packages/desktop/src-tauri/src/commands/exec/mod.rs`：`agent_start` 签名尾部追加 `engine: Option<EngineKind>`（缺省 `EngineKind::Cli`）并透传 `start_agent_run`；文档注释补「参数选择引擎」语义（invoke body 携 engine 字段、壳层仅映射、引擎接线全在门面）——**v2 阶段八修订缺省值，本条存档**
- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：runner 组装改经门面——`for_root` 解析 store → 组装 `ResumeTranscript` loader（闭包内 `list_agent_runs` 按 `session_id` 扫描 + `list_agent_run_events` 取转录，含 `find_events_by_session` 辅助函数；store 既有 API 组合，无 store 改动）→ `EngineFacade::with_resume_transcript(loader).runner_for(engine, EngineConfig::from_hardcoded_slot())`；`start_agent_run_with` runner 参数由泛型 `&R` 收敛为 `&dyn AgentRunner`（编排体 tee 双 sink / 状态机 / 停止收敛零改动）

## 阶段六：前端引擎选择与 bindings 再生成（AC-9）

- [x] `packages/desktop/src/views/agent/components/agent-run-form.tsx`：`AgentStartInput` 增 `engine: 'cli' | 'sdk'`（镜像生成绑定 `EngineKind`，缺省 `'cli'`）；工具行增引擎二值下拉（复用 `ModeSelect`，`data-testid="agent-engine"`，cli 缺省）——**v2 阶段九修订初始值为 `'sdk'`，本条存档**
- [x] `packages/desktop/src/hooks/use-agent-chat.ts`：`AgentChatSendInput` 增可选 `engine`；body 组装 `engine: input.engine ?? 'cli'`（explore 链不传 engine，行为与演进前一致）——**v2 阶段九修订为缺席透传，本条存档**
- [x] `packages/desktop/src/lib/agent-transport.ts`：`AgentStartChainParams` 增 `engine`（生成绑定派生位置参数）；`readChainParams` 增 engine 读取（`'cli' | 'sdk'` 清单校验，缺席 → null）；`invokeStart` 追加第 9 位置参数
- [x] `packages/desktop/src/views/agent/agent-debug-view.tsx`：`start` 透传 `input.engine` 至 `sendMessage`；文档注释补引擎选择语义（时间线 / 落库 / 重放组件零改动复用）
- [x] `packages/desktop/src-tauri/src/bindings/` + `packages/desktop/src/types/generated/bindings.ts`：`pnpm -C packages/desktop run bindings:export` 再生成（`agentStart` 签名含 engine 位置参数、`EngineKind` 类型镜像；`bindings/mod.rs` 预计零手改，仅当 `EngineKind` 未被命令签名自动收集时补注册）

## 阶段七：守线（静态，不含测试执行）

- [x] `cargo fmt --all -- --check` 与 `cargo clippy --workspace`（静态编译与 lint 检查；测试执行归 test-execution 阶段承接）
- [x] `pnpm -C packages/desktop run check`（TS 静态类型 / lint）
- [x] 边界检查（零 diff / grep）：`agent-cli` / `agent_cli` 代码库零残留（openspec 历史叙述除外）；`crates/core/agent` 目录 grep `engine|rig|openai` 零命中；`crates/infra/store` 与 golden 契约文件零改动（`git diff` 范围核对）；`crates/infra/agent/src/cli/` 平移文件内容零改动（移动非删除）

---

## 阶段八：后端默认 agent 硬编码与 debug-only 语义（AC-3 v2 修订读法）

- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：新增 `pub(crate) const DEFAULT_ENGINE: EngineKind = EngineKind::Sdk;`——默认 agent 硬编码预留位（当前 rig/SDK 引擎），文档注释绑定「与 `EngineConfig::from_hardcoded_slot()` 同一座位：后续与 api key 一起改为配置读取，换源时消费面（`mod.rs` 的 `unwrap_or` 消费点与门面 `runner_for` 签名）零改动」；`start_agent_run` 文档注释补「engine 缺省收敛 `DEFAULT_ENGINE`」一句（v2 不新增文件、不改门面）
- [x] `packages/desktop/src-tauri/src/commands/exec/mod.rs`：`engine.unwrap_or(EngineKind::Cli)` → `engine.unwrap_or(DEFAULT_ENGINE)`（`use agent::DEFAULT_ENGINE` 同步引入）；两处注释改写——命令文档注释「缺省 CLI 引擎——行为与演进前一致」→「缺省硬编码默认 agent（`DEFAULT_ENGINE`，当前 SDK/rig）：引擎选择仅调试页暴露，正式场景 MUST NOT 传 engine」，行内注释「引擎缺省 CLI（Option 保证…）」→「引擎缺省收敛默认 agent（Option 保证不传 engine 的既有调用零改动）」

## 阶段九：前端穿透链缺省语义修订（AC-9 v2 增补）

- [x] `packages/desktop/src/hooks/use-agent-chat.ts`：body 组装 `engine: input.engine ?? 'cli'` → `engine: input.engine ?? null`（缺席透传不注入——缺省裁决权归后端）；`AgentChatSendInput` 注释与行内注释改写（仅调试页传入 engine；explore 链等正式场景不传、无选择 UI，走后端默认 agent）
- [x] `packages/desktop/src/views/agent/components/agent-run-form.tsx`：引擎下拉初始值 `useState<EngineKind>('cli')` → `useState<EngineKind>('sdk')`（与后端硬编码默认一致）；`ENGINE_OPTIONS` 注释与组件文档注释改写 debug-only 语义（引擎选择仅调试页暴露、正式场景无选择入口；cli 为显式可选项）
- [x] `packages/desktop/src/lib/agent-transport.ts`：`readEngine` 文档注释更新（「后端以 null 即 CLI 缺省收敛」→「null → 后端默认 agent（硬编码 SDK）」；代码零改动）
- [x] `packages/desktop/src/views/agent/agent-debug-view.tsx`：文档注释同步（「cli 缺省 / sdk 直连」→「sdk 初始（与后端默认一致）/ cli 显式可选；引擎选择仅调试页暴露，正式场景无入口」）

## 阶段十：守线（静态，不含测试执行）

- [x] `cargo fmt --all -- --check` 与 `cargo clippy --workspace`（静态编译与 lint 检查；测试执行归 test-execution 阶段承接）——clippy 全 workspace 通过；fmt 本变更触碰文件（`commands/exec/agent.rs` / `mod.rs`）零 diff，`--all` 余下 diff 均为 `crates/core/config` / `crates/core/foundation` 既有未格式化（已提交代码、不在本变更清单、工作树零改动，非本变更引入）
- [x] `pnpm -C packages/desktop run check`（TS 静态类型 / lint）——通过（exit 0；vp check 135 文件零告警、knip 零报错）
- [x] 边界检查（零 diff / grep）：`use-agent-chat.ts` 无 `?? 'cli'` 缺省注入残留；`commands/exec/mod.rs` 无 `unwrap_or(EngineKind::Cli)` 字面残留；`agent_start` IPC 签名与 `EngineKind` 零变化（bindings 产物仅随 `pnpm check` 内置 bindings:export 同步 `mod.rs` 命令文档注释镜像——specta 嵌入 doc comment，签名与 `EngineKind` 类型零 diff）；`crates/core/agent` grep `engine|rig|openai` 零命中（`event_test.rs` 命中为 "original" 子串误报）；`crates/infra/store` 与 golden 契约文件零改动；`crates/infra/agent/`（门面与 sdk/、cli/）v2 零改动（缺省修订全落壳层与前端）
