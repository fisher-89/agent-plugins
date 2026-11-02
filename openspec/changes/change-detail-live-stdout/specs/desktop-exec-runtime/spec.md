# desktop-exec-runtime Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

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

## ADDED Requirements

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
| `crates/infra/exec/src/run.rs`（新） | 统一执行体 | `run_shell(command: SystemCommand, options: RunOptions) -> RunReport`：spawn（`kill_on_drop`）→ 双管道并发 chunk 循环排水（每块 tee 给 `live`）→ `select!`（取消轮询 200 ms / timeout / `wait`）→ `kill_tree` + 收尸；外层按 `RetryPolicy` 循环（复用同一 `attempt_once`） |
| 同上 | 执行入参 | `RunOptions { cwd: Option<PathBuf>, env: Vec<(OsString, OsString)>, timeout: Duration /* 必填 */, cancel: CancelFlag, retry: RetryPolicy, live: Option<LiveSink> }` |
| 同上 | 执行产出 | `RunOutcome { Completed, TimedOut, Cancelled }`；`RunReport { outcome, exit_code: i32 /* TimedOut/Cancelled 为 -1 */, stdout, stderr, attempts: u32, retried: Vec<AttemptTrace> }`（`stdout` / `stderr` = 末次尝试） |
| 同上 | 重试循环早停 | 循环内维护按尝试序的计数序列，命中签名之后按 `is_progress_stalled` 判卡死并返回末次结果（`attempts < max_attempts`）；`RunReport` / `RunOutcome` / `RunError` 字段面零变更 |
| 同上（排水单点） | `drain` | `async fn drain<R: AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>, live: Option<&LiveSink>)`：8 KiB chunk 循环读尽至 EOF；每块先 `buffer.extend_from_slice` 再 `live.absorb`；读失败即 break（「已读即输出」）；`attempt_once` / `await_exit` 透传 `live`（自然退出臂与被杀两臂的有界排水同语义） |
| `crates/infra/exec/src/lib.rs`（新增） | 运行中记录 | `LiveSink`（`pub(crate) new(append: impl Fn(&[u8]) + Send + Sync + 'static)` / `pub(crate) absorb(&[u8])`；手写 `Debug`）——公开面只有类型本身（`RunOptions.live` 的字段类型），构造与投递留在 crate 内（唯一生产路径 `LiveExec::sink()`） |
| 同上（新增） | 运行中记录 | `LiveExec::{begin(root, change, slot) -> Result<Self, String>, exec_id() -> &str, log_path() -> &Path, sink() -> LiveSink, write_command(&str) -> Result<(), String>, commit(&[u8]) -> Result<String, String>}`；`begin` 内 `create_dir_all` + 建空 `.log`（**先于 id 对外可见**），`commit` 以同 id 覆盖并 take 句柄 |
| 同上（新增） | 增量与 sidecar | `ExecOutputDelta { text: String, next_offset: u64 }`（零 derive）；`read_exec_output_delta_in(change_dir, exec_id, from_offset) -> Option<ExecOutputDelta>`（字符边界收口、越界夹 0、空文件 `Some("")`）；`read_exec_command_in(change_dir, exec_id) -> Option<String>`；`exec_output_log_path_in(change_dir, exec_id) -> Option<PathBuf>` |
| `crates/infra/exec/src/lib.rs` | 导出面 | `pub use run::{run_shell, RunOptions, RunOutcome, RunReport, RunError}`（`RunOptions` 增 `live`）；`pub use {LiveExec, LiveSink, ExecOutputDelta}`；`record_exec_output` / `read_exec_output` / `read_exec_output_in` / `mint_exec_id` 签名与语义零变（前者降为薄包装） |
| `crates/infra/exec/src/transient.rs` | 瞬态判定与重试策略 | 零改动（`RetryPolicy::{none, transient}` / `is_transient` / 推进判据三函数） |
| `crates/infra/exec/src/lockprobe.rs` | 锁占用诊断 | 零改动 |
| `crates/infra/exec/Cargo.toml` | 依赖面 | **零改动**：不引 serde / specta；不引 tokio `fs`（依赖 `rt`，破零运行时纪律）；Windows Restart Manager 条件依赖不变 |
| 既有输出解读面 | 边界 | `ProcessOutput` 三方法零改动；`read_exec_output` / `mint_exec_id` 零改动；尝试分隔头与归因文案的落文格式归调用方 |
| `crates/core/foundation/src/layout` | 目录派生单点 | `change_reports_in(change_dir, slot)` 消费方新增三件（log 路径 / sidecar / 增量读同锚）；本 crate 零改动（只被消费） |
