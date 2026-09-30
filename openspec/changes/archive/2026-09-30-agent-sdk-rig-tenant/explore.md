# agent-sdk-rig-tenant 探索笔记

> 状态：探索收敛，待 phase-proposal 晋升
> 日期：2026-09-30
> 拟议 change 名候选：`agent-engine-facade-rig-sdk`（phase-proposal 定夺）
> 覆盖 spec：`desktop-agent-execution`（修订为主，非新建）

## 背景与起点

- 现状：agent 边界双 crate —— `crates/core/agent`（裸名 `agent`，中立契约：AgentEvent 五变体信封 / AgentRunner trait / RunHandle / run 状态机）+ `crates/infra/agent`（裸名 `agent-cli`，CLI 租户 MVP：discover / flags / jsonl / runner）。
- spec 预留原话（desktop-agent-execution）：「三租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在该 trait 预留内——本变更仅实现 CLI 租户，SDK / API 租户 MUST NOT 预建」。本探索即掀这个盖子。
- 起点：以 rig（rig-core，Rust LLM 框架）封装进程内 SDK 租户。
- 契约红利（探索中确认）：trait 为同步 `start()` 返回有界 mpsc —— SDK 租户 spawn tokio 泵任务灌流，与 CLI 泵同构；**编排层（`run_agent` / tee / 状态机）零改动**。事件信封的 ToolUse/ToolResult 成对、`parent_tool_use_id`、RunResult 汇总字段本就照 agent loop 形状设计 —— SDK 引擎喂对事件后，AgentTimeline / 落库 / 重放 / explore 链全部复用，前端除 runner 选择参数面外零改动。

## 已拍板决策

### D1：定位 = 编码 agent（B 线）

不做轻量对话伴侣，做进程内编码 agent。CLI 租户夹带的私货（文件工具、bash、权限门控、MCP、会话续话）中，工具面与权限面由 infra 层自建承接（见 D2 形状）；MVP 不做 MCP、不做子代理（`parent_tool_use_id` 留空）。

### D2：infra/agent 升格为引擎门面（用户裁定）

在 `infra/agent` 内抹平 cli / sdk / 不同 agent 引擎差异，对外一套协议，通过参数选择 agent 引擎：

- 壳层保持三件事薄命令（参数转换 → 调用 → 错误映射），引擎接线全在门面内消化。
- 协议统一在**应用协议层**（invoke 面 + 事件流 + store 记录 + Timeline 渲染），而非 core trait 层。
- 内部结构（示意）：

```
                    core/agent（中立契约，不动）
                              ▲ impl
┌─────────────────────────────┴─────────────────────┐
│ crates/infra/agent（引擎门面）                     │
│  门面：EngineKind 参数 → 引擎分发（唯一 match）    │
│  ┌────────────┬────────────┬────────────┐         │
│  │ cli/ 引擎   │ sdk/ 引擎  │ 引擎 #3 …  │         │
│  │ discover   │ rig loop   │ (codex CLI/ │         │
│  │ flags      │ tools      │  gemini/远  │         │
│  │ jsonl      │ policy     │  程 API)    │         │
│  │ runner     │ sandbox    │            │         │
│  │            │ normalize  │            │         │
│  └────────────┴────────────┴────────────┘         │
└───────────────────────────────────────────────────┘
                              ▲ 唯一调用点
                    shell: agent_start（invoke body 带 engine 字段）
```

- 单 crate 起步（工具/policy/sandbox 收在 `sdk/` 子系统内）；若膨胀再拆引擎子 crate、门面 re-export（留逃生门，不预建）。
- **rig 恒编译、不设 feature gate（已拍板）**：SDK 引擎定位为**不依赖本地环境的 agent** —— 无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；rig 进默认构建，不提供 CLI-only 裁剪档。

### D3：EngineKind 住 infra（座位 B）

三个候选座位裁决：

- 座位 A（core 的 `AgentRunParams` 加 engine 字段）：否 —— 违反契约中立（core 连 claude 都不认识，不认识引擎）。
- **座位 B（infra 门面构造参数，采纳）**：invoke body 带 `engine` 字段 → shell 映射 `EngineKind` → 门面分发。core 契约零污染。
- 座位 C（模型推导引擎）：否 —— 隐式耦合，违背「显式参数选择」。
- **注入缝（已拍板，分两步走）**：目标形状 `runner_for(kind, engine_cfg)` —— engine_cfg（api_key / base_url / model 等）由壳层注入，SDK 引擎用以组装 rig client；**CLI 引擎暂不消费 engine_cfg**（预留后续解决，如将来映射 `--model`）。**MVP 第一步：engine_cfg 从预留硬编码位构造**（用户手填做真机验证），注入通道与配置落盘（硬骨头六座位 ②vs③）后续迭代偿还；硬编码位保持 `EngineConfig` 结构体形态，换构造源时消费面零改动。

「参数选择引擎」语义要在 proposal 写死在命令面，防 evaluator 误读为 core 参数面。

### D4：静态权限三档起步（已确认：足够；默认 BypassPermissions）

审批的反向通道（事件流外的应答路）是契约缺口，MVP 不付契约演进成本：

| 档位 | SDK 引擎工具面 |
|---|---|
| Default | 只读（read/grep/glob/ls）；write/edit/bash 拒绝 |
| AcceptEdits | + write/edit；bash 拒绝 |
| BypassPermissions | 全放行 |

- 拒绝经 `SystemNotice{subtype:"permission_denied"}` + is_error ToolResult 流出 —— 与 CLI `-p` 无头行为同构（spec 对 Default 档本就定义「需审批工具直接被拒」）。
- 交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）留独立 change，CLI 租户将来同样受益。
- **默认档 = BypassPermissions（已拍板）**：与调试页既有默认一致（spec 已裁本机自有 repo 调试场景以完整循环为默认）。MVP 实际体验 = 默认全放行，权限档位是用户显式降级用的。连带推高「骨头二」的分量：默认档下路径沙箱是模型与任意文件写之间的唯一护栏。

### D5：resume = 「store 即会话」

SDK 引擎不自建会话存储；resume 时从 workspace 库事件转录重建对话历史喂 rig。事件模型本就是中立转录格式，此路不引入第二份状态。保真度缺口留痕：Raw 事件丢弃、子代理消息压平，MVP 只重建顶层对话。

## 抹平矩阵（「一套协议」的验收清单）

| 差异维度 | CLI 引擎 | SDK 引擎 | 抹平归宿 |
|---|---|---|---|
| 启动失败 | CliMissing / SpawnFailed | key 未配 / 模型不存在 | `AgentStartError` 加中性变体（core 唯一触碰点，加法） |
| 权限 | flag 转交 CLI 内部裁决 | 自建 policy 三档映射 | 三档语义统一（D4） |
| resume | `--resume <session_id>` | store 转录重建 | 同一参数，引擎各自解释 |
| 停止 | `taskkill /T /F` 树杀 | drop future | RunHandle 已抹平 |
| 模型 | CLI 用户默认 | workspace config | `RunStarted.model` 各填各的 |
| 工具清单 | CLI 自带全套 | 自建 toolset | `RunStarted.tools` 各报各的 |
| API 重试/错误 | CLI system 事件 | rig 错误拦截 | `SystemNotice{subtype}`（开放枚举，零契约改动） |
| cost | CLI 报价 | 无价格表 | `RunResult.cost_usd = None` |

## 五块硬骨头（design 阶段的输入）

1. **审批反向通道**：静态策略 vs 审批协议，裁决见 D4；协议版留独立 change。
2. **沙箱与刀刃**：权限兜底人从 claude CLI 变成我们，且调试页默认 bypassPermissions（spec 已裁「本机自有 repo 调试场景」）。最低要求：write/edit/read 全链 canonicalize + workspace root 前缀校验（拦 `..\` 逃逸与符号链接绕过，纯函数可密集测试）；bash 工具要么 MVP 不带（只文件工具 + grep），要么带但明示「bypass 档裸奔」已知限制。命令级白/黑名单是假安全，真沙箱（job object / 容器）非本期体量。
3. **loop 归属**：倾向手搓 agent loop（流式 response → 映射事件 → 执行工具 → 回灌 → 续轮，收敛与 RunResult 填充全自控），rig 只用 provider client + 消息/工具类型层。⚠️ rig 当前版本 API 形状（Tool trait 签名 / streaming / multi_turn）有记忆陈旧风险，必须 spike 实测，不凭记忆断言。
4. **provider 首公民（已拍板：openai）**：走 openai-compatible 端点 —— 一个自定义 base_url 解锁 GLM/DeepSeek/Kimi/自部署。normalize 层围绕 openai chat completions 形状设计（`tool_calls` 数组 → ToolUse 块）。新增设计考量：GLM/DeepSeek 系的 reasoning 经非标准 `reasoning_content` 扩展暴露 —— 映射进 `Thinking` 块还是 MVP 缺席留痕，design 阶段定；anthropic provider 降为后续可选。
5. **resume 保真度**：见 D5 缺口留痕。
6. **配置落盘座位（已裁：MVP 预留硬编码位，座位之争顺延）**：勘察发现现行 workspace config 文件 `openspec/config.json` 为 **git 追踪文件**（spec-driven 工作流配置，团队共享语义），api_key 直入即机密进版本库。候选座位：①直入（否，泄密）；②拆两截——model/base_url 进 openspec/config.json、api_key 走不入库处；③全部走 app 侧——全局数据目录按 root 键控。**裁定：MVP 预留硬编码位（用户手填 key/base_url/model 做真机验证），注入方式与配置座位后续迭代再裁（②vs ③保留为素材）**。硬编码位形状建议保持 `EngineConfig` 结构体形态（api_key / base_url / model 三件套），后续迭代只换构造源（硬编码 → config 读取），消费面零改动。

## 未决问题（proposal / design 输入）

- [x] D4 静态权限起步是否够 → **够**；默认 BypassPermissions（与调试页既有默认一致）
- [x] provider 首公民 → **openai-compatible 端点**（自定义 base_url；GLM/DeepSeek/自部署第一公民，anthropic 降为后续可选）
- [ ] 引擎归属落点：run 记录加列（动 store schema）/ `RunStarted` 加字段（动事件线格式 + specta DTO，倾向此座——事件本就是 run 自我描述，落库重放免费携带）/ MVP 不记留痕
- [ ] bash 工具进不进 MVP
- [x] model 不进 params → **已确认**：workspace config 增 api_key / model 等配置，注入 agent（注入缝见 D3）；CLI 引擎暂不消费这些参数，预留后续解决。**MVP 预留硬编码位（用户手填真机验证），注入方式与配置座位后续迭代**（openspec/config.json git 追踪的座位之争见硬骨头六，顺延不阻塞）
- [x] rig 是否 feature-gate → **不 gate，恒编译**：SDK 引擎 = 不依赖本地环境（claude CLI）的 agent，开箱即用；依赖足迹 spike 升为必过关卡

## spike 清单

- [ ] rig-core 依赖足迹（**恒编译裁定后为必过关卡，无 gate 回退逃生门**）：`default-features = false` + openai feature 实际入树体积（reqwest/schemars/tracing 系）；0.x 版本线是否 `=` pin（workspace 依赖纪律 / static-check 关）；超标时缓解仅剩 provider feature 收窄与版本选择
- [ ] rig 当前 API 形状核实：Tool trait 签名、streaming response 形状、openai provider 自定义 base_url 路径、`reasoning_content` 类非标准扩展的暴露形态（anthropic thinking 支持度降为次要）

## 连带影响清单（proposal 范围素材）

1. **crate 改名**：裸名 `agent-cli` 名不副实（候选 `agent-runtime` / `agent-engines`）；现在改便宜（消费者唯壳层 + members 一行 + spec 一段），SDK 落地后改贵。
2. **spec 修订**：`desktop-agent-execution` 的「SDK / API 租户 MUST NOT 预建」条款由本变更掀盖；模块契约表 `agent-cli` 行重写；增补引擎门面与 SDK 引擎 requirements（tools/policy/sandbox 契约留痕、MVP 边界：无 MCP / 无子代理 / 无交互审批 / bash 裸奔或缺席）。
3. **core 契约唯一触碰点**：`AgentStartError` 中性变体（如 `ConfigMissing`），加法不改既有形态。
4. **测试纪律沿既有**：normalize 层纯函数 + fixture（不打网络）、工具 tempdir 驱动、policy 纯决策表、sandbox 路径校验密集测试、runner 注入假流缝（同 `pump_lines` 注入模式）；不重测 rig 自身语义。

## 对话纪要（五轮收敛轨迹）

1. 勘察：确认三租户预留座位 + rig 吻合度 + 定位分叉（A 轻量伴侣 / B 编码 agent）。
2. 裁定 B；摊开五块硬骨头（审批通道 / 刀刃 / loop 归属 / provider / resume）。
3. 裁定 infra 门面 + 引擎参数选择；EngineKind 座位 B；抹平矩阵；连带影响（改名 / spec 修订 / 引擎归属）。
4. 拍板未决两项：D4 静态权限足够、默认 BypassPermissions；provider 首公民 = openai-compatible 端点。
5. 拍板：rig 恒编译不设 feature gate——SDK 引擎定位为不依赖本地环境（claude CLI）的 agent，开箱即用；依赖足迹 spike 升为必过关卡。
6. 拍板：workspace config 增 api_key / model 等配置注入 agent，CLI 引擎暂不消费预留；勘察发现 openspec/config.json 为 git 追踪文件 → 配置落盘座位新开硬骨头六（座位 2 拆两截 vs 座位 3 全 app 侧，待裁）。
7. 拍板：硬骨头六顺延——MVP 预留硬编码位（用户手填 key/base_url/model 真机验证），注入方式与配置座位后续迭代；硬编码位保持 EngineConfig 结构体形态，后续只换构造源。
