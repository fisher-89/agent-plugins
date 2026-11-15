# 设计: worktree-remove-orphan-defect

> **变更**: worktree-remove-orphan-defect
> **状态**: 设计定稿（基于已定稿 proposal + 4 份 delta spec）
> **基于**: `proposal.md`（四现象 / 两段式提案 / D1–D9 裁决 / 变更范围 / AC-1…AC-10 / R1–R10）与 `specs/{desktop-change-worktree,desktop-change-archive,desktop-change-delete,desktop-change-create}/spec.md`
> **性质**: **单点回收自愈化 + 文案双源校正**。`crates/infra/vcs` 的 `git::remove_worktree` 由「一次 `git worktree remove --force`，失败即 `Err`」改为两段式（git 失败 → OS 层 `remove_dir_all` → `git worktree prune`）；归档缝的自组 wrapper 退役为纯透传（文案单点收口）；create 的四处面向用户文案（前置⑨ + 两处补偿残留 + 一处补偿成功确认）改为双源对账；`walker` 的 Cleanup 步 doc 随动（实现零改动）。
>
> **本篇边界**: `crates/infra/vcs/src/git.rs`（两段式 + 四个新私有助手 + doc）、`crates/infra/vcs/src/lib.rs`（`ProcessArchiveVcs::remove_worktree` 透传）、`crates/core/workflow/src/write/create.rs`（文案单点 helper + 四处文案 + 两处私有函数加形参）、`crates/core/orchestration/src/walker.rs`（doc）、`packages/desktop/package.json`（版本一行）。**零新增 crate、零新增依赖（`std::io` / `std::fs` 属 std）、零 port 签名变更、零 wire DTO、零新命令、零前端改动、零架构模型改动**。

---

## 提案与规格同步状态

proposal 的两段式提案、D1–D9 九条裁决与 4 份 delta spec 均已定稿。本设计不重复其论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体函数与文案字面**（见「关键设计决策」DD1–DD10 与「文案单点面」）。
2. **消解 spec 内部的一处张力**并给出可执行判据：`Err` 须「携原 git 错误」而 `git_impl` 的错误原文本身回显命令串 `git worktree remove --force <path> 失败（退出码 N）`——「MUST NOT 指引用户执行该命令」因此**只能**以「指引短语缺席」为判据，不能以「命令串缺席」为判据（DD3）。
3. **把 R1（误删）与 AC-1（孤儿必自愈）的互斥性证否**：护栏的「`main_root` 子目录」判据在布局上**不可能**命中合法 worktree（证明见 DD1），故护栏不削弱自愈面。
4. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（测试由独立相位承接）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **`crates/infra/vcs/src/git.rs`**：`:223-231` `remove_worktree(main_root, worktree)` —— 一次 `git(main_root, &["worktree", "remove", "--force", …])` + `.map(|_| ())`，零自愈段；`:208-221` `add_worktree`（同族先例：失败前先装 exclude 以保「失败面零残留」）；`:9-73` `git` / `git_impl`（`Err` 形如 `git <args> 失败（退出码 N）: <stderr>`，即**原文回显命令行**——DD3 的依据）；`[vcs]` stderr 留痕既有先例 `:102`（保留名跳过）、`:272`（`commit_all` 幂等收口）；当前**无** `use std::io;` / `use std::fs;`（新增一行 `use std::io;`）。
- **`crates/infra/vcs/src/lib.rs`**：`:98-100` `ProcessWorktree::remove_worktree`（create / delete 缝，已为纯透传——**逐字不动**）；`:184-194` `ProcessArchiveVcs::remove_worktree`（归档缝，自组 `worktree 移除失败（{path}）: {error}；请手动执行 git worktree remove --force {path} 清理` —— 本变更退役该 wrapper）；`:30-42` `WORKTREES_DIR_NAME = ".worktrees"` + `worktree_root(workspace_root)`；`:43-66` `worktree_root_from_toplevel`（私有；落位根 = `<git 根父目录>/.worktrees/<仓库目录名>`）。
- **`crates/core/workflow/src/write/create.rs`**：`:24` `ACTIVE_CHANGE_LIMIT = 5`、`:30` `WORKTREE_DIR_SLACK = 2`（**零改动**）；`:193` `let worktree_root = vcs.worktree_root(main_root)?;`；`:194-213` 前置⑨（`fs::read_dir(&worktree_root)` 数直属子目录 → `worktree_dirs > 7` 即拒；文案 `:209-211` 仅「请先以 git worktree list 对账并手动清理后再创建」——计数源为磁盘、对账源为 git 的错配所在）；`:258-260` `compensate_add_failure(store, vcs, main_root, &id, &branch, &error)` 调用点；`:268-270` `compensate_tree_failure(store, vcs, main_root, &id, &branch, &worktree, &error)` 调用点；`:382-411` `compensate_add_failure`（残留言案 `:403-405`）；`:413-447` `compensate_tree_failure`（成功面 `:436-438`、残留面 `:440-443`）；`:449-456` `compensate_record_delete`（**零改动**）；`:457-459` `residual_note`（**零改动**）。
- **`crates/core/orchestration/src/walker.rs`**：`:2764-2769` `step_cleanup` doc；`:2791-2806` 执行体（`None` → `TXT_SKIP_LEGACY`、`!is_dir` → `TXT_SKIP_ABSENT`、`Ok` → 「已移除 worktree（<path>）」、`Err` → 「worktree 清理失败（尽力步，不影响归档收口）: {error}」）；`:47-48` 两个 detail 常量。**实现零改动**，只改 doc。
- **`crates/core/workflow/src/write/delete.rs`**：`:59-89` 执行段一（`if worktree_path.is_dir() { vcs.remove_worktree(...)?; removed_worktree = true; }` + branch 判据）。**零改动**（`is_dir` 前置判据保留——缺席跳过是既有幂等面，两段式的缺席 `Ok` 只在其后兜住竞态）。
- **既有测试断言锚（本相位不改，登记给测试相位）**：`git_test.rs:378-402`（缺席路径 / 非 worktree 目录的 `Err` 面——第一形态改判 `Ok`）；`lib_test.rs:460-500`（`ProcessArchiveVcs` 的 `Err` 手工指引断言——`git worktree remove --force {path}` 字面锚退役，改判「指引短语缺席」）；`create_test.rs:1691-1810`（两处补偿的 `git worktree list` 锚可存活）、`create_test.rs:2035-2070` / `:2159-2190`（前置⑨ 文案锚，追加落位根路径与「直接检查」措辞）；`walker_test.rs:7331-7425`（假件注入的 `Err` 字符串内嵌 `请手动执行 git worktree remove --force {path} 清理` —— 注入文案随新形态改写）。
- **架构模型**：`App.Backend.Infra.VcsRuntime`（`api = none`，`modelFile = .dev-team/architecture/models/03-backend-infra.toml:39`）——本变更**零新增公共导出**（`remove_worktree` 是 `pub(crate)`），`.dev-team/architecture/**` 零改动，`api:export` / `api:check` 零 diff；依赖边零变化（`std::io` / `std::fs` 属 std，不用 `foundation` / `store` 的 `\\?\` 剥除点）。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.42"` → 落地写 `0.6.43`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml:4`（crate 版本 `0.1.0`）**不随动**。

---

## 关键设计决策

> D1–D9 见 proposal「裁决表」（已定稿，本篇不重复）。下表 DD1–DD10 是 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | 护栏的落点、判据与归一 | 护栏在 **`remove_worktree` 入口第一句**（早于 git 段）：`is_protected_path(main_root, worktree)` 命中四类即 `Err`。四类 = ① `worktree.as_os_str().is_empty()` ② `worktree` 无父目录（文件系统根）③ `main_root.starts_with(worktree)`（path == main_root 被此式包含）④ `worktree.starts_with(main_root)`。两侧先经 `comparable_path`（`fs::canonicalize` 优先、失败退 `std::path::absolute`；Windows 剥 `\\?\` 前缀并小写）归一 | 入口而非「仅兜底段」加护栏：`git worktree remove --force <main_root>` 在 git 侧确实会拒绝，但**护栏必须在任何副作用之前**（AC-4 要求「磁盘零变化」，含 git 侧的登记改动）。归一是必需项：`main_root` 由调用方传入、`worktree` 来自 `ChangeRecord`，两侧大小写 / 分隔符 / 扩展前缀形态不保证一致（Windows 大小写不敏感，`starts_with` 逐分量比较会漏判 `D:\Ws` vs `d:\ws`）。**黑名单之外不设形态判据**（含「目标须是 `<落位根>/<name>` 形态」这一看似安全的加码）：记录路径由 create 单点写入（`worktree_root.join(name)`），异常形态只可能来自库记录被人工改写，而形态判据在删除中途 abort 的窗口上（DD2 的最需自愈形态）必然误拒——这正是 D4 拒绝 `.git` 判据的同一逻辑 |
| DD2 | 「目录在场」判据的单点 | **不设** `worktree.exists()` 前置分支；唯一判据是 `delete_tree` 的返回值——`Ok` / `Err(kind == NotFound)` → `prune` → `Ok`，其余 `Err` → 失败面 | 两个判据（先 `exists()` 再删）之间存在 TOCTOU 窗口，且 `exists()` 对断链符号链接返回 `false`（会静默留一个链接残骸）。`remove_dir_all` 的 `NotFound` 是「不在场」的权威判据，同时覆盖「从未存在」「已被手动删」「检查后瞬间消失」三形态，且**零额外 stat** |
| DD3 | 「携原 git 错误」与「MUST NOT 单源指引」的张力 | `Err` 必携 `git_impl` 原文（其中回显 `git worktree remove --force <path> 失败（退出码 N）`）；「MUST NOT 指引」的判据是**指引短语缺席**——实现侧 MUST NOT 出现 `请手动执行` / `请以 git worktree list 对账并手动清理` 两类祈使用语，测试侧锚定这两个短语而非裸命令串 | 两条要求同为 spec 明文：AC-6「含原 git 错误」+「不含单源指引」。原样回显命令行是**复现判据**（R3 / R8 要靠退出码与 stderr 定位），删除 / 改写原文会让失败面失去可诊断性；而「指引」是祈使用语，用户执行它才对孤儿无效。因此二者可共存，判据必须落在祈使句上——不写明这条，测试相位必然写出「`!error.contains("git worktree remove --force")`」这类与原文要求直接冲突的断言 |
| DD4 | 文案单点面的**两处**（不是一处） | ① vcs 侧单点 = `git::remove_worktree` 的 `Err`（覆盖 delete + Cleanup 两消费点）；② create 侧单点 = `create.rs` 私有 `worktree_reconcile_hint(&Path) -> String`（覆盖前置⑨ + 两处补偿残留 + 一处补偿成功确认） | D8 只钉了 vcs 侧收口。create 的四处文案不能共享 vcs 侧函数（`core/workflow` 不依赖 `infra/vcs`，且前置⑨ 在 vcs 调用之前），若各写一份必然漂移（正是现状三份不同文案的成因）。两处单点各自覆盖自己的消费面，跨 crate 之间以 spec 的「同一双源形态」对齐（形态一致性由 spec 的 Scenario「残留指引双源」横向钉住） |
| DD5 | `prune` 的调用点与失败处置 | 单点调用：第二段收敛后（OS 删除成功**或** `NotFound`）统一 `let _ = prune_worktrees(main_root);` 再 `Ok`。`prune_worktrees` 返回 `Result<String, String>`（保 spec Module Contract 签名），调用点 `let _ =` 丢弃——失败零日志、零上抛 | 为何在「两支收敛之后」而非各支内部各调一次：两支的收口动作完全相同（目录已不在场 → 清残留 admin），写成单点即不可能出现「一支忘了 prune」。为何失败**完全静默**：spec 明文「静默吞」；prune 失败的唯一后果是残留 admin 目录（`git worktree list` 会标 `prunable`，用户仍可自救），而此刻主目标（目录自磁盘消失）已达成，把它升级成 `Err` 会把成功变成失败 |
| DD6 | `delete_tree` 的 Windows 形态与已知限制 | Windows：路径为**绝对路径**且不以 `\\?\` 开头时包一层扩展前缀（`PathBuf::from(format!(r"\\?\{}", text))` 后 `fs::remove_dir_all`），否则直删；POSIX：恒 `fs::remove_dir_all(path)`。零重试、零属性清理、零保留名特判 | 扩展前缀覆盖「路径 > 260」这一唯一实测可预判的 OS 层障碍（D9）；前缀形态沿仓库既有先例（`git_test.rs:2339` 的 `format!("\\\\?\\{}", …)`）。**已知限制留痕**：只读属性 / Windows 保留名 / ADS / 符号链接目标会 `Err`（git 侧删除本已处理只读，故只读面实际罕见；其余按 D9 诚实上报，`Err` 携 OS 错误码便于复现）。不做「清属性 + 逐项删除」的完整梯子：那要自实现递归遍历并处理保留名 / ADS / 尾随空格，风险高于收益 |
| DD7 | 幂等语义改变（缺席 `Ok`）对三处消费点的影响 | create 补偿面**自动受益**（`add_worktree` 失败且目录未生成 → 不再产出假残留行）；delete 面与 Cleanup 面的 `is_dir` 前置判据**逐字保留**（缺席跳过先于 port 调用发生，两段式的缺席 `Ok` 只在其后兜住竞态与「记录路径在而目录刚消失」） | 保留 `is_dir` 而不删：删掉会把「缺席跳过」的 detail 文案（`TXT_SKIP_ABSENT` / `removed_worktree = false`）语义改成「调了一次回收」，而这两个可观测面由 spec（archive「worktree 缺席幂等跳过」、delete「目录已缺席仍清分支」）逐字钉住 |
| DD8 | 失败面文案形态 | **单行**（`；` 与 `：` 分隔，**零 `\n`**），五段：路径 / 原 git 错误 / OS 删除错误 / 固定成因清单 / 双源对账。成因清单为**常量短语**（`WT_HOLDERS_HINT`），不插值进程名与文件路径 | walker 把 `Err` 原文拼进 step detail（单行渲染面），`\n` 会在 UI 与日志里表现为不可控空行（R7）。清单不插值：失败面是「目录树里某个未知文件被锁」，目录规模（`node_modules`）下逐文件点名不可行（D5 结论），故给**可执行的排除动作**（关终端 / 关编辑器 / 杀子进程 / 暂停扫描 / 暂停同步）而不是假精确的占用者 |
| DD9 | git 段失败的**无例外**处置 | `git` 段的任何 `Err`（含 `git 不可用（PATH 未发现 git）` 的 spawn 失败）同归第二段，**不设分支** | 与 D4 拒绝形态判据同一逻辑：分支只增加误判面。回收的目标是「目录不再在盘」，与 git 是否可达无关；而「git 不可达」的环境诊断已由上游 `probe`（create 前置⑤）与 `git_impl` 的显式引导承担。若在此加「git 缺失即不兜底」，`Err` 会退化成用户无法自救的形态（既没回收，也没有 `prune` 的替代路径），与本变更方向相反 |
| DD10 | `walker` / `delete` 的改动边界 | `walker.rs` 只改 `step_cleanup` 的 doc（+ 一句「失败面 = 两段式皆失败、成因由 vcs 单点提供」），执行体逐字不动；`delete.rs` 逐字不动 | 两处的错误处理策略各自正确（归档尽力 / 删除硬失败），本变更收回的是**失败面**（从「一次 git 失败」到「两段式皆失败」）与**成因可辨性**，两者都由 vcs 单点提供；在消费点再写一遍文案就是 D8 要消灭的漂移源 |

---

## 文案单点面（实现字面）

### vcs 侧（`git.rs`，delete + Cleanup 共用）

- **常量** `WT_HOLDERS_HINT`：`"终端 cwd（终端或子进程在 worktree 内）、编辑器打开的文件、常驻子进程（dev server / watcher）、杀毒实时扫描、云同步客户端"`。
- **兜底成功（目录在场 → OS 删成功）**：`eprintln!("[vcs] git worktree remove 失败，已以 OS 层兜底回收 {}: {error}", worktree.display())`。
- **目录不在场（`NotFound`）**：`eprintln!("[vcs] git worktree remove 失败且目录不在场（按幂等成功收口）{}: {error}", worktree.display())`。
- **两段皆失败**（单行）：
  ```
  worktree 回收失败（{path}）：git 与 OS 层删除均失败；原 git 错误: {git_error}；
  OS 删除错误: {io_error}；常见占用源: {WT_HOLDERS_HINT}——释放占用后重试即收敛
  （强杀占用进程后立即释放，无需等待）；残留对账须双源: ① git worktree list；
  ② 直接检查 worktree 落位根 {root_hint} 下的子目录——删除失败留下的残留不在
  git 登记面（git 已注销: list 不显示、prune 无事可做、remove 报 not a working tree），
  直接删除该子目录即可
  ```
  （实际写成单行字符串；`{root_hint}` = `worktree.parent()` 的绝对路径，缺父时退「（未知：worktree 路径无父目录）」——护栏已排除该支，纯防御）

### create 侧（`create.rs`，前置⑨ + 两处补偿残留 + 补偿成功确认共用 `worktree_reconcile_hint`）

```
worktree 残留对账须双源: ① git worktree list；② 直接检查 worktree 落位根 {worktree_root} 下的子目录
——删除失败留下的残留不在 git 登记面（git 已注销: list 不显示、prune 无事可做、
remove 报 not a working tree），直接删除该子目录即可
```

- 前置⑨ = `worktree 目录数已达保底门槛（当前 {n}，门槛 {t}）：{hint}；清理后再创建`。
- 补偿残留面 = `…补偿回收未完成，残留: {residuals}。{hint}；branch 侧仍以 git branch -D {branch} 清理`（`git branch -D` 保留——分支不受本缺陷影响）。
- 补偿成功面 = `…（worktree、分支 {branch} 与建档已补偿回收；{hint}）`。

**两个表面共同的不变量**：MUST NOT 出现 `请手动执行` / `请以 <git 单源> 对账并手动清理` 这两类「只给 git 一侧」的祈使句；`git worktree list` 字样与 `直接检查 … 落位根 … 下的子目录` 字样同时在场。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| worktree 回收单点（**修改**） | `remove_worktree(main_root, worktree)` 两段式：护栏 → `git worktree remove --force` → 失败落 OS 层删除 → `prune` → `Ok` / 两段皆失败 → `Err`（五段成因 + 双源） | `crates/infra/vcs/src/git.rs`（`remove_worktree` 体内重写 + 四私有助手 + 一常量） | `std::{fs, io, path}`、既有 `git()`；crate 依赖集不变 | 同步阻塞 IO + 子进程（既有形态，零 async / 零重试） |
| 归档缝文案收口（**修改**） | `ProcessArchiveVcs::remove_worktree` 退役自组 wrapper → 纯透传单点 | `crates/infra/vcs/src/lib.rs:184` | 既有 | trait impl 一行体 + doc |
| create 文案单点（**修改**） | 私有 `worktree_reconcile_hint` + 前置⑨ / 两处补偿改双源 + 两处私有函数补 `worktree_root` 形参 | `crates/core/workflow/src/write/create.rs` | 既有（`fs`） | 纯字符串装配（零 vcs 调用增量） |
| Cleanup 步（**doc 随动**） | doc 改写为两段式语义（失败面 / 成因来源 / 已注销孤儿回收） | `crates/core/orchestration/src/walker.rs:2764-2769` | 既有 | 零实现改动 |

<!-- 零新增 crate、零新增依赖边、零新增架构模型元素：VcsRuntime / Workflow / Orchestration 三元素文件面内部改动；.dev-team/architecture/** 零改动（VcsRuntime api = none，remove_worktree 为 pub(crate) 不构成公共面）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀。本清单只列**实现文件**；proposal「测试文件」表的用例新增 / 改造归测试相位（其文案锚点迁移清单见「验收标准落点」表末段）。

### 新增文件

**无**——本变更全部落点为既有文件内改动（四私有助手与一常量驻 `git.rs` 既有文件，`create.rs` 的 helper 驻既有文件）。

<!-- 新增文件子表为空：两段式助手是私有函数，随 remove_worktree 同址；不新建模块文件。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/vcs/src/git.rs` | ① 顶部加 `use std::io;`（`std::fs` 走全限定调用，不引入第二行 use）；② 新增常量 `WT_HOLDERS_HINT`；③ `remove_worktree` 体内重写为两段式（护栏 → git 段 → `delete_tree` → `prune` → `Ok` / 失败面 `Err`），签名逐字不变；④ 新增四私有助手 `is_protected_path` / `comparable_path` / `delete_tree` / `prune_worktrees`；⑤ doc 注释改写（两段式语义 + 「git 先拆登记再删文件 ⇒ 失败即永久孤儿」的实现理由 + 三消费点） | AC-1…AC-6 / AC-10；`git_impl` / `add_worktree` / `delete_branch` 等其余函数**逐字不动** |
| `crates/infra/vcs/src/lib.rs` | `ProcessArchiveVcs::remove_worktree`（`:184-194`）退役自组 wrapper → `git::remove_worktree(main_root, worktree)` 纯透传；doc 改写为「两段式与成因文案归单点（D8），本缝只透传」 | AC-6（文案单点透传）；`ProcessWorktree::remove_worktree`（`:98-100`）与 `ArchiveVcsPort` / `WorktreePort` 签名**逐字不动** |
| `crates/core/workflow/src/write/create.rs` | ① 新增私有 `worktree_reconcile_hint(worktree_root: &Path) -> String`（双源形态单点，置于补偿族小节开头）；② 前置⑨（`:194-213`）拒绝文案改为「当前目录数 + 门槛 + `worktree_root` 绝对路径 + 双源 hint + 清理后再创建」，判据与常量零改动；③ `compensate_add_failure`（`:382`）加形参 `worktree_root: &Path` 并改文案（残留面 → hint + branch 指引）；④ `compensate_tree_failure`（`:413`）加形参 `worktree_root: &Path`（保留既有 `worktree: &Path` 用于残留行路径），残留面与成功面均改 hint；⑤ 两处 call site（`:258-260` / `:268-270`）补传 `&worktree_root` | AC-7；`residual_note` / `compensate_record_delete` / 补偿链次序（`remove_worktree` → `delete_branch` → 删建档）/ `residuals` 拼形**逐字不动**；本文件零新增 port 调用 |
| `crates/core/orchestration/src/walker.rs` | `step_cleanup` doc（`:2764-2769`）改写：失败面 = 两段式皆失败、detail 携 vcs 单点成因（路径 + 原 git 错误 + OS 错误 + 占用成因清单 + 双源）、已注销孤儿经兜底回收后记「已移除」、`prune` 由单点负责 | AC-8（doc only）；执行体 `:2770-2807` **逐字不动**（`StepSettle::passed` 尽力语义、detail 前缀、branch 零触碰） |
| `packages/desktop/package.json` | `version`「落地时 `HEAD` 现值 + 1 patch」（现值 `0.6.42` → `0.6.43`） | 版本纪律（AC-10）；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行，MUST NOT 写死字面号；`dependencies` / `devDependencies` 零变化 |

### 公共函数 / API

> 本变更**零新增公共导出**（`git::remove_worktree` 已是 `pub(crate)`；四个新助手全 `fn` 私有）。下表为签名面**声明**：公开项签名逐字不变，私有项列出以钉实现形态（spec Module Contract 在册）。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `git::remove_worktree` | `crates/infra/vcs/src/git.rs` | 修改（体内两段式；签名不变） | `pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String>` | 唯一回收单点；`Ok` 语义统一为「磁盘已无该目录」（含缺席幂等）；`Err` 面收窄为「两段式皆失败」 |
| `git::is_protected_path` | 同上 | 新增（私有） | `fn is_protected_path(main_root: &Path, worktree: &Path) -> bool` | 护栏判据（DD1 四类）；`true` → 入口即 `Err` 且零副作用 |
| `git::comparable_path` | 同上 | 新增（私有） | `fn comparable_path(path: &Path) -> PathBuf` | 护栏归一（canonicalize 优先 → `std::path::absolute` → 原文；Windows 剥 `\\?\` + 小写） |
| `git::delete_tree` | 同上 | 新增（私有） | `fn delete_tree(path: &Path) -> io::Result<()>` | OS 层兜底删除（Windows 绝对路径经 `\\?\` 扩展前缀）；`NotFound` 由调用点解读为「不在场」 |
| `git::prune_worktrees` | 同上 | 新增（私有） | `fn prune_worktrees(main_root: &Path) -> Result<String, String>` | `git worktree prune`（stdout 原文返回供调用点丢弃）；仅在目标目录已不在场后调用；失败由调用点静默吞（DD5） |
| `ProcessArchiveVcs::remove_worktree` | `crates/infra/vcs/src/lib.rs` | 修改（自组 wrapper 退役） | `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` | `ArchiveVcsPort` impl；纯透传单点（文案单点收口） |
| `ProcessWorktree::remove_worktree` | `crates/infra/vcs/src/lib.rs` | **零改动**（声明） | `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` | 已为透传；两缝同实现同文案 |
| `workflow::write::create` | `crates/core/workflow/src/write/create.rs:115` | **零改动**（声明） | `pub fn create(main_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, project_roots: &[String], name: &str, goal: &str, title: &str, workflow_type: &str) -> Result<CreateOutcome, String>` | 只有前置⑨ 与两处补偿的**文案**变化（签名与实参序逐字不动） |
| `workflow::write::delete` | `crates/core/workflow/src/write/delete.rs` | **零改动**（声明） | `pub fn delete(main_root: &Path, layout: &Layout, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, id: &str, clean_worktree: bool) -> Result<DeleteOutcome, String>` | 经 port 受益（孤儿可回收、`Err` 收窄），实现零改动 |
| `worktree_reconcile_hint` | `crates/core/workflow/src/write/create.rs` | 新增（私有） | `fn worktree_reconcile_hint(worktree_root: &Path) -> String` | create 侧文案单点（前置⑨ + 两补偿残留 + 补偿成功确认） |
| `compensate_add_failure` | 同上 | 修改（加形参） | `fn compensate_add_failure(store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, main_root: &Path, id: &str, branch: &str, worktree_root: &Path, add_error: &str) -> String` | 新形参置于 `branch` 之后（与 `compensate_tree_failure` 同序）；文案改双源 |
| `compensate_tree_failure` | 同上 | 修改（加形参） | `fn compensate_tree_failure(store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, main_root: &Path, id: &str, branch: &str, worktree: &Path, worktree_root: &Path, fs_error: &str) -> String` | 新形参紧邻 `worktree`（两路径相邻便于对读）；文案改双源（残留面 + 成功面） |

### 类型定义

**无**——零新增 / 修改 `struct` / `enum` / `type` / trait：`Err` 形状逐字保持 `String`（D2），`WorktreePort` / `ArchiveVcsPort` 方法族零 diff，wire DTO 零演进。

<!-- 类型定义子表省略原因：本变更不引入任何公共类型，唯一「新事物」是四个私有函数与一个私有常量（列在「公共函数 / API」小节）。 -->

### 配置

**无**——零 `config.json` 键新增 / 删除 / 改义；`config.schema.json` 零改动（无 `KNOWN_KEYS` / `TestFramework` 变动）。前置⑧ / ⑨ 的 `ACTIVE_CHANGE_LIMIT`（5）与 `WORKTREE_DIR_SLACK`（2）为源码常量，**值不变**。

<!-- 配置子表省略原因：本变更为回收单点与文案，零配置面触及。 -->

---

## 数据模型

**零 schema 变更、零持久化变更。**

- `ChangeRecord.worktree`（`crates/infra/store/src/model.rs:619` 的记录字段，语义见 desktop-change-state-store）与生命周期**不变**：create 写入绝对路径、archive Cleanup 回收后**原样保留**（历史锚）、delete 面按该字段定位。本变更不读写该字段。
- 回收本身**不落库**（零 git 触点的归档双写红线不动，见 desktop-change-state-store「归档双写」）。
- 运行时无内存态模型（两段式是纯函数 + 子进程调用，无缓存、无注册表、无重试状态机）。
- 「已注销孤儿」不是数据形态而是磁盘事实：git 的登记（`<main>/.git/worktrees/<name>/`）与目录生死不一致。本变更的宇宙论是**以磁盘为回收判据、以 git 为快路径**（`Ok` 语义 = 「磁盘已无该目录」，与 delete / Cleanup 消费侧既有 `is_dir` 判据同源）。

---

<!-- 路由 / API 设计省略原因：零新 IPC 命令、零 DTO 字段演进、零 HTTP 端点；残留文案全部经既有 `Err` / `warnings` / step `detail` 出线（零前端改动、生成物零 diff）。 -->

## 依赖 (Dependencies)

**运行时**：零新增。`std::fs` / `std::io` / `std::path` 均为 std（`infra/vcs` 现有依赖为 `exec` / `foundation` / `orchestration` / `workflow`，本轮零增删；不需要 `store` 的 `\\?\` 剥除点或 `foundation::layout` 的路径面）。

**构建 / 测试**：零新增。既有 dev-dependencies（`tempfile`、`tokio`）与既有真实 git tempdir 装置（`git_test.rs` 的 `init_repo` / `worktree_slot`）足够；Windows 句柄装置用 `std::os::windows::fs::OpenOptionsExt`（std，零新依赖）。

---

## 验收标准落点

| AC | 实现落点 | 可观测面（判据形态） |
|----|----------|----------------------|
| AC-1 孤儿自愈 | `git.rs::remove_worktree` 第二段（`delete_tree` + `prune_worktrees`） | `Ok` + 目录消失 + `git worktree list` 无该行 + `<main>/.git/worktrees/` 无残留 admin；一次调用零重试（两段式是直线代码，无循环） |
| AC-2 原语义零漂移 | 第一段 `git worktree remove --force` 成功即 `Ok`（`git_impl` 与参数面逐字未改） | 既有四条用例零改动通过；主仓树 / branch / 同路径再 `add` 全在 |
| AC-3 幂等 | 第二段 `Err(NotFound)` 分支（DD2 单判据） | 缺席路径 → `Ok`；连续两次均 `Ok`；零删除动作、零 panic |
| AC-4 护栏 | 入口 `is_protected_path`（DD1 四类，`comparable_path` 归一） | 四类路径 → `Err` 含路径与拒绝因；主仓文件与目录全在（护栏早于 git 段，故连 git 登记面也未触碰） |
| AC-5 `prune` 不误伤 | `prune_worktrees` 单点调用（两支收敛之后）+ `prune` 语义本身只清「目录不存在」的 admin | 兜底分支执行后，另一活 worktree 仍在 `git worktree list` 且目录在场、其 admin 目录未被清除 |
| AC-6 失败面成因可辨 | 两段皆失败分支的五段单行文案（DD8）+ `lib.rs` 纯透传（DD4①） | `Err` 含绝对路径 / 原 git 错误 / OS 错误 / `WT_HOLDERS_HINT` / 双源短语；MUST NOT 含 `请手动执行`、`请以 git worktree list 对账并手动清理`（DD3：不锚裸命令串）；释放占用后重试 → `Ok` |
| AC-7 文案双源 | `create.rs::worktree_reconcile_hint` + 前置⑨ + 两补偿（DD4②） | 前置⑨ `Err` 含 `当前 8` / `门槛 7` / 落位根绝对路径 / `git worktree list` / `直接检查`；两补偿残留同形且不含单源祈使句；`git branch -D` 仍在场 |
| AC-8 尽力步不变 | `walker.rs` doc 随动（DD10） | 步恒 `passed`、detail 含「worktree 清理失败」+ 路径 + 成因原文、链 `Completed`、branch 零触碰 —— 全为既有结构与既有 detail 前缀 |
| AC-9 删除面收口 | `delete.rs` 零改动 + vcs 单点受益 | 孤儿 → `removed_worktree = true` 且链续走；两段皆失败 → 整体 `Err`、库记录与产物目录未动 |
| AC-10 零面漂移 | 本清单「零新增公共导出 / 零类型 / 零配置 / 零依赖」+ `package.json` 恰 +1 patch | 两 port 签名零 diff、`crates/infra/exec` 零 diff、生成物零 diff（零新命令）、`src-tauri/Cargo.toml` crate 版本行不动 |

**文案锚点迁移清单（登记给测试相位，本相位不改断言）**：

| 面 | 旧锚 | 新锚 |
|----|------|------|
| `lib_test.rs` `ProcessArchiveVcs::remove_worktree` `Err` | `git worktree remove --force {path}` 字面在场 | 改为「单点透传」判据：`Err` 含绝对路径 + 双源短语，且不含 `请手动执行`（缺席路径 / 普通目录两形态现已是 `Ok`，`Err` 面需在真占用装置下取得） |
| `git_test.rs` 缺席路径 `Err` | `expect_err`（两形态均 `Err`） | 形态一改判 `Ok`（幂等）；形态二（普通目录）改判「兜底删除成功 → `Ok`」，另置真占用形态承载 `Err` 面 |
| `create_test.rs` 前置⑨ | `git worktree list` 在场 | 保留该锚，追加 `直接检查` / 落位根绝对路径 |
| `create_test.rs` 两补偿 | `git worktree list` 在场 | 保留该锚，追加 `直接检查`，并断言不含 `请手动执行` / `git worktree remove --force` 祈使形态 |
| `walker_test.rs` Cleanup 失败 | 假件注入文案内嵌 `请手动执行 git worktree remove --force {path} 清理` | 假件注入改为新形态（路径 + 原 git 错误 + OS 错误 + 成因清单 + 双源），detail 断言同步；结构断言零改动 |

---

## 待决问题 (Open Questions)

1. **只读属性 / Windows 保留名 / ADS / 符号链接在 `remove_dir_all` 下的行为未实测**（explore 3.x 待核实项）。按 D9 的裁决**不额外自愈**：失败即 `Err`（携 OS 错误码，便于复现与后续变更补梯子）。若后续实测证明「只读」是常见形态，可在 `delete_tree` 内加「清属性 + 重试一次」的窄梯子（本变更不含）。
2. **POSIX 的 cwd 锁是否阻断 `remove_dir_all`**（R9）未实测。两段式本身平台无关（无 `cfg` 分叉除扩展前缀），POSIX 允许删被打开文件故兜底只会更易成功；若实测发现 cwd 锁在 Linux / macOS 上同样阻断，属平台差异而非逻辑分叉，无需改设计。
3. **`git 不可用`（PATH 缺失）时兜底删除会留下「登记在而目录亡」的 prunable admin**（DD9 的无例外分支）。当前裁决是「照兜底、`prune` 尽力」，环境引导由上游 `probe` 承担。是否需要在 `Err` / 日志里额外区分「git 不可达」与「git 命令失败」，留待后续观察（本变更零分支）。
