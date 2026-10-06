# desktop-workflow-write-face Specification

## Purpose

core/workflow 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体为 workspace 库（desktop-change-state-store，落库经 port 缝）；相位表与 prompt 模板单源驻 core/workflow，walker 经进程内直调消费，desktop 全链不读不写 workflow.json（双向墙）。

## Requirements

### Requirement: workflow.json 写面进程内单权威

core/workflow SHALL 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体 SHALL 由 workflow.json 重定义为 workspace 库（desktop-change-state-store；落库经 port 缝由壳层装配 store 实现，core/workflow 零 infra 依赖）。写面 SHALL 提供：

- `phase_next`：相位路由状态机——返回 next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单、重试上限判定；`sessionAnchors` 以进程内状态复活（mid-phase interruption 分支可达，重入时中断相位可感知并标定）；
- `phase_start`：开启阶段并 attempt 计时；
- `phase_log`：eval 记录落账（checklist 信封 + skipped + 会话槽位，见「eval 条目会话槽位落账」），单事务原子落库；
- `backtrack`：回跳落账（stale 标记 + 传播 + reason ≤500）；
- `archive`：归档双写（db status 翻转 + 目录改名，见 desktop-change-state-store「归档双写」）。

状态读取 SHALL 单源自 workspace 库（ChangeRecord / PhaseRecord / ChecklistItemRecord）；desktop MUST NOT 读或写任何 workflow.json。字段演进 SHALL 经 native_model 版本机制治理，MUST NOT 再依赖 JSON 文件形状保形（`persist.rs` raw Value 保形改写随载体退役）。写面 SHALL 保持 sync（store 为同步操作面，写面 MUST NOT 引入 tokio / async runtime）。V1 仅移植 requirement 工作流相位表；bug-fix / test-only 相位表后续独立变更。

#### Scenario: 写面全操作进程内可达

- **WHEN** 命令层不经 walker、不起任何子进程，直接调用写面的 `phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`
- **THEN** 各操作在进程内完成状态变更并落 workspace 库，无 CLI 子进程、无网络调用、无 workflow.json 读写

#### Scenario: 写后状态库一致可读

- **WHEN** 写面完成一次 `phase_start` + `phase_log` + `backtrack` 序列后经查询层读取该 change
- **THEN** PhaseRecord / ChecklistItemRecord / active_phase / stale 标记与写面返回语义逐项一致，状态完全来自 db

#### Scenario: 落库经 port 缝零 infra 依赖

- **WHEN** 审查 `crates/core/workflow/Cargo.toml` 与写面落库调用点
- **THEN** 零 infra/store 直接依赖；落库经 port 缝（trait）由 desktop-app 装配的 store 实现承载

#### Scenario: sync 边界保持

- **WHEN** 审查 core/workflow 写面的依赖引入
- **THEN** 无 tokio / async runtime 依赖，写面函数为同步签名；既有读路径消费方不因写面引入运行时重编译

### Requirement: eval 条目会话槽位落账

`phase_log` 写操作 SHALL 支持随行落会话槽位：落账输入（`PhaseLogInput`）SHALL 增加可选的 executor / evaluator / decision 会话 id 槽位；落账时 SHALL 仅将显式在位的槽位写入该 PhaseRecord 的三槽位列（缺省槽位列以 None 呈现，沿磁盘模型可选字段先例的显式在位纪律），verdict 条目随行携带 executor / evaluator 槽位、fail 升格条目同构处理。槽位值由调用方（walker）从 `WorkerTurnOutcome.session_id` 取值传入，写面 MUST NOT 自行解释或改写槽位值。decision 槽位的写入时机（decision 会话产生于 fail 条目落账之后：backtrack 写挂 / 显式 amend 写面操作 / 随下一 attempt 条目）由 design 定稿，本需求约束的是槽位落库的载体与显式在位纪律，不预设时机方案。槽位列 SHALL 与 `SessionRecord` 同库（join 一等查询，见 desktop-change-state-store）。

#### Scenario: 槽位随行落库

- **WHEN** walker 以携带 executor / evaluator session id 的 `PhaseLogInput` 调用写面 `phase_log`
- **THEN** 落库的 PhaseRecord 三槽位列携带对应会话 id，attempt 推导与 verdict 推导语义不受影响

#### Scenario: 缺省槽位列 None

- **WHEN** 以不含会话槽位的 `PhaseLogInput` 调用写面 `phase_log`（或调用方未传）
- **THEN** 落库条目对应槽位列为 None，查询层以 null 透出，不虚构值

#### Scenario: decision 槽位在案可查

- **WHEN** decision 会话按 design 定稿的时机落挂后查询该 phase 的 PhaseRecord
- **THEN** 对应条目的 decision 槽位列可查得该会话 id，且与其 executor / evaluator 槽位同驻记录（同 attempt），库内可 join 会话转录

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 500 字符 SHALL 被拒绝。

#### Scenario: 白名单内回跳落账

- **WHEN** 以 `backtrack_to="test-gen"`（白名单内）、合法 reason 调用写面 backtrack
- **THEN** 回跳记录落库、目标相位可被 `phase_next` 路由至、被跳过相位的 PhaseRecord 带 stale 标记

#### Scenario: 越权目标被拒绝

- **WHEN** 以 `backtrack_to` 不在 `allowed_backtrack_phases` 内调用写面 backtrack
- **THEN** 调用显式失败（错误抵达调用方），状态库零变更（无新行、无 stale 翻转、active_phase 不动）

#### Scenario: reason 超长被拒绝

- **WHEN** 以超过 500 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，状态库零变更

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

写面以**对照功能验收**为准：逐项对照既往 desktop 直跑形态下 `phase_next` / `phase_start` / `phase_log` / `backtrack` 的行为语义验收（路由结果、重试上限、白名单下发、attempt 计时、eval 落账、stale 传播）；MUST NOT 建差分 oracle（双跑自动比对）。长期单实现，接受微小行为漂移。workflow.json schema 兼容命题随双向墙退役（desktop 与插件工作流不再共写共享文件）；状态语义演进以本 spec 与 desktop-change-state-store 为准。

#### Scenario: 对照验收清单逐项通过

- **WHEN** 以同一 change 状态分别经既往 desktop 形态与本写面推进等价相位序列
- **THEN** 对照清单逐项核验一致（active_phase 演进、eval 条目语义、白名单内容、stale 标记），差异仅限既知可接受面（时间戳值）

#### Scenario: 状态语义漂移显式捕获

- **WHEN** 写面重构后重放既有相位机行为测试集（路由 / 重试 / 白名单 / stale）
- **THEN** 测试全绿；语义变化必须以 spec delta 显式提出而非实现静默漂移

### Requirement: 写面独立可达

写面 SHALL 为命令层可达的独立能力：不经 walker run 即可直调（单次 `backtrack` / `phase_log` / `phase_start` 调用不要求拉起编排循环），为后续 UI「手动 backtrack / 重试」留口。

#### Scenario: 免编排单点调用

- **WHEN** 命令层直接调用写面完成一次 backtrack（无 walker run 在场）
- **THEN** 回跳落账成功，无需创建编排会话或 run 状态

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/` 写面模块 | change 流程状态域权威写面（db 载体） | `phase_next`（路由 + prompt 插值 + 白名单 + sessionAnchors 进程内锚点）/ `phase_start` / `phase_log`（checklist 信封 + skipped + 会话槽位，单事务原子）/ `backtrack`（stale + 传播 + 白名单二次校验 + reason ≤500）/ `archive`（双写）；落库经 port 缝；sync 无 tokio；`persist.rs` raw Value 保形改写退役 |
| 落库 port 缝（新） | core/workflow → store 的进程内缝 | trait 定义落位 design 定稿；desktop-app 装配 store 实现；core/workflow 零 infra 依赖 |
| core/workflow 相位表（含 prompt 模板，不改） | requirement 工作流相位序单源 | executor / evaluator prompt 模板驻此；TS 侧 `lib/workflow.ts` 冻结；`PIPELINE_PHASES` 保持布局身份 |
| `crates/core/orchestration`（消费方，不改面） | walker 经 port 缝进程内直调写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则；LocalToolSteps 直调面签名不变（载体在写面内换血） |
| 插件 MCP / TS 写路径（彻底分离，不改） | CLI 工作流自有写通道 | 双向墙：desktop 不读不写 workflow.json；插件零改动 |
| `crates/infra/store`（workspace 库实例） | 状态落库载体 | ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord（见 desktop-workspace-store / desktop-change-state-store） |
