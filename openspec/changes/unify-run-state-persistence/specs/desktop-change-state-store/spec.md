# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

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

## ADDED Requirements

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
| `crates/infra/store/src/model.rs`（RunRecord / RunStepRecord，新） | run 运行史形状 | run_id 主键 / change 二级索引 / status 五值词汇（interrupted 仅标定产生）；RunStepRecord 步词汇封闭集五值 + seq（emit 序）+ session_id / detail 有界；native_model 版本治理；workspace 维度 |
| `crates/infra/store/src/store.rs`（run 域操作面，新） | run_start（running 行）/ run_finish（终态 + 步整包单事务）/ 启动标定 sweep / run 史查询 | 每 run 两写、零每步写放大；步词汇过滤谓词单点（挂点 design 定稿）；整包原子回滚；标定幂等 |
| 步词汇单点 | 过滤词汇唯一定义 | 五留五忽略封闭集；落库写面与读面同一消费；walker / control 零过滤逻辑 |
| 与 StepRecord 职责分立 | 审计 vs 图史 | StepRecord 职责与词汇不变；两表不互为替代、不双写同一语义行 |
