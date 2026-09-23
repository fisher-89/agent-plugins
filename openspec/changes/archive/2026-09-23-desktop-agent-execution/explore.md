# 探索: desktop-agent-execution

> **日期**: 2026-09-23
> **状态**: 形状已收敛，待 phase-proposal 收敛为正式提案
> **主题**: packages/desktop 增加执行 agent 能力；MVP 接入 Claude Code CLI；统一抽象预留多租户扩展；desktop 提供 agent 调试页

---

## 需求原貌

- packages/desktop 增加执行 agent 功能
- MVP 接入 claude code cli（纯净模式和 json stream）
- 需要做统一抽象预留扩展（CLI / 进程内 SDK / 远程 API 三种租户）
- desktop 提供 agent 调试页，本质是**调试 agent 的 loop 能力**（tool_use → tool_result 循环的可见性）

## CLI 事实基础（2026-09-23 查证，claude-code-guide）

来源：官方 CLI Reference / Headless / Permission modes / Agent SDK TypeScript Reference（code.claude.com）。

1. `--output-format`：`text` / `json` / `stream-json`。stream-json 官方示例全部搭配 `--verbose`，**组装 flag 固定带 `--verbose`**。
2. **`--bare` 存在**：跳过 hooks / skills / custom commands / subagents / plugins / MCP servers / auto memory / CLAUDE.md 的自动发现；设 `CLAUDE_CODE_SIMPLE`；**不读 OAuth 凭据与系统 keychain**，必须 `ANTHROPIC_API_KEY` 或 `--settings` 配 `apiKeyHelper`。官方称其为脚本/SDK 调用推荐模式，未来将成为 `-p` 默认。
3. stream-json 事件面（与 Agent SDK `SDKMessage` 同构）：
   - `tool_use` 块位于 **assistant** 事件 `message.content`；`tool_result` 块位于紧随的 **user** 事件 `message.content`；子代理靠 `parent_tool_use_id` 归因。
   - `result` 终点事件：`subtype`（success / error_max_turns / error_during_execution / …）、`is_error`、`num_turns`、`duration_ms`、`total_cost_usd`、`usage` / `modelUsage`、`session_id`、`permission_denials`。
   - `system` 事件 subtype 众多（init / permission_denied / api_retry / …），全集开放演进。
   - `--include-partial-messages` 产生 `stream_event`（原始 Anthropic 流增量），MVP 不启用。
4. `-p` 默认 permission mode 是 `default`（Manual）——无头下需审批的工具**直接被拒**，loop 只会显示 permission_denial 流。
5. 会话续接：`--continue` / `--resume <id>`（session id 从各事件的 `session_id` 拿）；MVP 不做，信封保留 session_id 不堵路。

## 决策表

| # | 问题 | 裁决 | 依据 / 备注 |
|---|------|------|------------|
| 1 | 输出格式 | **stream-json 唯一线上格式**（+ `--verbose`） | 统一解析路径；text 只是 UI 渲染关切 |
| 2 | 纯净模式 `--bare` | **双档位：`default`（页面默认）/ `bare`（显式开关）** | bare 纯净正是调试 loop 想要的，但不读 OAuth——默认 bare 会杀死首次运行（订阅用户无 API key）；开关旁提示认证前提 |
| 3 | 抽象预留 | **三租户（CLI / SDK / API）都留**，多抽一层**逻辑事件流**：`AgentRunner::start(params) → 事件流 + 句柄`；CLI 泵 stdout 生产流，API 轮询/watch 模拟流 | 流是 tokio mpsc channel 上的逻辑事件，不是 stdout 直译；trait 面上进程模型不可见；claude 特有细节（flag 组装、JSONL 解析）关在 ClaudeCliRunner 内部，core 不认识 claude |
| 4 | 传输 + 落库 | **Channel（命令作用域实时流）+ 事件落库重放**；重看结果走 store 查询，不引入全局事件广播 | run 寿命长于页面会话，持久化提供解耦 |
| 5 | 用户 kill | **MVP 不做** | 孤儿进程风险记录为已知限制 |
| 6 | app 层 | **不抽**（翻转信号已触发、裁决落痕），app 层微形态进化为 commands 旁的 `run_agent()` 编排函数（tee 组合点）；**agent 视为第五类边界，core/infra 分离** | 需修 desktop-app-shell（信号裁决可考）与 desktop-crate-layout（分类学增补） |
| 7 | permission-mode | **默认 `bypassPermissions`（`--dangerously-skip-permissions`）**，参数面三档下拉 `default / acceptEdits / bypassPermissions` | `-p` 默认 default 下工具直接被拒，看不到真实 loop；本机自己的 repo、调试页场景，用户裁决直接以完整循环为默认 |
| 8 | 落库维度 | **user 维度**（redb 表 `user_agent_runs` + `user_agent_run_events`） | 调试运行记录是个人活动历史（不可重建、非派生缓存），非 workspace 域派生数据；进 repo 触发三笔账 + 污染仓库；user 维度零新债 |

## 架构形状

```
┌─ React ────────────────────────────────────────────────────────────┐
│  侧栏导航（新增）:  [变更] [Agent 调试]                               │
│  AgentDebugView                                                     │
│    参数面: prompt* / env(default|bare) / permission-mode(默认bypass) │
│            / cwd=当前 workspace root（隐含）                          │
│    事件时间线: init → assistant → tool_use → tool_result → … → result│
│    原始 JSONL 切换 · 历史运行列表 → 点开重放                          │
└──────────────▲──────────────────────────▲───────────────────────────┘
               │ Channel (实时流)          │ invoke (列表/重放)
┌──────────────┴──────────────────────────┴───────────────────────────┐
│ dev-team 壳 · commands/exec/  ← 第一条真实 exec 命令落轨              │
│   agent_start        = 参数转换 → run_agent() 编排 → 错误映射         │
│   run_agent()        = 组装 runner → 事件流 tee（Channel + store sink）│
│   agent_runs / agent_run_events = 查询薄包装                          │
└──────┬───────────────────────────────┬───────────────────────────────┘
       │ implements                    │ 写入
┌──────▼──────────────┐        ┌───────▼─────────────────────────┐
│ infra/agent          │        │ infra/store                      │
│  ClaudeCliRunner     │        │  user_agent_runs        (元数据)  │
│  · spawn claude 进程  │        │  user_agent_run_events  (事件流)  │
│  · JSONL 逐行泵       │        │  user 维度，key: (run_id, seq)    │
│  · 归一化 → AgentEvent│        └─────────────────────────────────┘
│  · Windows .cmd 解析  │
└──────┬──────────────┘
       │ 依赖契约
┌──────▼─────────────────────────────────────────────────────────────┐
│ core/agent（纯，禁 Tauri 禁进程）                                    │
│  · AgentEvent 信封（+ Block/子代理归因）   · AgentRunner trait        │
│  · run 状态机（running→completed/failed）  · 事件流抽象（轮询可模拟）   │
└─────────────────────────────────────────────────────────────────────┘
```

分层纪律：进程 spawn 是 agent 边界自身的实现细节，住在 `infra/agent`（禁 Tauri，可 spawn）；`core/agent` 只放中立契约（信封、trait、状态机、事件流抽象），不认识 claude；壳 crate 的 command 维持三件事纪律，编排收在 `run_agent()`。

## AgentEvent 信封（按真实 schema 定形）

```
AgentEvent
├─ RunStarted   { model, session_id, tools, mcp_servers… }   ← system/init
├─ Message      { role, blocks[], parent_tool_use_id }        ← assistant / user
│    Block = Text | Thinking | ToolUse{id,name,input} | ToolResult{id,content,is_error}
├─ SystemNotice { subtype, payload }                          ← permission_denied / api_retry…
├─ RunResult    { subtype, is_error, num_turns, cost_usd, usage, session_id }
└─ Raw          { type, 原文 JSON }                           ← 未知的未来类型透传
```

- `Raw` 变体刻意设计：与 ArtifactEnvelope 的 Fallback 同哲学——CLI 事件面开放演进（system subtype 二十余种），未知事件透传到调试页"原始流"面板，**永不丢事件、永不炸解析**。
- loop 调试关键可见性在一等公民位：tool_use→tool_result 循环、`parent_tool_use_id` 子代理归因、每轮成本 / num_turns / session_id。
- 每事件带 `seq`（入库排序键）与时间戳。

## 调试页形态

- **侧栏长出第二个组（页面导航）**：`[变更] [Agent 调试]`——AppSidebar 首次出现非 workspace 入口语义；App.tsx 加视图切换状态（无路由，沿用状态切视图）。
- **参数面（最小集）**：prompt（必填）、env 档位（默认 default）、permission-mode 三档下拉（**默认 bypassPermissions**）；cwd 隐含 = 当前 workspace root；model 不进 MVP（继承用户默认）。
- **主面板 = 事件时间线**：assistant/user 渲染为对话流，tool_use / tool_result 折叠块，result 汇总卡片（num_turns / cost / duration / session_id 可复制）。
- **原始 JSONL 切换**：Raw 事件与所有事件的原文可见。
- **历史运行**：run 列表 → 点开从 store 重放（invoke 查询，不依赖广播）。

## 已知边界与风险（MVP 接受，须写进提案）

- **孤儿进程**：不做 kill → 应用中途关闭时 claude 进程可能残留（Windows 尤甚）。已知限制。
- **CLI 发现**：Windows 的 `claude` 是 `.cmd` shim，spawn 需 `cmd /C` 包装或解析真实入口；找不到 CLI 要显式错误。
- **bare 档认证**：无 `ANTHROPIC_API_KEY` 时 bare 运行必然失败——错误事件如实流入时间线呈现。
- **留存策略**：事件转录无上限增长，MVP 不做清理，留痕为未来账。
- **bypassPermissions 默认**： deny 规则仍生效；AskUserQuestion 类交互工具在 `-p` 下不可用表现为直接拒绝——调试页如实呈现。

## Spec 动作预览（供 phase-proposal）

1. **新增能力** `desktop-agent-execution`：agent 边界与 core/infra 分层、AgentEvent 信封、AgentRunner trait、逻辑事件流抽象（轮询可模拟）、run 持久化（user 维度两表）、无 kill MVP 边界、调试页最小参数面（含 permission-mode 默认 bypass 的显式声明）。
2. **增补 `desktop-crate-layout`**：边界分类学加 agent 第五类；细化 shell 行措辞——"**通用** shell 执行仅 desktop-app；runner 内部进程管理是 agent 边界实现细节"；租户归位表加 agent 执行 → core/agent + infra/agent。
3. **增补 `desktop-app-shell`**：翻转信号 #2（exec 第一条真实命令）已触发 → 裁决：仍不抽 app 层；`run_agent()` 编排函数为 app 层微形态（`*_inner` 先例的进化），可考。
4. **注记 `desktop-data-dimensions`**（如需）：租户表补 agent 调试运行记录 → user 维度。

## 遗留问题

- 事件 tee 的背压策略（mpsc 有界容量、store 写入节奏）→ dev-design 定。
- `run_agent()` 编排函数落点（commands/exec 模块内 vs 更小粒度）→ dev-design 定。
- 留存/清理策略 → 未来变更。
- `--resume` 续会话、`--include-partial-messages` 增量流 → 未来租户，信封已预留。
