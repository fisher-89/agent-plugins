# desktop-checks-domain Specification (Delta)

## MODIFIED Requirements

### Requirement: infra/checks 进程执行层与 static_check 平移

`crates/infra/checks` SHALL 承载检查域进程执行：`static_check.rs`（ProcessStaticCheck）自 infra/agent 平移、零逻辑改动（config 读命令 → shell 语义 spawn → 诊断捕获 + 退出码映射；无配置 / 空命令直过；配置在位而程序不可达显式 `Err`）；`testexec/`（test-execution runner：框架探测、命令模板展开、spawn / 超时 / 输出捕获、报告写盘编排）。Windows shell shim（cmd /C）与程序解析前置检查（PATH / cwd 探测、可执行后缀候选——防 shell 吞命令缺失烧反馈边预算）SHALL 复用 static_check 既有经验。输出捕获 SHALL 经 infra/exec 的 `ExecOutput` 记录模式（见「输出记录收敛到 infra/exec」），checks 层 MUST NOT 保留私有 stdout 捕获 / 落盘分支。

#### Scenario: 平移零逻辑改动

- **WHEN** ProcessStaticCheck 及其测试自 infra/agent 迁入 infra/checks
- **THEN** 执行语义逐行等价（无配置直过 / 程序不可达显式 Err / 退出码映射不变），平移后测试全绿，infra/agent 无 static_check 残留

#### Scenario: Windows .cmd shim 可达

- **WHEN** runner 在 Windows 上执行 jest / vitest / npx 类 `.cmd` shim 命令
- **THEN** spawn 前完成程序解析前置检查（含可执行后缀候选），程序缺失显式 `Err` 停给用户而非落 passed=false 反馈边空转

## ADDED Requirements

### Requirement: 输出记录收敛到 infra/exec

stdout / stderr 记录职责 SHALL 从 checks 层收敛到 `crates/infra/exec`：`SystemCommand` 之外 SHALL 提供两种输出记录模式 `ExecOutput`：

- `ExecOutput::Transient`：spawn 即得、反馈边消费、即弃（无 id，全量不进库 / 不落盘引用）；
- `ExecOutput::Recorded { exec_id }`：stdout 落库 / 落盘、可回查（有 id 可查；`exec_id` 供 `RunNodeRecord.output_ref` 等引用，见 desktop-change-state-store「run 执行图落库」）。

static-check SHALL 由「临时（无 id，全量即丢）」升级为可选 `Recorded`（默认 `Transient`，反馈边消费 Transient 即够；需要审计 / 回查时升 `Recorded`）；test-execution 的 `report_dir` 与 `exec_id` SHALL 统一口径（报告全量仍落 change 报告目录，`exec_id` 为回查引用）。agent 会话记录 SHALL 继续走 `SessionEventRecord`（`session_id` 重放唯一来源），MUST NOT 内嵌进 `ExecOutput`（不与转录单表不变量冲突）。`ExecOutput::Recorded` 的落库载体（独立 exec 输出表 vs 落盘文件 + 引用）由 design 定稿，但 MUST NOT 全文进 `RunNodeRecord.detail`（引用式与截断式口径不变）。

#### Scenario: 两模式各司其职

- **WHEN** static-check spawn 产出 stdout（反馈边消费）与某 test-execution spawn 产出需回查的 stdout
- **THEN** 前者以 `ExecOutput::Transient` 即弃（反馈边拿到文本即可）；后者以 `ExecOutput::Recorded { exec_id }` 可经引用回查全量；checks 层无私有 stdout 捕获分支

#### Scenario: 记录单点

- **WHEN** 审查 infra/exec 与 infra/checks 源码
- **THEN** stdout 捕获 / 落库 / 落盘逻辑单点驻 infra/exec；static-check / test-execution 经 `ExecOutput` 消费，无各自私有的捕获或落盘路径

#### Scenario: 会话记录不内嵌

- **WHEN** 审查 WorkerAgent 步的输出记录
- **THEN** 会话记录仍经 `session_id` 引用 `SessionEventRecord` 重放（转录单表不变量不变），MUST NOT 经 `ExecOutput` 内嵌全量会话

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/exec`（扩展） | 输出记录单点 | `SystemCommand` 之外增 `ExecOutput::Transient` / `Recorded { exec_id }`；spawn 捕获 + 落库/落盘 + 回查；调用方（checks）零私有捕获 |
| `crates/infra/checks/src/static_check.rs` + `testexec/` | 输出消费升级 | static-check 默认 Transient / 可选 Recorded；test-execution `report_dir` 与 `exec_id` 统一；报告全量仍落 change 报告目录 |
| `crates/core/checks`（纯层，零触点） | 检查域纯层 | 报告 schema / parser / aggregate / diagnose / reuse 纯函数不变；不接触 `ExecOutput`（进程执行面归 infra） |
| `crates/infra/store` | 输出引用 | `RunNodeRecord.output_ref` 承载 `exec_id` 引用（见 desktop-change-state-store） |
