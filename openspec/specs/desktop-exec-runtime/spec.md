# desktop-exec-runtime Specification

## Purpose

定义桌面端系统进程执行的统一执行体：`crates/infra/exec` 的 `run_shell`（超时 / 进程树击杀 / 双管道并发排水 / 取消四能力单点）、`foundation::cancel` 取消原语（零 tokio 下沉），以及超时预算、输出解读面与未迁移 spawn 面的边界。执行体周边另含两项单点能力：瞬态签名字典与有限重试策略（`transient`，锁冲突类失败的自愈重试）与锁占用精确探针（`lockprobe`，占用者点名归因），供检查域装配环境错误文案。执行体服务检查域四类外部进程（static-check / test-execution suite / cargo-mutants 段 / Stryker 段）；agent 边界的 bash 工具与 vcs 边界的 git 步不属本执行体消费面。

## Requirements

### Requirement: 统一执行体 run_shell 契约

`crates/infra/exec` SHALL 提供单一进程执行体 `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`，作为 infra 层 tokio spawn 的唯一收口：

- `RunOptions` SHALL 为 `{ cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration, cancel: CancelFlag, retry: RetryPolicy }`；`timeout` **MUST 为必填**（MUST NOT 设缺省值 / MUST NOT 提供 `Default`）；`cancel` 缺省语义为「永不取消」（`CancelFlag::never()` 由调用方显式传或经 `Default` 表达）；`retry` 缺省语义为「单次执行」（`RetryPolicy::none()`）。
- `RunOutcome` SHALL 为封闭三值 `Completed` / `TimedOut` / `Cancelled`；`RunReport` SHALL 为 `{ outcome, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>, attempts: u32, retried: Vec<AttemptTrace> }`，`TimedOut` / `Cancelled` 的 `exit_code` MUST 为 `-1` 哨兵（MUST NOT 回落 `0`）。
- 执行体 MUST 以 `kill_on_drop(true)` spawn（future 被 drop 时兜底终止），MUST 双管道并发排水（stdout 与 stderr 各一路 `read_to_end` 并发进行，防单管道填满互锁）。
- `cwd` / `env` MUST 经 `RunOptions` 施加（命令规格 `SystemCommand` 只表达 program / argv / 平台标志），故 `SystemCommand` SHALL 可 `Clone`。
- 执行体 MUST NOT 吞掉「进程无退出码」情形：无可读退出码时 `exit_code` 同记 `-1`。
- **尝试面**：`attempts` SHALL 为本命令实际尝试次数（≥ 1）；`retried` SHALL 只承载**此前**各次尝试（末次不入 `retried`）；`stdout` / `stderr` MUST 仍为**末次尝试**的输出字节——既有消费点（诊断 / 合并流 / 工件解析）的读法零变。`RetryPolicy::none()` 或未命中瞬态签名时 SHALL 为 `attempts == 1 && retried.is_empty()`，其行为 MUST 与单次执行逐字节等价。
- 重试循环 MUST 复用同一执行骨架（同一 spawn / 排水 / 三臂竞速 / 杀树语义），MUST NOT 另写第二条 spawn 路径；每次尝试 MUST 同样观测 `cancel`。

#### Scenario: 正常退出三值收口

- **WHEN** `run_shell` 执行一条退出码 0 的脚本行，且 stdout / stderr 各有内容
- **THEN** 返回 `RunOutcome::Completed`、`exit_code == 0`、`stdout` / `stderr` 各自逐字节等于进程写入内容（零改写零截断），`attempts == 1`、`retried` 为空

#### Scenario: 双管道并发排水不互锁

- **WHEN** 被 spawn 进程先向 stderr 写入超过管道缓冲容量的数据、再向 stdout 写入超过管道缓冲容量的数据
- **THEN** 两路数据均完整读尽、执行体不悬挂于任一路管道

#### Scenario: 超时与取消的退出码哨兵

- **WHEN** 执行体因超时到期或取消置位而终止进程
- **THEN** `RunReport.exit_code` 为 `-1`，且 `outcome` 分别为 `TimedOut` / `Cancelled`（MUST NOT 记为 `Completed` 且 `exit_code == 0`）

#### Scenario: cwd 与 env 施加

- **WHEN** 以 `RunOptions { cwd: Some(dir), env: vec![(key, value)] }` 执行回显脚本
- **THEN** 子进程工作目录为 `dir`、环境变量 `key=value` 可见（脚本回显逐字断言）

#### Scenario: 单次语义零变

- **WHEN** 调用点以 `RetryPolicy::none()`（或命令未命中任何瞬态签名）执行
- **THEN** `attempts == 1`、`retried` 为空，`outcome` / `exit_code` / `stdout` / `stderr` 与本变更前逐字节一致，零额外 spawn、零额外退避等待

#### Scenario: 末次输出仍非合流

- **WHEN** 命令命中瞬态签名并重试至第 3 次后成功退出
- **THEN** `stdout` / `stderr` 恰为第 3 次尝试的两路原始字节（执行体不预合流、不拼接历史尝试），前两次尝试的字节只在 `retried` 中按尝试序可读

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

本执行体 SHALL NOT 改变既有输出解读面与未迁移的 spawn 面：`exec::ProcessOutput`（`error_excerpt` / `tail` / `failure_message`）语义零变（口径见下）；`record_exec_output` / `read_exec_output` / `mint_exec_id` 零变；调用方的 stdout / stderr 合并顺序 MUST 由调用方自行决定（执行体只给两路原始字节）。

`ProcessOutput` 三方法的口径 SHALL 为：

- `error_excerpt()`：提取前 10 个错误行组——每组 = 命中错误词表的行 + 后续至多 3 行非空续行（遇空行止；已被前组消费的行不再开新组）；组间以 `…` 行分隔；组数触 10 上限且仍有未扫行时尾加 `…`；无命中返回空串。错误词表为模块内封闭常量：`error` / `unable to` / `panic` / `exception` / `throw` / `failed` / `npm err!` / `traceback`（行内小写包含即命中）。
- `tail()`：取末尾 50 行（行口径，MUST NOT 回退为字符截断——旧「末 500 字符」口径会把 JS 错误首行截掉）；不足 50 行全给；截断时首行加 `…`；逐行剥尾随 `\r`；空输出返回空串。
- `failure_message(head)`：`head` +（非空时）「错误信息」段 +（非空时）「输出末尾（50 行）」段；空输出逐字等于 `head`（空面无冒号缀语义）；`error_excerpt` 与 `tail` 允许重叠、不去重。

以下 spawn 面 MUST NOT 迁移进本执行体（各有独立语义）:

- agent 边界的 bash 工具（`crates/infra/agent/src/sdk/bash.rs`）——`[stderr]` 标记合并口径与 `[1 s, 600 s]` 超时钳位属 agent 租户语义，本执行体只是其**形态参考**；
- `bootstrap`（`crates/infra/vcs/src/bootstrap.rs`：worktree 建置安装命令，自带 900 s 轮询 + `child.kill()`，属 vcs 边界）；
- testexec 框架版本探测（std 短命令）；
- vcs 边界 git 步（`bare` argv 直传，无壳包装）。

#### Scenario: ProcessOutput 语义零变

- **WHEN** 以非零退出码与合并输出构造 `ProcessOutput` 并调 `failure_message`
- **THEN** 「错误信息 + 输出末尾（50 行）」双段装配逐字等于本变更前行为（执行体只产字节，不改解读面）

#### Scenario: 错误主因与末尾行口径

- **WHEN** 合并输出含多个错误行组且总行数超过 50
- **THEN** `error_excerpt` 给前 10 个错误行组（每组错误行 + 至多 3 行非空续行、遇空行止，组间 `…`）；`tail` 只给末尾 50 行（截断时首行 `…`）；`failure_message` 双段可重叠、不内联去重

#### Scenario: 合流顺序归调用方

- **WHEN** 某消费点（test-execution / mutation）需要「stdout 前缀 + stderr 后缀」的单一流
- **THEN** 该顺序由调用方在拿到 `RunReport` 后自行拼接，执行体不预合流；`2>&1` 类 shell 级合流优先级仍归调用方命令串

#### Scenario: 未迁移面不受影响

- **WHEN** 审查 agent bash 工具、`bootstrap`、版本探测与 vcs git 步
- **THEN** 四者实现与契约零改动（不消费 `run_shell`），各自既有测试全绿

### Requirement: 瞬态签名字典与有限重试策略

执行体 SHALL 提供瞬态判定与重试策略的单点实现（`crates/infra/exec/src/transient.rs`）：

- **触发判据**：瞬态判定 MUST 同时满足「`outcome == Completed`」**且**「`exit_code != 0`」**且**「合并输出（`stdout` 与 `stderr` 两路，逐行包含匹配，大小写不敏感）命中签名字典中任一条」。`TimedOut` / `Cancelled` MUST NOT 命中（超时重试会把 1200 s 变数小时；取消必须立即生效）。
- **签名字典** SHALL 为以下封闭表（行内包含匹配）：

| 族 | 签名 |
|---|---|
| Windows 文件锁 | `os error 32`、`os error 5`、`另一个程序正在使用此文件`、`being used by another process`、`failed to rename archive file`、`failed to remove file`、`Access is denied` |
| MSVC 链接器 | `LNK1104` |
| POSIX / Node | `EBUSY`、`ETXTBSY`、`Text file busy`、`EPERM: operation not permitted`、`resource busy or locked` |

- **刻意不收**裸 `failed to write`：该措辞过于宽泛（会把真实写盘缺陷误判为瞬态）；rustc 的 `.rmeta` 写失败已由同一行内的 `os error 32` / `另一个程序正在使用此文件` 覆盖。字典扩展 SHALL 经本模块一处修改（MUST NOT 在调用点各写一份特征串）。
- **`RetryPolicy`** SHALL 为 `{ max_attempts: u32, backoff: [Duration; 4], total_budget: Duration }`，以代码常量提供 `transient()`（`max_attempts = 5` 即首跑 + 4 次重试，`backoff = [1 s, 2 s, 4 s, 8 s]`，`total_budget = 1500 s`）与 `none()`（单次执行）两构造；参数 MUST NOT 进 config schema。
- **预算硬顶**：`total_budget` SHALL 自**首跑开始时刻**起算；预算判定 MUST 在**每次重试尝试开始前**进行（`elapsed >= total_budget` → 不再发起新尝试，把末次结果原样返回：`outcome = Completed` + 非零退出码，由调用方按瞬态命中断言 fault）——MUST NOT 无限重试、MUST NOT 把预算拆成「每次尝试各一份」。MUST NOT 在尝试进行中因预算到期强杀进程（那是 `timeout` 的职责）；故墙钟上界为 `total_budget + 单次尝试预算`（有界，非无限）。
- **尝试面**：每次尝试 SHALL 记 `AttemptTrace { exit_code, transient: Option<String>, stdout, stderr }`（`transient` 为命中的签名原文，未命中为 `None`）；重试间的退避等待 MUST 同样观测 `cancel`（置位即刻收敛 `Cancelled`，不先等完退避）。
- 执行体 MUST NOT 自带日志格式（尝试分隔头 / 归因文案归调用方），只提供尝试面字节。

#### Scenario: flaky 锁冲突重试自愈

- **WHEN** 假命令第 1、2 次以「`failed to rename archive file` + `os error 32`」文本退出 1、第 3 次退出 0，且以 `RetryPolicy::transient()` 执行
- **THEN** `outcome == Completed`、`exit_code == 0`、`attempts == 3`；`retried` 长度 2，两条各携 `exit_code == 1` 与命中的签名原文（`os error 32`），退避按 1 s / 2 s 生效

#### Scenario: 持续锁冲突耗尽重试

- **WHEN** 假命令每次都输出同一锁文本并退出 1，以 `RetryPolicy::transient()` 执行
- **THEN** `attempts == 5`（不再增加）、`retried` 长度 4、末次 `outcome == Completed` 且 `exit_code == 1`；全程墙钟不超过 `total_budget`（虚拟时钟断言至多 1500 s）

#### Scenario: 超时与取消零重试

- **WHEN** 命令以 `TimedOut` 或 `Cancelled` 收场（输出中含任一签名文本，或 `retry` 为 `transient()`）
- **THEN** `attempts == 1`、`retried` 为空；MUST NOT 因输出文本命中签名而追加尝试

#### Scenario: 真红零重试

- **WHEN** 命令以「普通编译错误」文本退出 1（不命中任何签名，如 `error[E0308]: mismatched types`），且 `retry = transient()`
- **THEN** `attempts == 1`、`retried` 为空、`outcome == Completed`；重试策略在位但不触发，调用方可据此判「检查红」

#### Scenario: 裸 failed to write 不命中

- **WHEN** 输出仅含 `failed to write /tmp/x`（无 `os error` 码、无锁措辞）
- **THEN** 瞬态判定为否（`attempts == 1`），MUST NOT 因该短语单独触发重试

#### Scenario: 退避期间取消即刻生效

- **WHEN** 第 1 次尝试命中签名、执行体正处于重试退避等待，此时 `cancel` 置位
- **THEN** 执行体不再发起下一次尝试，即刻以 `Cancelled` 收口（不先等完剩余退避），`attempts` 记已完成次数

#### Scenario: 预算耗尽停止重试

- **WHEN** 每次尝试耗时 450 s 并持续命中签名（`transient()` 策略，`max_attempts = 5`）
- **THEN** 第 4 次尝试结束时已用约 1807 s（含退避）→ 第 5 次尝试开始前预算判定为耗尽，不再发起新尝试，`attempts == 4`；末次结果原样返回，总墙钟不超过 `total_budget + 单次尝试预算`

### Requirement: 锁占用精确探针与占用者点名

`crates/infra/exec` SHALL 提供锁占用诊断探针（`crates/infra/exec/src/lockprobe.rs`），产出「被锁文件 + 占用者清单」的归因原料，供检查域装配环境错误文案：

- **判据（只问「是否阻塞 rename / delete」）**：探针 MUST 以 `DELETE | FILE_READ_ATTRIBUTES` 访问权 + `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` 全共享模式打开目标文件一次并立即关闭。打开成功 = 可 rename / delete（未阻塞）；打开失败且 OS 错误码为 `32`（`ERROR_SHARING_VIOLATION`）= 存在不允许删除共享的持有者（与 rustc / cargo 失败同条件）。探针 MUST NOT 独占打开、MUST NOT 写入 / 截断 / 修改时间戳——它 MUST NOT 自己成为 `os error 32` 的成因。
- **点名（Windows）**：SHALL 经 Restart Manager 归因——`RmStartSession` → `RmRegisterResources([被锁文件])` → `RmGetList` → 取 `RM_PROCESS_INFO[].strAppName` + `RM_UNIQUE_PROCESS.dwProcessId`，按 `(进程名, pid)` 去重后输出；session MUST 在返回前 `RmEndSession`。
- **降级（非 Windows）**：SHALL 以 `lsof` 为点名手段（存在则用、不存在即跳过）；`lsof` 不可用 MUST NOT 阻断主流程、MUST NOT 视为错误。
- **空清单**：`RmGetList` 返空或 Restart Manager 不可用时，归因结果 SHALL 为「未定位占用者」并保留被锁文件清单（附「可能为内核过滤驱动（安全软件），无法归因到用户进程」语义），MUST NOT 因此判「无锁」、MUST NOT 阻断检查主流程。
- **路径解析**：探针 SHALL 从诊断文本按三种错误串形态提取被锁文件路径——`failed to rename archive file: <path>`、`failed to write <path>`、`failed to remove <path>`（均大小写不敏感）；同一诊断中多路径 SHALL 去重且有界（至多 8 条）。
- **触发时机**：调用方 MUST 仅在「非零退出 + 命中瞬态签名」时调用探针（常态路径零开销）；探针自身失败 MUST 静默降级（只丧失归因文本，不改变瞬态 / fault 判定）。
- **归因文本**：探针 SHALL 提供单点装配函数产出人读文本段（占用进程清单 / 被锁文件清单 / 「未定位占用者」降级说明），供检查域拼进 fault 文案与终局文案（MUST NOT 由各调用点各写一遍格式）。

#### Scenario: 共享读句柄不误报

- **WHEN** 另一句柄以「读权限 + `share = READ | WRITE | DELETE`」打开目标文件，探针对同一文件执行判据
- **THEN** 判据为「未阻塞」（打开成功）——旧「独占打开探测」把正常读句柄误报成锁的形态 MUST NOT 复现

#### Scenario: 无删除共享即真阻塞

- **WHEN** 另一句柄以「读权限 + `share = READ`（不含 delete）」打开目标文件，探针执行判据
- **THEN** 探针打开失败、OS 错误码为 `32`，判据为「阻塞」（与 rustc / cargo 的失败同条件）

#### Scenario: 自锁自证与释放

- **WHEN** 本进程以不含 delete 共享的方式打开一个临时文件并调用探针点名，随后释放该句柄再次调用
- **THEN** 前者点名结果含本进程自身（`进程名 (pid)` 与当前进程一致）；后者返回空清单（「未定位占用者」），两次调用均不 panic、不返回错误

#### Scenario: 三种错误串路径解析

- **WHEN** 诊断文本分别含 `failed to rename archive file: D:\w\target\debug\deps\liba.rlib`、`failed to write D:\w\target\libb.rmeta`、`failed to remove D:\w\target\c.rcgu.o`
- **THEN** 三次解析各得一个对应路径（去重、POSIX / 原生分隔符按输入原样保留），无多余条目

#### Scenario: 非 Windows 无 lsof 不阻断

- **WHEN** 在非 Windows 平台调用点名且 `lsof` 不在 PATH 上
- **THEN** 归因结果为「未定位占用者」，调用方照常装配 fault 文案；MUST NOT 报错、MUST NOT 阻断检查主流程

#### Scenario: 探针只读不扰动

- **WHEN** 审查探针实现对目标文件的操作
- **THEN** 仅存在一次「DELETE + 全共享」打开后立即关闭的调用；零 `create` / `truncate` / `write` / 独占共享模式，零残留句柄

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/exec/src/run.rs`（新） | 统一执行体 | `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`：spawn（`kill_on_drop`）→ 双管道并发排水 → `select!`（取消轮询 200 ms / timeout / `wait`）→ `kill_tree` + 收尸；外层按 `RetryPolicy` 循环（复用同一 `attempt_once`） |
| 同上 | 执行入参 | `RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration /* 必填 */, cancel: CancelFlag, retry: RetryPolicy }` |
| 同上 | 执行产出 | `RunOutcome { Completed, TimedOut, Cancelled }`；`RunReport { outcome, exit_code: i32 /* TimedOut/Cancelled 为 -1 */, stdout, stderr, attempts: u32, retried: Vec<AttemptTrace> }`（`stdout` / `stderr` = 末次尝试） |
| `crates/infra/exec/src/transient.rs`（新） | 瞬态判定与重试策略 | `RetryPolicy::{none, transient}`（`max_attempts` / `backoff` / `total_budget`）、`AttemptTrace { exit_code, transient: Option<String>, stdout, stderr }`、`is_transient(&RunReport) -> Option<&'static str>`；签名表封闭单点 |
| `crates/infra/exec/src/lockprobe.rs`（新） | 锁占用诊断 | `locked_paths(diagnostics: &str) -> Vec<PathBuf>`（三形态解析、去重、≤8）、`lock_holders(paths: &[PathBuf]) -> Vec<LockHolder>`（Windows Restart Manager / 非 Windows `lsof` 降级）、`LockHolder { name: String, pid: u32 }`、`attribution_text(diagnostics: &str) -> Option<String>`（占用者 + 被锁文件 + 降级说明单点装配） |
| `crates/infra/exec/src/lib.rs` | 导出面 | `SystemCommand` 加 `#[derive(Clone)]`；`pub use run::{run_shell, RunOptions, RunReport, RunOutcome}`；`pub use transient::{AttemptTrace, RetryPolicy}`；`pub use lockprobe::{locked_paths, lock_holders, LockHolder, attribution_text}`；`kill_tree` / `ProcessOutput` / `record_exec_output` 语义零变 |
| `crates/infra/exec/Cargo.toml` | 依赖面 | 新增 `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }`（仅 Restart Manager 绑定；精确探针用 std `OpenOptionsExt` 的 `access_mode` / `share_mode`，不引 FileSystem / Security feature）；非 Windows 目标零新增编译依赖 |
| `crates/core/foundation/src/cancel.rs` | 取消原语 | 零改动（`CancelFlag::{never, is_set}` / `CancelHandle::set`） |
| 既有输出解读面 | 边界 | `ProcessOutput` / `record_exec_output` / `read_exec_output` / `mint_exec_id` / `kill_tree` 零改动；尝试分隔头与归因文案的落文格式归调用方 |
| `crates/core/foundation/src/cancel.rs`（新） | 取消原语 | `CancelFlag::{never, is_set}`（`Clone` 共享原子）、`CancelHandle::set`；std-only、零 tokio |
| `crates/infra/exec/Cargo.toml` | 依赖面 | tokio 特性 `process` + `io-util` + `time` + `macros`（不引 `rt-multi-thread` / `full`）；dev：`rt` / `macros` / `test-util` / tempfile |
