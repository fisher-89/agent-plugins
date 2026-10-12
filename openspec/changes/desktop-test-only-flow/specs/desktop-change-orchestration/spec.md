# desktop-change-orchestration — desktop-test-only-flow delta

## MODIFIED Requirements

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎恒 CLI**：change 执行会话恒走 claude code CLI 引擎（插件 dev-team 天然加载，protect-files 护栏过渡期依赖）；SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）。
2. **工作流类型**：requirement 与 test-only 工作流全链；bug-fix 相位表后续独立 change。
3. **计划图静态编译期、非通用图引擎**：计划图（workflow → phase → step 序列 + 依赖边 + artifact 清单）SHALL 为编译期 const 表 + `include_str!` 静态装配，MUST NOT 实现运行时加载的通用图引擎或图定义文件格式；执行图（节点 + 边）为 db 落库的运行记录，MUST NOT 上位为路由权威（路由仍问 `phase_next`）。原「walker 只硬编码 walk 本相位循环图」表述随本变更翻案显式立项（step 序列下沉为 `PhaseDefinition.steps`）。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。
5. **protect-files 过渡依赖**：步前实时护栏过渡期依赖插件 hook；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向），本期仅留痕。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含计划图静态编译期翻案留痕），后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 恒解析为 CLI 引擎（默认 agent 解析），无 SDK 分支；change run 发起不依赖 SDK 连接配置

## ADDED Requirements

### Requirement: 发起前置校验类型分派（严格拒绝未知类型）

`change_flow_start` 前置校验 SHALL 保留「`phase_table(record.workflow_type)` 返回 `None` → 显式拒绝」的门：`"requirement"` / `"test-only"` 出线放行（test-only 进表后自然放行），其余（含空白 / bug-fix）SHALL 显式 `Err`。错误文案 SHALL 通吃 `{requirement, test-only}`，MUST NOT 再写死「V1 仅 requirement」、MUST NOT 引入「未知类型静默跑 requirement」的 fallback。walker 全程 SHALL 不新增 workflow_type 分支——相位序 / 步序列仍由写面 `phase_next` 依 `record.workflow_type` 下发的相位表承载。

#### Scenario: test-only 发起放行

- **WHEN** 对已建档 `workflow_type="test-only"` 的 change 调用 `change_flow_start` 前置校验
- **THEN** 校验通过（test-only 相位表在位），run 按 `TEST_ONLY_TABLE` 相位序走（proposal → code-analyze → … → archive）

#### Scenario: 未知类型显式拒绝

- **WHEN** 对 `workflow_type="bug-fix"`（或空白）的 change 调用 `change_flow_start`
- **THEN** 显式 `Err`（文案含 `{requirement, test-only}` 支持面），不进入运行态、不落任何账

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/commands/change_flow/mod.rs` | 发起前置校验 | `phase_table(record.workflow_type)` `None` → 显式 `Err`；文案通吃 `{requirement, test-only}`；test-only 出线放行；walker 零 workflow_type 分支 |
| `crates/core/orchestration`（walker） | 相位循环 | 零改动：相位序 / 步序列自写面 `phase_next` 依 record.workflow_type 下发，walker 不自持类型分派 |
