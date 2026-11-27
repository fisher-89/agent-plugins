# desktop-change-state-store Specification (Delta)

## ADDED Requirements

### Requirement: `ChangeRecord.exec_root` 执行根列与 native_model v5 升级链

`ChangeRecord` SHALL 增一列 `exec_root: Option<String>`（绝对路径），承载该 change 的**执行根**——workspace root 为 git 仓库子目录时，执行根为 worktree 落位目录（host）内的对应子目录（`worktree/<rel>`）；`None` SHALL 表示「与 `worktree` 同值」（legacy 记录与根 workspace 两种形态同形，二者在记录面不可区分、在读面亦无需区分）。

约束：

- **零冗余写入**：根 workspace（`rel = ""`）建档 SHALL NOT 写该列（落库为 `None`）——由此得一条可断言的不变量：*git 根 workspace 的建档记录与拆分前逐字相同*。
- **列形态**：`exec_root` SHALL 与 `worktree` 列同形态——绝对路径、同一路径渲染规则（与写面 `Path` 的字符串形态一致，MUST NOT 额外归一大小写 / 分隔符 / 尾分隔符），使读面零路径拼接、零分隔符考古。
- **归档后原样保留**：worktree 被 Cleanup 移除、change 转 archived 后，两列 SHALL 原样保留（历史锚，与 `worktree` 列同口径，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；MUST NOT 在归档路径清零 `exec_root`（清零会让存量记录形态分叉，且归档 read 面本就按 status 回落主仓根）。
- **访问器单点**：执行根的解析 SHALL 收口为一个访问器（`ChangeStateRecord::exec_root`），语义为「`exec_root` 在案即取值，否则回落 `worktree`」；消费点 MUST NOT 各自写 `exec_root.or(worktree)` 形态的回落表达式（散落回落 = 第二份语义）。
- **防漂移守卫**：`exec_root` 不是 `worktree` 的真后代（陈旧行 / 手改库 / 跨盘异常）SHALL 以 stderr 留痕（`[workflow]` 前缀）并回落 `worktree`——失败形态退回「执行根 = 落位目录」，MUST NOT 产出 worktree 之外的越界路径；`worktree` 为 `None` 时该列 MUST NOT 单独生效（返回 `None`，由调用方回落主 workspace root）。
- **版本演进**：加列 SHALL 走 native_model 版本链（`ChangeRecord` v4 → **v5**，`from = ChangeRecordV4`；现役形态抄为 `ChangeRecordV4`，其自身 `from = ChangeRecordV3` 保留、V3 双向 `From` 保留），新列带 `#[serde(default)] Option<String>`。**MUST NOT** bump `WORKSPACE_STORE_FORMAT_VERSION`——库级格式版本的门是「低版本删库重建」，一动即令全部 workspace 库（会话 / 流程史 / provider）作废；加列属模型版本面，与格式版本面职责分立。
- **零迁移脚本**：旧行（v4 编码）SHALL 由 native_model 升级链解码并读出 `exec_root = None`，无手工迁移步骤、无第二形态解码层。
- **写路径自动化**：该列 SHALL 只在两处映射单点被搬运——`change_state()`（记录 → 中性快照）与 `create_change_record`（中性快照 → 记录）；其余 change 写路径（读记录 → 改一字段 → upsert）MUST NOT 逐处补列（读改写在 `ChangeRecord` 上原位进行，新列自动随行）。

#### Scenario: 子目录记录两列并存

- **WHEN** 在子目录 workspace 建档 change 后关闭并重开 workspace 库查询
- **THEN** `worktree` 为落位目录绝对路径、`exec_root` 为 `worktree/<rel>`，两字段逐字与写入一致；`exec_root()` 访问器返回 `exec_root` 值

#### Scenario: 根 workspace 零新列

- **WHEN** 在 git 根 workspace 建档 change 并查询记录
- **THEN** `exec_root` 读出 `None`；`exec_root()` 访问器回落返回 `worktree` 值；落库形态与本变更前同形态（该列以 `None` 缺省写入，行为面逐字等价）

#### Scenario: v4 存量行升级读出 None

- **WHEN** 打开含 v4 形态 `ChangeRecord`（无 `exec_root` 列）的存量 workspace 库并查询
- **THEN** 解码成功零报错，`exec_root` 读出 `None`、其余字段原值；无迁移步骤、无手工回填；`WORKSPACE_STORE_FORMAT_VERSION` 不变，库不触发作废重建

#### Scenario: 越界 exec_root 回落并留痕

- **WHEN** 库内记录被外部改写为 `exec_root` = `worktree` 自身 / `worktree` 的祖先 / 与 `worktree` 无关的绝对路径
- **THEN** 访问器返回 `worktree` 值、stderr 有 `[workflow]` 留痕；全链路径（会话 cwd / Layout / 检查器）均落在 `worktree` 内，无越界路径产出；访问器不 panic、不 `Err`

#### Scenario: 两处映射单点之外零补列

- **WHEN** 审查 change 域其余写路径（开相 / 落账 / 回溯 / 归档翻转 / 标定）
- **THEN** 均以「读记录 → 改一字段 → upsert」形态在原记录上操作，`exec_root` 原样留存；源码无逐处补列的赋值语句（`grep` 命中的只有访问器、两处映射与 DTO 透出点）

## MODIFIED Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护七个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`id`（String 主键，建档铸出的稳定唯一标识——UUID 形态，身份锚，终身恒定不复用）、`name`（String，change 名，恒裸名——可变属性，MUST NOT 作身份键）、`title`（String，人类可读标题，恒非空——创建 / 升级默认 = name，promote 路径继承 explore.title）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change；清理后原样保留作历史锚，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）、`exec_root`（Option：执行根绝对路径；None = 与 `worktree` 同值——legacy 记录与根 workspace 同形；workspace root 为仓库子目录时 = `worktree/<rel>`，见 desktop-change-worktree「子目录 workspace 的执行根对齐」）、`base_commit`（Option：创建基线 fork 点）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change_id`（二级索引，指向 `ChangeRecord.id`）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change_id`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution` | `mutation_test`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键）、`change_id`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）；
- `RunNodeRecord`：`run_id`（二级索引）、`change_id`、稳定身份键 `(phase, attempt, step, instance_no)`、`step_kind`（**全词汇封闭集二十一值**：`executor` | `evaluator` | `decision` | `static_check` | `test_execution` | `mutation_test` | `clean` | `phase_start` | `phase_log` | `verdict_gate` | `retry_gate` | `whitelist_gate` | `preflight` | `commit` | `rebase` | `resolve_conflicts` | `merge` | `spec_sync` | `cleanup` | `seal` | `finalize`）、`status`、`session_id`（Option，WorkerAgent 步）、`detail`（Option，有界）、`output_ref`（Option，`ExecOutput::Recorded { exec_id }` 引用）、时间戳；
- `RunEdgeRecord`：`run_id`（二级索引）、`from_node_key`（二级索引）、`to_node_key`、`edge_kind`（封闭集：`forward` | `feedback` | `retry` | `backtrack`）、时间戳。

`ChangeRecord` 的身份换锚（name 主键 → id 主键 + name 属性）SHALL 经 native_model 版本段演进（表名含模型版本与主键段，旧形态表对新读面不可见），旧形态库由 store 格式版本机制整体作废重建；MUST NOT 保留 name → id 的迁移链、解码链或手工迁移步骤。`title` 字段 SHALL 经 native_model 版本升级（v3→v4 decode-only）原地演进；`exec_root` 字段 SHALL 经 v4→v5 原地演进（见「`ChangeRecord.exec_root` 执行根列与 native_model v5 升级链」），且其 `None` 形态 MUST 覆盖「legacy 记录」与「git 根 workspace」两种情形。`worktree` / `exec_root` 两字段均为执行锚引用，MUST NOT 反向影响库身份锚定。

**执行图 model 重设计（不计历史兼容）**：`RunStepRecord`（`seq` + 五词汇 + 10→5 过滤）整体退役，`RunNodeRecord` / `RunEdgeRecord` 取代之；其后 step_kind 全词汇由十值扩为十八值（新增 8 个归档步词），再扩为十九值（新增 `mutation_test` 突变门禁步词），worktree 清理变更再扩为二十值（新增 `cleanup` 归档清理步词，白名单式扩展），基线检查变更再扩为二十一值（新增 `clean` initialize 基线恢复步词，白名单式扩展），归档节点复用 `RunNodeRecord`（零新表），MUST NOT 保留归档独立落库表。**枚举变体 SHALL 仅尾部追加**——`RunNodeRecord.step` 经 bincode 按下标编码，中途插入 / 重排会令存量节点行步词整体错位（`clean` 中插事故已由一次性数据修复纠正）。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`；磁盘 change 目录只承载 markdown 产物与报告工件。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（含 initialize 基线检查相位与 archive 相位空 checklist 落账）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunNodeRecord / RunEdgeRecord 逐字段与写入一致（含 title、三会话槽位、worktree 与 base_commit、initialize 基线步词 clean、归档 9 个步词（含 cleanup）与 mutation_test、时间戳原值），状态查询不依赖任何磁盘 workflow.json

#### Scenario: 双向墙机械可验

- **WHEN** 扫描 desktop 全部源码对 `workflow.json` 的读与写触点
- **THEN** 零触点（测试夹具内的字面量除外）；parse 模块已删除，desktop 不再解析该文件

#### Scenario: 历史 CLI change 零发现零触碰

- **WHEN** 主仓存在仅有 workflow.json 与 markdown 产物、无 db 记录的存量 change 目录
- **THEN** 目录与其 workflow.json 字节原样未动；清单不入列、详情不可达（零发现），不报错、不建档

#### Scenario: 旧形态数据零可达与解码链退役

- **WHEN** 打开含旧形态 `ChangeRecord` 行（name 主键）或旧 `RunStepRecord` 行的存量 workspace 库并查询
- **THEN** 打开成功且零报错；作废重建后新库为空（旧 change / run / 会话 / 步数据零残留）；模型源码零 name → id 迁移链 / `RunStepRecord` → `RunNodeRecord` 迁移链 / 解码链 / 手工迁移步骤

#### Scenario: worktree 字段不反噬库身份

- **WHEN** 某 workspace 存在带 worktree 记录的 change，重开其 workspace 库
- **THEN** db 文件路径仍由 workspace root 身份派生，worktree 路径仅作为记录字段被读取，从不参与库路径派生

#### Scenario: 存量 v3 记录 title decode-only 升级

- **WHEN** 打开含 v3 形态 `ChangeRecord`（无 `title` 字段）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`title` 读出 = `name`，无手工迁移步骤、无 legacy 迁移层；新建档记录 `title` 按写入回读一致

## Module Contract

### 模块增量：`exec_root` 列与 v5 升级链（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/infra/store/src/model.rs`（`ChangeRecord`） | 模型版本 | `#[native_model(id = 9, version = 5, from = ChangeRecordV4)]`；现役形态增 `#[serde(default)] pub exec_root: Option<String>`；历史形态 `ChangeRecordV4`（`from = ChangeRecordV3`，双向 `From` 保留）与 `ChangeRecordV3` 原样在案 |
| `crates/infra/store/src/model.rs`（`ChangeRecord::new`） | 构造 | 签名增 `exec_root: Option<String>`（调用点单处：写面 create 建档） |
| `crates/infra/store/src/store.rs`（`change_state` / `create_change_record`） | 映射单点 | 两处各补一列搬运；其余 change 写路径（`start_change_phase` / `log_change_phase` / `apply_change_backtrack` / 归档翻转 / 启动标定）零改动（读改写在 `ChangeRecord` 上原位进行） |
| `crates/core/workflow/src/state.rs`（`ChangeStateRecord`） | 中性快照与访问器 | 增 `pub exec_root: Option<String>`；`pub fn exec_root(&self) -> Option<&str>`（回落 + 真后代守卫 + `[workflow]` 留痕） |
| `crates/infra/store/src/model.rs`（`WORKSPACE_STORE_FORMAT_VERSION`） | 库格式版本 | **零改动**（仍为 3）：加列属模型版本面，格式版本一动即全库作废重建 |
| `crates/infra/store/src/model_test.rs` | 版本链覆盖 | v4→v5 往返、V4 行升级读出 `None`、字段集 / 编码形态直证 |
| `crates/core/workflow/src/state_test.rs` | 访问器覆盖 | 回落 / 命中 / 越界三条 + 留痕可断言 |
