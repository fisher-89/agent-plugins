# worktree 删除的永久孤儿缺陷（Windows）

workflow_type: requirement

> **一句话**：`infra/vcs` 的 `remove_worktree` 只信 `git worktree remove`，而 git 的语义是"先拆登记、再删文件"。于是**任何一次删除失败都会留下一个 git 永远看不见、也永远删不掉的孤儿目录**——且三处调用点的错误文案都在引导用户去查一个查不到问题的地方。
>
> 这条**与 CodeGraph 无关**；CodeGraph 只是把"偶发"变成"必然"的一个实例。

## 来源

从 `openspec/explores/builtin-codegraph-mcp-bridge.md` 的索引归属推演中拆出。原文的 8.10 把两件事混在一起：**codegraph server 该在哪关停**（那一侧的设计问题，属 codegraph 接入）与 **`remove_worktree` 失败即永久孤儿**（这一侧与 codegraph 无关，是 `infra/vcs` 的既有缺陷）。后者证据充分、边界清楚、可独立交付，故单独立文。

拆分后的边界：
- 本文：缺陷本体、实验证据、兜底配方、三处调用点的文案修正、触发面枚举。
- `builtin-codegraph-mcp-bridge` 8.10：关停放哪（三选一）、`CODEGRAPH_NO_DAEMON=1` 的托管要求。

实验证据见第三节（2026-11-13 实测，非推理）。

## 一、缺陷陈述

### 1.1 实现（单点）

`crates/infra/vcs/src/git.rs:223`：

```rust
pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String> {
    git(main_root, &["worktree", "remove", "--force", &worktree.to_string_lossy()])
        .map(|_| ())
}
```

一次 git 调用，成功即可，失败即 `Err`。**没有任何自愈段。**

### 1.2 三处调用点，三种错误处理，同一个失效结局

| 调用点 | 位置 | 失败处理 | 用户看到什么 |
|---|---|---|---|
| 归档 Cleanup 步 | `orchestration/src/walker.rs:2798` | **尽力步**：吞为 detail 警告，步恒 `passed`，归档照常收口（`walker.rs:2764`） | 归档"成功"，日志里一行 warn。**孤儿静默留盘** |
| 删除 change | `workflow/src/write/delete.rs:65` | **硬 Err**，整条删除链中止 | `worktree 移除失败（<path>）: <git 原文>`。**失败可见，归因不可见** |
| create 树写出补偿 | `workflow/src/write/create.rs:424` | 收进 `residuals`，随最终 Err 呈现 | `补偿回收未完成，残留: worktree <path>: <git 原文>…` |

### 1.3 三段文案犯同一个错

三处都在指引用户：

> 请以 `git worktree list` 对账并手动清理（`git worktree remove --force <path>` / `git branch -D <branch>`）

而下面第四节会说明：**孤儿正是 `git worktree list` 不显示的那一类**，而 `git worktree remove --force` 对它们报 `not a working tree`（exit 128）。用户按指引操作，会得出"没有残留"或"删不掉"的结论，然后是第二次、第三次失败。

---

## 二、为什么"失败"不可重试（git 的删除语义）

实测得出的执行序（`--force` 与非 `--force` 一致）：

```
git worktree remove --force <path>
        │
   ①    ├── 删 <main>/.git/worktrees/<name>/          ← admin 目录（登记）
        │
   ②    ├── 从 worktree 登记表移除该条目
        │
   ③    ├── 递归删 <path> 下的文件
        │        │
        │        └── 若某个文件被独占打开 ──► 在此 abort
        │
   ④    └── 删 <path> 目录本身
                 │
                 └── 若目录被占用（cwd / 句柄）──► 在此 abort
```

**①② 在 ③④ 之前**。所以：

```
        任何删除失败
             ↓
   git 侧已"注销"该 worktree
             ↓
   ┌─────────────────────────────────────────┐
   │ 目录还在磁盘上，但：                     │
   │  · git worktree list   不显示            │
   │  · git worktree prune  无输出（无事可做）│
   │  · git worktree remove 报 not a working  │
   │    tree (exit 128)                       │
   │  · 目录内 .git 指针指向已删除的 admin    │
   │    ⇒ 从里面跑任何 git 命令都             │
   │      fatal: not a git repository: (NULL) │
   └─────────────────────────────────────────┘
             ↓
      无锁之后 plain rm -rf 才能删掉
      （只能靠 OS 层删，git 已完全失去该对象）
```

**关键点：不存在"失败即回滚、可安全重试"这条路径。** 这与我最初的直觉相反，也是这个缺陷真正难缠的地方——它不是"偶发失败"，而是"一次失败即终态"。

---

## 三、实验记录（2026-11-13，Windows / git 2.54.0.windows.1）

沙箱：临时 git 仓 + `git worktree add` 落在 `.worktrees/<repo>/<name>`（复刻本仓布局），逐个注入占用源后跑 `git worktree remove`。

| 形态 | 占用源 | git 报错 | 删后**目录内容** | git 登记 / admin | 能否用 git 回收 |
|---|---|---|---|---|---|
| 基线 | 无 | 成功 (0) | 全删 | 正常清理 | — |
| **A：cwd 被占** | Node 进程 `chdir` 到该目录 | `Permission denied` (255) | **内容全被删，只剩空壳目录** | **已拆** | ❌ |
| B：Node 打开的句柄 | `fs.openSync` | 成功 (0) | 正常删除 | 正常 | — |
| **B′：SQLite 语义句柄** | PowerShell `FileShare.Read`（不含 Delete） | `Invalid argument` (255) | **完整留存**（`.codegraph/` + `src/` 都在） | **已拆** | ❌ **永久** |
| 强杀后 | 先 `taskkill /F` 再删 | 成功 (0)，**94ms** | 全删 | 正常清理 | — |

> **形态 B 的陷阱**：Node 的 libuv 打开文件时自带 `FILE_SHARE_DELETE`，所以"Node 开着这个文件"并不阻止删除。**只有模拟 SQLite / Windows 原生 `FileShare.Read`（不含 Delete）才能复现真实数据库的占用语义。** 第一版实验因此假阴性，必须重做。

### 3.1 四条硬结论

1. **git 先拆登记与 admin，再删文件**——与 `--force` 无关（非 force 实测同样先拆）。任何删除失败都留下"已注销的孤儿"。
2. **失败形态取决于占用源**：cwd 占用锁的是**目录**，内容先被删光、只剩空壳；文件句柄锁的是**文件**，删除中途 abort、内容完整留存。两种形态后果不同（一个只剩空目录，一个留几十 MB 数据），但都是孤儿。
3. **孤儿对 git 永久不可见、不可回收**（见第二节流程图）。
4. **强杀立即释放句柄**：`taskkill /F` 后 94ms 内 `remove --force` 即成功。⇒ 释放占用**不需要优雅握手、不需要等待、不需要驱动侧 `close()`**；"kill 然后删"是正确姿势。

### 3.2 兜底配方（已验证）

```
两段式：
  git worktree remove --force <path>
     │ 成功 ──► 完事
     └ 失败 ──► std::fs::remove_dir_all(<path>)      （OS 层删，不看 git 登记）
                git worktree prune                    （清可能的残留 admin）
                   │ 成功 ──► 完事（原错误降为 detail）
                   └ 失败 ──► Err（此时才真的需要人介入，附归因）
```

两条路径都验过：

- 对**已登记但目录已消失**的 worktree：`worktree list` 标 `prunable`，`prune -v` 输出 `Removing worktrees/eps: gitdir file points to non-existent location` 并清掉 admin 目录；
- 对**已注销的孤儿**（形态 A / B′）：`prune` 是 no-op，但此时 `remove_dir_all` 直接就能删（无锁后）。

### 3.3 一个不能交换的顺序

清理沙箱时 `rm -rf` 同样报 `Device or resource busy`——**plain 删除在句柄持有期间也失败**。所以"释放占用"必须排在"两段式删除"**之前**，顺序不可交换。兜底不是"绕过占用"，只是"绕过 git 的登记语义"。

---

## 四、后果：孤儿如何累积，以及为什么没人发现

```
   一次删除失败（任何原因）
        ↓
   孤儿目录（git 已注销）
        ↓
   ┌──────────────────────────────┬─────────────────────────────┐
   │ 归档链                        │ 删除链                       │
   │ 尽力步 ⇒ 归档成功            │ 硬 Err ⇒ 用户看到失败        │
   │ 孤儿静默留盘                  │ 但指引去查 git worktree list │
   │                              │ ⇒ 查不到 ⇒ 无法归因           │
   └──────────────────────────────┴─────────────────────────────┘
        ↓
   累积（每条 change 一次机会）
        ↓
   create 前置⑨：worktree 目录数 > 5+2=7 才拦
        ↓
   文案：「worktree 目录数已达保底门槛（当前 N，门槛 7）：
          请先以 git worktree list 对账并手动清理后再创建」
        ↓
   ✗ list 什么都不显示 ⇒ 用户无法按指引自救
```

**实测印证了误导性**：实验沙箱收尾时磁盘上有 **2 个孤儿**，`git worktree list` 只显示主仓一个；而前置⑨ 的计数函数（`create.rs:196` 的 `fs::read_dir` 计数）会把它们数进去。**计数源（磁盘）与对账指引（git）不是同一个真相源**——这是文案层面最直接的 bug。

---

## 五、谁会触发（不只是 CodeGraph）

| 触发源 | 机制 |
|---|---|
| 杀毒软件实时扫描 | 扫描期独占打开新写入的文件 |
| Windows Search / Everything 等索引器 | 同上 |
| 编辑器 / IDE 打开了 worktree 内的文件 | 文件句柄 |
| 用户开着资源管理器 / 终端 `cd` 在 worktree 内 | **目录 cwd 锁** |
| 备份 / 云同步客户端（OneDrive / Dropbox 等） | 文件句柄 |
| 长驻子进程（dev server、watcher、**codegraph server**） | cwd + 文件句柄 |
| 上一次删除留下的半完成态重试 | 已注销 ⇒ `not a working tree` |

前两行意味着：**这不是"接了 CodeGraph 才会遇到"的问题**。在任何 Windows 用户机器上，归档时撞上一次杀毒扫描就足够。CodeGraph 只是把概率从"偶发"推到"必然"（它每次都持有 cwd + db 且不主动退出）。

---

## 六、修复方案

### 6.1 改 `remove_worktree` 本体（单点，三处调用点自动受益）

```
git remove --force
  └ 失败 → remove_dir_all → prune
              └ 失败 → Err（携：原 git 错误 + remove_dir_all 错误 + 占用者归因提示）
```

### 6.2 三处文案同步修正

- 不再单指 `git worktree list`；改为「`git worktree list` + 直接检查 `<main_root>/../.worktrees/<repo>/` 目录」两条；
- 归档 Cleanup 的"尽力步"语义可以保留（不因清理失败而失败归档），但**警告必须携带成因**（哪个文件/目录被占）而不是原始 git 错误串。

### 6.3 边界：什么时候仍应上报

- `remove_dir_all` 也失败 ⇒ 真的有人占用 ⇒ Err，并尽量给出占用者（`infra/exec` 已有 `lockprobe` / `attribution_text` 可复用——被锁文件 + 占用者归因原料）。
- 不要为了"清理成功"而重试/退避：实测强杀即释放，**没有需要等待的窗口**；加退避只会掩盖真正的持久占用。

### 6.4 与 CodeGraph 的接口（若两者同做）

CodeGraph 那侧还有一条**独立于本缺陷**的要求：`Cleanup` 前必须主动关停该 worktree 的 server。落法三选一（下沉进 `remove_worktree` / 塞进 `Cleanup` 步 / 新增计划图步）见 `builtin-codegraph-mcp-bridge` 8.10。

若选"下沉进 `remove_worktree`"，则本缺陷的修复与它天然合流：`remove_worktree` 变成"先释放占用者（若注册表里有），再两段式删除"。**但本缺陷的修复不应等 CodeGraph**——它自成一条可独立交付的改动。

---

## 七、待裁决 / 待核实

**裁决**

1. 归档 Cleanup 遇到"两段式也失败"时，是否仍然尽力（只警告）？我倾向保留尽力（清理失败不该回滚已收口的归档），但警告必须带成因。
2. 要不要引入"占用者归因"（复用 `exec::lockprobe`）？成本是多一次 fs 探测，收益是用户第一次就知道该关什么。

**待核实**

- `remove_dir_all` 面对只读文件、超长路径（>260）、Windows 保留名的行为（现有 `add_all_skipping_reserved` 已处理保留名入索引，但删除面未验）。
- `prune` 在所有失败分支下是否安全（它只清"目录不存在"的 admin，理论上不会误删活 worktree——需一条反向用例钉住）。
- 形态 A（cwd 锁）在 `--force` 与不带 force 时行为是否一致（本次只测了 clean tree 的非 force；脏树非 force 会在①②之前拒绝，那条路径未测，但本仓三处调用点全用 `--force`）。
- macOS 上是否有同构形态（POSIX 允许删除被打开文件，但 cwd 锁是否阻止 `rmdir` 需实测；影响是否需要 `cfg` 分叉）。
