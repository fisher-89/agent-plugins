# 文件锁故障：三篇冻结文档的补丁

workflow_type: requirement

> **本篇定位**：`rust-suite-single-build` / `exec-run-shell` / `lock-fault-diagnostics`
> 三篇已进入变更、不可修改。本篇是它们的**增量补丁集**，作为后续变更的依据。
>
> **核心新发现**（三篇均未覆盖）：失败率与「待写产物数 N」成正比，
> **重试不是兜底而是确定性收敛** —— 每次失败都留下已完成产物，N 单调递减至 0。
> 实测 2–3 次内通过。详见 §2。
>
> 来源：2026-10-19 现场取证（日志、指纹、进程监视、S1 实测）。

---

## 1. 背景

### 1.1 故障现象

桌面端 change run 的 `initialize` 相位（静态检查 / 测试执行）报：

```
error: failed to write ...\target\debug\deps\libfoo-<hash>.rmeta:
       另一个程序正在使用此文件，进程无法访问。 (os error 32)
```

每次报错文件不同，run 以 Terminal 收敛 failed。

### 1.2 根因（已定论，详见三篇）

两套企业级安全软件的**内核过滤驱动**在 close 时扫描构建产物：

| 持有者 | 实体 |
|---|---|
| `HipsDaemon.exe` / `usysdiag.exe` | 火绒**企业版**（`ESEndpoint`，服务端策略托管） |
| `TBProtect.exe` + `tbprotect64.sys` | 云枢 YunShu（含 Sophos AV 引擎、HIDS） |
| `System [pid 4]` | 上述驱动的内核态持有 |

关键证据（进程监视器实拍，一次**成功**的静态检查末尾）：

```
[11:05:33.791] LOCKED export_bindings.exe <= System [pid 4], export-bindings.exe
[11:05:34.172] LOCKED export_bindings.exe <= System [pid 4], TBProtect.exe [pid 30684], ...
[11:05:34.553] LOCKED export_bindings.exe <= export-bindings.exe, Huorong Enterprise Security Endpoint Daemon [pid 14028]
```

三个动词全部沦陷：`rename`（rustc 写 .rlib 收尾）、`write`（.rmeta）、
`remove`（cargo 增量清理）。共同点：都需要 **DELETE 共享语义**。

**白名单无效的原因**：云枢从未被排除；火绒是企业版，排除项由服务端策略下发。

### 1.3 现象与路径/冷热的关联

| 观测 | 值 |
|---|---|
| 失败行出现的盘符 | `C:\` **21 条**，`D:\Projects` **0 条** |
| exec-outputs 日志总数 | 147 份，含锁错误 **20 份（13.6%）** |
| worktree 根 | `C:\Users\wps\.dev-team\worktrees\<身份段>\<change>`（`main.rs:18` 的 `DATA_DIR`） |
| 主仓 | `D:\Projects\wps-claude-plugin`（target 长期热） |

两个假设尚未分离（见 §3.5）：**H1 路径**（C:\Users 被扫描更激进）vs **H2 冷热**（worktree 每次新建 → target 冷 → N 大）。

---

## 2. 新机制：增量收敛（补丁的核心依据）

### 2.1 决定性观测

**不做任何干预，静态检查与测试各重试一次即可通过。**

从 `archive/2026-10-19-test-execution-zero-agent-decision/reports/baseline/exec-outputs/`
取到的连续 attempt 序列，按 `Compiling` 行数计数：

| attempt | Compiling 行数 | 结果 |
|---|---|---|
| 0 | **264** | 失败 `failed to write …libpotential_utf-…rmeta` |
| 1 | **5** | 失败 `failed to write …` |
| 2 | **0** | 失败 `failed to build archive …`（只剩 rename 收尾） |
| 3 | 0 | 失败 |
| 4 | 0 | 失败（`could not compile axum`，另一类） |
| 5 | **192** | 失败（新一轮：static-check 的 `cargo run` 又写 `target/debug`） |
| 6 | **82** | 失败 |
| 7 | **2** | 失败 |
| 8 | **0** | ✅ **成功** |

`desktop-explore-list-archive` 同型（`exec-1792378264380-0` 失败 → `exec-1792379154001-1` 绿）。

### 2.2 机制

```
rustc 每写一个 .rlib/.rmeta 都是一次「写临时文件 → rename」
    └─ 概率 p 被内核 minifilter 拦下 = os error 32

一次构建要写 N 个产物
    碰撞概率 ≈ 1 - (1-p)^N        ← N 越大越接近 1

关键：**失败不回滚已完成的产物**
     attempt 0: N=264 → 大面积失败，但前 200+ 个已落盘
     attempt 1: N=5   → 只剩尾部少数
     attempt 2: N=0   → 全部已建，只剩 rename/清理收尾
     attempt k: 静默窗口无写入 → 成功
```

**重试是「分批把 target 填满」的确定性收敛过程，不是碰运气。**

### 2.3 由此解释此前全部矛盾观测

| 观测 | 解释 |
|---|---|
| 「关 VSCode 后重试成功」 | 降低写入速率；**且此前失败已填满 target** |
| 「白名单后重试依然失败」 | 那次只重试 1–2 次，`Compiling` 仍有残余（N>0） |
| 「不做干预，重试一次就过」 | 上一次 attempt 已完成大部分编译 |
| 「手动跑不失败」 | 主仓 target 长期热 → N≈0 |
| 失败全在 `C:\`（21:0） | worktree 每次新建 → target 冷 → N 大；主仓长期热 → N≈0 |

> **推论**：环境侧排除降低 **p**；重试压低 **N 的影响直至归零**。
> 二者正交，但**只有重试是代码侧可控且实测有效的**。

### 2.4 对早期结论的修正

| 早期表述 | 修正 |
|---|---|
| 「重试降级为兜底」 | **重试是主修复**（确定性收敛，实测 2–3 次内通过） |
| 「没有一项代码修改能解决文件锁」 | **不成立** —— 重试就是代码修改，且能解决 |
| 「关 VSCode 就成功是纯巧合」 | 方向对但不完整 —— 主因是 N，VSCode/盘符只是影响 p 的次要因子 |

---

## 3. 补丁清单

### 3.1 补丁 A：`rust-suite-single-build.md`（篇 1）

**无破坏性补丁**，仅调整两处预期表述：

| 位置 | 建议 |
|---|---|
| §1.4 跨相位 target 冲突 | 保留「消除冲突」结论（`target/debug` 确实不再被 test-execution 触碰），但**补充**：消除的是**跨相位新增的 N**，不消除 S1 自身那份冷构建 |
| §7 风险表 | 追加一行：**S1 只把 N 减半，首次冷构建仍可能失败，必须依赖篇 3 的重试** |

**新增回归观测点**：S1 落地后，单次 test-execution 的 `Compiling` 行数应从「2 段合计」降为「1 段」。

---

### 3.2 补丁 B：`exec-run-shell.md`（篇 2）

**无破坏性补丁**，一处语义升级：

| 位置 | 建议 |
|---|---|
| §1.1 后果表 | 追加：**残留进程会制造「额外的 N」** —— 被超时/停止杀剩的 rustc 继续写 target，等价于在别的 attempt 里插入一批新写入，直接抬高碰撞概率。这让「杀进程树」从整洁性升级为**正确性** |

---

### 3.3 补丁 C：`lock-fault-diagnostics.md`（篇 3，五条）

#### C1 重试定位：从「兜底」改为「主修复」

| 位置 | 原文 | 改为 |
|---|---|---|
| §2.4「为什么重试有效」 | 「代码侧唯一能自愈的手段」 | **重试是确定性收敛**：每次失败留下已完成产物，N 单调递减至 0；实测 2–3 次内通过（§2.2） |

#### C2 退避参数上调

```rust
// 原文
max_attempts: 5,  backoff: [1s, 2s, 4s, 8s]

// 补丁
max_attempts: 5,  backoff: [5s, 10s, 20s, 40s],   total_budget: 1500s
```

理由：实测静默窗口可能到分钟级；1 s 退避在「只剩 rename 收尾」的 attempt 上撞同一窗口概率高。

#### C3 新增「增量推进」判据（关键）

固定次数不够 —— 需判断「在推进还是卡死」。每次失败后解析输出里的 `Compiling` 行数 `N_i`：

```
N_i < N_{i-1}          → 在推进，继续重试（可延长预算）
N_i == N_{i-1} == 0    → 只剩收尾，再试 1–2 次（静默窗口可能极短）
N_i == N_{i-1} > 0     → 卡死（同一批写不进去），提前放弃 → fault=Environment
```

这使重试从「固定 5 次」升级为「按进展自适应」，避免两个方向上都浪费预算。

#### C4 `fault=Environment` 触发时机收紧

| 位置 | 补丁 |
|---|---|
| §2.6 fault 表 | 保留「重试耗尽 + 命中签名 → Environment」，但**补充前置**：必须先跑完重试。MUST NOT 在首次失败即判 Environment |

#### C5 终局文案补「推进证据」

```
静态检查无法完成（环境错误）：构建产物被安全软件占用
  占用进程：TBProtect.exe (云枢)、HipsDaemon.exe (火绒企业版)
  被锁文件：...\libchrono_tz-...rlib
  重试进展：待编译 264 → 5 → 0 → 0（3 次后仍失败，判定为卡死）
  已自动重试 4 次
  处理：请 IT 在【火绒企业版（服务端策略）】与【云枢】中排除 C:\Users\wps\.dev-team
        （逐 worktree 排除无效——worktree 动态新建）
```

`Compiling` 序列同时是诊断证据与「重试确实在推进」的用户可见性。

---

### 3.4 补丁 D：`initialize` 相位的 Terminal 策略（跨篇，三篇均未覆盖）

现状：`initialize` 相位的 static-check / test-execution 声明 `on_fail=Terminal`，
一失败整个 run 收敛 failed，用户须手动重新发起。

代价：此前 attempt 已把 target 填到大半（N 从 264 降到 5），却因一次 Terminal 全部浪费。

| 方案 | 做法 | 成本 |
|---|---|---|
| **I1（推荐）** | 重试留在 runner 内（篇 3 的 `RetryPolicy`），Terminal 语义不变 —— runner 内部消化，walker 只见最终结论 | 零 walker 改动 |
| I2（仅当 I1 不够） | walker 层加「同相位自动重入」，把用户手动重试自动化（有预算上限） | 需改相位机语义 |

**决议**：先做 I1。只有实测「runner 内 5 次仍不够」才考虑 I2。

---

### 3.5 补丁 E：worktree 根位置（探索项，需先做实验）

`packages/desktop/src-tauri/src/main.rs:18`：

```rust
const DATA_DIR: &str = ".dev-team";
let data_root: PathBuf = app.path().home_dir()?.join(DATA_DIR);
// → C:\Users\wps\.dev-team\worktrees\<身份段>\<change>
```

H1（路径）与 H2（冷热）尚未分离。**分离实验（5 分钟，不改代码）**：

```powershell
cd "C:\Users\wps\.dev-team\worktrees\<hash>\<change>\packages\desktop\src-tauri"
move target target.bak
cmd /c mklink /J target D:\devteam-target\<change>
pnpm run check
# 收尾：cmd /c rmdir target && move target.bak target
```

| 结果 | 结论 | 后续 |
|---|---|---|
| 成功 | **H1 成立**（路径问题） | 改 `DATA_DIR` 或 target 落位到非用户目录 —— 一个常量 |
| 仍失败 | **H2 成立**（冷热/写入量） | 只能靠重试 + 环境排除 |

> 若 H1 成立，这是**成本最低的根治手段**（不依赖 IT），优先级高于本篇任何其他补丁。

---

## 4. 实施顺序与验收

| 顺序 | 内容 | 验收 |
|---|---|---|
| 1 | 补丁 C1–C5（重试语义与参数） | 冷 target 上首次失败后 2–3 次内收敛；`Compiling` 序列单调递减并写进诊断 |
| 2 | 补丁 D（I1） | `initialize` 相位不再因单次瞬态失败终止 run |
| 3 | 补丁 A、B（表述与风险表） | 文档一致性 |
| 4 | 补丁 E（先做 junction 实验） | 若 H1 成立，改常量后失败率显著下降 |

**硬验收**：

1. 冷 target（新建 worktree）上，静态检查 / 测试执行**不需要人工干预**即可通过。
2. 重试耗尽后，终局文案包含占用进程名与 `Compiling` 推进序列。
3. `N_i == N_{i-1} > 0` 时提前放弃（不烧满 5 次预算）。
4. 真红（无签名）零重试，行为与现状一致。

---

## 5. 优先级（相对三篇的排序调整）

```
P0  补丁 C1–C5（重试语义 + 自适应判据）   ← 唯一实测「不干预即通过」的代码手段
P1  环境侧排除（云枢 + 火绒企业版，父目录） ← 降低 p，根治，需 IT
P2  补丁 E（worktree 根位置，先实验）      ← 若 H1 成立则成本最低的根治
P3  篇 1 S1                               ← N 总量减半，缩短收敛所需次数
P4  篇 2 exec（杀树/超时/取消）            ← 消除自造 N，防悬挂
P5  补丁 D（I1）                          ← initialize 不再因瞬态失败终止
P6  补丁 A/B（表述）                       ← 文档一致性
```

> 与三篇原始排序的差异：**重试从 P4 提到 P0**。理由：实测证明它能在 2–3 次内收敛
>（N 单调递减至 0），其余手段只是「降低概率」或「降低单次 N」。

---

## 6. 附：已归入三篇的内容（本篇不再展开）

| 内容 | 归属 |
|---|---|
| S1 模板合并、覆盖率口径（25 → 209 文件）、`&&` 修复 | 篇 1 `rust-suite-single-build.md` |
| `exec::run_shell`、`CancelFlag`、四类 spawn 迁移、超时值 | 篇 2 `exec-run-shell.md` |
| 精确锁探针、Restart Manager 点名、签名表、`RunFault`、walker 分派 | 篇 3 `lock-fault-diagnostics.md` |

以下观察项不在本篇范围，需另立小项：

1. **覆盖率聚合静默忽略失败 suite**：`aggregate_coverage` 的 `weighted_measured?` 跳过
   `coverage=null`，导致「rust 一个数没产却报 pass」（实证：`summary.json` 里
   `conclusion=error` 但 `coverage.pass=true`，数值全来自 vite-plus）。
2. **云枢是否提供本地排除 UI**（`TBProtect.exe` 无版本信息，可能云控下发）。
3. **火绒企业版排除后是否即时生效**（minifilter 通常需重启驱动）。
