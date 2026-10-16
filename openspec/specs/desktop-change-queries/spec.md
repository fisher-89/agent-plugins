# desktop-change-queries Specification

## Purpose

定义 workflow crate 查询层的聚合契约：change 列表为 workspace 库 ChangeRecord 全量（db 单源、零磁盘扫描、按 id 寻址、按月分组），单 change 详情聚合（9 站流水线、PhaseRecord attempt 序列、active_phase、backtrack、会话槽位、run 运行史全量读面），查询层保持纯读且无指令概念。

## Requirements

### Requirement: change 列表扫描与分组

workflow crate 的 queries SHALL 返回全量 change 列表，条目集合 SHALL 为 **workspace 库 ChangeRecord 全量**（唯一基准数据源）：MUST NOT 扫描磁盘目录——active / archive 两树目录发现与「db 记录 ∪ 磁盘目录去重并集」语义整体退役，无 db 记录的存量 CLI change 不再入列（零发现，见 desktop-change-state-store「双向墙」）。条目 SHALL 携状态面：`id`（身份锚）、`name`（恒裸名）、`title`（人类可读标题，恒非空——自记录直读，见 desktop-change-state-store）、`status`（active | archived）、创建时间（`created_at`）、`active_phase` 运行中标注（可得时）。**状态归组以 db status 为权威**：带 worktree 记录的 active change 在 merge 前主仓两树未命中（目录位于其 worktree 内），SHALL 照常以 db status 归入进行中组，MUST NOT 因主仓目录缺席而被丢弃、MUST NOT 误归「未知时间」组，其工件 / 产物解析 SHALL 经记录的 worktree 路径（见 desktop-change-worktree）。归档 change SHALL 按月分组，分组时间 SHALL 以 `archived_at` 为唯一权威（磁盘目录名日期前缀不再参与分组），`archived_at` 缺失 SHALL 归入「未知时间」组而非被丢弃。缺失目录（如无归档树）SHALL NOT 影响列表结果（零磁盘触点，无降级分支）。inventory 代际标注（v0 / v1 / v2）SHALL 保持退役，MUST NOT 出现在列表 DTO。

#### Scenario: db 建档条目状态面呈现

- **WHEN** 对含已建档 change（active 与 archived 各若干）的 workspace 调用列表查询
- **THEN** 各条目携 id、裸名 name、title、status、created_at，归档条目按 `archived_at` 月分组可达，active 运行中条目标注 active_phase

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

#### Scenario: title 出线

- **WHEN** 某 change 记录 `title` 为「API 重试调研」而 `name` 为 `api-retry`
- **THEN** 列表条目 `title` 为「API 重试调研」、`name` 仍为裸名 `api-retry`（title 为纯投影零派生改写）

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情（**按 id 寻址**），状态面单源自 workspace 库：流水线相位序 SHALL 随 `record.workflow_type` 出线（`phase_table(workflow_type)` 的相位序——requirement 9 列、test-only 6 列；MUST NOT 使用固定 10 站 `PIPELINE_PHASES` 常量）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变。详情 SHALL 出线 `id`（身份锚）、`name`（恒裸名）与 `title`（恒非空）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null；归档清理后字段原样保留——历史锚，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）。产物发现与信封读取的路径解析 SHALL 经 **id → 记录 → name / worktree** 分辨率；`worktree=None` 的 change 维持主仓两树解析既有语义。**文档形态分支整体退役**：db 无该 id 记录时详情 SHALL 返回「未找到」（`None`），MUST NOT 回退磁盘目录解析、MUST NOT 读取任何 workflow.json。`id` / `title` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程。

详情 SHALL 同时出线 **run 执行图全量读面**：`runs` 数组（自 RunRecord 全史投影），每 run 的 `nodes`（自 RunNodeRecord 投影：phase / attempt / step / instanceNo / stepKind / status / sessionId / detail / outputRef）与 `edges`（自 RunEdgeRecord 投影：fromNodeKey / toNodeKey / edgeKind ∈ forward / feedback / retry / backtrack）。节点身份键 `(phase, attempt, step, instanceNo)` SHALL 为稳定出线键；边 SHALL 显式出线。节点 `stepKind` SHALL 为二十值全词汇（含 `mutation_test` 与 9 个归档步词——`cleanup` 在案）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null）；DTO SHALL 纯 derive 零字段属性，流水线相位序 / 二十值 nodes / edges 字段演进 SHALL 走 golden 显式重写流程，bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致；archive 相位完成落账后 `phase_next` 路由 done，详情 MUST NOT 呈运行中假象（有界降级例外——通知重查失败轮保 last-good 的过渡态由 desktop-change-flow-view「通知重查失败降级与有界重试」承载：有界重试、带「刷新失败」内联提示、收敛保证；本条款禁杀的是无界、静默的假态，强度不变）。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件按 phase 定义清单驱动发现）；archive 与 code-analyze 相位无产物清单（不产 proposal / design / tasks 类文档），MUST NOT 为 archive / code-analyze 列挂过程文档。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 归档 change 全状态面（错配修复锚点）

- **WHEN** 打开归档 change 详情（记录 status=archived，磁盘目录名为 `YYYY-MM-DD-<name>`，经 id 寻址）
- **THEN** 出线 `status=archived`、相位序随 workflow_type 的 pipeline 与 runs 全量呈现（archive 列 PhaseRecord 与 step 节点在案），`name` 为裸名、`id` 恒在案；产物清单经带前缀目录命中

#### Scenario: worktree 路径出线与产物经 worktree 解析

- **WHEN** 查询带 worktree 记录的 change 详情并逐个读取其产物信封
- **THEN** detail DTO 的 `worktree` 与库内记录值逐字一致；产物清单与信封读取命中 worktree 内目录

#### Scenario: legacy 与 null 出线

- **WHEN** 查询 `worktree=None` 的 change 详情
- **THEN** `worktree` 出线 null、产物解析走主仓两树既有语义，聚合不报错

#### Scenario: 未知 id 未找到

- **WHEN** 查询 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 详情返回「未找到」（`None`），零磁盘目录解析回退、零文档形态空面、workflow.json 零读取，不报错

#### Scenario: 运行中状态可见且不撒谎

- **WHEN** 某 change 的 ChangeRecord.`active_phase` 非空（含 archive 相位运行中）
- **THEN** 详情标示该 phase 为运行中；run 死亡后 `active_phase` 已处置，详情 MUST NOT 呈运行中假象

#### Scenario: run 执行图全量出线

- **WHEN** 某 change 历经一次含 archive 相位、feedback 重跑、phase 重试与 backtrack 的 run 后查询详情
- **THEN** `runs` 出线全史记录；每 run `nodes` 按稳定身份键出线（二十值全词汇零过滤、mutation_test 与 archive 9 个步词（含 cleanup）在案、feedback 重跑为 `instanceNo` 递增新节点），`edges` 显式出线四 kind；无 `seq` 字段、无 `deriveEdges` 派生残留

#### Scenario: 10 站流水线 golden 显式重写

- **WHEN** detail DTO 相位序随 workflow_type 出线后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: title 出线与 golden 显式重写

- **WHEN** 查询 title 非空的 change 详情
- **THEN** `title` 自记录直读逐字一致；detail DTO 增 `title` / nodes / edges 后运行语料回归测试，diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: golden 显式重写留痕

- **WHEN** detail DTO 增 `id` 字段（与列表 `id` 投影）后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: 相位序随 workflow_type 出线

- **WHEN** 分别查询 `workflow_type="requirement"` 与 `"test-only"` 的 change 详情
- **THEN** requirement 的 `pipeline` 为 9 列（proposal…archive，无 code-analyze，archive 第 9 列）、test-only 为 6 列（proposal / code-analyze / test-design / test-gen / test-execution / archive，archive 第 6 列）；`pipeline` 相位序与 `phase_table(workflow_type)` 逐一一致

#### Scenario: 未知 workflow_type 读面不退化

- **WHEN** 查询 `workflow_type` 为不受支持值（如 `"bug-fix"`）的 change 详情
- **THEN** 详情按 design 定稿形态呈现（`None` / 空 pipeline / 显式错误三选一），MUST NOT 静默按 requirement 的 9 列出线；产物清单不误按 requirement 清单发现

### Requirement: 详情暴露 attempt 会话槽位

change 详情聚合 SHALL 在 attempt 记录（PhaseRecord 的线面投影）中暴露记录在案的会话槽位：executor / evaluator / decision 三槽位的会话 id，自 PhaseRecord 三槽位列直读透出（与 `SessionRecord` 同库，join 为库内一等查询）。槽位列 None 的记录（缺省落账）SHALL 以 null 呈现三值，MUST NOT 报错、MUST NOT 以 sourceRef 反查等派生手段虚构槽位值。run 执行图节点行（`runs[].nodes[].sessionId`）SHALL 直读透出，作为前端会话转录的 sessionId 直查键（前端不再以 provenance `source_ref` 反查会话；`source_ref` 仍为后端 `SessionProvenance` 留痕字段，见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」，前端零消费）。DTO SHALL 沿既定口径（纯 derive、零字段属性、时间出线 ISO 串 + null），经 bindings 再生成同步前端类型。查询层 SHALL 保持纯读：槽位为直读透出，MUST NOT 在查询层写入或回填 db。

#### Scenario: 含槽位记录透出会话 id

- **WHEN** 查询某 PhaseRecord 携带 executor / evaluator / decision 会话槽位的 change 详情
- **THEN** 对应 attempt 记录的三个会话 id 与库内记录值逐字一致，无派生改写

#### Scenario: 缺槽位记录降级 null

- **WHEN** 查询槽位列为 None 的 PhaseRecord（缺省落账）
- **THEN** 全部 attempt 记录的三槽位均为 null，聚合不报错、详情其余区块照常

#### Scenario: run 节点 sessionId 直出任转录直查键

- **WHEN** 查询含已发起 WorkerAgent 步的 run 详情
- **THEN** 各节点行 `sessionId` 与库内 `RunNodeRecord.session_id` 逐字一致（WorkerAgent 步有值、非 WorkerAgent 步为 null）；前端经该 id 直查会话转录，零 `source_ref` 反查消费

#### Scenario: bindings 同步

- **WHEN** DTO 演进后执行 bindings 重导出
- **THEN** 前端生成物含槽位与线面字段最新形状，一致性守卫（git diff --exit-code）绿

### Requirement: 查询层纯读且无指令概念

queries SHALL 为纯读操作：只读 workspace 文件与 workspace 库（经只读查询面），MUST NOT 写入、移动或修改任何文件，MUST NOT 产生 db 写事务。域 crate（workflow）MUST NOT 出现指令 / 命令概念（exec 属 desktop-app 层的预留轨道）；域 crate MUST NOT 依赖 Tauri。

#### Scenario: 只读保证

- **WHEN** 连续执行列表与详情查询
- **THEN** workspace 目录树的文件内容与结构、workspace 库的记录状态均无任何变化

#### Scenario: 域 crate 无 Tauri 依赖

- **WHEN** 检查 `workflow` crate 的依赖
- **THEN** 无 Tauri 相关依赖，queries 可独立于桌面壳被测试复用

### Requirement: 详情查询错误语义分流（None / Err）

详情查询 SHALL 将两种「详情不可达」形态分流出线，MUST NOT 混同：

- **None = 真「未找到」**：db 无该 id 记录（含 `workflow_type` 不在相位表内的既有 `None` 语义，见「change 详情聚合」）→ `Ok(None)`；前端「未找到」兜底页语义不变。
- **Err = 读失败**：workspace 库读 `Err`（`get_change` 返回 `StoreFault`）SHALL 以 `Err` 原样传播出线（错误串透传到命令层与前端），MUST NOT 经 `.ok()` / `unwrap_or_default` 降级为 `None` 或空面——读失败 MUST NOT 呈「未找到」假象（运行中的 change 因 store 瞬时读错被前端整页塌兜底的竞态由此封堵）。

core `change_detail` 签名 SHALL 由 `Option<ChangeDetail>` 改为 `Result<Option<ChangeDetail>, StoreFault>`；既有其余子表读（`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges`）的 `unwrap_or_default` 单区块空面降级哲学 SHALL 保持不变（单区块读失败不升级为整份 `Err`）。命令面 `get_change_detail` SHALL 随动为 `Result<Option<ChangeDetailUnified>, String>`：blank root 维持空结果 `Ok(None)` 语义；`StoreFault` → 错误串映射收命令层单点。DTO 线面形状零演进：`Result<Option<T>, String>` 经 bindings 出线仍为 `Promise<T | null>`（Err 走 reject 通道），前端类型面零漂移、golden 零重写。前端失败轮的保 last-good 降级行为由 desktop-change-flow-view「通知重查失败降级与有界重试」承载。

#### Scenario: store 读 Err 传播不降级

- **WHEN** mock store 的 `get_change` 返回 `Err(StoreFault)`（该 id 记录实际在库）
- **THEN** `change_detail` 返回 `Err`（fault 错误串透传），MUST NOT 出 `Ok(None)`（未找到假象）、MUST NOT 出部分区块空面的降级 `Some`

#### Scenario: 未知 id 仍 Ok(None)

- **WHEN** 查询 db 无该 id 记录的 id（含磁盘存在同名目录的情形）
- **THEN** `change_detail` 返回 `Ok(None)`（真「未找到」），零磁盘目录解析回退、零 workflow.json 读取，不报错——既有语义回归

#### Scenario: 命令面 Err 出线为 reject

- **WHEN** core `change_detail` 返回 `Err`
- **THEN** IPC `get_change_detail` 以 store 错误串 reject（前端 invoke catch 面可承接），非 `null` 应答、非 panic；blank root 仍 `Ok(None)`（空结果语义不变）

#### Scenario: 子表读降级哲学不变

- **WHEN** `get_change` 读 `Ok(Some)` 而 `list_runs` 读 `Err`
- **THEN** 详情照常 `Ok(Some)` 出线（runs 为空数组），单区块缺失不升级为整份 `Err`——既有 `unwrap_or_default` 哲学保持

#### Scenario: bindings 零漂移

- **WHEN** 签名 Result 化后执行 export-bindings 再生成
- **THEN** `getChangeDetail` TS 类型面不变（`Promise<ChangeDetailUnified | null>`，Err 走 reject 通道），一致性守卫绿、golden 零 diff

### Requirement: 步级时间与 token 出线

detail 线面 `ChangeRunNodeRecord` SHALL 增四字段出线：`startedAt: number` / `endedAt: number | null` / `tokenInput: number | null` / `tokenOutput: number | null`（自 `RunNodeStateRecord` 直读透出）。出线口径 SHALL 为**节点级毫秒数**（与活面 `ChangeStepState` 同形——统一视图 `UnifiedRunStep` 单一形状、`stepFromRecord` 纯复制、running 走秒直减 `Date.now()` 零 parse）；runs 级 `ChangeRunEntry.startedAt` / `finishedAt` 的 ISO 串口径 SHALL 维持不变——两口径并存为有意裁定留痕（节点级时间为节点自有数据而非聚合时刻）。DTO SHALL 保持纯 derive 零字段属性（旧库行缺省值 `0` / `null` 原样出线，MUST NOT 在查询层改写或过滤）；字段演进 SHALL 走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕），bindings 再生成同步前端类型。查询层 SHALL 保持纯读（MUST NOT 回填或派生任何时间 / token 值）。

#### Scenario: 四字段出线与活面同形

- **WHEN** 查询含已收口 WorkerAgent 步（token 有值）与工具步的 run 详情
- **THEN** `runs[].nodes[]` 各行携 `startedAt` / `endedAt`（毫秒数）、`tokenInput` / `tokenOutput`（WorkerAgent 步有值、工具步 null），与活面 `ChangeStepState` 字段形状一致；`stepFromRecord` 无字段转换

#### Scenario: 旧行缺省值原样出线

- **WHEN** 查询含本变更前落库 run 的详情（节点行无步级时间 / token）
- **THEN** 各节点 `startedAt=0` / `endedAt=null` / `tokenInput=null` / `tokenOutput=null` 原样出线（零改写、零回填），聚合不报错、详情其余区块照常

#### Scenario: runs 级 ISO 口径不变

- **WHEN** 对同一详情响应检查 runs 级与节点级时间字段
- **THEN** `runs[].startedAt` / `finishedAt` 仍为 ISO 串 + null；节点级四字段为毫秒数 / null——两口径并存，无相互转换逻辑

#### Scenario: golden 显式重写

- **WHEN** detail DTO 节点级四字段出线后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/list.rs` | 列表 db 单源 | 零磁盘扫描（`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` 退役；签名收敛）；`ChangeSummary` 增 `id`、name 恒裸名；月分组 `archived_at` 唯一权威 |
| `crates/core/workflow/src/queries/mod.rs` | change 目录定位单点 | `locate_change(layout, worktree, name)` 名义签名不变（调用前经 id → 记录 → name 分辨率）；archive 后缀扫描与 `is_single_component_name` 校验保留 |
| `crates/core/workflow/src/queries/detail.rs` | 详情 id 寻址与线面收敛 | `change_detail(..., id)`：`get_change(id)` → 记录 → `locate_change`（记录 name / worktree）；文档形态分支删除；`ChangeDetail` 增 `id`；未知 id → `None`；`inventory` / `fileLog` / `unparsable` 字段删；槽位三列直读透出（None → null）；纯 derive 零字段属性口径不变 |
| `crates/infra/store`（workspace 库实例，经 port 缝消费） | 状态读源 | ChangeRecord / PhaseRecord / ChecklistItemRecord 只读查询；时间转换收 queries 层单点（i64 millis → ISO 串 + null） |
| `foundation::layout` | 路径输入 | 产物发现仅经 `Layout` 取目录，不自行拼磁盘路径 |
| `workflow::artifacts` | 产物发现协作 | 详情返回产物清单，发现机制复用 matcher 注册表；带 worktree 记录与 legacy 两类 change 同等可用 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | 列表 db 单源与 id 字段演进走 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕）；语料见 desktop-corpus-regression |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeSummary.id` / `ChangeDetail.id` 经 export-bindings 再生成；一致性守卫拦截漂移 |
| `crates/core/workflow/src/queries/`（detail DTO stepKind 投影） | 二十值出线 | `RunStepKind` 词面扩展自动跟随（纯 derive 零字段属性）；golden 覆盖 cleanup 步词样本经显式重写 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `RunStepKind` 词面再生成；`bindings:check` 守卫拦截漂移 |
| `crates/core/workflow/src/queries/detail.rs` | 错误语义分流单点 | `change_detail` 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`：`get_change` 读 `Err` 传播（`.ok()` 吞错消除）；子表读 `unwrap_or_default` 哲学不变；DTO 线面形状零演进 |
| `src/commands/changes/mod.rs`（`get_change_detail`） | IPC 面分流 | 返回 `Result<Option<ChangeDetailUnified>, String>`：blank root 仍 `Ok(None)`；`StoreFault` → 错误串映射收命令层单点 |
| `crates/core/orchestration/src/snapshot.rs`（机械随动） | 唯一 orchestration 调用点适配 | `Err` 透传 store 错误串（取代误报「change 不存在」）；`None` 仍「change 不存在: {id}」；`WorkflowSnapshotPort::detail` 契约与 walker 消费面不变 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型零漂移 | `Result<Option<T>, String>` 出线 `Promise<T \| null>`（Err 走 reject 通道）；再生成预期零 diff、一致性守卫绿 |
| `crates/core/workflow/src/queries/detail.rs`（`ChangeRunNodeRecord`） | 线面字段扩展 | 增 `startedAt: number` / `endedAt: number | null` / `tokenInput` / `tokenOutput`（毫秒数口径、纯 derive）；`From<&RunNodeStateRecord>` 直读透出 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeRunNodeRecord` 四字段再生成；`bindings:check` 守卫拦截漂移 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | detail nodes 投影四字段入 golden 显式重写流程（语料维度见 desktop-corpus-regression delta） |
