# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: 相位收口失败可辨与相位内系统异常仍标 failed

run 的失败面 SHALL 分两型且文案可辨，MUST NOT 混同：

- **相位内步骤系统异常**（`run_worker` 返回 `Err`、会话 `status = Failed` / `Stopped` / `Interrupted`、门禁 runner `Err`、vcs 步失败）：**该步节点 SHALL 呈 failed（或 stopped）**，run 随之 `failed`（或 `stopped`）。红节点是用户定位失败步的唯一入口，MUST NOT 为统一形态而把节点染成 passed。其中 `status = Interrupted` SHALL 归入 **failed 侧**（详见下文「`Interrupted` 臂的收口语义」）；`status = Stopped` 仍归 stopped 侧（用户主动停止），两值 MUST NOT 互换归属。
- **相位收口落账失败**（相位内全部步皆 passed，`settle_phase` 的 `phase_log` 返回 `Err`）：**节点 SHALL 保持全绿**（它们确实都通过了），run SHALL `failed`，终态 reason SHALL 以固定前缀「相位 `<phase>` 全部步通过但落账失败: `<err>`」点明成因，MUST NOT 呈现为步失败。

两型的 reason 文案 MUST 可区分（前者含步骤 / 会话语境，后者含「全部步通过但落账失败」前缀）。该降级（全绿 + failed）为「落账不再是可观测步」的显式代价，已接受；缓解手段是原因文案可归因。

**`Interrupted` 臂的收口语义**：会话状态面分类单点 `worker_settle` 的穷尽 `match` SHALL 为 `AgentRunStatus` 五变体各设**显式变体臂**（MUST NOT 以 `_` 兜底臂吞掉任何变体）——`Completed` → passed 无终态；`Failed` → failed 携成因折叠；`Stopped` → stopped「会话被终止」；`Running` 漂移 → failed「会话收口缺终态」；`Interrupted` → **failed「会话被中断」**（`StepSettle::failed` + `Terminal::failed`，步 detail 与 run 终态同文）。理由：`interrupted` 由启动标定写入（见 desktop-workspace-store「agent 轮启动标定与 running 残留终结」），语义是「本步没有可信产出、且不是用户主动停止」——归 stopped 侧会误导用户以为自己停的，归 passed 侧则让 run 带着未完成相位继续前进；故按失败收口并让红节点 + 文案把成因说清。本臂在生产路径**不可达**（进程中断时 walker 一并死亡，标定发生在下次启动），其存在是枚举扩值经编译器强制的显式裁决面，MUST NOT 因此为它在运行时新增对账 / 复活逻辑（MUST NOT 新增独立的中断终态通道，节点词汇与 `Terminal` / `StepSettle` 变体面零扩展）。

#### Scenario: 落账失败节点全绿

- **WHEN** 某相位全部计划步 passed 后 `log_phase` 返回 `Err`（store 故障注入）
- **THEN** 该相位全部节点终态为 passed；run 终态 `failed`，reason 前缀「相位 `<phase>` 全部步通过但落账失败」并携 `Err` 文案；状态库无该 attempt 的 PhaseRecord

#### Scenario: 步骤异常仍是红节点

- **WHEN** 相位内 executor 会话以 `status = Failed` 收场（API 错 / 模型错）
- **THEN** 该 executor 节点呈 failed 并携失败诊断摘要，run `failed`；MUST NOT 出现「节点全绿但 run 失败」的形态，MUST NOT 调用 `settle_phase`

#### Scenario: 两型文案对拍

- **WHEN** 分别触发（a）步骤系统异常与（b）相位收口落账失败
- **THEN** 两终态 reason 不含同一句引导语，（b）必含「全部步通过但落账失败」前缀，（a）不含该前缀

#### Scenario: 中断轮按失败语义收口

- **WHEN** 以 `WorkerTurnOutcome.status = Interrupted` 直驱会话状态面分类单点（`worker_settle_probe` 直调面；生产路径不可达，由枚举扩值强制覆盖）
- **THEN** 步终态拍为 failed、detail 为「会话被中断」，run 终态 `Terminal::failed` 且 reason 同文；MUST NOT 呈 passed（`Completed` 臂形态）、MUST NOT 呈 stopped（`Stopped` 臂形态），文案 MUST 与「会话被终止」可辨

#### Scenario: 五态分类与实现同源

- **WHEN** 审查 `crates/core/orchestration/src/walker.rs` 的 `worker_settle` 与本节「相位内步骤系统异常」枚举
- **THEN** `AgentRunStatus` 五变体各有显式臂、无 `_` 兜底（新增变体由编译器强制落臂）；本节枚举（`Failed` / `Stopped` / `Interrupted` 三值进系统异常面）与实现逐值同源，MUST NOT 出现第二份状态分类判据

## Module Contract

### Function: `crates/core/orchestration/src/walker.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `worker_settle` | 增 `Interrupted` 臂（签名零改动，`match` 仍穷尽、无 `_`） | `AgentRunStatus::Interrupted` → `(StepSettle::failed(Some("会话被中断")), Some(Terminal::failed("会话被中断")))`；臂位置紧邻 `Stopped` 便于对拍；函数 doc 注释与 `worker_settle_probe` doc 注释的「四态 / 四臂」字面同步扩为五态 / 五臂 |
| `worker_settle_probe` | **零签名改动** | 探针透传 `worker_settle` 的 `(StepSettle, Option<Terminal>)`，新臂经既有直调面自动可测（零新测试缝） |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/core/orchestration/src/port.rs`（`RunFault`） | 变体封闭集仍 `{Stopped, Environment(String)}`：中断不是门禁 runner 的产出（它是库态标定事实），MUST NOT 新增变体、MUST NOT 让 runner 产出它 |
| `Terminal` / `StepSettle` 变体面 | 零扩展：中断复用 failed 面，MUST NOT 新增 `Terminal::interrupted` / `StepSettle::interrupted` |
| `crates/core/orchestration/src/state.rs`（`ChangeStepStatus` / `ChangeRunStatus` / `RunUpdate`） | 状态词汇零扩展（节点 failed / run failed），前端流程视图零新词 |
| `crates/core/orchestration/src/ask.rs` + `control.rs`（`AskPort` / `ChangeFlowControl`） | 零改动：ask 停等态非 durable 的归属在 desktop-agent-execution「ask 工具协议与阻塞应答通道」与 desktop-workspace-store 标定，本能力不新增停等持久化 / 恢复面 |
| `crates/infra/store/**` | 零改动：标定与轮状态落 desktop-workspace-store「agent 轮启动标定与 running 残留终结」 |
| 相位机其余面（`enter_phase` / `settle_phase` / `drive` / `run_step` / `run_worker`） | 零改动：本 delta 只扩分类单点的一条臂，不改相位序列、不改窗口 / 决策 / 门禁任何分支 |

### Test: `crates/core/orchestration/src/walker_test.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `worker_settle四态分类与成因折叠与边界` | 改（含改名扩五态） | 增 `Interrupted` 段：`step_status = Failed` / `detail = "会话被中断"` / `terminal_status = Failed` / `terminal_reason == detail`；函数名与文件头注释的「四态」字面同步改五态；既有 `Completed` / `Failed` / `Stopped` / `Running` 四段断言逐字不动 |
