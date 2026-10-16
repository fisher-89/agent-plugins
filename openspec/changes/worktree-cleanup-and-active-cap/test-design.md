# 测试设计: worktree-cleanup-and-active-cap

> **变更**: worktree-cleanup-and-active-cap
> **基于**: proposal.md（AC-1 ~ AC-11）、design.md（D1–D8 定稿、变更清单、公共函数 / API 表）
> **角色**: test-design-planner 产物（只设计测试范围与用例面，不含测试命令细节；静态检查 / 测试执行由桌面端承接）

## 框架与布局惯例（盘点结论）

- **Rust 侧**：内联 `#[test]` / `#[tokio::test]` 单元测试，测试文件为源文件同目录 `*_test.rs`（经 `#[cfg(test)] mod` 挂载）；断言风格 `assert_eq!` / `assert!`（中文失败信息）。集成节收 `crates/core/workflow/tests/corpus_golden_test.rs`（db 种子语料 → 读链投影 → golden 快照，`DESKTOP_GOLDEN_REWRITE=1` 显式重写）。`infra/vcs` 侧沿真实 git tempdir 真件形态（`tempfile::TempDir` 隔离 + 直驱 `git` 子命令断言）。
- **前端侧**：vitest（`vite-plus/test` 导入面：`describe` / `it` / `expect` / `vi`），colocated `*.test.ts` / `*.test.tsx`；组件测试直渲染（`screen` / `fireEvent` + `data-testid` 锚点），纯函数测试内联 fixture 直测。
- **用例表口径**：`路径类型` ∈ 正向 / 异常 / 边界 / 组合（组合 = 跨模块链路用例，挂靠链路发起方所在章节）；`迭代类型` ∈ 单次 / 重试链（幂等重入面）/ 枚举遍历（全词汇穷举）。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | active 上限：db 已有 5 条 `status=active` 记录时 create 显式 `Err`（文案含当前计数 5 与上限 5、引导归档或手动清理），零建档零目录零 worktree 零 vcs 调用；4 条时第 5 个创建成功；归档记录不计入 | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`、`packages/desktop/src-tauri/src/commands/changes/mod.rs`（零改动沿用——组合透传回归，用例挂靠 create.rs 章节） |
| AC-2 | worktree 磁盘保底：`worktrees/{身份段}/` 直属目录数 8 时 create 显式 `Err`（文案含目录数 8 与门槛 7、`git worktree list` 对账引导）；≤ 7 照常创建；db 归档数与磁盘漂移场景（如 3 active + 5 残留）被拦 | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`、`packages/desktop/src-tauri/src/commands/changes/mod.rs`（组合透传回归） |
| AC-3 | 上限硬编码：上限与门槛为源码常量（create 域单点），`config.json` / config schema 零新增键；两道检查均全 IO 前置（拒绝面零副作用） | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`（config 零新键为静态核对，见「不可测试项」） |
| AC-4 | 11 步序列与二十值词汇：`ARCHIVE_STEPS` 11 步在案（… Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog）；`StepNode` / `RunStepKind` / `ChangeStepKind` 二十值同词面全映射，`cleanup` 归 Git 分组 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（11 步链序）、`packages/desktop/src/views/changes/flow/run-state.ts` |
| AC-5 | Cleanup 成功清理：带 worktree 的 change 归档收口后 worktree 目录消失（`git worktree list` 不再列出）、branch `change/<name>` 在案、db status=archived、`ChangeRecord.worktree` 字段原值保留 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`、`packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`、`packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`（单点回归锚） |
| AC-6 | Cleanup 尽力与幂等：remove 失败步恒 `passed` + detail 警告（worktree 路径 + 手动清理指引），Seal / Finalize / PhaseLog 照常收口、`ArchiveOutcome` 字段零变化；worktree 缺席（手动清 / 重试链）`passed` + detail 注记、零失败面 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` |
| AC-7 | legacy 与 branch 零触碰：`worktree=None` change 归档 Cleanup `passed` + detail 记跳过因、零 `git worktree` 触点；全程（含清理失败路径）零 `git branch -d` 调用 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`、`packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`、`packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` |
| AC-8 | 确认面清理告知：stop-before-archive 停等面对 worktree 记录在场的 change 呈现「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支保留）」；警告不阻断语义不变 | `packages/desktop/src/views/changes/flow/run-control-panel.tsx` |
| AC-9 | 观测面二十值：cleanup 节点零过滤落 `RunNodeRecord` 并串行 forward 边入列；前端 `normalizeKind` / `stepWord` 归一、git 分类徽章与标签上图；语料含 cleanup 样本与 archive 11 步样本、golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；bindings 再生成且守卫绿 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（落库组合）、`packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`、`packages/desktop/src-tauri/crates/infra/store/src/model.rs`、`packages/desktop/src-tauri/crates/infra/store/src/store.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`、`packages/desktop/src/types/generated/bindings.ts`（生成物，见「不可测试项」）、`packages/desktop/src/views/changes/flow/run-state.ts`、`packages/desktop/src/views/changes/flow/run-step-node.tsx` |
| AC-10 | 裸命令漏清留痕：裸 `archive_change` 直调（两树命中形态）零清理——留痕可考；spec 边界条款在案 | `packages/desktop/src-tauri/src/commands/changes/mod.rs`（零改动沿用——既有零 vcs 依赖语义回归持衡；spec 文本面见「不可测试项」） |
| AC-11 | 版本交付：`packages/desktop` version 0.5.9（`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 仍 2.10.44；`vp test` / `client:check` / knip 全绿 | `packages/desktop/package.json`（静态门，见「不可测试项」） |

> 覆盖核对：AC-1 ~ AC-11 十一条全量在案；「被测文件或模块」列合计 18 项 = design.md 修改文件子表 17 项 + 零改动沿用的命令面组合回归锚（`commands/changes/mod.rs`）——修改文件子表无一遗漏；源文件 → 测试文件映射见下文 per-file 章节标题。

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- StepNode.as_str(): 补 `Cleanup => "cleanup"` 臂（十九值 → 二十值封闭集词面单点）
- StepNode.group(): `Cleanup` 派生 `StepGroup::Git`（Git 步族 Commit / Rebase / Merge / Cleanup / Finalize）
- StepNode.role(): `Cleanup` 恒 `None`（非 agent 步、零 prompt 零角色）
- RunStepKind.as_str(): 补 `Cleanup => "cleanup"` 臂（与 StepNode 同词面 1:1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| StepNode.as_str() | 正向 | `StepNode::Cleanup` 出线 `"cleanup"` 逐字；serde wire 出线 `json!("cleanup")`（snake 线词与 as_str 一致） | 单次 |
| StepNode.as_str() | 边界 | 二十值全词汇穷举逐字对拍（既有十九值持衡 + 新臂入集；`len()==20` 无遗漏无重复） | 枚举遍历 |
| StepNode.as_str() | 异常 | 非法词反序列化拒绝持衡：camel 近似词 / 空串 / 越集词仍拒绝（扩词不松动封闭集防线） | 单次 |
| StepNode.group() | 正向 | `StepNode::Cleanup` → `StepGroup::Git`；与 Commit / Rebase / Merge / Finalize 同组五值断言 | 单次 |
| StepNode.group() | 边界 | 二十值逐值分组穷举（cleanup 入 Git、agent 步 / gate 步 / toolStep 归组持衡零漂移） | 枚举遍历 |
| StepNode.role() | 正向 | `StepNode::Cleanup` → `None`（非 agent 步） | 单次 |
| StepNode.role() | 边界 | 二十值逐值 role 穷举（cleanup 与 exec 族 None 面一致、agent 步角色持衡） | 枚举遍历 |
| RunStepKind.as_str() | 正向 | `RunStepKind::Cleanup` → `"cleanup"`；wire 词面与 as_str 逐字一致 | 单次 |
| RunStepKind.as_str() | 边界 | `StepNode` 与 `RunStepKind` 逐字同源表补 `(Cleanup, Cleanup, "cleanup")` 行（二十值 1:1 零错位） | 单次 |
| RunStepKind 反序列化 | 异常 | 越集词形拒绝（封闭集外词一律 Err——十九值 → 二十值后边界外词形仍拒） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：中性类型 + serde_json 直驱词面断言，零进程边界依赖 | 全节（词汇 / 分组 / role 纯派生面真实组合） |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- phase_table(): archive 相位 `PhaseDefinition` 随 `ARCHIVE_STEPS` 声明自然出线 11 步（行为随表，零函数改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| phase_table() | 正向 | requirement 与 test-only 两表现均出 archive 相位且步序 11 步：PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → **Cleanup** → Seal → Finalize → PhaseLog 逐位断言（`ARCHIVE_STEP_ORDER` 夹具常量 10 → 11 扩位） | 单次 |
| phase_table() | 正向 | 两 workflow_type 共用同一 static 单源：requirement 与 test-only 的 archive 步序逐位一致 | 单次 |
| phase_table() | 边界 | Cleanup 步 `StepDefinition` 形态：`node=StepNode::Cleanup`、`prompt_ref=None`、`on_fail=None`（尽力语义驻 walker 执行体，声明面与其余步一致） | 单次 |
| phase_table() | 边界 | 十一步词表与 `StepNode` 二十值封闭集对照（步词无漂移、无重复、全在集内） | 枚举遍历 |
| phase_table() | 异常 | 非法 workflow_type → `None` 持衡（扩步不改变未知类型拒绝面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：编译期 const static 表直读断言，零 IO | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能

- create(): 前置⑧ active 建档数上限（`ACTIVE_CHANGE_LIMIT = 5`，store 读 `status=active` 计数；归档记录不计入）
- create(): 前置⑨ worktree 磁盘保底门槛（门槛 = `ACTIVE_CHANGE_LIMIT + WORKTREE_DIR_SLACK` = 7，fs 扫 `worktree_root` 直属子目录数）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| create() 前置⑧ | 异常 | db 预置 5 条 `status=active` → 显式 `Err`；文案含「当前 5」「上限 5」与「归档或手动清理」引导（D6 单点文案） | 单次 |
| create() 前置⑧ | 正向 | db 4 条 active → 第 5 个创建成功（`CreateOutcome` 正常出线，建域四段照常） | 单次 |
| create() 前置⑧ | 边界 | 归档记录不计入：5 条 `status=archived` + 4 条 active → 创建成功（计数仅 status=active 面） | 单次 |
| create() 前置⑨ | 异常 | `worktree_root` 直属子目录 8 个 → 显式 `Err`；文案含目录数 8、门槛 7 与「git worktree list」对账引导 | 单次 |
| create() 前置⑨ | 正向 | 7 个直属子目录（恰为门槛值）→ 照常创建（严格大于才拒） | 单次 |
| create() 前置⑨ | 边界 | `worktree_root` 不存在（NotFound）→ 计 0 通过（尚无 worktree 面）；目录内普通文件条目不计入（仅 `file_type().is_dir()`） | 单次 |
| create() 前置⑨ | 异常 | `fs::read_dir` 非 NotFound 读错误 → 显式 `Err`（不静默计 0、不伪装通过） | 单次 |
| create() 前置⑨ | 边界 | db 与磁盘漂移场景：3 条 active + 5 个残留目录（合计 8 > 7）→ 前置⑨拦（孤儿 / 手动残留防线，AC-2） | 单次 |
| create() 拒绝面 | 边界 | 两道拒绝面零副作用：store 零建档调用、worktree_root 零新目录、主仓 active 树零目录、worktree 零创建、vcs 零调用（假件调用记录空——AC-1 / AC-3） | 单次 |
| create() 前置序 | 边界 | ⑧⑨ 插在前置④（同名 active 查重）之后、前置⑤（`vcs.probe` git 探测）之前：同名 active 场景仍先④拒；active 计数 5 且磁盘目录超门槛场景错误文案为 active 上限道（先⑧后⑨可辨） | 单次 |
| create()（组合：commands::changes::create_change → write::create） | 正向 | 链路方向命令面 → 写面：真实 tempdir Store 预置 4 条 active + 真实 git 仓夹具 → 第 5 个经 `create_change_with` 创建成功（mod_test 回归，三件事纪律不变） | 单次 |
| create()（组合：commands::changes::create_change → write::create） | 异常 | 同链路：第 6 个请求 → 上限 `Err` 串经既有 `Result<T, String>` 面逐字透传（零包装零改写，mod_test 断言文案含计数与引导） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ChangeStateStore`（进程内假件 CreateStore） | 沿既有 create_test 假件扩展：可预置 N 条 active / archived 记录 + 建档调用捕获（DB 进程边界以注入 trait 假件替代） | 前置⑧ 计数面、拒绝面零建档断言、归档不计入边界 |
| `WorktreePort`（进程内脚本化假件 FakeVcs） | 沿既有假件：probe / branch_exists 等可编程 + 调用与参数记录（git 子进程边界以 port 假件替代） | 拒绝面零 vcs 调用断言、前置序（⑧⑨ 先于 git 探测）断言 |
| 文件系统 | 真实 tempdir 双根（主仓 root + worktree_root）× 目录数夹具（0 / 7 / 8 组 mkdir 预置）——fs 不 mock | 前置⑨ 磁盘实占扫描（真实 fs 组合） |
| 命令面组合（mod_test 节） | 零 mock：真实 tempdir Store + 真实 git 仓（`as_git_repo` 夹具）+ 真实 fs + `tauri::test::MockRuntime` 句柄 | create_change / create_change_with 透传回归（沿既有 mod_test 装置） |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（纯 derive 零字段属性、零代码改动）——下列条目为 design.md 变更清单声明的线面随动面，经既有 detail 读链入口驱动；另收语料 golden 组合节。 -->

- 详情节点投影 stepKind 线面: 二十值全词汇出线零过滤（doc 十九值 → 二十值注释随动，词汇扩展自动跟随）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 详情节点投影 stepKind | 正向 | 二十值全词汇出线零过滤：一次覆盖二十值词汇的 run → detail 节点 `stepKind` 全量在案（含 cleanup；既有十九值断言 19 → 20 更新） | 枚举遍历 |
| 详情节点投影 stepKind | 边界 | cleanup 节点 detail 出线形态与归档 git 步族一致（`cleanup` snake 线词、phase=archive） | 单次 |
| 详情节点投影 stepKind | 异常 | db 坏行 / 越集词既有拒绝面持衡（扩词不改变读侧错误语义——回归锚） | 单次 |
| 语料组合（链路：tests/corpus_golden_test.rs 种子 → change_detail 投影 → golden） | 正向 | 语料节点样本补 cleanup 步词样本 + archive 相位 11 步节点样本（PhaseStart … PhaseLog 含 Cleanup）→ golden 全量对比随动；重写经 `DESKTOP_GOLDEN_REWRITE=1` 显式留痕（fixtures/ 矩阵随动，AC-9） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| db store | 真实 tempfile Store（dev-dep store 真件）——沿既有 detail_test / corpus 装置，不 mock | 全节（db 为真实组合面，读链投影零过滤断言） |
| 磁盘产物树 | 真实 tempdir 合成产物树（corpus 节既有构造器） | corpus golden 组合节 |
| golden 文件 | 入仓 golden 快照 + `DESKTOP_GOLDEN_REWRITE` 环境变量开关（进程边界经测试环境注入） | corpus golden 重写留痕 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- ChangeStepKind.as_str(): 补 `Cleanup => "cleanup"` 臂（serde camelCase 线词 `cleanup` 与 snake 同形）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ChangeStepKind.as_str() | 正向 | `ChangeStepKind::Cleanup` → `"cleanup"`；serde wire 出线 `json!("cleanup")`（camel 与 snake 同形逐字） | 单次 |
| ChangeStepKind 词汇 | 边界 | 二十值穷举互异无碰撞（`len()==20`、wire 集合互异——既有十九值断言扩位） | 枚举遍历 |
| ChangeStepKind 反序列化 | 异常 | 越集词拒绝：`"clean-up"` / `"cleanUp"` / 空串等封闭集外词形一律 Err | 单次 |
| ChangeStepKind 词面同源 | 边界 | `ChangeStepKind::Cleanup` 与 `workflow::state::RunStepKind::Cleanup` 的 `as_str()` 逐字同源（身份串词面对拍表补行） | 单次 |
| ChangeStepKind 往返 | 正向 | `"cleanup"` 序列化 / 反序列化双向派生往返等值（rename_all camelCase 线格式持衡） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯词面 / 派生断言（serde_json 直驱），零进程边界依赖 | 全节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表显式注记：step_cleanup 为私有 async 执行体、Cleanup 路由臂为 run_step 内臂，均非模块级导出（不入公共函数表）——Cleanup 五面行为经既有归档链公共入口驱动。 -->

- run_step(): 增 `StepNode::Cleanup` 路由臂（SpecSync 与 Seal 臂之间分派至清理步体）
- step_cleanup(): worktree 字段解析 → legacy / 缺席幂等跳过（`passed` + detail 注记）→ 在场经 `ctx.vcs.remove_worktree`，`Err` 吞为 detail 警告、步恒 `passed`、链必收口（永不 `Terminal`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| step_cleanup（经归档链公共入口驱动） | 正向 | 成功移除：preflight 载 worktree 字段且目录在场、FakeVcs remove Ok → Cleanup 节点 `passed` + detail「已移除 worktree（{path}）」；Seal / Finalize / PhaseLog 续走、run 收口 Completed（AC-5） | 单次 |
| step_cleanup | 异常 | remove 失败警告不断链：FakeVcs remove 可编程 `Err` → 步恒 `passed`、detail 含警告（worktree 路径 + `git worktree remove --force {path}` 手动指引）；Seal / Finalize / PhaseLog 照常收口、`ArchiveOutcome` 字段零变化（AC-6） | 单次 |
| step_cleanup | 边界 | worktree 缺席（目录不在场——手动清 / 重试链形态）：`passed` + detail「worktree 已不在场，跳过清理」（TXT_SKIP_ABSENT）；零失败面 | 单次 |
| step_cleanup | 边界 | 幂等重入：缺席跳过后链重入（重试链二次进入 Cleanup）同样 `passed` + 同词注记，不自绊 | 重试链 |
| step_cleanup | 边界 | legacy（`worktree=None`）：`passed` + detail 记跳过因（复用既有 TXT_SKIP_LEGACY）；零 `git worktree` 触点（FakeVcs 调用记录无 remove——AC-7） | 单次 |
| step_cleanup | 异常 | 缺 preflight 载荷（计划漂移面）：failed 节点 + Terminal——沿既有归档步漂移面（非清理失败面，尽力语义不吞计划漂移） | 单次 |
| step_cleanup 参数面 | 正向 | `remove_worktree` 以 `main_root` + `worktree` 双参原样到达 port（preflight.worktree 载荷透传、零改写） | 单次 |
| 归档链 11 步序 | 正向 | 既有十步序断言扩为 11 步：phase_start → preflight → commit → rebase → resolve_conflicts → merge → spec_sync → **cleanup** → seal → finalize → phase_log 逐位一致；全步 phase=archive、emit 序即落包序（AC-4） | 单次 |
| branch 零触碰 | 边界 | 全程（含 remove 失败路径）零 branch 删除触点：FakeVcs 调用记录词汇白名单断言（dirty / commit_all / rebase / merge / snapshot / remove_worktree 在列，无 delete_branch 面——AC-7） | 单次 |
| 落库组合（链路：walker → run_history → RunNodeRecord） | 正向 | cleanup 节点零过滤落 `RunNodeRecord` 并串行 forward 边入 `RunEdgeRecord`（SpecSync → Cleanup → Seal 相邻边序断言——AC-9） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ArchiveVcsPort`（进程内假件 FakeVcs） | 沿既有 walker_test 假件扩展 `remove_worktree`：Ok / Err 可编程 + 调用与双参捕获（git 子进程边界以注入 port 假件替代） | Cleanup 五面、参数透传、branch 零触碰断言 |
| `ToolStepPort` / `WorkerAgentPort` / `WorkflowSnapshotPort`（既有假件 FakeTools / FakeWorker / StubSnapshot） | 沿既有装置：写面预录归档产出（Seal / Finalize 消费面）、timeline 调用序捕获（被测 walker 显式注入依赖） | 归档链 11 步序、收口不断链断言 |
| 文件系统 | 真实 tempdir 承载 worktree 目录在场 / 缺席两形态（fs 不 mock——在场探测为被测逻辑本体） | 缺席幂等跳过、成功移除路径 |
| 落库面 | 进程内 history 收集器（沿既有 finish_command 落包装置），真件 Store 组合行收 corpus_golden_test | cleanup 节点 / 边落库断言 |

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表显式注记：mapped_step 为私有映射函数，不入公共函数表——条目为 design.md 变更清单声明；finish_command 为本文件既有公共落包入口（本次零签名改动）。 -->

- mapped_step(): 补 `ChangeStepKind::Cleanup => RunStepKind::Cleanup` 臂（二十值 1:1 直映射零过滤）
- finish_command(): 落包入口——cleanup 节点零过滤入 command.nodes、串行 forward 边入 `RunEdgeRecord`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| mapped_step | 正向 | `ChangeStepKind::Cleanup` → `RunStepKind::Cleanup` 直映射（新臂逐对断言） | 单次 |
| 词汇直映射 | 边界 | 二十值全枚举 1:1 同词面零过滤（`all.len()==20`；既有十九值断言扩位） | 枚举遍历 |
| finish_command 落包 | 边界 | 一次覆盖二十值词汇的 run → `command.nodes.len()==20` 零过滤（cleanup 节点在案、无静默丢弃） | 单次 |
| finish_command 落包 | 异常 | 悬空边 / 空包既有透传语义持衡（扩词不改变落包边界行为——回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯映射函数 + 进程内 history 构造直测 | 全节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- ArchiveVcsPort.remove_worktree(): 新增 trait 方法 `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>`（在场探测驻调用方、port 零探测方法）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ArchiveVcsPort.remove_worktree() | 正向 | trait 面可注入：测试假件实现该面（Ok / Err 可编程 + 调用与双参捕获），walker 消费面以 `Arc<dyn ArchiveVcsPort>` 注入编译通过并按可编程结果出线 | 单次 |
| ArchiveVcsPort.remove_worktree() | 异常 | `Err` 面契约：`String` 错误串（生产实现的路径 + 手动指引组装内容由 infra/vcs 真件节断言——port 面只锚签名与透传） | 单次 |
| ArchiveVcsPort.remove_worktree() | 边界 | port 面零探测方法：在场探测驻调用方 fs 判定（trait 无 probe / exists 方法——编译面锚定，测试假件仅实现既有方法族 + remove_worktree 即编译通过） | 单次 |
| ArchiveVcsPort 契约 | 边界 | trait 对象 `Send + Sync` 契约持衡（既有断言面随新方法扩展不破） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ArchiveVcsPort` 测试假件 | 进程内假件实现 trait 全方法族（沿既有 port_test 假件形态：可编程结果 + 调用捕获）——trait 为被测 API 显式注入依赖 | 清理面构造、Err 面契约、契约边界 |

### packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs

#### 待测功能

- ProcessArchiveVcs.remove_worktree(): 委托既有 `git::remove_worktree` 单点（与 ProcessWorktree 共点、零新 spawn）；`Err` 经 map_err 附路径与 `git worktree remove --force {path}` 手动指引

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ProcessArchiveVcs.remove_worktree() | 正向 | 真实 git tempdir：add_worktree 建位 → remove_worktree Ok → worktree 目录移除、`git worktree list` 不再列出、主仓无损（AC-5） | 单次 |
| ProcessArchiveVcs.remove_worktree() | 正向 | 转发对拍：经 `&dyn ArchiveVcsPort` 调用与 `git::remove_worktree` 自由函数效果逐字等值（沿既有清单 / 补丁转发测试形态——单点复用防漂移） | 单次 |
| ProcessArchiveVcs.remove_worktree() | 异常 | 目标非 worktree 目录 / 缺席路径 → `Err` 含 worktree 路径字面量与「git worktree remove --force {path}」手动指引（D6 Err 面） | 单次 |
| ProcessArchiveVcs.remove_worktree() | 边界 | branch 零触碰：remove 后 `git branch` 列表仍含 `change/<name>`（`worktree remove --force` 不动分支——AC-7） | 单次 |
| ProcessArchiveVcs.remove_worktree() | 边界 | 脏 worktree（未提交 `node_modules` 形态目录）经 `--force` 仍可移除（D1：--force 带走 bootstrap 脏面——不带 force 恒失败的语义锚） | 单次 |
| ProcessArchiveVcs 构造 | 正向 | `new()` / `default()` 等值构造既有断言随新方法持衡 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 git tempdir 仓 + 真实 spawn（`tempfile::TempDir` 隔离、worktree 落位与主仓分置——沿既有 lib_test / git_test 真件形态；git 子进程为被测本体不 mock） | 全节 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（remove_worktree 为既有 pub(crate) 单点，本次仅补 doc 注释、零逻辑改动）——条目为 design.md 变更清单声明的单点回归锚面。 -->

- git::remove_worktree(): 既有 `git worktree remove --force <path>` 单点回归持衡——create 补偿面之外由归档 Cleanup 步经 `ProcessArchiveVcs` 复用（零逻辑改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| git::remove_worktree 既有单点 | 正向 | 既有回归持衡：add 后 remove（--force）→ worktree 目录移除、主仓无损、同路径可再 add（零逻辑改动——新消费方 ProcessArchiveVcs 复用同一单点不改变行为） | 单次 |
| git::remove_worktree 既有单点 | 边界 | remove 后 branch `change/<name>` 留存（既有注释语义断言面：worktree remove 只回收目录与登记、分支留存） | 单次 |
| git::remove_worktree 既有单点 | 异常 | 缺席路径 remove 失败 Err 原文语义持衡（回归锚——Err 面行为不因 doc-only 改动漂移） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 git tempdir 仓直驱（沿既有 git_test 真件装置） | 全节 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（RunNodeRecord.step 仅字段 doc 注释随动，表结构零变化、零迁移）——条目为 design.md 变更清单 / 类型定义表声明的词汇随动面。 -->

- RunNodeRecord.step: `RunStepKind` 词汇白名单式扩展 + `cleanup`（二十值全词汇封闭集；旧库可读零迁移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| RunNodeRecord.step 词汇 | 正向 | `RunStepKind::Cleanup` 节点构造 / 序列化 / 反序列化往返等值（serde 线词 `"cleanup"` 白名单式扩展可用） | 单次 |
| RunNodeRecord.step 词汇 | 边界 | 既有十九值词汇断言扩为二十值（len 与词面互异持衡）；旧串（既有十九值）round-trip 兼容读不破（零迁移面） | 枚举遍历 |
| RunNodeRecord.step 词汇 | 异常 | 越集词反序列化拒绝持衡（封闭集防线不松动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：模型类型 serde 直驱（真件往返沿既有 model_test 形态） | 全节 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（run 节点读面仅 doc 十九值 → 二十值注释随动，零代码改动）——条目为 design.md 变更清单声明的读面行为持衡面。 -->

- run 节点读面: cleanup 词节点读写回环原样出线（二十值全词汇零过滤——doc 与行为一致）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| run 节点读面 | 正向 | 写入含 cleanup 词节点 → 读面原样出线（零过滤——doc 注释与行为一致） | 单次 |
| run 节点读面 | 边界 | 既有 run 读面断言随二十值全量持衡（词面扩位不改变行集完整性） | 枚举遍历 |
| run 节点读面 | 异常 | 既有 StoreError 显式记因面持衡（回归锚——零代码改动不漂移） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 tempfile Store 读写回环（沿既有 store_test 真件形态） | 全节 |

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——normalizeKind / stepWord / groupOf 为模块私有归一函数（design.md 变更清单声明），经既有导出面（unifiedRunNodes / runStepNodes / nodeIdentityKey）与内联 fixture 直测。 -->

- normalizeKind(): `cleanup` 为 identity 词走 `default` 分支直返（snake / camel 同形零显式 case；词汇注释 19 → 20）
- stepWord(): 反向归一 `cleanup` 回 snake 词面（round-trip 恒等）
- groupOf(): git 组判定补 `step === 'cleanup'`（cleanup 归 git 分类）
- nodeIdentityKey() / unifiedRunNodes(): 既有导出面——cleanup 节点身份键词面（`{phase}:{attempt}:cleanup:{instanceNo}`）与双源合成归一对新词透明

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| normalizeKind | 正向 | `'cleanup'` → `'cleanup'`（identity 词走 default 分支直返——snake / camel 同形零显式 case，与 commit / rebase / merge / finalize / preflight / seal 先例一致） | 单次 |
| normalizeKind / stepWord | 边界 | 二十值全量对拍表更新（snake ↔ camel 期望表补 cleanup 行，词汇漂移即断）；round-trip `stepWord(normalizeKind(word))` 恒回 snake 词 | 枚举遍历 |
| groupOf | 正向 | `'cleanup'` 归 git 分类（git 臂补值——经 runStepNodes / 节点分组出线断言，与 commit / rebase / merge / finalize 同组） | 单次 |
| groupOf | 边界 | 二十值逐词分组穷举（cleanup 入 git、既有归组零漂移——agent / gate / toolStep 三类持衡） | 枚举遍历 |
| nodeIdentityKey | 正向 | cleanup 节点身份键含 stepWord 词面（`{phase}:{attempt}:cleanup:{instanceNo}`——与后端边表端点逐字可比） | 单次 |
| stepFromRecord / unifiedRunNodes | 正向 | 库读史行 step='cleanup'（RunStepKind 线词）→ 统一视图节点归一出线（双源合成对新词透明） | 单次 |
| stepFromRecord / unifiedRunNodes | 异常 | 既有越集词拒绝 / 载荷缺省边界持衡（回归锚——归一面不因扩词松动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯函数直测 + 内联 fixture（stepRow / runStepRow 构造器沿既有形态），零 react 零进程边界 | 全节 |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——STEP_LABEL 为组件内私有常量表（design.md 变更清单声明），经 RunStepNode 组件渲染面直测。 -->

- STEP_LABEL: `Record<ChangeStepKind, string>` 封闭联合补 `cleanup: 'worktree 清理'` 词条
- RunStepNode: cleanup 节点渲染——词条出线（`data-testid="run-step-kind"`）、git 组徽章与 detail 载荷降级面持衡

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| STEP_LABEL | 正向 | `cleanup` → 「worktree 清理」词条渲染（`data-testid="run-step-kind"` 出线断言——新键值对拍） | 单次 |
| STEP_LABEL | 边界 | `Record<ChangeStepKind, string>` 封闭联合键集恰等二十词零缺键（编译期穷尽 + 运行时穷举兜底断言 19 → 20 更新） | 枚举遍历 |
| git 分类徽章 | 正向 | cleanup 节点归 git 组徽章（group='git' 与 commit / rebase / merge / finalize 同列可辨——AC-9 观测半边） | 单次 |
| RunStepNode 渲染 | 边界 | 载荷缺省形态（detail=null / sessionId=null）既有降级渲染持衡（扩键不改变缺省面——回归锚） | 单次 |
| RunStepNode 渲染 | 异常 | 既有异常载荷回归面持衡（零渲染崩溃，回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：组件直渲染（沿既有 run-step-node.test 形态，零 IPC / 零 react 依赖注入面） | 全节 |

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——ArchiveMergeNotice 为组件内文案块（design.md 变更清单 / D8 声明），经组件直渲染断言。 -->

- ArchiveMergeNotice(): 文案改写（D8）——「将执行提交 + 合入主仓当前分支：{mergeTarget}；收口后自动移除 worktree（分支 {branch} 保留）」；worktree 在场才渲染（legacy 零清理告知）
- 归档确认卡片: 头动作清单句补「自动移除 worktree」；inform + confirm 语义不变（警告不阻断）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ArchiveMergeNotice | 正向 | worktree 记录在场（preflight.worktree ≠ null）→ 告知文案含「将执行提交 + 合入主仓当前分支：{mergeTarget}」与「收口后自动移除 worktree（分支 {branch} 保留）」（branch 取 preflight.branch——AC-8 / D8） | 单次 |
| ArchiveMergeNotice | 边界 | legacy（preflight.worktree === null）→ 零清理告知行（既有条件渲染不变——归档卡片其余面照常） | 单次 |
| 归档确认卡片 | 正向 | 卡片头动作清单句含「自动移除 worktree」；卡片仍 inform + confirm 语义（警告不阻断：proceed / stop 按钮行为与 confirm 载荷回归不变） | 单次 |
| 归档确认卡片 | 异常 | archive 载荷缺省（archive === null）→ 降级渲染持衡（既有边界回归不回退） | 单次 |
| 归档确认卡片 | 边界 | 跳过同步勾选 / 普通相位确认互斥面既有断言持衡（文案改写不改变交互载荷——回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：组件直渲染 + preflight / run fixture 内联（沿既有 run-control-panel.test 形态；confirm 回调用 `vi.fn()` 捕获——测试替身非进程边界 mock） | 全节 |

## 不可测试项

| 模块 / 事项 | 原因 |
|-------------|------|
| `packages/desktop/src/types/generated/bindings.ts` | 生成物（specta 管线自动再生，非手写测试面）：`RunStepKind` / `ChangeStepKind` TS 联合类型 + `"cleanup"` 由 `pnpm -C packages/desktop run bindings:export` 产出；漂移由 `bindings:check` 静态守卫拦截（静态步骤由桌面端执行，agent 内不重复）。词面正确性的可执行半边由 Rust 侧 state_test / run_history_test 二十值断言与前端 run-state.test 对拍表承接。 |
| `packages/desktop/package.json`（version 0.5.8 → 0.5.9） | 版本交付面零行为可断言：AC-11 由发布流水线 / 静态核对（`src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 仍 2.10.44、`vp test` / `client:check` / knip 全绿——静态门，桌面端执行）。 |
| create.rs 私有常量 `ACTIVE_CHANGE_LIMIT` / `WORKTREE_DIR_SLACK` | 非导出面（create.rs 私有 `const`，D5）：不以测试为目的新增导出（No test-only exports 纪律）；定值与派生门槛（= 7）经拒绝 / 边界用例的行为断言（文案含计数与上限）覆盖。 |
| AC-10 「裸 archive_change 漏清留痕」的 spec 边界条款 | 留痕主体是 openspec delta 文本（desktop-change-archive V1 边界条款）与 D2 决策记录，非可执行代码；可执行半边为既有 `archive_change` 裸双写命令零 vcs 依赖语义持衡（mod_test 既有回归不回退，本轮零新增断言面）。 |
| `plugins/dev-team` 全部 | 「不要修改」清单项（零改动，版本 2.10.44）——不属被测范围。 |




