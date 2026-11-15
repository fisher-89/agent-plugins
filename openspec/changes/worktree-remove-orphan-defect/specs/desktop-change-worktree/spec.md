# desktop-change-worktree Specification (Delta)

## ADDED Requirements

### Requirement: worktree 回收两段式（git 注销失败即 OS 层兜底）

`crates/infra/vcs` 的 `remove_worktree(main_root, worktree)` SHALL 为 worktree 磁盘回收的唯一单点（create 树写出补偿、删除面「同时清空 worktree」、归档相位的 Cleanup 步三处消费；`WorktreePort` 与 `ArchiveVcsPort` 两缝 SHALL 委托同一实现、同一 `Err` 文案）。执行序 SHALL 为两段式：

1. `git worktree remove --force <path>` 成功 → `Ok`（既有语义零变化）。
2. 该调用失败 → 以 **OS 层删除**兜底（绕过 git 的登记语义）：目录已不在场 → `git worktree prune`（尽力）→ `Ok`；目录仍在场 → `fs::remove_dir_all`（Windows 经 `\\?\` 扩展前缀）→ 成功后 `git worktree prune`（清可能的残留 admin）→ `Ok`。
3. 兜底亦失败 → `Err`（见下「失败面」）。

该两段式的存在依据 SHALL 作为实现理由留痕：git 的删除语义是**先拆登记（admin 目录 + 登记表条目）、再删文件**（`--force` 与否一致），因此任何一次删除失败都留下「git 已注销」的孤儿目录——`git worktree list` 不显示、`prune` 无事可做、`worktree remove` 报 `not a working tree`。**不存在「失败即回滚、可安全重试」的路径**，故兜底是唯一的自愈手段。

约束：

- **幂等**：目录不在场 MUST 以 `Ok` 收口（MUST NOT 以 `not a working tree` 报错），使 create 补偿的「失败面零残留」与 delete / Cleanup 的重试链可达收敛。
- **护栏**：兜底 MUST NOT 施于 `main_root` 自身、`main_root` 的祖先、`main_root` 的子目录与文件系统根（命中即 `Err` 且磁盘零变化）。除此黑名单外 MUST NOT 以「是否 worktree 形态（目录内 `.git` 指针在场）/ 目录是否为空」作白名单——删除中途 abort 时 `.git` 常在 ③ 阶段已被先删，形态判据会在最需要自愈的窗口拒绝自愈。
- **`prune` 次序与安全**：`git worktree prune` SHALL 仅在目标目录已不在场之后调用（登记仍有效且目录在场的活 worktree MUST NOT 被波及）；`prune` 失败 SHALL 静默吞（目标目录已回收即主目标达成）。
- **零重试零退避**：兜底失败即 `Err`，MUST NOT 引入重试、退避或「优雅握手」（实测强杀后立即释放句柄，不存在需要等待的窗口；退避只会把持久占用伪装成偶发失败）。
- **降级留痕**：兜底成功而 git 段失败时，原 git 错误 SHALL 以 `[vcs]` 前缀的 stderr 日志留痕（沿 `commit_all` 既有先例），MUST NOT 借此扩 port 签名、`Result` 形状或 wire DTO。
- **失败面**：`Err` SHALL 携 worktree 绝对路径、原 git 错误、OS 错误与成因清单（终端 cwd / 编辑器打开的文件 / 常驻子进程 / 杀毒实时扫描 / 云同步客户端），MUST NOT 指引用户执行 `git worktree remove --force <path>`（对已注销孤儿恒报 `not a working tree`），MUST NOT 以 `git worktree list` 单源对账收尾。
- **双源对账**：本单点 `Err`、create 前置⑨ 保底门槛文案、create 两处补偿残留文案 SHALL 采用同一双源形态——「① `git worktree list` 对账；② 直接检查 worktree 落位根 `<git 根父目录>/.worktrees/<仓库目录名>/` 下的子目录（删除失败留下的残留不在 git 登记面，直接删除该子目录即可）」。MUST NOT 出现「以磁盘计数引导、以 git 对账收尾」的错配（计数源与对账源须为同一真相源）。branch 侧指引仍为 `git branch -D <branch>`（分支不受本缺陷影响）。
- **平台**：两段式对 Windows 与 POSIX 同构（除扩展路径前缀仅 Windows 生效外零 `cfg` 分叉）；POSIX 允许删除被打开的文件，兜底只会更容易成功。

#### Scenario: 已注销孤儿自愈回收

- **WHEN** 磁盘上存在一个 git 已注销的孤儿目录（登记与 admin 目录已拆、目录仍在场，`git worktree list` 不显示、`git worktree prune` 无输出），对该路径调用 `remove_worktree`
- **THEN** 返回 `Ok`；目录自磁盘消失；`git worktree list` 仍只列主仓；`<main>/.git/worktrees/` 无该 worktree 的残留 admin；全程一次调用、零重试

#### Scenario: 登记与目录一致时原语义零变化

- **WHEN** worktree 由 `add_worktree` 正常铸出（目录与登记一致）后调用 `remove_worktree`
- **THEN** 返回 `Ok`、目录移除、主仓工作树无损、branch `change/<name>` 留存、同路径可再 `add` 成功（既有补偿面语义逐字不变）

#### Scenario: 目录缺席幂等

- **WHEN** 对从未存在（或已被手动删除）的路径调用 `remove_worktree`
- **THEN** 返回 `Ok`（`prune` 尽力后收口），零删除动作、零 panic；重复调用仍 `Ok`

#### Scenario: 主仓护栏拒绝

- **WHEN** 以 `main_root` 自身、`main_root` 的祖先或其子目录调用 `remove_worktree`
- **THEN** 返回显式 `Err`（含路径与拒绝因），磁盘零变化（主仓文件与目录全在）

#### Scenario: prune 不误伤活 worktree

- **WHEN** 两个活 worktree 在案（登记与目录均在场），对其中一个执行两段式的兜底分支
- **THEN** 另一个仍在 `git worktree list` 中且目录在场，其 admin 目录未被清除

#### Scenario: 两段式皆失败携成因

- **WHEN** worktree 内某文件被不允许删除共享的句柄独占（Windows 语义：`share = READ` 不含 delete），两段式均失败
- **THEN** 返回 `Err`，文案含绝对路径、`git worktree remove` 失败原文、OS 错误与成因清单，且**不含**「`git worktree remove --force <path>`」单源指引；句柄释放后重试同一路径 → `Ok`（收敛）

#### Scenario: 零 port 与 wire 变更

- **WHEN** 审查 `WorktreePort` / `ArchiveVcsPort` 的方法签名与 IPC DTO
- **THEN** 二者方法族与 `Result<(), String>` 形状逐字不变，两缝委托同一实现；无新命令、无 DTO 字段演进、生成物零 diff

## MODIFIED Requirements

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **branch 清理手动（删除面已偿还）**：worktree 由 archive 相位 Cleanup 步自动移除（两段式回收尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」与本 spec「worktree 回收两段式」）；归档路径下 branch `change/<name>` 仍保留（历史可溯）。需要彻底清理时走清单删除入口并勾选「同时清空 worktree」——该路径同时删除 worktree 目录与 branch（见 desktop-change-delete）。
2. **legacy 主 root 路径**：`worktree=None` 存量记录照旧主 root 执行，零迁移；非 git 仓 workspace 的 create 显式拒绝（不提供 legacy 创建逃生口，真实诉求另立变更）。
3. **`agent_start` 与探索笔记留主仓 root**：exec 轨道手动会话与 `openspec/explores/` 笔记不混入 change worktree。
4. **config / AGENT.md 冻结**：worktree 内配置停在创建时刻（base commit 检出），主仓中途改动不传导，文档化；唯一例外是 create 在创建时读取**主 workspace root** 的当前配置以选择 `projects[].root`（依赖引导锚点），worktree 内配置副本仍冻结。
5. **构建成本接受**：fresh worktree 依赖 / 构建产物全量重建；共享 target 目录类逃生口（`CARGO_TARGET_DIR` 等）仅文档化，不内建机制。
6. **孤儿 worktree 对账（两源）**：create 中途崩溃遗留的 worktree / branch、以及删除失败留下的已注销孤儿，SHALL 以**两源**对账——① `git worktree list`；② 直接检查 worktree 落位根 `<git 根父目录>/.worktrees/<仓库目录名>/` 下的子目录。已注销孤儿不在 git 登记面（`list` 不显示、`prune` 无输出、`remove` 报 `not a working tree`），单靠 git 对账无法定位，故面向用户的残留指引 MUST NOT 只给 git 一侧。有建档记录的残留经清单删除入口勾选「同时清空 worktree」清理（desktop-change-delete——删除面按记录定位，故只覆盖有建档记录的残留），且回收单点自带 OS 层兜底、一次失败不再等于终态（见「worktree 回收两段式」）；无记录的孤儿仍须手动删除其目录。create 前置校验保证下次创建显式报错而非误用；create 前置 worktree 磁盘保底门槛为失控防线（见 desktop-change-create）。
7. **report_dir 绝对路径接受失效**：phase_log 报告文本内嵌的 worktree 绝对路径在 worktree 删除后失效（转录历史，仅展示面），不做相对化；归档 Cleanup 步使删除成为缺省路径，本边界维持。
8. **并行 merge 冲突处置分层**：两 change 改同一文件各自绿、merge 撞时——经 archive 相位自动合入的冲突由 Rebase / ResolveConflicts 步的解冲突 agent 裁决（agent 无法裁决时 lean 档停链转人工，见 desktop-change-archive）；链外用户手动 merge 的冲突走 git 原生冲突处理。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 残留指引双源

- **WHEN** 审查三处面向用户的 worktree 残留指引（`remove_worktree` 的 `Err`、create 前置⑨ 门槛文案、create 补偿残留文案）
- **THEN** 三者均同时给出「`git worktree list` 对账」与「直接检查 `.worktrees/<仓库目录名>/` 下的子目录」两源，且均不含「执行 `git worktree remove --force <path>` 清理」单源指引

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/infra/vcs/src/git.rs`（`remove_worktree`，改） | worktree 回收单点 | `pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String>`：签名不变，体内改两段式（`git worktree remove --force` → 失败按目录在场分流 → OS 层兜底 → `prune` → `Ok`）；`Err` 携路径 / git 原文 / OS 错误 / 成因清单 / 双源指引；兜底成功时 `eprintln!("[vcs] …")` 留痕原 git 错误 |
| `crates/infra/vcs/src/git.rs`（`prune_worktrees`，新私有） | 残留 admin 清理 | `git worktree prune`；仅目标目录不在场后调用；失败静默吞（`Result` 由调用点择取，不外泄为用户面错误） |
| `crates/infra/vcs/src/git.rs`（`delete_tree`，新私有） | OS 层兜底删除 | `fs::remove_dir_all`；Windows 对绝对路径加 `\\?\` 扩展前缀（超长路径面），POSIX 直删；零重试零退避 |
| `crates/infra/vcs/src/git.rs`（`is_protected_path`，新私有） | 兜底护栏 | path == `main_root` / 祖先 / 子目录 / 文件系统根 → 拒绝；命中即 `Err` 且零删除 |
| `crates/infra/vcs/src/lib.rs`（`ProcessArchiveVcs::remove_worktree`，改） | 归档缝透传 | 取消自组「请手动执行 `git worktree remove --force {path}` 清理」wrapper，纯透传单点 `Err`（文案单点收口，防两缝漂移）；`ArchiveVcsPort` 签名不变 |
| `crates/infra/vcs/src/lib.rs`（`ProcessWorktree::remove_worktree`） | create / delete 缝 | 逐字不变（已为透传）；两缝同实现同文案 |
| `crates/infra/vcs/src/git_test.rs` / `lib_test.rs` | 回收语义覆盖 | 孤儿回收 / 缺席幂等 / 主仓护栏 / `prune` 不误伤 / 两段式皆失败携成因（Windows 句柄装置）/ 单点文案透传 |
| `packages/desktop/package.json` | 版本交付 | 落地时 `HEAD` 现值 + 1 patch（现值 0.6.42 → 0.6.43）；`src-tauri/Cargo.toml` 不随动 |
