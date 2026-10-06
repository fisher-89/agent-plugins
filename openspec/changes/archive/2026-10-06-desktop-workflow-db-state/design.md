# 设计: desktop-workflow-db-state

> **变更**: desktop-workflow-db-state
> **日期**: 2026-10-06

---

## 提案与规格同步状态

`proposal.md` 与 `openspec/changes/desktop-workflow-db-state/specs/**`（10 个 spec delta）已由提案阶段写入并通过评审，本设计不重复其内容、不将其列为待办。本设计在 proposal 五项待决问题的基础上定稿（见「关键设计决策」与「待决问题」）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 状态缝与中性类型（新） | `ChangeStateStore` port trait + 中性状态类型（port 流量词汇，时间戳 i64 unix millis）+ `StoreFault` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs`（新） | crate 内 `model`（`Verdict` / `ChecklistItem`） | Rust trait object，serde/specta derive（零 native_db） |
| change 状态四记录模型 | `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 持久化形状与 native_model 版本治理 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | native_db / native_model / serde | workspace 库四模型（id 9–12，默认 bincode） |
| store change 域操作面 | 建档查重、`log_change_phase` 单事务原子、backtrack、amend、status 翻转、步骤追加与查询；workspace 组注册 4→8 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `model`、`workflow::state`（中性类型） | redb 写事务 + max+1 主键惯例 |
| port 适配器（新） | `impl ChangeStateStore for Store`：记录 ↔ 中性类型映射委托 | `packages/desktop/src-tauri/crates/infra/store/src/change_port.rs`（新） | `store`、`workflow::state` | trait 实现胶水，零新命名导出 |
| 相位路由状态机 | `phase_next` 只读路由（db 读源；prompt 插值 + 白名单 + 重试上限 + SessionAnchors） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | `state`、`phase_table` | 纯函数 + 进程内锚点 |
| 开相写操作 | `phase_start` 落库重写（表位校验前置保留，持久化经 port） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs` | `state`、`phase_table` | sync 签名，零 tokio |
| 落账写操作 | `phase_log` 落库重写（verdict 推导 / 长度 / 表位 / active_phase 匹配校验前置保留） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | `state`、`model` | sync 签名，零 tokio |
| 回溯写操作 | `backtrack` 落库重写（白名单二次校验前置保留；stale 闭包计算迁入） | `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs` | `state`、`phase_table` | sync 签名，零 tokio |
| 决策挂账写操作 | `decision_log` amend 落库重写（幂等定点改写，D9） | `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs` | `state` | sync 签名，零 tokio |
| create 写操作 | 目录 + explore.md + db 建档三合一双写（建档先行 + 补偿） | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | `foundation::layout`、`state` | sync，kebab-case 校验保留 |
| archive 写操作（新） | 归档双写：目录改名（日期前缀）→ db status 翻转 + 续半边重试分支 | `packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs`（新） | `foundation::layout`、`state` | sync，fs + port |
| 列表查询 | db 记录 ∪ 磁盘目录去重并集、按月分组、文档形态入列 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | `state`、`foundation::layout` | 纯读 DTO（ISO 串 + null） |
| 详情查询 | PhaseRecord / ChecklistItemRecord 重组 9 站流水线；`inventory` / `fileLog` / `unparsable` 字段删除 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `state`、`artifacts`、`foundation::layout` | 纯 derive DTO，golden 守卫 |
| change 定位 | active 树精确名 + archive 树日期前缀后缀匹配（db 名不变可达） | `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` | `foundation::layout` | 纯路径推导 |
| 产物发现输入面演进 | `discover_artifacts` / `read_artifact` 入参 `(Inventory, Option<&Workflow>)` → db 相位条目切片 | `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs` | `state`、matcher 注册表 | 信封机制不变 |
| 域类型收敛（新） | `Verdict` / `ChecklistItem` 自退役的磁盘模型文件迁出单点 | `packages/desktop/src-tauri/crates/core/workflow/src/model/domain.rs`（新） | serde / specta | 纯域类型 |
| 编排工具步 | `LocalToolSteps` 增 store 缝与 run 级 run_id；七臂命令包络 StepRecord 审计落库 | `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `workflow::write`、`workflow::state`、checks port | ToolStepPort 直调面不变 |
| 编排快照源 | `FsSnapshot` → `StoreSnapshot`：详情读源改 db（经 queries） | `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | `workflow::queries`、`workflow::state` | WorkflowSnapshotPort 实现换血 |
| run 控制命令组 | 发起前置校验改 db 建档校验；组合根注入 store 缝 | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `WorkspaceStores`、orchestration、workflow | 三件事薄包装 |
| change 域命令组 | list / detail / create / read_artifact 接线 db 读面；新增 `archive_change` 薄命令 | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `WorkspaceStores`、workflow | 读命令无状态 + 记录面带 State |
| 命令注册宏 | `archive_change` 进 invoke_handler 清单 | `packages/desktop/src-tauri/src/commands/mod.rs` | 各命令组 | tauri::generate_handler |
| 清单视图退役面 | `InventoryBadge` / unparsable 标注删除；文档形态条目照常入列 | `packages/desktop/src/views/changes/change-list-view.tsx` | bindings 类型、Badge | React |
| 详情视图退役面 | 代际徽章、损坏警示条、`WorkflowPanel` 删除；建档两态分流 | `packages/desktop/src/views/changes/change-detail-view.tsx` | bindings 类型、flow 组件 | React |
| 抽屉二分节 | 右列三分节 → 二分节（本站文档 \| eval report+checklist），文件表节删除 | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `attachments`、renderers registry | React |
| 挂载分支收缩 | file_log 挂节点与 `scope='workflow'` 图外分支删除；文档挂列 + checklist 挂节点保持 | `packages/desktop/src/views/changes/flow/attachments.ts` | `types`、`graph` | 纯函数 |
| 前端类型收敛 | `FileLogEntry` 派生类型与 `nodeFiles` / `outsideFiles` 挂载状态退役 | `packages/desktop/src/views/changes/flow/types.ts` | bindings | TS 类型 |
| 类型跟随 | DTO 演进后重导出（`inventory` / `fileLog` / `unparsable` 类型消失，槽位与状态面类型在场） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | specta 生成物 |

不变组件（零触点核对结论）：`crates/core/workflow/src/write/phase_table.rs`（相位表 / prompt 模板 / `dependents` 单源不动）、`crates/core/orchestration/src/port.rs` 与 `walker.rs`（ToolStepPort / 薄图红线不动）、`crates/core/orchestration/src/verdict.rs` / `decision.rs` / `prompt.rs`、`crates/infra/watch/`（crate 保留）、`src/commands/db/`（信封 API 零改动覆盖新模型）、`crates/core/workflow/src/artifacts/` 其余 matcher 文件（仅 registry 输入面演进）、`plugins/dev-team` 全部与 openspec CLI（双向墙另一侧零改动）。

---

## 关键设计决策

| # | 问题（proposal 待决） | 定稿 | 理由 |
|---|----------------------|------|------|
| D1 | port 缝形态与落位 | trait `ChangeStateStore` 驻 `core/workflow/src/state.rs`（core 自持中性类型词汇：记录快照 + 写命令 + `StoreFault`，时间戳 i64 millis）；适配器 `impl ChangeStateStore for Store` 驻 `crates/infra/store/src/change_port.rs`；依赖方向 infra/store → core/workflow 单向（infra → core 为既有合法方向，store 已依 core/agent） | core/workflow 不命名任何 infra 类型（AC-2 硬约束）；port 流量类型与持久化记录分离——native_model 版本治理全留 infra，core 零 derive 负担；测试以进程内假件实现 trait（写面 / 编排测试既有 fake port 先例） |
| D2 | store 操作面签名词汇 | store.rs 的 change 域操作面直接以 `workflow::state` 中性类型为入参/出参，记录 ↔ 中性类型映射收 store 内单点；`change_port.rs` 仅 trait 委托 | 消除「命令结构体双份」；映射单点 = native_model 字段演进的唯一改动面 |
| D3 | native_model id 分配 | `ChangeRecord` id=9、`PhaseRecord` id=10、`ChecklistItemRecord` id=11、`StepRecord` id=12（version 均 1，默认 bincode codec） | 既有占用 1/2/4/5/6/7/8；id=3 为历史退役空缺不复用（避免与任何旧升级链歧义）；四模型字段平直无 flatten，不需要 `SerdeJsonCodec` |
| D4 | `ChecklistItemRecord` 打包键 serde | 沿 `SessionEventRecord.event_key` 先例：u128 打包键 `(phase_id as u128) << 64 \| item_index` 以十六进制字符串 serde 定制出线（信封 API JSON 可表达）；同相位内 item_index 升序 = evaluator 输出序 | 先例同构；serde_json 无 u128 数字面 |
| D5 | create 双写顺序与补偿 | **db 建档先行**（冲突双检查：目录已存在或 db 已有同名 active 记录均显式拒绝，检查全 IO 前置）→ `create_dir_all` + explore.md 写出 → **fs 失败补偿删除本次新插记录**（独立写事务，只删本次自插行，不触既有记录）；补偿亦失败则 Err 呈现残留记录名（不静默自愈） | 被禁破口是「目录在而记录缺」——建档先行使其仅在补偿再失败的双故障角落可能出现且显式报错；「已存在拒绝 + db 零建档」（AC-6）语义不受补偿影响（补偿只回滚本次写入） |
| D6 | archive 双写顺序与补偿 | **先目录改名**（active → archive 树 `YYYY-MM-DD-<name>`，目标已存在先查拒绝）**后 db 翻转**（status=archived + archived_at）；改名成功而翻转失败 → Err 呈现半完成态，重试经「archive 树定位命中 + db 仍 active」分支仅补 db 翻转（不重复改名） | 半完成态（目录已归档、db 仍 active）在 D7 的读时对账下展示正确；续半边分支只触 db 不触 fs；「无建档目录拒绝」先于一切变更 |
| D7 | 外部归档对账 | **读时以磁盘事实归组**：queries 定位条目来源树（active / archive）以磁盘目录存在性为准并据此分组展示；db `status` 仅为最近一次桌面写面事实，查询路径**不回写 db**（查询层纯读纪律） | 对 openspec CLI 归档（外部改名）零竞态、零写路径；两载体短暂不一致时展示向磁盘事实收敛，桌面 `archive` 命令是 status 唯一写口 |
| D8 | `SessionAnchors` 存续形态 | 保持进程内 `Mutex<HashMap<(change, run_id), usize>>` 不落库；基线语义从「eval 条目数」平移为「该 change 的 PhaseRecord 行数」（phase_next 读 db 后取 len），锚点基线即首见行数 → 首轮 round=1，与既往首见语义等价；重启后新 run 铸新锚点 | 锚点是 run 作用域组合根状态（每 run 一实例），db 化不改变其窗口语义；落库反而引入过期锚点恢复难题 |
| D9 | decision 槽位写入时机 | 沿 `decision_log` amend 单点不变：决策会话产生于该相位 fail 条目落账之后，经 `amend_change_decision_session` 定点改写该相位最新 PhaseRecord 的 decision 槽位列（幂等覆写，无条目 `NotFound` fault） | 现行语义平移，零新时机方案；spec「显式 amend 写面操作」选项即此 |
| D10 | StepRecord 摘要上限 | `summary` 截断上限 **500 字符**（`chars().count()` 口径，截断追加 `…（截断，共 N 字符）` 留痕）；`reference: Option<String>` 携 checks 报告目录（change 内相对路径）或会话 id 出全量 | 步骤行高频（每相位多行），500 字符审计粒度足够（发生了什么/结果如何/去哪看全量）；report ≤2000 口径留给 eval 面 |
| D11 | 前端归档入口形态 | **IPC 薄命令先行，本轮无前端入口**：`archive_change` 命令 + 写面 `archive` 落地；详情页归档按钮留后续变更 | 本变更前端面已含大面积退役工作；AC-7 只约束命令路径语义；UI 入口无阻塞性 |
| D12 | 历史 workflow.json 语料处置 | **删除**（13 个 fixtures 目录 v0-a…v3-a / corrupt-* 与对应 golden 快照，git 历史可考）；`fixtures/README.md` 改写为 db 种子语料矩阵说明；新语料 = db 种子构造器（经真实 store 操作面）+ 运行时合成磁盘产物树（沿 layout fixture 先例），文档形态样本含 workflow.json 惰性字节样本（desktop 不解析、字节不进投影） | parse 退役后旧夹具失去解析主体；保留即死语料。文档形态仍需 workflow.json 在场样本钉住「目录存在即 change、字节零读取」的发现语义，故保留少量惰性字节样本而非全删 |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  公共函数仅列模块级导出函数；私有函数（crate 内 pub(crate) 及以下）不列入。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 状态缝：`ChangeStateStore` trait（读半边 + 写半边）、中性状态类型（`ChangeStateRecord` / `ActivePhaseState` / `PhaseStateRecord` / `StepStateRecord` / `StepKind` / `ChangeStatus` / `PhaseStartState`）、写命令结构（`PhaseLogCommand` / `BacktrackCommand` / `StepCommand`）、`StoreFault` 错误面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs` | 归档双写操作：白名单外的第三条 fs+db 混合路径（D6 顺序与续半边分支） |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/domain.rs` | `Verdict` / `ChecklistItem` 自退役磁盘模型的迁出收敛单点 |
| `packages/desktop/src-tauri/crates/infra/store/src/change_port.rs` | `impl ChangeStateStore for Store`：记录 ↔ 中性类型映射委托（无具名导出） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/lib.rs` | 模块声明收敛：`parse` 移除、`state` 加入、`model` 导出面跟随 | crate 门面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | `persist` 移除、`archive` 加入；导出面跟随 | 写面门面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | 读源 workflow.json → port（`store: &dyn ChangeStateStore` 入参替换 `layout`）；锚点基线平移 PhaseRecord 行数（D8）；`LastResult.timestamp` → `Option<i64>` | 路由语义逐项对照保持（重试上限 / 白名单 / backtrack 检测 / prompt 插值） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs` | 持久化改 `store.start_change_phase`（attempt 在写事务内推导）；`PhaseStartOutcome.start_at` → `i64` | 相位表位校验保留在 core |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | 持久化改 `store.log_phase`（单事务原子）；verdict 推导 / report 长度 / 表位 / active_phase 匹配校验前置保留 | checklist 信封与会话槽位随行（`PhaseLogInput` 形状不变） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs` | 持久化改 `store.apply_backtrack`；stale 闭包计算（目标最新 pass + `dependents` BFS 全条目）自 persist 迁入本文件 | 白名单二次校验 / 表位 / reason ≤500 保留在 core |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs` | 持久化改 `store.amend_change_decision_session` | amend 幂等语义不变（D9） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 删除 workflow.json 初始文档写出；`create(layout, store, name, goal)` 三合一双写（D5）；`CreateOutcome.created` 改取 db `created_at` 日期 | kebab-case / goal 校验与 `Layout` 路径纪律保留 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` | `locate_change` 增 archive 树日期前缀后缀匹配（db 名 `foo` ↔ `YYYY-MM-DD-foo`） | 单分量名校验保留 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | 读源改 db ∪ 磁盘去重并集（同名 db 优先）；`ChangeSummary` 删 `inventory` / `unparsable`、增 `status: Option<ChangeStatus>` / `active_phase`；按月分组（db 取 `archived_at`，磁盘回退目录前缀；无前缀入未知时间组） | 发现语义 = db 记录 ∪ 磁盘目录（proposal 拍板） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 读源改 PhaseRecord / ChecklistItemRecord 重组；`ChangeDetail` 删 `inventory` / `unparsable` / `file_log` 三字段；`AttemptRecord` 形状不变（槽位三列直读透出 None → null）；时间出线 ISO + null 收本层单点（millis → RFC3339） | 文档形态 = 空流水线 + 产物清单，零 workflow.json 读取 |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs` | `discover_artifacts` / `read_artifact` 入参 `(Inventory, Option<&Workflow>)` → `&[PhaseStateRecord]`；eval-checklist 候选锚定从 `Workflow.eval` 下标平移到 PhaseRecord 序列 | matcher 注册表与信封机制零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs` | 导出收敛为 `domain`（`Verdict` / `ChecklistItem`）；磁盘类型导出删除 | 域类型门面 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 四记录模型落位（id 9–12，D3/D4）；打包键 serde 定制；构造器 | workspace 维度 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `workspace_models()` 4→8 注册；change 域操作面十方法（D2 词汇）；`(change, phase, attempt)` 写事务查重 | 信封 API 零改动 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `mod change_port` 挂载 | crate 门面 |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | `[dependencies]` 增 `workflow`（workspace 内路径依赖） | infra → core 单向边 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `LocalToolSteps` 增 `store: Arc<dyn ChangeStateStore>` 与 run 级 `run_id`；七臂命令包络 StepRecord 审计落库（成功 / 失败皆落）；写面调用签名跟随 | ToolStepPort 直调面不变（walker 对载体无感知） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | `FsSnapshot` → `StoreSnapshot { root, store }`：详情读源改 db（经 `change_detail`）；「unparsable 显式 Err」分支删除 | WorkflowSnapshotPort 契约不变 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 发起前置校验 1/2 改 db 建档校验（`find_change_record` + `phase_table`）；组合根向 `LocalToolSteps` / `StoreSnapshot` 注入 `for_root` store | 停止 / 应答 / 确认 / watch 命令不变 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | list / detail / read_artifact / create 接线 db 读面与建档写面（`for_root` store 入参）；新增 `archive_change` 薄命令（三件事 + `_with` 泛型测试缝）；组文档注释更新（记录面带 State） | 读命令 IPC 签名不变 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `archive_change` 进命令注册宏清单 | invoke_handler 面 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | `InventoryBadge` 与 unparsable「无法解析」标注删除；条目状态面改 `status` / `active_phase` 消费 | 文档形态条目照常入列 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 代际徽章映射、`UnparsableNote`、`WorkflowPanel` 删除；两态分流改建档判别（`status` 在场与否） | error / loading / 未找到降级页沿用 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 右列三分节 → 二分节（文件表节删除）；`FileLogTable` 引用与 `hasFileLog` 链路删除 | 左列会话区、双列滚动、单一交互入口不变 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | file_log 挂节点分支与 `scope='workflow'` 图外素材分支删除 | 文档挂列映射表 + eval-checklist 挂节点保持 |
| `packages/desktop/src/views/changes/flow/types.ts` | `FileLogEntry` 派生类型与 `nodeFiles` / `outsideFiles` 挂载状态字段删除 | 随 bindings 演进 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `bindings:export` 重导出（DTO 三字段删除 + 状态面 / 槽位类型跟随） | 一致性守卫拦截漂移 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md` | 语料清单表改写为 db 种子语料矩阵说明（覆盖面：多 attempt / backtrack stale / 槽位全缺 / 文档形态 / 坏行） | 随 D12 处置；新语料构造与 golden 再生成由 test-design / test-gen 阶段承接 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/`（目录：`mod.rs` / `detect.rs` / `detect_test.rs` / `workflow_file.rs` / `workflow_file_test.rs`） | parse 面整体退役（`detect_inventory` / `load_workflow` / `parse_workflow_file` / `WORKFLOW_FILE_NAME` 导出随目录删除） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs` | raw Value 保形改写层退役（写触点五类全部由 store 事务承接） |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | 磁盘模型退役（`Workflow` / `PhaseLog` / `ActivePhase` / `FileLogEntry` / `FileLogOp` / lenient_timestamp）；`Verdict` / `ChecklistItem` 迁 `model/domain.rs` |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/inventory.rs` | 代际模型退役 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/generation_parse_test.rs` | parse 集成测试随主体退役 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/`（v0-a / v0-b / v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt-* 共 13 个 workflow.json 语料目录） | 处置定稿 = 删除（D12）；新 db 种子语料由 test-design / test-gen 阶段重建 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/`（对应 13 份 change golden 快照） | 随夹具删除；新 golden 经显式重写流程再生成 |
| `packages/desktop/src/views/changes/flow/file-log-table.tsx` | fileLog 面板组件退役 |

### 公共函数 / API

<!-- identifier: 模块级导出函数 / trait 方法 / Tauri 命令；私有实现不列 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `ChangeStateStore`（trait） | `crates/core/workflow/src/state.rs` | 新增 | `pub trait ChangeStateStore: Send + Sync` | 落库 port 缝（方法逐条见下） |
| `ChangeStateStore::get_change` | `crates/core/workflow/src/state.rs` | 新增 | `fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault>` | 建档单查（None = 文档形态） |
| `ChangeStateStore::list_change_records` | `crates/core/workflow/src/state.rs` | 新增 | `fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault>` | 建档全量（列表并集 db 半边） |
| `ChangeStateStore::list_phase_records` | `crates/core/workflow/src/state.rs` | 新增 | `fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault>` | 相位评估史（checklist 已按打包键序内联） |
| `ChangeStateStore::list_steps` | `crates/core/workflow/src/state.rs` | 新增 | `fn list_steps(&self, change: &str, run_id: Option<&str>) -> Result<Vec<StepStateRecord>, StoreFault>` | 步骤审计枚举（时间序；run 可选圈定） |
| `ChangeStateStore::create_change_record` | `crates/core/workflow/src/state.rs` | 新增 | `fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault>` | 建档（同名 active 冲突 → `StoreFault::Conflict`） |
| `ChangeStateStore::start_phase` | `crates/core/workflow/src/state.rs` | 新增 | `fn start_phase(&self, change: &str, phase: &str, now: i64) -> Result<PhaseStartState, StoreFault>` | 写 active_phase；attempt 事务内推导 |
| `ChangeStateStore::log_phase` | `crates/core/workflow/src/state.rs` | 新增 | `fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault>` | 单事务原子（PhaseRecord + checklist + active_phase 清位 + 查重），返回 attempt |
| `ChangeStateStore::apply_backtrack` | `crates/core/workflow/src/state.rs` | 新增 | `fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault>` | 单事务：回跳标记 + stale 翻转传播 |
| `ChangeStateStore::amend_decision_session` | `crates/core/workflow/src/state.rs` | 新增 | `fn amend_decision_session(&self, change: &str, phase: &str, session_id: &str) -> Result<(), StoreFault>` | decision 槽位幂等挂账（D9） |
| `ChangeStateStore::set_archived` | `crates/core/workflow/src/state.rs` | 新增 | `fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault>` | status 翻转（归档 db 半边；主键 name 不变） |
| `ChangeStateStore::append_step` | `crates/core/workflow/src/state.rs` | 新增 | `fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault>` | 步骤审计行追加 |
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | `pub fn phase_next(store: &dyn ChangeStateStore, change: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | 读源 db；`SessionAnchors` 类型与位置不变 |
| `phase_start` | `crates/core/workflow/src/write/phase_start.rs` | 修改 | `pub fn phase_start(store: &dyn ChangeStateStore, change: &str, phase: &str) -> Result<PhaseStartOutcome, String>` | `PhaseStartOutcome.start_at: i64` |
| `phase_log` | `crates/core/workflow/src/write/phase_log.rs` | 修改 | `pub fn phase_log(store: &dyn ChangeStateStore, change: &str, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>` | `PhaseLogInput` / `PhaseLogOutcome` 形状不变 |
| `backtrack` | `crates/core/workflow/src/write/backtrack.rs` | 修改 | `pub fn backtrack(store: &dyn ChangeStateStore, change: &str, input: &BacktrackInput) -> Result<BacktrackOutcome, String>` | `BacktrackInput` / `BacktrackOutcome` 形状不变 |
| `decision_log` | `crates/core/workflow/src/write/decision_log.rs` | 修改 | `pub fn decision_log(store: &dyn ChangeStateStore, change: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String>` | amend 语义不变 |
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(layout: &Layout, store: &dyn ChangeStateStore, name: &str, goal: &str) -> Result<CreateOutcome, String>` | 目录 + explore.md + 建档三合一（D5） |
| `archive` | `crates/core/workflow/src/write/archive.rs` | 新增 | `pub fn archive(layout: &Layout, store: &dyn ChangeStateStore, change: &str) -> Result<ArchiveOutcome, String>` | 归档双写（D6） |
| `list_changes` | `crates/core/workflow/src/queries/list.rs` | 修改 | `pub fn list_changes(layout: &Layout, store: &dyn ChangeStateStore) -> ChangeList` | db ∪ 磁盘并集 |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, store: &dyn ChangeStateStore, name: &str) -> Option<ChangeDetail>` | db 读源 + 文档形态 |
| `locate_change` | `crates/core/workflow/src/queries/mod.rs` | 修改 | `pub fn locate_change(layout: &Layout, name: &str) -> Option<ChangeLocation>` | 增 archive 前缀后缀匹配 |
| `discover_artifacts` | `crates/core/workflow/src/artifacts/registry.rs` | 修改 | `pub fn discover_artifacts(change_dir: &Path, phases: &[PhaseStateRecord]) -> Vec<ArtifactDescriptor>` | 输入面演进 |
| `read_artifact` | `crates/core/workflow/src/artifacts/registry.rs` | 修改 | `pub fn read_artifact(change_dir: &Path, phases: &[PhaseStateRecord], kind: &str, source: &str) -> Option<ArtifactEnvelope>` | 同上 |
| `Store::create_change_record` | `crates/infra/store/src/store.rs` | 新增 | `pub fn create_change_record(&self, record: ChangeStateRecord) -> Result<ChangeStateRecord, StoreError>` | 建档 + 同名 active 查重（`StoreError`） |
| `Store::find_change_record` | `crates/infra/store/src/store.rs` | 新增 | `pub fn find_change_record(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreError>` | 主键直查 |
| `Store::list_change_records` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreError>` | 主键自然序 |
| `Store::start_change_phase` | `crates/infra/store/src/store.rs` | 新增 | `pub fn start_change_phase(&self, change: &str, phase: &str, now: i64) -> Result<PhaseStartState, StoreError>` | 单事务 active_phase 写入 |
| `Store::log_change_phase` | `crates/infra/store/src/store.rs` | 新增 | `pub fn log_change_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreError>` | 单事务原子 + `(change, phase, attempt)` 查重返回 `StoreError` |
| `Store::apply_change_backtrack` | `crates/infra/store/src/store.rs` | 新增 | `pub fn apply_change_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreError>` | 单事务回跳 + stale 传播 |
| `Store::amend_change_decision_session` | `crates/infra/store/src/store.rs` | 新增 | `pub fn amend_change_decision_session(&self, change: &str, phase: &str, session_id: &str) -> Result<(), StoreError>` | 最新条目定点改写 |
| `Store::set_change_archived` | `crates/infra/store/src/store.rs` | 新增 | `pub fn set_change_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreError>` | status 翻转（miss `Err`） |
| `Store::append_change_step` | `crates/infra/store/src/store.rs` | 新增 | `pub fn append_change_step(&self, command: &StepCommand) -> Result<(), StoreError>` | 步骤行追加（id max+1） |
| `Store::list_change_steps` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_change_steps(&self, change: &str, run_id: Option<&str>) -> Result<Vec<StepStateRecord>, StoreError>` | 按 change（可选 run）枚举 |
| `Store::list_phase_records` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreError>` | 相位行 + checklist 子行内联重组 |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 修改 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>, store: Arc<dyn ChangeStateStore>, run_id: String) -> Self` | run 作用域注入（组合根一次） |
| `StoreSnapshot::new` | `crates/core/orchestration/src/snapshot.rs` | 新增（替换 `FsSnapshot::new`） | `pub fn new(root: String, store: Arc<dyn ChangeStateStore>) -> Self` | db 读源快照 |
| `StoreSnapshot::detail` | `crates/core/orchestration/src/snapshot.rs` | 修改（port 实现） | `fn detail(&self, root: &str, change: &str) -> Result<ChangeDetail, String>` | WorkflowSnapshotPort 实现换血 |
| `archive_change` | `src/commands/changes/mod.rs` | 新增 | `pub fn archive_change(app: AppHandle, root: String, change: String) -> Result<ArchiveOutcome, String>` | IPC 薄命令（D11 无 UI 入口）；另有 `archive_change_with<R: Runtime>` 测试缝 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ChangeStateRecord` | `crates/core/workflow/src/state.rs` | 新增 | 建档中性快照：`name` / `workflow_type` / `created_at: i64` / `status: ChangeStatus` / `archived_at: Option<i64>` / `active_phase: Option<ActivePhaseState>` |
| `ActivePhaseState` | `crates/core/workflow/src/state.rs` | 新增 | `phase: String` / `attempt: u32` / `start_at: i64` |
| `PhaseStateRecord` | `crates/core/workflow/src/state.rs` | 新增 | 相位评估条目中性快照：`id` / `change` / `phase` / `attempt` / `verdict` / `report` / `checklist: Vec<ChecklistItem>`（适配器内联重组）/ `skipped` / `stale` / `backtrack_to` / `backtrack_reason` / 三会话槽位 / `start_at: Option<i64>` / `timestamp: i64` |
| `StepStateRecord` | `crates/core/workflow/src/state.rs` | 新增 | 步骤审计快照：`id` / `run_id` / `change` / `step_kind: StepKind` / `status` / `timestamp: i64` / `summary` / `reference: Option<String>` |
| `StepKind` | `crates/core/workflow/src/state.rs` | 新增 | 封闭集：`PhaseNext \| PhaseStart \| PhaseLog \| Backtrack \| DecisionLog \| StaticCheck \| TestExecution`（serde 小写线值） |
| `ChangeStatus` | `crates/core/workflow/src/state.rs` | 新增 | `Active \| Archived`（serde 小写线值；queries DTO 直接复用） |
| `PhaseStartState` | `crates/core/workflow/src/state.rs` | 新增 | `attempt: u32` / `start_at: i64` |
| `PhaseLogCommand` | `crates/core/workflow/src/state.rs` | 新增 | 落账写命令：change / phase / verdict / report / skipped / checklist / 三会话槽位 / `start_at: Option<i64>` / `timestamp: i64` |
| `BacktrackCommand` | `crates/core/workflow/src/state.rs` | 新增 | 回跳写命令：change / phase / to / reason / `stale_dependents: Vec<String>`（core 计算的 dependents 闭包） |
| `StepCommand` | `crates/core/workflow/src/state.rs` | 新增 | 步骤审计写命令：run_id / change / step_kind / status / summary / reference / timestamp |
| `StoreFault` | `crates/core/workflow/src/state.rs` | 新增 | port 错误面：`Db(String)` / `Conflict(String)` / `NotFound(String)` |
| `ArchiveOutcome` | `crates/core/workflow/src/write/archive.rs` | 新增 | `name: String` / `archived_date: String`（UTC `YYYY-MM-DD`） |
| `Verdict` / `ChecklistItem` | `crates/core/workflow/src/model/domain.rs` | 修改（迁移落位） | 自退役的 `model/workflow.rs` 迁入，形状与导出面不变（`state.rs` 与 DTO 的既有消费词汇） |
| `ChangeSummary` | `crates/core/workflow/src/queries/list.rs` | 修改 | 删 `inventory` / `unparsable`；增 `status: Option<ChangeStatus>` / `active_phase: Option<ActivePhase>`；`name` 语义注明为磁盘目录名（归档含日期前缀） |
| `ChangeDetail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | 删 `inventory` / `unparsable` / `file_log`；其余字段面不变 |
| `PhaseStartOutcome` | `crates/core/workflow/src/write/phase_start.rs` | 修改 | `start_at: OffsetDateTime` → `start_at: i64` |
| `LastResult` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | `timestamp: Option<OffsetDateTime>` → `Option<i64>` |
| `ChangeRecord` | `crates/infra/store/src/model.rs` | 新增 | 持久化记录（id=9）：`name` 字符串主键 / `workflow_type` / `created_at` / `status` / `archived_at` / `active_phase: Option<ChangeActivePhase>`（嵌套 struct 不落独立模型，先例 `SessionConfigSnapshot`） |
| `PhaseRecord` | `crates/infra/store/src/model.rs` | 新增 | 持久化记录（id=10）：`id` i64 主键（写事务 max+1）/ `change` 二级索引 / `phase` / `attempt` / `verdict` / `report`（内联 ≤2000）/ `skipped` / `stale` / `backtrack_to` / `backtrack_reason` / executor / evaluator / decision 三槽位 / `start_at: Option<i64>` / `timestamp: i64` |
| `ChecklistItemRecord` | `crates/infra/store/src/model.rs` | 新增 | 持久化记录（id=11）：打包主键 `(phase_id as u128) << 64 \| item_index`（十六进制字符串 serde）/ `phase_id` 二级索引 / `item` / `pass` / `evidence` |
| `StepRecord` | `crates/infra/store/src/model.rs` | 新增 | 持久化记录（id=12）：`id` i64 主键（max+1）/ `change` 二级索引 / `run_id` / `step_kind` / `status` / `timestamp` / `summary` / `reference` |
| `ChangeActivePhase` | `crates/infra/store/src/model.rs` | 新增 | `ChangeRecord` 嵌套结构：`phase` / `attempt` / `start_at: i64` |
| `FileLogEntry`（前端） | `packages/desktop/src/views/changes/flow/types.ts` | 删除 | file_log 前端镜像类型随 DTO 字段删除 |
| `Inventory`（bindings） | `packages/desktop/src/types/generated/bindings.ts` | 删除 | 代际类型随 DTO 字段删除（重导出后消失） |

### 配置

<!-- 本变更实现面不改任何配置键。AC-10 的 packages/desktop/package.json version 0.4.12 → 0.4.13 于归档时执行（proposal 明定），不属实现清单；src-tauri/Cargo.toml 版本不随动。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ChangeRecord`（id=9） | `name: String`（主键，change 名，身份主键）、`workflow_type: String`、`created_at: i64`、`status: ChangeRecordStatus`（active \| archived）、`archived_at: Option<i64>`、`active_phase: Option<ChangeActivePhase{phase, attempt, start_at}>` | 1:N `PhaseRecord`（按 `change` 名关联，名称引用非外键约束）；主键 name 不随归档改名变 | workspace 库（app-data `workspaces/{可读段}-{hash}.redb`），native_model bincode |
| `PhaseRecord`（id=10） | `id: i64`（主键，写事务 max+1）、`change: String`（非唯一二级索引）、`phase: String`、`attempt: u32`、`verdict`（pass \| fail）、`report: String`（≤2000 内联）、`skipped: bool`、`stale: bool`、`backtrack_to: Option<String>`、`backtrack_reason: Option<String>`、`executor_session_id` / `evaluator_session_id` / `decision_session_id: Option<String>`、`start_at: Option<i64>`、`timestamp: i64` | N:1 `ChangeRecord`；1:N `ChecklistItemRecord`（`phase_id`）；三槽位 → `SessionRecord.id` 同库 join（库内一等查询，禁跨库引用惯例下天然收敛） | 同上 |
| `ChecklistItemRecord`（id=11） | 打包主键 `(phase_id as u128) << 64 \| item_index`（u128，hex 字符串 serde）、`phase_id: i64`（非唯一二级索引）、`item: String`、`pass: bool`、`evidence: String` | N:1 `PhaseRecord`；同相位内主键自然序 = item_index 升序 = evaluator 输出序 | 同上；独立 native_model 版本链（evidence 长文本演进与 PhaseRecord 解耦） |
| `StepRecord`（id=12） | `id: i64`（主键，写事务 max+1）、`change: String`（非唯一二级索引）、`run_id: String`、`step_kind`（封闭集七值）、`status: String`、`timestamp: i64`、`summary: String`（≤500 截断留痕）、`reference: Option<String>` | N:1 `ChangeRecord`（名称引用）；`run_id` 串链同 run 步骤序列；审计 only，不做 run 恢复依据 | 同上 |

关系与既有面：四模型与 `SessionRecord` / `SessionEventRecord` / `AgentRunRecord` / `ExploreRecord` 同库共存（workspace 组 4→8 additive 注册，零迁移）；`PhaseRecord` 三槽位列与 `SessionRecord` 的 join 使详情抽屉 transcript 逆查为库内一等查询。双载体边界：db 持状态，磁盘 change 目录持 markdown 产物（proposal / design / tasks / specs / reports / explore），`workflow.json` 双向墙。

---

## 依赖

### 运行时依赖

- `workflow`（crates/core/workflow，infra/store 新增 workspace 路径依赖）— port trait `ChangeStateStore` 与中性类型词汇；infra → core 单向边，无环（workflow 依赖 foundation，不依赖任何 infra）
- 既有依赖零新增外部 crate：native_db / native_model / serde / specta / time / foundation 均为各 crate 既有依赖；core/workflow 依赖面保持 foundation + serde + serde_json + specta + time（无 tokio、无 Tauri、无 infra）

### 构建/测试依赖

- `store`（crates/infra/store，workflow 的 dev-dependencies 新增）— 语料 db 种子构造器经真实 store change 域操作面（desktop-corpus-regression delta 明定「种子构造器经 store change 域操作面」）；该 dev-dependency 随 test-design / test-gen 阶段的语料构造落地，不属实现任务清单；cargo 允许 dev-dependency 与对方普通依赖成环（store 普通 dep → workflow，workflow dev-dep → store），无构建问题
- `DESKTOP_GOLDEN_REWRITE=1`（既有环境开关）— detail 线面字段演进的 golden 显式重写流程不变量

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | store 四模型与 change 域操作面：`model.rs` 四模型（D3 id 9–12、D4 打包键）、`store.rs` 操作面（max+1 主键、`(change, phase, attempt)` 写事务查重返回 `StoreError`、组注册 4→8 使 `list_models` / `scan` 零改动覆盖）；回环语义由「状态回环读写」scenario 支撑 |
| AC-2 | 写面 db 化与 port 缝：`state.rs` trait + `change_port.rs` 适配（D1/D2）；`log_change_phase` 单 redb 事务原子（PhaseRecord + ChecklistItemRecord + active_phase 清位，回滚零残留）；`crates/core/workflow/Cargo.toml` 零 infra/store 依赖由守线任务显式审查；写面全函数 sync 签名 |
| AC-3 | 双向墙与 parse 退役：`parse/` 与 `persist.rs` 删除任务；queries 文档形态零 workflow.json 读取（detail 只走 db + 产物发现）；守线任务含全源码 workflow.json 触点扫描（测试夹具除外） |
| AC-4 | queries db 读与发现语义：`list.rs` db ∪ 磁盘去重并集、同名 db 优先、按月分组与未知时间兜底、缺目录空结果；`detail.rs` 文档形态空流水线 + 产物清单；时间出线 ISO 串 + null 收 queries 单点（golden 守卫）；`change_flow_start` 前置校验改建档校验、无建档显式拒绝 |
| AC-5 | orchestration 全链走通：`steps.rs` 七臂命令经写面（port）落库并逐条落 StepRecord（run_id 串链）；`snapshot.rs` db 读源；节点状态 = db 状态 × provenance 派生（无 flow_runs 表，StepRecord 仅审计）；重启续走 = `phase_next` 依 eval 历史重算 + active_phase 自 db（D8 锚点不承担恢复） |
| AC-6 | create 建档：`create.rs` 目录 + explore.md（goal 原文）+ `ChangeRecord` 三合一（D5 顺序与补偿）；无 workflow.json 产出；成功即清单 / 详情可见、前置校验通过；同名 active 冲突拒绝且 db 零建档 |
| AC-7 | 归档双写：`archive.rs` 改名 → 翻转 + 续半边重试分支（D6）；主键 name 不变；按月分组可达经 `locate_change` 前缀后缀匹配与 list 合并；无建档目录显式拒绝；IPC 薄命令 `archive_change` |
| AC-8 | 前端退役面：`change-list-view.tsx` / `change-detail-view.tsx` / `detail-drawer.tsx` / `attachments.ts` / `types.ts` / `file-log-table.tsx`（删）退役清单 + `bindings.ts` 重导出；守线阶段为静态检查（client:check / knip 零新增豁免），测试执行由 test-execution 阶段承接 |
| AC-9 | golden 显式重写：DTO 三字段删除（inventory / fileLog / unparsable）走 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成 + diff 人工确认留痕（语料处置任务 D12）；db 种子语料覆盖矩阵（多 attempt / backtrack stale / 槽位全缺 / 文档形态 / 坏行）由 test-design / test-gen 阶段承接，spec「三代代表性 fixture 语料」已明定 |
| AC-10 | 版本交付：`packages/desktop/package.json` 0.4.12 → 0.4.13 于归档时执行（proposal 明定，`src-tauri/Cargo.toml` 不随动）；非实现任务，此处留痕防遗漏 |

---

## 待决问题

proposal 五项待决问题已全部在本设计定稿：

1. **外部归档对账** → D7：读时以磁盘事实归组，查询纯读不回写 db；桌面 archive 命令是 status 唯一写口。
2. **前端归档入口形态** → D11：IPC 薄命令先行，本轮无 UI 入口（V1 留痕，后续变更补按钮）。
3. **历史 workflow.json 语料处置** → D12：删除 + fixtures/README 改写 + 少量惰性字节样本钉文档形态语义。
4. **native_model id 分配与 SessionAnchors 存续** → D3 / D8：id 9–12；锚点进程内不落库、基线平移 PhaseRecord 行数。
5. **create 双写顺序与失败补偿** → D5：db 建档先行 + fs 失败补偿删档。

剩余无阻塞性待决问题。两点实现期注意（非待决）：

- `archive` 对 archive 树中无日期前缀的同名目录（外部手工挪入）不识别为续半边对象——按「active 树缺失 + archive 树精确/前缀均未命中」走常规 Err，人工处置；该形态不在 AC-7 约束面。
- `StoreSnapshot` 更名后 `WorkflowSnapshotPort` 的测试假件（`port_test.rs` / `walker_test.rs` / `steps_test.rs` 等 fake）随签名演进改写，归 test-design / test-gen 阶段承接，不在实现任务列表。
