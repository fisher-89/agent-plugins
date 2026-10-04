# desktop-workflow-write-face Delta

## MODIFIED Requirements

### Requirement: workflow.json 写面进程内单权威

core/workflow SHALL 补全 workflow.json 写面，成为 workflow.json 的 Rust 单一权威（读 + 写）。写面 SHALL 提供：

- `phase_next`：相位路由状态机——返回 next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单、重试上限判定；`sessionAnchors` 以进程内状态复活（mid-phase interruption 分支可达，重入时中断相位可感知并标定）；
- `phase_start`：开相位并启动 attempt 计时；
- `phase_log`：eval 记录落账（checklist 信封 + skipped + 会话槽位，见「eval 条目会话槽位落账」）；
- `backtrack`：回跳落账（stale 标记 + 传播 + reason ≤500）。

写面 SHALL 将变更持久化回 workflow.json。schema 形状约束自「零新字段」显式演进为本变更裁定的唯一例外：eval 条目 SHALL 允许携带 executor / evaluator / decision 三槽位的可选会话 id 字段（形态由 design 定稿）；除该例外外的字段集合 MUST NOT 改变，写面 MUST NOT 引入需要迁移历史 workflow.json 的格式变更。槽位字段在磁盘模型 SHALL 为可选（`#[serde(default)]` 读兼容无槽位的旧文件），serde 写出 MUST 保持可被插件解析面与既有读面解析（`backtrack_to` / `backtrack_reason` 演进先例同型）。写面 SHALL 保持 sync（MUST NOT 引入 tokio / async runtime，守住纯读叶库不陪跑运行时重编译的边界）。V1 仅移植 requirement 工作流相位表；bug-fix / test-only 相位表后续独立变更。

#### Scenario: 写面全操作进程内可达

- **WHEN** 命令层不经 walker、不起任何子进程，直接调用写面的 `phase_next` / `phase_start` / `phase_log` / `backtrack`
- **THEN** 各操作在进程内完成状态变更并持久化 workflow.json，无 CLI 子进程、无网络调用

#### Scenario: 写后 workflow.json 保持既有形状

- **WHEN** 写面完成一次 `phase_start` + `phase_log` + `backtrack` 序列并持久化
- **THEN** workflow.json 的字段集合与既有 schema 形状一致（唯一例外：eval 条目的会话槽位可选字段，显式在位才写），既有读面（`ChangeDetail` / list 查询）与插件解析面均可读取

#### Scenario: 旧文件读兼容

- **WHEN** 读取不含会话槽位字段的既有 workflow.json（v2 / v1 历史形状）并对其发起写操作
- **THEN** 解析与写操作均不报错，槽位字段以缺省（无键或 null）呈现，既有代际兼容行为不收窄

#### Scenario: sync 边界保持

- **WHEN** 审查 core/workflow 写面的依赖引入
- **THEN** 无 tokio / async runtime 依赖，写面函数为同步签名；既有读路径消费方不因写面引入运行时重编译

## ADDED Requirements

### Requirement: eval 条目会话槽位落账

`phase_log` 写操作 SHALL 支持随行落会话槽位：落账输入（`PhaseLogInput`）SHALL 增加可选的 executor / evaluator / decision 会话 id 槽位；落账时 SHALL 仅将显式在位的槽位写入该 eval 条目（缺省槽位不产生键，沿 skipped / start_at 扩展字段先例），verdict 条目随行携带 executor / evaluator 槽位、fail 升格条目同构处理。槽位值由调用方（walker）从 `WorkerTurnOutcome.session_id` 取值传入，写面 MUST NOT 自行解释或改写槽位值。decision 槽位的写入时机（decision 会话产生于 fail 条目落账之后：backtrack 写挂 / 显式 amend 写面操作 / 随下一 attempt 条目）由 design 定稿，本需求约束的是槽位落账的载体与显式在位纪律，不预设时机方案。

#### Scenario: 槽位随行落账

- **WHEN** walker 以携带 executor / evaluator session id 的 `PhaseLogInput` 调用写面 `phase_log`
- **THEN** 落账的 eval 条目携带对应槽位值，attempt 推导与 verdict 推导语义不受影响

#### Scenario: 缺省槽位不产生键

- **WHEN** 以不含会话槽位的 `PhaseLogInput` 调用写面 `phase_log`（或调用方未传）
- **THEN** 落账条目不出现槽位键，条目形状与既有形态一致

#### Scenario: decision 槽位在案可查

- **WHEN** decision 会话按 design 定稿的时机落挂后查询该 phase 的 eval 条目
- **THEN** 对应条目的 decision 槽位可查得该会话 id，且与其 executor / evaluator 槽位同驻条目（同 attempt）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/model/workflow.rs` `PhaseLog` | 磁盘模型演进 | 增三槽位可选字段（`#[serde(default)]`，形态 design 定稿）；旧文件读兼容；宽松解析口径不变 |
| `crates/core/workflow/src/write/phase_log.rs` | 槽位落账 | `PhaseLogInput` 增可选槽位；显式在位才写（扩展字段先例）；attempt / verdict 推导不受影响 |
| `crates/core/orchestration/src/walker.rs` | 槽位取值传入 | `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值；decision 时机 design 定稿 |
| serde 持久化 | schema 兼容面 | 唯一例外 = eval 条目槽位字段；serde 写出可被插件解析面读取；历史文件零迁移 |
