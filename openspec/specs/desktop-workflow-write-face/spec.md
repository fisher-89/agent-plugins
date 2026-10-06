# desktop-workflow-write-face Specification

## Purpose

core/workflow 补全 workflow.json 写面（phase_next / phase_start / phase_log / backtrack），成为 workflow.json 的 Rust 单一权威（读 + 写）；相位表与 prompt 模板单源驻 core/workflow，walker 经进程内直调消费，serde 写出保持插件 zod schema 兼容。

## Requirements

### Requirement: workflow.json 写面进程内单权威

core/workflow SHALL 补全 workflow.json 写面，成为 workflow.json 的 Rust 单一权威（读 + 写）。写面 SHALL 提供：

- `phase_next`：相位路由状态机——返回 next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单、重试上限判定；`sessionAnchors` 以进程内状态复活（mid-phase interruption 分支可达，重入时中断相位可感知并标定）；
- `phase_start`：开启阶段并 attempt 计时；
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

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态）；校验通过 SHALL 落回跳记录、对被跳过相位打 stale 标记并传播，`reason` 超过 500 字符 SHALL 被拒绝。

#### Scenario: 白名单内回跳落账

- **WHEN** 以 `backtrack_to="test-gen"`（白名单内）、合法 reason 调用写面 backtrack
- **THEN** 回跳记录落账、目标相位可被 `phase_next` 路由至、被跳过相位带 stale 标记

#### Scenario: 越权目标被拒绝

- **WHEN** 以 `backtrack_to` 不在 `allowed_backtrack_phases` 内调用写面 backtrack
- **THEN** 调用显式失败（错误抵达调用方），workflow.json 无任何变更

#### Scenario: reason 超长被拒绝

- **WHEN** 以超过 500 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，workflow.json 无任何变更

### Requirement: 相位表与 prompt 模板单源收敛

requirement 工作流相位表（相位序、executor / evaluator prompt 模板）SHALL 单源驻 core/workflow：walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份 MUST NOT 上位为路由权威；TS 侧相位表（`lib/workflow.ts`）冻结，不再是桌面路径依赖。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准

#### Scenario: 相位表单一来源

- **WHEN** 同一 requirement 工作流的相位序在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序与 prompt 模板；walker 侧无第二份定义

### Requirement: phase_log 回跳后重评条目修复

写面 `phase_log` SHALL 修复插件侧已知缺陷（backtrack 后同相位重评时 eval 条目被跳过丢失）：backtrack 回跳后的同相位每一次重评 SHALL 全部落账，不因该相位已有 pass 记录而跳过；插件侧 TS 实现冻结不修。

#### Scenario: 回跳后重评条目齐全

- **WHEN** test-gen backtrack 回跳后，同相位完成第二次评估并以写面 `phase_log` 落账
- **THEN** 第二次 eval 条目完整落账（含 checklist 与时间戳），与首次条目并存，eval 序列无丢失

### Requirement: 等价性对照验收口径

写面以**对照功能验收**为准：逐项对照插件 CLI 直跑形态下 `phase_next` / `phase_start` / `phase_log` / `backtrack` 的行为语义验收（路由结果、重试上限、白名单下发、attempt 计时、eval 落账、stale 传播）；MUST NOT 建差分 oracle（双跑自动比对）。长期单实现，接受微小行为漂移；workflow.json schema 兼容（serde 写出可被插件 zod schema 读取）MUST 保持。

#### Scenario: 对照验收清单逐项通过

- **WHEN** 以同一 change 状态分别经插件 CLI 直跑与本写面推进等价相位序列
- **THEN** 对照清单逐项核验一致（active_phase 演进、eval 条目形态、白名单内容、stale 标记），差异仅限既知可接受面（时间戳值）

#### Scenario: serde 写出兼容插件读取

- **WHEN** 写面持久化 workflow.json 后以插件 zod schema 解析同一文件
- **THEN** 解析通过且语义等价（无字段丢失、无类型漂移）

### Requirement: schema 权威移交与双写并存声明

过渡期 skill 路径（插件 MCP）与 Rust 写面并存写同一 workflow.json 为既定事实；本变更 SHALL 在 design 中显式声明 schema 校验权威的移交时点（zod → serde）。serde 写出 MUST 兼容既有 workflow.json schema；MUST NOT 因写面引入需要迁移历史 workflow.json 的格式变更。

#### Scenario: 双写并存不互斥

- **WHEN** 同一 change 先经 skill 路径（MCP）推进一个相位，再经 desktop 写面推进下一相位
- **THEN** 两次写入均成功，最终 workflow.json 被双方解析面均正确读取

#### Scenario: 历史文件零迁移

- **WHEN** 写面对既有 change（含 v1/v2 历史 workflow.json 形状）发起写操作
- **THEN** 既有解析面的代际兼容行为不变（读面既有宽松解析契约不因写面收窄）

### Requirement: 写面独立可达

写面 SHALL 为命令层可达的独立能力：不经 walker run 即可直调（单次 `backtrack` / `phase_log` / `phase_start` 调用不要求拉起编排循环），为后续 UI「手动 backtrack / 重试」留口。

#### Scenario: 免编排单点调用

- **WHEN** 命令层直接调用写面完成一次 backtrack（无 walker run 在场）
- **THEN** 回跳落账成功，无需创建编排会话或 run 状态

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/` 写面模块（新增，划分 design 定稿） | workflow.json 域权威写面 | `phase_next`（路由 + prompt 插值 + 白名单 + sessionAnchors 进程内锚点）/ `phase_start` / `phase_log`（checklist 信封 + skipped + 会话槽位）/ `backtrack`（stale + 传播 + 白名单二次校验 + reason ≤500）；serde 持久化；sync 无 tokio；schema 唯一例外 = eval 条目会话槽位字段 |
| core/workflow 相位表（含 prompt 模板） | requirement 工作流相位序单源 | executor / evaluator prompt 模板驻此；TS 侧 `lib/workflow.ts` 冻结；`PIPELINE_PHASES` 保持布局身份 |
| `crates/core/orchestration`（消费方） | walker 经 port 缝进程内直调写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则 |
| 插件 MCP / TS 写路径（过渡并存） | skill 路径既有写通道 | 冻结不改；serde 写出 MUST 经 zod 兼容 fixture 对照；schema 权威移交时点 design 显式声明 |
| `crates/core/workflow/src/model/workflow.rs` `PhaseLog` | 磁盘模型演进 | 增三槽位可选字段（`#[serde(default)]`，形态 design 定稿）；旧文件读兼容；宽松解析口径不变 |
| `crates/core/workflow/src/write/phase_log.rs` | 槽位落账 | `PhaseLogInput` 增可选槽位；显式在位才写（扩展字段先例）；attempt / verdict 推导不受影响 |
| `crates/core/orchestration/src/walker.rs` | 槽位取值传入 | `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值；decision 时机 design 定稿 |
| serde 持久化 | schema 兼容面 | 唯一例外 = eval 条目槽位字段；serde 写出可被插件解析面读取；历史文件零迁移 |
