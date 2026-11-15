# 提案: worktree-remove-orphan-defect

> **变更**: worktree-remove-orphan-defect
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/worktree-remove-orphan-defect/explore.md`（缺陷陈述 + 2026-11-13 实测实验：git 先拆登记再删文件、两种失败形态、强杀 94ms 释放）
> **范围接缝**: 本变更只做 **`infra/vcs` worktree 回收单点的自愈化 + 三处调用点文案的双源校正**：① `crates/infra/vcs/src/git.rs::remove_worktree` 由「一次 `git worktree remove --force`」改为两段式（git 失败 → OS 层 `remove_dir_all` → `git worktree prune`），失败面收窄到「两段式皆失败」并携成因；② `ProcessArchiveVcs::remove_worktree` 自组的「请手动执行 `git worktree remove --force <path>`」文案退役（文案单点收口，防两处漂移）；③ create 前置⑨保底门槛、create 补偿残留、delete / Cleanup 三处面向用户的残留指引改为「`git worktree list` + 直接检查 `.worktrees/<仓库目录名>/`」双源。MUST NOT 做：占用者进程点名（见 D5）、重试 / 退避 / 优雅握手（D6）、`remove_worktree` 之外的 vcs 方法、`WorktreePort` / `ArchiveVcsPort` 签名与 `Result` 形状变更、wire DTO 与前端类型改动、归档 Cleanup 尽力语义改写（D7）、只读属性与保留设备名的额外自愈梯子（D9）、CodeGraph 关停时序（独立变更 `builtin-codegraph-mcp-bridge` 8.10）。
> **前序**: `remove_worktree` 现为「一次 git 调用、失败即 `Err`」的补偿面单点（`git.rs:223` 起），被 create 补偿（`create.rs:424`）、删除面（`delete.rs:65`）、归档 Cleanup（`walker.rs:2798`）三处消费；opening 语义基线见 desktop-change-worktree「V1 范围与边界留痕」第 6 条（孤儿对账单源 `git worktree list`）——该条正是本变更要修正的错误指引。

---

## 问题

### 现象一：一次失败即终态（无自愈段）

`git worktree remove --force` 的执行序是**先拆登记、再删文件**（实测，`--force` 与非 `--force` 一致）：

```
① 删 <main>/.git/worktrees/<name>/      （admin 目录）
② 从 worktree 登记表移除该条目
③ 递归删 <path> 下的文件    ← 任一文件被独占打开即在此 abort
④ 删 <path> 目录本身        ← 目录被占用（cwd / 句柄）即在此 abort
```

①② 在 ③④ 之前，因此**任何一次删除失败都留下一个「git 已注销」的孤儿目录**：`git worktree list` 不显示、`git worktree prune` 无输出、`git worktree remove` 报 `not a working tree`（exit 128）、目录内 `.git` 指针指向已删除的 admin ⇒ 从里面跑任何 git 命令都 `fatal: not a git repository`。**不存在「失败即回滚、可安全重试」的路径**——当前实现的 `Err` 就是该对象的永久终态。

两种失败形态后果不同但都是孤儿：cwd 锁（目录占用）→ 内容已被删光、只剩空壳目录；文件句柄锁（SQLite 语义，`FileShare.Read` 不含 `Delete`）→ 删除中途 abort、内容完整留存（几十 MB 数据 + `node_modules` / `target` 全留在盘上）。

### 现象二：三处调用点、三种错误处理、同一个失效结局

| 调用点 | 失败处理 | 用户看到什么 |
|---|---|---|
| 归档 Cleanup 步（`walker.rs:2798`） | 尽力步：吞为 detail 警告，步恒 `passed`、归档照常收口 | 归档「成功」，日志一行 warn，**孤儿静默留盘** |
| 删除 change（`delete.rs:65`） | 硬 `Err`，整条删除链中止 | `worktree 移除失败（<path>）: <git 原文>`，**失败可见、归因不可见**——且重试恒失败（登记已拆，目录还在） |
| create 树写出补偿（`create.rs:424`） | 收进 `residuals`，随最终 `Err` 呈现 | 「补偿回收未完成，残留: worktree <path>: …」，同样不可自愈 |

### 现象三：文案把用户指向一个查不到问题的地方

`ProcessArchiveVcs::remove_worktree`（`infra/vcs/src/lib.rs`）与 create 两处补偿文案都在指引：

> 请以 `git worktree list` 对账并手动清理（`git worktree remove --force <path>` / `git branch -D <branch>`）

而**孤儿正是 `git worktree list` 不显示的那一类**：用户按指引操作会得出「没有残留」（`list` 空）或「删不掉」（`remove` 报 not a working tree），无从自救。create 前置⑨ 的文案同理——它的计数源是**磁盘**（`fs::read_dir` 数 `.worktrees/<repo>/` 下的子目录），对账指引却是 **git**；实测沙箱收尾时磁盘上有 2 个孤儿、`git worktree list` 只显示主仓。**计数源与对账源不是同一个真相源**，这是文案层最直接的 bug。

### 现象四：触发源不是「接了 CodeGraph 才会遇到」

杀毒实时扫描、Windows Search / Everything 索引器、编辑器打开的 worktree 内文件、用户 `cd` 在 worktree 内的终端、OneDrive / Dropbox、长驻子进程（dev server、watcher）——任一都足够。CodeGraph server 只是把「偶发」推到「必然」（它每次都持 cwd + db 且不主动退出）。这条缺陷与 CodeGraph 无关，可独立交付。

### 症结

缺陷本体不在三处调用点（三处的错误处理策略各自合理：归档不该因清理失败回滚、删除该硬失败、补偿该报残留），而在**回收单点缺一层「绕过 git 登记语义」的兜底**，以及**面向用户的残留指引只有一个（错的）真相源**。改单点 + 正文案即可三处同时受益。

---

## 提案

一句话形态：**`remove_worktree` 变两段式——git 先试，失败即以 OS 层回收目录（并 `prune` 清残留 admin），两段皆失败才 `Err` 并携成因；所有残留指引改双源对账。**

```
git worktree remove --force <path>
   │ 成功 ──► Ok（既有语义零变化）
   └ 失败 ──► 目录不在场 ──► git worktree prune（尽力）──► Ok（幂等）
              目录在场   ──► fs::remove_dir_all（Windows 经 \\?\）──► git worktree prune ──► Ok
                             （原 git 错误以 `[vcs]` 前缀 stderr 日志留痕）
                             兜底亦失败 ──► Err（路径 + 原 git 错误 + OS 错误 + 成因清单）
```

三条已验事实支撑该形态（explore 第三节）：

1. 对**已登记但目录已消失**的 worktree：`prune` 输出 `Removing worktrees/<name>: gitdir file points to non-existent location` 并清掉 admin；
2. 对**已注销孤儿**：`prune` 是 no-op，但此时 `remove_dir_all` 直接可删（无锁后）——两段式两条路径都通；
3. 强杀后 94ms 内 `remove --force` 即成功 ⇒ **没有需要等待的窗口**，本变更因此不引入任何重试 / 退避 / 优雅握手。

### 裁决表

| # | 议题 | 裁决 | 被拒备选与理由 |
|---|---|---|---|
| D1 | 修复落点 | **`crates/infra/vcs/src/git.rs::remove_worktree` 单点**（`WorktreePort` 与 `ArchiveVcsPort` 两缝同实现，三处调用点自动受益、零调用点改动） | **被拒：三处调用点各自兜底**——同一份 `remove_dir_all` + `prune` 写三遍，且 create 补偿 / delete / Cleanup 分别住在 workflow 与 orchestration（core 零进程 spawn 红线内拿不到 fs 兜底之外的一致性），文案必漂。**被拒：新增 port 方法 `purge_worktree`**——两 port 签名扩张换不来语义增量（兜底是回收的组成部分，不是新能力） |
| D2 | Err 形状 | **保持 `Result<(), String>`**，成因拼进文案 | **被拒：结构化 `RemoveWorktreeError{ git, fs, path, holders }`**——两 port 类型 + 十余处测试假件随之改签名，而消费侧（walker detail / 写面 Err / 补偿残留）只做字符串拼接，收益为零 |
| D3 | 目录不在场语义 | **`prune` 后 `Ok`（幂等）** | **被拒：保留 `not a working tree` 的 `Err`**——创建补偿面会对「add 失败且目录未生成」产出假残留告警；delete / Cleanup 的 retry 链永不自愈，与本变更反向 |
| D4 | 兜底形态白名单 | **不设形态判据**，只设主仓黑名单（`main_root` 自身 / 其祖先 / 其子目录 / 文件系统根 → 拒绝 + `Err`） | **被拒：要求目录内 `.git` 指针在场（或目录为空）才兜底**——删除中途 abort 时 `.git` 往往已被先删（③ 阶段逐文件删），该判据会在**最需要自愈的窗口**拒绝自愈，把形态 B′ 的孤儿挡住。接受度依据：路径来自机器写入的 `ChangeRecord.worktree`，黑名单已挡住「误删主仓 / 误删祖先」这类真实灾难面 |
| D5 | 占用者归因 | **不复用 `exec::lockprobe` / `attribution_text`**；`Err` 携原始 OS 错误码 + 固定成因清单（终端 cwd / 编辑器 / 常驻子进程 / 杀毒实时扫描 / 云同步） | **被拒：复用 `attribution_text`**——`RmRegisterResources` 只认**文件**、不认目录，而失败面是「目录树里某个未知文件被锁」，点名结果恒为「未定位占用者（可能为内核过滤驱动…）」；要真点名需逐文件探测（`node_modules` 级规模不可行）。白引 `desktop-exec-runtime`「锁占用精确探针」的消费面扩面（其契约明写「供检查域装配环境错误文案」且「仅在非零退出 + 命中瞬态签名时调用」），收益为负 |
| D6 | 重试 / 退避 | **零重试零退避**（兜底失败即 `Err`） | **被拒：有限重试**——实测强杀即释放，没有等待窗口；退避只会把「持久占用」伪装成「偶发失败」，与 explore 3.1 第 4 条结论冲突 |
| D7 | Cleanup 尽力语义 | **保留**：清理失败仍不 Terminal、步恒 `passed`、归档照常收口 | **被拒：清理失败升 Terminal**——归档已收口的事实不该因磁盘残留回滚；且失败面收窄后（仅两段式皆失败）该分支已罕见 |
| D8 | 文案归属 | **单点驻 `git::remove_worktree` 的 `Err`**；`ProcessArchiveVcs::remove_worktree` 的自组 wrapper 退役为纯透传 | **被拒：两处各写**——正是当前两份不同文案的来源（`git.rs` 裸 git 错误 vs `lib.rs` 加手动命令指引），必然漂移 |
| D9 | 只读属性 / 保留设备名 / 超长路径 | **只覆盖超长路径**：Windows 兜底经 `\\?\` 扩展前缀；只读与保留名**不额外自愈**（失败即 `Err` 诚实上报） | **被拒：清只读属性 + 逐项删除的完整梯子**——git 侧删除本就处理只读（Windows 下先改属性再 unlink），仅剩「OS 层唯一障碍是只读」的窄面未实测（explore 待核实）；为此自实现递归删除遍历（且要处理保留名 / ADS / 尾随空格）风险高于收益，留作后续变更 |

### 双源对账（文案统一形态）

```
旧：请以 git worktree list 对账并手动清理（git worktree remove --force <path> / git branch -D <branch>）
新：请① 以 git worktree list 对账；② 直接检查 worktree 落位根 <git 根父目录>/.worktrees/<仓库目录名>/
    下的子目录——删除失败留下的残留不在 git 登记面（git 已注销），list 看不到、prune 无事可做、
    remove 报 not a working tree，直接删除该子目录即可。
```

四处消费同一个形态：`git::remove_worktree` 的 `Err`（delete / Cleanup 共用）、create 前置⑨、create 补偿残留（`compensate_add_failure` / `compensate_tree_failure`）。branch 侧仍 `git branch -D <branch>`（分支不受本缺陷影响）。

### 版本与交付纪律

`packages/desktop/package.json` 的 `version` 按「落地时 `HEAD` 现值 + 1 patch」相对递增（现值 `0.6.42` → 落地写 `0.6.43`；MUST NOT 写死字面号作判据）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。零新命令、零新 DTO ⇒ `bindings:export` 生成物零 diff。

---

## 能力

### 新增的能力

**无。** 回收单点属 worktree 生命周期的一个支面，归 desktop-change-worktree（该能力已持有 worktree 落位、create 建域、清理语义挂账）；本变更在既有能力面内增一条 requirement 并修正其对账指引。

### 修改的能力

- **desktop-change-worktree** —
  - **ADDED**「worktree 回收两段式（git 注销失败即 OS 层兜底）」：单点位置与两段式执行序、目录缺席幂等（`prune` 后 `Ok`）、主仓黑名单护栏、`prune` 仅在目标目录不在场后调用（不误伤活 worktree）、零重试零退避、`[vcs]` stderr 降级留痕、`Err` 成因面（路径 / 原 git 错误 / OS 错误 / 成因清单）、MUST NOT 单源对账、MUST NOT 指引 `git worktree remove --force`、双源指引为三处消费面共同形态、Windows `\\?\` 与 POSIX 同构、零 port / wire 变更。
  - **MODIFIED**「V1 范围与边界留痕」：第 6 条孤儿对账由「`git worktree list` 对账」改为「两源对账 + 回收单点自带 OS 层兜底（一次失败不再等于终态）+ 无记录孤儿须手动删目录」。
- **desktop-change-archive** — **MODIFIED**「Cleanup 步与 worktree 自动清理」：清理改为经两段式单点（`--force` 与兜底由单点决定）、失败面收窄为「两段式皆失败」、detail 必携成因（`vcs Err` 原文，MUST NOT 仅原始 git 错误串 + 单源指引）、幂等面显式含「已注销孤儿（git 无登记而目录在场）回收成功」；尽力步语义与 branch 零触碰零变化。
- **desktop-change-delete** — **MODIFIED**「写面删除与执行序」：执行段一的 worktree 清理改为经两段式单点（已注销孤儿亦可回收，勾选「同时清空 worktree」对这类残留不再是永久失败）、`Err` 面收窄为「两段式皆失败」且文案携成因；「worktree 移除失败即中断」场景改述为两段式皆失败，并补一条孤儿回收场景。
- **desktop-change-create** — **MODIFIED**「写面 create 操作进程内创建 change」：前置⑨ 文案由「引导 `git worktree list` 对账与手动清理」改为「含 worktree 落位根绝对路径 + 双源引导（`git worktree list` + 直接检查落位根子目录）」；注明该门槛是失控防线而非自愈手段（自愈归回收单点）。

### 其余能力零改动（含理由）

- **desktop-exec-runtime**「锁占用精确探针与占用者点名」：`lockprobe` 消费面不扩（D5）；`crates/infra/exec` 零 diff。
- **desktop-change-orchestration**：`step_cleanup` 只在 walker 内转发 `vcs Err` 到 detail，成因内容由 vcs 单点提供 ⇒ 零实现改动（doc 措辞随动，语义零变化）；`ArchiveVcsPort` 方法族签名不变。
- **desktop-change-state-store**：`ChangeRecord.worktree` 字段语义与归档双写零 git 触点红线均不变（回收仍不落库）。
- **desktop-change-worktree** 其余 requirement（落位单点 / create 分配 / 双 root 执行 / bootstrap / 脏仓 / merge-first / UI 可见性）：逐字不变。
- **desktop-ipc-type-bindings / desktop-app-shell**：零新命令、零 DTO 演进 ⇒ 生成物零 diff（`bindings:export` 后 `git diff` 为空）。
- **repo-archi-model / desktop-crate-layout**：零新 crate、零新依赖边（`fs` 属 std，`infra/vcs` 依赖集不变）。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/vcs/src/git.rs` | `remove_worktree` 重写为两段式（见上流程图）：`git worktree remove --force` → 失败分支按「目录是否在场」分流（缺席 → `prune_worktrees` → `Ok`；在场 → 兜底删除 → `prune_worktrees` → `Ok`）→ 兜底失败 `Err`（路径 + 原 git 错误 + OS 错误 + 成因清单 + 双源指引）。新增私有 `prune_worktrees(main_root) -> Result<String, String>`（`git worktree prune`，仅目标目录不在场后调用）与 `delete_tree(path) -> io::Result<()>`（Windows 经 `\\?\` 扩展前缀的 `fs::remove_dir_all`，绝对路径才加前缀）。新增私有护栏 `is_protected_path(main_root, path)`（path == main_root / 祖先 / 子目录 / 无父的根 → 拒绝）。兜底成功而 git 段失败时 `eprintln!` 以 `[vcs]` 前缀留痕（`commit_all` 既有先例）。doc 注释改写为两段式语义（原「补偿面，尽力回收」措辞随动） |
| `crates/infra/vcs/src/lib.rs` | `ProcessArchiveVcs::remove_worktree`：自组的「…请手动执行 `git worktree remove --force {path}` 清理」wrapper **退役**，改为纯透传 `git::remove_worktree`（文案单点收口，D8）；doc 注明两段式与「成因由单点提供」 |
| `crates/core/workflow/src/write/create.rs` | 前置⑨ `Err` 文案：含当前目录数、门槛、`worktree_root` 绝对路径与双源引导（`git worktree list` 对账 + 直接检查该根下的子目录，说明已注销孤儿不在 git 面）；`compensate_add_failure` / `compensate_tree_failure` 的残留指引改为同一双源形态（`git worktree remove --force <path>` 单源指引退役；branch 侧 `git branch -D` 保留） |
| `crates/core/orchestration/src/walker.rs` | `step_cleanup` doc 随动（失败面 = 两段式皆失败；detail 携 vcs 单点成因），实现零改动（仍 `StepSettle::passed` + 前缀 + `Err` 原文） |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 + 1 patch**（现值 `0.6.42` → `0.6.43`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行，MUST NOT 写死字面号） |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| 已注销孤儿回收（正向）：`add_worktree` 后手动删 `<main>/.git/worktrees/<name>/` 使 git 侧注销 → `remove_worktree` → `Ok`、目录消失、`git worktree list` 无该行、`.git/worktrees/` 无残留 admin | `crates/infra/vcs/src/git_test.rs` | 新增 |
| 原始语义持衡：登记与目录一致的 worktree → `remove_worktree` → `Ok`、目录消失、主仓树无损、branch 留存（既有四条用例零改动通过） | 同上 | 复用（零改动） |
| 目录缺席幂等：从未存在的路径 → `Ok`（原 `remove_worktree缺席路径err原文语义持衡` 的第一形态从 `Err` 改判 `Ok`，断言改为「零 panic、零删除、幂等」） | 同上 | 改造 |
| 非 worktree 普通目录（`Err` 面保留但语义改述）：目录在场、git 报 not a working tree → 兜底删除成功后 `Ok`；目录内文件随之消失（文案与断言按两段式改述，保留「Err 为 git 语境失败而非环境缺失」的环境缺失判据用例另置） | 同上 | 改造 |
| 护栏拒绝：path == `main_root` / path 为 `main_root` 祖先 / path 在 `main_root` 之内 → `Err` 且零删除 | 同上 | 新增 |
| `prune` 不误伤：两个活 worktree 在案，对其中一个执行两段式（走兜底分支）→ 另一个仍在 `git worktree list` 且目录在场 | 同上 | 新增 |
| 兜底失败 `Err` 携成因（平台分叉）：Windows 用「不含 delete 共享的句柄」（`OpenOptions` share=READ，沿 `lockprobe_test` 装置）占住 worktree 内文件 → 两段式皆失败 → `Err` 含路径 / `git worktree remove` / OS 错误 / 成因清单，MUST NOT 含 `git worktree remove --force <path>` 单源指引；释放句柄后重试 → `Ok`（收敛） | 同上 | 新增 |
| `ProcessArchiveVcs::remove_worktree` 的 `Err` 为单点透传（不再追加裸 git 命令指引） | `crates/infra/vcs/src/lib_test.rs` | 新增 |
| 前置⑨ 文案：目录数 8 → `Err` 含目录数 8、门槛 7、`worktree_root` 绝对路径与双源引导（既有断言以 `git worktree list` 为锚者保留，另加落位根路径与「直接检查」措辞） | `crates/core/workflow/src/write/create_test.rs` | 改造 |
| 补偿残留文案：`remove_worktree` 注入 `Err` → `Err` 文案含双源引导、不含 `git worktree remove --force <path>` 单源指引；`remove_worktree` 注入 `Ok` → 「已补偿回收」面零残留 | 同上 | 改造 |
| Cleanup 失败 detail 携成因：假件注入两段式皆失败的 `Err`（新形态文案）→ detail 含「worktree 清理失败」+ 路径 + 成因原文、步恒 `passed`、链收口 `Completed` | `crates/core/orchestration/src/walker_test.rs` | 改造（断言文案形态随动，结构断言零改动） |
| 删除面孤儿回收：`remove_worktree` 返回 `Ok` 而目录已消失（假件模拟兜底成功）→ `removed_worktree = true`、链续走；`remove_worktree` 注入 `Err`（两段式皆失败）→ 整体 `Err` 且库记录 / 产物目录未动 | `crates/core/workflow/src/write/delete_test.rs` | 新增 |

### 不要修改

- **`crates/core/workflow/src/write/worktree.rs`（`WorktreePort`）与 `crates/core/orchestration/src/port.rs`（`ArchiveVcsPort`）**：方法签名、trait 面、`Result` 形状逐字不动（D1 / D2）；出现 diff 即越界。
- **`crates/infra/exec/**`**：`lockprobe` / `attribution_text` 零改动（D5）；`crates/infra/checks/**` 同理。
- **`crates/core/orchestration/src/walker.rs::step_cleanup` 的实现体**（除 doc）：`StepSettle::passed` 尽力语义、detail 前缀、「branch 全程零触碰」均逐字不动（D7）。
- **`crates/core/workflow/src/write/delete.rs` 的执行序与 `delete_change_cascade` 范围**：磁盘先 / 库最后、三步幂等、级联六类行零改动；本变更只经 port 受益。
- **`crates/core/workflow/src/write/archive.rs`**：归档双写零 git 触点红线不动（Cleanup 不属双写本体）。
- **`crates/core/foundation/**`**：`\\?\` 前缀处理不住 foundation（本变更只需 vcs 局部一行前缀，且 sandbox / store 已有各自的剥除点）；零 diff。
- **`src-tauri/src/**` 与 `packages/desktop/src/**`**：零命令、零 DTO、零前端文案（残留文案全部经后端 `Err` / `warnings` / detail 出线）；`src/types/generated/bindings.ts` 零 diff。
- **`openspec/changes/worktree-remove-orphan-defect/explore.md`**：只读。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **孤儿自愈**：已注销孤儿（git 无登记、目录在场）经 `remove_worktree` → `Ok`、目录从磁盘消失、`git worktree list` 不再含它、`<main>/.git/worktrees/` 无残留 admin；全程一次调用、零重试 | `git_test`（真实 git tempdir 夹具 + 手动删 admin 制造孤儿） |
| AC-2 | **原语义零漂移**：登记与目录一致的 worktree → `Ok`、目录消失、主仓树无损、branch `change/<name>` 留存、同路径可再 `add`；主仓 active 树零触碰 | `git_test`（既有四条用例零改动通过） |
| AC-3 | **幂等**：对不存在路径调 `remove_worktree` → `Ok`（MUST NOT 因 `not a working tree` 报错）；连续两次调用均 `Ok` | `git_test` |
| AC-4 | **护栏**：path == `main_root` / `main_root` 祖先 / `main_root` 子目录 → `Err` 且磁盘零变化（主仓文件全在） | `git_test` |
| AC-5 | **`prune` 不误伤**：兜底分支执行后，另一活 worktree 仍在 `git worktree list` 且目录在场 | `git_test` |
| AC-6 | **失败面成因可辨**：两段式皆失败 → `Err` 含绝对路径、原 git 错误、OS 错误与成因清单；且**不含**「`git worktree remove --force <path>`」单源指引；释放占用后重试收敛为 `Ok` | `git_test`（Windows 句柄装置）+ `lib_test`（文案单点透传） |
| AC-7 | **文案双源**：create 前置⑨ 的 `Err` 含 worktree 落位根绝对路径与「`git worktree list` + 直接检查落位根子目录」双源措辞；create 两处补偿残留文案同为该形态且不含单源 `git worktree remove --force` 指引 | `create_test` |
| AC-8 | **尽力步不变**：Cleanup 遇两段式皆失败的 `Err` → 步 `passed`、detail 携成因、Seal / Finalize 照常收口、run `Completed`、branch 零触碰 | `walker_test` |
| AC-9 | **删除面收口**：勾选清空对已注销孤儿的 change → `removed_worktree = true`、branch 删除、产物目录与库记录级联；两段式皆失败 → 整体 `Err`、库记录与产物目录原样未动 | `delete_test` |
| AC-10 | **零面漂移**：两 port 签名零 diff、`crates/infra/exec` 零 diff、`bindings:export` 后生成物零 diff、`src-tauri/Cargo.toml` crate 版本行不动、`package.json` version 恰 +1 patch | `git diff` 复核 + `bindings:export` |

> **验收表口径**：AC-1…AC-9 是本变更的**能力验收面**（每条对应 `specs/` 下 delta requirement 的行为判据）；静态检查链、全量测试与版本递增是全仓既有交付门（AC-10 只核其零漂移部分）。

### 验证命令

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p vcs-runtime
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p workflow
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml --all-targets
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run check
git diff HEAD -- packages/desktop/package.json   # version 行恰 +1 patch
git diff HEAD -- packages/desktop/src/types/generated/bindings.ts   # 空 diff
```

> **验收面口径**：回收语义止于**单元测试**（真实 git tempdir 夹具 + Windows 句柄装置）。真机「杀毒扫描正在扫 worktree 时归档」不属自动化验证面；其判据被 AC-1 / AC-6 两个方向夹住（孤儿必自愈、真占用必 `Err` 携成因）。

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 兜底删除误伤**（记录路径异常 → 删掉非 worktree 的目录） | 用户数据丢失 | 低 | 只设黑名单护栏（`main_root` 自身 / 祖先 / 子目录 / 文件系统根一律拒绝，AC-4）；路径来自机器写入的 `ChangeRecord.worktree`；兜底**只在 git 已失败后**发生（顺序即第二重护栏）；D4 被拒备选的失效机制（`.git` 判据在最需要自愈时拒绝）已在 spec 留痕，供后续评估 |
| **R2 形态 B′ 的 `.git` 已被先删** | 部分残留目录不再可自动回收 | 中 | 该形态下目录无 `.git` 也照兜底（D4 不设形态判据）；`Err` 文案直接给出「删除该子目录」的可执行动作，用户一次即可自救，不再是「查不到 / 删不掉」 |
| **R3 只读属性 / 保留设备名 / ADS 阻断 `remove_dir_all`** | 兜底失败 → `Err`（诚实上报，非静默孤儿） | 低 | D9：git 侧删除本处理只读；`\\?\` 覆盖超长路径；未实测面（explore 待核实）作为已知限制留痕，`Err` 携 OS 错误码便于复现 |
| **R4 `git worktree prune` 误删他项 admin** | 活 worktree 失去登记 | 低 | `prune` 仅在目标目录已不在场后调用（顺序约束）+ AC-5 反向用例钉住；`prune` 语义本身只清「worktree 目录不存在」的 admin |
| **R5 幂等语义改变（缺席 → `Ok`）掩盖真实错误**（如记录路径拼错） | 误判「已回收」 | 低 | 消费侧 delete / Cleanup 已有 `is_dir` 判据且缺席本就跳过；创建补偿面反而因此不再产出假残留告警（D3）；「路径拼错」的可观测面由 create 前置⑦（目标目录已存在）与前置⑨（目录数门槛）承担 |
| **R6 三处文案改动波及既有断言**（`create_test` / `walker_test` / `lib_test` 以 `git worktree list` / `git worktree remove --force` 为锚） | 测试红 | 高 | 变更范围测试表已逐条列出改造点；双源措辞**保留** `git worktree list` 字样（既有 contains 断言多可存活），仅在「单源指引」断言处改判 |
| **R7 `Err` 长度膨胀**（路径 + git 原文 + OS 错误 + 成因清单）进 detail / 弹窗 | 一个 `passed` 步的 detail 过长 | 中 | detail 只有清理失败才携全量成因（罕见分支）；成因清单为固定短语（不插值进程名 / 文件路径）；文案单点可统一截断策略 |
| **R8 与 CodeGraph 关停的次序**（先删目录、server 仍持 cwd ⇒ 两段式都失败） | 本变更后仍可能 `Err`（但成因可见） | 中 | 本变更是 CodeGraph 侧要求的前置（`builtin-codegraph-mcp-bridge` 8.10 三选一）：关停放哪不属本变更；`Err` 成因清单显式点名「常驻子进程」，使该形态可诊断 |
| **R9 macOS / Linux 同构未实测**（cwd 锁是否阻断 `rmdir`） | 平台分叉判断失误 | 低 | 两段式本身平台无关（POSIX 允许删被打开文件 ⇒ 兜底更易成功）；仅扩展路径前缀为 `cfg(windows)`；无平台分叉逻辑，故无分叉 bug 面 |
| **R10 版本与生成物纪律** | 交付门红 | 低 | `package.json` 按「落地时 HEAD +1 patch」相对递增（现值 `0.6.42` → `0.6.43`）；`Cargo.toml` crate 版本行不随动；零新命令 ⇒ 生成物零 diff（AC-10） |
