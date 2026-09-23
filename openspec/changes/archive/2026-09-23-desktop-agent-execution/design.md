# 设计: desktop-agent-execution

> **变更**: desktop-agent-execution
> **日期**: 2026-09-23

---

## 提案与规格同步状态

proposal.md 与 `specs/desktop-agent-execution`、`specs/desktop-crate-layout`、`specs/desktop-app-shell`、`specs/desktop-data-dimensions` 四份 delta 已由提案阶段写入并通过，**不属于本 design 的待办**；本 design 只覆盖 proposal「变更范围 - 实现文件」，并回答提案遗留的两项待决问题（tee 背压策略、`run_agent()` 落点）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| core/agent（裸名 `agent`） | agent 域中立契约：`AgentEvent` 信封（五变体 + Block + seq/时间戳 + Raw 透传）、`AgentRunner` trait 与 params、`AgentRun`/`RunHandle`、run 状态机 | `packages/desktop/src-tauri/crates/core/agent/` | serde、serde_json、tokio（仅 `sync`，mpsc） | Rust 2021；零 Tauri、零进程 spawn、零 workspace 内依赖、不认识 claude |
| infra/agent（裸名 `agent-cli`） | CLI 租户实现：CLI 发现（Windows `.cmd`）、flag 组装、spawn、stdout 逐行泵、JSONL 归一化为 `AgentEvent`，实现 `AgentRunner` | `packages/desktop/src-tauri/crates/infra/agent/` | `agent`、tokio（`process`/`io-util`/`rt`/`macros`）、serde_json | Rust 2021；允许 spawn、禁 Tauri |
| store（增两表） | run 元数据 + 事件流持久化（user 维度），重放查询入口 | `packages/desktop/src-tauri/crates/infra/store/src/{store,model,lib}.rs` | redb、serde_json（既有） | redb 两表：`user_agent_runs` / `user_agent_run_events`；不依赖 core 任何 crate |
| commands/exec | exec 轨道首批三命令：`agent_start`（三件事，编排收 `run_agent()`）、`agent_runs` / `agent_run_events`（薄包装） | `packages/desktop/src-tauri/src/commands/exec/{mod,agent}.rs` | `agent`、`agent-cli`、`store`、tauri（`Channel`/`State`） | `run_agent()` = app 层微形态（`*_inner` 先例进化） |
| 壳注册 | 注册三命令进 invoke handler | `packages/desktop/src-tauri/src/main.rs` | commands、tauri | 既有 `generate_handler!` 惯例 |
| 页面导航壳 | 顶层视图切换（changes \| agent，本地 state 无路由）+ 侧栏页面导航组 | `packages/desktop/src/App.tsx`、`src/components/AppSidebar.tsx` | react、shadcn sidebar（既有） | useState 切视图；折叠持久化沿用既有「会话内 state」决策，不重开 |
| Agent 调试页 | 参数面 / 事件时间线 / 原始 JSONL 切换 / 历史运行重放 | `packages/desktop/src/views/agent/`（View + `components/` 四子组件） | hooks、dto、shadcn ui | 无路由；事件时间线组件供实时流与重放共用 |
| agent 域 hooks | Channel 实时订阅（执行流通道例外）+ start；历史列表 + 重放查询（显式触发） | `packages/desktop/src/views/agent/hooks/` | `@tauri-apps/api`（`Channel`/`invoke`） | 沿 `useChangeList`/`useChangeDetail` 形态 |
| DTO 镜像 | agent 域 TS 类型（对齐 Rust serde camelCase） | `packages/desktop/src/types/dto.ts` | — | discriminated union，`kind` 判别 |

依赖方向（由 crate 图机械保证）：`dev-team → agent + agent-cli`、`agent-cli → agent`、`agent` 无 workspace 内依赖、两新 crate 无 tauri 系依赖、`store` 不依赖 core（事件行以 `serde_json::Value` 进出 store，见决策 D3）。

---

## 关键设计决策

### 提案待决问题收敛

| # | 待决问题（proposal） | 裁决 | 理由 |
|---|----------------------|------|------|
| D1 | 事件 tee 的背压策略 | mpsc 通道**有界容量 256**；tee 循环 `recv → 状态机 apply → store 逐事件单事务追加 → Channel 发送`；**Channel 发送失败（页面已关闭）不中断落库**；**store 写入失败立即收敛 run 为 failed（error 记因）并终止 tee**——落库是兜底路径，失败不可静默 | 逐事件单事务在个人调试强度下足够快，保证逐事件不丢；事件洪峰写入节奏若成为实测问题，未来变更改批写，不影响表结构 |
| D2 | `run_agent()` 落点 | `src/commands/exec/agent.rs`（exec 模块旁新文件）：`run_agent()` 薄入口（组装 `ClaudeCliRunner`）+ `run_agent_with()` 泛型编排（tee 双 sink + 状态收敛），假 runner 经 `run_agent_with` 注入 | 命令体保持三件事（组装 runner 不内联回命令体）；泛型缝让编排路径可被假 runner 测试；文件独立于 mod.rs，将来抽 app crate 时单文件平移 |

### 其他设计决策

| # | 问题 | 裁决 | 理由 |
|---|------|------|------|
| D3 | store 如何引用 `AgentEvent`（store 禁依赖 core） | 事件行 API 以 `serde_json::Value` 进出 store；命令层 `serde_json::to_value` / `from_value` 与 `AgentEvent` 互转 | 保住「store 不依赖 core 任何 crate」的既有机械约束，不在 store 复制五变体信封模型 |
| D4 | run_id 分配 | `Store::begin_agent_run` 在写事务内 `max(id)+1` 分配并落 `running` 行，返回含 id 的记录；不引入 uuid 依赖 | 与插入原子；单调可读 |
| D5 | 启动阶段失败是否落 run 行 | **不留行**：`AgentRunner::start` 失败（CLI 不可发现 / spawn 失败）直接 `Err(String)` 抵达前端，不产生 run 记录 | 失败的启动没有 loop 内容；历史列表只收「跑过」的 run |
| D6 | 进程异常退出（EOF 无 `result` 事件）如何收敛 | 泵任务在进程退出且未见 `result` 事件时**补发合成 `RunResult`**（subtype `error_process_exit`，is_error=true，error 记退出码）；正常 `result` 后退出不补发 | 「收敛恒由 RunResult 驱动」单一机制，状态机不需要 EOF 特判 |
| D7 | `agent_start` 的 await 形态 | async 命令**阻塞至 run 结束**，返回最终 `AgentRunRecord`；in-band 失败（`result.is_error`）返回 `Ok(failed 记录)`，仅启动阶段失败返回 `Err` | 无 kill 的 MVP 下最简正确；事件实时性由 Channel 承担，与命令返回值解耦 |
| D8 | seq 归属 | seq 由事件生产者（runner 泵 / 假 runner）在归一化时赋值，每 run 从 0 单调递增；tee 透传不重排；空白行跳过不占 seq，非 JSON 行以 `Raw` 兜底占 seq | 归一化层可单测「seq 单调」；store key `(run_id, seq)` 天然有序 |
| D9 | cwd 的传递 | 页面无 cwd 输入；前端 invoke 固定传当前 workspace root（IPC 参数 `root`），命令层转 `AgentRunParams.cwd` | spec「隐含、不设参数」指 UI 参数面；后端不引入「当前 workspace」全局状态 |
| D10 | 切页后 change 选中状态 | **不保持**：`ChangeView` unmount，`selectedChange` 重置回清单 | `useChangeList` 在 App 层不 unmount，清单数据不丢；重取数仍显式触发，不引入全局缓存 |
| D11 | 运行中切页再返回 | agent 页 remount 后经 `useAgentRunHistory` 重放落库事件呈现已有内容，**不自动续订实时流**；后续内容经历史区显式刷新/重放取得 | 符合「重放不依赖进程、查询显式触发」；Channel 是命令作用域流，不引入全局广播 |
| D12 | 原始 JSONL 视图的数据源 | 渲染每个事件的落库 JSON 形态（与重放一致）；`Raw` 变体内嵌 `rawJson` 原文保真；不为非 Raw 事件另存 CLI 原始行 | 落库即归一化形态，单一数据源；「与落库事件一致」由同源保证 |
| D13 | 历史列表刷新时机 | run 结束**不自动刷新**历史列表；列表经历史区显式刷新/点开取得 | 守「查询取数由用户显式动作触发」，Channel 例外不外溢 |
| D14 | CLI 发现实现 | 自实现 PATH 扫描（`discover_in(dirs)` 纯函数 + 读 `PATH` 的薄包装）：Windows 按 `claude.cmd`/`claude.bat`/`claude.exe` 顺序、其余按 `claude`；`.cmd`/`.bat` 经 `cmd /C` 包装 spawn；不引入 `which` crate | 依赖面最小；`discover_in` 可用合成目录单测，不依赖真实 CLI |

### 机械约束（实现须遵守，评审按此核对）

- 产品 `.rs` 全文（含 doc comment）MUST NOT 含磁盘域根目录名字面量（`foundation/src/layout_test.rs` 全包扫描，`layout.rs` 唯一例外）；spec 指针一律写「能力名 + `specs/<capability>/spec.md`（路径相对域根）」定式，沿 `commands/mod.rs` 既有注释。
- `crates/core/agent`：无 tauri 系依赖、无 workspace 内依赖、无 spawn；`crates/infra/agent`：无 tauri 系依赖；spawn 调用仅出现在 `infra/agent`。
- store 两表命名携 `user_` 前缀；`SCHEMA_VERSION` 不动（新增表不改变既有表格式，`init_schema` 内顺手打开两新表保证读事务恒可开）。
- 无 kill/cancel 入口；无 `--resume` / `--include-partial-messages` / model flag；SDK / API 租户 MUST NOT 预建。

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界；测试文件不在本清单（测试由独立工作流阶段负责）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/Cargo.toml` | crate 清单（裸名 `agent`）：serde / serde_json / tokio(sync)；无 tauri、无 workspace 内依赖 |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | 模块组装（`event` / `runner` / `state`）与公共导出；crate 级 doc 说明「中立契约、零 Tauri、零 spawn、不认识 claude」（spec 指针用相对域根定式） |
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | `AgentEvent` 信封、`AgentEventKind` 五变体、`AgentBlock` 四变体、盖戳构造（seq + 当前时间毫秒）、serde camelCase 线格式 |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `AgentRunner` trait、`AgentRunParams`、`AgentEnvMode`、`AgentPermissionMode`、`AgentRun`、`RunHandle`、`AgentStartError` |
| `packages/desktop/src-tauri/crates/core/agent/src/state.rs` | run 状态机：running → completed/failed，由 `RunResult.is_error` 驱动，收敛后拒绝变更 |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` | crate 清单（裸名 `agent-cli`）：`agent` + tokio(process/io-util/rt/macros) + serde_json；禁 Tauri |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 模块组装（`discover` / `flags` / `jsonl` / `runner`）与 `ClaudeCliRunner` 导出；crate 级 doc 说明「实现 agent 契约、允许 spawn、禁 Tauri」 |
| `packages/desktop/src-tauri/crates/infra/agent/src/discover.rs` | CLI 发现：`discover_in(dirs)` 纯函数 + 读 `PATH` 薄包装；Windows `.cmd`/`.bat`/`.exe` 顺序；缺失显式 `AgentStartError::CliMissing` |
| `packages/desktop/src-tauri/crates/infra/agent/src/flags.rs` | flag 组装纯函数：`-p <prompt>` + `--output-format stream-json --verbose` 恒有；env/permission-mode 映射（见路由/API 节表格） |
| `packages/desktop/src-tauri/crates/infra/agent/src/jsonl.rs` | stream-json 逐行归一化：行 → `AgentEventKind`（映射表见数据模型节）；未知 type / 非 JSON 行 → `Raw` 透传 |
| `packages/desktop/src-tauri/crates/infra/agent/src/runner.rs` | `ClaudeCliRunner`：discover → spawn（`current_dir(cwd)`、stdout piped；Windows shim 经 `cmd /C`）→ tokio 泵任务逐行归一化发入有界 mpsc → EOF 无 result 补发合成 RunResult |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `run_agent()` 薄入口 + `run_agent_with()` 泛型编排：组装 runner → tee 双 sink（Channel + store）→ 状态机收敛 → `finish_agent_run`（决策 D1/D2） |
| `packages/desktop/src/views/agent/AgentDebugView.tsx` | Agent 调试页骨架：参数面 + 实时时间线（原始流切换）+ 历史运行区组装；props `{ root: string \| null }` |
| `packages/desktop/src/views/agent/components/AgentRunForm.tsx` | 参数面：prompt（空则禁用启动）、env 双档默认 default（bare 旁认证前提提示）、permission-mode 三档下拉默认 bypassPermissions；无 cwd / model 输入 |
| `packages/desktop/src/views/agent/components/AgentEventTimeline.tsx` | 事件时间线（实时与重放共用）：对话流、tool_use/tool_result 折叠块成对、`parentToolUseId` 子代理分组、result 汇总卡（numTurns / cost / duration / sessionId 可复制） |
| `packages/desktop/src/views/agent/components/AgentRawStream.tsx` | 原始 JSONL 面板：逐事件 JSON dump（决策 D12） |
| `packages/desktop/src/views/agent/components/AgentRunHistory.tsx` | 历史运行区：run 列表（状态/时间/摘要）→ 点开重放 + 显式刷新按钮 |
| `packages/desktop/src/views/agent/hooks/useAgentRun.ts` | 实时运行 hook：`new Channel<AgentEvent>()` 订阅 + invoke `agent_start`；事件累积、running/error/result 三态 |
| `packages/desktop/src/views/agent/hooks/useAgentRunHistory.ts` | 历史 hook：invoke `agent_runs`（显式刷新）+ `agent_run_events`（点开重放）；沿 `useChangeDetail` 形态 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | members 增 `crates/core/agent`、`crates/infra/agent`；`[workspace.dependencies]` 增 `agent` / `agent-cli` / `tokio`；根包 `[dependencies]` 增 `agent`、`agent-cli` | workspace 级版本收敛（tokio 见配置节）；rust 套件注册于 src-tauri 根，新 crate 测试自动覆盖 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 `AgentRunRecord`（含 `encode`/`decode` JSON 编解码，沿 `WorkspaceRecord` 模式） | store 自含模型；全平文字段（决策见数据模型节） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增 `USER_AGENT_RUNS` / `USER_AGENT_RUN_EVENTS` 两表定义与五操作（begin / append / finish / list_runs / list_events）；`init_schema` 顺手打开两新表 | 事件行以 `serde_json::Value` 进出（决策 D3）；`(run_id, seq)` 复合键 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `pub use model::AgentRunRecord;` | 新模型出 crate 公共面（命令层 DTO） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 移除「预留空轨道」注释与说明；落三命令 `agent_start` / `agent_runs` / `agent_run_events`（`agent_start` 为 async，body 三件事：参数转换 → 调 `run_agent()` → 错误映射） | 结束空轨道；`run_agent` 编排在 `agent.rs` |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 模块 doc：exec 由「预留空轨道」改为「已开通（agent 首批三命令）」，补 `run_agent()` 微形态与 `*_inner` 同列的注记（spec 指针相对域根定式） | 既有 doc 与新现实一致 |
| `packages/desktop/src-tauri/src/main.rs` | `invoke_handler` 注册三命令 | 既有 `generate_handler!` 列表追加 |
| `packages/desktop/src/App.tsx` | 新增 `page` 顶层切换 state（`TopPage`）；`SidebarInset` 内容区按 page 渲染 `ChangeView` 或 `AgentDebugView`（决策 D10）；`AppSidebar` 传入 page / onPageChange | 无路由；`useChangeList` 留在 App 层不随页面卸载 |
| `packages/desktop/src/components/AppSidebar.tsx` | 新增并导出 `TopPage` 类型；props 增 `page` / `onPageChange`；清单组上方新增「页面」导航组（[变更] [Agent 调试]，lucide 图标）；workspace 清单组语义不变 | AppSidebar 首次出现非 workspace 入口语义 |
| `packages/desktop/src/types/dto.ts` | 新增 agent 域 DTO：`AgentEnvMode` / `AgentPermissionMode` / `AgentRunStatus` / `AgentBlock` / `AgentEvent`（kind 判别 union）/ `AgentRunRecord` | 对齐 Rust serde camelCase 线格式 |

### 删除文件

<!-- 无删除文件（proposal「删除文件：无」）。 -->

### 公共函数 / API

Rust 侧（二进制 crate 内为 `pub`/`pub(crate)`，签名即契约；导出 class 按公开方法逐条列出）：

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `AgentRunner.start` | `crates/core/agent/src/runner.rs` | 新增 | `fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError>`（trait 要求 `Send + Sync`） | 唯一 trait 方法；进程模型不出现在签名 |
| `RunStateMachine.new` | `crates/core/agent/src/state.rs` | 新增 | `pub fn new() -> Self` | 初始 `Running` |
| `RunStateMachine.apply` | `crates/core/agent/src/state.rs` | 新增 | `pub fn apply(&mut self, event: &AgentEvent) -> AgentRunState` | `RunResult` 驱动收敛；收敛后拒绝变更 |
| `RunStateMachine.current` | `crates/core/agent/src/state.rs` | 新增 | `pub fn current(&self) -> AgentRunState` | 当前状态观测 |
| `AgentEvent.stamp` | `crates/core/agent/src/event.rs` | 新增 | `pub fn stamp(seq: u64, kind: AgentEventKind) -> AgentEvent` | 盖当前时钟毫秒；生产者统一入口 |
| `ClaudeCliRunner.new` | `crates/infra/agent/src/runner.rs` | 新增 | `pub fn new() -> Self` | 无状态 runner 构造 |
| `ClaudeCliRunner.start` | `crates/infra/agent/src/runner.rs` | 新增 | `fn start(&self, params: AgentRunParams) -> Result<AgentRun, AgentStartError>` | `impl AgentRunner for ClaudeCliRunner`；内部 discover → spawn → 泵任务 |
| `Store.begin_agent_run` | `crates/infra/store/src/store.rs` | 新增 | `pub fn begin_agent_run(&self, run: &AgentRunRecord) -> Result<AgentRunRecord, StoreError>` | 写事务内 `max(id)+1` 分配 id，落 `running` 行，返回含 id 记录 |
| `Store.append_agent_run_events` | `crates/infra/store/src/store.rs` | 新增 | `pub fn append_agent_run_events(&self, run_id: i64, events: &[serde_json::Value]) -> Result<(), StoreError>` | 单事务批量追加；key `(run_id, seq)`，seq 取事件自带值 |
| `Store.finish_agent_run` | `crates/infra/store/src/store.rs` | 新增 | `pub fn finish_agent_run(&self, run_id: i64, record: &AgentRunRecord) -> Result<(), StoreError>` | 以传入记录整行替换（status / finished_at / 汇总 / error 由调用方填充） |
| `Store.list_agent_runs` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_agent_runs(&self) -> Result<Vec<AgentRunRecord>, StoreError>` | `started_at` 降序（与 workspace 清单同哲学） |
| `Store.list_agent_run_events` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_agent_run_events(&self, run_id: i64) -> Result<Vec<serde_json::Value>, StoreError>` | `(run_id, seq)` 区间扫描，seq 升序 |
| `agent_start` | `src/commands/exec/mod.rs` | 新增 | `#[tauri::command] pub async fn agent_start(store: State<'_, Store>, on_event: Channel<AgentEvent>, root: String, prompt: String, env: AgentEnvMode, permission_mode: AgentPermissionMode) -> Result<AgentRunRecord, String>` | IPC 键 camelCase（`onEvent` / `permissionMode`）；三件事纪律 |
| `agent_runs` | `src/commands/exec/mod.rs` | 新增 | `#[tauri::command] pub fn agent_runs(store: State<'_, Store>) -> Result<Vec<AgentRunRecord>, String>` | 无状态薄包装 |
| `agent_run_events` | `src/commands/exec/mod.rs` | 新增 | `#[tauri::command] pub fn agent_run_events(store: State<'_, Store>, run_id: i64) -> Result<Vec<AgentEvent>, String>` | 薄包装；`Value → AgentEvent` 反序列化后返回 |
| `run_agent` | `src/commands/exec/agent.rs` | 新增 | `pub(crate) async fn run_agent(store: &Store, on_event: Channel<AgentEvent>, params: AgentRunParams) -> Result<AgentRunRecord, String>` | 薄入口：组装 `ClaudeCliRunner` 后委托 `run_agent_with` |
| `run_agent_with` | `src/commands/exec/agent.rs` | 新增 | `pub(crate) async fn run_agent_with<R: AgentRunner>(store: &Store, runner: &R, on_event: Channel<AgentEvent>, params: AgentRunParams) -> Result<AgentRunRecord, String>` | 泛型编排（tee + 收敛），假 runner 注入缝（决策 D1/D2/D5） |

TypeScript 侧（组件即导出函数，props 接口同文件导出）：

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `useAgentRun` | `src/views/agent/hooks/useAgentRun.ts` | 新增 | `export function useAgentRun(root: string \| null): AgentRunState` | `AgentRunState = { events: AgentEvent[]; running: boolean; error: string \| null; result: AgentRunRecord \| null; start: (input: AgentStartInput) => void }` |
| `useAgentRunHistory` | `src/views/agent/hooks/useAgentRunHistory.ts` | 新增 | `export function useAgentRunHistory(): AgentRunHistoryState` | `AgentRunHistoryState = { runs: AgentRunRecord[]; events: AgentEvent[]; selectedRunId: number \| null; loading: boolean; error: string \| null; refresh: () => void; openRun: (runId: number) => void }` |
| `AgentDebugView` | `src/views/agent/AgentDebugView.tsx` | 新增 | `export function AgentDebugView({ root }: { root: string \| null }): React.JSX.Element` | 页面骨架 |
| `AgentRunForm` | `src/views/agent/components/AgentRunForm.tsx` | 新增 | `export function AgentRunForm(props: AgentRunFormProps): React.JSX.Element` | `props = { disabled: boolean; onStart: (input: AgentStartInput) => void }`；`AgentStartInput = { prompt: string; env: AgentEnvMode; permissionMode: AgentPermissionMode }` |
| `AgentEventTimeline` | `src/views/agent/components/AgentEventTimeline.tsx` | 新增 | `export function AgentEventTimeline(props: AgentEventTimelineProps): React.JSX.Element` | `props = { events: AgentEvent[]; running: boolean }` |
| `AgentRawStream` | `src/views/agent/components/AgentRawStream.tsx` | 新增 | `export function AgentRawStream(props: AgentRawStreamProps): React.JSX.Element` | `props = { events: AgentEvent[] }` |
| `AgentRunHistory` | `src/views/agent/components/AgentRunHistory.tsx` | 新增 | `export function AgentRunHistory(props: AgentRunHistoryProps): React.JSX.Element` | `props = { state: AgentRunHistoryState }` |
| `TopPage` | `src/components/AppSidebar.tsx` | 新增 | `export type TopPage = 'changes' \| 'agent'` | App 与 Sidebar 共用的顶层视图判别 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentEvent` | `crates/core/agent/src/event.rs` | 新增 | struct `{ seq: u64, timestamp_ms: i64, kind: AgentEventKind }`；`kind` 为 serde 内部 tag（`#[serde(flatten)]` 扁平进线格式），derive `Clone/Debug/Serialize/Deserialize` + camelCase |
| `AgentEventKind` | `crates/core/agent/src/event.rs` | 新增 | enum 五变体（RunStarted / Message / SystemNotice / RunResult / Raw），字段见数据模型节 |
| `AgentBlock` | `crates/core/agent/src/event.rs` | 新增 | enum 四变体：Text / Thinking / ToolUse{id,name,input} / ToolResult{id,content,is_error} |
| `AgentRunner` | `crates/core/agent/src/runner.rs` | 新增 | trait（三租户预留面）；`AgentRun { events: mpsc::Receiver<AgentEvent>, handle: RunHandle }` |
| `AgentRunParams` | `crates/core/agent/src/runner.rs` | 新增 | struct `{ prompt: String, cwd: PathBuf, env: AgentEnvMode, permission_mode: AgentPermissionMode }` |
| `AgentEnvMode` / `AgentPermissionMode` | `crates/core/agent/src/runner.rs` | 新增 | 单元 enum，serde camelCase（值 `default\|bare`、`default\|acceptEdits\|bypassPermissions`），附 `as_str()` 供落库 |
| `RunHandle` | `crates/core/agent/src/runner.rs` | 新增 | 预留空结构：MVP 无 kill/cancel，为未来 kill 能力挂点（trait 形状「事件流 + 句柄」的句柄半边） |
| `AgentStartError` | `crates/core/agent/src/runner.rs` | 新增 | enum `{ CliMissing(String), SpawnFailed(String) }`，实现 `Display` |
| `AgentRunState` | `crates/core/agent/src/state.rs` | 新增 | enum `Running / Completed / Failed` |
| `AgentRunRecord` | `crates/infra/store/src/model.rs` | 新增 | struct（store 自含）：`id/i64, prompt, cwd, env, permission_mode, status, started_at/i64, finished_at/Option<i64>, num_turns/Option<u64>, cost_usd/Option<f64>, duration_ms/Option<u64>, session_id/Option<String>, error/Option<String>`；status/env/permission_mode 为受控字符串（`running\|completed\|failed` 等），store 不引本地枚举 |
| `AgentEvent`（TS） | `src/types/dto.ts` | 新增 | kind 判别 union，五变体字段镜像 Rust 线格式（camelCase） |
| `AgentBlock`（TS） | `src/types/dto.ts` | 新增 | discriminated union：`text / thinking / toolUse / toolResult`（`isError` 驼峰） |
| `AgentRunRecord`（TS） | `src/types/dto.ts` | 新增 | interface，镜像 store 记录 |
| `AgentEnvMode` / `AgentPermissionMode` / `AgentRunStatus`（TS） | `src/types/dto.ts` | 新增 | 字符串字面量 union |
| `AgentStartInput` | `src/views/agent/components/AgentRunForm.tsx` | 新增 | `{ prompt, env, permissionMode }`，hooks 与表单共用 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `[workspace] members` | `packages/desktop/src-tauri/Cargo.toml` | 修改 | — | 追加 `crates/core/agent`、`crates/infra/agent`（根包仍为隐式成员不显式列出） |
| `workspace.dependencies.agent` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | `path = "crates/core/agent"` | 契约 crate 收敛 |
| `workspace.dependencies.agent-cli` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | `path = "crates/infra/agent"` | 实现 crate 收敛 |
| `workspace.dependencies.tokio` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | `{ version = "1", default-features = false, features = ["sync"] }` | 版本收敛处；`core/agent` 用 workspace 原样（仅 mpsc），`agent-cli` 追加 `features = ["process", "io-util", "rt", "macros"]` |
| 根包 `[dependencies] agent` / `agent-cli` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | `{ workspace = true }` | 壳依赖两新 crate（翻转信号 #2 裁决下仍不抽 app crate） |

<!-- 无其他配置变更：不新增 npm 依赖、不动 tauri.conf.json / capabilities（Channel 与 invoke 属 core IPC 默认权限）。 -->

---

## 数据模型

### redb 两表（user 维度，落既有 db 文件，`Store::open` 注入路径不变）

| 表 | key | value | 说明 |
|----|-----|-------|------|
| `user_agent_runs` | `id`（i64，`max+1` 分配） | `AgentRunRecord` JSON（camelCase 字节串） | run 元数据；`finish_agent_run` 整行替换收敛状态 |
| `user_agent_run_events` | `(run_id: i64, seq: u64)` 复合键 | 单个 `AgentEvent` 的 JSON 字节串 | 事件流；`(run_id, seq)` 半开区间扫描即 seq 升序重放 |

### `AgentEventKind` 五变体字段（serde camelCase 线格式 = 落库形态 = TS DTO）

| 变体 | 字段 | 源 |
|------|------|----|
| `runStarted` | `model: string \| null, sessionId: string \| null, tools: string[], mcpServers: string[]` | `type=system, subtype=init` |
| `message` | `role: string, blocks: AgentBlock[], parentToolUseId: string \| null` | `type=assistant / user` |
| `systemNotice` | `subtype: string, payload: unknown`（原 JSON） | `type=system` 其余 subtype（不封闭枚举） |
| `runResult` | `subtype: string, isError: boolean, numTurns: number \| null, durationMs: number \| null, costUsd: number \| null, usage: unknown, sessionId: string \| null` | `type=result`（`total_cost_usd → costUsd`、`duration_ms → durationMs`） |
| `raw` | `eventType: string, rawJson: string`（原文完整保留） | 未知 `type` / 非 JSON 行 / 解析退化 |

### JSONL → 变体映射（`jsonl.rs` 归一化口径）

| 输入行特征 | 产出 |
|------------|------|
| 空白行 | 跳过（不占 seq） |
| `type=system` + `subtype=init` | `runStarted`（取 `model`/`session_id`/`tools`/`mcp_servers`，缺失置 null/空） |
| `type=system` 其余 | `systemNotice{subtype, payload=原 JSON}` |
| `type=assistant \| user` | `message{role=type, blocks 由 message.content 数组映射, parent_tool_use_id}`；块映射 `text→Text`、`thinking→Thinking`、`tool_use→ToolUse{id,name,input}`、`tool_result→ToolResult{id, content 扁平化为字符串（数组拼接，其他形态 JSON 序列化兜底）, is_error}` |
| `type=result` | `runResult`（`is_error`/`num_turns`/`duration_ms`/`total_cost_usd`/`usage`/`session_id`，缺失置 null） |
| 其余已知合法 JSON、未知 `type` | `raw{eventType=type, rawJson=原文}` |
| 非 JSON 行 | `raw{eventType="unparsable", rawJson=原文}`——永不丢事件、永不炸解析 |

### flag 组装口径（`flags.rs`）

| 参数 | 命令行 |
|------|--------|
| prompt | `-p <prompt>`（恒有） |
| 输出格式 | `--output-format stream-json --verbose`（恒有） |
| env=bare | `--bare`；env=default → 无 flag |
| permission-mode=bypassPermissions | `--dangerously-skip-permissions` |
| permission-mode=acceptEdits | `--permission-mode acceptEdits` |
| permission-mode=default | 无 flag（CLI `-p` 默认档） |
| cwd | `Command::current_dir`（非 flag）；无 model / resume / partial flag |

### 关系与持久化

- `user_agent_runs 1 — N user_agent_run_events`（`run_id` 外键语义，无跨表约束）；`agent_runs` 列表 → `agent_run_events(run_id)` 重放，前端两段式还原时间线。
- 事件双路 tee：runner → mpsc(256) → tee 循环 → {Tauri Channel（实时）, store 追加（持久）}，两路 seq 一致（决策 D1/D8）。
- 不落 workspace repo、不与 workspace 维度数据混用；MVP 无清理策略（已知限制留痕 spec）。

---

## 路由 / API 设计

本变更无 HTTP API；前端可调用面为 Tauri IPC 命令（`invoke(name, args)`，args 键 camelCase），等效列示：

| 命令 | 类别 | 输入 args | 输出 | 错误 |
|------|------|-----------|------|------|
| `agent_start`（async） | 执行 | `{ onEvent: Channel<AgentEvent>, root: string, prompt: string, env: AgentEnvMode, permissionMode: AgentPermissionMode }` | `Promise<AgentRunRecord>`（run 结束时 resolve，含 failed 终态） | 启动阶段失败（CLI 缺失 / spawn 失败 / store 失败）`reject(string)` |
| `agent_runs` | 查询 | `{}` | `Promise<AgentRunRecord[]>`（startedAt 降序） | `reject(string)` |
| `agent_run_events` | 查询 | `{ runId: number }` | `Promise<AgentEvent[]>`（seq 升序） | `reject(string)` |

实时流：`agent_start` 作用域内经 `onEvent` Channel 逐事件推送（执行流通道，取数模型显式例外）；查询命令仍由用户显式动作触发。

---

## 依赖

### 运行时依赖（Rust）

- `tokio`（新，workspace 收敛：`sync` / `process` / `io-util` / `rt` / `macros`）— mpsc 逻辑事件流（core）与进程 spawn + stdout 泵（agent-cli）；不引入 `async-trait`（trait 方法为同步 `start`，内部任务自泵）
- `serde` / `serde_json`（既有）— 事件信封线格式与 store 行编码
- `redb`（既有）— 两新表持久化
- `tauri`（既有，仅壳 crate）— `Channel` / `State` / async command

### 运行时依赖（前端）

- `@tauri-apps/api`（既有）— `Channel` 订阅与 `invoke`；无新 npm 依赖
- `lucide-react`（既有）— 页面导航组图标

### 构建/测试依赖

- 无新增：`tempfile`（既有 dev-dep）覆盖 store 测试；tokio 测试所需特性不进入本清单（测试阶段自定）；CLI 发现不引入 `which`、run_id 不引入 `uuid`（决策 D4/D14）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | core/agent 四文件（变更清单「新增文件」前五项）；五变体 + Block + `parent_tool_use_id` + seq/时间戳 + Raw 透传（数据模型节映射表）；状态机收敛语义（决策 D6）；Cargo.toml 无 tauri 系 / 无 workspace 内依赖（机械约束） |
| AC-2 | infra/agent 五文件；flag 组装表（default+bypass 含四 flag、bare 加 `--bare`）；JSONL fixture 归一化 seq 单调（D8）；未知 type → Raw；`discover` 缺失显式 `CliMissing`；Cargo.toml 禁 Tauri |
| AC-3 | store 两表 + `AgentRunRecord`；`(run_id, seq)` 复合键；`user_` 前缀 + 既有 db 落位（数据模型节） |
| AC-4 | `agent_start` → `run_agent()` / `run_agent_with()` tee 双 sink（D1/D2/D7）；`agent_runs` / `agent_run_events` 无状态薄包装；启动失败 `Err(String)`（D5）；`main.rs` 注册三命令 |
| AC-5 | AgentDebugView + 四子组件 + 两 hooks；参数面默认值（AgentRunForm）；时间线 loop 可见性（AgentEventTimeline：成对折叠块 / 子代理归因 / 汇总卡可复制）；原始流切换（D12）；历史经 invoke 重放（D11/D13）；页面导航组无路由（App.tsx / AppSidebar.tsx，D10） |
| AC-6 | 机械约束清单：无 kill、无 resume/partial/model；五条边界以 spec 为准（本 design 不复写） |
| AC-7 | spec delta 与本 design 一致：五类分类学（架构组件表依赖方向行）、exec 轨道开通（commands/exec 修改项）、`run_agent()` 微形态（决策 D2）、页面导航组、流例外（路由/API 节）、user 维度注记（数据模型节） |
| AC-8 | `cargo test --workspace` 由 src-tauri 根 workspace members 自动覆盖新 crate；`client:check`（vp check --fix + knip）无新导出面外的依赖变更；最终任务含静态守线自检（fmt / clippy / check，不含测试执行） |

---

## 待决问题

- 无阻塞项。提案遗留的背压策略（D1）与 `run_agent()` 落点（D2）已在本 design 收敛。
- 未来账（已在 spec 留痕，非本 design 待决）：留存/清理策略、用户 kill（`RunHandle` 挂点已预留）、`--continue` / `--resume` 续会话、`--include-partial-messages` 增量流、SDK / API 租户。
- 实现期关注点（非待决）：Windows `cmd /C` 包装的多参数引号处理（`args` 逐个传参，实现时以发现 + 组装单测锁定口径）。
