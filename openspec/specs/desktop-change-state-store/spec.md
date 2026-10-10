# desktop-change-state-store Specification

## Purpose

定义 change 流程状态的 workspace 库持久化契约：四状态模型（ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord）单源承载、与 workflow.json 的双向墙、相位机写面单事务落库、归档双写与步骤审计——desktop 的 change 流程状态不再驻磁盘 JSON 文件。

## Requirements

### Requirement: change 流程状态 workspace 库单源与双向墙

desktop 的 change 流程状态 SHALL 以 workspace 库（app-data `workspaces/` 子树 per-root db 文件）为唯一载体，状态 SHALL NOT 再驻 `openspec/changes/<name>/workflow.json`。store SHALL 维护六个 workspace 维度模型（落所属 workspace 库，native_model 版本治理，时间戳 i64 UTC unix millis 口径）：

- `ChangeRecord`：`name`（String 主键，即 change 名，身份主键）、`workflow_type`、`created_at`、`status`（`active` | `archived`）、`archived_at`（Option）、`active_phase`（Option：phase / attempt / start_at）、`worktree`（Option：该 change 分配的 worktree 绝对路径；None = legacy 主 root change，存量语义，见 desktop-change-worktree）、`base_commit`（Option：创建基线 fork 点，调试 / UI 价值）；
- `PhaseRecord`：`id`（i64 主键，写事务内 max+1 分配）、`change`（二级索引）、`phase`、`attempt`、`verdict`、`report`（内联 ≤2000 字）、`skipped`、`stale`、`backtrack_to` / `backtrack_reason`、executor / evaluator / decision 三会话槽位（Option）、`start_at`、`timestamp`；
- `ChecklistItemRecord`：打包主键 `(phase_id as u128) << 64 | item_index`（同相位内 item 序即自然序）、`phase_id` 二级索引、`item` / `pass` / `evidence`；
- `StepRecord`：`run_id`、`change`、`step_kind`（封闭集：`phase_next` | `phase_start` | `phase_log` | `backtrack` | `decision_log` | `static_check` | `test_execution`）、`status`、时间戳、有界输出摘要；
- `RunRecord`：`run_id`（String 主键，即 walker 既有 run id 词汇）、`change`（二级索引）、`status`（`running` | `completed` | `stopped` | `failed` | `interrupted`）、`reason`（Option）、`started_at` / `finished_at`（Option）——run 运行史主行，形态细则见「run 运行史落库」；
- `RunStepRecord`：`run_id`（二级索引）、`phase` / `attempt` / `step`（封闭集五词汇：`executor` | `evaluator` | `decision` | `static_check` | `test_execution`）、`status`、`seq`（emit 序）、`session_id`（Option）、`detail`（Option，有界）、时间戳——run 步节点史行，形态细则见「run 运行史落库」。

`ChangeRecord` 的 `worktree` / `base_commit` 两字段 SHALL 经 native_model 版本升级（v1→v2 decode-only：旧版本记录自动升级读出、两字段为 None，provider `context_length` v1→v2 先例同模式）原地演进，MUST NOT 手工迁移步骤或 legacy 迁移层；存量库 SHALL additive 打开。`worktree` 字段为执行锚引用（读面与 run 组合消费，见 desktop-change-worktree / desktop-change-orchestration），MUST NOT 反向影响库身份锚定：workspace 库路径派生仍恒以 workspace root 为锚。

**双向墙**：desktop 全链（写面 / 查询 / 编排 / 命令 / 前端）MUST NOT 读或写任何 `workflow.json`——db-backed change 对 CLI 工作流不可见，反向亦然；磁盘 change 目录只承载 markdown 产物（proposal / design / tasks / specs / reports / explore；对 worktree change 该目录位于其 worktree 内，merge 前不在主仓）。存量 CLI change（有 workflow.json 无 db 记录）SHALL 零迁移零触碰，desktop 以文档形态展示其产物。

#### Scenario: 状态回环读写

- **WHEN** 对某 workspace 建档 change 并推进若干相位（phase_log 落 eval 与 checklist、backtrack 落 stale 标记）后关闭并重开同一 db 文件查询
- **THEN** ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord / RunRecord / RunStepRecord 逐字段与写入一致（含三会话槽位、worktree 与 base_commit、时间戳原值），状态查询不依赖任何磁盘 workflow.json

#### Scenario: 双向墙机械可验

- **WHEN** 扫描 desktop 全部源码（core / infra / commands / views）对 `workflow.json` 的读与写触点
- **THEN** 零触点（测试夹具内的字面量除外）；parse 模块已删除，desktop 不再解析该文件

#### Scenario: 历史 CLI change 文档形态

- **WHEN** 打开一个仅有 workflow.json 与 markdown 产物、无 db 记录的存量 change
- **THEN** 目录与其 workflow.json 字节原样未动，清单可见、详情以文档形态呈现（空状态面 + 产物清单），不报错、不建档

#### Scenario: 存量 ChangeRecord decode-only 升级

- **WHEN** 打开含 v1 形态 `ChangeRecord`（无 worktree / base_commit 字段）的存量 workspace 库并查询
- **THEN** 记录经 native_model 版本机制自动升级可读，`worktree` / `base_commit` 读出 None（legacy 主 root 语义），无手工迁移步骤、无 legacy 迁移层；新建档记录两字段按写入回读一致

#### Scenario: worktree 字段不反噬库身份

- **WHEN** 某 workspace 存在带 worktree 记录的 change，重开其 workspace 库
- **THEN** db 文件路径仍由 workspace root 身份派生（`workspaces/` 子树），worktree 路径仅作为记录字段被读取，从不参与库路径派生

### Requirement: 相位机写面单事务落库

写面 `phase_log` SHALL 在单个 redb 写事务内原子完成：PhaseRecord 落行 + 该条目全部 ChecklistItemRecord 落行 + `ChangeRecord.active_phase` 更新 + backtrack 触发的 stale 翻转传播——任一环节失败 SHALL 整体回滚，MUST NOT 留半截状态。`(change, phase, attempt)` SHALL 在写事务内查重（沿 store 既有唯一性惯例，同 `AgentProviderRecord.name`），重复落账 SHALL 返回 `StoreError`。checklist 落行顺序 SHALL 保持 evaluator 输出序（打包键同相位内自然序）。`report` 内联上限 2000 字与插件 500 字读侧上限彻底解耦（desktop 自有 schema）。

#### Scenario: 单事务原子零残留

- **WHEN** phase_log 落库过程中某环节失败（如 checklist 编码错误注入）
- **THEN** 写事务整体回滚：无 PhaseRecord 行、无 ChecklistItemRecord 行、active_phase 与 stale 标记零变化

#### Scenario: 重复 attempt 被查重拒绝

- **WHEN** 对已存在 `(change, phase, attempt)` 记录的组合再次 phase_log
- **THEN** 返回显式 `StoreError`，库内记录数量与内容零变化

#### Scenario: checklist 顺序保真

- **WHEN** evaluator 输出含多条 checklist 项的 phase_log 落库后按打包键扫描
- **THEN** 条目按 item_index 升序返回且与 evaluator 输出序逐条一致

### Requirement: 归档双写

写面 SHALL 提供 `archive` 操作：db `ChangeRecord.status` 翻转为 `archived`（附 `archived_at`）与主仓 active→archive 目录改名（`YYYY-MM-DD-<name>` 日期前缀）SHALL 以同一提交语义完成（双写顺序与失败补偿由 design 定稿）；`ChangeRecord` 主键 `name` MUST NOT 随目录改名变化（db 身份与磁盘目录名解耦）。对带 worktree 记录的 change，归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该目录时 SHALL 显式拒绝并引导「先 merge worktree 分支回主仓再归档」（merge-first 语义见 desktop-change-worktree；归档编排链对前置的自动满足路径见 desktop-change-archive）；merge 后主仓目录在场，既有双写语义零特判。**双写操作本体** MUST NOT 触碰 worktree / branch（清理为用户手动边界）；归档编排链（desktop-change-archive）前置段的 worktree 提交与主仓合入不属双写本体，且双写收口 SHALL 保持归档链末段唯一收口触点身份（单点复用、语义零改动）。对无 db 记录的目录 MUST NOT 提供归档（显式拒绝）。IPC 面 SHALL 提供对应薄命令（三件事纪律，`Result<T, String>`）。

#### Scenario: 双写一致

- **WHEN** 对已建档 change 调用归档命令成功
- **THEN** db status 为 `archived` 且 `archived_at` 在案，主仓磁盘目录已迁移至 archive 树并带日期前缀，两者任一不存在即视为失败

#### Scenario: 主键不随改名变

- **WHEN** 归档后按 change 名查询 db 与按月分组的归档列表
- **THEN** `ChangeRecord.name` 保持原名，归档条目经原名可达且月份分组正确

#### Scenario: 未 merge 的 worktree change 归档被引导拒绝

- **WHEN** 对带 worktree 记录且主仓 active / archive 两树均未命中目录的 change 发起归档
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支（非泛化「目录未找到」），db 与磁盘零变化

#### Scenario: 双写本体零 worktree / branch 触碰

- **WHEN** 归档链末段双写收口执行（其前置段已由 desktop-change-archive 完成提交与合入）
- **THEN** 双写操作自身仅做目录改名与 db 翻转：零 `git worktree` / `git branch` 调用、零 worktree / branch 状态变更（清理仍为用户手动边界）

#### Scenario: 无建档目录拒绝归档

- **WHEN** 对仅有 workflow.json 的 CLI change 目录发起归档
- **THEN** 显式拒绝（无 ChangeRecord 不可归档），零落账零目录改动

### Requirement: 步骤审计落库可查

写面与编排的相位机步骤（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `decision_log` / `static_check` / `test_execution`）SHALL 逐条落 StepRecord 审计行，行携 `run_id` 使同一 run 的步骤序列可串链；审计查询 SHALL 可按 change（与 run）枚举步骤历史（新增能力）。StepRecord SHALL 为审计面，MUST NOT 作为 run 恢复依据：run 状态仍驻内存 ChangeFlowControl，重启后 run 由 `phase_next` 依 eval 历史重算。输出摘要 SHALL 有界截断（上限对齐 report ≤2000 口径，具体值 design 定稿），完整输出以引用（checks 报告目录 / 会话 id）出全量，MUST NOT 全文落库。

#### Scenario: 步骤历史可枚举

- **WHEN** 一次 run 走完 proposal 相位（含 phase_next / phase_start / executor / static_check / evaluator / phase_log）后按 change 查询步骤审计
- **THEN** 各步骤行齐备且按时间序可枚举，同 run 行经 `run_id` 串链

#### Scenario: 审计不承担恢复

- **WHEN** 桌面重启后对遗留 active_phase 的 change 重新发起 run
- **THEN** 续走由 `phase_next` 依 eval 历史重算（StepRecord 不参与状态重建），已 pass 相位不重头执行

#### Scenario: 摘要有界且引用可达

- **WHEN** 某 static_check 步骤输出超长诊断文本后检查其 StepRecord
- **THEN** 摘要字段不超过既定上限且截断留痕，引用字段指向 checks 报告树中的完整输出位置

### Requirement: run 运行史落库

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunStepRecord` 两表，全史保留不截 last_run——多次 run 全量留存为运行史审计面；attempt 经 `phase_start` max+1 分配跨 run 不撞号，全史叠加在同一列面分层）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立（支撑启动标定），run 收口时 SHALL 以终态更新（status / reason / finished_at）+ 该 run 全部步整包**单事务**落库；MUST NOT 每步写放大（无每步写事务）。步整包内 `seq` SHALL 取 emit 序且 finish 保序落（重挂恢复时 live 面与库面的拼接依赖稳定序）。

步词汇 SHALL 为封闭集单点定义，**落户子集 = 回显子集**：仅 agent 阶段（`executor` / `evaluator` / `decision`）与脚本阶段（`static_check` / `test_execution`）五词汇落库；流程面步骤（`phase_start` / `phase_log` 与三门 `verdict_gate` / `retry_gate` / `whitelist_gate`——纯 Rust 分支，无会话无独立产物）SHALL NOT 落 run_steps。过滤词汇 SHALL 单点定义、落库写面与读面同一份消费；walker 与 control MUST NOT 持有过滤逻辑。`RunStepRecord` 的 `session_id`（WorkerAgent 步）/ `detail`（人读记因 / 摘要，有界截断）SHALL 随行落库。

`RunStepRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行（职责与词汇不变，见「步骤审计落库可查」），RunStepRecord 为图节点史行；两表 MUST NOT 互为替代，词汇重叠项（`static_check` / `test_execution` / decision 面）SHALL 各自单点、MUST NOT 双写同一语义行。run 中途崩溃（无 finish 落包）SHALL 丢该 run 步史——start 行经启动标定可见（见「启动标定与 active_phase 悬挂处置」），为既定权衡留痕，MUST NOT 以每步写透补偿。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static_check / evaluator 多步）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunStepRecord 整包落库；期间零每步写事务

#### Scenario: 步整包原子

- **WHEN** finish 落包过程中某环节失败（如步行编码错误注入）
- **THEN** 写事务整体回滚：无 RunStepRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 词汇口径一致

- **WHEN** 一次 run 覆盖全部十类步词汇（含 phase_start / phase_log 与三门）后查询 run_steps
- **THEN** 恰五词汇落行（executor / evaluator / decision / static_check / test_execution），流程面步骤零行；落库写面与读面消费同一份过滤词汇单点

#### Scenario: 全史保留与跨 run 不撞号

- **WHEN** 同一 change 先后发起两次 run（第二次自续走锚点推进）
- **THEN** 两 run 的 RunRecord 与各自 RunStepRecord 全量在库，同 phase 的 attempt 经 max+1 分配递增不撞号，按 `(run 起始, emit 序)` 可稳定分层枚举

#### Scenario: 崩溃丢步史为既定权衡

- **WHEN** run 运行中途进程被 kill（finish 落包未达）后重启查询
- **THEN** 该 run 的 RunRecord 经启动标定为 interrupted、其 RunStepRecord 零行（过程史不可回放），attempt 级评估史仍可达自 PhaseRecord；库中无每步写透补偿痕迹

### Requirement: 启动标定与 active_phase 悬挂处置

store SHALL 提供启动标定操作：启动（workspace 库打开 / 应用装配）时发现库中 `RunRecord.status=running` 残留行 SHALL 标定为 `interrupted`（重启后 run 客观已死，库 MUST NOT 继续声称 running）；`interrupted` SHALL 仅由启动标定产生，运行期写路径 MUST NOT 写入该值。库中 run 状态词汇 SHALL 为 `running` + 终态三值（`completed` / `stopped` / `failed`）+ `interrupted`；停等态（waitingAsk / waitingConfirm）SHALL NOT 落库（应答通道活在进程内，见 desktop-change-orchestration）。

run 收口的 finish 落包 SHALL 在同一事务内处置 `ChangeRecord.active_phase`（杀悬挂）：run 死亡（终态收口）后 `active_phase` MUST NOT 停留 `Some` 悬挂——处置形态（清位 vs 转「上次运行停在 X」静态记录）由 design 定稿，但处置后详情状态面 MUST NOT 在无运行 run 时呈「运行中」假象（见 desktop-change-queries「change 详情聚合」）。active_phase 处置 MUST NOT 破坏续走语义：重新发起 run SHALL 自续走锚点推进、已 pass 相位 MUST NOT 重头执行（锚点与处置形态的关系由 design 定稿论证）。

#### Scenario: 残留 running 标定

- **WHEN** 构造库中 status=running 的残留 RunRecord（模拟中途死亡）后启动装配执行标定
- **THEN** 该行翻为 interrupted（附标定记因），再次启动无 running 残留可标定（幂等）

#### Scenario: 悬挂杀除

- **WHEN** run 在相位中途被停止（phase_log 未达、active_phase 在位）收口后查询详情
- **THEN** active_phase 已按 design 定稿形态处置，详情头部不呈「运行中 · phase · attempt」；库中无「active_phase=Some 且无在飞 run」的记录形态残留

#### Scenario: 续走不重头

- **WHEN** 悬挂处置后的 change 重新发起 run
- **THEN** 续走自锚点推进（已 pass 相位不重头执行），attempt 序延续不撞号

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（四模型，新） | change 状态持久化形状 | `ChangeRecord`（name 主键 / status / active_phase）/ `PhaseRecord`（id max+1 / report ≤2000 内联 / 三会话槽位）/ `ChecklistItemRecord`（打包键 `(phase_id as u128) << 64 \| item_index`）/ `StepRecord`（step_kind 封闭集 / 有界摘要）；native_model 版本治理；workspace 维度落所属 workspace 库 |
| `crates/infra/store/src/model.rs`（ChangeRecord v2） | worktree / base_commit 字段与版本升级 | `worktree: Option<String>`（None = legacy 主 root）、`base_commit: Option<String>`；native_model v1→v2 decode-only（旧记录自动升级读 None）；字段为执行锚引用，不参与库身份派生 |
| `crates/infra/store/src/store.rs`（change 域操作面，新） | 建档 / 相位落账 / 步骤追加 / status 翻转 | 写事务内 max+1 与查重；phase_log 单事务原子；信封 API（`list_models` / `scan`）零改动覆盖 |
| `crates/core/workflow/src/write/`（落库 port 缝，新） | 写面 → store 的进程内缝 | core/workflow 零 infra 依赖（port 由壳层装配 store 实现）；sync 签名；事务边界由 port 语义承载 |
| 归档路径（写面 `archive` + 命令薄包装） | 双写提交语义 + worktree 前置引导 | db status 翻转 + 主仓目录改名；name 主键不变；无建档拒绝；带 worktree 记录且主仓两树未命中 → 引导 merge 的显式拒绝；归档不触碰 worktree / branch |
| 时间口径 | db / 出线两段 | db i64 unix millis；出线 ISO 串 + null，转换收 queries 层单点 |
| StepRecord 审计查询（新） | 步骤历史面 | 按 change / run 枚举；审计 only 不做恢复；摘要有界 + 引用出全量 |
