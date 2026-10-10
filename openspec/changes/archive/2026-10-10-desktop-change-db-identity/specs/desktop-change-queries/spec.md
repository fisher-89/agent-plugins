# desktop-change-queries Specification (Delta)

## MODIFIED Requirements

### Requirement: change 列表扫描与分组

workflow crate 的 queries SHALL 返回全量 change 列表，条目集合 SHALL 为 **workspace 库 ChangeRecord 全量**（唯一基准数据源）：MUST NOT 扫描磁盘目录——active / archive 两树目录发现与「db 记录 ∪ 磁盘目录去重并集」语义整体退役，无 db 记录的存量 CLI change 不再入列（零发现，见 desktop-change-state-store「双向墙」）。条目 SHALL 携状态面：`id`（身份锚）、`name`（恒裸名）、`status`（active | archived）、创建时间（`created_at`）、`active_phase` 运行中标注（可得时）。**状态归组以 db status 为权威**：带 worktree 记录的 active change 在 merge 前主仓两树未命中（目录位于其 worktree 内），SHALL 照常以 db status 归入进行中组，MUST NOT 因主仓目录缺席而被丢弃、MUST NOT 误归「未知时间」组，其工件 / 产物解析 SHALL 经记录的 worktree 路径（见 desktop-change-worktree）。归档 change SHALL 按月分组，分组时间 SHALL 以 `archived_at` 为唯一权威（磁盘目录名日期前缀不再参与分组），`archived_at` 缺失 SHALL 归入「未知时间」组而非被丢弃。缺失目录（如无归档树）SHALL NOT 影响列表结果（零磁盘触点，无降级分支）。inventory 代际标注（v0 / v1 / v2）SHALL 保持退役，MUST NOT 出现在列表 DTO。

#### Scenario: db 建档条目状态面呈现

- **WHEN** 对含已建档 change（active 与 archived 各若干）的 workspace 调用列表查询
- **THEN** 各条目携 id 与裸名 name、status、created_at，归档条目按 `archived_at` 月分组可达，active 运行中条目标注 active_phase

#### Scenario: worktree change 以 db status 归组

- **WHEN** 某 change 带 worktree 记录（status=active）且主仓 active / archive 两树均无其目录（merge 前）时调用列表查询
- **THEN** 该条目以 db status 归入进行中组且状态面完整呈现，不报错、不丢弃、不误归未知时间组

#### Scenario: 磁盘目录零发现

- **WHEN** 主仓 changes / archive 两树存在仅有 workflow.json 与 markdown 产物的存量目录（无 db 记录）
- **THEN** 清单零呈现该条目（不入列、不报错、不建档、零磁盘读取），目录字节原样未动

#### Scenario: 同名共存双行

- **WHEN** db 存在两条同名记录（一 active 一 archived，id 相异）
- **THEN** 清单各以自身 id 出列两行、status 如实，无并集去重、无条目顶撞（行键 id）

#### Scenario: 按月分组与未知时间兜底

- **WHEN** 某归档记录 `archived_at` 缺失（坏数据 / 手工注入）
- **THEN** 该条目归入「未知时间」组置尾，不丢弃

#### Scenario: 磁盘缺席不影响列表

- **WHEN** workspace 无归档目录树（或主仓目录被整体移除）
- **THEN** 列表照常全量出线（db 单源、零磁盘触点），不报错、无降级分支

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情（**按 id 寻址**），状态面单源自 workspace 库：9 站流水线（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / code-analyze）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变（存储级拆分，出线不动）。详情 SHALL 出线 `id`（身份锚）与 `name`（自记录直读，恒裸名——归档 change 的日期前缀仅存在于磁盘目录名，MUST NOT 进入出线值）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null，纯 derive 零派生改写口径），作为用户 review / 手动 commit / merge 的可达锚（见 desktop-change-worktree「worktree 路径 UI 可见性」）。产物发现与信封读取的路径解析 SHALL 经 **id → 记录 → name / worktree** 分辨率：`locate_change` 在记录带 worktree 时于主仓 active / archive 两树未命中回退 worktree 路径（目录解析单点收口）；`worktree=None` 的 change 维持主仓两树解析既有语义（archive 树含日期前缀后缀扫描）。**文档形态分支整体退役**：db 无该 id 记录时详情 SHALL 返回「未找到」（`None`），MUST NOT 回退磁盘目录解析、MUST NOT 返回空流水线文档形态、MUST NOT 读取任何 workflow.json。fileLog 区块与 unparsable 状态 SHALL 保持退役；`id` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程（wire contract 冻结记忆，`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）。

详情 SHALL 同时出线 **run 运行史全量读面**：`runs` 数组（自 RunRecord 全史投影，不截 last_run：runId / status / reason / 起止时刻）与每 run 的 `steps`（自 RunStepRecord 投影：phase / attempt / step / status / seq / sessionId / detail），供前端步节点常驻渲染与运行史审计消费（见 desktop-change-state-store「run 运行史落库」、desktop-change-flow-view）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null，转换收 queries 层单点）；DTO SHALL 纯 derive 零字段属性，runs / steps 字段演进 SHALL 走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕），bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致：active_phase 悬挂杀除后（desktop-change-state-store「启动标定与 active_phase 悬挂处置」），MUST NOT 出现无运行 run 而呈运行中的假象；在飞 run 的并入见 desktop-change-orchestration「运行控制命令面」读路统一。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件 matcher 发现，见 `desktop-artifact-plugins`），供前端按信封逐个请求；产物发现 SHALL 对带 worktree 记录与 legacy（worktree=None）两类 change 同等可用（均经 id → 记录定位，文档形态类别已整体退役）。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 归档 change 全状态面（错配修复锚点）

- **WHEN** 打开归档 change 详情（记录 status=archived，磁盘目录名为 `YYYY-MM-DD-<name>`，经 id 寻址）
- **THEN** 出线 `status=archived`、9 站 pipeline 与 runs 全量呈现（MUST NOT 落文档形态空面），`name` 为裸名、`id` 恒在案；产物清单经带前缀目录命中

#### Scenario: worktree 路径出线与产物经 worktree 解析

- **WHEN** 查询带 worktree 记录的 change 详情并逐个读取其产物信封
- **THEN** detail DTO 的 `worktree` 与库内记录值逐字一致；产物清单与信封读取命中 worktree 内目录（主仓两树未命中不致 detail / read_artifact 落空）

#### Scenario: legacy 与 null 出线

- **WHEN** 查询 `worktree=None` 的 change 详情
- **THEN** `worktree` 出线 null、产物解析走主仓两树既有语义，聚合不报错

#### Scenario: 未知 id 未找到

- **WHEN** 查询 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 详情返回「未找到」（`None`），零磁盘目录解析回退、零文档形态空面、workflow.json 零读取，不报错

#### Scenario: 运行中状态可见且不撒谎

- **WHEN** 某 change 的 ChangeRecord.`active_phase` 非空（phase 正在运行）
- **THEN** 详情标示该 phase 为运行中（含 attempt 序号与 start_at）；run 死亡后（含中途停止与重启标定）`active_phase` 已处置，详情 MUST NOT 呈运行中假象

#### Scenario: run 运行史全量出线

- **WHEN** 某 change 历经两次 run（第二次自续走推进）后查询详情
- **THEN** `runs` 出线两条全史记录（各自 status / reason / 起止），每 run `steps` 按 emit seq 稳定序出线（恰五词汇，流程面步骤零行），attempt 跨 run 递增不撞号

#### Scenario: golden 显式重写留痕

- **WHEN** detail DTO 增 `id` 字段（与列表 `id` 投影）后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/list.rs` | 列表 db 单源 | 零磁盘扫描（`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` 退役；签名收敛）；`ChangeSummary` 增 `id`、name 恒裸名；月分组 `archived_at` 唯一权威 |
| `crates/core/workflow/src/queries/mod.rs` | change 目录定位单点 | `locate_change(layout, worktree, name)` 名义签名不变（调用前经 id → 记录 → name 分辨率）；archive 后缀扫描与 `is_single_component_name` 校验保留 |
| `crates/core/workflow/src/queries/detail.rs` | 详情 id 寻址 | `change_detail(..., id)`：`get_change(id)` → 记录 → `locate_change`（记录 name / worktree）；文档形态分支删除；`ChangeDetail` 增 `id`；未知 id → `None` |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | 列表 db 单源与 id 字段演进走 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕）；语料见 desktop-corpus-regression |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeSummary.id` / `ChangeDetail.id` 经 export-bindings 再生成；一致性守卫拦截漂移 |
