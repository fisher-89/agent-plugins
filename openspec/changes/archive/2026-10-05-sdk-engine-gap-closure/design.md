# 设计: sdk-engine-gap-closure

> **变更**: sdk-engine-gap-closure
> **日期**: 2026-10-04

---

## 提案与规格同步状态（信息性）

- 本变更的两个能力 spec delta（`desktop-agent-execution`、`desktop-agent-management`）已随提案阶段写入本变更目录的 `specs/`，属提案层产物，**不进变更清单与任务**。
- 阶段化（拍板 #14）：facade 切换为第一阶段先行，机械、可独立验证；后续全部工作落在新依赖上。
- 拆分触发器（沿用探索定稿）：T1 = facade re-export 面/memory feature 验证失败 → backtrack 剔除 facade 项（退路 rig-memory 直连或内抄算法）；T2 = compaction 实现失控 → backtrack 缩范围，L1/L2 先行。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| workspace 依赖面 | `rig` facade 条目（`=0.42.0`、`default-features = false`、features `reqwest`/`native-tls`/`memory`）+ `regex` 条目；`rig-core` 条目保留为解析钉子 | `packages/desktop/src-tauri/Cargo.toml` | crates.io | cargo workspace.dependencies |
| crate 依赖声明 | agent-runtime 消费 `rig` + `regex`（`rig-core` 声明保留，双条目钉死） | `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` | workspace 依赖面 | cargo |
| facade 路径迁移（loop） | `rig_core::` → `rig::` 迁移 + preamble 接入 + 防线调用点编排 + 三处「CLI 口径」注释善后 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | rig、preamble、context、compact | rig completion/streaming/message |
| facade 路径迁移（normalize） | 纯路径迁移（流项归一化逻辑零改动） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` | rig | rig message/streaming |
| facade 路径迁移（resume） | 纯路径迁移（转录重建逻辑零改动） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` | rig | rig message |
| facade 路径迁移（runner） | 路径迁移 + `SdkRunner::new` 增窗长参数 + resume 重建后防线调用点 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | rig、context、facade | rig providers/client |
| facade 路径迁移（tools） | 路径迁移 + 七工具面 + read/grep 质量 + L1 上限收口 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | rig、regex、bash | rig ToolDefinition |
| AGENT.md preamble 读取 | 起播前/每轮读 `<workspace_root>/AGENT.md`，存在逐字注入、缺席 `None`、无兜底、32KB 截断 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs`（新） | tokio::fs | 纯函数 + 异步读 |
| bash 执行体（第七工具） | 进程执行 + shell 底座探测（纯函数）+ 超时 + 进程树清理 + 双管道输出合并 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs`（新） | tokio（process feature 已在树） | `tokio::process` + `taskkill /T /F` 形状 |
| 三档权限决策表 | bash 行为变更：Default 拒 / AcceptEdits 与 Bypass 放（两档首次真实分化）；模块头知情边界措辞改写 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | agent（core 契约枚举） | 纯决策表 |
| L2 上下文防线 | 窗长解析 + 字节启发式 token 估算 + prune（占位符/配对完整/保首条 user/保护窗）+ 轮对裁剪 + 硬裁 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`（新） | rig Message | 纯函数 |
| L3 compaction | 当前 model 自摘要老历史（中文要点 prompt 含决策及原因）+ 一次重试 + 失败交降级路径 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact.rs`（新） | rig、context | rig CompletionModel |
| 引擎门面接线缝 | `EngineFacade` 增 `context_window` 载荷与 `with_context_window` builder，`runner_for` 签名不动 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | agent-runtime 内部 | builder 缝（`with_resume_transcript` 同型） |
| 组合根接线 | `resolve_agent_engine` 读 provider `context_length` → `ResolvedEngine` → facade 构造表达式（AC-9 接线唯一 compose 触点） | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | store 全局库、facade | 组合根旁路（不经 `EngineConfig`） |
| provider 记录加列 | `AgentProviderRecord.context_length: Option<u64>` + native_model v1→2 演进（V1 legacy + From 双向） | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | native_db/native_model | envelope 版本链（v3→v4 先例同型） |
| provider CRUD 命令面 | `save_agent_provider` 携带 `context_length`（留空 = `None`，MUST NOT 落 0/128000 字面） | `packages/desktop/src-tauri/src/commands/agents/mod.rs` | store、tauri-specta | 既有平参命令模板 |
| 管理页取数 hook | save 入参增 `contextLength: number \| null` 透传 | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | bindings | 既有取数 hooks 形态 |
| 管理页表单 | Providers 栏表单增可空数字字段（留空 = 未配置 + 缺省启发式语义提示） | `packages/desktop/src/views/agents/components/provider-panel.tsx` | hook、bindings | 既有 config-driven 表单 |
| bindings 镜像 | 管理 DTO 演进经 export-bindings 再生成（TS 类型 + Rust 快照测试），非手改 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src-tauri/src/bindings/mod_test.rs` | export-bindings crate | `pnpm bindings:export` 再生成 |

路径沙箱（`sdk/sandbox.rs`）、引擎配置结构（`sdk/config.rs`）、CLI 引擎（`crates/infra/agent/src/cli/**`）、core 内核（`crates/core/agent/**`）：本变更零 diff（`config.rs` 虽在提案迁移清单内，实际无 `rig_core::` 触点）。

---

## 设计定值（design 拍板）

| 项 | 定值 | 依据 |
|----|------|------|
| facade 条目 | `rig = { version = "=0.42.0", default-features = false, features = ["reqwest", "native-tls", "memory"] }` | crates.io 事实核查：rig 0.42.0 的 `memory` feature = `["dep:rig-memory"]`；`reqwest`/`native-tls` 转发 rig-core 同名 feature；`rig::completion/message/streaming/providers/client` 均为既有 `rig_core::` 路径的同名 re-export，`rig::memory` 模块暴露 rig-memory 政策层 |
| pin 策略 | **双条目钉死**：`rig-core = "=0.42.0"` 既有条目退役为解析钉子（agent-runtime 继续声明，代码路径零 `rig_core::`） | facade 对 rig-core 的需求为 `^0.42.0`（crates.io 事实核查），单条目下 `cargo update` 可浮动 rig-core 至 0.42.x 最新，违反本仓 0.x 线 `=` pin 纪律；双条目强制 resolver 统一在 0.42.0。提案「删除文件」节的条件性删除**不触发** |
| regex 引入 | workspace `regex = "1"` + agent-runtime 直依 | 锁内已有 regex 1.13.1（rig-core 传递依赖），直连零新增包足迹（glob/futures 先例同款） |
| `context_length` 接线路径 | **组合根旁路**：compose 解析 provider 记录 → `ResolvedEngine.context_window` → `EngineFacade::with_context_window(Option<u64>)` → `SdkRunner::new` 第三参 → `ContextDefense::resolve`。`EngineConfig` 三字段、`runner_for` 签名、`SessionCtx` 均不动 | 三候选中「ctx 可扩展位」撞 core 零 diff 承诺、「引擎自查 provider 记录」不可行（引擎持有的 store port 绑 workspace 库，provider 在全局库）；facade 构造缝与既有 `with_resume_transcript` 完全同型。compose.rs 的 diff 仅限接线（见待决问题 #1） |
| L2 水位 / L3 水位 | 75% / 90% | proposal 给定区间（70-75%、85-90%）取上沿；余量覆盖响应 token 与估算误差 |
| prune 保护窗 / 单结果 prune 门槛 | 40k tokens / 20k tokens | opencode 参考值（探索 §4/§5）；只裁保护窗外、单体超门槛的老工具结果 |
| token 估算 | `estimate_tokens` = rig Message 的 `serde_json` 序列化 UTF-8 字节数 ÷ 4（向上取整）；自研纯函数，常数口径对齐 rig-memory `HeuristicTokenCounter`，rig::memory 不进 loop 消费面 | 序列化字节数含结构开销 = 保守高估，宁早裁不触 L4；不用其容器契约（探索 §5 拍板：loop 自持 `Vec<Message>`） |
| 缺省窗长 | `DEFAULT_CONTEXT_WINDOW = 128 * 1024`（`context_length` 缺席时） | proposal 拍板 #3 |
| read 截断 | `MAX_READ_LINES = 2000`（缺省与显式 limit 同上限），截断尾部留痕「已截断，可用 offset 翻页」形态 | proposal 待决区间定值；边界用例恰 2000 过 / 2001 截 |
| L1 单结果字节上限 | `MAX_RESULT_BYTES = 30_000`，`tools::execute` 统一收口，Ok/Err 双路截断留痕；bash 输出同层 | claude code 30k 口径；单一收口点保证全工具（含 bash）覆盖 |
| grep 语义 | `regex` 正则匹配（非法正则 → `Err`）；新增可选 `context` 参数（匹配行 ± N 行，缺省 0；窗口合并去重、不连续组间以 `--` 分隔）；200 条命中上限维持 | proposal AC-3；子串字面匹配退役 |
| bash 超时 | `timeout_ms` 可选入参，钳位 `[1_000, 600_000]` ms，缺省 `120_000` | claude code 120s 缺省 / 600s 上限口径；模型可申请长超时，活性护栏封顶 |
| bash shell 底座 | Windows：纯函数 `probe_windows_shell` 扫 PATH 中 `Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态条目（排除 System32 的 WSL bash 同名误中）→ `bash -c`；未命中退 `cmd /C`；unix：`sh -c` | 拍板 #10；探测函数纯化可测（proposal 场景要求） |
| bash 进程语义 | `kill_on_drop(true)`；超时/终止时进程树清理复用 `cli/runner.rs` `kill_process_tree` 的 `taskkill /PID <pid> /T /F` + `CREATE_NO_WINDOW` 形状（形状复制，`cli/` 零 diff）；stdin 置空、env 继承、`cwd = workspace root`；stdout/stderr 并发读取，stdout 段在前、stderr 段带标记在后拼接 | proposal AC-4；非零退出码 → `Err(退出码 + 输出)`（is_error ToolResult） |
| AGENT.md 体量 | `MAX_PREAMBLE_BYTES = 32 * 1024`，超限截前 32KB（char 边界）；严格单文件、不级联嵌套目录；读取失败（含非 NotFound IO 错误）一律 `None`（尽力增强，不炸 run） | proposal 待决项定值；逐字语义指「注入内容即文件前缀原文，不包装」 |
| 摘要 prompt | 中文；指令含「这是历史摘要，请勿重复已完成的工作」前缀形态 + 要点集：已完成工作与产物、关键技术决策**及其原因**、进行中的工作、下一步、重要约束；摘要以带 `[历史摘要]` 头的 user 消息置顶；失败**一次重试**，仍失败降级 L2 硬裁 | opencode 要点集 + codex 防循环教训（探索 §4）；一次重试是成本与密度的折中，无指数退避 |
| 降级 SystemNotice | `SystemNotice{subtype:"context_compacted", payload:{before, after, layer:"l3", fallback:true}}`；常规剪裁 `layer:"l2"`、常规压缩 `layer:"l3"`；before/after 为估算 tokens | AC-8 载荷形状；开放词典现成 |
| 防线调用点 | L2 两个调用点：runner 泵内 resume 重建后（notice 先于 `RunStarted` 流出，属重建史防线留痕）+ loop 每请求前；L3 仅 loop 请求前路径触发（需 model 引用） | proposal AC-6 两调用点可考；泵先发 notice 再进 loop，事件序确定 |
| L4 错误细分 | **不做** `prompt_too_large` 细分，维持泛化 `api_error` 记因（错误原文进 payload） | 各 provider 400 文案不一，文本嗅探脆弱；极限场景（首条 user 单块超窗 / 单轮即超窗）如实留 L4 兜底，用户引导靠管理页 `context_length` 字段的缺省语义提示 |
| 窗长承接 | `LoopTurn` 增 `defense: ContextDefense`（runner 侧已解析缺省），引擎内部字段 | `EngineConfig`/core 契约零 diff 承诺下的引擎内部通道 |
| 上下文防线文件命名 | `sdk/context.rs`（窗长/估算/L2）+ `sdk/compact.rs`（L3）；提案待决的「loop.rs 三处 CLI 口径注释」在阶段一路径迁移时顺手改为 core 协议口径 | proposal 待决问题收口 |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。标注「清单外补入」的文件为提案实现文件之外的级联触点，均附理由。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs` | AGENT.md preamble 读取：`load` 纯读单文件（逐字 / 缺席 None / 无兜底 / 32KB 截断 / IO 失败 None），每轮重读由 loop 调用点保证 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` | bash 第七工具执行体：`execute`（tokio::process + 超时 + 进程树清理 + 输出合并）、`probe_windows_shell`（PATH 纯探测）、`kill_process_tree`（`taskkill /T /F` 形状复制） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs` | 上下文防线 L2：`ContextDefense` 窗长解析与阈值、字节启发式 token 估算、prune（占位符替换 / 配对完整 / 保首条 user / 保护窗 / 轮对裁剪 / 孤儿清扫）、`hard_prune`（L3 失败降级用） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact.rs` | 上下文防线 L3：`summarize` 以当前 model 自摘要老历史（中文要点 prompt + 一次重试），产出 `[摘要, 首条 user, 最近轮对]` 新史 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 新增 `rig` facade 条目与 `regex = "1"` 条目；`rig-core` 既有条目保留、注释改写为解析钉子角色 | facade 切换 + 锁 `=0.42.0` 平移不升版；提案「删除文件」节的条件性删除不触发（见设计定值 pin 策略） |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（清单外补入） | `[dependencies]` 新增 `rig = { workspace = true }`、`regex = { workspace = true }`；`rig-core` 声明保留 | 级联触点：workspace 条目须被 crate 消费方声明才生效；rig-core 声明是钉子的力学来源（无代码路径使用） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | `rig_core::` → `rig::`；请求构造接入 `preamble::load(&turn.cwd)` 每轮重读；每次发请求前 L2 prune + L3 触发/摘要/降级编排与 notice 转发；`LoopTurn` 增 `defense: ContextDefense`；三处「CLI 口径」注释改 core 协议口径 | 引擎内部特性全落本文件；core 协议事件词汇零改动（`SystemNotice` 开放 subtype） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs` | `rig_core::` → `rig::`（use 路径） | 纯迁移，归一化逻辑零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs` | `rig_core::` → `rig::`（use 路径） | 纯迁移，重建逻辑零改动（剪裁不动重建源） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `rig_core::` → `rig::`；`SdkRunner::new` 增 `context_window: Option<u64>` 参数；泵内装配 `ContextDefense` 并在 resume 重建后执行 L2 prune（notice 先于 `RunStarted` 转发） | AC-6 第一调用点；`open_session` 校验面不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | `TOOL_NAMES` 扩至七工具；read 截断 + offset 翻页 + 尾部留痕；grep 正则 + 上下文行（regex crate）；`execute` 统一 L1 字节上限收口；bash 分发接入；模块头「bash 不进 MVP 工具面」措辞改写 | AC-3 / AC-5 / AC-4 工具面半边 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | 新增 bash 行：Default 拒、AcceptEdits 与 BypassPermissions 放（新增执行面清单常量）；模块头改写为三档 + bash 知情边界措辞 | AcceptEdits ≠ Bypass 首次真实分化；签名不变、行为变更 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（清单外补入） | 注册 `bash` / `context` / `compact` / `preamble` 四个产品模块 | 级联触点：新模块须挂载 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`、`normalize_test.rs`、`resume_test.rs` | `rig_core::` → `rig::` 路径迁移 | 提案「测试文件」节回归面中属编译必需的迁移部分（仅路径，断言语义翻转归测试工作流阶段） |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（清单外补入） | `EngineFacade` 增 `context_window: Option<u64>` 字段与 `with_context_window` builder；`runner_for` 的 Sdk 臂传参 `SdkRunner::new` | `runner_for` 签名不动；builder 与 `with_resume_transcript` 同型 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（清单外补入） | `ResolvedEngine` 增 `context_window: Option<u64>`；`resolve_agent_engine` 的 Sdk 臂读 `provider.context_length`；facade 构造表达式串接 `with_context_window` | AC-9 接线唯一 compose 触点；`SessionInjections` 恒传 default 行零改动（见待决问题 #1 的 AC-2 口径） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `AgentProviderRecord` 增 `context_length: Option<u64>`（`#[serde(default)]`）；native_model `version = 1` → `2` + `V1` legacy 结构与 `From` 双向 impl（v3→v4 先例同型）；`AgentProviderRecord::new` 增参；手写 masked `Debug` impl 补新字段（字段清单断言对齐） | 可空列 + envelope 版本演进沿既有惯例，不做 legacy 迁移层；envelope 查看器扫描整记录 serde 序列化，新字段自动携带（`envelope.rs` 零 diff） |
| `packages/desktop/src-tauri/src/commands/agents/mod.rs` | `save_agent_provider` 增 `context_length: Option<u64>` 平参并传入记录构造 | 留空 = `None` 贯穿存储层；`Result<T, String>` 模板不变；`list`/`delete` 原样携带整记录零改动 |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | `AgentProviderSaveInput` 增 `contextLength: number \| null`；save 调用透传 | 取数 hooks 形态不变、无轮询不变 |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | `ProviderFormState` 增 `contextLength: string`（空串 = 未配置，规避既有 string-only handler 的加宽）；`PROVIDER_FIELDS` 增行；`toFormState` 映射（null → 空串）；保存 parse（空 → null、非正整数拒绝提交）；「跟随缺省 128K」语义提示 | 前端以 string 承载数字输入，保存边界单点 parse，`TextField` 复用不动 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 经 export-bindings 再生成（管理 DTO 演进的 Rust 侧快照镜像） | 非手改 |
| `packages/desktop/src/types/generated/bindings.ts`（清单外补入·生成物） | 经 export-bindings 再生成（`AgentProviderRecord.contextLength` + `saveAgentProvider` 签名） | 级联触点：`pnpm bindings:export` 产物，非手改 |
| `packages/desktop/package.json` | `version` 0.4.7 → 0.4.8 | AC-11，归档时执行（用户可见变更：SDK 引擎能力补全） |

<!-- 提案迁移清单中的 sdk/config.rs 实际无 rig_core:: 触点，本变更零 diff，不列修改行。提案「删除文件」节的条件性 rig-core 条目删除不触发（双条目钉死决策），故无删除文件子节。policy_test.rs / tools_test.rs 两条 bash 旧语义锁定断言的翻转为测试语义变更，归测试工作流阶段承接，不属实现清单。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `load` | `…/sdk/preamble.rs` | 新增 | `pub(crate) async fn load(root: &Path) -> Option<String>` | 读 `<root>/AGENT.md`：存在逐字（超 32KB 截前缀）、缺席或读取失败 `None`；每轮调用即每轮重读 |
| `execute` | `…/sdk/bash.rs` | 新增 | `pub(crate) async fn execute(root: &Path, input: &Value) -> Result<String, String>` | bash 执行体：探测 shell 底座 → `tokio::process` 执行 → 超时收口 → 进程树清理 → stdout/stderr 合并返回；非零退出码 `Err` |
| `probe_windows_shell` | `…/sdk/bash.rs` | 新增 | `#[cfg(windows)] pub(crate) fn probe_windows_shell(path_var: &OsStr) -> ShellBase` | PATH 纯探测（`Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态），未命中退 `Cmd`；fabricated PATH 可测 |
| `kill_process_tree` | `…/sdk/bash.rs` | 新增 | `fn kill_process_tree(pid: Option<u32>)`（模块私有） | `taskkill /PID <pid> /T /F` + `CREATE_NO_WINDOW` 形状（unix `kill -9`），仅复用 `cli/runner.rs` 形状、`cli/` 零 diff |
| `TOOL_NAMES` | `…/sdk/tools.rs` | 修改 | `pub const TOOL_NAMES: [&str; 7]` | 追加 `"bash"`；`RunStarted.tools` 恰七工具 |
| `definitions` | `…/sdk/tools.rs` | 修改 | `pub fn definitions() -> Vec<ToolDefinition>` | read schema 增 offset/limit 描述不变 + 截断语义描述；grep 增 `context` 参数、pattern 描述改正则；新增 bash 条目（`command` 必填、`timeout_ms` 可选） |
| `execute` | `…/sdk/tools.rs` | 修改 | `pub async fn execute(root: &Path, name: &str, input: &Value) -> Result<String, String>` | 分发增 `"bash"` 臂；结果统一经 L1 字节上限收口（Ok/Err 双路） |
| `allows` | `…/sdk/policy.rs` | 修改 | `pub fn allows(mode: AgentPermissionMode, tool: &str) -> bool` | bash 行为变更：Default 拒、AcceptEdits / BypassPermissions 放；签名不变 |
| `ContextDefense::resolve` | `…/sdk/context.rs` | 新增 | `pub(crate) fn resolve(context_window: Option<u64>) -> ContextDefense` | 缺席走 128K 缺省；装配阈值与保护窗常量 |
| `estimate_history` | `…/sdk/context.rs` | 新增 | `pub(crate) fn estimate_history(history: &[Message]) -> u64` | 各 Message serde_json 字节 ÷ 4 求和（字节启发式） |
| `prune` | `…/sdk/context.rs` | 新增 | `pub(crate) fn prune(history: Vec<Message>, defense: &ContextDefense) -> (Vec<Message>, Vec<DefenseNotice>)` | L2：占位符替换老 tool_result（配对完整）→ 不够丢最老完整轮对；保首条 user、保护最近 40k、单体 >20k 才裁；产出 `context_pruned` notice |
| `hard_prune` | `…/sdk/context.rs` | 新增 | `pub(crate) fn hard_prune(history: Vec<Message>, defense: &ContextDefense) -> (Vec<Message>, DefenseNotice)` | L3 失败降级：保首条 user + 保护窗，中间整段丢弃 |
| `summarize` | `…/sdk/compact.rs` | 新增 | `pub(crate) async fn summarize<M: CompletionModel>(model: &M, history: &[Message], defense: &ContextDefense) -> Result<Vec<Message>, String>` | L3：摘要请求（禁工具、一次重试）；成功返回 `[摘要, 首条 user, 保护窗]` 新史，失败 `Err` 交 loop 降级 |
| `SdkRunner::new` | `…/sdk/runner.rs` | 修改 | `pub fn new(config: EngineConfig, resume: Option<ResumeTranscript>, context_window: Option<u64>) -> Self` | 第三参承接组合根旁路的窗长载荷 |
| `with_context_window` | `…/src/lib.rs` | 新增 | `pub fn with_context_window(self, context_window: Option<u64>) -> EngineFacade` | builder 缝；`new()`/`with_resume_transcript` 缺省 `None` 不变 |
| `AgentProviderRecord::new` | `…/crates/infra/store/src/model.rs` | 修改 | `pub fn new(name: String, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>) -> Self` | 五参构造；`None` = 未配置语义入存储层 |
| `save_agent_provider` | `…/src-tauri/src/commands/agents/mod.rs` | 修改 | `pub fn save_agent_provider(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>) -> Result<AgentProviderRecord, String>` | 留空提交 = `None`；api_key 留空保持原值语义不变 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ShellBase` | `…/sdk/bash.rs` | 新增 | `enum ShellBase { GitBash(PathBuf), Cmd, Sh }`——shell 底座探测产物 |
| `ContextDefense` | `…/sdk/context.rs` | 新增 | 窗长 + L2/L3 触发比（0.75 / 0.90）+ 保护窗 40k + prune 门槛 20k 的防线配置（引擎内部，不入 core） |
| `DefenseNotice` | `…/sdk/context.rs` | 新增 | `{ subtype: String, payload: Value }`——防线产出的 `SystemNotice` 前体（`context_pruned` / `context_compacted`） |
| `LoopTurn` | `…/sdk/loop.rs` | 修改 | 增 `defense: ContextDefense` 字段（引擎内部轮参数投影） |
| `AgentProviderRecord` | `…/crates/infra/store/src/model.rs` | 修改 | 增 `context_length: Option<u64>`；native_model `(id = 5, version = 2)`；masked `Debug` 字段对齐 |
| `AgentProviderRecordV1` | `…/crates/infra/store/src/model.rs` | 新增 | `pub(crate)` legacy 结构（五字段，`version = 1`），`From` 双向 upgrade/downgrade 承接旧库读 |
| `EngineFacade` | `…/src/lib.rs` | 修改 | 增 `context_window: Option<u64>` 载荷字段与 builder；`runner_for` 签名不动 |
| `ResolvedEngine` | `…/src/compose.rs` | 修改 | 增 `context_window: Option<u64>`（组合根内部结构） |
| `ProviderFormState` | `…/views/agents/components/provider-panel.tsx` | 修改 | 增 `contextLength: string`（表单 string 承载，保存边界 parse） |
| `AgentProviderSaveInput` | `…/views/agents/hooks/use-agent-providers.ts` | 修改 | 增 `contextLength: number \| null` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.4.8"` | AC-11，归档时执行 0.4.7 → 0.4.8 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `AgentProviderRecord`（v2） | 既有：`id`(PK)、`name`(唯一)、`base_url`、`api_key`、`models: AgentModelTiers`；新增：`context_length: Option<u64>`（token 数；`None` = 未配置） | 被 `AgentInstanceRecord.provider_id` 引用（删被引用 provider 阻止，既有语义不变） | 全局 native_db 库，`native_model(id = 5, version = 2)`，bincode 默认 codec；旧记录经 `AgentProviderRecordV1` + `From` 链 upgrade 读入（`context_length = None`），无 legacy 迁移层 |
| `AgentProviderRecordV1`（legacy） | 既有五字段原形，`native_model(id = 5, version = 1)` | 仅作 envelope 版本链升级源 | 不落新库；仅旧 payload 解码中转 |
| 防线 `SystemNotice`（密封事件） | `payload = { before: u64, after: u64, layer: "l2"\|"l3", fallback?: true }`（`context_pruned` 无 fallback） | 挂 session 事件流，随密封事件 write-through 落库 | `SessionEventRecord` 既有通道；store 转录全量不变（剪裁/压缩只作用请求史），delta 零落库纪律不破 |

---

## 依赖

### 运行时依赖

- `rig = "=0.42.0"`（`default-features = false`，features `reqwest` + `native-tls` + `memory`）— rig-core 的 re-export 门面：provider client、消息/工具/流类型层；`memory` feature 拉 rig-memory 算法层入树（家族能力开洞，MCP 将来同法开 `rmcp`）
- `rig-core = "=0.42.0"`（既有条目保留）— 解析钉子：facade 对 rig-core 为 `^0.42.0` 需求，双条目强制锁 0.42.0（无代码路径使用）
- `regex = "1"` — grep 正则匹配（锁内 1.13.1 既有传递依赖，直连零新增包足迹）
- `tokio`（process feature）— bash 进程执行（agent-runtime 既有声明，零新增）

### 构建/测试依赖

- 无新增 — `tempfile` / `native_db` / `tokio` rt+macros 等既有 dev-dependencies 覆盖新模块测试需求

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | workspace `rig` facade 条目（`=0.42.0`、`default-features = false` + 显式 features）+ 五产品文件/三测试文件路径迁移；`rig_core::` 零残留由守线 grep 静态核验；proposal 的工作区测试全绿过关线由 test-execution 阶段核验（本设计守线仅静态） |
| AC-2 | `preamble.rs` + loop 每轮接入：在场逐字 / 缺席 None / 每轮重读 / 无兜底；`crates/core/agent` 与 `cli/` 零 diff；compose 面按 preamble 机制口径零触碰（compose.rs 唯一 diff 属 AC-9 接线，见待决问题 #1） |
| AC-3 | `tools.rs` read 截断（2000 行 + 尾部留痕 + offset/limit 翻页）与 grep 正则 + `context` 上下文行；边界（恰阈值过 / 超 1 截断）归测试用例 |
| AC-4 | `bash.rs` 执行体（合并回灌 / 超时收口 / 进程树清理 / `cmd /C` 兜底探测）+ `policy.rs` 三档矩阵 + `TOOL_NAMES` 七工具（`RunStarted.tools` 恰七） |
| AC-5 | `tools::execute` 统一 30KB 单结果字节上限（Ok/Err 双路），bash 输出同层截断留痕 |
| AC-6 | `context.rs` prune（占位符 + 配对完整 + 保首条 user + 40k 保护窗 + 轮对裁剪）+ runner resume 后与 loop 每请求前两调用点；store 转录全量不变（防线只作用请求史） |
| AC-7 | `compact.rs` 摘要（新史 = [摘要, 首条 user, 最近轮对]）+ 失败一次重试后降级 `hard_prune`，run 不失败收敛 |
| AC-8 | `DefenseNotice` → `SystemNotice{context_pruned\|context_compacted, payload:{before, after, layer}}` 经既有密封通道流出落库；非 delta 事件，零落库纪律不破 |
| AC-9 | store 可空列（v2 envelope + V1 缺列读兼容）→ facade/compose 接线（不经 `EngineConfig`）→ `ContextDefense::resolve` 缺省 128K → 命令面与管理页编辑（留空 = 未配置）→ bindings 再生成 |
| AC-10 | spec 边界留痕已随提案写入（「bash 无沙箱」知情边界条款）；实现面落点为 `policy.rs` 决策表与模块头措辞改写；`policy_test` / `tools_test` 两条锁定断言翻转归测试工作流阶段承接 |
| AC-11 | `packages/desktop/package.json` version 0.4.7 → 0.4.8，归档时执行（任务列表交付阶段承载） |

---

## 待决问题

- **AC-2「compose 面零 diff」的核验口径**：`context_length` 接线（AC-9）在冻结面（`EngineConfig` 三字段 / `runner_for` 签名 / `SessionCtx`）约束下唯一可行路径穿越 compose.rs（`resolve_agent_engine` 携带 + facade 构造表达式，约 3 行）。本设计将 AC-2 的该句读作「preamble 机制不触及 core 与 compose」——preamble 自身实现零 compose 触碰；若验收按文件级零 diff 字面核验则两 AC 相互矛盾，需评审确认口径。
- **rig::memory item 面复用窗**：token 估算已定自研纯函数（serde_json 字节 ÷ 4）；实现期若确认 `rig::memory` 暴露可纯函数复用的计数器，允许单点替换 import（常数对齐），不改变防线其余结构。facade 条目 / 路径迁移本身已由 crates.io 事实核查消除 feature 名与 re-export 路径的不确定性，剩余 spike 范围仅此 item 级确认。
- **阈值常数的实现期微调窗**：75% / 90% / 40k / 20k / 32KB / 30KB / 2000 行 / 120s-600s 为 design 拍板值；若实现期发现中文重载估算偏差导致过早/过晚触发，允许在常数层微调（结构不动），偏差证据留实现记录。
- **摘要 prompt 措辞定稿**：语言（中文）、前缀形态（勿重复已完成工作）、要点集框架已定；实现期允许措辞打磨，要点集不减。
- **Windows 探测覆盖率**：`probe_windows_shell` 以 PATH 扫描 `Git\bin|usr\bin\bash.exe` 形态为准，未覆盖非标准安装（如 scoop shim 指向）；此类环境退 `cmd /C` 兜底，开箱即跑不破坏（覆盖率与兜底语义的权衡已知情，不再扩探测面）。
