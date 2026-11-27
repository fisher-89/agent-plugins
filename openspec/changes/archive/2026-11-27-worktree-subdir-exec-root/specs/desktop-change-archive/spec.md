# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: worktree 提交与主仓合入

**锚点分工（本变更增补）**：归档链的 git 步（`Commit` / `Rebase` / `ResolveConflicts` / `Merge`）SHALL 锚 **host**（`ChangeRecord.worktree` 即 git 落位目录；`PhaseCursor.preflight.worktree` 承载该列），执行根 MUST NOT 参与任何 git 步——`commit_all` 的 `add -A`、`dirty` 的 `status` 都是全树语义，而 `git worktree remove` / `rebase` 认的是落位目录，传执行根会得「只提交 / 只回收子目录、登记与整树留下」的静默残缺。前置载荷的**产物与 delta specs 探测** SHALL 锚 **执行根**（`PhaseCursor.preflight.execRoot`；`execRoot = None` 时回落 `worktree`）——workspace root 为仓库子目录时，change 目录在 `host/<rel>/openspec/changes/<name>/`，锚 host 会恒判「产物全缺」。双列 MUST NOT 互相顶替。

带 worktree 记录的 change，archive 相位 SHALL 经四步完成 worktree 提交与主仓合入：`Commit` 步在 worktree 内提交本 change 编辑集（git add 范围 = worktree 全域——worktree 为 change 私有执行锚，全域即本 change 编辑集；提交信息派生规则由 design 定稿），worktree 脏即提交、干净则跳过，worktree 目录缺失且分支已合入则跳过；`Rebase` 步将 branch `change/<name>` 以 `rebase --empty=drop` 重放至主仓当前分支之上（重放目标 = 主仓当前分支；已在目标中的重放提交显式丢弃），重放冲突时 SHALL 保留冲突态于 worktree（MUST NOT 自动 `rebase --abort`），冲突判据为 rebase 非零退出且冲突清单非空（`--diff-filter=U` 清单），命中时产出冲突清单入 `PhaseCursor.rebase_conflicts`；`ResolveConflicts` 步 SHALL 条件执行——无冲突 `passed` + `detail` 记「无冲突」；有冲突时唤起一个解冲突 agent 会话（cwd = worktree、bypassPermissions、经既有 WorkerAgent 通道），agent SHALL 仅编辑冲突清单内文件并以 pathspec 纪律收口（只 add 冲突清单，禁止 `add -A`），rebase 收口动词（`rebase --continue` / `rebase --abort`）由 walker 代行、agent 无收口权；`Merge` 步 SHALL 在主仓以 `merge --ff-only` 纯快进收口——线性历史零 merge commit。walker SHALL 对解冲突 agent 结果后验（REBASE_HEAD 在场 + 冲突标记复查 + 冲突清单外无新改动）：后验通过则 `rebase --continue` 续走、重放完成后快进；后验不通过（agent 失败 / 被停止 / 残留冲突 / REBASE_HEAD 缺席 / 清单外新改动）SHALL 以 lean 档收口（`rebase --abort` 恢复 worktree 干净态 + 步失败 `Terminal` + 冲突摘要与手动裁决引导）；rich 档留后续变更。非冲突失败（rebase 非零退出且冲突清单为空）SHALL 尽力 `rebase --abort` 后 `Terminal`（携带 git 语境与手动处置引导）。全程 MUST NOT 强制推送、MUST NOT 改写主仓历史；主仓无关未提交改动与无关 staged 条目 MUST NOT 被任何自动提交或解冲突 agent 吞并（主仓全程零冲突态——冲突面囚于 worktree）。legacy 记录（`worktree=None`）SHALL 跳过 Commit / Rebase / ResolveConflicts / Merge 四步（主仓工作区编辑的提交仍为用户手动面，语义见 desktop-change-worktree「V1 范围与边界留痕」）。

**模型冲突不进 agent 解算（本期增补）**：worktree 模型（`<archi_dir>/` 下任何路径）与主仓冲突时，`ResolveConflicts` SHALL 在任何 agent 会话发起之前按**路径分流** `PhaseCursor.rebase_conflicts`：

```
pending = cursor.rebase_conflicts
   │
   ├─ 含 <archi_dir>/ 下任何路径 → Terminal::failed（零 agent 会话、不 abort）
   │     文案：模型冲突需人工裁决 + worktree 绝对路径 + 「解完模型冲突后重跑归档」
   │
   └─ 全部为代码路径 → 既有 agent 解算流程（逐字零改动）
```

- **判据单点**：路径归属 SHALL 经 `foundation::layout::is_archi_rel_path` 判定（`/` 与 `\` 分隔均归一），MUST NOT 在 core 侧比较 `architecture` 字面量。
- **不 abort**：该臂 MUST NOT 调用 `rebase --abort`——rebase 半程 SHALL 保留给人工处置（既有「冲突态保留于 worktree」语义不破）；由此产生的重入风险由 `Preflight` 的半程挡回兜住（见「归档编排链与阶段幂等」）。
- **理由**：模型是设计权威，agent 自动解算等于让 LLM 替人做架构决策（"哪个模块该依赖哪个"），解错无人能察。
- **混合冲突**：同一冲突清单同时含模型路径与代码路径时 SHALL 走模型臂（整体交人），MUST NOT 先解代码再解模型的半自治形态。
- 该分流 MUST NOT 改变既有代码冲突臂的任何可观察行为（后验三查 / pathspec 纪律 / lean 与 rich 分档 / 非冲突失败处理逐字不变）。

#### Scenario: 提交合入成功

- **WHEN** 对 dirty worktree 的 change 走 archive 相位至 Merge 步成功
- **THEN** Commit 步产生一个包含本 change 编辑集的提交（零 spec 同步产物——SpecSync 在 Merge 之后），Rebase 重放完成后主仓 ff-only 快进，`openspec/changes/<n>/` 目录在场且主仓 HEAD 前移（零 merge commit）

#### Scenario: 子目录 workspace 的提交与产物探测

- **WHEN** 子目录 workspace（`rel = packages/desktop`）的 worktree change 走 archive 相位至 Merge 步成功
- **THEN** Commit 步在 `host` 全域提交（`add -A` 覆盖整棵检出，含执行根子树之外的改动）、Rebase / Merge 以 `host` 的 branch 收口；Preflight 的 `missingArtifacts` / `deltaSpecs` 以执行根 `host/packages/desktop` 解析（不出现「产物全缺」的假阴性）；主仓出现 `packages/desktop/openspec/changes/<name>/`

#### Scenario: 冲突 agent 解冲突续链

- **WHEN** 分支重放与主仓当前分支对同一文件有不兼容修改时 Rebase / ResolveConflicts 步执行
- **THEN** 冲突态保留于 worktree（零 abort），解冲突 agent 会话发起（cwd = worktree、provenance 在案）；agent 仅编辑冲突清单内文件并以清单 pathspec 收口，后验通过（REBASE_HEAD 在场、零残留标记、清单外零新改动）后 walker 代 `rebase --continue` 收口并快进；主仓无关改动保持归档前状态

#### Scenario: 无法裁决 lean 停链

- **WHEN** 解冲突 agent 失败 / 被停止 / 后验发现残留冲突、REBASE_HEAD 缺席或冲突清单外新改动
- **THEN** walker 执行 `rebase --abort` 收口（worktree 恢复重放前干净态、主仓无任何合入态），该步 `Terminal` 呈现冲突摘要与手动裁决引导；db 零变化；用户手动处置后重发 run，已合入跳过、续走至收口

#### Scenario: 非冲突失败显式引导

- **WHEN** rebase 非零退出且冲突清单为空（worktree git 状态不允许等）
- **THEN** walker 尽力 `rebase --abort` 后 `Terminal` 呈现 git 语境与手动处置引导，零解冲突 agent 会话，db 零变化

#### Scenario: 主仓无关改动不吞并

- **WHEN** 主仓预先存在无关未提交文件与无关 staged 条目，archive 相位全程收口（含解冲突 agent 路径）
- **THEN** 两者保持归档前的 git 状态（未被吞并进任何自动提交或 agent 编辑；主仓全程零冲突态，冲突面囚于 worktree），可机械验证

#### Scenario: legacy 跳过 git 步

- **WHEN** 对 `worktree=None` 的存量建档 change 进入 archive 相位
- **THEN** Commit / Rebase / ResolveConflicts / Merge 四步零动作，链径直经（可选 SpecSync 后）Seal / Finalize 收口

#### Scenario: 模型冲突不进 agent 解算

- **WHEN** rebase 冲突清单含 `.dev-team/architecture/graph/model/02-auth.toml`（单条或与代码路径混合）
- **THEN** `ResolveConflicts` 步以 `Terminal::failed` 收口：零解冲突 agent 会话（run 节点无会话槽位）、零 `rebase --abort` 调用、rebase 半程保留于 worktree；文案含 worktree 绝对路径与「解完模型冲突后重跑归档」引导

#### Scenario: 纯代码冲突路径零回归

- **WHEN** 冲突清单全部为代码路径（无架构域路径）
- **THEN** 行为与本次变更前逐字一致（唤起解冲突 agent、仅编辑清单内文件、pathspec 收口、后验三查、成功续走 / lean 收口两臂均不变）

### Requirement: Cleanup 步与 worktree 自动清理

archive 相位 SHALL 在 SpecSync 步之后、Seal 步之前执行 `Cleanup` 步，对带 worktree 记录（`ChangeRecord.worktree = Some`）的 change 经 `crates/infra/vcs` 的 worktree 回收单点移除该 worktree（**回收锚 = 记录 `worktree`（host）**；MUST NOT 传执行根——`git worktree remove <exec_root>` 只会删掉子目录并把整树与登记留下，是静默残缺）（两段式：`git worktree remove --force` 失败即以 OS 层兜底回收并 `prune` 残留 admin，见 desktop-change-worktree「worktree 回收两段式」；`--force` 带走 bootstrap 的 `node_modules` / `target` 残留）；branch `change/<name>` MUST NOT 删除（回收本就不动分支，历史可溯）。Cleanup SHALL 为**尽力步**：清理失败 SHALL 仅以 step detail 警告呈现，步状态 `passed`、链照常收口（Seal / Finalize / PhaseLog 续走——归档必收口，残留是警告不是回滚），MUST NOT 以 `Terminal` 停链、MUST NOT 扩 `ArchiveOutcome` 字段面（golden wire 冻结不动）。清理失败面 SHALL 收窄为「两段式皆失败」（git 与 OS 层删除均失败），且该 `detail` SHALL 携**成因**——`ArchiveVcsPort` 缝透出的 `Err` 原文（worktree 绝对路径 + 原 git 错误 + OS 错误 + 占用成因清单），MUST NOT 只呈现原始 git 错误串、MUST NOT 以「`git worktree list` 单源对账」收尾（已注销孤儿正是 `list` 不显示的那一类，单源指引会让用户无从自救）。Cleanup SHALL 幂等：worktree 目录（host）已不在场（用户手动清理 / 重试链）时 SHALL `passed` + `detail` 注记跳过（MUST NOT 失败）；在场判定 SHALL 锚 host，MUST NOT 判执行根（执行根缺席而 host 在场 = 该 change 的执行面被手动删除，回收仍应删 host）；**已注销孤儿**（git 登记已拆而目录仍在场，如上一次回收半程失败）SHALL 经兜底回收并以 `passed` + `detail` 记「已移除」收口（MUST NOT 因 `not a working tree` 判失败）；legacy 记录（`worktree=None`）SHALL 跳过该步（`passed` + `detail` 记跳过因）。清理的 git 触点 SHALL 经 orchestration 侧 `ArchiveVcsPort` 缝承载（进程执行驻 `crates/infra/vcs`），MUST NOT 进入写面 `archive` 双写本体（desktop-change-state-store「归档双写」零 git 触点红线保持）。清理完成后 `ChangeRecord.worktree` 字段 SHALL 原样保留（历史锚——change 已 archived 无后续操作，字段作创建期执行锚留痕；详情信息行照旧呈现路径，「已清理」注记为 design 可选非必须项）。

#### Scenario: 清理成功收口

- **WHEN** 带 worktree 记录的 change 走 archive 相位至 Cleanup 步成功
- **THEN** worktree 目录自磁盘消失（`git worktree list` 不再列出），Seal 照常双写收口（db status=archived + 主仓目录改名）；`ChangeRecord.worktree` 字段原值保留

#### Scenario: 子目录 workspace 的 Cleanup 锚 host

- **WHEN** 子目录 workspace（`rel = packages/desktop`）的 worktree change 走到 Cleanup 步且 host 与执行根均在盘
- **THEN** `remove_worktree` 收到 `(main_root, host)`；清理后 `git worktree list` 不再列出该 worktree、登记目录无残留、`host/packages/desktop` 随整树消失；Seal 双写照常收口，`ChangeRecord.worktree` / `exec_root` 原值保留

#### Scenario: 已注销孤儿经兜底回收成功

- **WHEN** 该 change 的 worktree 已在上一次回收中半程失败（git 登记已拆、目录仍在场，`git worktree list` 不显示），重试链再次走到 Cleanup 步
- **THEN** 回收单点的 OS 层兜底删除该目录，Cleanup 步 `passed` + `detail` 记「已移除 worktree（<path>）」，Seal / Finalize / PhaseLog 照常收口、run 正常完成；`ArchiveOutcome` 字段面零变化

#### Scenario: 两段式皆失败仅警告不断链

- **WHEN** Cleanup 步的回收两段式皆失败（`git worktree remove` 与 OS 层删除均失败，如目录被常驻子进程持有）
- **THEN** Cleanup 步 `passed` + `detail` 携成因警告（worktree 绝对路径 + 原 git 错误 + OS 错误 + 占用成因清单，不含「执行 `git worktree remove --force <path>`」单源指引），Seal / Finalize / PhaseLog 照常收口、run 正常完成；`ArchiveOutcome` 字段面零变化

#### Scenario: worktree 缺席幂等跳过

- **WHEN** 用户已手动删除 worktree 目录后经重试链再次走到 Cleanup 步
- **THEN** Cleanup 步 `passed` + `detail` 注记「worktree 已不在场，跳过清理」，零失败面、链续走

#### Scenario: legacy 记录跳过清理

- **WHEN** 对 `worktree=None` 的存量建档 change 进入 archive 相位
- **THEN** Cleanup 步 `passed` + `detail` 记跳过因，零 `git worktree` 触点，链径直经 Seal / Finalize 收口

#### Scenario: 分支保留可溯

- **WHEN** 归档全链（含 Cleanup 成功与失败两路径）完成后审查主仓 branch 清单
- **THEN** branch `change/<name>` 在案（全程零 `git branch -d` 调用），历史可溯

## Module Contract

### 模块增量：归档链双列锚点（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/orchestration/src/state.rs`（`ArchivePreflight`） | 前置载荷 DTO | 增 `pub exec_root: Option<String>`（camelCase → `execRoot`，`None` = 与 `worktree` 同值）；`worktree` 列语义钉为 host；其余字段（`branch` / `merge_target` / `missing_artifacts` / `delta_specs` / `completed` / `incomplete_phases`）逐字不变 |
| `crates/core/orchestration/src/steps.rs`（`run_archive_preflight` / `archive_context`） | 载荷装配与消费 | 装配：`worktree` = 记录 `worktree`、`execRoot` = 记录访问器值；消费：`missing_artifacts` / `detect_delta_specs` 吃 `execRoot`，`archive_context` 仍返回 `(name, worktree)`（git 步与 Cleanup 只认 host） |
| `crates/core/orchestration/src/walker.rs`（`step_commit` / `step_rebase` / `step_resolve_conflicts` / `step_merge` / `step_cleanup`） | 归档步执行体 | 逐字不变（继续用 `archive_context` 的 host）；语义随动登记：git 步与回收锚 host、MUST NOT 传 `execRoot` |
| `crates/core/orchestration/src/walker.rs`（`step_preflight`） | 半程检出 | 逐字不变（`worktree_snapshot` 吃 host——快照含 `ls-files` 索引，锚执行根会缩域） |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 归档确认面 | 「新图预览」根取 `preflight.execRoot ?? preflight.worktree ?? root`；合入 / 清理告知文案按 `worktree`（host）呈现，不改文案结构 |
| `crates/core/orchestration/src/{steps,walker}_test.rs` | 测试 | 双列装配断言、子目录 workspace 的提交全域 / 探测执行根断言、Cleanup 锚 host 断言 |
