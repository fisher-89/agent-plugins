# desktop-checks-domain Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

### Requirement: 输出记录收敛到 infra/exec

stdout / stderr 记录职责 SHALL 从 checks 层收敛到 `crates/infra/exec`：`SystemCommand` 之外 SHALL 提供两种输出记录模式 `ExecOutput`：

- `ExecOutput::Transient`：spawn 即得、反馈边消费、即弃（无 id，全量不进库 / 不落盘引用）；
- `ExecOutput::Recorded { exec_id }`：stdout 落库 / 落盘、可回查（有 id 可查；`exec_id` 供 `RunNodeRecord.output_ref` 等引用，见 desktop-change-state-store「run 执行图落库」）。

`record_exec_output` SHALL 收敛为**单一 `.log`**（`.err` 退役）：单一交错流（static-check 的 `2>&1` 交错、test-execution 的 stdout+stderr 合并流）即完整还原终端，MUST NOT 再分文件落 stdout / stderr。`read_exec_output(root, change, exec_id)` SHALL 读同一 `.log`（未知 / 非法 `exec_id` → `None`，miss 非错误）。

static-check SHALL 由「默认 `Transient`」改为**默认 `Recorded`**（`run(root, name)` 收 name，`output_ref` 恒回填；反馈边仍消费 `diagnostics` 文本，全量经 `output_ref` 可回查）；test-execution 的 `report_dir` 与 `exec_id` SHALL 统一口径（报告全量仍落 change 报告目录，`exec_id` 为回查引用，record 内容为脚本真实输出——见 desktop-test-execution）。新增 desktop-app 命令 `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）暴露 `infra/exec::read_exec_output` 给前端 stdout 面板。agent 会话记录 SHALL 继续走 `SessionEventRecord`（`session_id` 重放唯一来源），MUST NOT 内嵌进 `ExecOutput`（不与转录单表不变量冲突）。`ExecOutput::Recorded` 的落库载体（独立 exec 输出表 vs 落盘文件 + 引用）由 design 定稿，但 MUST NOT 全文进 `RunNodeRecord.detail`（引用式与截断式口径不变）。

**运行中记录生命周期（本期扩写）**：两 runner 的记录落盘 SHALL 由既有一次性 `record_exec_output` 改为 `LiveExec` 三段链（见 desktop-exec-runtime「运行中实时输出记录与增量回查」），且 MUST 遵守下述顺序与口径：

- **顺序（硬约束）**：`LiveExec::begin`（建目录建空 `.log`）→ `write_command`（写命令 sidecar）→ **上抛 `exec_id`**（经 runner 的 `on_output_ref` 回调，见 desktop-change-orchestration「脚本步运行中输出引用前置上抛」）→ `run_shell(RunOptions { live: Some(live.sink()), .. })` → `commit(权威内容)`。上抛 MUST 发生在 live 文件已存在之后；该顺序是前端 watch 订阅可生效的前提，MUST NOT 以运行期退避重试兜底。
- **同 id 收口**：running 期上抛的 `exec_id` 与终态回填的 `output_ref` SHALL 为同一值；`commit` MUST NOT 另铸新 id。
- **命令 sidecar 写入时机与形态**：spawn **之前**写入（命令解析 / 模板展开已完成），纯文本、与 `.log` 同目录同 id 前缀（`<exec_id>.cmd`）。static-check 写**一条**（`{static_analysis} 2>&1`——与 spawn 命令串逐字一致，含 `2>&1`）；test-execution 写**每 plan 一段**，格式 `=== plan <id> · <framework> ===\n<展开后的命令>`（沿既有 plan 分隔头措辞，同一记录内分段可读）。写入失败 MUST NOT 阻断检查主流程（`Err` 静默降级，面板回查退化为「命令未记录」）。
- **无配置不建记录**：static-check 的「无配置 / 空命令直过」分支 MUST NOT 创建 live 记录（零 spawn ⇒ 无 live），其终态 `output_ref` 仍为 `None`。
- **被杀两臂仍收口**：超时 / 取消击杀进程后 SHALL 仍以 `commit` 写入已收字节——终态 `output_ref` 在位、面板可读部分输出（部分输出对用户可见优于空面）。
- **记录单点不破**：目录派生、`exec_id` 铸造、落盘写入仍 MUST 全部驻 `crates/infra/exec`；两 runner 只按上述顺序调用 `LiveExec` 的四个方法，MUST NOT 出现私有的 `fs::write` / 目录拼接 / id 生成。

#### Scenario: 两模式各司其职

- **WHEN** 某检查 spawn 产出仅需反馈边消费的 stdout，与某需回查的 spawn stdout
- **THEN** 前者可经 `ExecOutput::Transient` 即弃；后者以 `ExecOutput::Recorded { exec_id }` 可经引用回查全量；checks 层无私有 stdout 捕获分支

#### Scenario: 记录单点

- **WHEN** 审查 infra/exec 与 infra/checks 源码
- **THEN** stdout 捕获 / 落库 / 落盘逻辑单点驻 infra/exec；static-check / test-execution 经 `ExecOutput` 消费，无各自私有的捕获或落盘路径

#### Scenario: 会话记录不内嵌

- **WHEN** 审查 WorkerAgent 步的输出记录
- **THEN** 会话记录仍经 `session_id` 引用 `SessionEventRecord` 重放（转录单表不变量不变），MUST NOT 经 `ExecOutput` 内嵌全量会话

#### Scenario: 单一 .log 记录与回查

- **WHEN** static-check（2>&1 交错）或 test-execution（stdout+stderr 合并）产出一段输出并经 `record_exec_output` 落盘
- **THEN** 仅写 `<report_dir>/exec-outputs/<exec_id>.log`（无 `.err`）；`read_exec_output` 读回字节与该单一流逐字一致

#### Scenario: read_exec_output 命令 miss 与 lossy

- **WHEN** 前端以未知 / 非法 `exec_id` 调 `read_exec_output` 命令，或以已知 `exec_id` 调取含非 UTF-8 字节的输出
- **THEN** 前者返回 `None`（miss 非错误）；后者返回 UTF-8 lossy 字符串（非法字节以替换符呈现），不 panic、不报错

#### Scenario: static-check 运行中记录已可读

- **WHEN** static-check 命令开始输出但尚未退出（假命令分阶段写 stdout），检查其报告目录
- **THEN** `<report_dir>/exec-outputs/<exec_id>.log` 已存在且内容为迄今已写字节（连续读单调增长；收口时整体替换为权威内容，故不保证是终态内容的前缀）；`on_output_ref` 收到该 `exec_id` 的时刻该文件与父目录均在案

#### Scenario: 命令 sidecar 逐字可查

- **WHEN** 以 `static_analysis = "pnpm run -C ./packages/desktop check"` 执行 static-check，或以两个 suite 执行 test-execution
- **THEN** 前者 `<exec_id>.cmd` 内容逐字为 `pnpm run -C ./packages/desktop check 2>&1`；后者含两段，各以 `=== plan <id> · <framework> ===` 起手并紧随该 plan 展开后的命令；两文件均与 `.log` 同目录同 id 前缀

#### Scenario: 同 id 收口与权威覆盖

- **WHEN** 上抛的 running `exec_id` 与终态 emit 的 `output_ref` 比对，并读该 `.log`
- **THEN** 两者字符串相等；`.log` 终态内容逐字节等于 `attempts_log(&run)`（static-check）或 `merged_output`（test-execution）——即 live 交错流被同 id 覆盖为权威内容

#### Scenario: 无配置直过不建 live 记录

- **WHEN** workspace 未配置 `static_analysis`（或为空白）
- **THEN** 零 `LiveExec::begin`、`reports/<slot>/exec-outputs/` 零新增 `exec_id`、终态 `output_ref == None`；`passed == true`、`fault == None` 语义不变

#### Scenario: 取消与超时仍收口已收字节

- **WHEN** static-check / test-execution 因取消或超时被击杀
- **THEN** 仍以同一 `exec_id` `commit` 已收字节，`output_ref` 在位（static-check 的 `fault` 文案与 `passed` 口径不变，test-execution 走既有 `fault_outcome`）；面板可读到截断前的部分输出

#### Scenario: test-execution 命令在每 plan spawn 前落盘

- **WHEN** test-execution 依次执行 rust plan 与 vite-plus plan（第二个 plan 因超预算未 spawn）
- **THEN** sidecar 含已 spawn 那段命令（未 spawn 的 plan 零新段）；`.log` 的 plan 分隔头与 sidecar 的分段措辞一致可对读

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/checks`（新，裸名 `checks`） | 检查域纯层 | model / parser / aggregate / diagnose / reuse 纯函数；零 spawn 零 tokio process 零 Tauri；依赖 config + foundation；fixtures 直驱 + corpus 黄金可测 |
| `crates/infra/checks`（新，裸名 `checks-runtime`） | 检查域进程执行 | static_check.rs 平移（零逻辑改动）+ testexec/（detect / runner / report）；依赖 orchestration + checks + config + foundation + tokio process；零 Tauri |
| `orchestration::port`（原地不动） | runner port 归属 | `TestExecutionRunner` 与 `StaticCheckRunner` 并列；`ToolCommand::TestExecution` / `ToolStepOutput::TestExecution`（最小载荷）入封闭集；core/orchestration 零依赖 checks（**本轮两 trait 各扩一参 `on_output_ref: Option<OutputRefSink>`**——形态与上抛语义见 desktop-change-orchestration「脚本步运行中输出引用前置上抛」，本域只负责在文件已建后调用一次） |
| `crates/infra/agent`（裸名 `agent-runtime`，收窄） | 会话租户身份回归 | cli / sdk / worker / compose / store_port；static_check 移出 |
| `crates/core/foundation/src/`（名单单点，新） | 依赖与工具产物树名单 | 封闭目录名集 + dot 剪枝判定纯函数；公开共享；detect 与 vcs 两消费点同源 |
| `crates/infra/checks/src/static_check.rs` | static-check 槽位与记录生命周期 | 报告锚点按槽位参数化（test 缺省 / baseline）；执行语义零分叉；记录链 `LiveExec::begin → write_command("{command} 2>&1") → on_output_ref(exec_id) → run_shell(live) → commit(attempts_log)`；无配置直过分支零 live |
| `crates/infra/checks/src/testexec/`（runner / detect） | test-execution 槽位与记录链 | 报告锚点按槽位参数化；detect 剪枝改引 foundation 名单；`execute()` 内 `begin` 一次、逐 plan spawn 前 `write_command` 追加一段、`outcome` / `fault_outcome`（含复用门空记录分支）均以同 id `commit(&merged_output)`；`execute_plan(plan, report_dir, cancel, live)` 只把 sink 交给 `run_shell`，MUST NOT 自持记录生命周期、MUST NOT 自写 sidecar / commit |
| `crates/infra/vcs`（restore_baseline，新） | 基线恢复排除面 | 排除面 = foundation 名单 + `openspec/`；实现驻 infra（core 零 spawn 红线），经 `ArchiveVcsPort` 缝消费 |
| `crates/infra/checks/src/static_check.rs` | static-check 执行与装配 | `execute(root, name, slot, cancel, on_output_ref)` 经 `run_shell(command, RunOptions { cwd: root, timeout: 1200 s, cancel, retry: RetryPolicy::transient(), live: Some(sink) })`；命令串仍为 `{command} 2>&1`；`diagnostics = report.stdout`、`passed = exit_code == 0`、record 拼全部尝试（attempt 分隔头）；按 fault 装配表填 `StaticCheckOutcome.fault`；命中签名时调 `lockprobe::attribution_text` |
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行与归因 | `SUITE_COMMAND_TIMEOUT` 600 s → **1200 s**；`execute_plan` 经 `run_shell(cwd = plan.cwd, timeout, cancel, retry = RetryPolicy::transient(), live)`；`TimedOut` / 瞬态耗尽 → `fault = Environment(…)`（不产 plan `execution_error`）；`Cancelled` → `fault = Stopped`（不返回 `Err`）；`PlanExecution` 增 `fault` 并上抛；合并流「stdout 前缀 + stderr 后缀」零变；私有 `read_all` 退役 |
| `crates/infra/checks/src/testexec/mod.rs` | 结论与 fault 上传 | 多 plan fault 聚合（`Stopped` 优先）；`conclusion = Error` 成因面收窄为解析 / harness / 框架探测失败 |
| `crates/infra/checks/src/mutation/mod.rs` | 段执行失败面 | 不接 retry；`SegmentFailure::Cancelled` → `fault = Some(Stopped)`；`Interrupted` → `conclusion = Error` + `fault = None`；`Fatal` → `Err`；**本轮零 live 接入**（突变段不产输出记录，记录面扩张留待后续变更） |
