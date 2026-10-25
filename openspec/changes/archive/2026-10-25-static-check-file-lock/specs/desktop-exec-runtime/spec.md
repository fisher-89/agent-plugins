# desktop-exec-runtime Specification（delta: static-check-file-lock）

## MODIFIED Requirements

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

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/exec/src/transient.rs` | 瞬态判定、重试策略与推进判据 | `is_transient(&RunReport) -> Option<&'static str>`；`RetryPolicy::{none, transient}`（`backoff = [5, 10, 20, 40] s` / `total_budget = 1500 s` / `max_attempts = 5`）；`AttemptTrace` 零变更；签名表 `failed to remove file` → `failed to remove`；新增 `compiling_count(stdout: &[u8], stderr: &[u8]) -> u32`、`progress_series(&RunReport) -> Vec<u32>`、`is_progress_stalled(&[u32]) -> bool`（三者为纯函数，零 IO 零 spawn） |
| `crates/infra/exec/src/run.rs` | 重试循环早停 | 循环内维护按尝试序的计数序列，命中签名之后按 `is_progress_stalled` 判卡死并返回末次结果（`attempts < max_attempts`）；`RunOptions` / `RunReport` / `RunOutcome` / `RunError` 字段面零变更 |
| `crates/infra/exec/src/lib.rs` | 导出面 | `pub use transient::{compiling_count, is_progress_stalled, progress_series, is_transient, AttemptTrace, RetryPolicy}`（供 `crates/infra/checks` 装配「重试进展」文案）；`kill_tree` / `ProcessOutput` / `record_exec_output` 语义零变 |

