# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报**本轮实际下发的工具面**（无注入会话即七工具；注入面见「会话级工具面注入与 explore 笔记工具」）：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理沿 `taskkill /T /F` 形状（原 `cli/runner.rs` `kill_process_tree` 同形，形状随 retire-cli-engine 迁驻 `sdk/bash.rs` 唯一持有；future drop 不自动杀子进程树）
- **超时**：缺省 120s；显式值须在 [1s, 30min] 内，越界显式拒绝（不静默钳位）——活性护栏，非安全护栏
- **调用意图参数 `intent`（定义面）**：`bash` 工具定义（挂载清单单源 `sdk/mount.rs`）SHALL 在 `parameters.properties` 增 `intent`（`type: "string"`）并 SHALL 将其并入 `parameters.required`（与 `command` 平级），使「每次调用都带一行意图」成为模型可见的协议要求；`intent` MUST NOT 声明 `maxLength`（长度是审美约束，加一个无执行体配合的上限只会造成假精确）。取值纪律与推荐长度 SHALL 在 `intent` 的字段 `description`（或工具级 `description`，二者分工单点、MUST NOT 各写一份独立措辞）中表达：一句话说明这条命令要干什么、折叠态用户只读这一行、建议不超过 30 字、写目的而非复述命令文本。其余六个内建工具定义 MUST NOT 变化。
- **调用意图参数 `intent`（入参闸）**：`sdk/bash.rs::execute` SHALL 在 `command` 校验**之后**（`resolve_timeout_ms` 之前）校验 `intent`——缺失 / 非字符串 SHALL `Err`（文案指明缺失或非字符串字段名 `intent`），`trim()` 后为空 SHALL `Err`（文案指明不得为空）；两类 Err 与既有 `command` 缺失 / 空白 Err 同形（执行体硬 `Err`、模型在同一 turn 内自纠重试）。校验序 SHALL 为 `command` 先报——`command` 与 `intent` 同缺时错误必须指向 `command`（既有错误语义与既有断言零漂移）。
- **长度软约束**：`intent` 超长（含远超 30 字的输入）MUST NOT 导致执行体 `Err`、MUST NOT 触发任何拒绝分支——长度只经 `description` 软提示，视觉截断由前端工具卡的省略链承载（见 desktop-agent-chat-infra「bash 工具卡折叠态意图行」）；若后续出现稳定的超长样本，升为硬闸属独立变更。
- **意图的消费边界**：`intent` SHALL 仅随 `ToolUseBlock.input`（`serde_json::Value` 原样）落转录与回放，`core/agent` 事件词汇 MUST NOT 扩展、store 落库形状 MUST NOT 变化（无新列、无编码改动）；执行、写保护裁定、路径沙箱、上下文剪裁 / compaction 全链 MUST NOT 消费 `intent`——保护闸的判定输入仍为 `command` 原文与会话执行根，`tools::input_path` 对 `bash` 仍恒为 `None`。
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

- **WHEN** BypassPermissions 档下先执行 `echo {} > .dev-team/config.json`，再执行 `echo {} > notes.txt`
- **THEN** 第一条被保护闸拒绝（`permission_denied`，run 不中断）；第二条正常执行——护栏只覆盖受保护文件面，不构成命令白/黑名单

#### Scenario: 定义面声明意图为必填

- **WHEN** 审查 `bash` 工具定义（挂载清单单源）
- **THEN** `parameters.properties` 含 `intent`（`type: "string"`）且无 `maxLength` 键，`parameters.required` 逐值含 `command` 与 `intent`（`timeout_ms` 仍不在其中）；工具定义文本点出「折叠态只读这一行 / 建议 ≤30 字 / 写目的不复述命令」，且该引导与 `intent` 字段 description 分工单点不重复

#### Scenario: 意图缺失或空白硬拒且命令优先

- **WHEN** 以 `{}`、仅带 `command`、`intent` 为非字符串（数字 / 对象 / `null`）、`intent` 为纯空白串四种输入调用 `bash` 执行体
- **THEN** 四者均以硬 `Err` 收口且文案分别指明 `intent` 缺失 / 非字符串、`intent` 不得为空；其中 `{}` 的报错指向 `command`（校验序命令先报），既有 `command` 缺失 / 空白用例断言零漂移

#### Scenario: 超长意图不拒绝

- **WHEN** 模型传入合规 `command` 与 200 字长 `intent`
- **THEN** 命令照常执行并回灌合并输出，执行体零 `Err`；长度提示仅存在于定义面文本，折叠态截断由前端承载

## Module Contract

### Module: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `generic_definition("bash")` | `properties` / `required` / `description` 扩位 | `properties` 增 `intent`（`type: "string"`，**无 `maxLength`**）；`required` 由 `["command"]` 改为 `["command", "intent"]`；工具级 `description` 增一句「每次调用必须给一行意图」的引导（与字段 description 分工单点）。其余六工具定义逐字不变，`TOOL_NAMES` 七件与注册序不变 |
| `ToolDefinition` / `definitions(...)` 装配面 | 零改动 | 签名、注入面（名字清单 / `finish` / `ask` 位）与 `RunStarted.tools` 上报语义零变化——本变更只动一个工具定义的字面内容 |

### Module: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `execute(root, input)` | 新增意图闸 | `command` 闸之后、`resolve_timeout_ms` 之前：`input.get("intent").and_then(Value::as_str)` 缺失 / 非字符串 → `Err`（文案含 `intent`）；`trim().is_empty()` → `Err`（文案含 `intent`）。不新增长度分支 |
| `resolve_timeout_ms` / `build_command` / `drain` / `merge_output` | 零改动 | 超时解析、底座组装、双管道排水与合并口径、退出码分支逐字不变 |
| 消费边界 | 只读 `command` / `intent` / `timeout_ms` | `intent` 仅用于校验，不进子进程命令行、不进 ToolResult、不进日志 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/infra/agent/src/sdk/protect/**` | 保护闸判定输入仍只有 `input["command"]` 与会话执行根；`intent` 不进判别链（出现 diff 即越界） |
| `crates/infra/agent/src/sdk/{sandbox.rs,tools.rs}` | 路径沙箱与 `tools::input_path` 零改动（bash 恒 `None`）；`tools::execute` 分派表零改动 |
| `crates/infra/agent/src/sdk/{context.rs,compact.rs}` | 不裁剪 tool input，不为 `intent` 加豁免规则 |
| `crates/infra/agent/src/sdk/policy.rs` | 三档决策表零改动（bash 档位语义不变） |
| `crates/core/agent/**` | `ToolUseBlock.input` 仍为 `serde_json::Value`，事件词汇与块词汇零扩展 |
| `crates/infra/store/**` | 转录与轮记录形状零变更（`intent` 藏在既有 input JSON 内） |
| `src/commands/**` | 命令层零改动（`intent` 是工具载荷字段，不出线 IPC 类型） |
