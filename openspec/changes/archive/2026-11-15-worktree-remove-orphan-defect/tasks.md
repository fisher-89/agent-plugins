# 任务: worktree-remove-orphan-defect

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 5 项（4 个 Rust 文件 + 1 个版本行）：`crates/infra/vcs/src/git.rs`（两段式主体）、`crates/infra/vcs/src/lib.rs`（归档缝文案收口）、`crates/core/workflow/src/write/create.rs`（双源文案）、`crates/core/orchestration/src/walker.rs`（doc 随动）、`packages/desktop/package.json`（版本）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：proposal「测试文件」表逐行的新增 / 改造用例归 test-design / test-gen / test-execution 相位（design「验收标准落点」末段已给出**锚点迁移清单**，供测试相位直接取用）。本相位对测试文件的唯一容忍是「不触碰」。
>
> **依赖序（五阶段）**：
>
> - **阶段一**（vcs 两段式单点：护栏 + OS 层兜底 + `prune` + 失败面文案）是**底座**——它决定 `Ok` / `Err` 的语义面，后续所有阶段只是在消费它（归档缝透传、create 文案、walker doc 的措辞都以「两段式」为准）。
> - **阶段二**（归档缝 `ProcessArchiveVcs::remove_worktree` 纯透传）依赖阶段一的 `Err` 文案；它是「文案单点收口」的关闭动作（两缝同文案）。
> - **阶段三**（create 双源文案）与阶段二互相独立（不同 crate 面），但两者的措辞必须与阶段一的 vcs `Err` 同形（spec：四处消费同一双源形态）；先做阶段一可避免措辞二次返工。
> - **阶段四**（walker doc）依赖阶段一、二（doc 要写准「失败面 = 两段式皆失败、成因由单点提供」）。
> - **阶段五**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **逐字不动面**（`git diff` 出现即越界）：`WorktreePort::remove_worktree` / `ArchiveVcsPort::remove_worktree` 的签名与 `Result<(), String>` 形状；`ProcessWorktree::remove_worktree`（`lib.rs:98-100`）；`walker.rs::step_cleanup` 执行体（`StepSettle::passed` 尽力语义 / detail 前缀 / branch 零触碰）；`delete.rs` 全文件；`archive.rs` 全文件；`crates/infra/exec/**`；`crates/core/foundation/**`；`src-tauri/src/**`；`packages/desktop/src/**`。
> ① **零重试零退避零握手**：两段式是直线代码（`match` + 一次性 `remove_dir_all`），MUST NOT 引入循环 / `sleep` / 退避 / 「优雅关停占用者」。
> ② **护栏先于副作用**：`is_protected_path` 是 `remove_worktree` 的**第一句**（早于 git 段），命中即 `Err` 且磁盘零变化。
> ③ **文案单点两处**：vcs 侧 = `git::remove_worktree` 的 `Err`（delete + Cleanup 共用）；create 侧 = `worktree_reconcile_hint`（前置⑨ + 两补偿残留 + 补偿成功确认共用）。跨 crate MUST NOT 互相引用，只在形态上对齐。
> ④ **祈使短语缺席**：实现文件内 MUST NOT 出现 `请手动执行` 与 `请以 … 对账并手动清理` 两类单源祈使句；`git worktree list` 字样与「直接检查 … 落位根 … 下的子目录」须同时在场（design DD3 / DD4）。
> ⑤ **门禁分工**：整链 `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与测试回归由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。自查项只收源码级静态判据（`grep` 零命中、结构复核、`git diff` 边界比对）；「零新命令 ⇒ 生成物零 diff」是零动作的自动结果。
> ⑥ **版本纪律**：`packages/desktop/package.json` 的 `version` 按「落地时 `HEAD` 现值 + 1 patch」相对递增（现值 `0.6.42` → 落地写 `0.6.43`，MUST NOT 写死字面号作判据）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

## 阶段一：vcs 回收单点两段式（`crates/infra/vcs/src/git.rs`）

- [x] `git.rs` 顶部 use 面：在 `use std::io::Write as _;` 之后加 `use std::io;`（`std::fs` 走全限定调用 `std::fs::remove_dir_all`，不引入第二行 use；`PathBuf` 已在既有 `use std::path::{Path, PathBuf};` 内）。
- [x] 在 `remove_worktree` 上方新增失败面常量（固定短语、零插值）：
  ```rust
  /// worktree 回收两段式皆失败时的常见占用源（固定短语——失败面是「目录树里某个
  /// 未知文件被锁」，目录规模下逐文件点名不可行，故给可执行的排除动作而非假精确
  /// 的占用者）。
  const WT_HOLDERS_HINT: &str = "终端 cwd（终端或子进程在 worktree 内）、\
      编辑器打开的文件、常驻子进程（dev server / watcher）、杀毒实时扫描、云同步客户端";
  ```
- [x] 新增私有护栏归一助手（`is_protected_path` 之前）：
  ```rust
  /// 护栏用的路径可比形态：能 canonicalize 则取其（存在路径的唯一真相，同时消解
  /// 符号链接与 Windows 大小写差异），失败退 `std::path::absolute`（缺席路径——
  /// 此时删除本就无对象，误判无害），再剥 Windows 扩展前缀并在 Windows 小写化。
  fn comparable_path(path: &Path) -> PathBuf { /* canonicalize → absolute → 原文；剥 `\\?\`；cfg(windows) 小写 */ }
  ```
  Windows 小写化 = `strip_prefix(r"\\?\")` 后 `to_lowercase()` 转 `PathBuf`；POSIX 只剥前缀（实际不存在）后原样。
- [x] 新增私有护栏（DD1 四类，`true` → 调用点入口 `Err`）：
  ```rust
  /// 兜底护栏：拒绝回收主仓自身、其祖先、其子目录与文件系统根（空路径同拒）。
  /// 「子目录」判据不会误伤合法 worktree——落位根 `<git 根父目录>/` + `.worktrees/`
  /// 与 git 根**同级**，而主仓（= git 根或其子目录）不包含 `.worktrees` 子树。
  fn is_protected_path(main_root: &Path, worktree: &Path) -> bool {
      if worktree.as_os_str().is_empty() {
          return true;
      }
      let main = comparable_path(main_root);
      let target = comparable_path(worktree);
      // path == main_root 被 main.starts_with(&target) 包含
      target.parent().is_none() || main.starts_with(&target) || target.starts_with(&main)
  }
  ```
- [x] 新增私有 OS 层兜底删除（DD6）：
  ```rust
  /// OS 层兜底删除（绕过 git 登记语义）：Windows 对**绝对路径**包一层 `\\?\`
  /// 扩展前缀（>260 长路径），已带前缀或相对路径直删；POSIX 直删。零重试、零属性
  /// 清理——只读 / 保留名 / ADS / 符号链接目标失败即 Err（诚实上报，见 D9）。
  fn delete_tree(path: &Path) -> io::Result<()> {
      #[cfg(windows)]
      {
          if path.is_absolute() {
              let text = path.to_string_lossy();
              if !text.starts_with(r"\\?\") {
                  return std::fs::remove_dir_all(PathBuf::from(format!(r"\\?\{}", text)));
              }
          }
      }
      std::fs::remove_dir_all(path)
  }
  ```
- [x] 新增私有残留 admin 清理（DD5）：
  ```rust
  /// `git worktree prune`（清「目录已不存在」的残留 admin）：**仅在目标目录已不
  /// 在场之后**调用——登记仍有效且目录在场的活 worktree 不受波及。失败由调用点
  /// 静默吞（目录已回收即主目标达成）。
  fn prune_worktrees(main_root: &Path) -> Result<String, String> {
      git(main_root, &["worktree", "prune"])
  }
  ```
- [x] 重写 `remove_worktree` 体内（签名 `pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String>` 逐字不变）：
  ```rust
  if is_protected_path(main_root, worktree) {
      return Err(format!(
          "拒绝回收 worktree {}：该路径为主仓 {} 自身、其祖先、其子目录或文件系统根\
           （护栏，磁盘零变化）",
          worktree.display(),
          main_root.display()
      ));
  }
  let git_error = match git(main_root, &["worktree", "remove", "--force", &worktree.to_string_lossy()]) {
      Ok(_) => return Ok(()),
      Err(error) => error,
  };
  // git 段的删除语义是「先拆登记（admin + 登记表）再删文件」，故该段失败即
  // 「已注销」——第二段只看目录生死，MUST NOT 依 git 状态判失败
  let root_hint = worktree.parent().map_or_else(
      || "（未知：worktree 路径无父目录）".to_owned(),
      |parent| parent.display().to_string(),
  );
  match delete_tree(worktree) {
      Ok(()) => eprintln!(
          "[vcs] git worktree remove 失败，已以 OS 层兜底回收 {}: {git_error}",
          worktree.display()
      ),
      Err(error) if error.kind() == io::ErrorKind::NotFound => eprintln!(
          "[vcs] git worktree remove 失败且目录不在场（按幂等成功收口）{}: {git_error}",
          worktree.display()
      ),
      Err(io_error) => {
          return Err(format!(
              "worktree 回收失败（{}）：git 与 OS 层删除均失败；原 git 错误: {git_error}；\
               OS 删除错误: {io_error}；常见占用源: {WT_HOLDERS_HINT}——释放占用后重试即收敛\
               （强杀占用进程后立即释放，无需等待）；残留对账须双源: ① git worktree list；\
               ② 直接检查 worktree 落位根 {root_hint} 下的子目录——删除失败留下的残留不在\
               git 登记面（git 已注销: list 不显示、prune 无事可做、remove 报 not a working tree），\
               直接删除该子目录即可",
              worktree.display()
          ));
      }
  }
  let _ = prune_worktrees(main_root);
  Ok(())
  ```
  要点：全部文案**单行**（零 `\n`，DD8）；`MUST NOT` 在此出现 `请手动执行` / `请以 … 对账并手动清理` 祈使句（DD3）。
- [x] 改写 `remove_worktree` 的 doc（覆盖 spec 要求留痕的四件事）：① 两段式执行序与「`Ok` = 磁盘已无该目录」；② 存在理由（git 先拆登记再删文件 ⇒ 任何一次失败都是「已注销孤儿」，`list` 不显示 / `prune` 无事可做 / `remove` 报 `not a working tree`，不存在「失败即回滚可安全重试」的路径）；③ 三消费点（create 补偿 / delete「同时清空 worktree」/ 归档 Cleanup）；④ 失败面构成与双源指引；⑤ `prune` 次序约束与零重试纪律；⑥ `[vcs]` 降级留痕先例（`commit_all`）。

## 阶段二：归档缝文案收口（`crates/infra/vcs/src/lib.rs`）

- [x] `ProcessArchiveVcs::remove_worktree`（`:184-194`）替换为纯透传，doc 改写：
  ```rust
  fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String> {
      // 文案单点收口：两段式（git 段失败即以 OS 层兜底回收 + prune）与失败面成因
      // 由 `git::remove_worktree` 提供，本缝只透传——MUST NOT 在此追加任何手动命令
      // 指引（与 create / delete 缝同一实现同一文案）
      git::remove_worktree(main_root, worktree)
  }
  ```
  原自组文案 `worktree 移除失败（{path}）: {error}；请手动执行 git worktree remove --force {path} 清理` **整段退役**（`git diff` 出现该字面即未完成）。
- [x] 复核 `ProcessWorktree::remove_worktree`（`:98-100`）**逐字未动**（两缝同实现同文案，spec Module Contract 在册）；复核同文件其余方法零改动。

## 阶段三：create 侧双源文案（`crates/core/workflow/src/write/create.rs`）

- [x] 新增私有文案单点（建议置于 `compensate_add_failure` 之前的补偿族小节开头）：
  ```rust
  /// worktree 残留对账引导（前置⑨ 与两处补偿共用单点，DD4②）：计数源 / 回收失败
  /// 留下的残留都不在 git 登记面（git 已注销：list 不显示、prune 无事可做、remove
  /// 报 not a working tree），故单源指引会让用户无从自救——MUST 双源。
  fn worktree_reconcile_hint(worktree_root: &Path) -> String {
      format!(
          "worktree 残留对账须双源: ① git worktree list；② 直接检查 worktree 落位根 {} 下的\
           子目录——删除失败留下的残留不在 git 登记面（git 已注销: list 不显示、prune 无事\
           可做、remove 报 not a working tree），直接删除该子目录即可",
          worktree_root.display()
      )
  }
  ```
- [x] 前置⑨ 拒绝文案（`:209-211`）改为「目录数 + 门槛 + 落位根绝对路径 + 双源 hint + 动作」：
  ```rust
  return Err(format!(
      "worktree 目录数已达保底门槛（当前 {worktree_dirs}，门槛 {worktree_threshold}）：{}；清理后再创建",
      worktree_reconcile_hint(&worktree_root)
  ));
  ```
  判据（`fs::read_dir` 计数 / `NotFound` 计 0 / 其余读错误显式 `Err`）与 `ACTIVE_CHANGE_LIMIT` / `WORKTREE_DIR_SLACK` **逐字不动**；本改动零新增 port 调用。
- [x] `compensate_add_failure`（`:382`）：签名加 `worktree_root: &Path`（置于 `branch` 之后，与 `compensate_tree_failure` 同序）；双故障角落文案改为
  ```rust
  format!(
      "创建 change 失败: worktree 建域失败: {add_error}；补偿回收未完成，残留: {residuals}。\
       {}；branch 侧仍以 git branch -D {branch} 清理",
      worktree_reconcile_hint(worktree_root)
  )
  ```
  `(None, None)` 支（建档与分支已补偿回收）文案**零改动**；补偿链次序与 `residuals` 拼形零改动。
- [x] `compensate_tree_failure`（`:413`）：签名加 `worktree_root: &Path`（紧邻既有 `worktree: &Path` 之后）；两处文案改为
  - 残留面 → `format!("创建 change 失败: {fs_error}；补偿回收未完成，残留: {}。{}；branch 侧仍以 git branch -D {branch} 清理", residual_note(residuals.into_iter()), worktree_reconcile_hint(worktree_root))`
  - 成功面 → `format!("创建 change 失败: {fs_error}（worktree、分支 {branch} 与建档已补偿回收；{}）", worktree_reconcile_hint(worktree_root))`
  `remove_worktree` → `delete_branch` → 删建档 的次序、`worktree_note` 残留行、`residuals.is_empty()` 判据**逐字不动**。
- [x] 两处 call site（`:258-260`、`:268-270`）补传 `&worktree_root`；确认 `worktree_root`（`:193` 已 derived）在两处均在作用域内，**不新增** `vcs.worktree_root` 调用（门槛道已派生一次，vcs 调用计数断言面不变）。
- [x] 自查：文件内 `git worktree remove --force` 字面**零命中**；`请手动执行` **零命中**；`git worktree list` 与 `直接检查` 均在 `worktree_reconcile_hint` 内（四处消费点经该单点自动满足）。

## 阶段四：Cleanup 步 doc 随动（`crates/core/orchestration/src/walker.rs`）

- [x] 改写 `step_cleanup` doc（`:2764-2769`），保留既有事实（SpecSync 与 Seal 之间、`--force` 带走 bootstrap 产物、legacy / 缺席幂等跳过、尽力步步恒 `passed`、branch 全程零触碰、缺 preflight 载荷走 Terminal），新增三句：① 回收经两段式单点（已注销孤儿亦可回收；`prune` 由单点负责，本步零 vcs 状态机）；② 失败面收窄为「两段式皆失败」（git 与 OS 层删除均失败）；③ `detail` 必携成因——即 `ArchiveVcsPort` 缝透出的 `Err` 原文（路径 + 原 git 错误 + OS 错误 + 占用成因清单 + 双源对账），MUST NOT 只呈原始 git 错误串、MUST NOT 以 `git worktree list` 单源对账收尾。
- [x] 复核执行体 `:2770-2807` **逐字未动**（`StepSettle::failed(Some("缺归档前置载荷"))` 计划漂移面、`TXT_SKIP_LEGACY` / `TXT_SKIP_ABSENT` 两支、`Ok` → 「已移除 worktree（<path>）」、`Err` → 「worktree 清理失败（尽力步，不影响归档收口）: {error}」、`close_step` 收口次序）。

## 阶段五：版本与边界自查

- [x] `packages/desktop/package.json`：`version` 写「落地时 `HEAD` 现值 + 1 patch」（现值 `0.6.42` → `0.6.43`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行，MUST NOT 写死字面号）；`dependencies` / `devDependencies` 零变化。
- [x] 零漂移自查（`git diff` 边界比对，逐条读 diff 非空即越界）：`crates/core/workflow/src/write/worktree.rs`、`crates/core/orchestration/src/port.rs`、`crates/infra/exec/**`、`crates/infra/checks/**`、`crates/core/workflow/src/write/delete.rs`、`crates/core/workflow/src/write/archive.rs`、`crates/core/foundation/**`、`src-tauri/**`、`packages/desktop/src/**`、`src-tauri/Cargo.toml` 的 crate 版本行。
- [x] 文案不变量自查（源码级 `grep`）：`请手动执行` 与 `请以 .* 对账并手动清理` 在四个实现文件内零命中；`WT_HOLDERS_HINT` 仅 `git.rs` 引用；双源短语在 vcs `Err` 与 `worktree_reconcile_hint` 各一处，无第二副本。
- [x] 生成物零 diff 判据确认：本变更零新 IPC 命令、零 DTO 演进 ⇒ `src/types/generated/**` 与 `.dev-team/architecture/**` 应零 diff（不执行 `bindings:export`，由 static-check 相位整链验证）。
- [x] 交接备忘写入实现提交说明：测试相位需按 design「验收标准落点 - 文案锚点迁移清单」改造既有断言（proposal 测试表第 3/4/9/10/11 行的 `git_test` 缺席与普通目录 / `create_test` 前置⑨ 与两补偿 / `walker_test` 假件注入文案），重写 `lib_test` 的归档缝 `Err` 用例（第 8 行——缺席与普通目录两形态已不再产生 `Err`），并按同表新增 6 条用例（第 1/5/6/7/8/12 行：孤儿自愈 / 护栏三形态 / `prune` 不误伤 / 两段皆失败携成因 + Windows 句柄装置 / 单点透传 / 删除面孤儿回收）。
