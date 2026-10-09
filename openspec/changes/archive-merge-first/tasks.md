# 任务: archive-merge-first

> **变更**: archive-merge-first
> **依据**: proposal.md + design.md（D1–D12 决策编号见 design）

任务边界：本列表只含实现任务；**新增测试锚**（FakeVcs 冲突编程面新锚、后验纯函数分支穷尽、git_test 冲突锚族重写、前端呈现序断言）由 test-design / test-gen / test-execution 阶段承接——但签名演进触及既有测试文件编译面，本列表阶段内含**机械随动**（改签名 / 改基设，不写新断言）。任务内引用的文件均在 design.md 变更清单内；`plugins/dev-team` 与 `openspec/specs/**` 为只读红线零触点。

## 阶段一：vcs port 签名演进与 git 工作面

- [x] `crates/core/orchestration/src/port.rs`：`ArchiveVcsPort` 签名演进（design D3）——新增 port 载荷类型 `MergeOutcome`（`Merged` / `Conflicted(Vec<String>)`）、`StatusEntry { x, y, path }`、`IndexEntry { mode, hash, stage, path }`、`WorktreeSnapshot { head, merge_head, status, index }`（文档注释注明非 IPC 面、后验 A/B 基面用途）；`merge_branch` 返回型改 `Result<MergeOutcome, String>`（冲突态保留语义——trait 文档注释随动）；新增 `worktree_snapshot(main_root) -> Result<WorktreeSnapshot, String>`、`commit_merge(main_root) -> Result<(), String>`、`abort_merge(main_root) -> Result<(), String>` 三方法（sync 签名零 tokio、`Err` 带引导文案纪律维持）；既有五方法（dirty / commit_all / branch_merged / current_branch / commit_paths）零改动
- [x] `crates/infra/vcs/src/git.rs`：`merge_branch` 冲突语义重写（design D3）——`git merge --no-edit <branch>` 成功 → `MergeOutcome::Merged`；非零退出先 `git diff --name-only --diff-filter=U -z` 读 unmerged 清单（NUL 分隔归一，非空 → **不 abort**，返回 `Conflicted(list)`）；清单空（非冲突失败——主仓状态不允许等）→ 维持既有面：尽力 `merge --abort` 后 `Err` 带 git stderr 语境与手动处置引导（AC-8 逐字语义不变）
- [x] `crates/infra/vcs/src/git.rs` 续：三新面——`worktree_snapshot`（四连：`rev-parse HEAD`；`rev-parse -q --verify MERGE_HEAD` 退出 0/1 映射 Some/None、其余 Err；`status --porcelain -z` 解析为 `StatusEntry` 族——XY 两字符 + NUL 分隔路径、rename（R/C）条目跳过第二原路径段记新路径、零引号形态（`-z` 下路径裸出）；`ls-files -s -z` 解析为 `IndexEntry` 族——`<mode> <hash> <stage>\t<path>` 逐字段切分）；`commit_merge`（`git commit --no-edit`——采用 MERGE_MSG 默认 merge 信息，D6）；`abort_merge`（`git merge --abort`，Err 上抛由链侧附注——D10）
- [x] `crates/infra/vcs/src/lib.rs`：`ProcessArchiveVcs` impl 四面随动（`merge_branch` 新返回型透传 + 三新方法委托 git 子命令）；crate 文档注释随动（冲突态保留 / 快照 / 代收口入 vcs 工作面词汇）；零新依赖（orchestration 既有）
- [x] 编译面机械随动：`crates/core/orchestration/src/archive_flow_test.rs` 的 `FakeVcs::merge_branch` 实现随新签名（最小改形——`merge_error` 注入路径返回 `Err`、其余返回 `MergeOutcome::Merged`；三新方法补空实现 + 调用捕获占位），`crates/infra/vcs/src/git_test.rs` 冲突 / non-ff 锚按新返回型机械改设（断言语义更新留给测试相位）——本任务仅保编译，`cargo check --workspace` 过

## 阶段二：归档链阶段机重整（orchestration）

- [x] `crates/core/orchestration/src/archive_flow.rs`：阶段序重整（design D1）——`ArchiveStage` 声明序对齐执行序（Preflight, Commit, Merge, SpecSync, Seal, Finalize；wire 值零变化）；`drive()` 段体重排为 校验 → 提交 → 合入 →（冲突分支）→ 同步 → 双写 → 落盘；`check_stop` 位点随段序；模块头与 SpecSync 旧注释（cwd 取舍、skill 步骤 2 桌面化措辞）随动摘除
- [x] Commit 段重写（design D2）：`branch_merged` 查询整体移除——worktree=None → skipped「legacy 无 worktree」；worktree 目录缺失 → skipped「已合入」（注释锚：前置已拦未合入形态，不设第二次 merged 查询；微秒 TOCTOU 退化形态留痕）；目录在场 → `dirty(worktree)` 真 → `commit_all(worktree, "archive: <change>")`、假 → skipped「干净」；Merge 段 merged 幂等跳过维持原样
- [x] Merge 段冲突分支（design D3–D7）：`merge_branch` 返回 `Merged` → passed；`Err` → failed（串即 port Err——AC-8 面零加工）；`Conflicted(list)` → ① `worktree_snapshot` 取 A 基面（Err → D10 lean）② 追加 running detail 更新（`合入冲突，解冲突 agent 裁决中`）③ 组装 `WorkerTurnRequest`（`merge_conflict_prompt(&change, &list)`；cwd = `request.root`；provenance `source="change"` / `source_ref="<change>/archive/merge-conflict"`；BypassPermissions；ModelLevel::High；role=Executor；SessionRef 新会话）经 `worker.run` 执行 ④ outcome `Completed` → 后验；`Stopped` → lean（串附「（归档链已停止）」注记——D7 特例，不用 TXT_STOPPED）；`Failed | Running` → lean（原因词「agent 会话失败」）⑤ 后验序：`residual_markers(&main_root, &list)`（fs 扫描清单文件行首 `<<<<<<< ` / `>>>>>>> ` 形态，读取失败 / 文件缺席按无标记处理）非空 → lean「残留冲突未解」；`worktree_snapshot` 取 B（Err → lean「后验探测失败」）；`verify_resolution(&A, &B, &list)` Err → lean（区分「残留冲突未解」/「清单外新改动」/「agent 违约自行收口 merge（MERGE_HEAD 缺席，串附 A.head 的 git reset --hard 引导——不自动执行）」原因词）⑥ 通过 → `commit_merge`（Err → failed，串附主仓 merge 态人工收口引导）→ passed（detail `已解冲突 <n> 文件`）
- [x] lean 收敛序单点（design D7/D10）：`abort_merge` 尽力执行（自身 Err → 附注「abort 未成功，请手动核验主仓 git 状态」，不掩盖原 Err）+ `fail_stage(Merge, lean 串)`；lean 串模板与原因词常量铸造（冲突文件逐行清单 + abort 告知 + 手动 merge 引导 + 重试幂等说明）
- [x] 后验纯函数（design D4）：`verify_resolution(baseline, after, conflicts) -> Result<(), String>`——① after.merge_head 在场性 ② 冲突路径索引无 stage>0 条目 ③ 冲突路径 worktree 干净（y==' '）且索引单条 stage-0 或删除缺席 ④ 非 C 路径 porcelain 状态与索引条目 A/B 逐字一致、无新路径、消失路径 ⊆ 冲突清单；`residual_markers(root, conflicts) -> Vec<String>`——清单文件逐行扫描冲突标记形态返回 `文件:行` 命中
- [x] SpecSync 段位随动（design D8）：`detect_delta_specs` 调用移至合入段之后（探测结果存局部变量供 Finalize 复用——D9）；会话 cwd 删 worktree 分支恒取 `request.root`；`spec_sync_prompt` 溯源摘除重写（design「数据模型」逐字——首行 skill 溯源括注删除，增量合并语义自持）；`merge_conflict_prompt(change, conflicts)` 新增（design「数据模型」逐字）
- [x] Finalize 扩围（design D9）：pathspec 集 = 归档两路径 + delta capability 清单各 `format!("{domain}/specs/{cap}")`（`domain_dir_name()` 单点拼；delta 缺席则两路径既有形态）；脏探测门 = 并集 dirty；`commit_paths` 调用零改动（`/**` glob 既有）；落盘提交信息维持 `archive: move <change> to archive`
- [x] `crates/core/orchestration/src/lib.rs`：re-export 面核对（`MergeOutcome` 等经 `port` 模块既有路径可见；至多注释随动，零依赖新增核对）

## 阶段三：命令面与前端呈现随动

- [x] `src/commands/archive_flow/mod.rs`：模块头注释六段序描述随动（校验 → 提交 → 合入（含冲突 agent）→ 同步 → 双写 → 落盘）；**零命令签名 / 装配 / ArchiveSink / 互斥面改动核对**（会话透传与停止寻址经既有泛化面天然覆盖两会话——单槽覆写即当前会话）
- [x] `packages/desktop/src/views/changes/flow/archive-state.ts`：`ARCHIVE_STAGES` 呈现序改 `['preflight', 'commit', 'merge', 'specSync', 'seal', 'finalize']`（注释随动；`applyArchiveUpdate` / `seedArchiveState` 零改动）
- [x] `packages/desktop/src/views/changes/flow/archive-panel.tsx`：注释随动（阶段清单新序、Merge 段 detail 演进 / lean 咨询面经既有 detail 与 error 透传——逻辑与 data-testid 零改动核对）
- [x] `packages/desktop/src/types/generated/bindings.ts`：经 `pnpm -C packages/desktop run bindings:export` 再生核对（预期零语义 diff——specta 枚举 union 声明序可能文字性重排；`bindings:check` 守线，不手改生成物）

## 阶段四：版本与守线收口（静态，不含测试执行）

- [x] `packages/desktop/package.json` `version` 0.4.25 → 0.4.26（design D12：`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）
- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误零新告警；crate 图审查：`vcs-runtime` 依赖面零新增（orchestration 既有）、`orchestration` 零依赖新增、core 各 crate 无 store / vcs-runtime 依赖
- [x] 红线自查 grep：`plugins/dev-team` 零 diff（版本 2.10.44、SKILL.md、三类交付产物）；`walker.rs` / `steps.rs` / `snapshot.rs` / `state.rs` / `control.rs` / `write/archive.rs` / `worker.rs` / `SessionRecord` 面零 diff；spec 与 prompt 与源码零 `openspec-archive-change` skill 溯源措辞（AC-9）；run 事件面零写入（`ArchiveControl` 触点仅归档链族）；git spawn 触点（merge / commit / status / ls-files / rev-parse）仅 `crates/infra/vcs`
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误且 knip 零新增豁免；`pnpm -C packages/desktop run bindings:check` 绿
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏）；AC-10 版本交付 0.4.26 已执行；lean / 幂等 / 后验各 AC 落点（design「验收标准对齐」表）留待测试相位机械断言
