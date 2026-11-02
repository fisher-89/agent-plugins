# 提案: change-detail-live-stdout

> **变更**: change-detail-live-stdout
> **日期**: 2026-10-31
> **状态**: draft
> **来源**: `explore.md`（requirement 类型：需求两句话 / 现状双门锁死 / 四个决策（D1–D4）/ push 流式反面论证 / 骨架与触点清单 / 五条待定 / 三条仓库红线）
> **前置依赖**: `extract-file-watch-signal-hook`（Change A）**已归档**——`packages/desktop/src/hooks/use-file-watch-signal.ts` 已在盘上（`SIGNAL_DEBOUNCE_MS = 500`、参数面恰三项）。本变更前端相位直接消费它，零新增防抖样板、零新增常量。后端相位（`变更范围 · 实现文件` 第 1–5 组）与 A 无任何依赖。
> **性质**: 功能变更——两处用户可见新能力（脚本步**运行中**可读 stdout；面板顶部显示**执行的命令**），后端形成一条 live 记录链（`begin → append → commit` 同 id），前端形成一条「watch 信号 → offset 增量读」跟随链。

---

## 问题

### 现状：脚本步跑起来的 20 分钟里，抽屉压根打不开

痛点强度由预算决定：`STATIC_CHECK_TIMEOUT = 1200 s`、`SUITE_COMMAND_TIMEOUT = 1200 s`，叠加 `RetryPolicy::transient()` 的 `[5, 10, 20, 40] s` 退避——最长 20 分钟里用户只看到一个 `animate-pulse` 的「运行中」节点。

| # | 分歧 | 位置 | 形态 |
|---|---|---|---|
| P1 | `outputRef` 只在终态出现 | `crates/core/orchestration/src/walker.rs` 的 10 处 `with_output_ref` 调用（static-check / test-exec 各 4 处 passed / failed 收口 + `settle_fault` 2 处 stopped / failed 收口） | running emit 恒 `output_ref = None` |
| P2 | 前端点击判据锁在 `outputRef` 上 | `views/changes/flow/change-flow-graph.tsx` `onNodeClick`：`role === null && outputRef === null` → `return` | 运行中的工具步**不可点击**，抽屉打不开（不是「面板不刷新」） |
| P3 | 输出记录一次性落盘 | `crates/infra/exec/src/lib.rs` `record_exec_output`：铸造 + 一次 `fs::write` | 零流式、零 sidecar、零命令信息 |
| P4 | 排水读到 chunk 不通知外部 | `crates/infra/exec/src/run.rs` `drain`：`read_to_end` | 字节在内存 `Vec` 里长齐，磁盘面在进程结束前不动 |
| P5 | 命令文本零出口 | 配置值 `static_analysis` / 模板展开结果 `expanded` 只在 spawn 处一闪 | 没有任何落盘 / IPC / 面板路径 |
| P6 | watch 父目录缺失静默休眠 | `crates/infra/watch/src/lib.rs` `watch_target_of`：父目录非目录 → `None` → `watcher.watch()` 压根没调，`Result` 仍 `Ok` | 对本消费者恰在默认路径上（`reports/test/exec-outputs/` 第一次记录前不存在） |

### 为什么要改排水而不是让子进程直写文件

`Stdio::piped()` 已在位、`read_to_end` 也**不是**「等进程结束才读」（它循环 `poll_read` 直到 EOF）——**唯一缺口是读到 chunk 时不通知外部**。既有消费面（static-check 的 `diagnostics` 进反馈边 prompt、`attempts_log`、`fault_text`、test-exec 的 `merged_output`）全部依赖**内存字节**：让子进程直写文件（`Stdio::from(File)`）会把这四条消费面整体推倒。故 `drain` 改为 chunk 循环 + tee（见提案第 2 节）。

### 为什么不做 push 流式（Channel 推 chunk）

| 理由 | 内容 |
|---|---|
| 粒度天然是「批」 | 一次 read 拿到 OS 当前就绪字节，cargo 一行 `Compiling foo` 约 40 字节 ⇒ 前端看到的仍是「一段段文字」 |
| 时间分布是「突发 + 长静默」 | 编译时一秒几十行、然后静默三十秒；push 与拉在两种相位下无差 |
| `Channel` 无背压 | 前端慢则 webview 队列涨（agent token 流「高频小量」正好相反） |
| 双数据源漂移 | push 的 chunk 流是真交错，权威 commit 是「stdout 前缀 + stderr 后缀」——收口瞬间面板跳变且不可归因 |
| 成本卡在 trait 边界 | push 的投递目标必须下沉到 `drain` 一层（chunk 级投递），而 `StaticCheckRunner::run` 的签名里没有位置放它 ⇒ 要么穿 3 trait + 2 实现 + 全部 mock，要么进程级注册表（破零全局可变状态）。拉模型只需在 runner 顶部上抛 `exec_id`（一个字符串）：chunk 落盘发生在 `crates/infra/exec` 内部（`LiveSink = Arc<dyn Fn(&[u8])>`，对未来消费者零知识），port 边界上只多一个 `Option<OutputRefSink>` 形参 |

拉-通知 + offset 的落盘路径因此把成本收在「一次回调」量级：`live` 的 tee 面是 infra 内部管道（不跨 port 边界、不认识任何消费者类型），重连状态只有一个 `u64`。

---

## 提案

### 1. `crates/infra/exec`：`LiveExec` 记录生命周期（begin / append / commit 同 id）

```rust
/// 实时输出落盘口（`RunOptions` 可选面；两路 drain 共用同一实例）
#[derive(Clone)]
pub struct LiveSink(Arc<dyn Fn(&[u8]) + Send + Sync>);

impl LiveSink {
    /// 构造面 `pub(crate)`：唯一生产路径是 `LiveExec::sink()`，不为尚未存在的
    /// 消费者开公开构造（规则 2）；crate 内用例可直构以单测 tee。
    pub(crate) fn new(append: impl Fn(&[u8]) + Send + Sync + 'static) -> Self;
    pub(crate) fn absorb(&self, bytes: &[u8]);
}

/// 手写 Debug：`Arc<dyn Fn>` 不实现 Debug，而 `RunOptions` 派生了 Debug
impl std::fmt::Debug for LiveSink { /* "LiveSink(..)" */ }

/// 运行中可回查的输出记录：先建文件 → 边跑边追加 → 收口用同一 id 覆盖为权威内容。
pub struct LiveExec {
    exec_id: String,
    log_path: PathBuf,
    writer: Arc<Mutex<Option<std::fs::File>>>,
}

impl LiveExec {
    /// 前置铸造：`create_dir_all` + 建空 `<exec_id>.log`。
    /// **必须先于任何 watch 订阅**（父目录缺失 → 订阅显式 `Err`，见「能力 · desktop-file-watch」与风险 R-4）
    pub fn begin(root: &Path, change: &str, slot: ReportSlot) -> Result<Self, String>;
    pub fn exec_id(&self) -> &str;
    pub fn log_path(&self) -> &Path;
    /// 追加口（交给 `RunOptions.live`）；内部 Mutex 支撑两路 drain 共用
    pub fn sink(&self) -> LiveSink;
    /// 命令 sidecar（`<exec_id>.cmd`）：spawn 前写
    pub fn write_command(&self, command: &str) -> Result<(), String>;
    /// 收口：take 句柄后同 id 覆盖为权威内容
    pub fn commit(&self, output: &[u8]) -> Result<String, String>;
}

/// 既有 API 降为薄包装（语义零变、仍单点）
pub fn record_exec_output(root, change, slot, output) -> Result<String, String>
{ LiveExec::begin(root, change, slot)?.commit(output) }

/// 命令 sidecar 回查（与 `read_exec_output_in` 同双锚探测：test 先 / baseline 后）
pub fn read_exec_command_in(change_dir: &Path, exec_id: &str) -> Option<String>;

/// 布局派生 log 绝对路径（双锚探测；目录名知识不下沉前端）
pub fn exec_output_log_path_in(change_dir: &Path, exec_id: &str) -> Option<PathBuf>;

/// 增量读：自 from_offset 起的新增文本 + 下次游标（**offset 唯一产出方在后端**）
pub fn read_exec_output_delta_in(change_dir: &Path, exec_id: &str, from_offset: u64)
    -> Option<ExecOutputDelta>;
```

`ExecOutputDelta { text: String, next_offset: u64 }` **不带任何 derive**：`crates/infra/exec/Cargo.toml` 保持**零改动**（不引 serde / specta / tokio `fs`）。出线 DTO `ExecOutputChunk`（serde camelCase + `specta::Type`）定义在壳层命令模块，由命令做 1:1 映射——这是对 explore §5.1 的一处**修正**（原文把 `Serialize` / `Type` derive 写在 exec 内部，会连带改依赖面）。

**关键口径**：

- `commit` 用**同一 exec_id** 覆盖 live 追加内容 ⇒ 节点 running 拍与终态拍引用相等，DB / 线格式零改。
- `\r`、半 UTF-8 字符、双路交错都属于**文本形态**，后端只在 UTF-8 字符边界收口（不产出半字符），不做终端语义归一（`\r` 归一归前端，见提案第 6 节）。
- 空文件 ≠ miss：运行中 0 字节读回 `text == ""`、`next_offset == from_offset`；面板靠 `status` 区分「运行中，等待输出…」与「（无输出记录）」。
- 追加写失败静默（置内部标记，不影响 `commit`）——live 面是观测面，MUST NOT 因它打断检查主流程。

### 2. `crates/infra/exec/src/run.rs`：`drain` 改 chunk 循环 + tee

```rust
pub struct RunOptions {
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
    pub cancel: CancelFlag,
    pub retry: RetryPolicy,
    /// 实时输出观察者（None = 纯内存，既有语义逐字节零变）
    pub live: Option<LiveSink>,
}

async fn drain<R: AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>, live: Option<&LiveSink>) {
    // 8 KiB chunk 循环：读满一块 → 写入 buffer（既有消费面）→ absorb 到 live（实时面）
    // Err → break（沿退役 read_to_end 语义：读失败不改变「已读即输出」结论）
}
```

- `attempt_once` / `await_exit` 各多一个 `live: Option<&LiveSink>` 形参，两处调用点（自然退出臂 + 被杀两臂的有界排水）原样透传 ⇒ 统一语义、零分叉。
- **全部 `RunOptions` 构造点**（4 处生产：`static_check.rs` / `testexec/runner.rs` / `mutation/{cargo_mutants,stryker}.rs`——后两者补 `live: None`；测试侧 `run_test.rs` 4 处辅助构造）补 `live: None`；`live: None` 路径与今日逐字节等价（既有 `run_test.rs` 全绿为判据）。
- **红线 ①**：MUST NOT 加 tokio `fs` feature（它依赖 `rt`，内部 `spawn_blocking`）——sink 落盘用 `std::fs::File`，chunk 级同步写几微秒。
- **红线 ③**：tee MUST NOT 引入新的无界等待——被杀两臂的有界排水仍以 `FINISH_GRACE`（2 s）截断，注释须写明。

### 3. `crates/infra/checks`：两 runner 接入（三步顺序是支点）

```rust
// static_check.rs
let live = exec::LiveExec::begin(Path::new(root), name, slot)?;
live.write_command(&format!("{command} 2>&1")).ok();           // ① sidecar 先落
if let Some(on_ref) = on_output_ref { on_ref(live.exec_id()); } // ② 文件已在，才上抛
let run = run_shell(.., RunOptions { live: Some(live.sink()), .. }).await?;
let output_ref = live.commit(&environment::attempts_log(&run)).ok();           // ③ 同 id 覆盖（失败降级 None）
```

- **①→②→③ 顺序不可倒**：必须先建目录建文件再上抛 `exec_id`——否则订阅会撞上父目录缺失的显式失败（`WatchError::ParentUnavailable`），运行中增量跟随失效（见风险 R-4）。
- 无配置 / 空命令直过分支**不建 live 记录**（零 spawn ⇒ 无 live，终态 `output_ref` 仍 `None`）。
- test-execution：`begin` 在 detect 之前；每个 plan **spawn 前**追加一行命令 `=== plan <id> · <framework> ===\n<expanded>`（沿既有 plan 头格式，符合同一记录内分段可读）；`commit(&merged_output)` 落在 `outcome` 与 `fault_outcome` 两个收口点（同 id，含复用门空记录分支）。
- 取消 / 超时：击杀后 `commit` 仍写已收字节 ⇒ 节点 stopped + ref，面板可读部分输出（正合用户预期）。

### 4. `crates/core/orchestration`：`OutputRefSink` 前置上抛 + emit 面收口

```rust
/// 输出引用上抛回调（exec_id 铸造后立即触发一次；触发时 live 文件已存在）
#[derive(Clone)]
pub struct OutputRefSink(Arc<dyn Fn(&str) + Send + Sync>);
// 手写 Debug / PartialEq（Arc::ptr_eq 身份语义，沿 CancelFlag 先例）/ Eq

pub struct ToolStepRequest {
    pub root: String,
    pub command: ToolCommand,
    pub cancel: CancelFlag,
    /// 缺省 None = 老行为（mock runner 只补一个未使用的形参；不回调即老行为）
    pub on_output_ref: Option<OutputRefSink>,
}

pub trait StaticCheckRunner: Send + Sync {
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag,
           on_output_ref: Option<OutputRefSink>) -> BoxToolFuture;
}
// TestExecutionRunner 同型
```

红利：`role === null && outputRef !== null` 立刻可点 ⇒ **前端点击判据一行不用改**；`accumulate_node` 既有的「`output_ref` some 才覆写」直接吃下补发拍，DB / 线格式零改。

`'static` 约束是硬性的（回调跨 port 边界时不能借用 `&RunGuard`）⇒ 新增 `RunGuard::emitter()` 产出 `StepEmitter { key, control: Arc<ChangeFlowControl> }`，其 `attach_output_ref(phase, attempt, step, instance_no, exec_id)` 补发一拍带 ref 的 running 步状态。`run_tool` 增加 `identity: Option<StepIdentity>` 形参（全仓 12 处 `run_tool` 调用点中，4 处脚本步传 `Some`：static-check 2 处（反馈边 + `on_fail=Terminal`）、test-execution 2 处（同两形态）；其余 8 处工具步传 `None`）。

**D6：`StepSettle` + `emit_node` + `now_millis` 上收 `control.rs`**。判据不是改动量（实际 0 处调用点改动，只需扩 walker 顶部一行 `use`），而是**语义单点归属**：让编译器兜住（`ChangeStepState` 加字段时铸造点只有一个）。附带收益：`emit_node`（造一行）与 `accumulate_node`（并一行）同屏可对读，身份键写侧 / 并侧相邻。

### 5. 壳层三命令（`src/commands/changes/mod.rs` + `all_commands!`）

| 命令 | 签名 | 语义 |
|---|---|---|
| `exec_output_path` | `(root, change_name, exec_id) -> Option<String>` | 布局派生 log 绝对路径（双锚探测），供前端 watch 寻址 —— 照 `explore_doc_path` 同构 |
| `read_exec_command` | `(root, change_name, exec_id) -> Option<String>` | sidecar 读取；老记录无 sidecar ⇒ `None`（面板优雅降级「命令未记录」） |
| `read_exec_output_delta` | `(root, change_name, exec_id, from_offset) -> Option<ExecOutputChunk>` | 增量读；`from_offset = 0` 即全量；**offset 唯一产出方在后端** |

`read_exec_output` 原命令**一字不动**（既有 spec 的「零变」承诺不破，仍为整读回查面）。workspace 定位（name → 记录 → worktree 候选序）沿既有 `read_exec_output` 单点复用，不另写第二份定位逻辑。

### 6. 前端：`use-exec-output.ts` 重写 + 面板三态

```ts
export function useExecOutput(params: {
  root: string | null;
  changeName: string | null;
  execId: string | null;
  /** 步状态：running → 挂 watch 增量；转终态 → 退订 + offset 归零整读一次 */
  status: ChangeStepStatus | null;
}): { command: string | null; text: string | null; error: string | null };
```

- **统一走 `ipc` 代理层**：explore 笔记 §5.6 记的「现状用裸 `commands.readExecOutput`」**已过期**——盘上实现（`use-exec-output.ts:41`）已走 `ipc.readExecOutput`（`ipc-zod-runtime-schema` 之后统一）。重写时保持 `ipc` 面即可，新三命令同样经 `ipc`（`ipc.ts` 键遍历自动覆盖，零改动）。
- **一条状态规则解决两件事**：`status` 转非 running ⇒ 退订 watch + offset 归零整读一次——同时处理「权威内容重排（stdout 前缀 + stderr 后缀）」与「文件被覆盖后 offset 越界」。
- 运行中：`useFileWatchSignal({ armKey: status === 'running' ? `${root}\0${execId}` : null, resolvePath: () => ipc.execOutputPath(root, changeName, execId), onSignal: readDelta })`——非运行态不订阅（收口后文件冻结），沿用 A 的 `SIGNAL_DEBOUNCE_MS = 500`，零新增常量。
- 装载首拍 `readDelta()`（覆盖订阅前已产生的字节）。
- `\r` 归一在**渲染前文本面**做（与实时同批）：只保留最后一个 `\r` 之后的内容，避免 cargo 进度行堆成残行。
- 面板三态 + 命令头行：`status === 'running'` 且 `text === ''` ⇒「运行中，等待输出…」；非运行且 `null` / 空 ⇒「（无输出记录）」；invoke reject ⇒ 错误态（不炸图）。命令头行在最上方（`<pre>` 多行原样 + 复制按钮，见「已定」Q1）。

---

## 已定（explore 五条待定项的裁定）

| # | 待定 | 裁定 | 理由 |
|---|---|---|---|
| Q1 | 命令呈现：单行截断 + hover 展开 vs 多行原样 + 复制按钮 | **多行原样（`whitespace-pre-wrap`）+ 复制按钮** | test-exec 每 plan 一条长命令，截断会把区分信息（`=== plan … ===`）藏进 hover——hover 不可测、触屏不可用；复制按钮复用 `components/ui/code-block` 既有 `CodeBlockCopyButton` |
| Q2 | mutation-test 是否纳入 | **不纳入** | 它今天完全没有输出记录（不走 `record_exec_output`），纳入要先给它建记录面，属范围扩张；记入「风险 R-9」留待后续 change |
| Q3 | 粘底滚动 | **纳入**：粘底自动跟随 / 用户上滑即停止粘底 | 运行中输出跟随是该能力的核心体验；判据抽为纯函数（`isStuckToBottom`）可直测，不引入新依赖 |
| Q4 | 节点卡片内露尾 3 行 | **不纳入** | 节点卡片已承载状态 / 耗时 / token 常驻行；露尾会引入第二数据源与运行中高频重渲染，且与「点击即抽屉」单一交互入口冲突 |
| Q5 | sidecar 格式：纯文本 vs JSON | **纯文本** | 一次写入、无扩展需求；JSON 需 schema 治理与转义约定，为假想扩展付现价（规则 2） |

---

## 能力

### 新增的能力

**无。** 三块新机制各自落在既有能力边界内：live 记录链属 `desktop-exec-runtime`（执行体的输出记录面）、上抛链属 `desktop-change-orchestration`（工具步 port + emit 面）、跟随链属 `desktop-change-flow-view`（stdout 面板）。另立能力会让「执行体契约」与「执行体产物」共用一条 requirement，降低可读性。

### 修改的能力

- **desktop-exec-runtime** — `MODIFIED`「统一执行体 run_shell 契约」（`RunOptions` 增 `live: Option<LiveSink>`；排水改 chunk 循环 + tee；`live: None` 逐字节等价；禁 tokio `fs`；tee 不引入无界等待）；`MODIFIED`「输出解读面与未迁移 spawn 面边界」（`record_exec_output` 由「零变」改为「语义零变 + 签名保持、降为 `LiveExec::begin + commit` 薄包装」，`read_exec_output` / `mint_exec_id` 一字不动；`ProcessOutput` 三方法逐字不变）；`ADDED`「运行中实时输出记录与增量回查」（`LiveExec` 生命周期 / 同 id 收口 / 顺序不变量 / 命令 sidecar / 增量读与字符边界 / 空文件非 miss / 双锚探测 / 追加失败静默）；Module Contract 新增 `LiveSink` / `LiveExec` / 三个只读函数 / tee 单点，并注明 `Cargo.toml` 零改动。
- **desktop-checks-domain** — `MODIFIED`「输出记录收敛到 infra/exec」（record 单点改为 `begin → sink → write_command → commit` 同 id；命令 sidecar 写入时机；无配置不建 live 记录；取消 / 超时仍 commit 已收字节）；Module Contract 更新 `static_check.rs` / `testexec/mod.rs` / `port.rs` 三行。
- **desktop-change-orchestration** — `ADDED`「脚本步运行中输出引用前置上抛」（`ToolStepRequest.on_output_ref` + 两 runner 签名 + `run_tool` 组装 + `StepEmitter` + `emit_node` / `StepSettle` / `now_millis` 上收 `control.rs` + `accumulate_node` 直接吃下 + mock runner 行为零变（仅补未使用形参））；Module Contract 更新四行。
- **desktop-change-flow-view** — `MODIFIED`「节点点击判据与 stdout 输出面板」（取数改经 `ipc` 三命令、运行中 watch 增量 / 终态整读、命令头行、三态空态、`\r` 归一、`status` 下传）；Module Contract 更新 `use-exec-output.ts` / `stdout-panel.tsx` / `detail-drawer.tsx` 三行。
- **desktop-file-watch** — `MODIFIED`「单文件 watch 订阅通道」（区分「目标文件缺失不报错」与「父目录缺失显式 `Err`」）；`ADDED`「父目录缺失的显式失败与订阅顺序不变量」（R4 安全网 + R5 顺序不变量、明确**不做退避重试**）；Module Contract 更新 `crates/infra/watch/` 一行。
- **desktop-test-execution** — `MODIFIED`「输出记录为脚本真实输出」（记录链由一次性 `record_exec_output` 改为 `begin` → 逐 plan `write_command` → `commit(&merged)` 同 id 收口；新增命令 sidecar `=== plan <id> · <framework> ===` + 展开命令的追加形态，明示命令文本**只进 `.cmd`、不进 `.log`**；`.log` 内容口径（脚本真实输出 + plan 分隔头）零变）；Module Contract 更新 `testexec/mod.rs` / `runner.rs` 两行。
- **desktop-ipc-type-bindings** — `ADDED`「实时输出三命令与出线类型入册」（`all_commands!` 三命令、`ExecOutputChunk` 出线、守卫 diff 为零）；Module Contract 增 `all_commands!` 与 DTO 两行。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-change-state-store** | `RunNodeRecord` / bincode v1 **零改动**：`output_ref` 字段本就在案，running 拍与终态拍**同值**（同 id 覆盖），节点行无需新字段。命令文本走 sidecar 而非节点行，正是为守住「bincode version bump + 升级链」这条代价 |
| **desktop-change-queries** | 详情线面零演进：`ChangeRunNodeRecord.output_ref` 直读透出语义不变；`ChangeStepState` 字段面不变（`StepSettle` 是 walker 内部构造器） |
| **desktop-ipc-runtime-guard** | 新命令经 `all_commands!` 入册后 `ipc.<name>` 自动具备参数面 strict / 结果面 soft-report 两级校验（`ipc.ts` 零 diff）；消费方本就经 `ipc`，无新校验语义、无新漂移面 |
| **desktop-app-shell** | 三命令同落既有 `commands/changes/` 组（该组定位本就是「change 域读 + 记录面同组」，`read_exec_output` 亦在其中）；blank root 走读命令空结果语义（`None`）；无新轨道、无新 `State`、零文件系统直连（只经 `exec` + 既有 name → worktree 定位） |
| **desktop-crate-layout** | `crates/infra/exec/Cargo.toml` **零改动**（不引 serde / specta / tokio `fs`；`ExecOutputChunk` DTO 落壳层）；`infra/watch` 只加一枚举变体 |
| **repo-archi-model** | `src/hooks/**` 在建模封闭排除面内；`views/changes/flow/` 三文件改动不增删元素 / 边（`stdout-panel.tsx` 已在 `Flow` 子域覆盖内） |
| **desktop-workspace-config / desktop-write-protection / desktop-release-pipeline** | 零配置面 / 零写保护面 / 门禁命令形态不变（仅用例增量与版本 patch） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | `LiveSink`（含手写 `Debug`）/ `LiveExec`（`begin` / `exec_id` / `log_path` / `sink` / `write_command` / `commit`）/ `ExecOutputDelta`（纯结构、零 derive）/ `read_exec_command_in` / `exec_output_log_path_in` / `read_exec_output_delta_in` / 命令 sidecar 后缀常量；`record_exec_output` 降为 `LiveExec::begin(..)?.commit(..)` 薄包装；复用私有 `exec_output_dir_in` / `is_valid_exec_id` / `mint_exec_id` |
| `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` | `RunOptions` 增 `live: Option<LiveSink>`；`drain` 改 8 KiB chunk 循环 + tee（`read_to_end` 形态退役）；`attempt_once` / `await_exit` 透传 `live`；注释写明「tee 不引入新的无界等待」 |
| `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` | `begin → write_command → on_output_ref → run_shell(live) → commit`；`StaticCheckRunner::run` 增 `on_output_ref` 形参；无配置分支不建 live 记录 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | `execute` 入口 `begin` + 上抛 + 收口 `commit`；逐 plan spawn 前 `write_command`；`outcome` / `fault_outcome` 改吃 `&LiveExec`（同 id）；复用门分支 commit 空内容 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` | `execute_plan` 增 `live: Option<LiveSink>` 形参并交给 `RunOptions`——记录生命周期（`begin` / `write_command` / `commit`）与上抛仍归 `mod.rs` 单点，runner 零记录职责 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `OutputRefSink`（手写 `Debug` / `PartialEq` / `Eq`）+ `ToolStepRequest.on_output_ref` + `StaticCheckRunner` / `TestExecutionRunner` 签名扩一参 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | 两分支透传 `on_output_ref` |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | 上收 `StepSettle` + `emit_node` + `now_millis`；新增 `RunGuard::emitter()` 与 `StepEmitter::attach_output_ref`（`'static` 发射器） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `run_tool` 增 `identity: Option<StepIdentity>` 并组装 `OutputRefSink`（4 处脚本步传 `Some`，其余 8 处传 `None`）；顶部 `use` 扩一行；`emit_node` 相关定义移出 |
| `packages/desktop/src-tauri/crates/infra/watch/src/lib.rs` | `WatchError` 由元组结构体改为 `enum WatchError { Notify(notify::Error), ParentUnavailable(PathBuf) }`（或等价形态）；`subscribe` 在 `watch_target_of` 返回 `None` 时显式 `Err`；文件缺失（父目录在）仍 `Ok`；crate 文档同步 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 三薄命令 `exec_output_path` / `read_exec_command` / `read_exec_output_delta` + 出线 DTO `ExecOutputChunk`（serde camelCase + `specta::Type`）；复用既有一对 name → worktree 候选定位 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 增三命令 |
| `packages/desktop/src/views/changes/hooks/use-exec-output.ts` | 重写：走 `ipc`、`status` 入参、`useFileWatchSignal` 增量跟随、终态整读 + offset 归零、命令头行一次读、`\r` 归一纯函数 |
| `packages/desktop/src/views/changes/flow/stdout-panel.tsx` | 命令头行（多行 `<pre>` + 复制按钮）/ 运行中空态 / 三态分派 / 粘底跟随（纯函数判据） |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 给 `StdoutPanel` 下传 `status`（取节点 `status`） |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | 重导出（三命令 + `ExecOutputChunk` 类型），入库 |
| `packages/desktop/package.json` | `version` 0.6.10 → **0.6.11**（一次 patch） |

### 测试文件

| 用例面 | 位置 | 覆盖点 |
|---|---|---|
| `LiveExec` 生命周期 | `crates/infra/exec/src/lib_test.rs` | `begin` 建目录建文件 / `sink` 追加可见 / `commit` 同 id 覆盖 / sidecar 写入与回查 / 老记录无 sidecar → `None` / 双锚探测（test 与 baseline）/ 非法 exec_id → `None` / 追加失败不阻断 commit |
| 增量读 | 同上 | `from_offset = 0` == 全量 / 连续增量拼接 == 全量 / `next_offset` 单调且不超长度 / 半 UTF-8 字符在字符边界收口 / 空文件 → `("", from)` |
| tee 排水 | `crates/infra/exec/src/run_test.rs` | `live: Some` 收到与 `stdout` 逐字节同内容 / `live: None` 与今日等价 / 双路共享 sink 时两路字节均抵达 / 被杀两臂有界排水不因 sink 悬挂 |
| static-check 记录链 | `crates/infra/checks/src/static_check_test.rs` | running 期 `.log` 已存在且可读、连续读单调增长 / 上抛 exec_id 时文件在案 / 终态 ref == running ref / `attempts_log` 权威内容覆盖 live 交错流 / 无配置不建记录 |
| test-exec 记录链 | `crates/infra/checks/src/testexec/mod_test.rs` | 逐 plan 命令追加 / `outcome` 与 `fault_outcome` 同 id 收口 / 复用门空记录仍回填 ref |
| 上抛与 emit | `crates/core/orchestration/walker_test.rs` | running 节点带 `output_ref` 且与终态同值 / `on_output_ref = None` 的 mock runner 路径零行为变化 / `StepEmitter` 补发拍被 `accumulate_node` 合并（不新增节点）/ emit 上收后 56 处调用点行为零变（既有用例全绿） |
| port 契约 | `crates/core/orchestration/port_test.rs` / `steps_test.rs` | 签名扩一参后既有 mock 补 `None` 默认；`OutputRefSink` 身份相等语义 |
| watch 显式失败 | `crates/infra/watch/src/lib_test.rs` | 父目录缺失 → `Err(WatchError::ParentUnavailable(路径))` / 文件缺失而父目录在 → `Ok` 且创建后出信号（既有 `目标不存在时订阅返回ok…` 用例**零改动**仍绿——其临时目录父目录本就存在，恰是「文件缺失」臂）；父目录补齐后重订即 `Ok` |
| 前端取数与跟随 | `src/views/changes/hooks/use-exec-output.test.ts` | 挂载首拍增量 / 运行中信号触发增量且 offset 推进 / `status` 转终态整读一次 + 退订 / `armKey` 非运行态为 null / 命令头行失败不阻断内容面 / `\r` 归一 |
| 前端面板 | `src/views/changes/flow/stdout-panel.test.tsx` | 命令头行在最上方 + 复制 / 三态空态 / 粘底判据纯函数 / 错误态不炸图 |
| 详情页点击判据 | `src/views/changes/change-detail-view.test.tsx` | 运行中（`status=running`）且携 `outputRef` 的 static-check 节点点击即开抽屉并呈 stdout 面板（判据本身零改，用例覆盖「运行中可点」形态） |

### 不要修改

- **`packages/desktop/src-tauri/crates/core/workflow/**`**（读面 DTO、golden 语料）与 **`crates/infra/store/**`**：本变更零 DB / 零线格式改动，`RunNodeRecord` bincode v1 不动；出现 golden diff 即为越界。
- **`packages/desktop/src/types/generated/**` 的手写编辑**：只经 `pnpm run bindings:export` 生成。
- **`crates/infra/exec/Cargo.toml`**：MUST NOT 新增 serde / specta / tokio `fs`；出线 DTO 落壳层。
- **`crates/infra/checks/src/mutation/**`**：突变段不接 live（Q2 裁定）。
- **`packages/desktop/src/hooks/use-file-watch-signal.ts`**（Change A 产物）：零改动——本变更只是新增第二个消费方；MUST NOT 为它开 `debounceMs` / `enabled` / 重试参数。
- **`packages/desktop/src/views/changes/hooks/use-change-detail.ts`**（`useDebouncedBump` / 通知去抖 / 有界重试）：驱动源不同（run 内存通知），划界原则不变。
- **`packages/desktop/src/views/changes/flow/change-flow-graph.tsx`**（点击判据 `role === null && outputRef === null`）：**一行不改**——D1 的红利即「运行中节点自然可点」。
- **`crates/infra/watch/src/lib.rs` 的 `watch_target_of` 休眠语义本体**：只把「父目录缺失」从静默 `Ok` 改为显式 `Err`，**不做退避重试**、不预建目录、不递归挂祖先。
- **`openspec/specs/**` 基线**：只在 change 目录出 delta（归档时合入）；`openspec/config.json` 零改动。
- **`openspec/changes/change-detail-live-stdout/explore.md`**：只读。

---

## 交付顺序（每步独立可验）

| # | 相位 | 内容 | 验证面 |
|---|---|---|---|
| 1 | exec | `LiveExec` / `LiveSink` / `begin-append-commit` / 命令 sidecar / 增量读三件；`RunOptions.live` + `drain` tee | `lib_test.rs` + `run_test.rs`（纯 infra，先测） |
| 2 | checks | static-check / test-execution 改用 live 链（同 id 收口、命令 sidecar 落盘、runner 只透传 sink） | `static_check_test.rs` / `testexec/mod_test.rs` |
| 3 | core | `OutputRefSink` + `ToolStepRequest.on_output_ref` + `run_tool` 组装 + `StepEmitter`；`StepSettle` / `emit_node` / `now_millis` 上收 `control.rs` | `port_test.rs` / `steps_test.rs` / `walker_test.rs` |
| 4 | watch | `WatchError::ParentUnavailable`（文件缺失 vs 父目录缺失两分） | `crates/infra/watch/src/lib_test.rs` |
| 5 | 壳层 | 三薄命令 + `ExecOutputChunk` + `all_commands!` + bindings 重导出 | `bindings:export` + `src/commands/changes/mod_test.rs` |
| 6 | 前端 | `use-exec-output.ts` 重写（watch 增量 + 终态整读）/ `stdout-panel.tsx`（命令头行 + 三态 + 粘底 + CR 归一）/ `detail-drawer.tsx` 下传 `status` | 两个前端测试文件 + `change-detail-view.test.tsx` |
| 7 | 收口 | 七处 spec delta + 版本 patch（0.6.10 → 0.6.11）+ 全量门禁 | `pnpm -C packages/desktop run check` |

相位 1–5 与前端零耦合（可独立验证）；相位 6 只依赖 1–5 与已归档的 Change A。第 3 相位内含一次纯搬迁（emit 面上收），须与上抛接线分开审阅——前者以「56 处调用点零改动 + 既有铸造用例全绿」为判据，后者以「补发拍被就地合并」为判据。

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | 运行中即可读：static-check / test-exec 步在进程未结束时 `<report_dir>/exec-outputs/<exec_id>.log` 已存在且内容随写入单调增长（后读 ⊇ 前读；收口 `commit` 时整体替换为权威内容，故运行中内容不保证是终态内容的前缀——多尝试 / 双路交错形态下二者本就不同源） | 假命令分阶段写输出并中途读取的用例（`static_check_test.rs` / `testexec/mod_test.rs`） |
| AC-2 | 同 id 收口：running 拍 `output_ref` 与终态 `output_ref` 相等；`commit` 后 `.log` 内容 == 权威内容（`attempts_log` / `merged_output`），live 交错流被覆盖 | 用例断言两拍 ref 相等 + 终态字节逐字一致 |
| AC-3 | 命令可查：`<exec_id>.cmd` 与 `.log` 同目录；static-check 为单条 `{static_analysis} 2>&1`；test-exec 每 plan 一段 `=== plan <id> · <framework> ===` + 展开命令；老记录（无 sidecar）`read_exec_command` 返回 `None` | sidecar 内容用例 + 无 sidecar 用例 |
| AC-4 | 增量读口径：`read_exec_output_delta(…, 0)` 与整读 `read_exec_output` 逐字相等；连续增量拼接 == 全量；`next_offset` 单调不减且不超文件长度；UTF-8 多字节字符不产出半字符（源字节合法时） | `lib_test.rs` 增量读用例组 |
| AC-5 | 点击判据零改而可用：运行中的 static-check / test-execution 节点可点击并呈 stdout 面板；`change-flow-graph.tsx` 零 diff | 新增用例（`change-detail-view.test.tsx`：`status=running` + `outputRef` 节点点击开抽屉）+ `git diff --stat` |
| AC-6 | 运行中跟随：`status === 'running'` 时面板经 `ipc.readExecOutputDelta` 取增量、watch 信号到达后再次取增量且 offset 推进；非运行态零 `watch_subscribe` | `use-exec-output.test.ts`（信号驱动 + `armKey` 断言） |
| AC-7 | 终态收敛：`status` 转非 running ⇒ `watch_unsubscribe` 恰 1 次、offset 归零整读恰 1 次，权威内容重排后文本与整读一致 | 同上（转终态用例） |
| AC-8 | 三态与 `\r`：运行中 0 字节呈「运行中，等待输出…」；非运行且无记录呈「（无输出记录）」；invoke reject 呈错误态；含 `\r` 的输出在 `<pre>` 中不堆残行（归一纯函数直测） | `stdout-panel.test.tsx` + 归一函数单测 |
| AC-9 | 命令头行与复制：面板最上方显示执行的命令（多行原样），老记录显示「命令未记录」占位；复制按钮可用且不阻断内容面 | `stdout-panel.test.tsx` |
| AC-10 | watch 显式失败：父目录缺失 ⇒ `Err(WatchError::ParentUnavailable)`；文件缺失而父目录在 ⇒ `Ok` 且文件创建后出信号；`watch_target_of` 休眠语义本体不变、无退避重试代码 | `crates/infra/watch/src/lib_test.rs` 两臂用例 + 源码审查（零重试状态机） |
| AC-11 | 顺序不变量：`on_output_ref` 触发时刻 live `.log` 与父目录已在（用例在回调内断言文件存在） | `static_check_test.rs` / `testexec/mod_test.rs` 回调内断言 |
| AC-12 | 执行体语义零变：`live: None` 路径与今日逐字节等价（`run_test.rs` 既有用例全绿）；tee 不引入新的无界等待（被杀两臂仍 `FINISH_GRACE` 截断）；`crates/infra/exec/Cargo.toml` 零 diff、无 `tokio` `fs` feature | 既有用例 + `git diff --stat` 审查 |
| AC-13 | 上收零回归：`emit_node` / `StepSettle` / `now_millis` 迁入 `control.rs` 后 56 个 `emit_node` 调用点源码零改动（仅 `use` 扩一行），既有时间戳 / token 断言全绿 | `walker.rs` diff 审查 + `walker_test.rs` 铸造用例组 |
| AC-14 | bindings 与门禁：`readExecCommand` / `readExecOutputDelta` / `execOutputPath` 三 typed 包装与 `ExecOutputChunk` 类型在案、入库；生成物随前置重导出更新（过期由提交前人工核对）；`cargo` 全量测试、`vp check`、`vp test`（覆盖率 ≥ lines 90 / branches 85 / functions 90）全绿 | `pnpm -C packages/desktop run check` |
| AC-15 | 边界纪律：`crates/infra/exec/Cargo.toml` / `use-file-watch-signal.ts` / `use-change-detail.ts` / `change-flow-graph.tsx` / `crates/infra/checks/src/mutation/**` 均零 diff；`crates/core/workflow/**` 与 `crates/infra/store/**` 零 diff（零 DB / 零线格式） | `git diff --stat` 审查 |
| AC-16 | 版本纪律：`packages/desktop/package.json` `version` 0.6.10 → 0.6.11（一次 patch）；`src-tauri/Cargo.toml` 的 crate 版本行**不跟** | `package.json` / `Cargo.toml` 审查 |
| AC-17 | 手工冒烟（Windows 桌面端）：发起 run 走到 implement 的 static-check / test-execution 步 ⇒ 点击运行中节点即打开面板、命令在最上方、输出逐段涌入且「粘底跟随」（上滑后停止跟随）；步收口后内容重排一次仍与权威一致；含 `Compiling` 进度行的 cargo 输出不堆残行 | 手工 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | `drain` 改 chunk 循环触碰**全仓唯一**排水单点，四消费点（static-check / test-suite / cargo-mutants / Stryker）共用 | 一处写错即四路输出异常 | 中 | `live: None` 路径逐字节等价由既有 `run_test.rs` 全绿守住；新增 tee 用例只加不减；`drain` 单函数 ≤ 15 行、无新状态 |
| R-2 | 同步写盘拖慢 drain，让被杀两臂的有界排水提前截断 | 超时 / 取消面输出更少 | 低 | sink 用 `std::fs::File`（微秒级）；注释写明「tee 不引入新的无界等待」；被杀两臂输出本就不被四消费点消费（spec 已声明允许截断） |
| R-3 | `RunOptions.live` 让全部构造点（4 生产 + 4 测试辅助）与 2 处 runner 签名扩参，`ToolStepRequest` 的既有全部构造点（含 5 个测试文件的 mock runner）需补 `on_output_ref: None` | 编译面噪声大，容易漏改 | 中 | `Option` 缺省语义 + mock 传 `None` 即老行为（D1 的核心取舍）；mutation 的两个 mock 只需补 `None` |
| R-4 | 若后人破坏「先建文件再上抛」顺序，订阅会落到父目录缺失的失败面 | 运行中不再增量跟随（后端错误可观测），用户只在装载 / 收口时看到输出、体感为「不实时」 | 中 | 双层：R5 顺序不变量（AC-11 用例钉住「上抛时刻文件已在」）+ `WatchError::ParentUnavailable` 显式失败（AC-10，静默空转变为可观测失败）；**不引入退避重试**（替可消除的依赖付运行时代价，与仓库「类型层可证」取向相反） |
| R-5 | `StepSettle` / `emit_node` / `now_millis` 上收 `control.rs` 后 `control.rs` 从约 530 行涨到约 650 行 | 单文件体积继续变大 | 低 | 判据是语义单点归属（铸造点从 2 收敛到 1），`accumulate_node` / `attach_session` 已在其中，界线早已跨过；AC-13 以「56 处调用点零改动」为回归判据 |
| R-6 | `\r` 归一 / 半 UTF-8 字符 / 双路交错的文本形态分裂（前端归一 vs 后端 lossy） | 面板出现残行或替换符 | 中 | 硬边界：后端只在 UTF-8 字符边界收口（AC-4），`\r` 归一只在渲染前文本面做（AC-8）；收口整读规则吸收交错重排（AC-7） |
| R-7 | 前端把 `useFileWatchSignal` 用在非 running 态（收口后文件已冻结） | 多一对 IPC + 无意义订阅 | 低 | `armKey = status === 'running' ? … : null` 单条判据，AC-6 直接断言「非运行态零 `watch_subscribe`」 |
| R-8 | explore §5.1 把 `ExecOutputChunk` 的 serde / `specta::Type` derive 写在 `crates/infra/exec` 内，会连带改 `exec/Cargo.toml` 依赖面 | 打破「`exec` 依赖仅 `foundation` + tokio」与零改动承诺 | 已消除 | 本提案修正：exec 侧返回零 derive 的 `ExecOutputDelta`，出线 `ExecOutputChunk` 落壳层命令模块（AC-12 / AC-15 钉住 `Cargo.toml` 零 diff） |
| R-9 | mutation-test 不在范围内，用户在同一个抽屉里对它点不出实时输出 | 能力面不一致的观感 | 中 | 明确记账（Q2 裁定）：突变段今天没有输出记录面，纳入属范围扩张；后续 change 给它建记录面时可直接复用 `LiveExec` + 面板 |
| R-10 | 500 ms 防抖对应「静默结束后的第一行晚半秒」 | 交互体感略钝 | 低 | 输出分布是「突发 + 长静默」，半秒不足以支撑第二份常量；若将来嫌慢，降 `SIGNAL_DEBOUNCE_MS` 一处两条消费链同时变快（Change A 已定的合一取向） |
| R-11 | 夹具 / 用例对「运行中途读文件」有平台时序敏感（写后未 flush 即读） | 偶发红 | 中 | sink 每次 `write` 后立即 `flush`（同一 `File` 句柄、微秒级）；用例以「包含关系（后读 ⊇ 前读）」而非「精确长度」断言 |
