# desktop-agent-execution Specification (Delta)

## ADDED Requirements

### Requirement: SDK 引擎模型协议选臂（openai / anthropic）

SDK 引擎 SHALL 支持两种模型协议并按 provider 记录声明选臂：`openai`（openai-compatible 端点，现有唯一形态）与 `anthropic`（Anthropic Messages 端点及其兼容网关）。协议 SHALL 为 provider 记录属性（`AgentProviderProtocol`），经组合根映射为门面 `Protocol` 后进 `EngineConfig.protocol`（映射沿 `AgentEngineKind` → `EngineKind` 双枚举先例，两臂 match），MUST NOT 编码进 `EngineKind`（协议是 provider 语义，不是租户语义；`EngineKind` 双值与 `runner_for` 唯一 match 点 MUST NOT 塌）。

模型组装点（`build_model`）SHALL 为协议唯一分叉处，两臂 MUST 同产 `DynModel<Completion>`（`rig::operation::Completion` 擦除型），使 `sdk/loop.rs` / `normalize.rs` / `resume.rs` / `crates/core/agent` 全链**协议中立、零改动**：

- **openai 臂**：`OpenAIConfig::new(api_key).with_base_url(base_url).client().chat(model).erase()`——与现状逐字等价，MUST NOT 追加任何请求参数（`max_tokens` 保持省略，各网关对超限行为不一）；
- **anthropic 臂**：`AnthropicConfig::new(api_key).with_base_url(base_url)`[条件 `.with_beta(..)`]`.client().completion(model)`，定型 wire 默认 `max_tokens`（取值链见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）后 `.erase()`；`AnthropicConfig` SHALL 经 `rig::providers::anthropic` 直接引用（rig 0.43 该模块**无 feature gate**，MUST NOT 为此新增 crate feature）。

协议差异 SHALL 全部由 rig 吸收，引擎 MUST NOT 自拼协议头或改写端点路径：

- **认证头**：openai `Authorization: Bearer`；anthropic `x-api-key` + `anthropic-version`（rig 内建），引擎零手写；
- **`base_url` 语义**：openai 原样拼 `/chat/completions`（地址须含版本段，如 `https://…/v1`）；anthropic 由 rig 先剥 `/v1`、`/messages`、`/v1/messages` 后缀、再拼 `/v1/messages`——用户填 `https://api.anthropic.com` 与 `https://api.anthropic.com/v1` SHALL 等价可用（MUST NOT 产生 `/v1/messages/v1/messages` 重复段）。

诊断面：引擎泵启动日志 SHALL 记 `protocol=…`（同一 `base_url` 配错协议时可一眼定位）与 `model=裸名 window=… raw=原文`；`RunStarted.model` SHALL 报**裸名**（反映实际发送），协议本身 MUST NOT 在事件面新增字段或变体（词汇不扩，前端适配层零改）。

#### Scenario: openai 臂逐字等价

- **WHEN** `protocol = openai` 的 provider 发起运行，审查 `build_model` 产物描述面
- **THEN** 得到的 `DynModel<Completion>` 的 `name()` 为 `openai`、`id()` 为模型名，与变更前的组装路径逐字等价；no 请求参数被追加（`max_tokens` 仍省略）

#### Scenario: anthropic 臂组装

- **WHEN** `protocol = anthropic` 的 provider 发起运行（模型名为裸名）
- **THEN** 得到的 `DynModel<Completion>` 的 `name()` 为 `anthropic`、`id()` 为裸模型名；请求带 `x-api-key` + `anthropic-version` 头（rig 内建，引擎零手写）；组装全程不打网络（仅描述面断言）

#### Scenario: loop 协议中立

- **WHEN** 以两臂产物分别驱动同一 `sdk/loop.rs` 会话
- **THEN** loop / normalize / resume / core 契约零改动可消费（两臂同为 `DynModel<Completion>`，无协议分支）；`AgentEvent` 词汇与变体集不变

#### Scenario: base_url 归一化

- **WHEN** anthropic 协议下 `base_url` 填 `https://api.anthropic.com/v1`（或带 `/v1/messages` 后缀）
- **THEN** rig 归一化后实际请求 `/v1/messages` 单段版本路径，不出现重复版本段；填 `https://api.anthropic.com` 与填带后缀形态等价可用

#### Scenario: 协议进配置不进 kind

- **WHEN** 审查 `EngineConfig`、`EngineKind` 与 `runner_for` 实现
- **THEN** 协议以 `EngineConfig.protocol` 携带（门面 `Protocol` 双值），`EngineKind` 仍为 `cli` | `sdk` 双值、`runner_for` 唯一 match 点结构与签名形态不塌；core 侧无协议字样

### Requirement: SDK 引擎模型名 `[1m]` 后缀与输出上限定档

SDK 引擎 SHALL 支持 provider 三档模型名携带 `<模型名>[1m]` 后缀（对齐 Claude Code 配置形态），并据此定档上下文窗长与 anthropic 请求参数。解析 SHALL 收在**纯函数单点** `parse_model_spec(raw) -> Result<ModelSpec{name, length: Option<u64>}, ModelSpecError>`（`agent-runtime` 公开导出，命令层保存期校验复用同一函数，MUST NOT 造第二套规则）：

- 末尾 `[1m]`（大小写不敏感）→ 剥离出裸名、`length = 1_000_000`（**1000 进制**：`1m = 1_000_000`）；
- **其余方括号形态一律显式拒收**：`[2m]`（无 2M 窗长常量，语义悬空）、`[8k]` / `[200k]`（本变更不做长度单位族）、`[]` / `[ 1m ]` 内空白、方括号不在末尾 → `ModelSpecError`；
- 无后缀 → `name = trimmed(raw)`、`length = None`；
- 两端空白 SHALL 裁剪；MUST NOT 把带后缀的原文发给 provider（请求 `model` 恒为裸名）。

窗长定档 SHALL 为**按档现算**（`high` / `low` 各自独立）：优先级 **模型名后缀 > provider 可空 `context_length` > 128K 缺省启发式**；后缀在场时 MUST NOT 忽略地叠加或夹取（后缀即该档窗长，`context_length` 被覆盖）。解析与防线装配 SHALL 发生在 `open_session`（那里同时拿到档位与模型原文），`EngineFacade::with_context_window` 签名 MUST NOT 变化——其语义降级为「缺省来源」。非法后缀在启动期 SHALL 以 `AgentStartError::ConfigMissing` 显式失败（MUST NOT 静默降级为裸文发送）。

anthropic 请求参数 SHALL 按以下规则定档：

- **`anthropic-beta`**：`protocol = anthropic` 且该档模型名带 `[1m]` → SHALL 追加 beta `context-1m-2025-08-07`（完全等价 Claude Code 的 1M 开关）；openai 协议或后缀缺席 → MUST NOT 发。
- **输出上限（`max_tokens`）取值链**：`provider.max_tokens`（显式值优先）→ rig 内置模型名单官方值（名单命中时**保留 rig 值、不被降级**）→ `32_000` 兜底（对齐 Claude Code 表外行为）。实现 SHALL 只在该链产出非 `None` 时定型 wire 默认 `max_tokens`（`Messages::with_default_max_tokens`），且**只在 `protocol = anthropic`**；openai 协议 MUST NOT 写入 `max_tokens`（保持「省略由协议自决」现状）。输出上限与 `[1m]` 后缀**完全解耦**（后缀不改输出上限）。
- **上报口径**：`RunStarted.model` 与请求 `model` 为裸名；`SessionConfigSnapshot.model` 保留配置原文（配置快照语义）；引擎日志记 `model=裸名 window=… raw=原文`。

边界知情：无模型元数据表 → `[1m]` 不做模型白名单校验、beta 一律发（第三方网关拒绝以 `api_error` 显式暴露）；不实现 `maxOutputTokensRecovery`（撞上限即按既有 `finish` 语义收敛，用户调 `max_tokens` 后重跑）。上述边界见「SDK 租户 MVP 边界与已知限制」。

#### Scenario: 后缀解析规则表

- **WHEN** 以 `claude-sonnet-4-6[1m]`、`CLAUDE-SONNET-4-6[1M]`、`glm-4.6`、`  glm-4.6  `、`x[2m]`、`x[8k]`、`x[]`、`x[1m]y` 驱动 `parse_model_spec`
- **THEN** 前两者 → 裸名 + `length = 1_000_000`（大小写不敏感）；`glm-4.6` 与带空白的同一名称 → 裸名 + `length = None`（空白裁剪）；其后四者 → `ModelSpecError`（显式拒收，MUST NOT 把原文透传）

#### Scenario: 按档窗长独立

- **WHEN** provider `models.high = "claude-sonnet-4-6[1m]"`、`models.low = "glm-4.6"` 且 `context_length = 200000`
- **THEN** high 档会话的防线窗长为 1M（后缀覆盖 `context_length`）、low 档会话为 200000；同一 provider 两档各自独立解析，`EngineFacade::with_context_window` 签名不变

#### Scenario: 后缀缺席回落到 context_length

- **WHEN** 三档模型名均不带后缀，`context_length` 分别填 200000 与留空
- **THEN** 填列时防线窗长取填列值、留空时取 128K 缺省启发式（既有行为零变化）

#### Scenario: beta 条件发

- **WHEN** `protocol = anthropic` + 模型名 `[1m]`；`protocol = anthropic` + 无后缀；`protocol = openai` + 模型名 `[1m]` 三种组合
- **THEN** 仅第一种请求含 `anthropic-beta: context-1m-2025-08-07`；后两者不发 beta（openai 协议的 `[1m]` 只影响本地防线窗长）

#### Scenario: 输出上限三档取值链

- **WHEN** anthropic 协议下分别以（a）`provider.max_tokens = 8192`、（b）`max_tokens` 留空 + 名单内官方模型（如 `claude-sonnet-4-6`）、（c）`max_tokens` 留空 + 名单外模型（如 `glm-4.6` / `claude-3-5-sonnet-*`）组装
- **THEN** （a）请求 `max_tokens = 8192`（不被 rig 官方值覆盖）；（b）请求取 rig 官方值（不被降级为兜底值）；（c）请求 `max_tokens = 32000`（兜底，请求不再触发 `max_tokens must be set for Anthropic` 的编码期失败）

#### Scenario: openai 协议不写上限

- **WHEN** `protocol = openai` 且 provider 填有 `max_tokens`
- **THEN** 请求 `max_tokens` 保持省略（字段被记录但不生效），行为与变更前逐字一致

#### Scenario: 非法后缀启动期显式失败

- **WHEN** 存量记录（或绕过保存期校验的路径）携非法后缀模型名发起运行
- **THEN** 启动以 `AgentStartError::ConfigMissing` 显式失败并指明模型名与后缀非法，不产生会话/轮记录、不推流、不把带后缀原文发给 provider

#### Scenario: 裸名上报

- **WHEN** 以 `claude-sonnet-4-6[1m]` 发起一轮并查看事件与快照
- **THEN** `RunStarted.model` 为 `claude-sonnet-4-6`（裸名）；`SessionConfigSnapshot.model` 为配置原文 `claude-sonnet-4-6[1m]`；引擎日志含 `model=claude-sonnet-4-6 window=1000000 raw=claude-sonnet-4-6[1m]`

## MODIFIED Requirements

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + 会话记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎（值域保留为多租户协议预留，`Cli` 为退役态）；协议统一 SHALL 落在内核协议层而非各引擎私有形态。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态为**连接配置字段面**（`protocol` + api_key / base_url / model_high / model_low：协议决定 wire 语义故在此面内，可空调参 `context_length` / `max_tokens` MUST NOT 进本结构、一律走门面旁路 builder）由组合根注入，SDK 引擎用以组装 rig client；`EngineKind::Cli` 臂 SHALL 退役为显式 `Err("CLI 租户已下线")` 形态（runner 构造失败面，具体形态——`Result` 包装或等价——由 design 定稿），**唯一 match 点结构与 `EngineKind` 双值 MUST NOT 塌**（多租户机制保留，未来本机 CLI / 远程 API 租户仍落协议预留；模型协议 MUST NOT 编码进 `EngineKind`）。**解析单点自命令层下沉至内核组合根**（desktop-agent-management「默认 agent 与运行发起解析」的落点平移，解析语义不变）：缺省路径解析默认 agent、显式路径按所选 agent 解析、产物为 `EngineKind` + `EngineConfig`（sdk 时由引用 provider 组装、model 取 provider 三档 high 档、协议取 provider `protocol`）；解析遇 `engine: Cli` 实例（缺省与显式路径同）SHALL 显式 `Err("CLI 租户已下线")`，MUST NOT 静默回退。换解析落点时 `EngineConfig` 消费面 MUST NOT 变化。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回引擎协议实现；core 无 engine / `EngineConfig` / rig / agent / 协议概念，`AgentStartError` 变体集不变

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现与门面签名
- **THEN** 解析语义（默认 agent / 显式 agent / sdk provider 组装 / high 档 / 无默认 `Err` / cli 实例 `Err`）与设计一致，`runner_for` 唯一 match 点结构与连接配置字段面零塌，命令层无引擎解析残留

#### Scenario: cli 解析显式失败

- **WHEN** 解析目标实例 `engine` 为 `cli`（缺省默认实例或显式选择的存量 cli 实例）
- **THEN** 返回 `Err("CLI 租户已下线")`（文案引导管理页），不构造任何引擎、不落库不推流

#### Scenario: 旁路与配置面的分工

- **WHEN** 审查 `EngineConfig` 字段面与门面 builder
- **THEN** `EngineConfig` 仅含连接必需项（`protocol` / `api_key` / `base_url` / `model_high` / `model_low`），`is_complete` 语义不含协议（协议恒有值）；`context_length` / `max_tokens` 经 `with_context_window` / `with_max_tokens` 旁路注入，MUST NOT 出现在 `EngineConfig` 中

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 可收窄该面并追加受控专用工具（见「会话级工具面注入与 explore 笔记工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位维持不消费，`tools` / `toolset` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变；窗长来源含模型名 `[1m]` 后缀（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）
- 事件归一化：归一化 SHALL 只消费 rig 抽象事件词汇（`StreamEvent` / `Completion` / `Message`），工具调用映射为 ToolUse 块并与 ToolResult 成对（同 id）；协议差异 MUST NOT 出现在归一化层（openai chat completions 形状 fixture 仅为测试形状，不是运行时协议判定；anthropic 经 Messages wire 产出同一抽象）；`RunStarted` SHALL 各报各的（model 报**裸名**——`[1m]` 后缀已剥离，tools 报本轮实际下发的工具面——默认七工具、注入面见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：workspace 依赖 SHALL 为 `rig` facade 单条目（`=0.43.0` 锁版——升版经显式修订而非静默漂移；`default-features = false` + 显式 features：`reqwest` / `native-tls` / memory feature，reqwest / native-tls 经 facade 转发到 rig-core）；代码 `rig_core::` 路径零残留。0.42 时代的 `rig-core` 双条目解析钉子退役（2026-10-06 修订）：0.43 家族拆出 rig-reqwest / rig-http / rig-tungstenite 等同伴 crate 后双条目只钉得住 rig-core 一个，全家族精确版本由 Cargo.lock 承担

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: facade 依赖足迹

- **WHEN** 审查 workspace 依赖与 `crates/infra/agent` 源码
- **THEN** 依赖为 `rig` facade 单条目（锁版 `=0.43.0`、`default-features = false` + 显式 features），`rig_core::` 路径零残留；恒编译、无 feature gate

#### Scenario: 双协议均可发起

- **WHEN** 分别以 `protocol = openai` 与 `protocol = anthropic` 的 provider 发起运行
- **THEN** 两者都走同一 `sdk/loop.rs` 全链（工具面 / 防线 / 事件词汇一致），仅 wire 与请求参数不同；归一化层与 core 侧无协议分支

### Requirement: SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）

SDK 引擎 SHALL 对**喂 provider 的请求史**（非 store 转录）实施两级防线，整体水位视角：L1 单结果上限恒常（归「SDK 引擎工具质量与单结果上限（L1）」管辖）→ L2 请求前确定性剪裁（约 70-75% 水位）→ L3 LLM compaction（约 85-90% 水位）→ L4 = 现状 provider 报错收敛兜底，**防线设计目标为 L4 永不抵达**。

- **窗长来源**：优先级 SHALL 为「该档模型名 `[1m]` 后缀 > provider 可空 `context_length` 列（desktop-agent-management）> 128K 缺省启发式」，且 SHALL 按档（`high` / `low`）独立解析（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）；后缀在场时 `context_length` 被覆盖（MUST NOT 叠加或取小值）；`EngineFacade::with_context_window` 签名不变（语义降为「缺省来源」）；token 估算 SHALL 用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款，中文重载比英文经验值准）
- **L2 确定性剪裁**（请求前跑，两个调用点：resume 转录重建之后 + loop 每次发请求前）：先 prune 老工具结果——老 tool_result 内容替换为占位符，tool_use / tool_result 配对结构完整，保护最近窗口（40k 保护窗 / 20k prune 门槛）；不够再丢最老完整轮对（rig-memory 算法 + 孤儿配对清理兜底）。**SHALL 保首条 user**（phase agent 首条 user 即任务书）
- **L3 LLM compaction**：剪裁后水位仍升 → 以当前 model 自摘要老历史，新史 = `[摘要, 首条 user, 最近轮对]`；摘要要点集含关键技术决策**及其原因**（参考 opencode）；compaction 失败 SHALL 降级为 L2 硬裁 + SystemNotice 留痕，**MUST NOT 打断 run**（phase agent 场景宁可丢密度不能丢收敛，比 codex 指数退避保守）
- **可观测**：剪裁与压缩 SHALL 发 `SystemNotice{subtype:"context_pruned"|"context_compacted", payload:{before, after, layer}}`（subtype 开放词典现成）
- **不变量**：store 转录**永远全量**——剪裁/压缩只作用于请求史，重放 / 审计 / resume 重建源不受影响；「转录无上限增长」既有边界留痕不变
- **极限场景留痕**：首条 user 单块自身超窗（巨型 phase prompt + 小窗模型）与单轮即超窗，L1-L3 不覆盖，归 L4 兜底 + 错误细分

#### Scenario: prune 老工具结果

- **WHEN** 历史含多轮大型工具结果且估算水位过 L2 阈值
- **THEN** 发请求前老 tool_result 被替换为占位符（配对完整、最近窗口保护、首条 user 在场），`SystemNotice{context_pruned}` 流出，store 转录仍全量

#### Scenario: compaction 摘要收窄

- **WHEN** prune 后水位仍过 L3 阈值
- **THEN** 老历史被摘要替换，新史 = [摘要, 首条 user, 最近轮对]，`SystemNotice{context_compacted}` 流出，run 继续

#### Scenario: compaction 失败降级

- **WHEN** compaction 摘要调用失败
- **THEN** 降级为 L2 硬裁 + SystemNotice 留痕，run 不中断、不失败收敛

#### Scenario: resume 重建后立即剪裁

- **WHEN** resume 转录重建出的历史即超 L2 水位
- **THEN** 重建后、首请求前执行一次 L2 剪裁（两个调用点之一可考）

#### Scenario: 窗长缺省启发式

- **WHEN** provider 记录 `context_length` 缺席且该档模型名不带 `[1m]` 后缀
- **THEN** 以 128K 启发式计算水位，行为可预期；填列后按填列值计算

#### Scenario: 后缀覆盖填列窗长

- **WHEN** 该档模型名带 `[1m]` 后缀且 provider 同时填有 `context_length = 200000`
- **THEN** 该档防线窗长按 1M 计算（后缀覆盖，不做叠加或取小值），其余档按各自解析结果计算；store 转录与防线算法零变化

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行），但授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change
5. **cost 恒缺**：`RunResult.cost_usd` 恒 `None`（无价格表），汇总卡如实呈现
6. **engine_cfg 硬编码位（已偿还，desktop-agent-management）**：api_key / base_url / model 手填真机验证的限制已由全局 agent 管理偿还——engine_cfg 构造源换为管理数据解析（默认 agent / 显式 agent），连接配置经管理页维护，明文边界与遮蔽治理见 desktop-agent-management「api_key 机密边界」
7. **resume 保真度缺口**：Raw 丢弃 / 子代理压平 / 仅顶层重建（L2 请求前剪裁只缓解请求侧成本随历史增长，重建源仍为 store 全量转录）
8. **引擎归属留痕未裁**：run 记录暂不区分引擎（座位 design 裁定后偿还）
9. **无模型元数据表（协议与后缀的知情边界）**：不知道任意模型的 `default/upper` 输出上限，也无法判定 `supports_1m_suffix` / `supports_1m_beta`——故 `[1m]` 后缀一律接受（不做模型白名单校验）、`anthropic-beta` 一律发（第三方网关可能拒 → `api_error` 显式暴露，非静默）；名单外模型输出上限兜底恒 `32_000`，老代模型（8K/16K 上限）与第三方网关需用户在 provider 显式填 `max_tokens` 才可跑通
10. **不实现 `maxOutputTokensRecovery`**：撞输出上限即按既有 `finish` 语义收敛（不注入续写消息、不自动抬高上限），用户调 provider `max_tokens` 后重跑
11. **跨协议续会话不做一致性校验**：改 provider `protocol` 后以 `Continue` 续旧会话，旧史由新 wire 重编码——rig 只回放本 dialect issuer 的 reasoning 块，**thinking 会静默丢弃**；`SessionConfigSnapshot` 不记协议（与「改 base_url / api_key 同权」裁决一致）；system 消息不进持久化史（每轮头部单独注入），故无残留 system 形态问题

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 十一条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 兜底边界的失败面可见

- **WHEN** anthropic 协议下对「名单外 + 8K 输出上限」模型留空 `max_tokens`（取 32_000 兜底）发起运行
- **THEN** 请求以 provider 400 失败并经 `api_error` 走既有错误收敛（显式、可读诊断），不静默；用户填 `max_tokens = 8192` 后可跑通——该组合作为兜底策略边界的留痕证据

## Module Contract

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `build_model` | `fn build_model(config: &EngineConfig, spec: &ModelSpec) -> DynModel<Completion>` | 协议唯一分叉处：openai 臂现状等价；anthropic 臂条件 `with_beta` + 定型 wire 默认 `max_tokens`；两臂同产 `DynModel<Completion>` |
| `SdkRunner::open_session` | 签名不变 | 按档 `parse_model_spec(raw)`（`Err` → `ConfigMissing`）、窗长按档现算 `spec.length.or(self.context_window)`、裸名下发 |
| `SdkRunner::with_max_tokens` | `fn with_max_tokens(self, max_tokens: Option<u64>) -> Self` | 旁路 builder（`new` 签名不动，既有调用点零改）；与 `context_window` 并列的解码后载荷 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs`（新）

| 项 | 签名 | 描述 |
|------|------|----------|
| `parse_model_spec` | `fn(raw: &str) -> Result<ModelSpec, ModelSpecError>` | 纯函数：末尾 `[1m]`（大小写不敏感）→ 裸名 + `1_000_000`；其余方括号形态显式 `Err`；无后缀 → 裸名 + `None`；空白裁剪 |
| `ModelSpec` | `{ name: String, length: Option<u64> }` | 解析产物（`length` 为窗长候选，`None` = 走 `context_length` / 128K） |
| `ModelSpecError` | 携原文与原因 | 保存期与启动期共用（错误串 MUST NOT 含 api_key） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `Protocol` | `enum { Openai, Anthropic }` | 门面协议枚举（映射自 store `AgentProviderProtocol`；两臂 match 于 `build_model`） |
| `EngineConfig` | `{ protocol, api_key, base_url, model_high, model_low }` | 连接配置字段面（协议在内）；`empty()` 给 `Openai`；`is_complete` 语义不变（协议不参与校验）；`context_length` / `max_tokens` MUST NOT 进本结构 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` + `compose.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|----------|
| `EngineFacade::with_max_tokens` | `fn(self, Option<u64>) -> Self` | 与 `with_context_window` 同型旁路缝 |
| `EngineFacade::runner_for` | 签名不变（`type` 双值 / 唯一 match 点） | sdk 臂把旁路载荷转入 `SdkRunner` |
| `resolve_agent_engine` | 内部实现变化 | `provider.protocol` → `Protocol`；`provider.max_tokens` → `ResolvedEngine` 旁路；解析语义（默认 / 显式 / high 档 / cli 拒收）零变 |
| `parse_model_spec` 再导出 | `pub use sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError}` | 命令层保存期校验复用同一函数（MUST NOT 造第二套规则） |

### API: `packages/desktop/src-tauri/crates/infra/store`（消费面）

| 项 | 变化 | 描述 |
|------|------|----------|
| `AgentProviderRecord` | 增 `protocol` / `max_tokens`（V3） | 见 desktop-agent-management Module Contract；本能力只读消费 |
| `AgentProviderProtocol` | 新枚举 | 与本能力门面 `Protocol` 一对一映射（映射点 = 组合根解析单点） |
