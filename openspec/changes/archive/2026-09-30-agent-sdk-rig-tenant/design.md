# 设计: agent-sdk-rig-tenant

> **变更**: agent-sdk-rig-tenant
> **日期**: 2026-09-30（v2 回溯修订：引擎选择语义）

---

## 提案与规格同步状态

- `proposal.md` 与 `specs/desktop-agent-execution/spec.md`（增量）已随提案阶段写入并通过评审，**不在本 design 的变更清单与任务范围内**（不为其建任务）。
- 本 design 为 **v2 回溯修订**（dev-design attempt 2）：第一轮 dev-design / test-design / implement 已完成（implement verdict pass）后，用户回溯至 dev-design 补充引擎选择语义——前端仅 debug 场景可指定 engine、正式场景不允许选择 agent、前端未传参时后端使用默认 agent（当前硬编码为 rig，之后随 api key 一起改为可配置）。逐处修订与差异见「回溯修订差异表」。
- proposal 的五项待决问题中，四项（loop 归属、rig 版本 pin、reasoning_content 映射、依赖足迹/feature 形状）已由 rig spike 实测定稿（见「rig-core spike 结论」）。
- **proposal/spec 文本差异留痕**：本回溯需求将缺省引擎由 cli 修订为硬编码默认 agent（rig）——proposal AC-3 与 spec「engine 缺省与分发」scenario 中「缺省 cli」的表述随用户回溯说明整体失效；用户回溯说明为权威，实现以本 design 为准，proposal/spec 文本的同步留待评审确认或提案层补丁（不列入变更清单与任务）。

---

## 回溯修订差异表（上一版 v1 → 本版 v2）

| # | 主题 | 上一版（v1，attempt 1） | 本版（v2，attempt 2） | 依据 |
|---|------|------------------------|----------------------|------|
| 1 | 后端缺省引擎 | `agent_start` 缺省 `EngineKind::Cli`（`mod.rs` `unwrap_or(EngineKind::Cli)`，「不传 engine 行为与演进前一致」） | 缺省收敛为硬编码默认 agent `DEFAULT_ENGINE = EngineKind::Sdk`（rig）——前端不传 engine 即走默认 agent；常量与 `EngineConfig::from_hardcoded_slot` 同点同座位，预留后续与 api key 一起改为可配置 | 用户回溯需求 |
| 2 | 引擎选择定位 | 「参数选择引擎」通用可选参数（任何调用面均可传） | **debug-only**：引擎选择 UI 仅调试页表单暴露；正式场景（explore 链）无选择入口且 MUST NOT 传 engine；IPC 参数形状不变（仍尾部 `Option<EngineKind>`） | 用户回溯需求 |
| 3 | hook 缺省注入 | `use-agent-chat.ts` body 组装 `engine: input.engine ?? 'cli'`（缺席强制注入 cli，缺省裁决在前端） | `engine: input.engine ?? null`（缺席透传不注入）——缺省裁决权归后端 | 随 #1 语义必然修正 |
| 4 | 调试页表单初始值 | 引擎下拉初始 `'cli'` | 初始 `'sdk'`（与后端硬编码默认一致，避免「页面默认」与「后端默认」两个心智；cli 为显式可选项） | 设计裁定 |
| 5 | explore 链行为 | 「explore 链不传 engine，行为与演进前一致（走 CLI）」 | explore 链不传 engine，走后端默认 agent（rig）——explore 会话链运行时行为自本回溯起切换到 SDK 引擎；`EngineConfig` 硬编码位为空时以 `ConfigMissing` 显式失败、MUST NOT 静默回退 CLI | 随 #1 的必然结果；显式失败不回退为设计裁定 |
| 6 | transport `readEngine` | 注释「后端以 null 即 CLI 缺省收敛」 | 代码零改动（缺席 → null 透传行为本就正确）；注释更新为「null → 后端默认 agent」 | 语义同步 |
| 7 | AC-3 读法 | 「缺省 cli；不传 engine 的既有测试零改动通过」 | 「缺省 `DEFAULT_ENGINE`（硬编码 Sdk）」；断言 v1 缺省语义的既有测试修订归 test 阶段（不在 tasks.md） | 用户回溯需求为权威 |
| 8 | bindings | 经 export-bindings 再生成（engine 参数入签名） | v2 零再生成（IPC 签名与 `EngineKind` 不变，缺省修订落在收敛点不在签名面）；v1 已再生产物维持 | 随 #1 落点推论 |

**保持不变清单**（v1 已定稿、v2 不得回退）：

- 门面 `EngineFacade.runner_for(kind, engine_cfg)` 唯一 match、返回 `Box<dyn AgentRunner>`；core 契约唯一加法触碰（`AgentStartError::ConfigMissing(String)`）。
- `EngineKind` 双值（`cli` | `sdk`）不变——「默认 agent」是缺省收敛策略，不是第三引擎概念。
- `EngineConfig{api_key, base_url, model}` 三件套形态与 `from_hardcoded_slot()` 冻结（换构造源时消费面零改动）。
- `sdk/` 引擎九模块（config / policy / sandbox / tools / normalize / loop / runner / resume）全部语义、`cli/` 平移、crate 改名 `agent-runtime`、rig `=0.42.0` pin + native-tls 裁定维持 v1。
- `sdk-` 前缀归属校验、resume 转录重建、停止抹平、事件通道容量 256 等关键语义不变。
- transport `readEngine` 的缺席 → null 代码行为不变（仅注释修订）。
- 变更清单文件集合不变（v2 不新增文件，仅修订既有条目的修改内容）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| core 契约（唯一加法触碰） | `AgentStartError` 增中性变体 `ConfigMissing(String)`（承接 SDK 侧 key 未配 / 模型缺失 / 会话缺失等启动失败）；trait / `AgentEvent` / `AgentRunParams` / `RunHandle` / 状态机零变化 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 无（core 不依赖 workspace 内任何 crate） | Rust enum 加法 + Display |
| 引擎门面（crate 裸名 `agent-runtime`，原 `agent-cli`） | `EngineKind`（`cli` \| `sdk`）参数选择引擎；`EngineFacade.runner_for(kind, engine_cfg)` 为引擎构造唯一 match 点；`ResumeTranscript` 注入缝；壳层只见门面、不经手引擎细节；**门面不做缺省收敛**（`runner_for` 恒收显式 `EngineKind`，缺省裁决在壳层） | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | `agent`（core 契约）、`rig-core` | Rust（单 crate 起步，`cli/` + `sdk/` 子模块） |
| `cli/` 引擎（既有平移） | CLI 发现（Windows `.cmd` shim）、flag 组装、stdout JSONL 泵、进程树击杀——四模块自 `src/` 平移至 `src/cli/`，内容零改动 | `packages/desktop/src-tauri/crates/infra/agent/src/cli/`（discover / flags / jsonl / runner） | `agent`、tokio | Rust（唯一 spawn 触点） |
| `sdk/` 引擎——config | `EngineConfig{api_key, base_url, model}` 结构体 + MVP 硬编码预留位构造 `from_hardcoded_slot()`（用户手填真机验证；换构造源时消费面零改动）——**与壳层 `DEFAULT_ENGINE` 常量同属「与 api key 一起可配置化」座位（v2）** | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | 无 | Rust struct |
| `sdk/` 引擎——policy | 静态权限三档纯决策表（档位 × 工具名 → 允许/拒绝），默认 BypassPermissions；bash 不在工具面（拒绝语义为将来预留档位） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | `agent`（`AgentPermissionMode`） | Rust 纯函数 |
| `sdk/` 引擎——sandbox | write/edit/read 全链 canonicalize + workspace root 前缀校验（拦 `..\` 逃逸与符号链接绕过）；BypassPermissions 默认档下唯一护栏 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox.rs` | 无（std 路径 API） | Rust 纯函数 |
| `sdk/` 引擎——tools | 自建工具面：read / grep / glob / ls / write / edit 六工具的定义（名称/描述/JSON schema 入参）与执行体；bash MUST NOT 进 MVP | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | `glob`（glob 工具）、serde_json | Rust（临时目录可驱动的执行函数） |
| `sdk/` 引擎——normalize | 引擎流 → `AgentEvent` 归一化：rig 归一化流项/聚合消息 → ToolUse 块（与 ToolResult 同 id 成对）、Thinking 块、未知项 Raw 透传、seq 单调盖戳 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` | `agent`（事件模型）、rig 流类型 | Rust 纯函数 |
| `sdk/` 引擎——loop | 手搓多轮 agent loop：流式响应 → 映射事件 → policy/sandbox 检查 → 执行工具 → 回灌 → 续轮；收敛与 `RunResult` 填充全自控（rig 仅作 provider client + 消息/工具类型层，multi_turn 托管不采用） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | rig（openai completions 模型）、`sdk/` 内各模块 | Rust async |
| `sdk/` 引擎——runner | `AgentRunner` 实现（`SdkRunner`）：start 校验 engine_cfg / 解析 resume → 组装 rig client → spawn tokio 泵任务跑 loop 灌有界 mpsc；`RunHandle` 信号取消泵任务（与 CLI 泵同构） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `agent`（trait/RunHandle）、`sdk/loop.rs` | Rust async（mpsc 有界通道，容量与 CLI 泵同为 256） |
| `sdk/` 引擎——resume | store 转录重建对话历史：顶层非 Raw 事件 → rig 消息序列（`sdk-` 前缀会话归属校验、缺失/非 SDK 会话显式失败） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` | `agent`（`AgentEvent`）、rig 消息类型 | Rust 纯函数 |
| 壳层命令面 | `agent_start` 尾部可选参数 `engine: Option<EngineKind>`——三件事纪律不变（参数转换 → 调用 → 错误映射）；**缺省收敛 `unwrap_or(DEFAULT_ENGINE)`（v2：硬编码默认 agent，`pub(crate) const DEFAULT_ENGINE: EngineKind = EngineKind::Sdk`，住 `agent.rs` 组装点、与 `EngineConfig::from_hardcoded_slot()` 同点同注释——两硬编码位绑定同一后续配置座位）**；`start_agent_run` 经门面组装 runner（`&dyn AgentRunner`），从硬编码位构造 `EngineConfig`、从 store 组装 resume 转录 loader | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/src/commands/exec/agent.rs` | `agent-runtime`、`store`、tauri | Rust（tauri-specta 命令） |
| 调试页参数面 | 引擎选择下拉（`sdk` 初始——与后端默认一致 / `cli` 显式可选项）入参数面最小集；**引擎选择 UI 仅调试页暴露（debug-only，v2）——正式场景无 agent 选择入口**；时间线 / 落库 / 重放组件零改动复用 | `packages/desktop/src/views/agent/agent-debug-view.tsx`、`packages/desktop/src/views/agent/components/agent-run-form.tsx` | 生成 bindings、use-agent-chat | React + TS |
| 会话基建 / transport | engine 参数沿发送链穿透（**调试链显式传、正式链缺席透传，hook 不注入缺省值——v2**）：`AgentChatSendInput.engine`（可选）→ body → transport `readChainParams`（缺席 → null）→ `commands.agentStart` 位置参数 → 壳层 `unwrap_or(DEFAULT_ENGINE)` | `packages/desktop/src/hooks/use-agent-chat.ts`、`packages/desktop/src/lib/agent-transport.ts` | 生成 bindings、ai | TS |
| bindings 镜像 | `engine` 参数 / `EngineKind` / `AgentStartError` 变化的 TS 镜像，经 `pnpm -C packages/desktop run bindings:export` 再生成（非手改）；**v2 IPC 签名零变化 → 零再生成** | `packages/desktop/src-tauri/src/bindings/`（specta builder）→ 产物 `packages/desktop/src/types/generated/bindings.ts` | tauri-specta | 生成代码 |

引擎接线全景（门面是唯一 match 点，编排层零引擎分支；**缺省裁决在壳层，缺省值 = 硬编码默认 agent（v2）**）：

```
调试页: form(engine 下拉, 仅调试页暴露) → use-agent-chat(body.engine, 显式传)
正式场景(explore 链): use-explore-session → use-agent-chat(不传 engine, 无选择 UI) → body.engine 缺席
  → use-agent-chat body 组装(engine: input.engine ?? null, 不注入缺省) → transport readEngine(缺席 → null)
  → commands.agentStart(engine: Option<EngineKind>) → 壳层 mod.rs unwrap_or(DEFAULT_ENGINE = Sdk 硬编码默认 agent)
  → agent.rs: EngineConfig(硬编码位) + ResumeTranscript(store loader)
  → EngineFacade.runner_for(kind, engine_cfg)──match──┬─ Cli → cli/ClaudeCliRunner（engine_cfg 忽略；显式选 cli 才触达）
                                                      └─ Sdk → sdk/SdkRunner（engine_cfg 组装 rig client；缺省与显式 sdk 皆触达）
  → start_agent_run_with(runner: &dyn AgentRunner)（tee 双 sink + 状态机，零改动复用）
```

---

## rig-core spike 结论（AC-10 留痕，design 据实定稿）

spike 以临时 scratch crate 实测（不入仓库树），四项关卡结论如下：

1. **版本与 feature 形状实测**：rig-core 最新稳定线 **0.42.0**（0.x，无 semver 兼容承诺）；`edition = "2024"`（本机 rustc 1.95.0 满足）。**无 per-provider feature**——提案预设的「`default-features = false` + openai feature」组合**不存在**（provider features 仅存在于 0.22 旧线，0.42 已移除、全部 provider 恒编译）；可用 features：`default = [reqwest, derive, rustls]`、`audio`、`epub`、`image`、`pdf`、`rayon`、`reqwest-middleware`、`socks`、`websocket`、`native-tls`、`reqwest-rustls`/`reqwest-tls` 等。
2. **依赖足迹实测**：`reqwest 0.13` 为 rig **非 optional 硬依赖**（`default-features = false`，自带 json/stream/multipart）；desktop 锁内已有 `reqwest 0.13.5`（tauri 引入）——**同主版本共享，零重复入树**。以 `default-features = false + ["reqwest", "native-tls"]` 实测，相对 desktop Cargo.lock **新增 12 个 crate**：`as-any`、`eventsource-stream`、`futures-timer`、`hyper-tls`、`mime_guess`、`minimal-lexical`、`native-tls`、`nom`、`ordered-float`、`rig-core`、`tokio-native-tls`、`tracing-futures`——全部小型纯 Rust；`schemars 1.2.2` 锁内已有（共享）。对照组合 `["reqwest", "rustls"]` 实测 190 vs 146 crates，且拉入 `aws-lc-rs` + `aws-lc-sys` + `cmake`（C 工具链构建成本，Windows 构建脆化）——**否决**。TLS 裁定：`native-tls`（Windows 走 schannel，锁内已有）；已知代价：Linux 构建将引入 openssl-sys（desktop 以 Windows 为构建/分发目标，可接受）。
3. **API 形状实测**（rig 0.42.0 源码核实，非记忆）：
   - provider client：openai `Client::builder()`（builder 宏生成）+ `ClientBuilder::base_url(...)`（client/mod.rs:829）支持自定义 base_url；`.completions_api()` 切换 Responses API → **Chat Completions API**（normalize 围绕 chat completions 形状，取此路径）。
   - completion 面：`trait CompletionModel { fn completion(...); fn stream(&self, request: CompletionRequest) -> Result<StreamingCompletionResponse, CompletionError>; }`——`StreamingCompletionResponse` 为归一化流（内置 abort handle）；openai 模型另有 inherent `raw_stream`（streaming.rs:319）暴露 provider 线格式，作回退缝。
   - 工具面：`PortableDynamicTool::new(name, description, parameters: serde_json::Value, callback: Fn(Value) -> Future<Result<ToolOutput, ToolExecutionError>>)` 动态工具闭包缝 + `ToolDefinition{name, description, parameters}`——与自建六工具面（policy/sandbox 在回调外层统一检查）贴合。
   - `reasoning_content`：**已原生解析**——openai assistant 消息 `reasoning: Option<String>`（serde rename `reasoning_content`、alias `reasoning`，completion/mod.rs:169-177；流式侧 streaming.rs:82），且工具轮 assistant 消息自动回显该字段。
4. **据实定稿**（消解 proposal 待决问题）：
   - **loop 归属 = 手搓 agent loop**：rig 只用作 provider client + 消息/工具类型层（`CompletionRequest` / `Message` / `ToolDefinition` / `PortableDynamicTool` / `StreamingCompletionResponse`）。rig multi_turn agent 托管不采用——其托管工具执行与 preamble 管制与我们的 `AgentEvent` 信封、policy 插值（拒绝 + 合成 ToolResult）、`RunResult` 填充语义不贴，贴合度要求全自控。
   - **rig 版本 pin = `=` 精确 pin `0.42.0`**（0.x 线无兼容承诺，纪律同 `native_db` / `specta` / `tokei` 的 workspace `=` pin）。
   - **reasoning_content 映射 = `AgentBlock::Thinking`**（rig 已解析为 assistant `reasoning` 字段，映射即可见；不缺席留痕）。
   - **normalize 输入形状 = rig 归一化流项**（rig 上游即 openai chat completions wire，tool_calls 聚合与 reasoning_content 解析复用库语义，不重造线格式累积器）；未知 rig 流项 / 非预期形态 → `Raw{event_type:"sdk_stream", raw_json}` 透传。若实现期发现归一化流缺失 `RunResult` 所需汇总（finish_reason / usage），回退缝为 openai 模型 inherent `raw_stream`（实测存在）。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  覆盖 proposal「变更范围-实现文件」全部条目；测试文件（proposal 测试文件节）不在本清单，
  由 test-design / test-gen / test-execution 阶段承接。
  壳层前端穿透链四个级联文件为「清单外补入」，逐条注明理由。
  v2 回溯修订以「v2 修订」标记内嵌于既有条目（不另立附录）；v2 不新增文件。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/mod.rs` | `cli/` 引擎模块声明与 re-export（discover / flags / jsonl / runner 平移落位） |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/discover.rs` | 自 `src/discover.rs` 平移（移动非删除，内容零改动） |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` | 自 `src/flags.rs` 平移（移动非删除） |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl.rs` | 自 `src/jsonl.rs` 平移（移动非删除） |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` | 自 `src/runner.rs` 平移（`ClaudeCliRunner` / `pump_lines` / 击杀缝，移动非删除；其测试 `runner_test.rs` 等四个测试文件随目录同步平移） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | `sdk/` 引擎模块声明 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | `EngineConfig` 结构体 + `from_hardcoded_slot()` 硬编码预留位（空缺省值：未手填时 SDK 启动以 `ConfigMissing` 显式失败） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | 权限三档纯决策表（档位 × 工具名 → 允许/拒绝；bash 在工具面缺席，表中拒绝语义为将来预留） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox.rs` | 路径沙箱纯函数：入参原始路径 → canonicalize → workspace root（canonical）前缀校验；`..\` 逃逸与符号链接绕过统一拦截 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | 六工具面：read / grep / glob / ls / write / edit 的 `ToolDefinition` 与执行体（grep = 行级子串匹配、glob = glob 模式、write = 覆写、edit = 精确串替换；bash 不进面） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` | rig 归一化流项/聚合消息 → `AgentEvent`：assistant text → Text 块、`reasoning` → Thinking 块、tool_calls → ToolUse 块、未知项 → Raw 透传；seq 由调用方单调传入 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | 手搓多轮 agent loop（轮循环 / policy-sandbox 拒绝合成 / 工具执行回灌 / RunResult 填充 / 停止信号协同） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `SdkRunner`：engine_cfg 校验 → resume 解析（经 `ResumeTranscript`）→ rig client 组装 → spawn 泵任务；`RunHandle` 取消 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` | store 转录 → rig 对话历史重建纯函数（仅顶层、Raw 丢弃、子代理压平；`sdk-` 前缀归属校验） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` | 包名 `agent-cli` → `agent-runtime`；新增 `rig-core`（workspace pin，`default-features = false`，features `["reqwest", "native-tls"]`）与 `glob` 依赖；tokio features 复核（rt 已备） | 门面 crate 载体；glob 为 rig 传递依赖同款（0.3，已在入树），glob 工具直连零新增足迹 |
| `packages/desktop/src-tauri/Cargo.toml` | workspace.dependencies 键 `agent-cli` → `agent-runtime`（目录名与 members 行不变）；新增 `rig-core = { version = "=0.42.0", default-features = false, features = ["reqwest", "native-tls"] }` 收敛条目；根包依赖行 `agent-cli` → `agent-runtime` | workspace 依赖收敛；`=` pin 依 spike 裁定 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 门面重构：crate 文档改写；`mod cli; mod sdk;`；`EngineKind` 枚举、`ResumeTranscript` 类型别名、`EngineFacade`（`runner_for` 唯一 match）；re-export `EngineConfig`；原 `pub use` 平移至 `cli` 前缀保持对外形状 | 引擎门面落位核心；v2 零修订（门面不做缺省收敛，缺省裁决在壳层） |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `AgentStartError` 增 `ConfigMissing(String)` 变体 + Display 分支（「配置缺失: {msg}」）；其余零触碰 | core 唯一加法触碰点（AC-2）；Display 消息区分成因（key 未配 / 模型缺失 / 会话缺失） |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `use agent_runtime::...` 替换 `agent_cli::ClaudeCliRunner`；`start_agent_run` 改为：`for_root` 解析 store → 组装 `ResumeTranscript` loader（闭包内 `list_agent_runs` 扫描 session_id → `list_agent_run_events`，`sdk-` 前缀校验）→ `EngineFacade::with_resume_transcript(loader).runner_for(engine, EngineConfig::from_hardcoded_slot())`；`start_agent_run_with` 的 runner 参数由泛型 `&R` 改为 `&dyn AgentRunner`（适配 `Box<dyn AgentRunner>`，编排体零改动）；新增 `find_events_by_session` 辅助（store 既有 API 组合，无 store schema/API 变化）。**v2 修订**：新增 `pub(crate) const DEFAULT_ENGINE: EngineKind = EngineKind::Sdk;`——默认 agent 硬编码预留位（当前 rig），文档注释绑定「与 `EngineConfig::from_hardcoded_slot()` 同一座位：后续与 api key 一起改为配置读取，换源时消费面零改动」；`start_agent_run` 文档注释补「engine 缺省收敛 `DEFAULT_ENGINE`」 | runner 组装经门面（MUST NOT 命令面直接构造引擎）；CLI 路径不消费 engine_cfg 与 loader；默认 agent 常量住组装点与 engine_cfg 硬编码位成对 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start` 尾部可选参数 `engine: Option<EngineKind>`；文档注释补 engine 语义。**v2 修订**：缺省收敛 `engine.unwrap_or(EngineKind::Cli)` → `engine.unwrap_or(DEFAULT_ENGINE)`（`use agent::DEFAULT_ENGINE` 同步）；两处注释改写——命令注释「缺省 CLI 引擎——行为与演进前一致」→「缺省硬编码默认 agent（`DEFAULT_ENGINE`，当前 SDK/rig）：引擎选择仅调试页暴露，正式场景 MUST NOT 传 engine」，行内注释「引擎缺省 CLI」→「引擎缺省收敛默认 agent（`Option` 保证不传 engine 的既有调用零改动）」 | 命令面显式参数选择引擎、缺省裁决归后端（v2）；core `AgentRunParams` 零污染（AC-3） |
| `packages/desktop/src-tauri/src/bindings/` | 经 `pnpm -C packages/desktop run bindings:export` 再生成（`EngineKind` 经 `agent_start` 签名自动收集，`bindings/mod.rs` 预计零手改；仅当类型未被自动收集时补注册）。**v2：零再生成**（IPC 签名与 `EngineKind` 不变） | specta builder 组装（非手改面） |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | `start` 透传 `input.engine` 至 `sendMessage`；文档注释补引擎选择语义。**v2 修订**：注释同步（「cli 缺省」→「sdk 初始 / 缺省与后端默认一致；引擎选择仅调试页暴露，正式场景无入口」） | proposal 实现文件条目；时间线 / 落库 / 重放组件零改动（AC-9） |

**清单外补入（级联文件，实现 engine 参数穿透所必需，proposal 实现文件条目未逐一列出）：**

| 文件路径 | 修改内容 | 补入理由 |
|----------|----------|----------|
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | `AgentStartInput` 增 `engine: 'cli' \| 'sdk'`；参数面工具行增引擎二值下拉（复用 `ModeSelect`，`data-testid="agent-engine"`）。**v2 修订**：下拉初始值 `'cli'` → `'sdk'`（与后端硬编码默认一致；cli 为显式可选项）；`ENGINE_OPTIONS` 与组件文档注释改写 debug-only 语义（引擎选择仅调试页暴露；正式场景无选择入口） | proposal「参数面增引擎选择」的实际渲染落点在表单组件；`agent-debug-view.tsx` 仅消费 `AgentStartInput` |
| `packages/desktop/src/hooks/use-agent-chat.ts` | `AgentChatSendInput` 增可选 `engine`。**v2 修订**：body 组装 `engine: input.engine ?? 'cli'` → `engine: input.engine ?? null`（缺席透传不注入——缺省裁决权归后端）；注释改写（仅调试页传入 engine；explore 链等正式场景不传、无选择 UI，走后端默认 agent） | explore 链（`use-explore-session.ts`）不传 engine，经后端缺省收敛走默认 agent（v2 起 SDK 引擎） |
| `packages/desktop/src/lib/agent-transport.ts` | `AgentStartChainParams` 增 `engine`（生成绑定派生位置参数）；`readChainParams` 增 `engine` 读取（清单校验：`'cli' \| 'sdk'`，缺席 → null）；`invokeStart` 追加第 9 位置参数。**v2 修订**：`readEngine` 文档注释更新（「后端以 null 即 CLI 缺省收敛」→「null → 后端默认 agent（硬编码 SDK）」；代码零改动） | transport 为 body → `commands.agentStart` 的唯一穿透点；缺席 → null 透传行为 v2 保持不变 |
| `packages/desktop/src/types/generated/bindings.ts` | export-bindings 再生成产物（`agentStart` 签名 + `EngineKind` 类型镜像）。**v2：零变化** | proposal「bindings 再生成」的实际产物路径（proposal 条目写 `src-tauri/src/bindings/` 即 specta builder 侧） |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `EngineFacade::new` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn new() -> Self` | 无 resume 注入的门面（CLI 路径零成本） |
| `EngineFacade::with_resume_transcript` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_resume_transcript(loader: ResumeTranscript) -> Self` | 携 store 转录 loader 的门面；CLI 引擎持有但不消费 |
| `EngineFacade::runner_for` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Box<dyn AgentRunner>` | 引擎构造唯一 match 点；返回形状 design 定稿为 `Box<dyn AgentRunner>`（引擎类型不出门面） |
| `EngineConfig::from_hardcoded_slot` | `crates/infra/agent/src/sdk/config.rs`（门面 re-export） | 新增 | `pub fn from_hardcoded_slot() -> EngineConfig` | MVP 硬编码预留位构造（api_key / base_url / model 手填真机验证）；与壳层 `DEFAULT_ENGINE` 同座位，后续迭代换构造源时本函数消费面零改动 |
| `AgentStartError::ConfigMissing` | `crates/core/agent/src/runner.rs` | 修改 | 变体 `ConfigMissing(String)`（Display 分支同步） | 中性命名，core 不出现 sdk / engine 字样；承接 key 未配 / 模型缺失 / 会话缺失 / 非 SDK 会话 |
| `agent_start` | `src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, resume_session_id: Option<String>, source: Option<String>, source_ref: Option<String>, parent_run_id: Option<i64>, engine: Option<EngineKind>) -> Result<AgentRunRecord, String>` | 尾部追加（位置参数稳定性）；`Option` 保证不传 engine 的既有调用零改动；**v2 缺省语义修订（缺省 → 硬编码默认 agent）落在收敛点，签名零变化** |

（`start_agent_run_with` 的 runner 参数改 `&dyn AgentRunner` 为 app 层内部编排缝（`pub(crate)`），不列公共 API；`SdkRunner` 仅经 `Box<dyn AgentRunner>` 出门面，其方法面不对外；`DEFAULT_ENGINE` 为 `pub(crate)` 常量、app 层内部缺省收敛缝（v2 新增），不列公共 API——v2 无公共 API 形状变化。）

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `EngineKind` | `crates/infra/agent/src/lib.rs` | 新增 | `enum EngineKind { Cli, Sdk }`，serde/specta camelCase（`"cli" \| "sdk"`）；住 infra 门面，core 零污染；v2 不新增第三值 |
| `EngineConfig` | `crates/infra/agent/src/sdk/config.rs`（门面 re-export） | 新增 | `struct EngineConfig { api_key: String, base_url: String, model: String }`；三件套形态冻结，换构造源时消费面零改动 |
| `ResumeTranscript` | `crates/infra/agent/src/lib.rs` | 新增 | `pub type ResumeTranscript = std::sync::Arc<dyn Fn(&str) -> Result<Option<Vec<AgentEvent>>, String> + Send + Sync>`；session_id → 事件转录（`None` = 会话不存在/非 SDK 产出；`Err` = 库读取失败），引擎中立数据（CLI 忽略） |
| `AgentStartError` | `crates/core/agent/src/runner.rs` | 修改 | + `ConfigMissing(String)`（加法，既有两变体与形态不变） |
| `AgentStartInput`（TS） | `src/views/agent/components/agent-run-form.tsx` | 修改 | + `engine: 'cli' \| 'sdk'`（类型镜像自生成 bindings `EngineKind`；调试页表单恒显式传值） |
| `AgentChatSendInput.engine`（TS） | `src/hooks/use-agent-chat.ts` | 修改 | `engine?: EngineKind`——可选；**v2 语义：缺席不注入缺省值（正式场景不传，后端默认生效）** |

### 配置

<!-- 本变更不涉及配置文件键变更：MVP 的 engine_cfg 落硬编码预留位（sdk/config.rs 源码常量）、
默认 agent 落壳层硬编码常量（agent.rs DEFAULT_ENGINE，v2 新增）——两者为同一可配置化座位的两半，
后续迭代与 api key 一起改为配置读取；openspec/config.json 为 git 追踪文件，机密 MUST NOT 入库
（配置落盘座位②vs③顺延后续迭代，见待决问题）。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `EngineConfig` | `api_key: String`、`base_url: String`、`model: String` | `EngineFacade.runner_for` 的第二参；SDK 引擎组装 rig client 的唯一来源；与壳层 `DEFAULT_ENGINE` 常量同座位（后续一起可配置化，v2） | 无（MVP 硬编码位，源码常量；不入任何配置文件） |
| `DEFAULT_ENGINE`（壳层常量，v2） | `pub(crate) const DEFAULT_ENGINE: EngineKind = EngineKind::Sdk` | `agent_start` 缺省收敛的唯一取值点（`mod.rs` `unwrap_or`）；与 `EngineConfig::from_hardcoded_slot()` 同组装点同注释，绑定同一后续配置座位 | 无（源码常量） |
| `ResumeTranscript`（loader 产物） | 输入 `session_id: &str` → 输出 `Option<Vec<AgentEvent>>` | 壳层组装（store 既有 API：`list_agent_runs` 按 `session_id` 扫描 → `list_agent_run_events`）；SDK runner `start()` 时调用 | 无（内存态；store 侧零 schema/API 变化） |
| SDK 会话转录重建（`resume.rs` 输出） | rig 对话历史消息序列（user / assistant text / tool_use ↔ tool_result 成对回灌） | 喂入 loop 首轮 `CompletionRequest.chat_history`；仅顶层（`parent_tool_use_id = None`）、Raw 丢弃、子代理压平（保真度缺口已留痕 spec） | 无（内存态，不引入第二份会话存储——store 即会话） |
| SDK session id | `sdk-` 前缀 + 进程内原子计数 + 毫秒时戳（如 `sdk-1-1759...`） | 写入 `RunResult.session_id` → 落 run 记录；resume 扫描按前缀定位——**前缀即引擎归属标记**（非 SDK 前缀 → 显式启动失败），不以 store schema 变化为代价；**explore 链自 v2 起默认产出 `sdk-` 会话** | 经既有 run 记录 `session_id` 字段落库 |
| store schema（不变声明） | `AgentRunRecord` / `AgentEventRecord` | 模型 shape 与 native_model id / version 零变化 | redb（既有 workspace 库文件） |

---

## 路由/API 设计

本变更不涉及 HTTP API；下表为 Tauri invoke 面（IPC 命令）的唯一变化点。

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| invoke | `agent_start` | 发起 agent 运行（提前 resolve running 记录）；尾部可选参数 `engine`（`"cli" \| "sdk"` \| null）——**v2 缺省语义：null → 后端默认 agent（硬编码 `DEFAULT_ENGINE = Sdk`，rig），不再是 CLI**；IPC 形状（签名/类型）零变化 | `Result<AgentRunRecord, String>`；SDK 配置缺失 / 会话缺失经 `AgentStartError::ConfigMissing` 以 `Err(String)` 抵达前端 | 无（本机应用） |

`agent_stop` / `agent_runs` / `agent_run_events` / `agent_run_chain` 零变化（`agent_stop` 经 `RunHandle` 对 SDK 引擎同样生效——信号取消泵任务，编排零改动收敛 stopped）。explore 链（`source="explore"`）不传 `engine`，经后端缺省走 SDK 引擎——其 run 记录 `source` 面零变化，仅引擎归属随缺省切换。

---

## 依赖

### 运行时依赖

- `rig-core`（`workspace` pin：`=0.42.0`，`default-features = false`，features `["reqwest", "native-tls"]`）— SDK 引擎唯一新框架依赖：provider client（openai chat completions + 自定义 base_url）与消息/工具类型层；**恒编译、不设 feature gate**（SDK 引擎 = 不依赖本地 claude CLI 的 agent，开箱即跑；v2 起更为默认 agent）
- `reqwest 0.13`（rig 硬依赖，非新增足迹）— desktop 锁内已有 0.13.5，同主版本共享
- `native-tls`（rig 传递）— Windows 走 schannel（锁内已有）；裁定否决 `rustls` feature（避免 aws-lc-sys + cmake 入树，spike 实测 190 vs 146 crates）
- `glob 0.3`（agent-runtime 直连）— glob 工具实现；rig 传递依赖同款已在入树，直连零新增足迹
- `tokio`（既有）— SDK 泵任务 spawn 与 select（rt 特性既有）

### 构建/测试依赖

- `tempfile`（既有 dev-dep）— sandbox / tools 的 tempdir 驱动测试装置
- `cargo fmt` / `cargo clippy --workspace`（既有工具链）— 守线静态检查

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | Cargo.toml 包名与 workspace 键改名、`cli/` 平移、门面 `lib.rs` 重构、壳层消费点切换；`agent-cli` / `agent_cli` 残留归守线阶段 grep 边界检查；验收由 test-execution 阶段承接 |
| AC-2 | `crates/core/agent/src/runner.rs` 唯一加法触碰（`ConfigMissing(String)` + Display）；`engine` / `rig` / `openai` 字样归守线阶段 core 目录 grep 边界检查 |
| AC-3 | `agent_start` 尾部可选 `engine` 参数（IPC 形状不变）、壳层仅参数映射、**缺省收敛 `DEFAULT_ENGINE`（硬编码 Sdk——v2 修订：用户回溯需求，proposal「缺省 cli」表述不再成立，差异留痕见同步状态与待决问题）**、`EngineFacade.runner_for` 分发；bindings v1 已再生成、v2 零变化；`AgentRunParams` 零改动；断言 v1 缺省语义的既有测试修订归 test 阶段 |
| AC-4 | `sdk/normalize.rs`：rig 归一化流 → `AgentEvent`（ToolUse 与 ToolResult 同 id 成对、RunStarted 报 model/tools、`cost_usd = None`、未知透传 Raw、seq 单调）；纯函数，fixture 由 test 阶段驱动、不打网络 |
| AC-5 | `sdk/policy.rs` 三档纯决策表；拒绝产出 `SystemNotice{subtype:"permission_denied"}` + is_error ToolResult（与 tool_use 同 id，run 不中断）；默认 BypassPermissions |
| AC-6 | `sdk/sandbox.rs` canonicalize + workspace root 前缀校验纯函数；`..\` 逃逸与符号链接绕过拦截语义由 tempdir 密集用例覆盖（test 阶段承接） |
| AC-7 | `sdk/resume.rs` 转录重建 + `sdk-` 前缀归属校验；不存在 / 非 SDK 会话 → `AgentStartError::ConfigMissing`（`start()` 阶段失败，不产生 run 记录，`Err` 抵达前端） |
| AC-8 | `sdk/runner.rs` 泵任务 select `RunHandle.wait_requested()`；置位即中止流读取（drop 流/future）、不合成 RunResult，编排侧显式收敛 stopped；编排层（tee 双 sink + 状态机）零引擎分支 |
| AC-9 | `agent-run-form.tsx` 引擎下拉（**v2：初始 `sdk`、与后端默认一致；引擎选择 UI 仅调试页暴露，正式场景无选择入口**）→ `agent-debug-view.tsx` / `use-agent-chat.ts` / `agent-transport.ts` 穿透（缺席透传不注入）→ 后端缺省收敛默认 agent；时间线 / 落库 / 重放组件零改动复用；真机手填 `EngineConfig` 后 sdk 引擎跑通完整 loop 属真机验证（test-execution / 用户验收承接） |
| AC-10 | 本 design「rig-core spike 结论」节留痕实测：feature 形状（无 provider feature，`default-features = false` + `["reqwest","native-tls"]`）、入树体积（新增 12 crate）、API 形状（Tool / streaming / base_url / reasoning_content），design 据实定稿 loop 归属 / pin / reasoning 映射 |

---

## 关键实现语义定稿

1. **SDK loop 轮循环**（`sdk/loop.rs`）：`RunStarted{model: EngineConfig.model, session_id: 生成的 sdk- id, tools: 六工具名, mcp_servers: []}` → 每轮组装 `CompletionRequest`（chat_history + 工具 definitions）→ `stream()` 归一化流 → normalize 增量出事件（Text / Thinking / ToolUse）→ 轮末聚合 assistant 消息回灌 history → 含 tool_calls 时逐个：policy 检查（拒绝 → `SystemNotice{permission_denied}` + is_error ToolResult）→ sandbox 校验（拒绝 → `SystemNotice{subtype:"sandbox_denied"}` + is_error ToolResult）→ 执行（ToolResult 事件）→ 回灌续轮；无 tool_calls 轮 → `RunResult{is_error: false, num_turns: 轮数, duration_ms, cost_usd: None, usage: 可得则填否则 null, session_id}` 收敛。API 错误/重试经 `SystemNotice{subtype:"api_retry"/"api_error"}`（开放枚举）+ `RunResult{is_error: true}` 流出。
2. **停止抹平**（AC-8）：泵任务 `tokio::select!{ 流项, handle.wait_requested() }`——与 CLI `pump_lines` 同构；停止路径不合成 RunResult（编排侧 `stop_requested` 显式收敛 stopped）；消费端关闭（页面关）泵自行退出。SDK 无进程树可杀，取消即 drop future。
3. **resume 语义**（AC-7）：`resume_session_id` 非 `sdk-` 前缀或扫描无命中 → `ConfigMissing("会话不存在或非 SDK 产出: ...")`；转录重建仅取顶层非 Raw 事件（RunStarted 忽略、Message user/assistant 重建、ToolUse/ToolResult 成对回灌）；重建空历史视同会话缺失。loader 为引擎中立参数，CLI 引擎持有不消费（CLI resume 仍走 `--resume` flag）。
4. **错误抹平**：SDK 启动失败全部落 `ConfigMissing(String)`（单一中性变体，消息区分成因）；运行内失败（API 错误等）由 `RunResult.is_error` 表达——启动与运行内失败的分界与 CLI 租户一致。
5. **事件通道**：SDK 泵与 CLI 同为有界 mpsc（容量 256，背压同策略）；`seq` 每 run 从 0 单调（`AgentEvent::stamp`）。
6. **引擎选择 debug-only 与缺省归属后端**（v2 修订）：引擎选择 UI 仅调试页表单（`agent-run-form.tsx` 二值下拉，初始值 `sdk` 与后端默认一致，`cli` 为显式可选项）；正式场景（explore 链）无选择入口、MUST NOT 传 engine。缺省裁决权归后端：`use-agent-chat` body 缺席透传（`?? null`，不注入缺省值）→ transport `readEngine` 缺席 → null → 壳层 `unwrap_or(DEFAULT_ENGINE)`。**explore 链运行时行为随本回溯切换**：默认走 SDK 引擎（v1 为「与演进前一致走 CLI」）——explore 会话自此产出 `sdk-` 会话，其 CLI `--resume` 续话路径不再被 explore 链触达（显式选 cli 的调试运行除外）。
7. **默认 agent 硬编码与可配置化扩展点**（v2 新增）：默认 agent 以 `pub(crate) const DEFAULT_ENGINE: EngineKind = EngineKind::Sdk` 硬编码于壳层组装点 `agent.rs`，与 `EngineConfig::from_hardcoded_slot()` 同点同注释——两硬编码位绑定同一后续配置座位（与 api key 一起改为配置读取），换源时消费面零改动（`mod.rs` 的 `unwrap_or` 消费点与门面 `runner_for` 签名均不动）。常量住 `agent.rs` 而非 `mod.rs` 的理由：与 engine_cfg 硬编码位成对同址，可配置化时单一改动点。SDK 配置未就绪（硬编码位空）时所有未传 engine 的调用（含 explore 链）以 `ConfigMissing` 显式失败，MUST NOT 静默回退 CLI——回退会令「正式场景不允许选择」名存实亡，显式失败即语义。

---

## 待决问题

- **proposal/spec「缺省 cli」文本差异**（v2 新增）：proposal AC-3 与 spec「engine 缺省与分发」scenario 的「缺省 cli」表述与本 design「缺省 `DEFAULT_ENGINE`（硬编码 Sdk/rig）」相悖——用户回溯说明为权威，实现以本 design 为准；proposal/spec 文本同步留待评审确认或提案层补丁（不列入变更清单与任务）。
- **配置落盘座位②vs③**（model/base_url 入 openspec/config.json + api_key 走不入库处 vs 全 app 侧按 root 键控）——顺延后续迭代，不阻塞 MVP；**v2 起该座位同时容纳默认引擎可配置化**：`DEFAULT_ENGINE` 与 api_key / base_url / model 同源改为配置读取，`EngineConfig` 结构体形态与 `unwrap_or` 消费点冻结保证换源零改动。
- **引擎归属留痕的 run 记录层面座位**（run 记录加列 vs `RunStarted` 加字段 vs 不记）——MVP 以 `sdk-` session id 前缀承载归属（本 design 裁定，覆盖 AC-7 的「非 SDK 会话显式失败」），run 记录层面的显式留痕留后续变更偿还；v2 起 explore 链默认产出 `sdk-` 会话，该前缀同时是 explore 链引擎归属的事实标记。
- **rig 归一化流的 usage / finish_reason 可得性**——spike 确认归一化流存在与 `raw_stream` 回退缝，但 `RunResult.usage` 的字段映射细目留实现期核对；若缺失走回退缝，不改变契约形状。
- **openai 兼容端点方言扩充**（tool_calls 形态差异、非标字段）——MVP 首公民锁定一个已实测端点；normalize fixture 按抓样扩充，属 test 阶段素材。
- **交互审批协议**（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）——独立 change，CLI 租户将来同样受益；三档表中 bash 拒绝语义为将来引入 bash 时的预留档位。
