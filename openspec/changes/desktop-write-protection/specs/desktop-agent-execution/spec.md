# desktop-agent-execution Specification（delta: desktop-write-protection）

## ADDED Requirements

### Requirement: loop 守卫链写入保护闸与工作区根注入缝

SDK 引擎的 loop 分发链 SHALL 在既有三段守卫之后、执行之前增设第四段「写入保护闸」，并由一条与 `with_explore_note_store` 同构的 builder 缝注入主仓 root：

- **四段顺序不变量**：`① 成员校验 → ② 档位门（policy）→ ③ 路径沙箱（sandbox_rewrite）→ ④ 写入保护闸 → 执行`。保护闸 MUST 位于沙箱改写**之后**（此时 `path` 已是 canonical 绝对路径，判定无需再做 IO 级规范化），MUST 位于 `tools::execute` / `explore_note` 执行**之前**；MUST NOT 提前到沙箱之前，MUST NOT 下沉进 `tools::execute`（那是纯函数，塞入读配置会让签名长出 IO 依赖，且拒绝形态只能由 loop 合成）。
- **判定输入**：`write` / `edit` 取沙箱改写后的 `path` 字段；`bash` 取 `command` 原文（命令内容不可 canonicalize）与会话执行根 `LoopTurn.cwd`（相对路径解析基座）。
- **拒绝形态复用既有 `deny()`**：`SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + 与 tool_use 同 id 的 is_error `ToolResult`，**run 不中断**（模型换路自愈）。MUST NOT 新增 subtype、MUST NOT 新增事件词汇、MUST NOT 新增收敛退出码；保护闸 MUST NOT 自行发事件（拒绝串内容归 desktop-write-protection）。
- **档位面**：保护闸在任何 `permission_mode` 下均生效（Default 档本就拒绝 write / edit / bash；AcceptEdits 与 BypassPermissions 档下受保护文件仍被拒）。MUST NOT 修改 policy 三档决策表、MUST NOT 引入审批通道。
- **装配时机**：保护规则 SHALL 每 run 装配一次（会话泵起播时 `protect::load`），随 `LoopTurn.protect` 下发（与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」形态同构）；loop 内 MUST NOT 读盘、MUST NOT 每 tool_call 重读配置。
- **工作区根通道（新 builder 缝）**：root 只在命令层显式在手（store 句柄无 root 字段），故 SHALL 沿既有先例逐层下行：

```
compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)
  → EngineFacade::with_workspace_root(root)
  → SdkRunner.workspace_root: Option<PathBuf>
  → PumpPayload.workspace_root → 泵起播时 protect::load(…)
  → LoopTurn.protect: ProtectRules
```

  `workspace_root` MUST 为**主仓** root（命令层 `root` 参数），MUST NOT 取会话执行根 `SessionCtx.workspace_root`（change 流程下为 worktree）——工作区配置的权威在主仓。
- **缺席语义**：`workspace_root` 为 `None`（未注入）时 SHALL 只装载内置规则（用户规则缺席），MUST NOT 关闭保护闸、MUST NOT 退化为空规则集。
- **射程边界**：专用工具 `create_explore_note` MUST NOT 经保护闸（专用工具不在保护射程内为既有裁定，一致性缺口留痕于 desktop-write-protection）。
- **错误归属次序**：沙箱拒绝（`sandbox_denied`）SHALL 优先于保护拒绝（`permission_denied`）——逃逸路径先按沙箱报错，不被保护闸抢先。

#### Scenario: 四段顺序与拒绝形态

- **WHEN** BypassPermissions 档下模型调用 `write` 指向 `<root>/openspec/config.json`（路径在 root 内）
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied", payload:{tool:"write", reason:…}}` 与同 id 的 is_error ToolResult（内容为同一拒绝串）；`tools::execute` 未被调用（文件未被写）；run 不中断，同轮后续工具调用照常执行

#### Scenario: 沙箱先于保护闸

- **WHEN** 模型调用 `write` 指向 `<root>/../outside/openspec/config.json`（越出 workspace root）
- **THEN** 拒绝理由为沙箱成因（`subtype:"sandbox_denied"`），而非 `permission_denied`——两段闸的错误归属不重叠、不抢报

#### Scenario: 每 run 一次装配与 loop 零 IO

- **WHEN** 在一次 run 进行中修改 `openspec/config.json` 的 `write_protection`，随后同一 run 内继续调用工具
- **THEN** 本轮行为不随修改改变（规则在泵起播时已装配）；`sdk/loop.rs` 内无 `config::load` / `fs` / `canonicalize` 调用点

#### Scenario: 主仓口径与 worktree 执行根

- **WHEN** change 流程会话的执行根为 worktree（`SessionCtx.workspace_root` = worktree 路径），主仓 root 另经 builder 缝注入
- **THEN** 用户规则取自主仓 `config.json`；写 worktree 内 `openspec/config.json` 仍被内置规则拒（内置 glob 不区分主仓 / worktree）

#### Scenario: 未注入只内置

- **WHEN** 门面未注入 `workspace_root`（`None`）
- **THEN** 保护闸仍生效且只含内置规则（写工作区配置被拒），用户配置的 glob 不生效

#### Scenario: 专用工具直通

- **WHEN** explore 会话调用 `create_explore_note`，且用户配置保护了该工具的目标目录
- **THEN** 调用不被保护闸拦截（专用工具直通为知情边界），`write` / `edit` 对同目录的写入仍被拒

## MODIFIED Requirements

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报**本轮实际下发的工具面**（无注入会话即七工具；注入面见「会话级工具面注入与 explore 笔记工具」）：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理沿 `taskkill /T /F` 形状（原 `cli/runner.rs` `kill_process_tree` 同形，形状随 retire-cli-engine 迁驻 `sdk/bash.rs` 唯一持有；future drop 不自动杀子进程树）
- **超时**：缺省 120s；显式值须在 [1s, 30min] 内，越界显式拒绝（不静默钳位）——活性护栏，非安全护栏
- **知情边界（desktop-write-protection 部分偿还）**：bash 命令对**受保护文件**的写入面 SHALL 由 loop 守卫链的写入保护闸覆盖（重定向 / 删除 / 移动 / 内联脚本等写法的命令文本提取，见「loop 守卫链写入保护闸与工作区根注入缝」与 desktop-write-protection）；**任意命令执行仍无沙箱**——BypassPermissions 默认档下 bash 仍可执行任意命令、可改动任何未受保护文件；命令级白/黑名单、真沙箱、交互审批留后续 change（边界条款见「SDK 租户 MVP 边界与已知限制」）

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

#### Scenario: 受保护写入被拒而任意命令仍可执行

- **WHEN** BypassPermissions 档下先执行 `echo {} > openspec/config.json`，再执行 `echo {} > notes.txt`
- **THEN** 第一条被保护闸拒绝（`permission_denied`，run 不中断）；第二条正常执行——护栏只覆盖受保护文件面，不构成命令白/黑名单

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit / bash 拒绝；AcceptEdits 档 = + write / edit / bash；BypassPermissions 档 = 全放行。bash 档位语义（Default 拒绝、AcceptEdits / BypassPermissions 放行）为现有三档对第七工具的自然延伸——只读档不给执行权；**AcceptEdits 与 BypassPermissions 自此首次真实分化**（此前两臂行为完全一致）。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。policy SHALL 为纯决策表（档位 × 工具名 → 允许 / 拒绝）密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 是模型与 workspace root **之外**任意文件写之间的唯一护栏，其实现质量为必过关卡；workspace root **之内**的文件写另有第二道独立闸——写入保护闸（受保护文件面，见「loop 守卫链写入保护闸与工作区根注入缝」与 desktop-write-protection），二者射程不重叠：沙箱管「能否越出 root」，保护闸管「是否是受保护文件」。bash 进工具面后，**命令执行本身不在路径沙箱射程内**——路径沙箱不过问命令内容，作用于受保护文件的写入由保护闸经命令文本提取覆盖（含重定向 / 删除 / 移动 / 内联脚本）；未受保护文件的改动仍无护栏，真沙箱（job object / 容器）与交互审批留后续 change；命令级白/黑名单 MUST NOT 作为安全手段（假安全）。

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

#### Scenario: 两道闸并存不互相替代

- **WHEN** AcceptEdits 档下 root 内普通文件（未受保护）被写，以及 root 内受保护文件被写
- **THEN** 前者经沙箱放行后正常执行；后者沙箱放行、保护闸拒绝——沙箱不承担保护职责，保护闸不承担逃逸职责

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 无沙箱（sdk-engine-gap-closure 偿还「bash 缺席」后的知情边界；desktop-write-protection 偿还「受保护文件写入面」半边）**：bash 已进工具面（Default 档拒绝、AcceptEdits / Bypass 放行）；受保护文件（内置工作区配置 + 用户 `write_protection` 规则）的写入经命令文本提取拦截，**但任意命令执行仍无沙箱**——BypassPermissions 默认档下 bash 可执行任意命令、可改动任何未受保护文件；命令级白/黑名单、真沙箱、交互审批留后续 change；已承认的静态判别漏面（`xargs` / `sh -c` 递归拆解 / 生成脚本再执行 / `-m` 模块 / 变量中转 / base64 编码）留痕于 desktop-write-protection
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

#### Scenario: 第 4 条边界的精确分界

- **WHEN** 审查 bash 的护栏现状
- **THEN** 受保护文件写入面有护栏（保护闸）、任意命令执行与未受保护文件改动无护栏——两者在同一段边界文本内可分界，MUST NOT 出现「完全无保护」或「已沙箱化」两种失真的表述

#### Scenario: 兜底边界的失败面可见

- **WHEN** anthropic 协议下对「名单外 + 8K 输出上限」模型留空 `max_tokens`（取 32_000 兜底）发起运行
- **THEN** 请求以 provider 400 失败并经 `api_error` 走既有错误收敛（显式、可读诊断），不静默；用户填 `max_tokens = 8192` 后可跑通——该组合作为兜底策略边界的留痕证据

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/loop.rs` | 守卫链第四段 | 顺序 `成员校验 → policy → sandbox_rewrite → 写入保护闸 → 执行`；闸门读 `turn.protect.evaluate(name, input, cwd)`，命中走既有 `deny(sender, "permission_denied", name, reason, id)` 后 `continue`；零 IO、零新事件词汇 |
| `crates/infra/agent/src/sdk/loop.rs`（`LoopTurn`） | 规则携带 | 新增 `protect: ProtectRules` 字段（与 `defense` / `retry` / `liveness` 同列，装配点算好、loop 纯消费） |
| `crates/infra/agent/src/sdk/runner.rs` | 装配点 + root 持有 | `SdkRunner.workspace_root: Option<PathBuf>`；`with_workspace_root(self, PathBuf) -> Self`（与 `with_max_tokens` 同型旁路缝）；`PumpPayload.workspace_root`；泵起播时 `protect::load(payload.workspace_root.as_deref())` 装入 `LoopTurn` |
| `crates/infra/agent/src/lib.rs`（`EngineFacade`） | 门面缝 | `workspace_root: Option<PathBuf>` + `with_workspace_root`；`runner_for` sdk 臂透传给 `SdkRunner`；`EngineKind::Cli` 臂与唯一 match 点结构零改 |
| `crates/infra/agent/src/compose.rs` | 组合根签名 | `compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)`（新增尾参）；只做透传，零解析残留 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 调用点（exec 轨） | `compose_turn` 补 `root`（主仓 root；`exec_root` 仍只进 `SessionCtx.workspace_root`） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 调用点（change 轨） | 同上：`compose_turn` 补 `root`，MUST NOT 传 `exec_root`（worktree） |
| `crates/infra/agent/src/sdk/protect/**` | 规则与判定 | 契约见 desktop-write-protection「Module Contract」；本能力只负责接线，MUST NOT 在其内实现第二份 glob / 文案 |
| `crates/core/agent`、`crates/infra/agent/src/sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec}.rs` | 本变更零语义改动 | policy 三档矩阵 / 沙箱逃逸判定 / bash 执行体 / 专用工具执行面逐条不变；`SessionCtx` / `SessionOpen` / 事件词汇零扩展 |
