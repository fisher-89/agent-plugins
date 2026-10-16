# 提案: worktree-cleanup-and-active-cap

> **变更**: worktree-cleanup-and-active-cap
> **日期**: 2026-10-14
> **状态**: draft
> **探索**: `openspec/changes/worktree-cleanup-and-active-cap/explore.md`（定夺 1–5 与二轮定夺 A/B/C 用户拍板在案）

---

## 问题

desktop change 的 worktree 生命周期止于归档：archive 相位十步（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog）走完后 **worktree 与 branch 原地留存**。这是被三处 spec 显式留痕的 V1 裁定（desktop-change-worktree 边界 1、desktop-change-archive 边界 1、desktop-change-state-store「归档双写」零 git 触点条款），不是遗漏——但随着 worktree 成为缺省执行锚，它已经变成结构性痛点：

1. **磁盘占用失控**：每个 change 一个全量 checkout + bootstrap 产物（`node_modules` / `target` 动辄数 GB）。归档不清 → 存量只增不减；用户手动清理需要知道 `worktrees/{身份段}/` 落位与 `git worktree remove` 命令（藏在 home 数据目录，无 UI 入口）。
2. **无并发上限**：create 前置七道校验没有数量维度——单 workspace 可无限累积 active change，每个都带一个 worktree；「开 8 个 change 跑构建」一次就能吃满磁盘。
3. **红线归属需要处置**：`write/archive.rs` 双写本体明写零 git 触点（纯 fs + db 双写纪律）。自动清理若进写面即破红线；正确落点是 orchestration 步（与 Commit / Rebase / Merge 同族）。

explore 已定夺的口径（详见 explore.md「定夺」与「开放点处置」）：**上限 = active 建档数 ≤ 5（硬编码）+ worktree 磁盘实占数保底门槛 7（= 5 + 2，防 db 与磁盘漂移）**，两道检查都是 create 前置；**清理 = archive 相位新增 Cleanup 步，插在 SpecSync 与 Seal 之间**（清理完成后 Seal 收口，「完成后修改 change 状态」由此成立）；**清理失败仅 step detail 警告、链照常收口**（不扩 `ArchiveOutcome` 两字段，零撞 golden wire 冻结）；**branch 不删**（`worktree remove` 本就不动分支，历史可溯）。

---

## 提案

1. **create 前置第八 / 九道（上限两道）**：前置⑧ active 建档数——db `status=active` 记录数 ≥ 5 时拒绝（上限 5 硬编码定值，不进 `config.json`，对齐 `FEEDBACK_LOOP_LIMIT` 先例）；前置⑨ worktree 磁盘实占数——`worktrees/{身份段}/` 子树直属目录数 > 7（= 5 + 冗余 2）时拒绝（db 与磁盘漂移防线：孤儿 worktree / 手动残留不占 active 额度但占磁盘）。两道均沿前置七道风格：全 IO 前置、拒绝面零 worktree / 零建档 / 零目录；错误文案含当前计数与上限，至少引导「归档或手动清理」（磁盘道另引导 `git worktree list` 对账）。弃置（abandon）面本轮不做（定夺 4 欠账，另立变更偿还）。
2. **archive 相位 Cleanup 步（11 步序列）**：`ARCHIVE_STEPS` 由 10 步扩为 11 步——SpecSync 之后、Seal 之前插入 `Cleanup`。Cleanup 为**尽力步**：带 worktree 记录时执行 `git worktree remove`（`--force` 与否 design 定稿，倾向 `--force` 以带走 bootstrap 的 `node_modules` / `target`）；worktree 目录已不在场（手动清 / 重试链）幂等跳过（`passed` + detail 注记）；legacy（`worktree=None`）跳过 + detail 记跳过因；清理失败仅以 step detail 警告呈现（含路径与手动清理指引），步恒 `passed`、链照常收口——归档必收口，残留是警告不是回滚。Finalize 步与 Cleanup / Seal 的相对座位 design 定稿（语义不变：主仓两路径脏探测落盘）。
3. **branch 保留**：`change/<name>` 分支不删（`git worktree remove` 不动分支；历史可溯）。`ChangeRecord.worktree` 字段清理后原样保留（历史锚——change 已 archived 无后续操作，二轮定夺 C；详情信息行照旧呈现路径，零 golden 重写）。
4. **git 触点归属**：Cleanup 的进程执行经 orchestration 侧 `ArchiveVcsPort` 缝（实现驻 `crates/infra/vcs`，复用 create 补偿面 `remove_worktree --force` 既有单点或等价）；写面 `archive` 双写本体零 git 触点红线保持不动。
5. **步词汇二十值**：`StepNode` / `RunStepKind` / `ChangeStepKind` 封闭集由十九值白名单式扩为二十值（新增 `cleanup`，`19→20`，先例 `mutation_test` `18→19`）；`cleanup` 归 `StepGroup::Git`。观测链随动：RunNodeRecord 全词汇落库、detail 线面 stepKind、前端 `normalizeKind` / `stepWord` 归一与 git 分类徽章、语料与 golden 显式重写、bindings 再生成。
6. **确认面告知**：stop-before-archive 停等面对 worktree 记录在场的 change，告知文案在「将执行提交 + 合入主仓当前分支」之外增加「收口后自动移除 worktree（分支保留）」——删除磁盘目录属用户可感知动作，确认在先。
7. **裸 `archive_change` 逃生口**：不经 archive 相位的裸双写命令（D11 逃生口、无前端入口）无 Cleanup 步 → 漏清。提案默认接受并留痕（低频逃生口；残留由 create 前置第九道兜底拦截）；是否补 best-effort 清理由 design 定稿——若补落命令层，MUST NOT 进写面双写本体。
8. **版本交付**：`packages/desktop` `0.5.8` → `0.5.9`；`plugins/dev-team` 零改动（2.10.44）。

---

## 能力

### 新增能力

- 无（两主题均落既有能力：清理 = 归档链扩步；上限 = create 前置扩道）。

### 修改的能力

- **desktop-change-archive** — 归档链 10 步 → 11 步（Cleanup 插 SpecSync 与 Seal 之间）；新增「Cleanup 步与 worktree 自动清理」requirement（尽力语义 / 幂等跳过 / 失败仅 detail 警告 / branch 不删 / worktree 字段保留 / git 触点走 ArchiveVcsPort）；确认面告知增清理项；V1 边界改写（branch 仍手动 + 裸命令漏清留痕 + 弃置欠账留痕）。
- **desktop-change-create** — 前置校验七道 → 九道：active 建档数上限 5（硬编码）与 worktree 磁盘实占保底门槛 7（`worktrees/{身份段}/` 子树目录数）；文案含计数与上限、引导归档或手动清理。
- **desktop-change-worktree** — merge-first 归档引导与 V1 边界的「worktree / branch 清理手动」条款改写：worktree 清理偿还为 Cleanup 步自动执行，branch 删除仍手动；孤儿对账条款补磁盘保底门槛回指。
- **desktop-change-state-store** — `RunNodeRecord.step_kind` 封闭集十九值 → 二十值（`cleanup` 白名单式扩展）；「归档双写」零 git 触点条款改写（红线保持，Cleanup 步不属双写本体）；「run 执行图落库」二十类全词汇。
- **desktop-change-orchestration** — walker 承接 archive 相位 11 步序列；Git 节点家族（commit / rebase / merge / cleanup / finalize）与归档步词 8 → 9；spawn 例外家族 vcs 域清单补 cleanup。
- **desktop-workflow-write-face** — `StepNode` 词汇十九值 → 二十值、`cleanup` 归 `Git` 分组；`ARCHIVE_STEPS` 11 步序列（Cleanup 尽力步、其余 `on_fail=None` Terminal）。
- **desktop-change-flow-view** — 步词汇归一二十值（`cleanup` ↔ `cleanup`、归 git 分类）；archive 列 11 步节点上图；Cleanup 警告 detail 呈现。
- **desktop-change-queries** — detail `nodes.stepKind` 二十值全词汇；golden 显式重写口径随动。
- **desktop-corpus-regression** — 语料节点全词汇样本二十类（含 cleanup）、archive 相位样本 11 步节点；聚合 golden 覆盖随动重写。

### 引用沿用（零 delta）

- desktop-app-shell — 命令面零新增：`create_change` / `archive_change` 签名不变，上限错误经既有 `Result<T, String>` 面透传。
- desktop-ipc-type-bindings — bindings 再生走既有管线（`RunStepKind` 词面扩展由 specta 自动跟随），一致性守卫不变。
- desktop-crate-layout — vcs 边界既有租户（git 工作面），零分类学变化。
- desktop-workspace-store / foundation — 身份段派生单点零改动（worktree 目录数扫描消费 `worktrees/{身份段}/` 派生产物）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `StepNode` / `RunStepKind` 词汇 19 → 20（`Cleanup` / `cleanup`，同词面 1:1；`cleanup` 归 `Git` 分组）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `ARCHIVE_STEPS` 10 → 11 步（SpecSync 与 Seal 间插 `Cleanup`）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` — 前置⑧ active 建档数上限（常量 `ACTIVE_CHANGE_LIMIT = 5`，命名 design 定稿）与前置⑨ worktree 目录保底门槛（= 上限 + 2）；文案单点常量（含计数 / 上限 / 引导）
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `ChangeStepKind::Cleanup`（`as_str` = `cleanup`）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `step_cleanup`（记录解析 worktree 字段 → 缺席 / legacy 幂等跳过；remove 失败吞为 detail 警告、步恒 passed；branch 零触碰）+ 计划步路由臂
- `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` — `ChangeStepKind → RunStepKind` 映射补 Cleanup
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `ArchiveVcsPort` 扩 worktree 清理面（在场探测 + 移除，形态 design 定稿；`Err` 面带手动清理引导）
- `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` + `lib.rs` — `ProcessArchiveVcs` 清理实现（复用既有 `remove_worktree --force` 单点或等价；真实 git tempdir 夹具可测）
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `RunStepKind` 词面消费随动（机械）
- `packages/desktop/src/views/changes/flow/run-state.ts` — `normalizeKind` / `stepWord` 补 `cleanup` 归一；git 分类
- `packages/desktop/src/views/changes/flow/run-step-node.tsx` — `STEP_LABEL` 补 cleanup 词条
- `packages/desktop/src/types/generated/bindings.ts` — 再生成（`RunStepKind` 词面）
- `packages/desktop/src/views/changes/`（stop-before-archive 停等面文案）— worktree 记录在场时告知增「收口后自动移除 worktree（分支保留）」
- `packages/desktop/package.json` — `version` 0.5.8 → 0.5.9

### 测试文件

- `crates/core/workflow/src/write/phase_table_test.rs` — `ARCHIVE_STEP_ORDER` 10 → 11 逐位断言、二十值封闭集
- `crates/core/workflow/src/write/create_test.rs` — 前置⑧⑨拒绝面（第 6 个 active 拒 / 8 个 worktree 目录拒 / 5 与 7 边界通过 / 文案含计数与引导 / 拒绝面零建档零目录零 worktree / 零 vcs 调用）
- `crates/core/workflow/src/state_test.rs` — `StepNode` / `RunStepKind` 二十值词面与分组
- `crates/core/orchestration/src/walker_test.rs` — Cleanup 步五面：成功移除（目录消失 + `git worktree list` 不列 + branch 在）/ remove 失败警告不断链（Seal / Finalize / PhaseLog 照常、`ArchiveOutcome` 字段零变化）/ worktree 缺席幂等跳过 / legacy 跳过 / 零 `git branch` 触点
- `crates/core/orchestration/src/state_test.rs` / `run_history_test.rs` — 词面与映射
- `crates/core/orchestration/src/port_test.rs` — 清理面构造与 `Err` 面
- `crates/infra/vcs/src/git_test.rs` — 清理单点回归（真实 git tempdir）
- `crates/core/workflow/src/queries/detail_test.rs` — stepKind 二十值出线
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/` — cleanup 步词样本 + archive 相位 11 步样本；`DESKTOP_GOLDEN_REWRITE=1` 显式重写
- `packages/desktop/src/views/changes/flow/run-state.test.ts` / `run-step-node.test.tsx` — cleanup 归一、git 分类徽章、标签
- `src/commands/changes/mod_test.rs` — create 命令面上限错误透传回归（三件事纪律不变）

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（MCP 工具、hooks、版本 2.10.44、三类交付产物）
- `crates/core/workflow/src/write/archive.rs` 双写本体（零 git 触点红线——清理不进写面；裸命令语义不变）
- `ArchiveOutcome` 字段面（golden wire 冻结；清理警告只落 step detail）
- `ChangeRecord.worktree` 字段语义与 detail / `CreateOutcome` 出线（清理后历史锚保留，零 golden 重写）
- `crates/core/workflow/src/write/` 相位机操作语义（phase_next / phase_start / phase_log / backtrack / create 四段组合与补偿链）
- branch `change/<name>` 生命周期（全程零 `git branch -d`；`delete_branch` 仍限 create 补偿面）
- `config.json` 与 config schema（上限硬编码，零新增键）
- `FEEDBACK_LOOP_LIMIT` 等既有常量、prompt 模板与相位表其余条目
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | active 上限 | db 已有 5 条 `status=active` 记录时 create 显式 `Err`（文案含当前计数 5 与上限 5、引导归档或手动清理），零建档零目录零 worktree零 vcs 调用；4 条时第 5 个创建成功；归档记录不计入 |
| AC-2 | worktree 磁盘保底 | `worktrees/{身份段}/` 直属目录数 8 时 create 显式 `Err`（文案含目录数 8 与门槛 7、`git worktree list` 对账引导）；≤7 照常创建；db 归档数与磁盘漂移场景（如 3 active + 5 残留）被拦 |
| AC-3 | 上限硬编码 | 上限与门槛为源码常量（create 域单点），`config.json` / config schema 零新增键；两道检查均全 IO 前置（拒绝面零副作用） |
| AC-4 | 11 步序列与二十值词汇 | `ARCHIVE_STEPS` 11 步在案（`… Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`）；`StepNode` / `RunStepKind` / `ChangeStepKind` 二十值同词面全映射，`cleanup` 归 Git 分组 |
| AC-5 | Cleanup 成功清理 | 带 worktree 的 change 归档收口后：worktree 目录消失（`git worktree list` 不再列出）、branch `change/<name>` 在案、db status=archived、`ChangeRecord.worktree` 字段原值保留 |
| AC-6 | Cleanup 尽力与幂等 | remove 失败：Cleanup 步 `passed` + detail 含警告（worktree 路径 + 手动清理指引），Seal / Finalize / PhaseLog 照常收口、`ArchiveOutcome` 字段零变化；worktree 缺席（手动清 / 重试链）：`passed` + detail 注记、零失败面 |
| AC-7 | legacy 与 branch 零触碰 | `worktree=None` change 归档：Cleanup `passed` + detail 记跳过因、零 `git worktree` 触点；全程（含清理失败路径）零 `git branch -d` 调用 |
| AC-8 | 确认面清理告知 | stop-before-archive 停等面对 worktree 记录在场的 change 呈现「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支保留）」告知；警告不阻断语义不变 |
| AC-9 | 观测面二十值 | cleanup 节点落 `RunNodeRecord`（零过滤）并串行 forward 边入列；前端 `normalizeKind` / `stepWord` 归一、git 分类徽章与标签上图；语料含 cleanup 样本与 archive 11 步样本，golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；bindings 再生成且守卫绿 |
| AC-10 | 裸命令漏清留痕 | 裸 `archive_change` 直调（两树命中形态）零清理——留痕可考（若 design 决定补 best-effort：落命令层、写面零改动、失败不阻断双写结果）；spec 边界条款在案 |
| AC-11 | 版本交付 | `packages/desktop` version 0.5.9（`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 仍 2.10.44；`vp test` / `client:check` / knip 全绿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 上限误伤：真实并行需求 > 5 | 第 6 个 change 被拦 | 中 | 文案引导归档或手动清理；上限为源码常量，调整成本低（另立变更）；门槛 5 定值经用户拍板（定夺 1） |
| R2 清理失败静默残留：remove 失败仅警告，archived 后无重试路径 | 磁盘仍占 | 低 | 警告含路径与手动命令；`--force` 倾向带走脏面（design 定稿）；create 前置第九道兜底拦后续失控 |
| R3 自动清理带走排查现场（node_modules / 检查报告目录随 worktree 删除） | 失败 change 事后排查缺现场 | 中 | 确认面告知在先（AC-8，用户确认进入 archive 相位即知情）；branch 保留（git 历史可溯）；既有边界「report_dir 绝对路径接受失效」（desktop-change-worktree 边界 7）维持 |
| R4 词汇扩展波及面：十九值 → 二十值牵动 5 个 spec 表述、语料与 golden | 机械改写遗漏 → 断言漂移 | 中 | 白名单式扩展先例（mutation_test 18→19）；AC-4 / AC-9 汇总断言；golden 显式重写流程 |
| R5 db 与磁盘漂移误判：active=3 但磁盘 8 目录（孤儿 / 手动建档） | 第二道拦正常创建 | 低 | 文案区分两道检查对象（active 记录 vs 磁盘目录）并引导 `git worktree list` 对账；门槛 +2 冗余只拦失控不拦正常流 |
| R6 裸命令漏清（D11 逃生口） | 逃生口归档后 worktree 残留 | 低 | 留痕 + 第九道兜底；design 可选补命令层 best-effort |
| R7 Seal 后移时序：Cleanup 卡在 Seal 前（git 慢 / 锁竞争） | 归档链尾部变长 | 低 | 尽力步无重试循环；并行归档各自 worktree、锁竞争失败仅警告 |
| R8 `ChangeRecord.worktree` 死路径呈现：清理后详情信息行仍显原路径 | 用户误以为目录在场 | 低 | 二轮定夺 C 接受（历史锚）；可选「已清理」注记为 design 非必须项 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 上限口径 | active 建档数 ≤ 5（db `status=active` 计数） | explore 定夺 1：db 是状态权威、计数零 fs；并行 5 已宽裕 | worktree 目录数单道（db 与磁盘漂移时失守）；用户可配（过度设计） |
| 磁盘保底门槛 | `worktrees/{身份段}/` 目录数 ≤ 7 = 5 + 2 | 定夺 1：冗余防线防 db 与磁盘漂移（孤儿 / 手动残留）失控；+2 容忍既有在途漂移 | 单道 db 计数；门槛 = 5（漂移即拦，误伤） |
| 清理落点 | archive 相位新增 Cleanup 步，SpecSync 之后、Seal 之前 | 定夺 2 + 二轮 B：Seal（改名 + db 翻转）整体后移，「清理完成后修改 change 状态」成立；Cleanup 成为双写收口前置 | 清理后置 PhaseLog 之后（收口后再动磁盘，状态与磁盘次序倒挂）；create 时清（语义不符） |
| 清理失败语义 | step detail 警告，链照常收口；步恒 passed | 二轮 A：不扩 `ArchiveOutcome`（零撞 golden wire 冻结）；归档必收口，残留是警告不是回滚 | Terminal 停链（归档卡尾）；扩 outcome 字段（破冻结） |
| Cleanup 幂等 | worktree 缺席幂等跳过（passed + 注记） | 开放点 E 收敛：手动清 / 重试链是合法常态，失败面伤重试 | 缺席报错（重试链自绊） |
| branch 处置 | 不删 | 定夺 3：历史可溯；`worktree remove` 本就不动分支 | 连带 `branch -d`（丢历史锚，超 explore 定夺） |
| worktree 字段 | 清理后原样保留（历史锚） | 二轮 C：change 已 archived 无后续操作；零 golden 重写 | 置 None / 加 cleaned 标记（字段演进无收益） |
| 上限定值落位 | 源码硬编码，不进 `config.json` | 定夺 5：对齐 `FEEDBACK_LOOP_LIMIT` 先例；配置面先例是用户域配置非流程护栏 | config 键（schema / 校验 / 文档成本，无真实诉求） |
| 弃置（abandon） | 本轮不做，错误文案引导「归档或手动清理」 | 定夺 4：上限生效后「active 占额度无退路」的洞已知，另立变更偿还 | 本轮连带做（扩面超 explore 定夺） |
| 裸 `archive_change` | 默认接受漏清 + 留痕；best-effort 补否 design 定 | D 开放点：逃生口无前端入口、低频；若补落命令层（写面零 git 触点红线不动） | 逃生口直接退役（超本轮范围） |
| 步词 | `cleanup`（snake/camel 同形），归 Git 分组 | 白名单式扩展先例（mutation_test）；vcs 步族分类一致 | 复用既有步词（无语义可复用） |

### 待决问题

- Cleanup 的 `--force` 取舍（explore 开放点 G）：倾向 `--force`（带走 bootstrap 的 node_modules / target 磁盘大头）；顾虑是手动边界场景用户可能想保留残留排查——确认面告知已部分对冲，design 定稿。
- 裸 `archive_change` 逃生口是否补 best-effort 清理（开放点 D）：若补，落命令层薄包装（双写成功后尽力 remove、失败仅警告），MUST NOT 进写面；design 定稿。
- Finalize 步与 Cleanup / Seal 的相对座位（二轮 B 遗留）：Finalize 留在 Seal 后（现状）或前移，语义不变（主仓两路径脏探测），design 定稿。
- `ArchiveVcsPort` 清理面形态：单方法 `cleanup_worktree`（内含在场探测）vs 探测 + 移除两方法；与 create 补偿面 `WorktreePort::remove_worktree` 的复用关系（infra 单点 vs 双 port 各表）——design 定稿。
- 上限常量命名与落位（`ACTIVE_CHANGE_LIMIT` / 门槛派生 `+2` 形态；create.rs 私有常量沿 `BOOTSTRAP_LOCKFILE_COMMANDS` 先例）——design 定稿。
- 清理警告 / 跳过 detail 文案定稿（含手动清理指引句式：`git worktree remove --force <path>`）与停等面告知文案——随实现定稿。
- 详情信息行是否加「已清理」可选注记（二轮 C 的非必须项）——design 定稿，默认不加。
