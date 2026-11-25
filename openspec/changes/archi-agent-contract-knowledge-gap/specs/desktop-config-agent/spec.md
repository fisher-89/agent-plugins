# desktop-config-agent Specification（delta: archi-agent-contract-knowledge-gap）

## Purpose

本 delta 只有一处**实现落位迁移**的登记追改：原子写单点 `write_atomic`（同目录临时文件 + rename，失败清临时文件）从壳层 `packages/desktop/src-tauri/src/commands/atomic.rs` 迁到 infra 侧 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic.rs`，命令层文件改薄委托。

动因：本 change 给 agent 面加两个模型写工具（`archi_element` / `archi_relation`），执行体在 `crates/infra/agent`（infra 不依赖壳层），而「模型写盘只有一份实现」这一既有承诺要求壳层命令面与 agent 写工具共用同一函数——故唯一实现必须落在 infra 侧可被两者共享的位置。

- **行为逐字不变**：签名 `(&Path, &str) -> Result<(), String>`、错误文案（含涉及路径与 IO 错误原文）、临时后缀 `.archi-tmp`、失败清临时文件、父目录缺失即 `Err` 不建目录、`fs::write` 唯一位点全部保留；`workspace_config_apply` 的三段命令体、零 `InvalidValue` 门、写后重读返回逐字不变。
- **登记随动**：Module Contract 的 `Component: .../src/commands/atomic.rs（新增）` 改为「（改）薄委托」，并新增 infra 侧 `Function` 条目；配置写面的其余条目（`to_file_value` 四面锚、`propose_config`、提案卡五态等）零改动。

## MODIFIED Requirements

### Requirement: 写面命令 `workspace_config_apply` 与写权边界

写回 `config.json` SHALL 只有一个入口：用户点击提案卡上的「应用」触发的 `workspace_config_apply(root, candidateFileJson)` 命令。

- **命令体三段**：`check root`（config 轨道内私有 fn：缺失 / 不可读 / 非目录 / 空白 → `Err`）→ `config::plan_apply_config(root, &candidate)`（纯函数、零 IO）→ `write_atomic(config_path(root), content)`，返回**重读**后的 `WorkspaceConfigReport`（前端直接替换，省一次往返）。
- **零 `InvalidValue` 门**：`plan_apply_config` SHALL 先 `validate(&candidate)`；存在任一 `InvalidValue` 即 `Err`（诊断逐条回传）且 **MUST NOT 产出待写内容**（失败零写入：文件内容与 mtime 双证不变）。`defaultApplied` 诊断 MUST NOT 拦截写回（它是正常形态）。
- **双校验不是冗余**：propose 侧的校验可能发生在很久以前、可能来自历史会话重放；apply 侧 MUST NOT 信任调用方（前端载荷可伪造）。
- **入参面**：SHALL 为 file-face `serde_json::Value`（出线 TS `unknown`），MUST NOT 用 typed `WorkspaceConfig` 作入参（wire / file 两面不同名，typed 入参即要求调用方按 camelCase 反向拼装）。
- **落盘形态**：`content` = `serde_json::to_string_pretty` + 尾换行；写入 SHALL 原子（临时文件 + rename），MUST NOT 直写目标文件留半截。
- **落盘单点与实现落位**：`write_atomic` 的**唯一实现** SHALL 位于 infra 侧 `crates/infra/agent/src/sdk/atomic.rs`（`fs::write` / `fs::rename` 的唯一位点）；壳层 `src/commands/atomic.rs` SHALL 只做薄委托——签名 `(&Path, &str) -> Result<(), String>`、错误文案、临时后缀 `.archi-tmp`、失败清临时文件语义、父目录缺失即 `Err` 不建目录逐字不变。壳层命令面（`archi_set_gate` / `workspace_config_apply`）与 agent 写工具 SHALL 共用这同一实现，MUST NOT 出现第二份「临时文件 + rename」。
- **写权边界**：`config.json` SHALL 保持「agent 不可写」的内置锁定（`desktop-write-protection` 的 `config.json` 内置锁定条款零改动；该能力本 change 只扩两写工具的保护射程与交叉引用，见其 delta）；`workspace_config_apply` MUST NOT 作为 agent 工具下发（模型不可调用、不可经 bash 绕过——bash 的写检测仍按内置锁拒绝）；apply 是用户点击触发的应用进程自写，**MUST NOT 注册 store 模型、MUST NOT 落库、MUST NOT 持有缓存**。
- **失败面**：读失败 / 校验失败 / 写失败 SHALL 返回 `Err(String)`（文案含原因），页面 inline 持久呈现；MUST NOT panic、MUST NOT 静默改动文件。

#### Scenario: 零 invalidValue 才落盘

- **WHEN** 以含越界阈值（或枚举外 `framework`）的候选调用 `workspace_config_apply`
- **THEN** 返回 `Err`、诊断逐条在文案内；目标文件内容与 mtime 逐字不变（零写入）

#### Scenario: 成功写回返回新报告

- **WHEN** 以合法候选调用 `workspace_config_apply`
- **THEN** 文件被整体替换为规范化 file-face 内容（pretty + 尾换行，原子写无残留），返回的报告来自**写后重读**（`diagnostics` 与盘面一致，无 `fileMissing`）

#### Scenario: agent 无法触发写面

- **WHEN** 审查 config 会话的工具面与 bash 写检测
- **THEN** 名字清单不含任何写工具，`workspace_config_apply` 不出现于任何 `<tool>` 定义；模型经 `bash` 重定向写 `config.json` 仍被写保护拒绝（内置锁定规则在案）

#### Scenario: 无效 root 走 reject

- **WHEN** 以不存在 / 非目录 / 空白 root 调用 `workspace_config_apply`
- **THEN** 返回 `Err(String)`，进程无 panic，任何文件零改动

#### Scenario: 落盘单点恰一份实现

- **WHEN** 审查 `write_atomic` 的两处定义与 `fs::write` / `fs::rename` 的命中面
- **THEN** infra 侧 `sdk/atomic.rs` 恰一份实现（`fs::write(&temporary` 与 `fs::rename(&temporary, path)` 各一处、含 `.archi-tmp`、零 `create_dir_all`）；壳层 `commands/atomic.rs` 零 `fs::write`、零 `fs::rename`、零 `create_dir_all`、只委托该符号；`archi_set_gate` 与 `workspace_config_apply` 调同一符号；错误文案与临时后缀逐字不变

## Module Contract

### Component: `packages/desktop/src-tauri/src/commands/atomic.rs`（改，薄委托）

| 项 | 变化 | 描述 |
|---|---|---|
| `write_atomic` | 实现改为委托 | 签名 `(&Path, &str) -> Result<(), String>` 与可见性 `pub(crate)` 不变；函数体转调 `crates/infra/agent` 的同一实现；零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`；错误文案与 `.archi-tmp` 后缀由被委托实现原样透传 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic.rs`（新增）

| 项 | 签名 | 描述 |
|---|---|---|
| `write_atomic` | `(path: &Path, content: &str) -> Result<(), String>` | 同目录临时文件 + rename 的原子写（`fs::write` / `fs::rename` 唯一位点；`.archi-tmp` 临时后缀、失败清临时文件、父目录缺失即 `Err` 不建目录——全部沿既有实现逐字保留） |

- **门面导出**：`crates/infra/agent/src/lib.rs` SHALL 以 `pub use sdk::atomic::write_atomic;` 导出该函数（`sdk` 模块本身仍私有），壳层经 `agent_runtime::write_atomic` 引用——命令层薄委托不得复制实现。

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `workspace_config_apply` 命令面 | 签名、三段命令体、零 `InvalidValue` 门、写后重读返回、错误面逐字不变（命令签名零 diff；`bindings.ts` 的唯一 diff 来自 `desktop-archi-agent` 的 `ArchiModuleDetail.fileHash` 字段） |
| `to_file_value` / `plan_apply_config` / `ConfigWrite` | 实现与四条机械锚（P1–P4）零改动；配置写面仍不引用 `ModelWrite.hash` |
| `propose_config` / 提案卡 `ConfigProposeCard` | 工具契约、五态与刷新纪律零改动 |
| agent 工具面 | config 会话仍不含任何写工具；本迁移不给 `config.json` 增加写面——agent 侧的模型写工具目标文件限在模型声明目录一级 `*.toml` |
