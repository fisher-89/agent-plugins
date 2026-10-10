# 设计: desktop-change-db-identity

> **变更**: desktop-change-db-identity
> **日期**: 2026-10-10

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（10 能力 delta：state-store / queries / flow-view / archive / orchestration / create / app-shell / page-routing / corpus-regression / workspace-store，路径相对域根）已由提案阶段定稿（proposal 评审 11/11 通过）；本设计不重复其内容、不将其列为待办，只在其六项「待决问题」之上逐项定稿（D1–D5、D9–D11），并补设计面决策 D6–D8、D12–D14。

改动基线核实在场（逐点可查）：

- `ChangeRecord` 主键 name + `ChangeRecordV1` 解码链（`crates/infra/store/src/model.rs`）；
- `ChangeStateStore` 全方法 name 形参 + 五命令载荷 `change` 字段（`crates/core/workflow/src/state.rs`）；
- 列表 db ∪ 磁盘并集（`queries/list.rs` 的 `scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name`）与详情文档形态分支（`queries/detail.rs` record 与定位双缺之外的空流水线面）；
- run / archive 注册表键 `(root, change)`（`control.rs` / `archive_flow.rs`）；walker provenance `source_ref = <change>/<phase>/<role>/<attempt>`（`walker.rs` ~920 行）；归档链两会话 `source_ref = <change>/archive/*`（`archive_flow.rs` ~781 / ~933 行）；
- 13 条 change 寻址命令的 `change: String` 参数（`all_commands!` 清单：changes 组 3 + change_flow 5 + archive_flow 5）；
- 前端 `:name` 路由、行键与 `detail.name` 作抽屉入参（`routes.tsx` / `change-list-view.tsx` / `change-detail-view.tsx`）。

**关键既有约束（设计必须绕开的）**：

1. **双向墙**：desktop 全链零 workflow.json 读写（测试夹具字面量除外）——本变更不触。
2. **磁盘 / git 面 name 化**：change 目录名、worktree 落位、branch `change/<name>`、归档目录 `YYYY-MM-DD-<name>` 前缀与后缀扫描——零扰动（proposal「不要修改」）。
3. **native_db 0.8.2 事实**：模型版本迁移是显式 API（`RwTransaction::migrate`，须新旧版本同组注册），本设计零调用；物理表名 = `{模型 id}_{模型版本}_{键名}`（如 `9_2_name`）——版本段与键名段任一变化即产生独立新表，旧表对新模型组结构性不可见（「旧表对新读面不可见」的机制依据，非行为承诺）。
4. **specta / bindings 一致性守卫**：命令面与 DTO 演进经再生成 + `git diff --exit-code` 拦截漂移。
5. **组合根装配事实**：`WorkspaceStores::for_root` 惰性开库 + 进程内缓存复用（`for_root` 首开即全库只触一次）——版本探测与启动标定的挂点依据。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 持久化模型换锚（新） | `ChangeRecord` v3（`id` 主键 / `name` 属性）；`PhaseRecord` / `StepRecord` / `RunRecord` v2（`change_id` 归属列）；`StoreMetaRecord`（库级格式版本标记）；`ChangeRecordV1` 解码链退役 | `crates/infra/store/src/model.rs`（修改） | native_db / native_model | native_model 版本段演进，零 `from` 链（无迁移 / 解码路径） |
| store 域操作面 | 全操作面 `change → change_id`；建档同 id 防御拒绝；打开路径格式版本探测与旧库作废重建；run 收口 / 启动标定按 `change_id` 定位记录 | `crates/infra/store/src/store.rs`（修改） | model.rs / foundation | redb 单文件；作废 = 文件删除重建 + 标记落笔 |
| port 适配胶水 | `impl ChangeStateStore for Store` 全方法 id 形参委托随动 | `crates/infra/store/src/change_port.rs`（修改） | workflow::state | 零具名导出委托 |
| 信封 key 投影 | change 信封主键 JSON 投影 `value["name"]` → `value["id"]` | `crates/infra/store/src/envelope.rs`（修改） | — | 查看器零模型代码不变 |
| 状态缝（中性类型 + port + 载荷） | `ChangeStateRecord` 增 `id`；`PhaseStateRecord` / `StepStateRecord` / `RunStateRecord` 与五写命令载荷 `change → change_id`；`ChangeStateStore` 全方法 id 形参 | `crates/core/workflow/src/state.rs`（修改） | serde / specta | 中性类型与持久化记录分离纪律不变 |
| 写面 create | 前置查重改 name 扫描（仅拒同名 active）；前置通过后铸 uuid 形态 `id` 入建档载荷；`CreateOutcome` 增 `id` | `crates/core/workflow/src/write/create.rs`（修改） | `uuid`（新普通依赖） | sync 零 tokio；双写补偿链语义零改动 |
| 写面 archive | 按 id 读记录 → `record.name` 供给目录改名；`id` / `name` 均不随改名变；未建档 id 拒绝面沿袭 | `crates/core/workflow/src/write/archive.rs`（修改） | queries 定位单点 | 双写本体零 worktree / branch 触碰不变 |
| 相位机写面 | `phase_start` / `phase_log` / `backtrack` / `decision_log` / `phase_next` / `run` 六文件形参与内部解析 id 化；prompt 插值改用 `record.name` | `crates/core/workflow/src/write/phase_next.rs`（修改，同组五文件同式） | state.rs | 校验与白名单语义零改动 |
| 列表查询 | db 单源（`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` 退役）；`ChangeSummary` 增 `id`、name 恒裸名；月分组 `archived_at` 唯一权威 | `crates/core/workflow/src/queries/list.rs`（修改） | state.rs | 签名收敛（去 `Layout`）；零 fs 触点 |
| 详情查询 | `change_detail(…, id)`：`get_change(id)` → 记录 → `locate_change`（record 供给 name / worktree）；文档形态分支删除，未知 id 恒 `None` | `crates/core/workflow/src/queries/detail.rs`（修改） | queries/mod.rs | `ChangeDetail` 增 `id`；created 磁盘前缀回退删除 |
| 目录定位单点 | `locate_change` 消费面改「记录供给 name / worktree」；archive 后缀扫描与单分量名校验保留（校验施于解析出的 name） | `crates/core/workflow/src/queries/mod.rs`（修改） | foundation::layout | 名义签名不变，消费语义修订 |
| run 控制注册表 | 键 `(root, change)` → `(root, change_id)`（全方法签名与键构造点随动，proposal 口径 12 处） | `crates/core/orchestration/src/control.rs`（修改） | tokio sync 原语 | DTO 零改动（`RunNotice` kind-only 不变） |
| 归档链 | `ArchiveControl` / `ArchiveRequest` / `ArchiveGuard` 键 id 化；`drive` 前置段一次解析 `record.name` 供 branch / prompt / pathspec / 目录名；两会话 provenance 身份段 id | `crates/core/orchestration/src/archive_flow.rs`（修改） | workflow 写面 | 阶段机语义与收口单点零改动 |
| run 载体与 provenance | `RunRequest.change → change_id`；`source_ref = <id>/<phase>/<role>/<attempt>` | `crates/core/orchestration/src/walker.rs`（修改） | port / control | 每 run 两写时序零改动 |
| 编排载荷与快照 | `ToolCommand` 五变体 `change → change_id`（`StaticCheck` 增 `change_id` 归键修正）；`WorkflowSnapshotPort::detail(root, id)`；test-execution 消费点 id → 记录 → name 解析 | `crates/core/orchestration/src/port.rs`（修改；steps.rs / snapshot.rs / run_history.rs 同式随动） | workflow | core 零 infra 依赖不变 |
| change 域命令组 | `get_change_detail` / `read_artifact` / `archive_change` 参数 `id`；`create_change` 出线 `CreateOutcome.id`；`list_changes` 行为 db 单源 | `src/commands/changes/mod.rs`（修改） | workflow / store | 薄包装三件事纪律不变 |
| 两 flow 命令组 | change_flow 五命令 / archive_flow 五命令定位参数 `id`；装配（建档校验 / exec root / 互斥）经 id 读记录 | `src/commands/change_flow/mod.rs`（修改；`src/commands/archive_flow/mod.rs` 同式） | orchestration / store | `Result<T, String>` 模板不变 |
| 路由表 | `/changes/:name` → `/changes/:id` | `packages/desktop/src/routes.tsx`（修改） | react-router | HashRouter 选型零改动 |
| 清单页 | 行键 / 导航 id 化；归档行显示裸名；空态文案随 db 单源修订 | `packages/desktop/src/views/changes/change-list-view.tsx`（修改） | bindings | 分组结构与 data-testid 零改号 |
| 详情页 | `useParams<'id'>`；`flow-empty` 文档形态分支删除（单态呈现）；页内身份源统一 URL id | `packages/desktop/src/views/changes/change-detail-view.tsx`（修改） | bindings | 降级页 / 抽屉 / 产物区结构不变 |
| 前端取数 hooks | `useChangeDetail(root, id)` / `useChangeFlowRun({ root, id, … })` / `useArchiveFlow({ root, id, … })` 参数 id 化；`useChangeList` DTO id 消费 | `packages/desktop/src/views/changes/hooks/use-change-detail.ts`（修改，同组三文件随动） | bindings | 显式刷新 / 去抖模型不变 |
| 转录联动 | sourceRef 定式组装身份段 = change id（`<id>/<phase>/<role>/<attempt>` 与 active 三 role 反查同式） | `packages/desktop/src/views/changes/flow/detail-drawer.tsx`（修改；`hooks/use-session-transcript.ts` 注释随动） | bindings | 槽位 id 直查优先不变 |
| 运行 / 归档面板 | props 收敛 `changeId`（命令面）+ `name`（展示面，取自 detail）；命令面 id 透传 | `packages/desktop/src/views/changes/flow/run-control-panel.tsx`（修改；`archive-panel.tsx` 同式） | bindings | 展示语义与 data-testid 不变 |
| 类型跟随 | 命令面与 DTO 再生成（13 命令 id 参数 + 三 DTO 增 `id`） | `packages/desktop/src/types/generated/bindings.ts`（修改） | export-bindings 管线 | specta 生成物零手改 |
| 版本交付 | `version` 0.4.30 → 0.4.31（归档时执行） | `packages/desktop/package.json`（修改） | — | `tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

**不变组件（零触点核对结论）**：`crates/core/foundation/src/layout/*`（磁盘域根常量与解析，唯一合法触点）、`crates/infra/vcs/*`（worktree 落位派生与 git 执行仍 name 化）、`crates/infra/checks/*`（test-execution 报告目录派生 `change_test_reports(root, name)` 零改动，仅调用侧解析值变化）、`crates/infra/agent/*` 与 `RunEventSink` seam、会话转录落库 / StopRegistry / `agent_sessions` 查询语义（仅 source_ref 字符串值更换）、相位机语义（校验 / 白名单 / 重试预算）与 run 双写时序、通知信封 `RunNotice`（零载荷 kind-only）、存档面 DTO（`ArchivePreflight` / `ArchiveSummary` / `ArchiveOutcome` 字段面）、`main.rs`（`WorkspaceStores::open` 装配零改动）、`plugins/dev-team` 全部、主基线 specs。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由（含被拒备选） |
|---|------|------|------|
| D1 | id 铸出点与 uuid 版本（待决 1 前段） | **写面 `create` 直铸 uuid v7**：前置七道全过、建档先行之前铸出（与 `created_at` 同段），入 `ChangeStateRecord.id`；`uuid` crate（`v7` feature）入 workspace 依赖 + workflow crate 普通依赖。**被拒**：命令层铸出经入参传入——写面「create 铸出建档身份」语义外移壳层，与 `created_at` 铸点分裂，写面独立可用性（corpus / 测试直调）损失；uuid v4——纯随机无时间序，v7 免费获得「主键自然序 ≈ 建档序」（redb 键有序，扫描 / 排查价值） | 与写面既有时钟铸点（`now_millis`）同构；core 铸 id 先例（`agent::session::new_session_id`）；uuid 属普通依赖不破 crate 方向（proposal R5 既定） |
| D2 | 语料确定性注入形态（待决 1 后段） | **语料 db 种子直携固定 id**：种子构造器（建档 / 开相 / 落账 / run 史）经 `ChangeStateRecord.id` 与命令载荷 `change_id` 传稳定字面量（不透明串，逐字入 golden）；「uuid 形态」断言归 create 铸出路径（行为断言 id 非空 / 同名单次重铸相异 / `CreateOutcome.id` 与库内逐字一致），不做固定值对拍。**被拒**：铸出点抽成可注入 port / 闭包供语料注固定值——为语料确定性引入铸出缝，复杂度不值（golden 语料不经 create 路径，已核实） | 语料 = db 种子直写（既有纪律「禁裸表插桩，经 store 域操作面」），id 归键后种子天然携 id |
| D3 | store 格式版本载体与作废实现形态（待决 2） | **库级版本标记记录 + 文件删除重建**：新模型 `StoreMetaRecord`（`{模型 id}=15`，固定单键 `key="format"`，`format_version: u32`）落 workspace 库；`WORKSPACE_STORE_FORMAT_VERSION = 2`（语义编号：1 = name 主键形态时代，实机从未写标记，探测以「缺失」判定；未来 bump 递增，探测规则 `缺失或低于当前 → 作废` 不变）。作废 = drop 句柄 → `fs::remove_file` 旧库文件 → `create` 重建全新空库 → 写标记（`format_version=2`）。标记模型 `pub(crate)`、注册进 `workspace_models()`（十一模型），**不入信封注册表**（内部治理记录，无查看面需求）。**被拒**：旧文件换名留档后新建——磁盘长期滞留死文件、无回收策略，「丢弃」语义不如删除干净；文件名版本段——无「标记就位」可探测面，与 workspace-store spec 场景「新库版本标记幂等」不符 | spec 明文二选一（「库级版本标记 + 文件删除重建 vs 旧文件换名留档后新建」，design 定稿） |
| D4 | 作废探测执行时序与错误面（待决 3） | **内嵌 `Store::open_workspace` 首开**（per-workspace 首次打开；`for_root` 缓存保证每库每进程一次）：① 文件缺失 / 空文件 → 建新库 + 写标记（就绪直进）；② 存量文件 → `open`（全模型组）→ 读标记：一致 → 照常就绪；缺失 / 低于当前 → 上述作废重建。幂等（新库标记恒在，重开不作废）。错误面：删除失败（文件锁等环境故障）→ `StoreError::Db` 显式 Err（恢复性故障语境，非旧形态库本身报错）；旧形态库的探测与作废路径本身零报错。**被拒**：应用启动批量执行——须枚举 workspaces 子树并预开全部既有库，启动耗时与错误面污染，且与「首触即开」惰性架构相悖 | workspace-store spec 场景「旧形态库打开即作废重建 / 新库版本标记幂等」；标定内嵌 `open_workspace` 先例（unify-run-state-persistence D12）同挂点 |
| D5 | 与既有「旧库惰性废弃」条款的衔接（待决 2 衔接措辞） | **两条款并列，共同构成 store 零迁移纪律**：既有条款（全局库与旧单库文件「换名免探测、旧文件惰性废弃」）不变——本变更零触碰全局库；本变更为 workspace 库新增「探测式作废重建」条款（标记缺失 / 低于当前 → 整体丢弃）。措辞定式：**旧形态数据只有两种去向——不可达（惰性废弃）或整体丢弃（探测作废）；MUST NOT 存在第三条 decode / migrate 路径**。落点：workspace-store spec（已定稿）+ `store.rs` 打开路径文档注释 | 旧条款文档驻 `store.rs` 常量注释（`GLOBAL_DB_FILE_NAME` 段），本设计不重开该文件面语义，只新增并列条款 |
| D6 | 表版本段演进范围与解码链退役 | **形状变更一律版本段演进、零 `from` 链**：`ChangeRecord` 9:v2→**v3**（`id` 主键 / `name` 属性）；`PhaseRecord` 10:v1→**v2**、`StepRecord` 12:v1→**v2**、`RunRecord` 13:v1→**v2**（`change` → `change_id` 二级索引列）；`ChecklistItemRecord` 11 / `RunStepRecord` 14 维持 v1（间接归属链不变）；`ChangeRecordV1` 及其双向 `From` 整体删除。表名映射：`9_2_name` → `9_3_id`；`10_1_id`+`10_1_change` → `10_2_id`+`10_2_change_id`（12 / 13 同式）；`15_1_format` 新增。旧表对新读面结构性不可见（表命名机制），为作废重建之外的第二道防线。**被拒**：仅改列名不 bump 版本——`10_1_id` 表名未变，作废失败竞态下旧 per-change 行对新读面可见（混读风险面），且与「形状变更 = 版本段演进」既有纪律不一致 | AC-8「表名版本段与旧表不可见断言」面；native_db 表名机制核实（`{id}_{version}_{key}`） |
| D7 | 分辨率单点形态（id → 记录 → name / worktree） | **记录读取单点，磁盘 / git 面消费直供**：一切库查询 / 命令面 / 注册表 / provenance / 路由以 id 为参数；磁盘与 git 面（change 目录定位、归档改名、worktree 落位、branch `change/<name>`、test-execution 报告目录、归档链 branch / pathspec / prompt 内文）仍 name 化，name / worktree 恒经 `get_change(id)` 读记录后**直供**（`record.name` / `record.worktree`），MUST NOT 在任何磁盘 / git 面做 name 查询。判定点保留：archive 后缀扫描 + `is_single_component_name` 校验（改施于解析出的 name；id 为不透明串不做目录名语义校验）。归档链 `drive` 前置段一次解析 `record.name` 后全程消费；`phase_next` prompt 插值改用 `record.name`；`steps.rs` test-execution 调用前经记录解析 name（`TestExecutionRunner` 保持 name 形参——磁盘面 port，infra 侧零 store 依赖；spec「签名随动」落在 `ToolCommand` 载荷与 `WorkflowSnapshotPort::detail(root, id)` 面）。**被拒**：令磁盘面 port 收 id 自行解析——port 侧无 store 依赖（core 零 infra 纪律），不可行 | proposal 定案「一切寻址以 id 为准；磁盘与 git 面仍 name 化，由 id → 记录 → name 分辨率单点供给」 |
| D8 | 出线 DTO 增量口径 | **增量最小化：只增 `id`**。`ChangeSummary` / `ChangeDetail` / `CreateOutcome` 增 `id: String`（首字段）；`ChangeDetail.name` 改自记录直读（恒裸名）；`created` 的磁盘前缀回退删除（记录恒在）；`status` / `created` 的 `Option` 形态与 `ChangeSummary.source` 字段**原样保留**（值恒在场 / 恒由 status 派生——缩减类型面属额外形态变更，golden 范围无谓扩大，proposal 口径只声明增 id）。存档面 DTO（`ArchivePreflight` / `ArchiveSummary` / `ArchiveOutcome`）与 `RunNotice` 零改动。**被拒**：全 DTO 无差别增 id——golden 与人工确认成本扩大，无消费者（proposal 已拒）；缩 Option / 删 source——超出 proposal DTO 口径 | proposal 决策表「出线 DTO 面」；wire 冻结纪律（golden 显式重写只覆盖声明范围） |
| D9 | 归档条目展示面（待决 4） | **不新增行内归档日期辅助信息**：归档行 = 裸名 + 既有 `created`（建档日期，与 active 行同口径）；时间语义由月分组承载（`archived_at` 唯一权威）；`ChangeSummary` 零新字段。同名并存可读性（title 展示演进）留后续变更（此时已具 id 主键前提）。**被拒**：行内补归档日期——引入第二日期语义（归档日 vs 建档日）反增混淆，月分组已覆盖 | proposal 待决 4「月分组已承载时间语义」暗示；R7 留痕 |
| D10 | 前端页内身份源合一与 props 收敛（待决 5） | **单一身份源 = URL `id`，props 显名收敛**：`useParams<'id'>`；`DetailDrawer` / `RunControlPanel` / `ArchivePanel` 的 `change` prop 拆为 **`changeId`（命令面透传，值 = URL id）+ `name`（展示面，取自 `detail.name`）**；hooks 参数 id 化（`useChangeDetail(root, id)` / `useChangeFlowRun({ root, id, … })` / `useArchiveFlow({ root, id, onFinish })`）；抽屉 sourceRef 定式身份段 = URL id；页内全链无 `detail.name` 作寻址残留。**被拒**：保持 prop 名 `change` 仅改实参——name / id 静默歧义源（仅靠阅读调用方分辨），漏改即 R3 复发；给面板继续传 name——命令面 name 寻址违规 | proposal 决策表「前端路由 `/changes/:id`」+ AC-7「页内单一身份源」 |
| D11 | name 唯一性回退边界（待决 6） | **create 前置两点拒绝，查重驻写面单点**：① 主仓 active 目录同名已存在（磁盘）；② db 已有同名 **active** 记录（写面经 `list_change_records` 扫描 name 比对——name 无唯一约束，主键冲突面消失）。归档同名共存**合法化且对话框不加提示**（展示面提示价值低；错误文案已含对象名 / 路径，语义可读）。store 的 `create_change_record` 零同名检查，仅**同 id 防御拒绝**（`Conflict`，不静默覆写）。**被拒**：对话框加「已有同名归档 change」提示——create 对话框当前不接列表数据，新增取数耦合不值；store 侧重复同名查重——双检查点，写面单点纪律破坏（同 desktop-change-create spec「同名 active 拒绝由写面前置扫描承担」） | desktop-change-create spec Module Contract；同名并存可读性归 D9 留痕 |
| D12 | StaticCheck 审计 `change_id` 归键修正 | `ToolCommand::StaticCheck` 增 `change_id: String` 字段（walker 填充 `request.change_id`），`steps.rs` 审计行由空串占位修为真实 change_id——spec「步骤审计落库可查」逐条 `change_id` 归键条款的合规修正 | 现实现空串占位在该条款下不再合规（`list_steps(change_id, …)` 枚举不到 static_check 行）；改造面小（一字段一填充点） |
| D13 | 语料与 golden 改造范围声明（承接边界） | 设计侧只钉范围与形态（执行归 test-design / test-gen / test-execution 阶段）：语料种子 id 归键（固定 id 携入）；「db 缺记录文档形态」样本退役改造为**零发现反例**（磁盘目录保留、字节零变化、任意 name / 前缀名寻址恒不可达）；归档前缀样本（裸名 name + `archived_at` + `YYYY-MM-DD-` 前缀目录）入 golden；list / detail golden 增 `id` 键、列表去磁盘-only 条目、月分组改 `archived_at` 权威；重写经 `DESKTOP_GOLDEN_REWRITE=1` 显式流程 + diff 人工确认留痕。**被拒**：静默再生成——wire 冻结纪律（golden 逐字节钉死）明拒 | proposal AC-9 + 既有 wire 冻结记忆（形态变更走显式重写流程） |
| D14 | 版本交付与守线 | `packages/desktop` 0.4.30 → **0.4.31**（归档时执行；`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 零改动；守线为静态检查（fmt / clippy / knip / bindings 一致性 / 零 diff 边界 grep），全管线测试与 golden 复核归 test-execution 阶段 | proposal AC-10 / AC-11；既有惯例 |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写；公共函数仅列模块级导出函数 / 公开方法 / Tauri 命令，私有函数不列入。
  测试文件（既有用例的机械保编译随动除外）由 test-design / test-gen / test-execution 阶段承接，不入本清单。
-->

<!-- 新增文件：无（`StoreMetaRecord` 与格式版本常量驻既有 `crates/infra/store/src/model.rs`；语料 / golden 文件归测试文件面，不入本清单），按模板省略本子节 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `ChangeRecord` v3（`id` 主键 + `name` 属性，`new` 构造携 id 入参）；`PhaseRecord` / `StepRecord` / `RunRecord` v2（`change_id` 列）；`StoreMetaRecord`（新模型 id=15）+ `WORKSPACE_STORE_FORMAT_VERSION = 2` 常量；`ChangeRecordV1` 及双向 `From` 删除 | D3 / D5 / D6 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `open_workspace` 打开路径格式版本探测与旧库作废重建（D4 定形）；`workspace_models()` 注册十一模型；change 域全操作面 id 形参（`find_change_record` / `delete_change_record` / `start_change_phase` / `amend_change_decision_session` / `set_change_archived` / `list_phase_records` / `list_change_steps` / `list_change_runs` 等）；`create_change_record` 同 id 防御拒绝（零同名检查）；`log_change_phase` / `finish_change_run` / `calibrate_interrupted_runs` 按 `change_id` 定位记录 | D3–D7 / D11 |
| `packages/desktop/src-tauri/crates/infra/store/src/change_port.rs` | `impl ChangeStateStore for Store` 全方法 id 形参委托随动（映射语义零改动） | D7 |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | `change_key` 投影 `value["name"]` → `value["id"]`（`StoreMetaRecord` 不入注册表） | D3 / 信封面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `ChangeStateRecord` 增 `id`；`PhaseStateRecord.change` / `StepStateRecord.change` / `RunStateRecord.change` → `change_id`；五写命令载荷（`PhaseLogCommand` / `BacktrackCommand` / `StepCommand` / `RunStartCommand` / `RunFinishCommand`）`change → change_id`；`ChangeStateStore` 全方法 id 形参（文档注释注明身份锚语义） | D6 / D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | db 单源：`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` / `archive_month` 磁盘回退段删除；`list_changes(store)` 签名收敛；`ChangeSummary` 增 `id`（首字段）、name 恒裸名；月分组改 `archived_at` 唯一权威（缺失归「未知时间」） | D8 / AC-3 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `change_detail(layout, store, id)`：`get_change(id)` → 记录 → `locate_change(layout, record.worktree, record.name)`；文档形态分支与空流水线面删除（record `None` → 恒 `None`）；`ChangeDetail` 增 `id`；`created` 磁盘前缀回退删除 | D7 / D8 / AC-2 / AC-4 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` | `locate_change` 消费语义随动（name / worktree 恒自记录供给）；`is_single_component_name` 校验点措辞随动（施于解析出 name） | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 前置④ 改 name 扫描查重（仅拒同名 active）；前置通过后铸 uuid v7 形态 `id`；`CreateOutcome` 增 `id`；补偿链删记录改按 id | D1 / D11 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs` | `archive(layout, store, id)`：id 读记录 → `record.name` 目录改名与续半边定位；错误文案随 id 语境修订（呈现记录名） | D7 / R8 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs` | 形参 `change → change_id`；内部 `get_change(change_id)`；错误文案随动 | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | 形参 `change → change_id`；校验语义零改动 | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs` | 形参 `change → change_id`；stale 闭包语义零改动 | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs` | 形参 `change → change_id` | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | `phase_next(store, change_id, run_id, anchors)`；prompt 插值改用 `record.name`；`SessionAnchors` 键 `(change_id, run_id)` | D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/run.rs` | `run_start` / `run_finish` 校验读记录面随动（载荷 `change_id`） | D7 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | 注册表键 `(root, change)` → `(root, change_id)`；全方法签名与键构造点随动（proposal 口径 12 处）；头注释语义随动（键 = 工作区 × change 身份） | D7 / AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs` | `ArchiveControl` / `ArchiveRequest` / `ArchiveGuard` 键 id 化；`drive` 前置段一次解析 `record.name` 供 branch / prompt 内文 / pathspec / 目录名 / `ArchiveSummary.name`；两会话 `source_ref = <id>/archive/spec-sync` 与 `<id>/archive/merge-conflict`；`preflight` 形参改 id（worktree 自记录读） | D7 / AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `RunRequest.change → change_id`；`source_ref = <id>/<phase>/<role>/<attempt>`；ToolCommand 构造点随动；每 run 两写时序零改动 | D7 / AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand` 五变体 `change → change_id`；`StaticCheck` 增 `change_id`（D12）；`WorkflowSnapshotPort::detail(root, id)`；`TestExecutionRunner::run` 形参改名 `name`（消费语义注明） | D7 / D12 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | 审计行 `change_id` 归键（StaticCheck 空串占位退役）；test-execution 调用前经 `get_change(change_id)` 解析 `record.name` | D7 / D12 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | `finish_command` 载荷 `change_id` 随动（过滤单点语义零改动） | D7 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | `detail(root, id)` 形参与 `change_detail` 调用随动 | D7 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `get_change_detail` / `read_artifact` / `archive_change` 参数 `change → id`（参数名 `id`）；`read_artifact` 经 `find_change_record(id)` 供给 worktree / name；`list_changes` 行为 db 单源（core 签名收敛调用随动）；`create_change` 返回 `CreateOutcome.id` 自动出线 | D7 / D8 / AC-5 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 五命令定位参数 `id`；装配（建档校验 / workflow_type / exec root / 归档互斥）经 id 读记录；`ChangeFlowSink` / `begin_run` / 订阅键 id；blank 检查文案随动 | D7 / AC-5 / R8 |
| `packages/desktop/src-tauri/src/commands/archive_flow/mod.rs` | 五命令定位参数 `id`；`ArchiveSink` / `ArchiveControl` 调用键 id；前置校验（建档 / status / run 互斥）经 id 读记录 | D7 / AC-5 |
| `packages/desktop/src-tauri/crates/core/workflow/Cargo.toml` | `[dependencies]` 增 `uuid = { workspace = true }`（增补：proposal 未逐文件列名的依赖面） | D1 |
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 增 `uuid = { version = "1", features = ["v7"] }`（增补） | D1 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `bindings:export` 再生成（13 命令 id 参数 + `ChangeSummary.id` / `ChangeDetail.id` / `CreateOutcome.id`） | 一致性守卫 |
| `packages/desktop/src/routes.tsx` | `/changes/:name` → `/changes/:id` | D10 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 行 `key` 与 `navigate` 取 `summary.id`；行展示取裸名；`onCreated(id)` 导航；空态文案随 db 单源修订（「未发现任何 change 目录」→ 未发现已建档 change） | D10 / AC-3 / AC-7 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `useParams<'id'>`；`flow-empty` 文档形态分支删除（`FlowSection` 单态）；页内标识符与参数链 id 化（`selected → id` 语义）；面板传 `changeId` + `name` | D10 / AC-4 / AC-7 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | `useChangeDetail(root, id)`；`loadUnifiedView` / `readArtifactSafe` 参数 id（命令调用随动） | D10 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | 参数 `change → id`（五命令调用随动） | D10 |
| `packages/desktop/src/views/changes/hooks/use-archive-flow.ts` | 参数 `change → id`（五命令调用与快照 / 补订随动） | D10 |
| `packages/desktop/src/views/changes/hooks/use-change-list.ts` | DTO `id` 消费随 bindings 自动跟随；文档注释随 db 单源语义修订（本行改动为注释面，签名零变化） | AC-3 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 注释措辞随动（sourceRef 身份段 = change id）；查询逻辑零改动 | D10 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | prop `change → changeId`；`selectionRoleRefs` 三处定式组装身份段改 changeId | D10 / AC-6 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | props 收敛 `changeId`（命令面）+ `name`（展示面：aria-label / 文案取 detail.name） | D10 |
| `packages/desktop/src/views/changes/flow/archive-panel.tsx` | props 收敛 `changeId` + `name`（确认对话 / 进行面标题取 detail.name；命令面 id 透传） | D10 |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | `onCreated` 回调改携 `outcome.id`（增补：desktop-change-create spec Module Contract 面） | D1 / AC-5 |
| `packages/desktop/src/views/changes/flow/types.ts` | 注释措辞随动（`sourceRef` 定式描述身份段 = change id）（增补） | D10 |
| `packages/desktop/package.json` | `version` 0.4.30 → 0.4.31（**归档时执行**） | D14 |

<!-- 删除文件：无整文件删除（list 磁盘扫描段 / detail 文档形态分支 / ChangeRecordV1 解码链 / flow-empty 分支均为段级删除），按模板省略本子节 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `ChangeStateStore.get_change` | `crates/core/workflow/src/state.rs` | 修改 | `fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault>` | 主键 id 直查；`None` = 未建档（不再有文档形态语义） |
| `ChangeStateStore.list_phase_records` | 同上 | 修改 | `fn list_phase_records(&self, change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault>` | 归属键 id |
| `ChangeStateStore.list_steps` | 同上 | 修改 | `fn list_steps(&self, change_id: &str, run_id: Option<&str>) -> Result<Vec<StepStateRecord>, StoreFault>` | 归属键 id |
| `ChangeStateStore.delete_change_record` | 同上 | 修改 | `fn delete_change_record(&self, id: &str) -> Result<bool, StoreFault>` | 补偿删除按 id（miss 幂等不变） |
| `ChangeStateStore.start_phase` | 同上 | 修改 | `fn start_phase(&self, change_id: &str, phase: &str, now: i64) -> Result<PhaseStartState, StoreFault>` | 开相按 id 定位记录 |
| `ChangeStateStore.amend_decision_session` | 同上 | 修改 | `fn amend_decision_session(&self, change_id: &str, phase: &str, session_id: &str) -> Result<(), StoreFault>` | decision 槽位挂账按 id |
| `ChangeStateStore.set_archived` | 同上 | 修改 | `fn set_archived(&self, id: &str, archived_at: i64) -> Result<(), StoreFault>` | 归档翻转按 id（id / name 均不变） |
| `ChangeStateStore.list_runs` | 同上 | 修改 | `fn list_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault>` | run 史按 id |
| `Store.create_change_record` | `crates/infra/store/src/store.rs` | 修改 | `pub fn create_change_record(&self, record: ChangeStateRecord) -> Result<ChangeStateRecord, StoreError>` | 签名不变；行为改：同 id 防御拒绝（`Conflict`），零同名检查（D11） |
| `Store.delete_change_record` | 同上 | 修改 | `pub fn delete_change_record(&self, id: &str) -> Result<bool, StoreError>` | id 形参 |
| `Store.find_change_record` | 同上 | 修改 | `pub fn find_change_record(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreError>` | id 形参 |
| `Store.start_change_phase` | 同上 | 修改 | `pub fn start_change_phase(&self, change_id: &str, phase: &str, now: i64) -> Result<PhaseStartState, StoreError>` | id 形参（attempt 推导语义不变） |
| `Store.amend_change_decision_session` | 同上 | 修改 | `pub fn amend_change_decision_session(&self, change_id: &str, phase: &str, session_id: &str) -> Result<(), StoreError>` | id 形参 |
| `Store.set_change_archived` | 同上 | 修改 | `pub fn set_change_archived(&self, id: &str, archived_at: i64) -> Result<(), StoreError>` | id 形参 |
| `Store.list_phase_records` | 同上 | 修改 | `pub fn list_phase_records(&self, change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreError>` | id 形参 |
| `Store.list_change_steps` | 同上 | 修改 | `pub fn list_change_steps(&self, change_id: &str, run_id: Option<&str>) -> Result<Vec<StepStateRecord>, StoreError>` | id 形参 |
| `Store.list_change_runs` | 同上 | 修改 | `pub fn list_change_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreError>` | id 形参 |
| `Store.open_workspace` | 同上 | 修改 | `pub fn open_workspace(path: &Path) -> Result<Self, StoreError>` | 签名不变；打开路径增格式版本探测与旧库作废重建（D3 / D4） |
| `Store.calibrate_interrupted_runs` | 同上 | 修改 | `pub fn calibrate_interrupted_runs(&self, now: i64) -> Result<Vec<String>, StoreError>` | 签名不变；清位按 `change_id` 定位记录 |
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str) -> Result<CreateOutcome, String>` | 签名不变；行为改：前置查重 name 扫描（仅拒同名 active）+ 铸 uuid v7 形态 id 入载荷 |
| `archive` | `crates/core/workflow/src/write/archive.rs` | 修改 | `pub fn archive(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Result<ArchiveOutcome, String>` | id 寻址；`record.name` 供给目录改名 |
| `phase_start` | `crates/core/workflow/src/write/phase_start.rs` | 修改 | `pub fn phase_start(store: &dyn ChangeStateStore, change_id: &str, phase: &str) -> Result<PhaseStartOutcome, String>` | id 形参 |
| `phase_log` | `crates/core/workflow/src/write/phase_log.rs` | 修改 | `pub fn phase_log(store: &dyn ChangeStateStore, change_id: &str, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>` | id 形参 |
| `backtrack` | `crates/core/workflow/src/write/backtrack.rs` | 修改 | `pub fn backtrack(store: &dyn ChangeStateStore, change_id: &str, input: &BacktrackInput) -> Result<BacktrackOutcome, String>` | id 形参 |
| `decision_log` | `crates/core/workflow/src/write/decision_log.rs` | 修改 | `pub fn decision_log(store: &dyn ChangeStateStore, change_id: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String>` | id 形参 |
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | `pub fn phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | id 形参；prompt 插值改 `record.name` |
| `list_changes` | `crates/core/workflow/src/queries/list.rs` | 修改 | `pub fn list_changes(store: &dyn ChangeStateStore) -> ChangeList` | 签名收敛（去 `Layout`）；db 单源零磁盘触点 |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Option<ChangeDetail>` | id 寻址；未知 id 恒 `None` |
| `locate_change` | `crates/core/workflow/src/queries/mod.rs` | 修改 | `pub fn locate_change(layout: &Layout, worktree: Option<&str>, name: &str) -> Option<ChangeLocation>` | 签名不变；消费语义修订（name / worktree 恒自记录供给） |
| `ChangeFlowControl.begin_run` | `crates/core/orchestration/src/control.rs` | 修改 | `pub fn begin_run(self: &Arc<Self>, root: &str, change_id: &str, run_id: String, started_at: i64) -> Result<RunGuard, String>` | 复合键 `(root, change_id)`；`subscribe` / `request_stop` / `current_session` / `publish` / `set_session` / `answer` / `confirm` / `snapshot` 全方法同式随动 |
| `ArchiveControl.begin` | `crates/core/orchestration/src/archive_flow.rs` | 修改 | `pub fn begin(self: &Arc<Self>, root: &str, change_id: &str) -> Result<ArchiveGuard, String>` | 键 id 化；`is_active` / `subscribe` / `request_stop` / `current_session` / `set_session` / `publish` / `snapshot` 全方法同式随动（与 run 注册表 12 处同式的迁移面） |
| `preflight` | 同上 | 修改 | `pub fn preflight(main_root: &Path, store: &dyn ChangeStateStore, id: &str, vcs: &dyn ArchiveVcsPort, run_active: bool) -> Option<ArchivePreflight>` | id 寻址（worktree 自记录读；`name` 出线取 `record.name`） |
| `WorkflowSnapshotPort.detail` | `crates/core/orchestration/src/port.rs` | 修改 | `fn detail(&self, root: &str, id: &str) -> Result<ChangeDetail, String>` | 第二参改 id（经 queries id 寻址） |
| `TestExecutionRunner.run` | 同上 | 修改 | `fn run(&self, root: &str, name: &str) -> BoxToolFuture` | 形参改名（磁盘面 port 收 name）；解析在 `steps.rs` 消费点（D7） |
| `get_change_detail` | `src/commands/changes/mod.rs` | 修改 | `pub fn get_change_detail(stores: State<'_, WorkspaceStores>, control: State<'_, Arc<ChangeFlowControl>>, root: String, id: String) -> Option<ChangeDetailUnified>` | IPC 参数名 `id` |
| `read_artifact` | 同上 | 修改 | `pub fn read_artifact(stores: State<'_, WorkspaceStores>, root: String, id: String, kind: String, source: String) -> Option<ArtifactEnvelope>` | id 寻址（经记录供给 worktree / name） |
| `archive_change` | 同上 | 修改 | `pub fn archive_change(app: AppHandle, root: String, id: String) -> Result<ArchiveOutcome, String>` | id 寻址；blank id 显式 `Err` |
| `list_changes` | 同上 | 修改 | `pub fn list_changes(stores: State<'_, WorkspaceStores>, root: String) -> ChangeList` | 签名不变；行为 db 单源 |
| `create_change` | 同上 | 修改 | `pub async fn create_change(app: AppHandle, root: String, name: String, goal: String) -> Result<CreateOutcome, String>` | 签名不变；返回 DTO 增 `id` |
| `change_flow_start` | `src/commands/change_flow/mod.rs` | 修改 | `pub async fn change_flow_start(app: AppHandle, on_event: Channel<RunNotice>, root: String, id: String, auto_next_phase: bool) -> Result<ChangeRunSummary, String>` | 定位参数 id |
| `change_flow_stop` | 同上 | 修改 | `pub fn change_flow_stop(app: AppHandle, root: String, id: String) -> Result<(), String>` | 定位参数 id（miss 幂等不变） |
| `change_flow_answer` | 同上 | 修改 | `pub fn change_flow_answer(app: AppHandle, root: String, id: String, answer: String) -> Result<(), String>` | 定位参数 id |
| `change_flow_confirm` | 同上 | 修改 | `pub fn change_flow_confirm(app: AppHandle, root: String, id: String, proceed: bool) -> Result<(), String>` | 定位参数 id |
| `change_flow_watch` | 同上 | 修改 | `pub fn change_flow_watch(app: AppHandle, on_event: Channel<RunNotice>, root: String, id: String) -> Result<(), String>` | 定位参数 id |
| `archive_flow_preflight` | `src/commands/archive_flow/mod.rs` | 修改 | `pub fn archive_flow_preflight(app: AppHandle, root: String, id: String) -> Option<ArchivePreflight>` | 定位参数 id |
| `archive_flow_start` | 同上 | 修改 | `pub async fn archive_flow_start(app: AppHandle, on_event: Channel<ArchiveUpdate>, root: String, id: String, sync_specs: bool) -> Result<(), String>` | 定位参数 id |
| `archive_flow_stop` | 同上 | 修改 | `pub fn archive_flow_stop(app: AppHandle, root: String, id: String) -> Result<(), String>` | 定位参数 id |
| `archive_flow_state` | 同上 | 修改 | `pub fn archive_flow_state(app: AppHandle, root: String, id: String) -> Option<ArchiveSnapshot>` | 定位参数 id |
| `archive_flow_watch` | 同上 | 修改 | `pub fn archive_flow_watch(app: AppHandle, on_event: Channel<ArchiveUpdate>, root: String, id: String) -> Result<(), String>` | 定位参数 id |
| `useChangeDetail` | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 修改 | `function useChangeDetail(root: string \| null, id: string \| null): ChangeDetailState` | 参数 id 化（取数 / 产物逐读与通知面语义不变） |
| `useChangeFlowRun` | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | 修改 | `function useChangeFlowRun(params: { root: string \| null; id: string \| null; activeRunPresent: boolean; onNotice: (kind: RunNotice['ipc']) => void }): UseChangeFlowRunResult` | 参数 id 化（四命令 + 订阅生命周期不变） |
| `useArchiveFlow` | `packages/desktop/src/views/changes/hooks/use-archive-flow.ts` | 修改 | `function useArchiveFlow(params: { root: string \| null; id: string \| null; onFinish: () => void }): UseArchiveFlowResult` | 参数 id 化（五命令 + 重挂恢复不变） |
| `DetailDrawer` | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 修改 | `function DetailDrawer({ selection, graph, materials, root, changeId, transcriptRefreshKey, onClose }: DetailDrawerProps): JSX.Element \| null`（props `change` → `changeId`） | 三处 sourceRef 定式身份段改 changeId |
| `RunControlPanel` | `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 修改 | `function RunControlPanel({ changeId, name, activeRun, lastRun, actions }: RunControlPanelProps): JSX.Element`（props `change` 拆 `changeId` + `name`） | 展示面取 name，命令面取 changeId |
| `ArchivePanel` | `packages/desktop/src/views/changes/flow/archive-panel.tsx` | 修改 | `function ArchivePanel({ root, changeId, name, archive, open, onClose }: ArchivePanelProps): JSX.Element \| null`（props `change` 拆 `changeId` + `name`） | D10 |
| `ChangeCreateDialog` 回调面 | `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | 修改 | `onCreated: (id: string) => void`（成功回调携 `outcome.id`） | 清单页导航 `/changes/<id>` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ChangeRecord` | `crates/infra/store/src/model.rs` | 修改 | native_model 9:v3；`id: String`（`#[primary_key]`，uuid 形态，身份锚）+ `name: String`（无唯一约束可变属性，恒裸名）；`new` 构造携 id 入参；其余字段面（workflow_type / created_at / status / archived_at / active_phase / worktree / base_commit）逐字不变 |
| `StoreMetaRecord` | 同上 | 新增 | native_model 15:v1；`key: String`（`#[primary_key]`，恒 `"format"`）+ `format_version: u32`；库级格式版本标记（`pub(crate)`，不入信封注册表） |
| `PhaseRecord` | 同上 | 修改 | native_model 10:v2；`change: String` → `change_id: String`（`#[secondary_key]`，指向 `ChangeRecord.id`） |
| `StepRecord` | 同上 | 修改 | native_model 12:v2；`change` → `change_id`（`#[secondary_key]`） |
| `RunRecord` | 同上 | 修改 | native_model 13:v2；`change` → `change_id`（`#[secondary_key]`） |
| `ChangeStateRecord` | `crates/core/workflow/src/state.rs` | 修改 | 增 `id: String`（身份锚，首字段）；`name` 降为普通属性（注释语义更新） |
| `PhaseStateRecord` | 同上 | 修改 | `change: String` → `change_id: String` |
| `StepStateRecord` | 同上 | 修改 | `change` → `change_id` |
| `RunStateRecord` | 同上 | 修改 | `change` → `change_id` |
| `PhaseLogCommand` | 同上 | 修改 | `change` → `change_id` |
| `BacktrackCommand` | 同上 | 修改 | `change` → `change_id` |
| `StepCommand` | 同上 | 修改 | `change` → `change_id` |
| `RunStartCommand` | 同上 | 修改 | `change` → `change_id` |
| `RunFinishCommand` | 同上 | 修改 | `change` → `change_id` |
| `ChangeSummary` | `crates/core/workflow/src/queries/list.rs` | 修改 | 增 `id: String`（首字段）；`name` 恒裸名（注释重写）；`source` / `status` / `active_phase` / `created` 字段面保留 |
| `ChangeDetail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | 增 `id: String`（首字段）；`name` 自记录直读（恒裸名） |
| `CreateOutcome` | `crates/core/workflow/src/write/create.rs` | 修改 | 增 `id: String`（首字段，本次铸出） |
| `RunRequest` | `crates/core/orchestration/src/walker.rs` | 修改 | `change: String` → `change_id: String` |
| `ToolCommand` | `crates/core/orchestration/src/port.rs` | 修改 | `PhaseNext` / `PhaseStart` / `PhaseLog` / `Backtrack` / `DecisionLog` / `TestExecution` 六变体 `change → change_id`；`StaticCheck` 增 `change_id: String`（D12） |
| `ArchiveRequest` | `crates/core/orchestration/src/archive_flow.rs` | 修改 | `change: String` → `change_id: String` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.4.30"` → `"0.4.31"` | 归档时执行（D14；`tauri.conf.json` 自动跟随） |

---

## 数据模型

### ChangeRecord v3（native_model 9:v3，`crates/infra/store/src/model.rs`）

| 字段 | 类型 | 语义 |
|------|------|------|
| `id` | `String`（主键） | 建档铸出的稳定唯一身份锚（uuid v7 形态），终身恒定、不复用；一切寻址键 |
| `name` | `String` | change 名（恒裸名）；可变属性，无唯一约束，MUST NOT 作身份键 |
| `workflow_type` / `created_at` / `status` / `archived_at` / `active_phase` / `worktree` / `base_commit` | 既有 | 逐字不变（v2 字段面平移） |

`ChangeRecordV1`（9:v1）与双向 `From` 删除；v3 无 `from` 链（零解码 / 迁移路径）。

### StoreMetaRecord（native_model 15:v1）

| 字段 | 类型 | 语义 |
|------|------|------|
| `key` | `String`（主键） | 恒 `"format"`（固定单键） |
| `format_version` | `u32` | 当前写 `WORKSPACE_STORE_FORMAT_VERSION = 2`（1 = name 主键形态时代；实机从未写标记，探测以「缺失」判定） |

### per-change 归属链（既有间接链不动）

| 表 | 归属键 | 指向 |
|----|--------|------|
| `PhaseRecord`（10:v2） | `change_id`（二级索引） | `ChangeRecord.id` |
| `StepRecord`（12:v2） | `change_id`（二级索引） | `ChangeRecord.id` |
| `RunRecord`（13:v2） | `change_id`（二级索引） | `ChangeRecord.id` |
| `ChecklistItemRecord`（11:v1） | `phase_id`（打包主键高 64 位） | `PhaseRecord.id`（间接） |
| `RunStepRecord`（14:v1） | `run_id`（打包主键哈希段） | `RunRecord.run_id`（间接） |

### 物理表名映射（旧表对新读面不可见，D6）

| 旧表名 | 新表名 | 变化 |
|--------|--------|------|
| `9_2_name` | `9_3_id` | 主键换锚 + 版本段 |
| `10_1_id` / `10_1_change` | `10_2_id` / `10_2_change_id` | 版本段 + 归属列改名 |
| `12_1_id` / `12_1_change` | `12_2_id` / `12_2_change_id` | 同上 |
| `13_1_run_id` / `13_1_change` | `13_2_run_id` / `13_2_change_id` | 同上 |
| （无） | `15_1_format` | 版本标记新增 |
| `11_1_item_key` / `14_1_step_key` | 不变 | 间接归属链零改动 |

### workspace 库打开流程（`Store::open_workspace` 定形，D3 / D4）

```
open_workspace(path)：
  ① 文件缺失 / 空文件 → create 新库（workspace_models 十一模型）→ 写标记（format_version=2）→ 就绪
  ② 存量文件 → open（全模型组）
       ├─ 读标记 == 2 → 就绪（幂等路径，零作废）
       └─ 标记缺失 / < 2 → 作废重建：drop 句柄 → fs::remove_file(path)
                            → create 新库 → 写标记（format_version=2）→ 就绪
  ③ 就绪后照常执行 calibrate_interrupted_runs（既有挂点不变）
```

错误面：仅环境性删除失败 → `StoreError::Db`（显式 Err）；旧形态库的探测 / 作废路径零报错。全局库（`open_global`）与主基线「旧文件惰性废弃」条款零触碰（D5）。

### 线面增量（golden 契约；片段示意，逐字节以 golden 为准）

```json
// 列表条目（ChangeSummary）
{ "id": "0192f1e2-…", "name": "seeded-archived", "source": "archive",
  "status": "archived", "activePhase": null, "created": "2024-09-22" }
// 详情头部（ChangeDetail）
{ "id": "0192f1e2-…", "name": "seeded-archived", "status": "archived", … }
// 建档产出（CreateOutcome）
{ "id": "0192f1e2-…", "name": "new-change", "created": "2026-10-10", "worktree": "…", "warnings": [] }
```

---

## 依赖

### 运行时依赖

- `uuid`（`v7` feature）——新增：change id 铸造（写面 `create` 单点）。落点：workspace 根 `[workspace.dependencies]` + `crates/core/workflow/Cargo.toml` `[dependencies]`。属普通依赖（纯 id 生成，无 infra / 进程面），不破 crate 依赖方向与「core 零 infra」纪律（specta / serde / sha2 先例同型）。
- 无其他新增：`store` / `orchestration` / 壳层零新依赖（id 为普通 `String`，穿既有 serde / specta 面）。

### 构建/测试依赖

- 无新增（`redb` dev-dep、`tempfile`、`native_db` 等既有面不变）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | D1 / D6（id 铸出点 + 表版本段）；数据模型节（ChangeRecord v3 + per-change 归属链）；`ChangeStateRecord` / 载荷 id 化（类型定义节）——回环读写经 store 域操作面逐字段一致 |
| AC-2 | D7 / D8（detail id 寻址 + `locate_change` 自记录供给 name / worktree）；`archive` 写面 id 寻址 + `record.name` 改名（组件表写面 archive 行）——归档 change 全状态面出线，错配链结构性退役 |
| AC-3 | D8 + `list_changes(store)` 签名收敛（公共 API 表）；D3（归档行展示不新增磁盘信息来源）——源码零磁盘扫描触点 / 磁盘-only 条目零呈现 / 月分组 `archived_at` 权威 |
| AC-4 | 详情查询组件行 + `change_detail` 行（未知 id 恒 `None`）；D10（`flow-empty` 分支删除） |
| AC-5 | 13 命令参数面（公共 API 表逐条）；D7 分辨率单点；bindings 再生成 + 一致性守卫（组件表类型跟随行） |
| AC-6 | `control.rs` / `archive_flow.rs` 组件行（键 `(root, id)`，12 处随动）；walker provenance `<id>/…` 与归档两会话 `<id>/archive/*`；D10（前端 sourceRef 定式逐字一致） |
| AC-7 | D10（路由 `/changes/:id`、行键 / `navigate`、hooks 参数、props 收敛 `changeId`）；路由 / 清单页 / 详情页组件行 |
| AC-8 | D3 / D4 / D5（标记 + 删除重建 + 首开探测）；`open_workspace` 行（数据模型节打开流程）——打开成功 / 旧数据零可达 / 零迁移路径 / 全局库不受波及 |
| AC-9 | D2（语料固定 id 种子）+ D13（golden 显式重写范围声明；执行与复核归 test-design / test-gen / test-execution 阶段承接）；`id` 键入 golden 属声明范围 |
| AC-10 | D14 守线为静态检查面（fmt / clippy / knip / bindings 一致性 / 零 diff 边界 grep，无新增豁免）；全管线测试与 golden 复核由 test-execution 阶段承接 |
| AC-11 | D14（0.4.31 归档时执行）；`plugins/dev-team` 零改动（不变组件节） |

## 风险对齐（proposal R1–R8 → 设计落点）

R1 数据损失 / 能力回退 → D3 / D4 / D5（作废 = 删除重建，语义即接受的损失面；磁盘-only 条目零发现由 db 单源 + 零磁盘触点承载）；R2 身份键散落面广 → D7 分辨率单点 + 公共 API 表逐面签名 + D14 守线 grep 清单（`(root, change)` / `sourceRef` / `change:` 形参）；R3 前端双身份源 → D10（props 显名收敛 `changeId` + 单身份源断言面）；R4 source_ref 格式破反查 → 旧库整体作废无历史包袱 + D7 双侧逐字定式；R5 id 铸出点与确定性 → D1 / D2（铸出点定稿 + 语料固定 id 注入，uuid 普通依赖不破方向）；R6 golden 重写面 → D13（显式重写流程 + 范围声明 + 人工确认留痕）；R7 同名并存可读性 → D9 / D11 留痕（title 展示演进留后续变更，本变更提供 id 主键前提）；R8 文案错位 → `archive` 写面 / 命令组行内的文案随动（错误面以 id 语境呈现记录名）。

---

## 待决问题

无遗留阻塞项——proposal 六项待决已全部定稿（id 铸出点 / uuid 版本 → D1；语料注入 → D2；版本载体 → D3；作废时序 → D4；归档行展示 → D9；前端 props 收敛 → D10；name 唯一性边界 → D11；衔接措辞 → D5）。

边界留痕（非本变更待决）：

1. 同名并存可读性（active + archived 同名两行）——title 展示演进须以本 id 主键为前提，留后续变更偿还（R7）。
2. `ChangeSummary` / `ChangeDetail` 的 `Option` 收窄（`status` / `created` 恒在场后）——本设计按 D8 保留现状，如后续演进走各自 golden 显式重写流程。
