# desktop-exec-runtime Specification

## Purpose

定义桌面端系统进程执行的统一执行体：`crates/infra/exec` 的 `run_shell`（超时 / 进程树击杀 / 双管道并发排水 / 取消四能力单点）、`foundation::cancel` 取消原语（零 tokio 下沉），以及超时预算、输出解读面与未迁移 spawn 面的边界。执行体周边另含两项单点能力：瞬态签名字典与有限重试策略（`transient`，锁冲突类失败的自愈重试）与锁占用精确探针（`lockprobe`，占用者点名归因），供检查域装配环境错误文案。执行体服务检查域四类外部进程（static-check / test-execution suite / cargo-mutants 段 / Stryker 段）；agent 边界的 bash 工具与 vcs 边界的 git 步不属本执行体消费面。

## Requirements

### Requirement: 统一执行体 run_shell 契约

`crates/infra/exec` SHALL 提供单一进程执行体 `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`，作为 infra 层 tokio spawn 的唯一收口：

- `RunOptions` SHALL 为 `{ cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration, cancel: CancelFlag, retry: RetryPolicy, live: Option<LiveSink> }`；`timeout` **MUST 为必填**（MUST NOT 设缺省值 / MUST NOT 提供 `Default`）；`cancel` 缺省语义为「永不取消」（`CancelFlag::never()` 由调用方显式传或经 `Default` 表达）；`retry` 缺省语义为「单次执行」（`RetryPolicy::none()`）；`live` 缺省语义为「纯内存、零实时观察」（`None`）。
- `RunOutcome` SHALL 为封闭三值 `Completed` / `TimedOut` / `Cancelled`；`RunReport` SHALL 为 `{ outcome, exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8>, attempts: u32, retried: Vec<AttemptTrace> }`，`TimedOut` / `Cancelled` 的 `exit_code` MUST 为 `-1` 哨兵（MUST NOT 回落 `0`）。
- 执行体 MUST 以 `kill_on_drop(true)` spawn（future 被 drop 时兜底终止），MUST 双管道并发排水（stdout 与 stderr 各一路**并发**进行，防单管道填满互锁）。排水 SHALL 为**chunk 循环读尽至 EOF**（单次块读 8 KiB 量级）：每读到一块即①追加进该路内存缓冲（既有消费面）、②在 `live` 在位时 tee 给观察者；读失败 MUST 按「已读即输出」收口（MUST NOT 上抛为 `RunError`），逐字节等价于此前 `read_to_end` 形态。
- **实时观察面零侵入**：`live == None` 时排水行为 MUST 与本变更前逐字节等价（零额外缓冲、零额外写）；`live == Some(sink)` 时 MUST 以到达顺序把**该路**字节逐个 chunk 交给 sink，且 sink 的阻塞 MUST NOT 引入新的无界等待（被杀两臂的排水仍以 `FINISH_GRACE` 截断，见下条）。
- **自然退出臂的有界排水**：子进程退出后，若孤儿孙进程仍持有继承的管道写端（Windows 上 cargo-mutants 超时只杀 `cargo test`、遗留测试可执行文件即典型），收尸完成后排水 MUST 以 `FINISH_GRACE` 截断收口，MUST NOT 等孙进程退出才返回。该异常面下 `stdout` / `stderr`（及 live 面已 tee 的字节）可能少掉尾字节（换取不悬挂），此为唯一允许的截断面；正常树输出仍 MUST 零截断。
- `cwd` / `env` MUST 经 `RunOptions` 施加（命令规格 `SystemCommand` 只表达 program / argv / 平台标志），故 `SystemCommand` SHALL 可 `Clone`。
- 执行体 MUST NOT 吞掉「进程无退出码」情形：无可读退出码时 `exit_code` 同记 `-1`。
- **尝试面**：`attempts` SHALL 为本命令实际尝试次数（≥ 1）；`retried` SHALL 只承载**此前**各次尝试（末次不入 `retried`）；`stdout` / `stderr` MUST 仍为**末次尝试**的输出字节——既有消费点（诊断 / 合并流 / 工件解析）的读法零变。`RetryPolicy::none()` 或未命中瞬态签名时 SHALL 为 `attempts == 1 && retried.is_empty()`，其行为 MUST 与单次执行逐字节等价。
- 重试循环 MUST 复用同一执行骨架（同一 spawn / 排水 / 三臂竞速 / 杀树语义），MUST NOT 另写第二条 spawn 路径；每次尝试 MUST 同样观测 `cancel`。重试循环 MUST 把同一个 sink 交给每次尝试（多尝试面下 sink 收到的是全部尝试字节按序，权威内容仍由调用方在收口时一次性写入）。

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

#### Scenario: live 缺省逐字节等价

- **WHEN** 以 `live: None` 执行与既有一致的示例命令（含双路并发、超时、取消、重试四类用例）
- **THEN** `RunReport` 全字段与本变更前逐字节一致，零额外文件 IO；既有排水用例组全绿，无需改断言

#### Scenario: live 在位收到同一份字节

- **WHEN** 以 `live: Some(sink)` 执行一条向 stdout 写若干行、向 stderr 写若干行的命令，sink 把字节追加进共享缓冲
- **THEN** 该缓冲内容与 `RunReport.stdout`（在 `2>&1` 单管道形态下）或两路拼接（双管道形态下，按到达顺序交错）一致；`stdout` / `stderr` 本身仍逐字节等于进程写入内容

#### Scenario: tee 不引入无界等待

- **WHEN** 子进程被超时 / 取消击杀后仍有孙进程持有管道写端，且 `live: Some(sink)` 在位
- **THEN** 排水仍以 `FINISH_GRACE` 截断收口并返回，墙钟上界与 `live: None` 路径一致；sink 收到截断前的字节，MUST NOT 因 sink 而挂起执行体

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

超时与取消两臂 MUST 终止**整棵进程树**并 `wait` 收尸（MUST NOT 仅 `start_kill()` 杀壳进程）。Windows 侧的回收 MUST 以作业对象（Job Object）为准：`spawn` 返回后**立即** `AssignProcessToJobObject` 把直接子进程挂进新建作业，被杀两臂以 `TerminateJobObject` 整组终止；`taskkill /PID <pid> /T /F` 可作幂等补刀，MUST NOT 作为唯一回收手段。unix 侧为既有 `kill -9 <pid>` 语义（直接子进程；进程组补强属独立变更，不在本要求内）。

作业成员资格 MUST NOT 依赖父子关系快照：`taskkill /T` 按调用时刻的父子关系向下遍历，msys（Git for Windows bash）fork 副本可能换过父 pid 而被报成功却漏杀；漏杀者继承执行体的管道写端，排水永不 EOF，进而钉死 tokio 阻塞线程池（runtime 析构无界等待）。作业挂载 MUST 紧贴 `spawn`（assign 之前创建的后代不进作业），且作业句柄 MUST NOT 设置 `KILL_ON_JOB_CLOSE`——自然退出臂的「孤儿孙进程存续、排水有界截断」语义不受影响。

作业对象 SHALL 兼任**异常展开路径**的兜底：执行体走不到任何收口代码时（future 被 drop——调用方 abort / panic 展开 / 外层 `select` 取消），`kill_on_drop` 只杀直接子进程，孙进程仍会带着继承的管道写端存活。故 `JobTree` SHALL 在 `Drop` 上整组终止，且仅在**未解除武装**时生效——自然退出臂须先解除武装（`disarm`），使孤儿孙进程按前款语义存续；被杀两臂显式 `terminate` 后同样处于解除武装态（幂等，`Drop` 不重复终止）。执行体 MUST NOT 依赖 `kill_on_drop` 作为孙进程回收手段。

收尸后排水任务 MUST 以管道关闭后的 EOF 收口，MUST NOT 悬挂于未关闭管道。

检查域 MUST NOT 残留裸 `tokio::time::timeout` + `start_kill` 组合，也 MUST NOT 保留各调用点私有的管道排水助手（`read_all` 一类）。

#### Scenario: 快照外的孙进程同样被回收

- **WHEN** 被 spawn 的进程在超时前已创建后台孙进程，且该孙进程的父子关系在击杀时刻被改写
- **THEN** 超时 / 取消两臂结束后该孙进程 MUST 已消失（作业成员资格不随父子关系改写失效）

#### Scenario: 孙进程一并回收

- **WHEN** 被 spawn 的壳进程派生子进程与孙进程（各自把 pid 写入文件），随后触发超时或取消
- **THEN** Windows 上壳进程与全部子孙进程均终止（pid 文件记录的进程在到期后可观测为不存在），执行体完成收尸并返回 `TimedOut` / `Cancelled`

#### Scenario: future 被 drop 时孙进程同样被回收

- **WHEN** 执行体 future 在子进程仍在运行时被 drop（调用方 abort / panic 展开），从未走到任何收口代码
- **THEN** Windows 上该子进程派生的孙进程 MUST 在作业对象 `Drop` 后终止（`kill_on_drop` 只覆盖直接子进程，不足以收敛持管道写端的孙进程）

#### Scenario: 收尸后不悬挂于管道

- **WHEN** 超时 / 取消触发 kill 后，被测进程曾打开两路管道
- **THEN** 执行体在有限时间内返回（排水任务以 EOF 结束），MUST NOT 无限等待管道关闭

#### Scenario: 自然退出后孤儿持写端仍有界收口

- **WHEN** 被 spawn 的脚本派生后台孙进程（继承 stdout / stderr 后自身退出），壳进程随即以 0 退出，而孙进程仍存活（管道 EOF 不会到达）
- **THEN** 执行体完成收尸即返回 `Completed`，MUST NOT 等到孙进程退出；返回时刻孙进程可观测为仍存活，已读输出（壳进程退出前写入的部分）完整保留

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

本执行体 SHALL NOT 改变既有输出解读面与未迁移的 spawn 面：`exec::ProcessOutput`（`error_excerpt` / `tail` / `failure_message`）语义零变（口径见下）；`record_exec_output` / `read_exec_output` / `mint_exec_id` **签名与语义零变**——其中 `record_exec_output` SHALL 降为 `LiveExec::begin(..)?.commit(..)` 的薄包装（同一 `exec_id` 铸造点、同一 `<report_dir>/exec-outputs/<exec_id>.log` 落点、同一「零截断零改写」内容口径），`read_exec_output` / `mint_exec_id` 一字不动；调用方的 stdout / stderr 合并顺序 MUST 由调用方自行决定（执行体只给两路原始字节）。

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

#### Scenario: 记录 API 面保持可在位旧行为

- **WHEN** 既有调用点以 `record_exec_output(root, change, slot, bytes)` 落盘并随后 `read_exec_output` 回查
- **THEN** 返回值形态（`Result<String, String>` / `Option<Vec<u8>>`）与落盘路径、内容均不变；新增能力只体现为同 id 的 `LiveExec::begin` → `commit` 链与 sidecar / 增量读三件（见「运行中实时输出记录与增量回查」），`mint_exec_id` 的 `exec-<millis>-<seq>` 形态不变

### Requirement: 瞬态签名字典与有限重试策略

执行体 SHALL 提供瞬态判定与重试策略的单点实现（`crates/infra/exec/src/transient.rs`）：

- **触发判据**：瞬态判定 MUST 同时满足「`outcome == Completed`」**且**「`exit_code != 0`」**且**「合并输出（`stdout` 与 `stderr` 两路，逐行包含匹配，大小写不敏感）命中签名字典中任一条」。`TimedOut` / `Cancelled` MUST NOT 命中（超时重试会把 1200 s 变数小时；取消必须立即生效）。
- **签名字典** SHALL 为以下封闭表（行内包含匹配）：

| 族 | 签名 |
|---|---|
| Windows 文件锁 | `os error 32`、`os error 5`、`另一个程序正在使用此文件`、`being used by another process`、`failed to rename archive file`、`failed to remove`、`Access is denied` |
| MSVC 链接器 | `LNK1104` |
| POSIX / Node | `EBUSY`、`ETXTBSY`、`Text file busy`、`EPERM: operation not permitted`、`resource busy or locked` |

- **刻意不收**裸 `failed to write`：该措辞过于宽泛（会把真实写盘缺陷误判为瞬态）；rustc 的 `.rmeta` 写失败已由同一行内的 `os error 32` / `另一个程序正在使用此文件` 覆盖。字典扩展 SHALL 经本模块一处修改（MUST NOT 在调用点各写一份特征串）。
- **remove 形态签名收窄措辞**：Windows 锁族条目 SHALL 为 `failed to remove`（MUST NOT 为 `failed to remove file`）——现场实际文本是 `failed to remove C:\…\xxx.rcgu.o`，**不含 `file` 字样**，带 `file` 的旧条目永不命中（功能上被同行 `os error 32` 兜住，但会误导维护者以为 remove 形态有独立覆盖）。收窄风险低：该短语只出现在删除失败语境。
- **`RetryPolicy`** SHALL 为 `{ max_attempts: u32, backoff: [Duration; 4], total_budget: Duration }`，以代码常量提供 `transient()`（`max_attempts = 5` 即首跑 + 4 次重试，`backoff = [5 s, 10 s, 20 s, 40 s]`，`total_budget = 1500 s`）与 `none()`（单次执行）两构造；参数 MUST NOT 进 config schema。退避取慢档的理由：实测安全软件的静默窗口可达分钟级，秒级退避在「只剩 rename 收尾」（待编译计数已归零）的尝试上大概率撞同一个扫描窗口——而那正是最需要耐心的一档；成功路径零影响（未命中签名不重试），只有真需要重试时才多等（最坏 5 次共 75 s），`total_budget` 不变。
- **预算硬顶**：`total_budget` SHALL 自**首跑开始时刻**起算；预算判定 MUST 在**每次重试尝试开始前**进行（`elapsed >= total_budget` → 不再发起新尝试，把末次结果原样返回：`outcome = Completed` + 非零退出码，由调用方按瞬态命中断言 fault）——MUST NOT 无限重试、MUST NOT 把预算拆成「每次尝试各一份」。MUST NOT 在尝试进行中因预算到期强杀进程（那是 `timeout` 的职责）；故墙钟上界为 `total_budget + 单次尝试预算`（有界，非无限）。
- **增量推进判据（卡死早停）**：执行体 SHALL 提供「待编译计数」单点（同模块）与配套判据，用于区分「还在推进」与「同一批写不进去」：
  - `compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`：以合并输出（`stdout ++ stderr`，UTF-8 lossy）的行口径计数——去前导空白后以 `Compiling ` 为前缀的行数（rustc / cargo 原文形态，大小写敏感）；无该字样 → `0`（非 cargo 构建形态退化为固定次数语义）。
  - `progress_series(report: &RunReport) -> Vec<u32>`：按尝试序给出各次尝试的待编译计数（`retried` 在前、末次在后，长度等于 `attempts`）。
  - `is_progress_stalled(series: &[u32]) -> bool`：末两次计数**非降**且末值 `> 0` 即判卡死（`series.len() < 2` 恒 `false`）。
  - 早停判定 SHALL 落在重试循环内（命中签名之后、尝试上限 / 总预算判定旁）：判为卡死即返回末次结果（`outcome = Completed` + 非零退出码，由调用方按瞬态命中断言 fault），MUST NOT 烧满 `max_attempts`。**`attempts == 1` 无前值不触发**；**末次计数为 `0` 不触发**——「只剩收尾」恰恰是最该再试的形态。该判据只用于**提前放弃**，MUST NOT 用于延长预算（`total_budget` 仍是硬顶）。
- **尝试面**：每次尝试 SHALL 记 `AttemptTrace { exit_code, transient: Option<String>, stdout, stderr }`（`transient` 为命中的签名原文，未命中为 `None`）；重试间的退避等待 MUST 同样观测 `cancel`（置位即刻收敛 `Cancelled`，不先等完退避）。
- 执行体 MUST NOT 自带日志格式（尝试分隔头 / 归因文案归调用方），只提供尝试面字节。

#### Scenario: flaky 锁冲突重试自愈

- **WHEN** 假命令第 1、2 次以「`failed to rename archive file` + `os error 32`」文本（不含 `Compiling ` 行）退出 1、第 3 次退出 0，且以 `RetryPolicy::transient()` 执行
- **THEN** `outcome == Completed`、`exit_code == 0`、`attempts == 3`；`retried` 长度 2，两条各携 `exit_code == 1` 与命中的签名原文（`os error 32`），退避按 5 s / 10 s 生效

#### Scenario: 持续锁冲突耗尽重试

- **WHEN** 假命令每次都输出同一锁文本（不含任何 `Compiling ` 行）并退出 1，以 `RetryPolicy::transient()` 执行
- **THEN** `attempts == 5`（不再增加）、`retried` 长度 4、末次 `outcome == Completed` 且 `exit_code == 1`；全程墙钟不超过 `total_budget`（虚拟时钟断言至多 1500 s）——待编译计数恒为 `0` 故不触发卡死早停

#### Scenario: 计数不降判卡死提前收口

- **WHEN** 假命令每次都以同一份输出（含 50 行 `Compiling <crate> v…` + 锁签名文本）退出 1，以 `RetryPolicy::transient()` 执行
- **THEN** 第 2 次尝试结束即收敛（`attempts == 2 < max_attempts`），末次 `outcome == Completed` + `exit_code == 1`、`retried` 长度 1；MUST NOT 继续烧到第 5 次

#### Scenario: 计数递减继续重试

- **WHEN** 假命令第 1 次输出 264 行 `Compiling …`、第 2 次 5 行、第 3 次 0 行且均命中签名退出 1，第 4 次退出 0
- **THEN** 计数递减不触发早停，执行体继续重试至第 4 次成功：`attempts == 4`、`outcome == Completed`、`exit_code == 0`、`retried` 长度 3

#### Scenario: 末次计数归零不早停

- **WHEN** 前一次尝试计数为 `0`、本次仍为 `0` 并命中签名（「只剩 rename / 清理收尾」形态）
- **THEN** `is_progress_stalled` 判否、执行体继续重试（直到尝试上限 / 总预算 / 成功），MUST NOT 因计数相等而提前放弃

#### Scenario: 非 cargo 输出计数为零

- **WHEN** 以 `compiling_count` 解析仅含锁错误的输出（如 Node / 链接器报错，无 `Compiling ` 行），或以 `progress_series` 读取未重试的 `RunReport`
- **THEN** 前者返回 `0`；后者返回长度等于 `attempts` 的序列（未重试即长度 1）；两调用均零 panic、零 spawn

#### Scenario: 超时与取消零重试

- **WHEN** 命令以 `TimedOut` 或 `Cancelled` 收场（输出中含任一签名文本，或 `retry` 为 `transient()`）
- **THEN** `attempts == 1`、`retried` 为空；MUST NOT 因输出文本命中签名而追加尝试

#### Scenario: 真红零重试

- **WHEN** 命令以「普通编译错误」文本退出 1（不命中任何签名，如 `error[E0308]: mismatched types`），且 `retry = transient()`
- **THEN** `attempts == 1`、`retried` 为空、`outcome == Completed`；重试策略在位但不触发，调用方可据此判「检查红」

#### Scenario: 裸 failed to write 不命中

- **WHEN** 输出仅含 `failed to write /tmp/x`（无 `os error` 码、无锁措辞）
- **THEN** 瞬态判定为否（`attempts == 1`），MUST NOT 因该短语单独触发重试

#### Scenario: remove 形态真实命中

- **WHEN** 输出含 `failed to remove D:\w\target\debug\deps\x.rcgu.o`（删除失败，同一行无 `os error` 码）
- **THEN** 瞬态判定为命中并返回表内原文 `failed to remove`——`failed to remove file` 旧措辞不再作为断言依据（生产文本不含 `file`）

#### Scenario: 退避期间取消即刻生效

- **WHEN** 第 1 次尝试命中签名、执行体正处于重试退避等待，此时 `cancel` 置位
- **THEN** 执行体不再发起下一次尝试，即刻以 `Cancelled` 收口（不先等完剩余退避），`attempts` 记已完成次数

#### Scenario: 预算耗尽停止重试

- **WHEN** 每次尝试耗时 450 s、输出不含 `Compiling ` 行并持续命中签名（`transient()` 策略，`max_attempts = 5`）
- **THEN** 第 4 次尝试结束时已用约 1835 s（含 5 / 10 / 20 s 退避）→ 第 5 次尝试开始前预算判定为耗尽，不再发起新尝试，`attempts == 4`；末次结果原样返回，总墙钟不超过 `total_budget + 单次尝试预算`

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

### Requirement: 运行中实时输出记录与增量回查

`crates/infra/exec` SHALL 提供**运行中可回查**的输出记录类型 `LiveExec`，把既有「一次性落盘」的 `record_exec_output` 扩展为三段一致的生命周期（同一 `exec_id`）：

1. `LiveExec::begin(root, change, slot) -> Result<LiveExec, String>`：铸造 `exec_id` 并**立即**创建输出目录与空 `<exec_id>.log`；
2. `LiveExec::sink() -> LiveSink`：返回可交给 `RunOptions.live` 的追加口（两路排水共用同一实例，内部互斥支撑并发写）；
3. `LiveExec::write_command(&str) -> Result<(), String>`：写同目录 sidecar `<exec_id>.cmd`（执行命令文本）；
4. `LiveExec::commit(&[u8]) -> Result<String, String>`：以**同一 `exec_id`** 覆盖 `.log` 为权威内容并返回该 id。

口径 SHALL 为：

- **同 id 收口**：`commit` MUST NOT 铸造新 id（否则运行中拍与终态拍的 `output_ref` 不同值，节点合并侧与前端跟随面都会断链）——`output_ref` 在 running 拍与终态拍 SHALL 相等。
- **顺序不变量（硬约束）**：文件与父目录 MUST 在 `exec_id` 上抛 / 对外可见**之前**已创建。该顺序是 watch 订阅可生效的前提（父目录缺失 → 显式 `Err`，见 desktop-file-watch）；违反即为实现缺陷，MUST NOT 以运行期退避重试兜底。
- **幂等与降级**：追加写失败 MUST NOT 阻断检查主流程（置内部失效标记，`commit` 仍按权威内容覆盖落盘）；`commit` 之后的追加 MUST 成为空操作。
- **命令 sidecar 形态**：纯文本（UTF-8，MUST NOT 用 JSON 信封）、与 `.log` 同目录同 id 前缀、后缀 `.cmd`；一次写入（MUST NOT 追加多次以外的格式约定）、零 schema 治理。缺失 sidecar 的老记录 SHALL 在回查面返回 `None`（非错误）。
- **增量读**：`read_exec_output_delta_in(change_dir, exec_id, from_offset) -> Option<ExecOutputDelta>` SHALL 返回「自 `from_offset` 起的新增文本 + 下次游标」，其中 `ExecOutputDelta { text: String, next_offset: u64 }` 为**零 derive 纯结构**（`crates/infra/exec` 的依赖面 MUST NOT 因出线需要而扩 serde / specta）。
  - `from_offset = 0` MUST 等价于全量读（与 `read_exec_output` 逐字节一致）；
  - `next_offset` MUST 为**下次调用应传的游标**，且 `next_offset <= 文件长度`、跨调用单调不减；
  - `next_offset` MUST 落在 UTF-8 字符边界：块末若截断多字节序列，该字符 MUST 留待下一次读取（MUST NOT 产出半个字符、MUST NOT 产出替换符）；
  - **推进保证**：只要文件新增了字节，`next_offset` MUST 严格大于上一次返回值（唯一例外是块末半字符尚未补齐的情形——此时以「等字符补齐」为界）。对**非法 UTF-8 字节** SHALL 以 UTF-8 lossy 替换符呈现并照常推进游标（MUST NOT 因非法字节而形成永不推进的死循环，调用方据此可安全轮询）；
  - `from_offset > 文件长度`（收口覆盖为更短内容）SHALL 被夹到 0 重读（MUST NOT panic、MUST NOT 返回空串让上层误以为无内容）。
- **空文件 ≠ miss**：文件存在但为 0 字节时 SHALL 返回 `text == ""` / `next_offset == from_offset`（`Some`）；只有 `exec_id` 非法或两槽位均无该记录时才返回 `None`（miss 非错误）。
- **双锚探测**：`read_exec_command_in` / `read_exec_output_delta_in` / 新增的 log 路径派生 SHALL 与 `read_exec_output_in` 同口径（同 change 目录下 `test` 槽先、`baseline` 槽后，`exec_id` 跨槽唯一，首中即返）。
- **布局单点**：日志与 sidecar 的目录派生 MUST 复用既有 `exec_output_dir_in`（`foundation::layout` 槽位单点），MUST NOT 出现第二份路径拼接。
- **零 tokio 文件 IO**：追加落盘 MUST 用 `std::fs::File`（同步写 + 写后 flush）；`crates/infra/exec/Cargo.toml` MUST NOT 引入 tokio `fs` 特性（其依赖 `rt`，破本 crate 的「零运行时」纪律）。

#### Scenario: 运行中即可读到已写字节

- **WHEN** `LiveExec::begin` 之后经 `sink` 追加一段字节，随后（进程尚未结束）以 `read_exec_output_delta_in(dir, exec_id, 0)` 读取
- **THEN** 返回 `Some`，`text` 逐字节等于已追加内容，`next_offset` 等于该 UTF-8 字符串的字节长度

#### Scenario: 同 id 收口且权威内容覆盖

- **WHEN** 运行中追加了交错字节，进程结束后调 `commit(权威内容)`
- **THEN** `commit` 返回的 id 与 `begin` 铸造的 id 相同；`<exec_id>.log` 内容逐字节等于权威内容（live 追加内容被覆盖）；此后再追加为空操作

#### Scenario: 连续增量拼接等于全量

- **WHEN** 分三次调 `read_exec_output_delta_in`（每次传上一次的 `next_offset`）读一段被逐段追加的日志
- **THEN** 三次 `text` 拼接逐字等于该日志全量；每次 `next_offset` 单调不减；第三次之后再次调用返回 `text == ""` 且 `next_offset` 不变

#### Scenario: 块末半字符留待下次

- **WHEN** 已追加字节在块边界处截断一个 3 字节 UTF-8 字符（只落了前 2 字节）
- **THEN** 本次 `text` MUST NOT 包含该字符的任何碎片或替换符（`next_offset` 停在该字符起点）；该字符的 3 字节补齐后，下次读取完整返回该字符

#### Scenario: 非法字节不卡游标

- **WHEN** 日志中落入了 0xFF 等非法 UTF-8 字节，随后继续追加合法文本，连续两次增量读
- **THEN** 第一次 `text` 以替换符呈现该字节且 `next_offset` 越过它（严格推进）；第二次读到其后的合法文本；两次调用均不 panic、不进入永不推进的循环

#### Scenario: 覆盖变短不越界

- **WHEN** 先前以 `from_offset = 100` 读到 offset 100，随后 `commit` 覆盖为 20 字节内容，再以 `from_offset = 100` 读取
- **THEN** 游标被夹到 0，返回全文与 `next_offset == 20`；MUST NOT panic、MUST NOT 返回空串

#### Scenario: 空文件与 miss 可分辨

- **WHEN** 对刚 `begin` 尚未追加的 `exec_id` 读增量，与对一个不存在的 `exec_id` 读增量
- **THEN** 前者返回 `Some { text: "", next_offset: 0 }`；后者返回 `None`；非法 `exec_id`（含路径分隔符 / `..`）同样 `None`

#### Scenario: 命令 sidecar 写入与回查

- **WHEN** 调 `write_command("pnpm run check 2>&1")` 后以 `read_exec_command_in(dir, exec_id)` 回查
- **THEN** 返回该文本；对无 sidecar 的老记录回查返回 `None`；sidecar 与 `.log` 同目录同 id 前缀

#### Scenario: 双锚探测跨槽命中

- **WHEN** 记录落在 baseline 槽位（`reports/baseline/exec-outputs/`），以 change 目录调三件回查函数
- **THEN** command / delta / path 三件均命中该 baseline 树（与 `read_exec_output_in` 的探测序一致），零 miss

#### Scenario: 依赖面零扩

- **WHEN** 审查 `crates/infra/exec/Cargo.toml` 与 `LiveSink` / `LiveExec` / `ExecOutputDelta` 的 derive 面
- **THEN** `[dependencies]` 仍仅 `foundation` + tokio（`process` / `io-util` / `time` / `macros`，无 `fs` / `rt`）；`LiveSink` 手写 `Debug`（`Arc<dyn Fn>` 不实现 `Debug` 而 `RunOptions` 派生 `Debug`）；`ExecOutputDelta` 零 derive；出线 DTO 定义在壳层命令模块

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/exec/src/run.rs`（新） | 统一执行体 | `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`：spawn（`kill_on_drop`）→ 紧贴 spawn 挂作业对象（Windows）→ 双管道并发排水 → `select!`（取消轮询 200 ms / timeout / `wait`）→ `JobTree::terminate` + `kill_tree` + 收尸；自然退出臂先 `JobTree::disarm`（孤儿孙进程存续）；外层按 `RetryPolicy` 循环（复用同一 `attempt_once`） |
| `crates/infra/exec/src/job.rs`（新） | 进程树回收 | `JobTree::attach(&tokio::process::Child) -> Option<JobTree>`（Windows：`CreateJobObjectW` + `AssignProcessToJobObject`；失败返 `None` 由调用方回落 `taskkill` 语义）、`JobTree::terminate(&mut self)`（`TerminateJobObject` + 解除武装，幂等）、`JobTree::disarm(&mut self)`（自然退出臂解除武装）、`Drop` 未解除武装时整组终止再关句柄（future 被 drop 的兜底）；非 Windows 为零行为差异空实现（`attach` 恒 `None`、`terminate` / `disarm` 空操作）。不设 `KILL_ON_JOB_CLOSE` |
| 同上 | 执行入参 | `RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration /* 必填 */, cancel: CancelFlag, retry: RetryPolicy, live: Option<LiveSink> }` |
| 同上 | 执行产出 | `RunOutcome { Completed, TimedOut, Cancelled }`；`RunReport { outcome, exit_code: i32 /* TimedOut/Cancelled 为 -1 */, stdout, stderr, attempts: u32, retried: Vec<AttemptTrace> }`（`stdout` / `stderr` = 末次尝试） |
| 同上 | 重试循环早停 | 循环内维护按尝试序的计数序列，命中签名之后按 `is_progress_stalled` 判卡死并返回末次结果（`attempts < max_attempts`）；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面零变更 |
| `crates/infra/exec/src/transient.rs`（新） | 瞬态判定、重试策略与推进判据 | `RetryPolicy::{none, transient}`（`backoff = [5, 10, 20, 40] s` / `total_budget = 1500 s` / `max_attempts = 5`）、`AttemptTrace { exit_code, transient: Option<String>, stdout, stderr }`、`is_transient(&RunReport) -> Option<&'static str>`；`compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`、`progress_series(&RunReport) -> Vec<u32>`、`is_progress_stalled(&[u32]) -> bool`（三者纯函数，零 IO 零 spawn）；签名表封闭单点（remove 形态条目为 `failed to remove`） |
| `crates/infra/exec/src/lockprobe.rs`（新） | 锁占用诊断 | `locked_paths(diagnostics: &str) -> Vec<PathBuf>`（三形态解析、去重、≤8）、`lock_holders(paths: &[PathBuf]) -> Vec<LockHolder>`（Windows Restart Manager / 非 Windows `lsof` 降级）、`LockHolder { name: String, pid: u32 }`、`attribution_text(diagnostics: &str) -> Option<String>`（占用者 + 被锁文件 + 降级说明单点装配） |
| `crates/infra/exec/src/lib.rs` | 导出面 | `SystemCommand` 加 `#[derive(Clone)]`；`pub use run::{run_shell, RunOptions, RunReport, RunOutcome}`；`pub use transient::{compiling_count, is_progress_stalled, progress_series, is_transient, AttemptTrace, RetryPolicy}`（推进判据供 `crates/infra/checks` 装配「重试进展」文案）；`pub use lockprobe::{locked_paths, lock_holders, LockHolder, attribution_text}`；`kill_tree` / `ProcessOutput` / `record_exec_output` 语义零变 |
| `crates/infra/exec/Cargo.toml` | 依赖面 | 新增 `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_System_JobObjects", "Win32_System_RestartManager"] }`（Restart Manager 占用者点名 + 作业对象进程树回收；精确探针用 std `OpenOptionsExt` 的 `access_mode` / `share_mode`，不引 FileSystem / Security / Threading feature）；非 Windows 目标零新增编译依赖 |
| `crates/core/foundation/src/cancel.rs` | 取消原语 | 零改动（`CancelFlag::{never, is_set}` / `CancelHandle::set`） |
| 既有输出解读面 | 边界 | `ProcessOutput` 三方法零改动；`read_exec_output` / `mint_exec_id` 零改动（`record_exec_output` 降为 `LiveExec::begin + commit` 薄包装，签名与语义零变）；尝试分隔头与归因文案的落文格式归调用方 |
| `crates/core/foundation/src/cancel.rs`（新） | 取消原语 | `CancelFlag::{never, is_set}`（`Clone` 共享原子）、`CancelHandle::set`；std-only、零 tokio |
| `crates/infra/exec/Cargo.toml` | 依赖面 | tokio 特性 `process` + `io-util` + `time` + `macros`（不引 `rt-multi-thread` / `full`）；dev：`rt` / `macros` / `test-util` / tempfile |
| 同上（排水单点） | `drain` | `async fn drain<R: AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>, live: Option<&LiveSink>)`：8 KiB chunk 循环读尽至 EOF；每块先 `buffer.extend_from_slice` 再 `live.absorb`；读失败即 break（「已读即输出」）；`attempt_once` / `await_exit` 透传 `live`（自然退出臂与被杀两臂的有界排水同语义） |
| `crates/infra/exec/src/lib.rs`（新增） | 运行中记录 | `LiveSink`（`pub(crate) new(append: impl Fn(&[u8]) + Send + Sync + 'static)` / `pub(crate) absorb(&[u8])`；手写 `Debug`）——公开面只有类型本身（`RunOptions.live` 的字段类型），构造与投递留在 crate 内（唯一生产路径 `LiveExec::sink()`） |
| 同上（新增） | 运行中记录 | `LiveExec::{begin(root, change, slot) -> Result<Self, String>, exec_id() -> &str, log_path() -> &Path, sink() -> LiveSink, write_command(&str) -> Result<(), String>, commit(&[u8]) -> Result<String, String>}`；`begin` 内 `create_dir_all` + 建空 `.log`（**先于 id 对外可见**），`commit` 以同 id 覆盖并 take 句柄 |
| 同上（新增） | 增量与 sidecar | `ExecOutputDelta { text: String, next_offset: u64 }`（零 derive）；`read_exec_output_delta_in(change_dir, exec_id, from_offset) -> Option<ExecOutputDelta>`（字符边界收口、越界夹 0、空文件 `Some("")`）；`read_exec_command_in(change_dir, exec_id) -> Option<String>`；`exec_output_log_path_in(change_dir, exec_id) -> Option<PathBuf>` |
| `crates/infra/exec/src/lib.rs` | 导出面 | `pub use run::{run_shell, RunOptions, RunOutcome, RunReport, RunError}`（`RunOptions` 增 `live`）；`pub use {LiveExec, LiveSink, ExecOutputDelta}`；`record_exec_output` / `read_exec_output` / `read_exec_output_in` / `mint_exec_id` 签名与语义零变（前者降为薄包装） |
| `crates/core/foundation/src/layout` | 目录派生单点 | `change_reports_in(change_dir, slot)` 消费方新增三件（log 路径 / sidecar / 增量读同锚）；本 crate 零改动（只被消费） |
