# change 详情：脚本步骤运行中 stdout 与执行命令

workflow_type: requirement

> **服务对象**：Change B `change-detail-live-stdout`（主功能）。
> 前置重构 Change A `extract-file-watch-signal-hook` 的探索见
> `openspec/explores/file-watch-signal-hook.md`——本文档不重复其内容，只承载 B 的结论，
> 对 A 的依赖收在 §8。
>
> **范围**：仅 static-check / test-execution 两类脚本步（今天唯二携带 `outputRef` 的步）。

## 0. 结论速览

| # | 决策点 | 结论 | 理由（详见） |
|---|---|---|---|
| D1 | exec_id 何时可知 | 前置铸造 + 回调上抛（`OutputRefSink`） | mock runner 零改动，id 与落盘仍单点驻 infra（§3.1） |
| D2 | 字节怎么流出来 | `drain` 改 chunk 循环 + tee 到 `LiveSink` | 唯一不改既有字节消费面的路（§3.2） |
| D3 | 前端怎么跟随 | 文件 watch 信号 + offset 增量读 | 单一数据源、天然背压、重连状态只有一个 `u64`（§3.3） |
| D4 | 命令文本放哪 | `<exec_id>.cmd` sidecar + 薄命令 | 不动 bincode 节点行；老记录优雅降级（§3.4） |
| D5 | 记录生命周期 | `begin` → `sink` → `write_command` → `commit` 同 id | 文件必先于 ref 上抛，否则 watch 静默休眠（§5.3） |
| D6 | 步状态 emit 归属 | `StepSettle` + `emit_node` 上收 `control.rs` | 铸造点单点；回调侧可字面复用（§6） |
| D7 | watch 父目录缺失 | 顺序不变量（R5）+ 显式失败（R4），**不做退避重试** | 问题被消除而非兜底（§7） |
| D8 | 防抖常量 | 沿用 A 的 `SIGNAL_DEBOUNCE_MS = 500`，零新增 | 另设 250 ms 是新增分歧换微小收益（§8.2） |
| D9 | push 流式（Channel 推 chunk） | **否** | 粒度天然是批 + Channel 无背压 + 双数据源漂移（§4.4） |
| D10 | DB 改动 | **零** | `RunNodeRecord` bincode v1 不动（§5） |

## 1. 需求

用户两句话是一件事的两个面：

```
(a) 脚本步骤运行中即可查看 stdout       ← 今天只有「已收口」的步骤能看
(b) stdout 面板最上方显示「执行的命令」  ← 今天命令文本没落盘、没有任何出口
```

痛点强度由预算决定：`STATIC_CHECK_TIMEOUT = 1200s`、`SUITE_COMMAND_TIMEOUT = 1200s`，
外加 `RetryPolicy::transient()` 的 `[5,10,20,40]s` 退避——最长 20 分钟只看到一个
`animate-pulse` 的「运行中」节点。

## 2. 现状（为什么今天看不到）

### 2.1 后端 emit 时序：`outputRef` 只在终态出现

```
step_static_check (walker.rs:966..)
  ├─ emit_node(StepSettle::running())          ← 节点 status=running，output_ref=None   ★
  ├─ run_tool(ToolCommand::StaticCheck{..})    → LocalToolSteps → ProcessStaticCheck::run
  │     └─ exec::run_shell(...)                → 双管道 drain 进内存 Vec，全程零对外字节
  │     └─ exec::record_exec_output(...)       ← ★★ exec_id 到这里才被铸造
  └─ emit_node(...with_output_ref(Some(id)))   ← 终态才带 ref
```

`with_output_ref` 的 6 个调用点全在终态 / fault 收口（walker.rs `1008/1019/1243/1254` +
`settle_fault` `3748/3766`；test-exec `1152/1164/1327/1339`）。**running emit 恒 `output_ref=None`。**

### 2.2 前端两道门都锁在 `outputRef` 上

```
change-flow-graph.tsx  onNodeClick:
   if (node.role === null && node.outputRef === null) return;   ← 运行中工具步：不可点击
detail-drawer.tsx  DrawerContent:
   role !== null        → 会话转录
   outputRef !== null   → StdoutPanel
   其余                  → 空态占位
use-exec-output.ts: 依赖 [root, changeName, execId] —— 运行中 execId 不变 ⇒ 永不刷新
```

⇒ 不是「面板不刷新」，而是**抽屉压根打不开**。

### 2.3 输出记录的生命周期（`infra/exec` 单点）

```
<changes_root>/<change name>/reports/<test|baseline>/exec-outputs/<exec_id>.log
                          ▲                                ▲
                change_reports_in (layout 单点)      mint_exec_id = "exec-<millis>-<seq>"
```

- `record_exec_output(root, change, slot, bytes) -> Result<exec_id, _>`：铸造 + 一次性
  `fs::write`（零流式、零 sidecar、零命令信息）。
- `read_exec_output_in(change_dir, exec_id)`：`[Test, Baseline]` 双锚探测，整文件读、UTF-8 lossy。
- IPC `read_exec_output(root, changeName, execId) -> Option<String>`（`commands/changes/mod.rs:154`）：
  按 change **name** 在工作树候选序里定位，首中即返。
- **无人清理 `exec-outputs/`**（无 GC）；runner 只清 `reports/test/<plan>` 子目录
  ⇒ 往里塞 live 文件不会被误删。

## 3. 四个决策

### 3.1 D1：exec_id 前置铸造 + 回调上抛

四条候选，取 B：

| 方案 | 做法 | 变更面 | 判定 |
|---|---|---|---|
| A 前置铸 + 显式下沉 | 新 port `begin(root,change,slot)->exec_id`，walker 铸 → running emit 带 ref → `ToolCommand` 透传 | `ToolCommand` 字段 + runner 签名 + **所有 mock runner** | 最显式，ripple 最大 |
| **B 回调上抛** | `ToolStepRequest.on_output_ref: Option<OutputRefSink>`；infra 铸 live id 后回调一次 → walker 补发一拍 running+ref | 单点 `run_tool` 构造 + runner 内部；**mock runner 零改动**（不回调即老行为） | ✅ 取此 |
| C 隐式 live 槽位 | exec 内进程级注册表按 `(root,change,slot)` 挂 live 记录，runner 签名零改 | 全局可变状态，测试要清场 | 与「显式参数」风格相悖 |
| D 前端按步身份问 | 新 IPC 以 `(phase,attempt,step,instance)` 取 live | runner 仍不知步身份，退化成 A/C | 不成立 |

**红利**：`role===null && outputRef!==null` 立刻可点 ⇒ **前端点击判据一行不用改**；
`accumulate_node` 已有的「`output_ref` some 才覆写」直接吃下补发拍，DB / 线格式零改。

### 3.2 D2：tee 到 `LiveSink`（`drain` 单点）

`Stdio::piped()` 早已在用（`run.rs:265-266`），**管道不是瓶颈**。`read_to_end` 也**不是**
「等进程结束才读」——它循环 `poll_read` 直到 EOF。**唯一缺口是读到 chunk 时不通知外部。**

```
子进程 write(2)
   │   ① 子进程自身 stdio 缓冲      ← 不可控，但 Rust / Node 系基本实时（§4.3）
   ▼
OS 匿名管道（~4KB 环）
   │   ② 我们的读粒度              ← ✅ 已具备（read_to_end 内部即 chunk 循环）
   ▼
Rust Vec<u8>（attempt 局部）
   │   ③ chunk 抵达时通知外部      ← ❌ 唯一缺口
   ▼
exec-outputs/<id>.log  →  watch 信号 → 前端
```

三条候选，取 tee：

| 路线 | 实时性 | 代价 |
|---|---|---|
| **T tee（管道 + 自己写文件）** | chunk 级（毫秒） | 无。`diagnostics` / `attempts_log` / `merged_output` 等消费面零改 |
| F 子进程直写文件（`Stdio::from(File)`） | 轮询粒度 50–200 ms | **收不到 bytes** → `static_check` 的 `diagnostics`（进反馈边 prompt）、`attempts_log`、`fault_text` 全断；stderr 要第二文件、testexec 交错序丢失 |
| P pty（ConPTY / openpty） | 行级 | Windows ConPTY 异步读写 + 句柄接管，成本极高 |

F 的诱惑在「OS 直写、零 Rust 中转」，但既有消费面**依赖内存字节**——F 把它们全推倒。
**T 是唯一「既有消费面一行不改」的路。**

### 3.3 D3：watch 信号 + offset 增量读

| 方案 | 机制 | 判定 |
|---|---|---|
| P 轮询 | 面板打开且 running 时 ~500ms 一次整读 | 可用但读放大 |
| **W 文件 watch + offset 增量** | `watch_subscribe` → 信号防抖 → `read_exec_output_delta(from)` | ✅ 取此 |
| S push（Channel 推 chunk） | 壳层新命令 + 内存 registry | 否，见 §4.4 |

```
drain ──tee──► <exec_id>.log          ← 单一数据源（唯一真相）
                     │
                     │ watch 信号（零载荷）
                     ▼
              ExecOutputSignal ──► 前端防抖 → readExecOutputDelta(fromOffset)
                                              │
                                              ▼
                                  { text, nextOffset }   ← 后端在 UTF-8 字符边界收口
```

四个理由：
- **零签名改动**——落盘发生在 runner 肚子里，通知源是 OS，不需要把 channel 穿进 `drain`。
- **单一数据源**——避免 push 的「chunk 流 vs 权威 commit」双源漂移。
- **天然背压**——读多少算多少，不像 Tauri `Channel` 那样前端慢就内存涨。
- **重连状态只有一个 `u64`**。

`nextOffset` **只由后端产出**：全量读也走 `readExecOutputDelta(…, 0)`，前端零字节计数
⇒ 「JS 字符数 vs UTF-8 字节数」的口径分裂根本不会出现。

### 3.4 D4：命令文本走 sidecar

| 方案 | 做法 | 判定 |
|---|---|---|
| **M1 sidecar** | 与 `.log` 同目录 `<exec_id>.cmd`；新 IPC `read_exec_command` | ✅ 取此 |
| M2 扩既有 wire | `read_exec_output` 返回 `{ command, text }` | 改既有命令形状 ⇒ bindings/前端/spec 三处改，收益等同 |
| M3 进节点行 | `ChangeStepState`/`RunNodeRecord` 加字段 | bincode version bump + 升级链 + 库史行缺失；命令属记录面不属图节点面 |

M1 的两个好处：**不改既有 wire**（`read_exec_output` 一字不动，保住 spec 的「零变」承诺）；
**老记录无 sidecar ⇒ 面板优雅降级「命令未记录」**。

写入时机：runner 解析出命令后、spawn 前。
- static-check：一条（`static_analysis` 配置值 + ` 2>&1`）
- test-execution：每 plan 一条，追加格式 `=== plan <id> · <framework> ===\n<expanded>`

## 4. 为什么不做 push 流式（D9 的反面论证）

### 4.1 技术上可行

`tokio::sync::broadcast` 落在 `sync` feature（workspace `Cargo.toml:115`，`default-features = false,
features = ["sync"]`），**不依赖 `rt`**，`send()` 同步非阻塞、满了丢最老一条。故可在 `drain()` 的
async future 里直接 publish —— 不起 task、不 spawn，`exec/Cargo.toml` 的「零 rt / 零 spawn」纪律不破。

### 4.2 但成本卡在 trait 边界

`StaticCheckRunner::run(&self, root, name, slot, cancel)` 的签名里**没有位置**放投递目标：

```
壳层 change_flow_start          ← Channel 在这里
  ▼
core  LocalToolSteps            ← ToolStepPort
  │  static_check.run(root, name, slot, cancel)   ← 4 参数，无 sink 位
  ▼
infra ProcessStaticCheck → exec::run_shell → drain()   ← 想 publish 的地方
```

| 绕法 | 代价 |
|---|---|
| Push-A 显式穿参 | 3 个 runner trait + 2 实现 + **~11 处测试 mock** |
| Push-B 进程级注册表 | 零签名改动，但破「零全局可变状态」风格，测试要清场 |

对照之下，**拉-通知 + offset 的落盘路径天然零签名改动**——落盘本该在 runner 内部发生。

### 4.3 收益又趋近于零（三条，从硬到软）

1. **粒度天然是「批」**：一次 `read` 拿到 OS 当前就绪的字节，cargo 一行 `Compiling foo` 约 40 字节，
   一次 read 会带回好几行 ⇒ 前端看到的仍是「一段段文字」，不是字符流。
2. **时间分布是「突发 + 长静默」**：编译时一秒几十行、然后静默三十秒。突发时 push 被打包成批
   （与拉无差），静默时无物可推（与拉无差）。唯一差别是「静默结束后的第一行」——500 ms 防抖
   在此退让半秒，且可调。
3. **Tauri `Channel` 无背压**：`on_event.send()` 同步序列化 + 投递 webview 队列，前端慢则内存涨。
   agent 轨道敢这么用是因为 token 流「高频但小量」；编译输出是「低频但大量」，正相反。

### 4.4 还有双数据源漂移

push 的 chunk 流是真交错，而 testexec 收口的权威 `commit` 是「stdout 前缀 + stderr 后缀」——
收口瞬间面板会跳变。拉模型同样会跳，但**只有一个源头**（文件被覆盖），跳变是可归因的单事件。

## 5. 骨架

### 5.1 `crates/infra/exec/src/lib.rs`

```rust
/// 实时输出落盘口（`RunOptions` 可选面；两路 drain 共用同一实例）
#[derive(Clone)]
pub struct LiveSink(Arc<dyn Fn(&[u8]) + Send + Sync>);

impl LiveSink {
    pub fn new(append: impl Fn(&[u8]) + Send + Sync + 'static) -> Self { Self(Arc::new(append)) }
    pub(crate) fn absorb(&self, bytes: &[u8]) { (self.0)(bytes) }
}

/// 手写 Debug：`Arc<dyn Fn>` 不实现 Debug，而 `RunOptions` 派生了 `Debug`（红线 ②）
impl std::fmt::Debug for LiveSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("LiveSink(..)") }
}

const EXEC_COMMAND_SUFFIX: &str = "cmd";

/// 运行中可回查的输出记录：先建文件 → 边跑边追加 → 收口用同一 id 覆盖为权威内容。
/// 磁盘面与 `record_exec_output` 同锚同口径（`<report_dir>/exec-outputs/`）。
pub struct LiveExec {
    exec_id: String,
    log_path: PathBuf,
    writer: Arc<Mutex<Option<std::fs::File>>>,
}

impl LiveExec {
    /// 前置铸造：`create_dir_all` + 建空 `.log`。
    /// **必须先于任何 watch 订阅**（§7：父目录缺失 → 监听休眠且不报错）
    pub fn begin(root: &Path, change: &str, slot: ReportSlot) -> Result<Self, String> { /* … */ }

    pub fn exec_id(&self) -> &str;
    pub fn log_path(&self) -> &Path;

    /// 追加口（交给 `RunOptions.live`）；内部 Mutex 支撑两路 drain 共用
    pub fn sink(&self) -> LiveSink;

    /// 命令 sidecar（`<exec_id>.cmd`）：spawn 前写
    pub fn write_command(&self, command: &str) -> Result<(), String>;

    /// 收口：take 掉句柄后同 id 覆盖为权威内容（= `record_exec_output` 的前置 id 变体）
    pub fn commit(&self, output: &[u8]) -> Result<String, String>;
}

/// 既有 API 降为薄包装（语义零变，仍单点）
pub fn record_exec_output(root: &Path, change: &str, slot: ReportSlot, output: &[u8])
    -> Result<String, String>
{ LiveExec::begin(root, change, slot)?.commit(output) }

/// 命令 sidecar 回查（与 `read_exec_output_in` 同双锚探测）
pub fn read_exec_command_in(change_dir: &Path, exec_id: &str) -> Option<String>;

/// 增量读：返回自 from_offset 起的新增文本 + 下次游标
pub fn read_exec_output_delta_in(change_dir: &Path, exec_id: &str, from_offset: u64)
    -> Option<ExecOutputChunk>;
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecOutputChunk {
    /// 新增文本（后端在 UTF-8 字符边界收口，前端永不拼出半字符）
    pub text: String,
    /// 下次读取游标（offset 的唯一产出方在后端）
    pub next_offset: u64,
}
```

### 5.2 `crates/infra/exec/src/run.rs`（tee 单点）

```rust
pub struct RunOptions {
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
    pub cancel: CancelFlag,
    pub retry: RetryPolicy,
    /// 实时输出观察者（None = 纯内存，既有语义零变；11 处构造点补 `live: None`）
    pub live: Option<LiveSink>,
}

/// 管道读尽至 EOF——chunk 循环 + tee（原 `read_to_end` 形态退役）
async fn drain<R: tokio::io::AsyncRead + Unpin>(
    pipe: &mut Option<R>,
    buffer: &mut Vec<u8>,
    live: Option<&LiveSink>,
) {
    let Some(pipe) = pipe.as_mut() else { return };
    let mut chunk = [0u8; DRAIN_CHUNK_BYTES]; // 8192
    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) => break,                                          // EOF
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);                // 既有消费面
                if let Some(live) = live { live.absorb(&chunk[..n]); } // 实时面
            }
            Err(_) => break,   // 沿退役语义：读失败不改变「已读即输出」结论
        }
    }
}
```

`attempt_once` 多一个 `live: Option<&LiveSink>` 形参；两处调用点（`await_exit` 自然退出臂 /
被杀两臂的有界排水）原样透传 ⇒ 统一语义，零分叉。

### 5.3 `crates/infra/checks`：两 runner 接入（三步顺序是支点）

```rust
// static_check.rs（命令单值）
let live = exec::LiveExec::begin(Path::new(root), name, slot)?;
live.write_command(&command).ok();                                // ① sidecar 先落
if let Some(on_ref) = on_output_ref { on_ref(live.exec_id()); }   // ② ★ 文件已在，才上抛
let output_ref = Some(live.commit(&environment::attempts_log(&run)).ok());  // ③ 同 id 覆盖
```

```rust
// testexec/runner.rs（每 plan 一值）→ testexec/mod.rs 汇总
// ① 循环内每 plan：live.write_command 追加一行 `=== plan <id> · <framework> ===\n<expanded>`
// ② 全 plan 跑完（或 fault 收口）：live.commit(&merged_output)
```

**①→②→③ 的顺序不可倒**：必须先建目录建文件，再上抛 exec_id；前端拿到 ref 时父目录必然已在。
若倒置，watch 会**静默休眠**（§7.2）。

### 5.4 `crates/core/orchestration`

```rust
/// 输出引用上抛回调（exec_id 铸造后立即触发一次；触发时 live 文件已存在）
#[derive(Clone)]
pub struct OutputRefSink(Arc<dyn Fn(&str) + Send + Sync>);

// 手写三 derive（`Arc<dyn Fn>` 不支持）；`PartialEq` 取身份语义——沿 CancelFlag 先例
impl std::fmt::Debug for OutputRefSink { /* "OutputRefSink(..)" */ }
impl PartialEq for OutputRefSink { fn eq(&self, o: &Self) -> bool { Arc::ptr_eq(&self.0, &o.0) } }
impl Eq for OutputRefSink {}

pub struct ToolStepRequest {
    pub root: String,
    pub command: ToolCommand,
    pub cancel: CancelFlag,
    /// 缺省 None = 老行为（mock runner 零改动）
    pub on_output_ref: Option<OutputRefSink>,
}

pub trait StaticCheckRunner: Send + Sync {
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag,
           on_output_ref: Option<OutputRefSink>) -> BoxToolFuture;
}
// TestExecutionRunner 同型（mutation 见 §10 待定）
```

```rust
// control.rs（新增，与 accumulate_node 同居）
impl RunGuard {
    /// 'static 发射器：回调跨 port 边界时只能持 Arc + 键
    pub fn emitter(&self) -> StepEmitter {
        StepEmitter { key: self.key.clone(), control: Arc::clone(&self.control) }
    }
}

#[derive(Clone)]
pub struct StepEmitter { key: (String, String), control: Arc<ChangeFlowControl> }

impl StepEmitter {
    /// 补发带输出引用的 running 步状态行（`accumulate_node` 的「ref some 才覆写」直接吃掉）
    pub fn attach_output_ref(&self, phase: &str, attempt: u32, step: ChangeStepKind,
                             instance_no: u32, exec_id: &str);
}
```

```rust
// walker.rs run_tool：identity 由调用方传入（static_check / testexec 共 4 处）
async fn run_tool<V>(tools, guard, root, command, label, identity: Option<StepIdentity>) -> … {
    let emitter = guard.emitter();
    let request = ToolStepRequest {
        root: root.to_owned(),
        command,
        cancel: guard.cancel_flag(),
        on_output_ref: identity.map(|id| OutputRefSink(Arc::new(move |exec_id| {
            emitter.attach_output_ref(&id.phase, id.attempt, id.step, id.instance_no, exec_id);
        }))),
    };
    // …其余不变
}
```

**`'static` 约束是硬性的**：`RunGuard { key: (String,String), control: Arc<ChangeFlowControl> }`
两字段都 `'static`，回调不能借用 `&RunGuard`，只能从 `Arc<ChangeFlowControl>` + 复合键重建。
这也是 D6（emit 上收）的直接起因。

### 5.5 壳层（`src/commands/changes/mod.rs` + `all_commands!`）

```rust
/// 布局派生 log 绝对路径（双锚探测；目录名知识不下沉前端）—— 照 explore_doc_path
pub fn exec_output_path(stores, root: String, change_name: String, exec_id: String) -> Option<String>;

/// 命令 sidecar 读取
pub fn read_exec_command(stores, root: String, change_name: String, exec_id: String) -> Option<String>;

/// 增量读（from_offset = 0 即全量；**唯一 offset 产出方**）
pub fn read_exec_output_delta(stores, root: String, change_name: String, exec_id: String,
                              from_offset: u64) -> Option<ExecOutputChunk>;
```

`read_exec_output` 原命令**一字不动**（spec「零变」承诺不破）。

### 5.6 前端 `views/changes/hooks/use-exec-output.ts`（重写）

⚠️ 现状不一致：本 hook 用 `commands.readExecOutput`（裸生成物），绕过了 `ipc` 代理的 zod 校验层；
同目录 `use-explore-doc.ts` 用 `ipc`。重写时统一走 `ipc`（否则 `desktop-ipc-runtime-guard` 对
这条命令失效）。

```ts
export function useExecOutput(params: {
  root: string | null;
  changeName: string | null;
  execId: string | null;
  /** 步状态：running → 挂 watch 增量；转终态 → 退订 + offset 归零整读一次 */
  status: ChangeStepStatus | null;
}): { command: string | null; text: string | null; error: string | null } {
  const offset = useRef(0);

  // ① 命令头行：一次读，失败不阻断内容面
  useEffect(() => { /* ipc.readExecCommand(...) → setCommand */ }, [root, changeName, execId]);

  // ② 内容面
  useEffect(() => {
    offset.current = 0;
    const readDelta = () =>
      ipc.readExecOutputDelta(root, changeName, execId, offset.current).then((chunk) => {
        if (cancelled || chunk === null) return;
        offset.current = chunk.nextOffset;
        if (chunk.text !== '') setText((prev) => (prev ?? '') + chunk.text);
      });

    // 收口：整读一次（权威内容可能与 live 交错流重排）
    if (status !== 'running') { readDelta(); return; }

    // 运行中：路径 → 订阅 → 信号防抖 → 增量读
    //   A 落地后此处改为 useFileWatchSignal({ armKey, resolvePath, onSignal: readDelta })
    readDelta();   // 装载首拍：订阅前已产生的字节
    // …
    return () => { /* 卸载退订 + 防抖取消 */ };
  }, [root, changeName, execId, status]);
}
```

**一条状态规则解决两件事**：`status` 转终态即 offset 归零整读 —— 同时处理「权威内容重排」
与「文件被截断后 offset 越界」。

## 6. D6：`StepSettle` + `emit_node` 上收 `control.rs`

**先纠正一处在早前讨论中被高估的成本**：实际是 **0 处调用点改动**（不是 54 处）。
`walker.rs` 顶部已有 `use crate::control::{ChangeFlowControl, RunGuard};`，扩这一行即可，
53 个调用点的 `emit_node(guard, ...)` 名字与形态一概不动。

| | 甲：上收（取此） | 乙：`StepEmitter` 自铸时间 | 丙：`trait StepEmitTarget` |
|---|---|---|---|
| 移动面 | walker.rs **428–542 行**（≈115 行）+ `now_millis` | 0 | 0 |
| 调用面 | **0**（1 行 import） | 0 | 0 |
| 铸造点 | **1** | **2** | 1 |
| 行为变化 | 无 | 无 | 无 |
| 判否理由 | — | `ChangeStepState` 加字段时两处都要填、编译器不提醒；保首见/终态铸造注释写两遍 | 只有两个实现者的 trait，撞「不为单次使用造抽象」 |

**判据不是改动量，而是语义单点归属**：仓库一贯用「让编译器兜住」换长期正确性
（`RunOptions` 不给 `Default`、超时必填「类型层不可表达」）。甲延续此取向。

**甲的反面如实记**：`control.rs` 从 ≈530 行涨到 ≈650 行。但 `accumulate_node` / `attach_session` /
`is_session_step` 已在其中——这条界线早就跨过了，甲是补认而非新开。
附带收益：`emit_node`（造一行）与 `accumulate_node`（并一行）相邻，身份键
`(phase, attempt, step, instance_no)` 的写侧与并侧同屏可对读。

## 7. D7：watch 父目录缺失 → 顺序不变量 + 显式失败，**不做退避重试**

### 7.1 历史决策的三个理由（有据可查）

出处：`openspec/changes/archive/2026-09-24-desktop-explore-page/design.md` **D5** +
`openspec/specs/desktop-file-watch/spec.md` + `crates/infra/watch/src/lib.rs` crate 文档。

```rust
fn watch_target_of(target: &Path) -> Option<PathBuf> {
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty())?;
    parent.is_dir().then(|| parent.to_path_buf())   // ← None 时 subscribe 仍返 Ok
}
```

1. **零 fs 副作用纪律**：watch 是通用通道，MUST NOT 替调用方造目录（「不预建目录」）。
   造目录属写入面职责。
2. **不递归挂祖先**：crate 文档明写。递归监听大子树的成本与「单文件 watch」定位冲突。
3. **恢复靠页面生命周期**：D5 原文「页面生命周期内**无中途重挂**」「父目录（重）出现后需重新
   订阅生效……页面重开自然重挂」——**接受休眠**。

顺带一处定义漂移：spec 里「目标缺失不报错」说的是**文件**缺失，不是父目录缺失。
父目录缺失只在 crate 文档里补了一句余量，不是被论证过的主路径。

### 7.2 盲区：两种「缺失」被合并成同一个 `None`

```
A：文件缺失、父目录在  → watch 父目录，文件 create 时出事件     ✅ 设计意图
B：父目录也缺失        → watcher.watch() 压根没调            ⚠️ 静默休眠
                              ↓
                   Result 仍是 Ok，调用方分辨不出 A / B
```

对原消费者（explore）不可达：`openspec/explores/` 恒存在。
**对本消费者恰好在默认路径上**——`reports/test/exec-outputs/` 第一次记录前不存在。

### 7.3 五条方案与结论

| 方案 | 成本 | 判定 |
|---|---|---|
| R1 线程退避重试 | `Watcher` 变异步建立 + 停止标志 + Drop 竞态，~50 行 + 每订阅一线程 | 可行但重 |
| R2 挂最近祖先 + 递归 | ~6 行零状态，但祖先可能是工作区根 → 全树递归监听，无上界 | 有坑 |
| R3 渐进下挂 | 状态机 + 控制线程（notify 回调线程内重建 watcher 有重入风险） | 复杂度≈R1 |
| **R4 静默变响** | `Err(WatchError::ParentUnavailable)`，~5 行 + 一枚举变体 + 一条用例 | ✅ 取此 |
| **R5 顺序不变量** | 0（§5.3 已如此排） | ✅ 取此 |

**结论：R5 + R4，不做退避重试。** 理由：

1. D1 的回调设计**已顺带保证顺序**——`LiveExec::begin` 建目录建文件 → `on_ref(exec_id)` 上抛
   → 前端拿到 ref 时父目录必然已在。问题被**消除**，不是被兜底。
2. 退避重试是替一个可消除的依赖付运行时代价，与仓库「类型层可证」的取向相反。
3. R4 作安全网：万一顺序被后人破坏，`Err` 让前端能降级到 500 ms 轮询兜底，
   **而不是永久空转**。静默失效是唯一不可接受的结局。

对共享 crate 的改动收敛为**一条枚举变体 + 5 行判定**，`watch_target_of` 的休眠语义本身不动。
R4 不改任何消费侧代码：explore 的 `.catch(() => {})` 已吞掉（父目录恒存在，实际零触发）。

## 8. 对 Change A 的依赖

### 8.1 消费 A 的三件（其余零耦合）

| A 的产物 | B 如何消费 |
|---|---|
| `useFileWatchSignal({ armKey, resolvePath, onSignal })` | `use-exec-output.ts` 调一次：`armKey = status === 'running' ? \`${root}\0${execId}\` : null`——非运行态不订阅（收口后文件冻结） |
| `SIGNAL_DEBOUNCE_MS = 500` | 直接沿用，零新增常量 |
| 「路径由命令层派生」范式（D6 先例） | B 的 `exec_output_path` 照 `explore_doc_path` 同构 |

B **不需要** A 的 `ExploreDocState` / `fetchDoc` / `useDebouncedBump`。

### 8.2 D8：不另设 250 ms 防抖

防抖常量收敛为一个 `SIGNAL_DEBOUNCE_MS = 500`，explore 与 exec 输出共用。

理由是合一取向：另设 250 ms 是**新增分歧换微小延迟收益**。若将来嫌慢，降这一处常量，
两条消费链同时变快——这才是「合一」应有的物理形态。

诚实承认代价：500 ms 对应「静默结束后的第一行晚半秒出现」。但脚本输出是「突发 + 长静默」
分布，一行晚半秒不足以支撑第二份常量。

### 8.3 依赖是单向且薄的

```
A:  useFileWatchSignal(通用信号消费)  ←──  EXPLORE 详情页（既有）
                                     ←──  exec 输出面板（B 新增）
B:  LiveExec / tee / 三个 IPC / stdout 面板   （A 零感知）
```

A 不认识 B 的任何类型（`ExecOutputChunk` / `ChangeStepStatus` / `outputRef`），只知道
`armKey: string | null` 与 `onSignal: () => void`。故 **A 可先于 B 独立归档**，
B 也可在 A 归档后单独回滚而不影响 A。

### 8.4 若 A 未落地的退路

`use-exec-output.ts` 改为内联一份 `Channel` + 防抖（约 25 行，形态照抄 explore 现状）。
代价：仓库回到「三份防抖」，且 B 的 diff 混入本属 A 的样板。**不预先实现退路。**

### 8.5 tasks 顺序约束

B 的前端相位 MUST 排在 A 归档之后（否则无法 import 公共件）。B 的后端相位
（§5.1–5.4）与 A **完全无依赖**，可并行推进。

## 9. 触点清单

**后端**
- `crates/infra/exec/src/lib.rs`：`LiveExec` / `LiveSink` / sidecar / 增量读 / 路径派生复用 `exec_output_dir_in`
- `crates/infra/exec/src/run.rs`：`RunOptions.live` + `drain` chunk 循环（11 处构造点补 `live: None`）
- `crates/infra/exec/Cargo.toml`：**零改动**（sink 用 `std::fs::File`，不引 tokio `fs`——见 §11 红线 ①）
- `crates/infra/checks/src/static_check.rs`（record 点 `:124`）
- `crates/infra/checks/src/testexec/{mod.rs,runner.rs}`（`outcome`/`fault_outcome` `:253/:281`；`run_shell` `:165`）
- `crates/core/orchestration/src/port.rs`：`ToolStepRequest.on_output_ref` + 两 trait 签名
- `crates/core/orchestration/src/steps.rs`：透传
- `crates/core/orchestration/src/control.rs`：`StepEmitter` + 上收的 `StepSettle`/`emit_node`/`now_millis`
- `crates/core/orchestration/src/walker.rs`：`run_tool` 组装回调；import 扩一行
- `crates/infra/watch/src/lib.rs`：`WatchError::ParentUnavailable`（R4）
- `src/commands/changes/mod.rs` + `src/commands/mod.rs`：三个新 IPC 入 `all_commands!`

**前端**
- `views/changes/hooks/use-exec-output.ts`：重写（走 `ipc`、消费 A、offset 增量、命令头行）
- `views/changes/flow/stdout-panel.tsx`：命令头行 + 运行中空态 + 粘底滚动
- `views/changes/flow/detail-drawer.tsx`：给面板下传 `status`
- `views/changes/change-detail-view.tsx`：如需刷新键（走 D3 后**不需要**）

## 10. 规格 / 纪律面 / 待定

### 10.1 spec 触点

- `desktop-change-flow-view`：stdout 面板节要写「运行中步携带 live `outputRef`、面板可实时取出
  增量、命令头行、空态语义」；点击判据文案从「带 outputRef 的**已收口**节点」改为
  「带 outputRef 的节点（运行中即有）」
- `desktop-exec-runtime`：现写「`record_exec_output` / `read_exec_output` / `mint_exec_id` **零变**」
  ——本轮要改成「零变指语义与既有签名；新增 live 记录 + commit 同 id + sidecar」
- `desktop-checks-domain`：两 runner 的「输出记录单点」措辞（record → begin/commit）
- `desktop-ipc-type-bindings`：三个新命令入册
- `desktop-file-watch`：R4 枚举变体（A 归档时的那行 Module Contract 修改**不在 B 范围**）

### 10.2 版本纪律

`packages/desktop/package.json` patch +1（生成物随前置重导出更新）。

### 10.3 待定问题

1. **命令呈现**：static-check 一条短命令、test-exec 每 plan 一条长命令（`cargo llvm-cov --workspace …`）
   ——单行截断 + hover/展开，还是原样多行换行 + 复制按钮？
2. **mutation-test 是否纳入**：它今天完全没有输出记录（不走 `record_exec_output`）。纳入则需先给它
   建记录面，属范围扩张。
3. **粘底滚动**：要不要「粘底自动跟随 / 用户上滑即停止粘底」这套终端行为？
4. **节点卡片内露尾**：运行中是否也在节点卡片直接露 3 行尾部（不进抽屉就能瞄）？
5. **sidecar 格式**：静态一条纯文本 vs 统一 JSON（`{command}` / `{commands:[…]}`）——
   纯文本更省事，JSON 更好扩展（例如带 `startedAt`）。

## 11. 边界与风险

### 11.1 三条仓库红线（都会踩到）

1. **不许加 tokio `fs` feature**：`exec/Cargo.toml` 注释明写「不引 rt / rt-multi-thread / full
   ——设计不 `tokio::spawn`，future 只持局部可变借用，故无运行时需求」。`tokio::fs` 依赖 `rt`
   （内部 `spawn_blocking`），一加即破 ⇒ sink 落盘用 `std::fs::File`，chunk 级同步写几微秒。
2. **`RunOptions` 的 `#[derive(Debug, Clone)]`**：`Arc<dyn Fn(&[u8])>` 不实现 `Debug`，
   新字段会让 derive 挂 ⇒ `LiveSink` 手写 `Debug`。
3. **背压与 `FINISH_GRACE`（2s）**：sink 慢盘阻塞会拖住 drain，让被杀两臂的有界排水提前截断。
   既有设计允许截断，但注释须写明「tee 不引入新的无界等待」。

### 11.2 语义边界

- **`\r` 进度行**：cargo 的 `Compiling …` 刷新在 `<pre>` 里不会原地刷新、会堆成残行
  ⇒ 前端须把 `\r` 归一（只留最后一个 `\r` 之后的内容）或做 CR/ANSI 收敛。**与实时同批处理**。
- **UTF-8 半字**：追加中途读到半个多字节字符 → lossy 出 `�`，下一拍自愈（不崩）。
- **两路交错**：testexec 的 stdout/stderr 并发 drain → live 流是真交错，权威 commit 是
  「stdout 前缀 + stderr 后缀」⇒ 收口会重排一次。由 §5.6 的「转终态整读」规则吸收。
- **空文件 ≠ miss**：运行中 0 字节时读回 `""`——面板必须靠 `status` 区分「运行中，等待输出…」
  与「（无输出记录）」。
- **停止 / 超时**：击杀后 `commit` 仍写已收字节，节点 stopped + ref ⇒ 面板读到部分输出
  （正合用户预期）。
- **无命令配置**（`static_analysis` 空 → 零 spawn 直过）：**不该建 live 记录**，终态 ref 仍 None。

### 11.3 实时性上限在子进程侧

- **Rust 系（cargo / rustc / cargo llvm-cov）**：stdout 为 `LineWriter`（行缓冲），进度行走 `\r`
  刷新 → 真·实时。
- **Node 系（vite-plus / stryker）**：`process.stdout` 在 pipe 下是 async stream，每次 write 直进
  libuv 队列 → 实时。
- **Python / C 程序**：非 TTY 时 libc **block buffering（4–64KB）**，`print` 攒满一块才 flush
  → 成块到达。两个脚本步都不涉及。

⇒ 「20 分钟只看到 animate-pulse」可被真正解决，不会退化成「每 8KB 蹦一次」。

### 11.4 仓库零先例

`grep Stdio::piped` 全仓 7 处（`exec/run`、`agent/sdk/bash`、`checks/testexec/detect`、
`vcs/{git,bootstrap}`），**没有一处做流式 drain**——`agent/sdk/bash.rs:108` 的 `drain` 同样是
`read_to_end` 一次收。agent 会话的实时流（`AgentRunMessage::Event`）是 LLM SDK 侧事件，
与子进程管道无关。

⇒ chunk 级 tee 是本仓新地面，但技术纵深很浅（一个函数 + 一个 sink 类型）。

## 12. 渐进交付顺序

每一步独立可验：

1. **exec**：`LiveExec` / `LiveSink` / `begin-append-commit` / sidecar / 增量读
   + `RunOptions.live` 观察者（纯 infra，先测）。
2. **checks**：static_check / testexec 改用 live 记录（同 id 收口），命令 sidecar 落盘。
3. **core**：`OutputRefSink` + `ToolStepRequest.on_output_ref` + `run_tool` 组装
   + D6 上收（`StepSettle`/`emit_node`/`now_millis` → `control.rs`）。
4. **watch**：`WatchError::ParentUnavailable`（R4）。
5. **壳层**：三个薄命令 + `all_commands!` + bindings 重导出。
6. **前端**：命令头行 + `status` 下传 + offset 增量 + `\r` 归一 + 空态（A 归档后接
   `useFileWatchSignal`）。
7. **收口**：spec 五处（§10.1）+ 版本 patch + `bindings:export` 重导出核对。

第 1–5 步与 Change A **完全无依赖**，可与 A 并行；第 6 步需 A 就位。
