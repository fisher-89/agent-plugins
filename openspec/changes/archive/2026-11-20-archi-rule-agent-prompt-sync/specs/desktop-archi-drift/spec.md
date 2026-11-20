# desktop-archi-drift Specification (Delta)

## ADDED Requirements

### Requirement: 资产哈希（assets_hash）与报告 modelHash 的分工

判定域 SHALL 另提供一份**资产哈希** `assets_hash(root) -> String`（`archi::drift` 公开面），用于回答「本轮会话前后架构资产有没有变」这类消费侧问题（当前唯一消费者 = 引擎收口自检，见 `desktop-archi-agent`「收口前引擎自检（架构判据读数）」）：

- **覆盖面 SHALL 为四类架构资产**：模型声明（`graph/model/*.toml`）、契约资产（`models/**/*.md`）、契约 schema（`models/**/*.schema.json`，含与契约 md 同级的派生 schema）、契约索引（`graph/INDEX.toml`）。路径 SHALL 经 `foundation::layout` 派生，MUST NOT 行内字面量。
- **累加口径 SHALL 与 `model_hash` 同构**：按**相对路径排序**后逐文件累加「文件名 + 字节」，输出 `sha256:` 前缀的十六进制摘要——同一份盘面必得同一值，与文件遍历顺序 / 文件系统时间戳无关。
- **与 `modelHash` 的分工封闭**：报告字段 `modelHash` SHALL 继续只覆盖 `graph/model/*.toml`（口径逐字不变，仍是「手上报告是否对应当前模型」的判据）；`assets_hash` MUST NOT 被写进报告、MUST NOT 取代 `modelHash`、MUST NOT 被用作缓存失效依据（本期零缓存）。
- **零写盘、零 spawn**：本函数只读盘（判定域既有边界逐字不变），不落盘、不触发 git。
- **稳定与容错**：目录 / 文件缺失 SHALL 按「不参与累加」收敛（冷启动合法，MUST NOT 报错）；单文件读取失败 SHALL 以空字节参与（宁可哈希变化，MUST NOT 让消费侧因 IO 抖动失败）。

#### Scenario: 四类资产各改一处即变

- **WHEN** 在固定盘面上依次①改任一 `graph/model/*.toml` 一个字节、②改任一 `models/**/*.md`、③改任一同级 `*.schema.json`、④改 `graph/INDEX.toml`
- **THEN** 四种改动各自使 `assets_hash` 变化；未改动时连续两次调用（含跨进程重启）返回值逐字相同

#### Scenario: 与 modelHash 面不同

- **WHEN** 只改契约 md（`models/**/*.md`）而不动任何模型 toml，分别取报告 `modelHash` 与 `assets_hash`
- **THEN** `modelHash` 逐字不变（既有口径），`assets_hash` 变化——两者覆盖面差异即本条的存在理由

#### Scenario: 空域与缺目录

- **WHEN** 在无 `graph/`、无 `models/` 的空观察根上调用
- **THEN** 返回确定的摘要串（缺失按不参与累加），零错误、零 panic

## MODIFIED Requirements

### Requirement: 判定单源与消费面同源

架构漂移的判定 SHALL 只有一处实现（`crates/core/archi` 的漂移判定纯函数：输入 = 观察根 + 模式 + 变更集 + 配置 + 模型，输出 = 报告）。消费面 SHALL 为**五处**调用点——IPC 命令面（`archi_drift(root, mode)`）、implement 相位门禁 runner（增量）、initialize 基线门禁 runner（全量）、**引擎工具面（`archi_drift` 只读工具的恒全量调用）**、**引擎收口自检（archi 会话自然收敛前的恒全量调用）**——后两者为同一入参形态（全量 + 空变更集 + 无 change 上下文），前两者为**同一 runner 的两次挂载**（见 desktop-archi-gate「门禁挂载点」），MUST NOT 各写一份归属 / 定级 / 状态推导；报告结构体 SHALL 为全部消费面共用的唯一形状（命令面出线经同一 DTO 投影，门禁面从同一结构派生 `passed` / 诊断文本，引擎面从同一结构经`drift_summary` 投影出工具响应与 notice 载荷）。

- **引擎两面的入参恒定**：`mode = Full`、`changed = &[]`、`change = None`、`root = 会话执行根`；MUST NOT 传 `Incremental`（`infra/agent` 不依赖 `vcs`，取不到 `change_edit_set`；而 archi 臂与相位会话的 root 皆无 change 基准语义）、MUST NOT 由消费面自行拼报告。
- **引擎面零写盘**：调用方 MUST NOT 落盘报告（命令面同款裁定）；落盘者恒为门禁 runner（`reports/archi/<timestamp>.json`）。
- **取消除外**：引擎工具面与收口自检 SHALL 以 `CancelFlag::never()` 调用（不为几十毫秒扩取消通道）；`CheckError::Stopped` 对其不可达，出现即按失败面处置（`is_error` / `skipped` notice）。

#### Scenario: 命令面与门禁面逐字段同源

- **WHEN** 以同一 root、同一模式（增量）与同一变更集分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告除 `timestamp` / `change`（命令面无 change 上下文）与落盘路径外逐字段相等（`model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` 全等）

#### Scenario: 全量面同源

- **WHEN** 以同一 root 分别经 `archi_drift(root, "full")` 与 initialize 基线门禁、引擎工具面、引擎收口自检触发全量检查
- **THEN** 四份报告除 `timestamp` / `change` / 落盘与否外逐字段相等；`passed` 派生面（`errors.is_empty() && model.structuralErrors.is_empty()` + `error` 档判据）四处一致——MUST NOT 出现「工具面 clean 而门禁面红」的同根分歧

#### Scenario: 零第二套判定

- **WHEN** 审查命令面、infra runner、`sdk/archi_tool.rs` 的 `archi_drift` 分支与 `sdk/preclose.rs` 的源码
- **THEN** 五处调用点均只做「取输入 → 调判定纯函数 → 投影 / 落盘」，无归属匹配、无定级分支、无 status 推导的第二份实现；模式差异只体现为入参，不体现为实现分支；引擎两面的投影共用 `drift_summary` 单点

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/mod.rs` | 资产哈希（新增公开面） | `pub fn assets_hash(root: &Path) -> String`：覆盖面 = `graph/model/*.toml` + `models/**/*.md` + `models/**/*.schema.json` + `graph/INDEX.toml`（路径经 `foundation::layout` 派生）；稳定序「相对路径 + 字节」累加、`sha256:` 前缀；缺失目录 / 文件按不参与累加收敛、读失败按空字节参与；纯读盘、零写盘、零 spawn |
| `crates/core/archi/src/drift/mod.rs`（既有面零改动） | 判定单点 | `check(CheckInput) -> Result<Report, CheckError>` 与 `model_hash` 语义逐字不变；`modelHash` 仍只覆盖 `graph/model/*.toml`；新增调用面只改「谁调」，不改判定 |
| `crates/infra/agent/src/sdk/archi_drift_view.rs`（新，归属 infra） | 引擎侧投影单点 | `drift_summary` / `drift_entries` / `drift_files`：从同一 `Report` 逐级加字段；工具响应与收口 notice 共用 `summary`（notice 另加 `changedAssets`） |
| `crates/infra/agent/src/sdk/archi_tool.rs` / `sdk/preclose.rs` | 引擎两调用面 | 恒 `Full` + `changed: &[]` + `change: None` + `CancelFlag::never()`；零写盘（不落 `reports/archi/**`） |
| `crates/infra/archi/src/runner.rs` / `src-tauri/src/commands/archi/mod.rs` | 零改动 | 门禁 runner 两模式与命令面两模式语义逐字不变（本变更加的是引擎侧两处调用面，不减既有面） |

### Function: `crates/core/archi/src/drift/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `assets_hash` | 新增 | 四类资产聚合哈希；与 `model_hash` 共享累加 helper，覆盖面不同 |
| `model_hash` | 零改动 | 仍只覆盖 `graph/model/*.toml`；报告 `modelHash` 口径不变 |
| `check` / `derive_status` / `Report` | 零改动 | 判定、定级、`status` 推导与报告字段面逐字不变 |

### API: 引擎两调用面

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_drift` 工具分支 | 新增（详见 `desktop-archi-agent`） | `scope` 三值；恒全量；`cap_result` 收口 |
| `preclose::run` | 新增（详见 `desktop-archi-agent`） | 自然收敛前一次全量判定；失败面 `archi_preclose_skipped` |
