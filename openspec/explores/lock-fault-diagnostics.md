# 文件锁：诊断归因 / RunFault 语义 / 瞬态重试

workflow_type: requirement

> **一句话**：构建产物被外部进程占用时，产品应**点名占用者**、把「环境坏」与「检查红」
> 彻底分开、并用有限重试自愈 —— 而不是让用户对着「请去修基线」的文案猜。
>
> **状态**：根因已定论（2026-10-19 实测），设计完成，未实施。
> **前置依赖**：`exec-run-shell.md`（重试与取消都挂在 `exec::run_shell` 上）。
> 若该篇未合入，本篇需先自行建立等价执行骨架。

---

## 1. 根因（已实测确认）

### 1.1 凶手：两套企业级安全软件的内核过滤驱动

现场监视（`static-check-file-lock.md` §12）抓到：

```
[11:05:33.791] LOCKED export_bindings.exe <= System [pid 4], export-bindings.exe
[11:05:34.172] LOCKED export_bindings.exe <= System [pid 4], TBProtect.exe [pid 30684], ...
[11:05:34.553] LOCKED export_bindings.exe <= export-bindings.exe, Huorong Enterprise Security Endpoint Daemon [pid 14028]
```

| 持有者 | 实体 | 性质 |
|---|---|---|
| `HipsDaemon.exe` / `usysdiag.exe`（pid 14028） | **火绒企业版** `C:\Program Files (x86)\Huorong\ESEndpoint\bin\` | 服务端策略托管 |
| `TBProtect.exe`（pid 30684） | **云枢 YunShu** `C:\Program Files\YunShu Plugin\tbprotect\`，同目录 `tbprotect64.sys`（内核过滤驱动）、`savapi.dll`（Sophos AV 引擎）、`hids.dll` | 企业 EDR + AV |
| `System [pid 4]` | 上述驱动的**内核态**持有 | RM 无法归因用户进程 |

驱动元数据：`tbprotect64.sys` / CompanyName `Hangzhou Eagle Cloud Security technology, Inc` / ProductName `YunShu`。

### 1.2 为什么白名单无效

1. **云枢从未被白名单**（用户只加了火绒）。
2. **火绒是企业版**（`ESEndpoint`），排除项通常由服务端策略下发，客户端本地添加会被覆盖。
3. 两者都是**内核 minifilter**，扫描发生在 close 时，与用户态进程无关。

### 1.3 三个动词全部沦陷

| 动词 | 实例 | 谁需要 |
|---|---|---|
| `rename` | `failed to rename archive file` | rustc 写 `.rlib` 收尾 |
| `write` | `failed to write …libpotential_utf-….rmeta` | rustc 写 `.rmeta` |
| `remove` | `failed to remove …*.rcgu.o` | cargo 增量清理 |

共同点：都需 **DELETE 共享语义**。cargo 的 build-dir 锁只互斥自家 cargo，对 minifilter 无效。

### 1.4 环境侧才是根治（代码改不掉）

```
云枢（TBProtect）          排除 C:\Users\wps\.dev-team
火绒企业版（服务端策略）     排除 C:\Users\wps\.dev-team
建议一并排除                C:\Users\wps\.cargo、C:\Users\wps\AppData\Local\pnpm
⚠ 逐 worktree 加无效 —— worktree 是动态新建的，必须用父目录
```

产品侧的价值：**诊断（一次定位）+ 重试（自愈）+ 语义（不误导）**。

---

## 2. 设计

### 2.1 精确锁探针（不误报、不扰动）

旧探针用「独占打开」探测，有两个缺陷：把正常读句柄误报成锁；且它自己独占打开
rustc 正要 finalize 的文件，**可能制造 `os error 32`**。

正确判据 —— 只问「是否阻塞 rename/delete」：

```
CreateFileW(path,
            access = DELETE | FILE_READ_ATTRIBUTES,
            share  = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING)
  成功     → 可 rename/delete（未阻塞）
  失败(32) → 有持有者不允许删除共享 → 与 rustc 失败同条件
```

自证（Windows PowerShell P/Invoke 实测）：

```
baseline                : ok
reader(share=rw+d)      : ok            ← 旧探针会误报
reader(share=read only) : BLOCKED err=32  ← 真阻塞，与 rustc 同码
```

### 2.2 占用者点名（Windows Restart Manager）

```
RmStartSession → RmRegisterResources([被锁文件]) → RmGetList
  → RM_PROCESS_INFO[].strAppName / dwProcessId
```

依赖 `windows-sys` 的 `Win32_System_RestartManager` feature（workspace 锁内已有传递依赖）。
实测可自证（独占打开一个 rlib，准确报出 `Windows PowerShell [pid 9692]`，释放后 `(no lockers)`）。

- **触发时机**：仅在「非零退出 + 命中瞬态签名」时调用，避免常态开销。
- **输入**：从诊断文本解析被锁文件路径（`failed to rename archive file: <path>` /
  `failed to write <path>` / `failed to remove <path>`）。
- **输出**：去重后的 `进程名 (pid)` 清单。
- 非 Windows 降级为 `lsof`（存在则用，不存在跳过），不影响主流程。

### 2.3 瞬态签名（`infra/exec/src/transient.rs`）

判定条件：`outcome == Completed && exit_code != 0` **且** 合并输出命中签名行。
**`TimedOut` / `Cancelled` 永不重试**（超时重试会把 1200 s 变 3600 s；取消必须立即生效）。

| 族 | 签名（行内包含，大小写不敏感） |
|---|---|
| Windows 文件锁 | `os error 32`、`os error 5`、`另一个程序正在使用此文件`、`being used by another process`、`failed to rename archive file`、`failed to remove file`、`Access is denied` |
| MSVC 链接器 | `LNK1104` |
| POSIX / Node | `EBUSY`、`ETXTBSY`、`Text file busy`、`EPERM: operation not permitted`、`resource busy or locked` |

- 刻意**不收**裸 `failed to write`（太泛）；rustc 的 rmeta 写失败已由 `os error 32` /
  `另一个程序正在使用此文件` 覆盖（同一行同时出现）。

### 2.4 重试策略

```rust
pub struct RetryPolicy {
    max_attempts: u32,          // 5（= 首跑 + 4 次重试）
    backoff: [Duration; 4],     // 1s / 2s / 4s / 8s
    total_budget: Duration,     // 1500s —— 整段重试预算硬顶
}
```

**为什么重试有效**：每次重试都利用 cargo 的**增量进度**——已建成的 crate 不会重建，
剩余待写文件数递减 → 碰撞概率递减。这是代码侧唯一能自愈的手段。

**总预算约束**：单次 timeout 1200 s，若按「最多 5 次」直接相乘会到 6000 s。因此
`total_budget = 1500 s`：从首跑起算，超预算即停止重试并直接走 fault。

```
快挂（每次 8–20 s）  1s/2s/4s/8s 退避，5 次约 100 s → 耗尽 fault
慢挂（每次 300 s）   300 → 600 → 900 → 1200 → 预算耗尽 → 停
```

- 重试期间同样响应 cancel。
- 每次尝试输出以分隔头拼接进最终 record：
  `===== attempt N/5 · exit=C · transient=<命中签名> =====`
- 全程保留所有尝试输出（审计不丢上下文），`output_ref` 落最后一次。

### 2.5 `RunFault`：把「环境坏」与「检查红」分开

```rust
// core/orchestration/src/port.rs
/// 执行面归因（检查结论之外的原因；None = 结论本身有效）
pub enum RunFault {
    /// 用户停止（walker 收敛 stopped）
    Stopped,
    /// 环境错误：瞬态锁耗尽 / 超时 / 进程拉不起——**不注回 executor 烧预算**
    Environment(String),
}

pub struct StaticCheckOutcome   { pub passed: bool,   pub fault: Option<RunFault>, .. }
pub struct TestExecutionOutcome { pub conclusion: .., pub fault: Option<RunFault>, .. }
pub struct MutationTestOutcome  { pub conclusion: .., pub fault: Option<RunFault>, .. }
```

`passed` / `conclusion` **保持原样**（不升三值），新增字段承载原因。

### 2.6 fault 的三条来源

```
static_check   ← run_shell 的 RunOutcome
                 TimedOut                        → Environment("超时 …")
                 Cancelled                       → Stopped
                 Completed + 重试耗尽 + 命中签名   → Environment(最后一次诊断摘要)
                 Completed + 其余非零             → None（真红，走反馈边 / 基线红）
                 无配置直过                        → None

test_execution ← 同表；
                 conclusion=Error 收窄为「解析失败 / harness 崩」等真环境面
                 —— 超时与瞬态耗尽 MUST 走 fault 而不是 conclusion

mutation_test  ← SegmentFailure::Cancelled（唯一来源）
                 Cancelled → Stopped
                 其余（Interrupted / 超时 / Fatal）保持既有语义：
                   Interrupted → conclusion=Error + 即时反馈面
                   Fatal       → Err（停给用户）
                 —— mutation 超时有意不进 fault（长跑是产品语义，超时是配置问题）
```

### 2.7 walker 分派：fault 先于结论

```
step_static_check            fault? → stopped / failed(env)  : 既有 passed 分支
step_static_check_terminal   fault? → stopped / failed(env)  : 既有基线红文案
step_test_execution          fault? → stopped / failed(env)  : 既有 pass / 反馈边
step_test_execution_terminal fault? → stopped / failed(env)
step_mutation_test           fault? → stopped                : 既有 pass / 反馈边②
```

`Environment` **一律 `Terminal::failed`**，不升格相位 fail、不进决策重试
（环境坏的相位重试必然再失败），也**不注回 executor**（省下 5 次反馈预算）。

### 2.8 终局文案（诊断直接落在错误信息里）

```
静态检查无法完成（环境错误）：构建产物被安全软件占用
  占用进程：TBProtect.exe (云枢)、HipsDaemon.exe (火绒企业版)、System(内核驱动)
  被锁文件：...\target\debug\deps\libchrono_tz-...rlib
  已自动重试 4 次（每次重试会跳过已完成部分）
  处理：请 IT 在【火绒企业版（服务端策略）】与【云枢】中排除 C:\Users\wps\.dev-team
        （逐 worktree 排除无效——worktree 动态新建）
```

---

## 3. 改动清单

| 文件 | 改动 |
|---|---|
| `crates/infra/exec/src/transient.rs`（新） | 签名表 + `RetryPolicy` + `is_transient(&RunReport)` |
| `crates/infra/exec/src/run.rs` | 接 `RetryPolicy`（缺省 none）；`RunReport.attempts` / `retried: Vec<AttemptTrace>` |
| `crates/infra/exec/src/lockprobe.rs`（新，cfg windows） | 精确探针（`CreateFileW`）+ Restart Manager 点名；非 Windows 降级 `lsof` |
| `crates/infra/checks/src/static_check.rs` | 开 `RetryPolicy::transient()`；fault 装配；探针接入诊断 |
| `crates/infra/checks/src/testexec/runner.rs` | 同上；`TimedOut`/`Cancelled` 上传至 outcome |
| `crates/infra/checks/src/mutation/mod.rs` | 只做 `Cancelled → fault=Stopped`（不接 retry） |
| `crates/core/orchestration/src/port.rs` | `RunFault`；三 outcome 加 `fault` 字段 |
| `crates/core/orchestration/src/walker.rs` | fault 分派五处；新文案常量（`TXT_ENV_STATIC` / `TXT_ENV_TEST`） |
| `openspec/specs/desktop-checks-domain/spec.md` | 新增「瞬态重试与环境错误归因」要求 |
| `openspec/specs/desktop-change-orchestration/spec.md` | 三门禁步失败分派新增 `RunFault` 分支 |
| `openspec/specs/desktop-mutation-testing/spec.md` | 取消通路 + 「段超时不进 fault」分叉口径 |
| `packages/desktop/src/types/generated/*` | **无**（`RunFault` / `MutationTestOutcome` 均不出线） |

---

## 4. 测试策略

| 层 | 用例 |
|---|---|
| `transient` | 签名命中/不命中表驱动；`TimedOut` / `Cancelled` 恒不命中；裸 `failed to write` 不命中 |
| `run_shell` + retry | flaky shim（第 1、2 次锁文本退出 1，第 3 次成功）→ `attempts=3`、`Completed`；持续锁 → `attempts=5` + 预算判定；真红（无签名）→ `attempts=1` |
| `lockprobe` | 自锁自证（本进程独占打开 → 报自己；释放 → no lockers）；路径解析（三种错误串） |
| static_check | 重试耗尽 → `fault=Environment` 且 `passed=false`；正常红 → `fault=None`；无配置直过 → `fault=None` |
| walker | `fault=Environment` → run failed 且**零 worker 调用**（不烧预算）；`fault=Stopped` → run stopped；文案含「已自动重试 N 次」与占用进程名 |
| mutation | cancel → `fault=Stopped`；段超时仍 → `Interrupted`（不进 fault） |

---

## 5. 验收标准

1. 命中瞬态签名时自动重试，耗尽后把**占用进程名**写进诊断与终局文案。
2. 环境错误不再注回 executor（静态检查反馈边计数不增长）。
3. 环境错误与基线检查红**文案可辨**：前者引导「排除安全软件路径」，后者引导「修基线」。
4. 取消运行中的检查步 → run 在秒级收敛 `stopped`（非 failed）。
5. `TimedOut` / `Cancelled` 零重试；总重试预算不超过 1500 s。
6. 真红（无签名）零重试，行为与现状一致。

---

## 6. 风险

| 风险 | 缓释 |
|---|---|
| 签名误判 → 真实编译错误被吞成「环境错误」 | 签名表刻意收窄（不含裸 `failed to write`）；真红用例锚定 `attempts=1` |
| 重试放大耗时 | 总预算 1500 s 硬顶；耗尽即停 |
| `RmGetList` 在某些环境返回空 | 空清单降级为「未定位占用者」，不阻断主流程 |
| `windows-sys` feature 膨胀 | 只开 `Win32_System_RestartManager` |
| 探针自身扰动构建 | 只读探测（share 全开 + DELETE 权限），不独占 |
| 与篇 2 的耦合 | 重试/取消都挂在 `run_shell`；篇 2 未合入时本篇需自建骨架（开头已声明） |

---

## 7. 遗留观察项（不在本篇范围）

1. 云枢是否提供本地排除 UI（`TBProtect.exe` 无版本信息，可能云控下发）。
2. 火绒企业版排除后是否即时生效（minifilter 通常需重启驱动）。
3. 若两者均无法排除：接受「重试 5 次 + 明确文案」作为长期形态（每次 run 多 1–3 分钟）。
4. 覆盖率聚合静默忽略失败 suite（`aggregate_coverage` 的 `weighted_measured?`）——
   失败 suite 的 `coverage=null` 不参与加权，导致「rust 一个数没产却报 pass」。
   建议单列小项修复。
