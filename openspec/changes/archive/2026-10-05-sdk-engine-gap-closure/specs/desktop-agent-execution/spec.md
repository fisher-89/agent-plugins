# desktop-agent-execution（delta：SDK 引擎产品层补全）

## ADDED Requirements

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎 SHALL 在起播前读取 workspace root 的 `AGENT.md` 单份文件作为系统提示词来源：文件存在 → 内容**逐字**注入请求 `preamble`（不包装、不改写）；文件缺席 → `preamble` 保持 `None`（现状不变）。读取范围 SHALL 严格限于 `AGENT.md`，MUST NOT 做 `CLAUDE.md` 等兜底回退。注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（不做起播快照——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

该机制 SHALL 为引擎内部特性（P2 原则：跨会话持久上下文是 agent 产品特性非协议义务），`crates/core/agent` 零改动；`SessionInjections` 消费面维持现状（compose 恒传 default 不动）。preamble 自身体量上限（32KB 量级）与嵌套 `AGENT.md` 不级联由 design 定稿。

#### Scenario: AGENT.md 在场逐字注入

- **WHEN** workspace root 存在 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求 preamble 为该文件内容逐字（不包装、不改写），`crates/core/agent` 与 compose 面零 diff

#### Scenario: 缺席维持现状

- **WHEN** workspace root 无 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** preamble 为 `None`，请求形状与注入机制上线前一致

#### Scenario: 会话中修改下一轮生效

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮请求注入修改后的内容（每轮重读，无起播快照）

#### Scenario: 无兜底回退

- **WHEN** workspace root 只有 `CLAUDE.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，preamble 仍为 `None`（严格单文件语义）

### Requirement: SDK 引擎工具质量与单结果上限（L1）

SDK 引擎自建工具面 SHALL 完成两项质量升级与一项统一上限：

- **read 截断与翻页**：read 工具 SHALL 缺省截断（约 2000 行，design 定稿），截断时 SHALL 在结果尾部留痕（「已截断，可 offset 翻页」形态）并 SHALL 支持 offset / limit 参数取回后续窗口；现行 read 无大小上限的形态 MUST NOT 延续
- **grep 正则与上下文行**：grep 工具 SHALL 按正则语义匹配（替换现行子串字面匹配，`sdk/tools.rs:180-197`）并 SHALL 支持上下文行参数（匹配行 ± N 行，缺省 0）；200 条结果上限维持
- **单结果字节上限（防线 L1 层）**：全工具 SHALL 施加统一单结果字节上限，超限截断并留痕；bash 工具 stdout/stderr 合并输出同归此层

工具 schema 演进 SHALL 随工具面定义更新（新增参数进 input schema），事件面零改动（ToolUse / ToolResult 词汇不变）。

#### Scenario: read 截断与翻页

- **WHEN** read 一个超过截断阈值的大文件
- **THEN** 返回前 N 行 + 尾部截断留痕；带 offset 请求返回后续窗口，两段拼接覆盖全文

#### Scenario: grep 正则与上下文

- **WHEN** grep 以正则 pattern（如 `foo.*bar`）与上下文行参数执行
- **THEN** 按正则语义匹配并携带匹配行上下文；子串字面匹配行为退役；结果超 200 条仍截断

#### Scenario: 单结果字节上限

- **WHEN** 任一工具单结果超统一字节上限
- **THEN** 结果截断至上限并留痕，ToolResult 事件面不炸、run 不中断

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报七工具：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理复用 `cli/runner.rs` `kill_process_tree` 的 `taskkill /T /F` 形状（future drop 不自动杀子进程树；仅复用形状，`cli/` 文件零 diff）
- **超时**：缺省超时（参照 claude code 120s 缺省 / 600s 上限，具体值 design 定稿）——活性护栏，非安全护栏
- **知情边界**：授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change（边界条款见「SDK 租户 MVP 边界与已知限制」）

#### Scenario: 放行档执行回灌

- **WHEN** AcceptEdits（或 BypassPermissions）档下模型调用 bash 执行命令
- **THEN** 命令执行、ToolResult 含 stdout/stderr 合并输出，bash 事件与其他工具同构进出转录

#### Scenario: Default 档拒绝

- **WHEN** Default 档下模型请求 bash 工具
- **THEN** 拒绝流出（`permission_denied` SystemNotice + is_error=true ToolResult），run 不中断

#### Scenario: 超时与进程树清理

- **WHEN** bash 命令超时，或命令运行中会话被终止
- **THEN** 进程树被清理（`taskkill /T /F` 形状），无孤儿进程残留；超时以活性护栏错误收口，不挂死 run

#### Scenario: Windows 兜底探测

- **WHEN** Windows 环境无 git-bash
- **THEN** 经 `cmd /C` 兜底执行，开箱即跑不破坏；探测与兜底选择可测（探测函数纯化）

### Requirement: SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）

SDK 引擎 SHALL 对**喂 provider 的请求史**（非 store 转录）实施两级防线，整体水位视角：L1 单结果上限恒常（归「SDK 引擎工具质量与单结果上限（L1）」管辖）→ L2 请求前确定性剪裁（约 70-75% 水位）→ L3 LLM compaction（约 85-90% 水位）→ L4 = 现状 provider 报错收敛兜底，**防线设计目标为 L4 永不抵达**。

- **窗长来源**：消费 provider 记录可空 `context_length` 列（desktop-agent-management）；缺席时 SHALL 以 128K 缺省启发式；token 估算 SHALL 用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款，中文重载比英文经验值准）
- **L2 确定性剪裁**（请求前跑，两个调用点：resume 转录重建之后 + loop 每次发请求前）：先 prune 老工具结果——老 tool_result 内容替换为占位符，tool_use / tool_result 配对结构完整，保护最近窗口（参考 opencode 40k/20k 保护窗，design 定稿）；不够再丢最老完整轮对（rig-memory 算法 + 孤儿配对清理兜底）。**SHALL 保首条 user**（phase agent 首条 user 即任务书）
- **L3 LLM compaction**：剪裁后水位仍升 → 以当前 model 自摘要老历史，新史 = `[摘要, 首条 user, 最近轮对]`；摘要要点集含关键技术决策**及其原因**（参考 opencode）；compaction 失败 SHALL 降级为 L2 硬裁 + SystemNotice 留痕，**MUST NOT 打断 run**（phase agent 场景宁可丢密度不能丢收敛，比 codex 指数退避保守）
- **可观测**：剪裁与压缩 SHALL 发 `SystemNotice{subtype:"context_pruned"|"context_compacted", payload:{before, after, layer}}`（subtype 开放词典现成）
- **不变量**：store 转录**永远全量**——剪裁/压缩只作用于请求史，重放 / 审计 / resume 重建源不受影响；「转录无上限增长」既有边界留痕不变
- **极限场景留痕**：首条 user 单块自身超窗（巨型 phase prompt + 小窗模型）与单轮即超窗，L1-L3 不覆盖，归 L4 兜底 + 错误细分（design 小项）

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

- **WHEN** provider 记录 `context_length` 缺席
- **THEN** 以 128K 启发式计算水位，行为可预期；填列后按填列值计算

## MODIFIED Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个 crate：

- `crates/core/agent`（裸名 `agent`）：**会话域内核**——会话域中立契约与内核编排：`AgentEvent` 词汇（delta/sealed 双层）、P1 引擎协议（会话建立 / question / stop / 观察回流）、会话状态机与收敛状态机（running → completed/failed/stopped）、seq/时间戳盖戳治理、停止注册（内存态）、write-through 持久化编排、查询契约与 port 定义。纯度原则修订（2026-10-01 裁定）：「纯类型无 IO」改写为「**无对外部世界的直接 IO、经 port**」——文件/进程/网络等直接 IO MUST NOT 出现，IO 一律经 core 定义的 port 由 infra 实现；进程内存原语（时钟盖戳、mpsc/Notify、内存注册表）不在禁令内。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析），MUST NOT 认识引擎（不出现 engine / rig / openai 概念）；loop / memory / LLM 协议归 infra 不变。
- `crates/infra/agent`（裸名 `agent-runtime`）：agent 边界的引擎门面与实现——对外以 `EngineKind`（`cli` | `sdk`）参数选择引擎，引擎分发 SHALL 为门面内唯一 match 点；内部承载 `cli/` 引擎（CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化）与 `sdk/` 引擎（rig 进程内租户：agent loop、自建工具面、静态权限 policy、路径沙箱、事件归一化、会话转录恢复）。SHALL 实现内核 port（含 store 持久面与查询面要求的落地）；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）；`rig`（rig-core 的 re-export facade，sdk-engine-gap-closure 自直连切换）SHALL 恒编译、MUST NOT 设 feature gate（SDK 引擎定位为不依赖本地环境的 agent，开箱即用，不提供 CLI-only 裁剪档；rig-core 经 facade 传递入树）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-runtime`，`agent-runtime → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate（port 落点裁定的默认形态：core 定义 port、infra 实现，该规则保持不变；若 design 裁定 core 直依 store crate，须同步修订 desktop-crate-layout）。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。单 crate 起步（`cli/`、`sdk/` 子模块收拢）；拆引擎子 crate + 门面 re-export 仅作膨胀后的逃生门，MUST NOT 预建。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-runtime` 依赖 `agent` 与 `rig`（facade），壳依赖两者
- **AND** `agent` 与 `agent-runtime` 的依赖中无 tauri 系条目，`agent` 无 workspace 内依赖（core 定义 port、infra 实现形态下）

#### Scenario: core 不认识 claude 与引擎且无直接 IO

- **WHEN** 审查 `crates/core/agent` 源码
- **THEN** 无 claude 特有概念（flag 名、JSONL 行解析、claude 事件 type 名），亦无引擎概念（engine / rig / openai 字样）；无文件/进程/网络直接 IO 触点（IO 全经 port 签名表达）；claude 细节全部关在 `agent-runtime` 的 `cli/` 引擎内，rig / openai 细节全部关在 `sdk/` 引擎内

#### Scenario: spawn 住在 infra

- **WHEN** 检索进程 spawn 调用（`std::process` / `tokio::process`）
- **THEN** 仅出现在 `crates/infra/agent` 的 `cli/` 引擎子模块；`core/agent` 与壳 command 层无 spawn

#### Scenario: 引擎唯一分发点

- **WHEN** 审查 `agent-runtime` 的引擎构造入口（`runner_for` 形状）
- **THEN** `EngineKind` → 引擎构造的 match 唯一，内核组合根仅做参数映射不经手引擎细节；新增引擎只动门面一处

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 首公民 SHALL 为 openai-compatible 端点（自定义 base_url——GLM / DeepSeek / Kimi / 自部署第一公民）；anthropic provider 降为后续可选
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `ReasoningDelta` 增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面）；轮末 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：MVP 工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求 preamble SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections` 消费面维持现状
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变
- 事件归一化：openai chat completions 形状的 `tool_calls` 数组 SHALL 映射为 ToolUse 块并与 ToolResult 成对（同 id）；`RunStarted` SHALL 各报各的（model 取 `EngineConfig.model`、tools 报自建七工具集）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：workspace 依赖 SHALL 为 `rig` facade 条目（`=0.42.x` 锁版平移、MUST NOT 升版本；`default-features = false` + 显式 features：`reqwest` / `native-tls` / memory feature；rig-core 经 facade 传递入树，MUST NOT 保留直连残留——单/双条目 pin 策略由 design spike 定稿）

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: facade 依赖足迹

- **WHEN** 审查 workspace 依赖与 `crates/infra/agent` 源码
- **THEN** 依赖为 `rig` facade 条目（锁版 `=0.42.x`、`default-features = false` + 显式 features），`rig_core::` 路径零残留；恒编译、无 feature gate

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit / bash 拒绝；AcceptEdits 档 = + write / edit / bash；BypassPermissions 档 = 全放行。bash 档位语义（Default 拒绝、AcceptEdits / BypassPermissions 放行）为现有三档对第七工具的自然延伸——只读档不给执行权；**AcceptEdits 与 BypassPermissions 自此首次真实分化**（此前两臂行为完全一致，`sdk/policy.rs:29-39`）。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。policy SHALL 为纯决策表（档位 × 工具名 → 允许 / 拒绝）密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 仍是模型与任意**文件写**（write / edit）之间的唯一护栏，其实现质量为必过关卡；bash 进工具面后，**命令执行不在路径沙箱射程内**——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零，此为知情边界留痕（见「SDK 租户 MVP 边界与已知限制」）而非安全缺口遗漏；命令级白/黑名单 MUST NOT 作为安全手段（假安全），真沙箱（job object / 容器）与交互审批留后续 change。

#### Scenario: 三档决策表

- **WHEN** 以三档 × 工具名矩阵驱动 policy 纯函数
- **THEN** Default 档对 write/edit/bash 拒绝、AcceptEdits 档放行 write/edit/bash、BypassPermissions 档全放行，逐格断言通过

#### Scenario: AcceptEdits 与 Bypass 分化

- **WHEN** 以 AcceptEdits 档与 Default 档分别驱动 bash 工具名
- **THEN** AcceptEdits 放行、Default 拒绝——两档对同一工具首次给出不同决策，既有「两臂一致」行为退役（分化由锁定测试钉住）

#### Scenario: 拒绝流出形态

- **WHEN** Default 档下模型请求 write 工具
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied"}` 与 is_error=true 的 ToolResult（与 tool_use 同 id），run 不中断

#### Scenario: 逃逸拦截

- **WHEN** 工具入参路径含 `..\` 相对逃逸，或为符号链接指向 workspace root 外目标
- **THEN** canonicalize + 前缀校验拒绝执行（tempdir 驱动密集用例全拦）；root 内合法路径不受影响

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

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/Cargo.toml`（workspace 依赖） | rig facade 切换 | `rig` 条目：`=0.42.x` 锁版、`default-features = false`、features `reqwest` + `native-tls` + memory feature；`rig-core` 直连条目按 spike pin 策略处置 |
| `crates/infra/agent/src/sdk/{loop,normalize,resume,runner,tools,policy,config}.rs` | facade 路径迁移 | `rig_core::` → `rig::`（约 15 处五文件，纯 re-export 同名）；loop.rs 三处「CLI 口径」注释改为 core 协议口径 |
| `crates/infra/agent/src/sdk/`（新增 preamble 读取） | AGENT.md 注入 | 起播前读 `<workspace_root>/AGENT.md` 逐字注入；每轮重读、缺席 None、无 CLAUDE.md 兜底；零 core 改动 |
| `crates/infra/agent/src/sdk/tools.rs` + bash 工具模块（新） | 七工具面与质量 | `TOOL_NAMES` 七项；read 截断 + offset 翻页；grep 正则 + 上下文行；全工具单结果字节上限；bash `tokio::process` + git-bash 探测 / `cmd /C` 兜底 / unix `sh -c`；`kill_on_drop` + `taskkill /T /F` 形状清理；超时活性护栏 |
| `crates/infra/agent/src/sdk/policy.rs` | 三档决策表（bash 分化） | bash：Default 拒、AcceptEdits / Bypass 放；AcceptEdits ≠ Bypass 首次分化锁定测试 |
| `crates/infra/agent/src/sdk/`（新增上下文防线模块，名 design 定） | L2 / L3 防线 | 窗长 = provider `context_length` 列缺省 128K 启发式；字节启发式 token 估算；prune 占位符 + 轮对裁剪 + 保首条 user；LLM compaction（rig-memory `Compactor` trait 的 LLM 版）；失败降级不打断 run；`context_pruned` / `context_compacted` SystemNotice；调用点 = resume 重建后 + 每请求前；store 转录全量不变 |
| `crates/core/agent/**`、`crates/infra/agent/src/cli/**`、`compose.rs` | 本变更零 diff | core 契约 / CLI 引擎不动；`SessionInjections` 恒传 default 不动（injections 收窄与交互审批留独立 change） |
