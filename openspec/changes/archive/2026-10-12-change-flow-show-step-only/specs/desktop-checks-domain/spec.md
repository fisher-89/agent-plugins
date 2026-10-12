# desktop-checks-domain Specification (Delta)

## MODIFIED Requirements

### Requirement: infra/checks 进程执行层与 static_check 平移

`crates/infra/checks` SHALL 承载检查域进程执行：`static_check.rs`（ProcessStaticCheck）自 infra/agent 平移，本轮**启用 recorded 输出记录并改为 `2>&1` 交错合并**；`testexec/`（test-execution runner：框架探测、命令模板展开、spawn / 超时 / 输出捕获、报告写盘编排）。Windows shell shim（cmd /C）与程序解析前置检查（PATH / cwd 探测、可执行后缀候选——防 shell 吞命令缺失烧反馈边预算）SHALL 复用 static_check 既有经验。输出捕获 SHALL 经 infra/exec 的 `ExecOutput` 记录模式（见「输出记录收敛到 infra/exec」），checks 层 MUST NOT 保留私有 stdout 捕获 / 落盘分支。

static-check 平移后的语义变更（其余平移等价项不变——无配置 / 空命令直过、配置在位而程序不可达显式 `Err`、退出码映射）：

- **`2>&1` 交错合并**：spawn 前把脚本改为 `{command} 2>&1`，stderr 并入 stdout 单一管道，两 fd 指向同一管道、写入顺序即真实顺序；`diagnostics = output.stdout`（交错，更还原终端），`output.stderr` 为空。
- **默认 recorded**：`ProcessStaticCheck` 去 `record_anchor` 构造，`StaticCheckRunner::run(root, name)` 收 name，`execute(root, name)` 直接以 `record_exec_output(root, name, output.stdout)` 落单一 `.log` 并回填 `output_ref`（不再有 `recorded()` 装配分支）。

#### Scenario: static-check 2>&1 交错合并

- **WHEN** static-check 命令同时向 stdout 与 stderr 交错写入（如先 stdout 一行、再 stderr 一行、再 stdout 一行）
- **THEN** spawn 后的 `output.stdout` 为三行按真实顺序的单一交错流、`output.stderr` 为空；`diagnostics` 逐字等于该交错流，MUST NOT 为「stdout 全量 + stderr 全量」的分离拼接

#### Scenario: static-check 默认 recorded

- **WHEN** 组合根以 `ProcessStaticCheck::new()` 装配并调用 `run(root, name)`
- **THEN** `StaticCheckOutcome.output_ref` 为 `Some(exec_id)`（落盘失败降级 `None`，不阻断检查主流程）；`<report_dir>/exec-outputs/<exec_id>.log` 在案且内容等于交错 stdout，无 `<exec_id>.err` 文件

#### Scenario: Windows .cmd shim 可达

- **WHEN** runner 在 Windows 上执行 jest / vitest / npx 类 `.cmd` shim 命令
- **THEN** spawn 前完成程序解析前置检查（含可执行后缀候选），程序缺失显式 `Err` 停给用户而非落 passed=false 反馈边空转

### Requirement: 输出记录收敛到 infra/exec

stdout / stderr 记录职责 SHALL 从 checks 层收敛到 `crates/infra/exec`：`SystemCommand` 之外 SHALL 提供两种输出记录模式 `ExecOutput`：

- `ExecOutput::Transient`：spawn 即得、反馈边消费、即弃（无 id，全量不进库 / 不落盘引用）；
- `ExecOutput::Recorded { exec_id }`：stdout 落库 / 落盘、可回查（有 id 可查；`exec_id` 供 `RunNodeRecord.output_ref` 等引用，见 desktop-change-state-store「run 执行图落库」）。

`record_exec_output` SHALL 收敛为**单一 `.log`**（`.err` 退役）：单一交错流（static-check 的 `2>&1` 交错、test-execution 的 stdout+stderr 合并流）即完整还原终端，MUST NOT 再分文件落 stdout / stderr。`read_exec_output(root, change, exec_id)` SHALL 读同一 `.log`（未知 / 非法 `exec_id` → `None`，miss 非错误）。

static-check SHALL 由「默认 `Transient`」改为**默认 `Recorded`**（`run(root, name)` 收 name，`output_ref` 恒回填；反馈边仍消费 `diagnostics` 文本，全量经 `output_ref` 可回查）；test-execution 的 `report_dir` 与 `exec_id` SHALL 统一口径（报告全量仍落 change 报告目录，`exec_id` 为回查引用，record 内容为脚本真实输出——见 desktop-test-execution）。新增 desktop-app 命令 `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）暴露 `infra/exec::read_exec_output` 给前端 stdout 面板。agent 会话记录 SHALL 继续走 `SessionEventRecord`（`session_id` 重放唯一来源），MUST NOT 内嵌进 `ExecOutput`（不与转录单表不变量冲突）。`ExecOutput::Recorded` 的落库载体（独立 exec 输出表 vs 落盘文件 + 引用）由 design 定稿，但 MUST NOT 全文进 `RunNodeRecord.detail`（引用式与截断式口径不变）。

#### Scenario: 两模式各司其职

- **WHEN** 某检查 spawn 产出仅需反馈边消费的 stdout，与某需回查的 spawn stdout
- **THEN** 前者可经 `ExecOutput::Transient` 即弃；后者以 `ExecOutput::Recorded { exec_id }` 可经引用回查全量；checks 层无私有 stdout 捕获分支

#### Scenario: 单一 .log 记录与回查

- **WHEN** static-check（2>&1 交错）或 test-execution（stdout+stderr 合并）产出一段输出并经 `record_exec_output` 落盘
- **THEN** 仅写 `<report_dir>/exec-outputs/<exec_id>.log`（无 `.err`）；`read_exec_output` 读回字节与该单一流逐字一致

#### Scenario: read_exec_output 命令 miss 与 lossy

- **WHEN** 前端以未知 / 非法 `exec_id` 调 `read_exec_output` 命令，或以已知 `exec_id` 调取含非 UTF-8 字节的输出
- **THEN** 前者返回 `None`（miss 非错误）；后者返回 UTF-8 lossy 字符串（非法字节以替换符呈现），不 panic、不报错

#### Scenario: 会话记录不内嵌

- **WHEN** 审查 WorkerAgent 步的输出记录
- **THEN** 会话记录仍经 `session_id` 引用 `SessionEventRecord` 重放（转录单表不变量不变），MUST NOT 经 `ExecOutput` 内嵌全量会话

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/static_check.rs` | static-check 进程执行 | `ProcessStaticCheck` 去 `record_anchor` 构造；`run(root, name)`；spawn `2>&1` 交错；`diagnostics` = 交错 stdout；`record` = 单一 `.log`、`output_ref` 回填 |
| `crates/infra/checks/src/testexec/` | test-execution 进程执行 | runner `PlanExecution.output` 合并流；mod 聚合按 plan 序落单一 `.log`（record 内容见 desktop-test-execution） |
| `crates/infra/exec/src/lib.rs` | 输出记录单点 | `record_exec_output(root, change, output)` 单一 `.log`（`.err` 退役）；`read_exec_output(root, change, exec_id)` 读同一 `.log`；`ExecOutput::Transient` / `Recorded` 两模式不变 |
| `orchestration::port`（原地不动） | runner port 归属 | `StaticCheckRunner::run(root, name)` 与 `TestExecutionRunner::run(root, name)` 同型；`StaticCheckOutcome.output_ref` 语义不变（恒回填） |
| `src/commands/changes/mod.rs` | IPC 读面 | `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）；经 tauri-specta 再生出线 bindings |
