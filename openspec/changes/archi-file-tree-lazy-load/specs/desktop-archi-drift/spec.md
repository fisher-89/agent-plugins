# desktop-archi-drift Specification（delta: archi-file-tree-lazy-load）

## Purpose

本 delta 只给「判定单源与消费面同源」的**边界条**补一条已知限制并附一条 scenario：命令面命令体从 IPC 线程挪到阻塞池之后，窗口可交互，`CancelFlag::never()` 的直接后果从「天然限流最多一条」变成「重复触发会真正并行扫同一批文件」。

- 零判定语义变化：扫描面、归属、定级、豁免、报告结构、`status` 五值、`REPORT_VERSION`、`apiFindings` 纳管口径**逐字不动**。
- 零代码护栏：本变更加的是留痕，不是取消 / 去重 / 节流；取消通道仍不扩（`foundation::cancel` 的基础设施在案，扩面归独立变更）。
- 五处消费面（命令 / 两条门禁 runner / 引擎工具面 / 引擎收口自检）的划分与入参恒定条款**逐字不动**。

## MODIFIED Requirements

### Requirement: 判定单源与消费面同源

架构漂移的判定 SHALL 只有一处实现（`crates/core/archi` 的漂移判定纯函数：输入 = 观察根 + 模式 + 变更集 + 配置 + 模型，输出 = 报告）。消费面 SHALL 为**五处**调用点——IPC 命令面（`archi_drift(root, mode)`）、implement 相位门禁 runner（增量）、initialize 基线门禁 runner（全量）、**引擎工具面（`archi_drift` 只读工具的恒全量调用）**、**引擎收口自检（archi 会话自然收敛前的恒全量调用）**——后两者为同一入参形态（全量 + 空变更集 + 无 change 上下文），前两者为**同一 runner 的两次挂载**（见 desktop-archi-gate「门禁挂载点」），MUST NOT 各写一份归属 / 定级 / 状态推导；报告结构体 SHALL 为全部消费面共用的唯一形状（命令面出线经同一 DTO 投影，门禁面从同一结构派生 `passed` / 诊断文本，引擎面从同一结构经`drift_summary` 投影出工具响应与 notice 载荷）。

- **引擎两面的入参恒定**：`mode = Full`、`changed = &[]`、`change = None`、`root = 会话执行根`；MUST NOT 传 `Incremental`（`infra/agent` 不依赖 `vcs`，取不到 `change_edit_set`；而 archi 臂与相位会话的 root 皆无 change 基准语义）、MUST NOT 由消费面自行拼报告。
- **引擎面零写盘**：调用方 MUST NOT 落盘报告（命令面同款裁定）；落盘者恒为门禁 runner（`reports/archi/<timestamp>.json`）。
- **取消除外**：引擎工具面与收口自检 SHALL 以 `CancelFlag::never()` 调用（不为几十毫秒扩取消通道）；`CheckError::Stopped` 对其不可达，出现即按失败面处置（`is_error` / `skipped` notice）。命令面同款：`CancelFlag::never()`，**无取消 ⇒ 无 in-flight 去重、无节流**（下文「并发扫描」条）。
- **并发扫描（已知限制，零代码护栏）**：命令面在阻塞池执行后窗口可交互，重复触发 SHALL 产生**并行的全仓扫描**——连点刷新（`refreshTick` 变 ⇒ `<ArchitectureCanvas key>` 整块重挂 ⇒ 三个 hook 全重取）、反复切页、重挂载各起一条，每条各扫同一批文件并各产完整报告。本能力 MUST NOT 因此新增取消通道、MUST NOT 引入 in-flight 去重或节流、MUST NOT 为此调整 blocking 池配置；此类治理（取消 / 去重 / 增量触发）SHALL 归独立变更。并发扫描 MUST NOT 被读作回归：单次成本改造前后等同，变化只发生在「窗口挡不挡住第二次触发」。

#### Scenario: 命令面与门禁面逐字段同源

- **WHEN** 以同一 root、同一模式（增量）与同一变更集分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告除 `timestamp` / `change`（命令面无 change 上下文）与落盘路径外逐字段相等（`model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` 全等）

#### Scenario: 全量面同源

- **WHEN** 以同一 root 分别经 `archi_drift(root, "full")` 与 initialize 基线门禁、引擎工具面、引擎收口自检触发全量检查
- **THEN** 四份报告除 `timestamp` / `change` / 落盘与否外逐字段相等；`passed` 派生面（`errors.is_empty() && model.structuralErrors.is_empty()` + `error` 档判据）四处一致——MUST NOT 出现「工具面 clean 而门禁面红」的同根分歧

#### Scenario: 零第二套判定

- **WHEN** 审查命令面、infra runner、`sdk/archi_tool.rs` 的 `archi_drift` 分支与 `sdk/preclose.rs` 的源码
- **THEN** 五处调用点均只做「取输入 → 调判定纯函数 → 投影 / 落盘」，无归属匹配、无定级分支、无 status 推导的第二份实现；模式差异只体现为入参，不体现为实现分支；引擎两面的投影共用 `drift_summary` 单点

#### Scenario: 重复触发产生并行扫描（已知限制）

- **WHEN** 在命令面一次全量检查未完成时连点刷新两次，使两次检查的扫描窗口重叠
- **THEN** 两次检查各自完整执行并各产出一份逐字段等价（除 `timestamp`）的报告；命令面零取消（`CancelFlag::never()` 不变）、零去重、零节流；该形态在案为已知限制，MUST NOT 驱动出「为命令面加取消 / 去重」的顺带改动

## Module Contract

### 模块增量：并发扫描留痕（archi-file-tree-lazy-load）

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/drift/mod.rs` | 判定单点 | **零改动**：`check(CheckInput)` 的入参（含 `cancel`）与判定语义逐字不变；本 delta 只登记消费面的并发语义，不改判定 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs::archi_drift` | 命令面 | 命令**形态**翻为 `pub async fn` + `spawn_blocking`（纪律归 `desktop-app-shell`）；`archi_drift_inner` 的入参（`mode` / `&dyn ArchiveVcsPort` / `CancelFlag::never()`）与错误文案零改动；零去重、零节流 |
| `packages/desktop/src/views/architecture/architecture-view.tsx` | 刷新入口 | **零改动**：`refreshTick` → `key` 重挂的既有语义不变（刷新钮恒可点，不新增 `disabled` 或节流） |
| `crates/infra/archi/src/runner.rs` / `sdk/archi_tool.rs` / `sdk/preclose.rs` | 其余四处消费面 | **零改动**：门禁两模式与引擎两面的入参恒定条款不变 |

### 零改动面

| 项 | 说明 |
|---|---|
| 报告结构 / `status` 五值 / `REPORT_VERSION` / `apiFindings` 纳管口径 | 逐字不变（本 delta 零判定改动） |
| `crates/core/archi/tests/corpus/**` 的 16 份金样 | 零变化 |
| `foundation::cancel` 公开面 | 零扩展（不新增 `CancelFlag` 变体、不给命令面加取消入参） |
