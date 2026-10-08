# 探索笔记：归档链 worktree 先合入主仓（archive-merge-first）

日期：2026-10-08。对应 todo 项：【Desktop】worktree先合入主仓再归档。
背景 change：desktop-archive-change（已归档，staged 未提交批次；本次探索即对其落地链路的缺陷修复）。

## 一、dogfood 考古：desktop-archive-change 自己的归档收场

| 证据 | 值 |
|---|---|
| branch `change/desktop-archive-change` | 零提交（tip = merge-base = e9d30075e7a4 创建基线） |
| worktree（~/.dev-team/worktrees/.../desktop-archive-change） | 脏：代码 + change 目录 staged；3 份主 specs 同步产物 unstaged |
| master | 同一批内容 staged 未提交；archive_flow.rs 哈希与 worktree 不同（两侧各自演化，后期修复轮在 master 侧继续） |
| master HEAD | 领先 worktree 基线 4 个 commit |

specs 内容比对（探索时验证）：

- worktree `changes/<n>/specs/` ≡ master 归档目录 specs（逐字一致，零搬运）；
- 3 份同步产物中 state-store / worktree 两份与 master 一致；crate-layout 一份 master 侧更新（worktree 是旧稿，仍含 0.4.24 已删除的 git_diff 留后续字样）。

**处置结论：dogfood 现场零搬运，worktree 可整体丢弃**
（`git worktree remove --force <path>` + `git branch -D change/desktop-archive-change`，用户手动执行）。

## 二、缺陷机理（死法 A / 死法 B）

spec 意图（desktop-change-archive「归档编排链与阶段幂等」）：worktree **干净**则跳过提交；
**分支已合入**则跳过合入。代码实际（archive_flow.rs Commit 段）：**先查 merged、true 即跳过，
脏不脏不看**。而 `branch_merged` = `merge-base --is-ancestor`（纯可达性）。

桌面执行相位从不提交（提交是归档时动作）→ 分支零提交是**常态**。零提交分支 tip = 创建基线；
主仓只要前进过哪怕一个 commit，基线必为 HEAD 祖先 → merged=true 误判。

```
worktree 创建:  master ── X(基线) ← change/<name>（零提交，编辑全在未提交区）
之后主仓前进:   master ── X ── a ── b ── c ── d (HEAD)
                         └── is-ancestor(X, HEAD)=true ──► "已合入" ← 误判
```

- **死法 A（纯桌面路径）**：Commit+Merge 双跳过 → Seal 撞写面 merge-first 守卫（archive.rs）→
  Err 引导"请先手动 merge"，但文案没说 worktree 里还有未提交内容（用户连 merge 都没得 merge）。
- **死法 B（手动 merge 后重试，静默）**：手动 merge → 点归档 → SpecSync 在 worktree 改 specs →
  Commit 段 merged=true 跳过 → **spec 同步产物静默搁浅**，主仓 specs 永不同步，摘要还报"已同步"。

## 三、裁定（用户确认）

1. **读法 A（merge 顺带搬运）**：SpecSync 挪到 Merge 之后、cwd=主仓。deltas 随 merge 进主仓，
   "specs 进主仓"由 merge 顺带完成，不设专门搬运动作。否决读法 B（fs 搬运解耦）——归档诚实性：
   db 翻 archived 时实现代码必须已在主仓。
2. **CLI skill 即将下线**：归档链成为唯一定义源。spec 中"skill 步骤 2 桌面化 / skill 护栏语义
   对译"溯源措辞摘除，prompt 语义单源改桌面自持。skill 下线本身另行处理，本变更不动 plugin。
3. **冲突 agent 化**：merge 冲突不再 `merge --abort`+Err，改为唤起 agent 解冲突；agent 无法
   裁决时中断咨询用户。
4. **地基修复**：Commit 段砍掉 merged 短路——脏即提交；Merge 段保留 merged 幂等跳过。

## 四、链形重整

```
现状（六段）                          重整后
──────────────────────               ──────────────────────
Preflight  重校验                     Preflight  重校验
SpecSync   agent 在 worktree 改specs  Commit     脏即提交（砍 merged 短路）
Commit     merged先查 → 误跳过 ◄死法A  Merge      branch_merged ? skip : merge
Merge      冲突→abort+Err              │  ├─ 冲突 → 不 abort（UU 态保留），唤起
Seal       双写                        │  │        agent 解冲突（cwd=主仓）
Finalize   pathspec 提交               │  └─ 无法裁决 → 中断咨询用户（lean 档：
                                                abort 收口 + 停链引导手动裁决，
                                     SpecSync   agent 在主仓合 delta → 主 specs
                                                （deltas 已随 merge 进主仓）
                                     Seal       写面双写（零改动）
                                     Finalize   pathspec 提交（扩围含 openspec/specs/**）
```

读法 A 的结构性收益：

- 死法 B 结构性消灭（sync 产物生在主仓，无处搁浅）；
- sync 合并基线永远是主仓**当前** specs（旧序在 worktree 冻结基线上合并，主仓 specs 被别的
  change 更新过会产出旧基线结果，再在 git 层反向撞 spec 文件冲突）；
- sync 不再骑在 git 编排上；代价是 D7"同步产物随分支合入、主仓 aftermath 最小"理由反转——
  sync 产物落主仓工作区，由扩围后的 Finalize 提交（merge commit = 纯 change 内容，更干净）。

## 五、冲突 agent 化机制要点（dev-design 待定稿）

- 冲突判据：merge 非零退出 + `git status` UU 标记；冲突态**保留**（不 abort）。
- 解冲突 agent：cwd=主仓，bypassPermissions——爆炸半径比 worktree 会话大（用户主仓），
  链必须后验：解后仍有 UU、或冲突清单（`diff-filter=U`）之外冒出新改动 → 判"无法裁决"。
- agent 完成后的收口守 pathspec 纪律：只 `git add` 冲突文件清单，禁止 `add -A`
  （主仓无关改动不吞并是既有红线）。
- "无法裁决中断咨询用户"两档：**lean**（推荐 V1）= `merge --abort` 收口 + 停链，错误对话呈现
  冲突摘要请用户手动裁决，用户自行 merge 后重点归档（幂等续走，merged 判定天然跳过）；
  **rich** = 链进"待裁决"态 + 交互咨询面（theirs/ours/手改）——需新增 IPC 面与状态机，留后续。

## 六、改动面预估

- 代码：`archive_flow.rs`（Commit 段逻辑 ~10 行、SpecSync 段位与 cwd、Merge 冲突分支、
  Finalize pathspec 扩围）；`vcs/git.rs`（merge_branch 冲突语义改不-abort + 冲突清单读取）；
  `port.rs`（ArchiveVcsPort 或有签名随动）。
- 测试锚：`archive_flow_test`"已合入双跳过直达收口"需把 fake worktree 设干净才成立；
  新增锚——零提交+脏 → 提交发生；sync 段 cwd=主仓；冲突 → agent 路径；无法裁决 → 停链咨询。
  `git_test` merge 冲突锚随不-abort 语义重写。
- spec delta：desktop-change-archive（阶段序、溯源措辞摘除、冲突 agent 语义、Finalize 扩围）；
  desktop-change-worktree（merge-first 引导措辞对齐新链形）。
- 版本：用户可见修复，desktop 0.4.25 → 0.4.26（0.4.25 为 archive change 自身的未提交 bump）。

## 七、遗留

- dogfood worktree/branch 清理（用户手动，见第一节命令）。
- master staged 的 0.4.25 批次提交（本探索不动）。
- rich 交互咨询面留后续变更。
