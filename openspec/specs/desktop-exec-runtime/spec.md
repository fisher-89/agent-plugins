# desktop-exec-runtime Specification

## Purpose

定义桌面端系统进程执行的统一执行体：`crates/infra/exec` 的 `run_shell`（超时 / 进程树击杀 / 双管道并发排水 / 取消四能力单点）、`foundation::cancel` 取消原语（零 tokio 下沉），以及超时预算、输出解读面与未迁移 spawn 面的边界。执行体服务检查域四类外部进程（static-check / test-execution suite / cargo-mutants 段 / Stryker 段）；agent 边界的 bash 工具与 vcs 边界的 git 步不属本执行体消费面。

## Requirements

### Requirement: 统一执行体 run_shell 契约

`crates/infra/exec` SHALL 提供单一进程执行体 `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`，作为 infra 层 tokio spawn 的唯一收口：

- `RunOptions` SHALL 为 `{ cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration, cancel: CancelFlag }`；`timeout` **MUST 为必填**（MUST NOT 设缺省值 / MUST NOT 提供 `Default`）；`cancel` 缺省语义为「永不取消」（`CancelFlag::never()` 由调用方显式传或经 `Default` 表达）。
- `RunOutcome` SHALL 为封闭三值 `Completed` / `TimedOut` / `Cancelled`；`RunReport` SHALL 为 `{ outcome, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> }`，`TimedOut` / `Cancelled` 的 `exit_code` MUST 为 `-1` 哨兵（MUST NOT 回落 `0`）。
- 执行体 MUST 以 `kill_on_drop(true)` spawn（future 被 drop 时兜底终止），MUST 双管道并发排水（stdout 与 stderr 各一路 `read_to_end` 并发进行，防单管道填满互锁）。
- `cwd` / `env` MUST 经 `RunOptions` 施加（命令规格 `SystemCommand` 只表达 program / argv / 平台标志），故 `SystemCommand` SHALL 可 `Clone`。
- 执行体 MUST NOT 吞掉「进程无退出码」情形：无可读退出码时 `exit_code` 同记 `-1`。

#### Scenario: 正常退出三值收口

- **WHEN** `run_shell` 执行一条退出码 0 的脚本行，且 stdout / stderr 各有内容
- **THEN** 返回 `RunOutcome::Completed`、`exit_code == 0`、`stdout` / `stderr` 各自逐字节等于进程写入内容（零改写零截断）

#### Scenario: 双管道并发排水不互锁

- **WHEN** 被 spawn 进程先向 stderr 写入超过管道缓冲容量的数据、再向 stdout 写入超过管道缓冲容量的数据
- **THEN** 两路数据均完整读尽、执行体不悬挂于任一路管道

#### Scenario: 超时与取消的退出码哨兵

- **WHEN** 执行体因超时到期或取消置位而终止进程
- **THEN** `RunReport.exit_code` 为 `-1`，且 `outcome` 分别为 `TimedOut` / `Cancelled`（MUST NOT 记为 `Completed` 且 `exit_code == 0`）

#### Scenario: cwd 与 env 施加

- **WHEN** 以 `RunOptions { cwd: Some(dir), env: vec![(key, value)] }` 执行回显脚本
- **THEN** 子进程工作目录为 `dir`、环境变量 `key=value` 可见（脚本回显逐字断言）

### Requirement: 超时必填与超时预算表

执行体的 `timeout` MUST 为必填参数——「无超时 spawn」在类型层不可表达，以杜绝检查域历史上「cargo 卡在文件锁等待时永久悬挂」的缺陷复发。四个消费点的超时预算 SHALL 为代码常量（MUST NOT 进 config schema、MUST NOT 作为用户可配项）：

| 消费点 | 预算 |
|---|---|
| static-check | 1200 s |
| test-execution suite | 1200 s |
| cargo-mutants 段 | 1800 s |
| Stryker 段 | 1800 s |

预算取值口径 SHALL 为「实测耗时 × 约 3 倍余量」；调整预算属代码常量变更（评审可见），不属配置面变更。

#### Scenario: 零超时不可表达

- **WHEN** 审查 `RunOptions` 定义与 `run_shell` 签名
- **THEN** `timeout` 为必填字段（无缺省值、无 `Default` 实现、无「None 即无限」分支）；任何调用点都必须显式给出预算

#### Scenario: 预算常量锚定

- **WHEN** 审查四个消费点的超时常量
- **THEN** 分别锚定为 1200 s / 1200 s / 1800 s / 1800 s，且各以单元测试常量断言钉住

#### Scenario: 到点强杀不悬挂

- **WHEN** 被 spawn 进程永不退出且持续占用管道
- **THEN** 预算到期后执行体收敛为 `TimedOut`，墙钟耗时不超预算 + 有限收尾时间（不悬挂）

### Requirement: 进程树击杀与残留收敛

超时与取消两臂 MUST 经 `exec::kill_tree(pid)` 终止**整棵进程树**并 `wait` 收尸（MUST NOT 仅 `start_kill()` 杀壳进程）：Windows 经 `taskkill /PID <pid> /T /F` 连子孙一并终止；unix 侧为既有 `kill -9 <pid>` 语义（直接子进程；进程组补强属独立变更，不在本要求内）。收尸后排水任务 MUST 以管道关闭后的 EOF 收口，MUST NOT 悬挂于未关闭管道。

检查域 MUST NOT 残留裸 `tokio::time::timeout` + `start_kill` 组合，也 MUST NOT 保留各调用点私有的管道排水助手（`read_all` 一类）。

#### Scenario: 孙进程一并回收

- **WHEN** 被 spawn 的壳进程派生子进程与孙进程（各自把 pid 写入文件），随后触发超时或取消
- **THEN** Windows 上壳进程与全部子孙进程均终止（pid 文件记录的进程在到期后可观测为不存在），执行体完成收尸并返回 `TimedOut` / `Cancelled`

#### Scenario: 收尸后不悬挂于管道

- **WHEN** 超时 / 取消触发 kill 后，被测进程曾打开两路管道
- **THEN** 执行体在有限时间内返回（排水任务以 EOF 结束），MUST NOT 无限等待管道关闭

#### Scenario: 源码无裸超时杀壳组合

- **WHEN** 审查检查域四个 spawn 点的源码
- **THEN** 无 `tokio::time::timeout` 与 `start_kill` 组合、无私有 `read_all` 实现；管道排水与杀树逻辑单点驻 `exec`

### Requirement: 取消原语零 tokio 下沉

取消信号 SHALL 以 `foundation::cancel` 提供：`CancelFlag`（`Clone`，共享同一底层原子标志；`never()` 构造恒 false 的零值；`is_set()` 读）与 `CancelHandle`（`set()` 写）。该原语 MUST 为零依赖、零 tokio 实现（`foundation` 的「不依赖任何 workspace 内 crate」纪律不变），故执行体的取消观测 MUST 为轮询（200 ms 间隔）而非异步通知。

下沉裁决留痕：`foundation` 的「第二个消费者到场才下沉」纪律在此正触发——两消费者为编排侧 `RunGuard`（写）与执行体 `run_shell`（读）。

#### Scenario: never 恒 false

- **WHEN** 以 `CancelFlag::never()` 作为执行体的 `cancel` 且运行中从不置位
- **THEN** `is_set()` 恒为 false，执行体走到正常完成臂（`Completed`），零取消行为

#### Scenario: handle 置位后 clone 共享可见

- **WHEN** 取 `handle: CancelHandle` 与其对应 `flag: CancelFlag`，克隆 flag 后经 handle `set()`
- **THEN** 原 flag 与克隆 flag 的 `is_set()` 均为 true（共享同一原子，非值拷贝）

#### Scenario: foundation 零 tokio

- **WHEN** 审查 `foundation` 的 `Cargo.toml` 与 `cancel` 模块源码
- **THEN** 依赖面仍无任何 workspace 内 crate、无 tokio；取消原语仅用 `std::sync::atomic` 与 `Arc`

### Requirement: 输出解读面与未迁移 spawn 面边界

本执行体 SHALL NOT 改变既有输出解读面与未迁移的 spawn 面：`exec::ProcessOutput`（`error_excerpt` / `tail` / `failure_message`）语义零变；`record_exec_output` / `read_exec_output` / `mint_exec_id` 零变；调用方的 stdout / stderr 合并顺序 MUST 由调用方自行决定（执行体只给两路原始字节）。

以下 spawn 面 MUST NOT 迁移进本执行体（各有独立语义）:

- agent 边界的 bash 工具（`crates/infra/agent/src/sdk/bash.rs`）——`[stderr]` 标记合并口径与 `[1 s, 600 s]` 超时钳位属 agent 租户语义，本执行体只是其**形态参考**；
- `bootstrap`（`crates/infra/vcs/src/bootstrap.rs`：worktree 建置安装命令，自带 900 s 轮询 + `child.kill()`，属 vcs 边界）；
- testexec 框架版本探测（std 短命令）；
- vcs 边界 git 步（`bare` argv 直传，无壳包装）。

#### Scenario: ProcessOutput 语义零变

- **WHEN** 以非零退出码与合并输出构造 `ProcessOutput` 并调 `failure_message`
- **THEN** 「错误信息 + 输出末尾（50 行）」双段装配逐字等于本变更前行为（执行体只产字节，不改解读面）

#### Scenario: 合流顺序归调用方

- **WHEN** 某消费点（test-execution / mutation）需要「stdout 前缀 + stderr 后缀」的单一流
- **THEN** 该顺序由调用方在拿到 `RunReport` 后自行拼接，执行体不预合流；`2>&1` 类 shell 级合流优先级仍归调用方命令串

#### Scenario: 未迁移面不受影响

- **WHEN** 审查 agent bash 工具、`bootstrap`、版本探测与 vcs git 步
- **THEN** 四者实现与契约零改动（不消费 `run_shell`），各自既有测试全绿

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/exec/src/run.rs`（新） | 统一执行体 | `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`：spawn（`kill_on_drop`）→ 双管道并发排水 → `select!`（取消轮询 200 ms / timeout / `wait`）→ `kill_tree` + 收尸 |
| 同上 | 执行入参 | `RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration /* 必填 */, cancel: CancelFlag }` |
| 同上 | 执行产出 | `RunOutcome { Completed, TimedOut, Cancelled }`；`RunReport { outcome, exit_code: i32 /* TimedOut/Cancelled 为 -1 */, stdout: Vec<u8>, stderr: Vec<u8> }` |
| `crates/infra/exec/src/lib.rs` | 导出面 | `SystemCommand` 加 `#[derive(Clone)]`；`pub use run::{run_shell, RunOptions, RunReport, RunOutcome}`；`kill_tree` / `ProcessOutput` / `record_exec_output` 语义零变 |
| `crates/core/foundation/src/cancel.rs`（新） | 取消原语 | `CancelFlag::{never, is_set}`（`Clone` 共享原子）、`CancelHandle::set`；std-only、零 tokio |
| `crates/infra/exec/Cargo.toml` | 依赖面 | tokio 特性 `process` + `io-util` + `time` + `macros`（不引 `rt-multi-thread` / `full`）；dev：`rt` / `macros` / `test-util` / tempfile |
