# 提案: desktop-agent-execution

> **变更**: desktop-agent-execution
> **日期**: 2026-09-23
> **状态**: 草案

---

## 问题

packages/desktop 目前只有 change 域只读能力（三查询命令 + workspace 注册），没有任何执行 agent 能力：桌面端无法发起一次 agent 运行，更无法观察 agent 的 loop。要在 desktop 落地「执行 agent」，MVP 选型为接入本机 Claude Code CLI（`-p` 无头模式），并带来三个结构性问题：

1. **抽象预留**：执行租户未来有三种形态——本机 CLI / 进程内 SDK / 远程 API。若把 CLI 的进程模型与 JSONL 细节直接写进壳层，后两类租户落地时就要推翻重来；需要在第一天就把 claude 特有细节关进单一适配层。
2. **翻转信号**：本次是 `commands/exec/` 轨道第一条真实命令落地，按 `desktop-app-shell`「app crate 翻转信号」#2，SHALL 重新决策是否抽独立 app 层，且裁决必须落痕。
3. **loop 可见性**：调试 agent 的本质是调试 loop——assistant 消息、tool_use 与紧随的 tool_result、子代理归因（`parent_tool_use_id`）、每轮成本 / num_turns / session_id 都要在一等公民位可见，而不是埋在原始 stdout 里；desktop 需要一个专门的 agent 调试页。

CLI 事实基础（2026-09-23 查证官方 CLI Reference / Headless / Permission modes / Agent SDK TypeScript Reference）：

- stream-json 官方示例全部搭配 `--verbose`，组装 flag 固定带 `--verbose`；
- `--bare` 存在（跳过 hooks / skills / plugins / MCP / 自动记忆发现），但**不读 OAuth 凭据与系统 keychain**，必须 `ANTHROPIC_API_KEY` 或 `--settings` 配 `apiKeyHelper`——默认 bare 会杀死订阅用户的首次运行；
- `-p` 默认 permission mode 为 `default`（Manual），无头下需审批的工具**直接被拒**，loop 只会剩下 permission_denial 流；
- stream-json 事件面开放演进（system subtype 二十余种，官方标注全集开放），解析层必须容错；
- 会话续接 `--continue` / `--resume` 存在但本期不做，信封保留 `session_id` 不堵路。

---

## 提案

新增 `desktop-agent-execution` 能力：agent 视为**第五类边界**，core/infra 分离，壳层开通 exec 轨道首批命令，前端长出 Agent 调试页。

1. **双 crate 分层**：`crates/core/agent`（裸名 `agent`）放中立契约——`AgentEvent` 信封、`AgentRunner` trait、run 状态机、逻辑事件流抽象；零 Tauri、零进程 spawn、不认识 claude。`crates/infra/agent`（裸名 `agent-cli`，workspace 内唯一性约束）放 CLI 适配——进程 spawn、stdout 逐行泵、JSONL 归一化、Windows `.cmd` 发现；实现 `AgentRunner`，禁 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）。
2. **AgentEvent 信封**：`RunStarted` / `Message`（Text | Thinking | ToolUse | ToolResult 块 + `parent_tool_use_id`）/ `SystemNotice` / `RunResult` / `Raw` 五变体，每事件带 `seq` 与时间戳。`Raw` 变体刻意承接未知事件透传（与 ArtifactEnvelope 的 Fallback 同哲学）：stream-json 事件面开放演进，**永不丢事件、永不炸解析**。
3. **AgentRunner trait 与逻辑事件流**：`start(params) → (事件流, 句柄)`，事件流是 tokio mpsc channel 上的逻辑事件（非 stdout 直译）；trait 面上进程模型不可见，抽象支持假 runner（预录事件）替换供上层测试。三租户都留在 trait 预留里，MVP 仅实现 CLI 租户，SDK / API MUST NOT 预建。
4. **Claude CLI 租户（MVP）**：stream-json + `--verbose` 唯一线上格式；env 双档 `default`（页面默认）/ `bare`（显式开关，旁提示认证前提）；permission-mode 三档下拉 `default / acceptEdits / bypassPermissions`，**默认 `bypassPermissions`**（`-p` 默认档下工具直接被拒看不到真实 loop；本机自有 repo 调试场景裁决以完整循环为默认）；cwd 隐含为当前 workspace root；model 不进 MVP；无 kill、无 `--resume`。
5. **run 持久化（user 维度）**：redb 两表 `user_agent_runs`（元数据）+ `user_agent_run_events`（事件流，key `(run_id, seq)`），落既有 app data dir db。事件投递双路 tee：Tauri Channel（命令作用域实时流）+ store sink（落库）；重看结果走 store 查询重放，不依赖全局广播或进程存活。
6. **exec 轨道开通**：`agent_start`（执行，body 三件事、编排收在 `run_agent()` 编排函数——`*_inner` 微形态先例的进化）、`agent_runs` / `agent_run_events`（查询薄包装）。翻转信号 #2 随此触发，**裁决：仍不抽独立 app crate**，理由与落痕进 `desktop-app-shell` spec。
7. **Agent 调试页**：侧栏增设页面导航组（[变更] [Agent 调试]，AppSidebar 首次出现非 workspace 入口语义；无路由，沿用 state 切视图）。页面含最小参数面（prompt 必填 / env 档位 / permission-mode 下拉）、事件时间线（tool_use → tool_result 折叠块成对、子代理按 `parent_tool_use_id` 归因、result 汇总卡）、原始 JSONL 切换、历史运行重放。

不做（MVP 边界，显式留痕于 spec）：用户 kill 运行（孤儿进程为已知限制）、`--continue` / `--resume` 续会话（信封保留 `session_id`）、`--include-partial-messages` 增量流、model 选择（继承用户 CLI 默认）、事件留存清理策略。

---

## 能力

### 新增能力

- `desktop-agent-execution` — agent 边界与双 crate 分层（core/agent 契约 + infra/agent 实现）、`AgentEvent` 信封与 loop 可见性、`AgentRunner` trait 与逻辑事件流抽象（三租户预留）、Claude CLI 租户（stream-json、env 双档、permission-mode 默认 bypass）、run 事件落库与重放（user 维度两表）、exec 轨道命令面、Agent 调试页、MVP 边界留痕。

### 修改的能力

- `desktop-crate-layout` — 边界分类学由四类扩展为五类（增 agent 边界，并明确 runner 内部进程管理非 shell 边界）；三层分组树与依赖规则纳入 `core/agent` / `infra/agent-cli` 两 crate；未来租户归位表增 agent 执行行，exec 轨道与「无预建目录」场景随落地更新。
- `desktop-app-shell` — exec 轨道由空轨道改为承载 agent 首批三命令；`run_agent()` 编排函数确立为 app 层微形态（`*_inner` 先例进化）；翻转信号 #2 触发与「仍不抽 app crate」裁决落痕；Sidebar 增设页面导航组；取数模型的「无事件订阅」约束为 agent 实时流开设显式例外。
- `desktop-data-dimensions` — user 维度代表清单补 agent 调试运行记录（`user_agent_runs` / `user_agent_run_events`），维度归属显式化。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/Cargo.toml` — workspace members 增 `crates/core/agent`、`crates/infra/agent`；workspace.dependencies 增 agent 两 crate 与 tokio（mpsc 事件流，版本收敛处）
- `packages/desktop/src-tauri/crates/core/agent/`（新 crate，裸名 `agent`）— `AgentEvent` 信封（五变体 + Block + seq/时间戳）、`AgentRunner` trait 与 params、run 状态机、逻辑事件流抽象
- `packages/desktop/src-tauri/crates/infra/agent/`（新 crate，裸名 `agent-cli`）— `ClaudeCliRunner`：CLI 发现（Windows `.cmd`）、flag 组装、spawn、JSONL 逐行泵、归一化为 `AgentEvent`
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` / `model.rs` — 新增 `user_agent_runs` / `user_agent_run_events` 两表与读写操作
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — 首批三命令：`agent_start`、`agent_runs`、`agent_run_events`（空轨道注释随开通移除）
- `packages/desktop/src-tauri/src/commands/exec/agent.rs`（新，落点 dev-design 定）— `run_agent()` 编排函数：组装 runner → 事件流 tee（Tauri Channel + store sink）→ 状态收敛
- `packages/desktop/src-tauri/src/main.rs` — 注册三命令
- `packages/desktop/src/App.tsx` — 顶层视图切换 state（changes | agent，无路由）
- `packages/desktop/src/components/AppSidebar.tsx` — 页面导航组（[变更] [Agent 调试]）
- `packages/desktop/src/views/agent/AgentDebugView.tsx`（新）及子组件（参数面 / 时间线 / 原始 JSONL / 历史运行，拆分 design 定）
- `packages/desktop/src/views/agent/hooks/`（新）— agent 域 hooks（Channel 订阅 + start + 重放查询）
- `packages/desktop/src/types/dto.ts` — agent 域 DTO

### 测试文件

- `packages/desktop/src-tauri/crates/core/agent/src/*_test.rs`（新）— 信封归一化 / 状态机收敛 / 假 runner 事件流
- `packages/desktop/src-tauri/crates/infra/agent/src/cli_runner_test.rs`（新）— flag 组装、JSONL fixture 逐行解析、未知事件 Raw 透传、`.cmd` 发现（不依赖真实 CLI）
- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` — 两表写入 / (run_id, seq) 重放 / 重开持久性
- `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`（新）— 命令层（MockRuntime 构造 `State`，沿 workspaces 轨道惯例）
- `packages/desktop/src/views/agent/*.test.tsx`（新）— 参数面默认值、时间线 loop 可见性、原始流切换、历史重放
- `packages/desktop/src/components/AppSidebar.test.tsx` — 页面导航组用例
- `packages/desktop/src/App.test.tsx` — 顶层视图切换用例

### 删除文件

- 无

### 不要修改

- `packages/desktop/src-tauri/crates/core/workflow/**` — change 域读模型与 agent 无关
- `packages/desktop/src-tauri/crates/core/foundation/**` — agent 不新增磁盘路径解析
- `commands/queries/` 三命令与 `commands/workspaces/` 四命令的既有语义
- `openspec/specs/desktop-workspace-store/spec.md` 既有语义（Store::open 注入、canonical key 等）
- 刷新取数模型既有约束（查询取数显式触发；agent 实时流为唯一显式例外，见 delta）
- 插件产物链（`plugins/` / `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/`）——desktop 独立包规则，不涉插件源

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `crates/core/agent` 新 crate | `AgentEvent` 五变体信封（含 Block、`parent_tool_use_id`、seq/时间戳、Raw 透传）、`AgentRunner` trait、run 状态机落地；单测：五变体归一化、未知事件透传不炸、is_error 收敛 failed；Cargo.toml 无 tauri 系依赖、无 workspace 内依赖 |
| AC-2 | `crates/infra/agent` 新 crate | `ClaudeCliRunner` 实现 trait；单测：default+bypass 组装含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`、bare 组装含 `--bare`；JSONL fixture 逐行解析 seq 单调；未知 type 产出 Raw；CLI 缺失显式错误；Cargo.toml 无 tauri 系依赖 |
| AC-3 | store 两表 | `user_agent_runs` / `user_agent_run_events` 落地，key `(run_id, seq)`；store_test 覆盖写入、按 run 重放、重开持久性；表名携 user 维度前缀 |
| AC-4 | exec 轨道三命令 | `agent_start` 经 `run_agent()` tee 双 sink（Channel + store）；`agent_runs` / `agent_run_events` 无状态薄包装；命令层测试（MockRuntime）；CLI 不可发现返回 `Err(String)`；`main.rs` 注册齐全 |
| AC-5 | Agent 调试页 | 侧栏页面导航组切换无路由；参数面默认 env=default、permission-mode=bypassPermissions、prompt 必填；时间线呈现 assistant → tool_use → tool_result → result 汇总（num_turns / cost / duration / session_id 可复制）、子代理归因；原始 JSONL 切换；历史运行经 invoke 重放；前端测试全绿 |
| AC-6 | MVP 边界 | 无 kill / cancel 入口；无 `--resume` / `--include-partial-messages` / model 参数；五条边界限制写进 spec 可考 |
| AC-7 | spec 一致性 | `desktop-crate-layout`（五类分类学、依赖规则、租户表）、`desktop-app-shell`（exec 轨道、微形态、翻转信号落痕、导航组、流例外）、`desktop-data-dimensions`（user 维度注记）delta 与实现一致 |
| AC-8 | 管线守线 | `cargo test --workspace`（rust 套件注册于 src-tauri 根，新 crate 自动覆盖）全绿；`pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过；`vp test` 全绿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 无 kill 导致孤儿进程（应用中途关闭，Windows 尤甚） | claude 进程残留占资源、可能继续改动 workspace | 中 | MVP 已知限制显式留痕；进程可经系统工具终止；kill 能力留未来变更 |
| stream-json 事件面开放演进（system subtype 二十余种） | CLI 升级后解析失败或丢事件 | 中 | `Raw` 变体透传未知类型：永不丢事件、永不炸解析；`SystemNotice` 只取 subtype 不封闭枚举 |
| `bypassPermissions` 默认的安全面 | agent 在 workspace 内无审批改动文件 | 高（设计即如此） | 显式裁决限于本机自有 repo 调试场景；deny 规则仍生效；三档下拉可回调 default；spec 留痕 |
| Windows CLI 发现（`claude` 为 `.cmd` shim） | spawn 失败或 PATH 解析错 | 中 | `cmd /C` 包装或解析真实入口；CLI 缺失显式错误；发现逻辑单测覆盖 |
| bare 档无 `ANTHROPIC_API_KEY` 必败 | 订阅用户切 bare 后运行失败困惑 | 高 | 开关旁提示认证前提；失败错误事件如实流入时间线 |
| mpsc 背压与 store 写入节奏未定 | 长运行事件洪峰下丢事件或卡 UI | 中 | dev-design 定背压策略（有界容量、写入节奏）；store sink 兜底保证事件不丢 |
| 长运行 run 与页面会话解耦 | 切页 / 关闭页面后丢实时流观感 | 中 | 双路 tee：Channel 实时 + 落库；重看走 store 重放，不依赖广播 |
| 留存无清理、转录无上限增长 | app data db 膨胀 | 低 | MVP 已知限制留痕；个人调试使用强度可控；清理策略未来变更偿还 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 输出格式 | stream-json 唯一线上格式（+ `--verbose`） | 统一解析路径；text 只是 UI 渲染关切 | text/json 多格式适配 |
| 纯净模式 | 双档位：`default`（页面默认）/ `bare`（显式开关） | bare 纯净正是调试 loop 想要的，但不读 OAuth——默认 bare 会杀死订阅用户首次运行 | 默认 bare；不做档位 |
| 抽象预留 | 三租户（CLI / SDK / API）都留在 trait 面，多抽一层逻辑事件流：`AgentRunner::start(params) → 事件流 + 句柄` | 流是逻辑事件非 stdout 直译，trait 面上进程模型不可见；claude 细节关在 `agent-cli` 内，core 不认识 claude | 只写 CLI 直连、将来再抽 |
| 传输 + 落库 | Channel（命令作用域实时流）+ 事件落库重放；重看走 store 查询 | run 寿命长于页面会话，持久化提供解耦；不引入全局事件广播 | 全局 broadcast；仅实时不落库 |
| 用户 kill | MVP 不做 | 孤儿进程风险记录为已知限制 | 进程句柄 + cancel 命令 |
| app 层 | 不抽独立 crate（翻转信号 #2 触发后重新裁决）；编排收在 `run_agent()` 函数，视为 `*_inner` 微形态的进化；agent 视为第五类边界，core/infra 分离 | 编排单点、仍在一屏内，无跨 store + fs 多点协调；裁决落痕进 desktop-app-shell spec | 抽 app crate |
| permission-mode | 默认 `bypassPermissions`，参数面三档下拉 | `-p` 默认 default 下工具直接被拒，看不到真实 loop；本机自有 repo 调试页场景以完整循环为默认 | 默认 default（loop 不可见） |
| 落库维度 | user 维度（`user_agent_runs` / `user_agent_run_events`） | 调试运行记录是个人活动历史（不可重建、非派生缓存），非 workspace 域派生数据；进 repo 触发三笔账 + 污染仓库；user 维度零新债 | workspace 维度 |
| infra crate 命名 | core 裸名 `agent`，infra 裸名 `agent-cli` | workspace 内 crate 名唯一性约束，`infra/agent` 目录下不能用同名裸名 | infra 裸名 `agent_runner` 等 |
| spec 头沿用 | `desktop-crate-layout` 分类学 requirement 保持既有头「四类边界分类学」，正文扩展为五类 | MODIFIED delta 头 MUST 与既有 spec 精确匹配，保 archive 合并安全 | RENAMED + MODIFIED 组合（合并顺序风险） |

### 待决问题

- 事件 tee 的背压策略（mpsc 有界容量、store 写入节奏）→ dev-design 定
- `run_agent()` 编排函数落点（`commands/exec` 模块内 vs 更小粒度文件）→ dev-design 定
- 留存 / 清理策略 → 未来变更
- `--resume` 续会话、`--include-partial-messages` 增量流 → 未来租户，信封已预留

---
