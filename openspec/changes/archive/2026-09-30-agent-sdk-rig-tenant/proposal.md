# 提案: agent-sdk-rig-tenant

> **变更**: agent-sdk-rig-tenant
> **日期**: 2026-09-30
> **状态**: proposed

---

## 问题

Desktop agent 执行（能力 `desktop-agent-execution`）目前仅有本机 Claude CLI 租户：调试页与 explore 会话链强依赖本机安装 claude CLI——无 CLI 的机器上整条 agent 轨道不可用；进程外 CLI 也带来 spawn / Windows 进程树击杀等平台成本。spec 预留了三租户座位（本机 CLI / 进程内 SDK / 远程 API），并明文「本变更仅实现 CLI 租户，SDK / API 租户 MUST NOT 预建」——本变更掀 SDK 租户这个盖子。

连带问题：

1. **架构落点**：第二个引擎落地时，引擎差异若散落在壳层，`agent_start` 命令体将随引擎数量膨胀，违背三件事纪律；引擎选择若进 core 契约（`AgentRunParams` 加 engine 字段）或由 model 隐式推导，则分别违反契约中立与显式参数原则。
2. **名不副实**：`crates/infra/agent` 裸名 `agent-cli` 在承载第二个引擎后不再成立；SDK 落地后再改名成本上升（现在消费者唯壳层 + workspace 依赖一行 + spec 一段）。
3. **预留条款过时**：`desktop-agent-execution` spec 的「SDK / API 租户 MUST NOT 预建」「agent-cli」行等表述随本变更演进，需要修订。
4. **刀刃转移**：SDK 引擎的权限兜底人从 claude CLI 变成我们自己——静态权限三档与路径沙箱必须自建，且调试页默认 BypassPermissions 档下沙箱是模型与任意文件写之间的唯一护栏。

---

## 提案

以 **rig-core**（Rust LLM 框架）封装进程内 SDK 租户，定位为**进程内编码 agent**（非轻量对话伴侣），落在既有 `AgentRunner` trait 预留内：SDK 引擎同步 `start()` 返回有界 mpsc——spawn tokio 泵任务灌流，与 CLI 泵同构，**编排层（`run_agent` / tee 双 sink / 状态机）零改动**；事件信封的 ToolUse/ToolResult 成对、`parent_tool_use_id`、RunResult 汇总字段本就照 agent loop 形状设计，AgentTimeline / 落库 / 重放 / explore 链全部复用，前端除 runner 选择参数面外零改动。

架构上 **`crates/infra/agent` 升格为引擎门面**并改名 `agent-runtime`：

- 对外一套应用协议（invoke 面 + 事件流 + store 记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎；引擎分发是门面内唯一 match 点，壳层保持三件事薄命令（参数转换 → 调用 → 错误映射）。
- 协议统一在**应用协议层**而非 core trait 层：core 契约唯一触碰点为 `AgentStartError` 增中性变体（加法），trait / `AgentEvent` / `AgentRunParams` / `RunHandle` / 状态机零变化；`AgentRunParams` MUST NOT 增 engine 字段（core 不认识引擎）。
- 内部结构：`cli/` 引擎（既有 discover / flags / jsonl / runner 平移）+ `sdk/` 引擎（loop、自建工具面、静态权限 policy、路径沙箱、normalize、store 转录 resume）；单 crate 起步，拆引擎子 crate 仅留逃生门不预建。

关键决策（详见「过程·决策」）：

- **rig 恒编译、不设 feature gate**：SDK 引擎定位为不依赖本地环境（claude CLI）的 agent，开箱即跑；依赖足迹 spike 升为必过关卡。
- **provider 首公民 = openai-compatible 端点**：一个自定义 base_url 解锁 GLM / DeepSeek / Kimi / 自部署；anthropic provider 降为后续可选。normalize 层围绕 openai chat completions 形状设计（`tool_calls` 数组 → ToolUse 块）。
- **静态权限三档起步**（无审批反向通道）：Default 只读 / AcceptEdits +write/edit / BypassPermissions 全放行，**默认 BypassPermissions**（与调试页既有默认一致）；拒绝经 `SystemNotice{subtype:"permission_denied"}` + is_error ToolResult 流出。交互审批协议留独立 change。
- **路径沙箱**：write/edit/read 全链 canonicalize + workspace root 前缀校验（拦 `..\` 逃逸与符号链接绕过），纯函数密集测试；**bash 工具不进 MVP 工具面**（砍掉 bypass 默认档下最大刀刃）。
- **resume = store 即会话**：SDK 引擎不自建会话存储，resume 时从所属 workspace 库事件转录重建对话历史喂 rig；保真度缺口（Raw 丢弃 / 子代理压平 / 仅顶层）留痕。
- **engine_cfg MVP 预留硬编码位**：`EngineConfig` 结构体形态（api_key / base_url / model），用户手填做真机验证——`openspec/config.json` 为 git 追踪文件不可入机密，配置落盘座位之争（拆两截 vs 全 app 侧）顺延后续迭代，换构造源时消费面零改动。
- **注入缝两步走**：目标形状 `runner_for(kind, engine_cfg)`——engine_cfg 由壳层注入；CLI 引擎 MVP 不消费 engine_cfg（预留）。

前置 spike（design 阶段必过关卡）：rig-core 依赖足迹实测（`default-features = false` + openai feature）与 rig 当前 API 形状核实（Tool trait 签名 / streaming / 自定义 base_url / `reasoning_content` 暴露形态）——不凭记忆断言。

---

## 能力

### 新增能力

（无——本变更不新建能力 spec，全部落点为既有能力的修订。）

### 修改的能力

- **desktop-agent-execution** — 掀「SDK / API 租户 MUST NOT 预建」预留条款：`infra/agent` 升格引擎门面（crate 改名 `agent-runtime`）+ SDK（rig 进程内）租户落地；`agent_start` 增 `engine` 参数选择（命令面显式参数，core 契约零污染）；`AgentStartError` 增中性变体；Agent 调试页参数面增引擎选择；agent_stop 语义抹平为引擎中立；SDK 租户 MVP 边界留痕。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` — 包名 `agent-cli` → `agent-runtime`；新增 `rig-core` 依赖（workspace pin，`default-features = false` + openai feature）
- `packages/desktop/src-tauri/Cargo.toml` — workspace.dependencies 键 `agent-cli` → `agent-runtime`（目录名与 members 行不变）；新增 `rig-core` 版本收敛条目
- `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` — 门面重构：`EngineKind` 枚举 + `runner_for(kind, engine_cfg)` 引擎分发（唯一 match）；既有模块平移至 `cli/` 子模块（discover / flags / jsonl / runner 及其测试）；新增 `sdk/` 子模块
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/`（新）— `normalize.rs`（openai 流 → `AgentEvent`）、`policy.rs`（权限三档纯决策表）、`sandbox.rs`（canonicalize + root 前缀校验纯函数）、`tools.rs`（read/grep/glob/ls/write/edit 自建工具面）、`loop.rs`（多轮 agent loop，归属 rig multi_turn vs 手搓由 design 定稿）、`runner.rs`（`AgentRunner` 实现：spawn tokio 泵任务 + RunHandle 取消）、`resume.rs`（store 转录重建对话历史）、`config.rs`（`EngineConfig` 结构体 + 硬编码预留位）
- `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` — `AgentStartError` 增中性变体（如 `ConfigMissing(String)`，加法；Display 同步）——core 其余文件零触碰
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — `agent_start` 增可选参数 `engine: Option<EngineKind>`（缺省 `cli`）；壳层仅做参数映射
- `packages/desktop/src-tauri/src/commands/exec/agent.rs` — `start_agent_run` 改经门面 `runner_for` 组装 runner；`EngineConfig` 从硬编码预留位构造（`EngineKind::Cli` 路径不消费）
- `packages/desktop/src-tauri/src/bindings/` — 经 export-bindings 再生成（`engine` 参数与 `EngineKind` / `AgentStartError` 变化的 TS 镜像，非手改）
- `packages/desktop/src/views/agent/agent-debug-view.tsx` — 参数面增引擎选择（`cli` 默认 / `sdk` 二值下拉）；时间线 / 落库 / 重放组件零改动复用

### 测试文件

- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs`（新）— openai 形状流 fixture 归一化（tool_calls → ToolUse 成对、未知透传 Raw、seq 单调），不打网络
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`（新）— 三档纯决策表逐档断言 + 拒绝流出形态
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox_test.rs`（新）— `..\` 逃逸 / 符号链接绕过 / root 内合法路径，tempdir 驱动密集测试
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`（新）— 工具执行 tempdir 驱动
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`（新）— 假流缝注入（同 `pump_lines` 模式）：泵 / 停止取消 / 收敛
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs`（新）— store 转录 fixture 重建 + 会话缺失显式失败
- `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs` — 新变体 Display / 错误映射
- `packages/desktop/src-tauri/src/commands/exec/agent_test.rs` / `mod_test.rs` — engine 缺省映射与分发
- `packages/desktop/src/views/agent/agent-debug-view.test.tsx` — 引擎选择默认值与传参
- `packages/desktop/src/app.test.tsx` — 修复乱序执行测试导致的偶发不通过

### 删除文件

- 无净删除；既有 `discover.rs` / `flags.rs` / `jsonl.rs` / `runner.rs` 及其测试平移至 `cli/` 子目录（移动非删除）。

### 不要修改

- `crates/core/agent` 除 `AgentStartError` 加法变体外零触碰（`AgentRunner` trait / `AgentEvent` / `AgentRunParams` / `RunHandle` / 状态机不动；不出现 engine / rig / openai 字样）
- `crates/core/config`（workspace config crate）MVP 不动——配置落盘座位顺延，机密不入 `openspec/config.json`（git 追踪文件）
- `crates/infra/store` schema 不动（`AgentRunRecord` / `AgentEventRecord` 模型 shape 与 native_model id / version 零变化；引擎归属留痕座位未裁，见待决问题）
- 既有 golden 测试与 desktop detail DTO 线面契约（冻结，不可重写）
- explore 会话链页面与 `use-agent-chat` 共享会话基建（engine 缺省 cli，行为与演进前一致）
- 交互审批协议（`permission_request` 事件 + `agent_respond` 命令）、MCP、子代理——均不实现，留独立 change

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | crate 改名与门面落位：`agent-cli` → `agent-runtime`，门面 + `cli/` + `sdk/` 子模块重构，workspace 依赖与壳层消费点同步 | `cargo test --workspace` 全绿；代码库无 `agent-cli` / `agent_cli` 残留（spec 历史叙述除外） |
| AC-2 | core 契约唯一加法触碰：`AgentStartError` 增中性变体 | `crates/core/agent` 变更仅该变体及其 Display；grep 无 engine / rig / openai 字样 |
| AC-3 | `agent_start` 增 `engine` 参数（缺省 cli），壳层映射 `EngineKind` 交门面；`AgentRunParams` 无 engine 字段 | bindings 再生成含 engine 参数；不传 engine 的既有测试零改动通过；core 契约 grep 无 engine |
| AC-4 | SDK normalize：openai 形状流 → `AgentEvent`（tool_calls → ToolUse 与 ToolResult 成对、RunStarted 报 model/tools、RunResult `cost_usd = None`、未知透传 Raw、seq 单调） | normalize 纯函数 fixture 测试全绿且不打网络 |
| AC-5 | 权限三档 policy 纯决策表（Default 只读 / AcceptEdits +write/edit / BypassPermissions 全放行） | 逐档断言测试全绿；拒绝产出 `SystemNotice{permission_denied}` + is_error ToolResult |
| AC-6 | 路径沙箱：write/edit/read 全链 canonicalize + workspace root 前缀校验 | `..\` 逃逸与符号链接绕过用例全部拦截（tempdir 密集测试全绿） |
| AC-7 | SDK resume = store 转录重建；引用不存在 / 非 SDK 会话显式启动失败 | 转录 fixture 重建测试全绿；会话缺失返回 `AgentStartError` 中性变体（`Err` 抵达前端） |
| AC-8 | SDK 停止抹平：`RunHandle` 信号取消泵任务，编排零改动收敛 stopped | 假流缝测试：置位停止后泵任务终止、run 收敛 stopped、编排代码无引擎分支 |
| AC-9 | 调试页引擎选择（默认 cli）；sdk 发起运行复用时间线 / 落库 / 重放 | 组件测试全绿；真机手填 `EngineConfig`（api_key / base_url / model）后 sdk 引擎跑通含工具调用的完整 loop，落库可重放、可停止，且无本地 claude CLI 环境可运行 |
| AC-10 | rig spike 双关卡：依赖足迹实测 + API 形状实测（Tool trait / streaming / 自定义 base_url / reasoning_content） | design 文档留痕实测入树体积（`default-features = false` + openai feature）与 API 形状结论，design 据实定稿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| rig 0.x 当前 API 形状与记忆不符（Tool trait 签名 / streaming / base_url 路径） | design 返工、loop 实现方案变更 | 高 | spike 前置于 design 定稿，不凭记忆断言；loop 仅依赖 provider client + 消息/工具类型层收窄 API 面 |
| 依赖足迹超预期且 rig 恒编译无 gate 回退 | 编译时间与产物体积增长 | 中 | `default-features = false` + openai feature 最小入树；超标缓解仅剩 feature 收窄与版本选择，design 重新权衡 |
| BypassPermissions 默认档下自建沙箱是唯一护栏 | 模型越权写 root 外文件 | 中 | canonicalize + 前缀校验纯函数密集测试；bash 缺席砍掉最大刀刃；限制显式留痕 spec |
| openai 兼容端点方言差异（tool_calls 形态 / `reasoning_content` 非标扩展） | 单一端点实测通过不代表全部兼容 | 中 | MVP 首公民锁定一个已实测端点；normalize fixture 按抓样扩充；reasoning_content 映射 design 定 |
| resume 保真度缺口（Raw 丢弃 / 子代理压平 / 仅顶层） | 续话上下文缺失致行为退化 | 低 | 缺口显式留痕 spec，不由实现隐式吸收 |
| 引擎归属未记留痕（座位未裁） | 历史 run 无法区分引擎；跨引擎误用 session id | 低 | 引用非 SDK 会话显式启动失败兜底；design 裁座后本变更或后续变更偿还 |
| crate 改名触及面遗漏 | 消费点编译断裂 | 低 | 消费者唯壳层 + workspace.dependencies + members 一行；`cargo check` 即暴露 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 定位 | 进程内编码 agent（B 线），工具面与权限面由 infra 自建承接 | 对齐 CLI 租户既有协议形状（事件信封 / Timeline / 落库 / 重放全复用） | 轻量对话伴侣（否——协议红利尽失） |
| 架构落点 | `infra/agent` 升格引擎门面，协议统一在应用协议层；壳层三件事保持薄 | 引擎接线消化在门面内，命令体不随引擎数膨胀 | core 契约加 engine 字段（否，违反中立）；model 推导引擎（否，隐式耦合） |
| EngineKind 座位 | 座位 B：invoke body 携 `engine` 字段 → 壳层映射 → 门面分发；core 零污染 | 显式参数选择；core 连 claude 都不认识，不认识引擎 | core params 加字段（座位 A，否）；模型推导（座位 C，否） |
| rig 是否 feature gate | 恒编译、不设 gate | SDK 引擎 = 不依赖本地环境（claude CLI）的 agent，开箱即用；无 CLI-only 裁剪档 | feature gate 裁剪（否）；连带将依赖足迹 spike 升为必过关卡 |
| 权限模型 | 静态三档起步（Default 只读 / AcceptEdits +write/edit / Bypass 全放行），默认 BypassPermissions | 审批反向通道是契约缺口，MVP 不付演进成本；与调试页既有默认一致（本机自有 repo 调试场景以完整循环为默认） | 交互审批协议（留独立 change，CLI 租户将来同样受益） |
| bash 工具 | 不进 MVP 工具面（工具集 = read/grep/glob/ls/write/edit） | bypass 默认档下路径沙箱是唯一护栏，bash 是最大刀刃；命令级白/黑名单是假安全，真沙箱非本期体量 | 带 bash + 明示裸奔已知限制（否——风险与体量不匹配；三档表中 bash 拒绝语义保留为将来预留） |
| provider 首公民 | openai-compatible 端点（自定义 base_url） | 一个 base_url 解锁 GLM / DeepSeek / Kimi / 自部署 | anthropic provider（降为后续可选） |
| resume | store 即会话：从 workspace 库事件转录重建对话历史 | 事件模型本就是中立转录格式，不引入第二份状态 | SDK 引擎自建会话存储（否——双份状态） |
| engine_cfg 来源 | MVP 预留硬编码位（`EngineConfig` 结构体：api_key / base_url / model，用户手填真机验证） | `openspec/config.json` 为 git 追踪文件不可入机密；结构体形态保证换构造源（后续配置落盘）时消费面零改动 | 座位②拆两截（model/base_url 入 config、key 走不入库处）／座位③全 app 侧按 root 键控（均顺延后续迭代，不阻塞 MVP） |
| crate 改名 | `agent-cli` → `agent-runtime` | 升格门面后旧名名不副实；现在消费者最少，改最便宜 | `agent-engines`（否——与内部 `cli/` `sdk/` 子模块语义重复）；维持 `agent-cli`（否） |
| SDK loop 归属 | 倾向手搓 agent loop，rig 只用 provider client + 消息/工具类型层 | 流式映射 / 工具执行 / 回灌 / 收敛与 RunResult 填充全自控，契约贴合度最高 | rig multi_turn 托管（待 spike 实测 API 形状后 design 定稿） |

### 待决问题

- 引擎归属留痕座位：run 记录加列（动 store schema）/ `RunStarted` 加字段（动事件线格式 + specta DTO，探索倾向此座——事件本就是 run 自我描述，落库重放免费携带）/ MVP 不记留痕。design 裁定；若裁 `RunStarted` 座，本提案 core 触碰点清单相应增补（仅加法字段）。
- SDK loop 归属定稿（rig multi_turn vs 手搓 provider-client loop）——待 rig API 形状 spike 实测。
- GLM / DeepSeek 系 `reasoning_content` 非标扩展：映射 `Thinking` 块还是 MVP 缺席留痕——design 定。
- rig 版本 pin 策略（0.x 线是否 `=` pin）——随依赖足迹 spike 定。
- 配置落盘座位②vs③（拆两截 vs 全 app 侧）——后续迭代，不阻塞 MVP。
- 交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）——独立 change，CLI 租户将来同样受益。

---
