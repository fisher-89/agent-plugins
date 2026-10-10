# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: 三类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为三类节点：

- **WorkerAgent 节点**（executor / evaluator / decision）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunStepRecord`，desktop-change-state-store「run 运行史落库」）∪ 进程内注册表在飞 run（`RunEntry` 步累积器）** × 按 provenance 反查的 session 集；run 状态落库为既定基线——原「MUST NOT 建立 run 私有持久表（不建 flow_runs 表）」红线随 unify-run-state-persistence 决策翻案显式立项（`control.rs` 头注释决策原文同步改写；StepRecord 仍为步骤审计行、MUST NOT 承担节点状态源）。步词汇过滤 SHALL 单点定义（落库子集 = 回显子集，五留五忽略封闭词汇，desktop-change-state-store），walker 与 control MUST NOT 持有过滤逻辑。已收口 run 的任一节点状态 SHALL 可由库读史单独重算得出（重启 / 重挂后不依赖进程内存或 run 私有状态文件）。

#### Scenario: ToolStep 失败同等可见

- **WHEN** static-check 或 test-execution spawn 步以非零退出 / 显式 Err 收场（或写面相位机步返回错误）
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点状态两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_steps 两表在案（desktop-change-state-store）、在飞 run 状态驻进程内注册表（RunEntry）；已收口 run 的任一节点状态均可由库读史重算得出，不依赖进程内存

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change 名发起 run，`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）随发起定格该 run 的停等节奏并透传 walker；发起 SHALL 先解析 exec root（db 记录 worktree 字段 → worktree 路径，None → 主 root，见「run 双 root 组合与 exec root 解析」）；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立（desktop-change-state-store「run 运行史落库」；写落位——命令层装配 vs walker 起点直调——由 design 定稿）；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

**读路统一（推拉反转）**：`get_change_detail`（或其扩展）SHALL 一次返回「库读史 ∪ 内存在飞 run」统一视图——库面 runs / steps 读史（desktop-change-queries）并入在飞 run 的状态 / 停等与 ask 载荷 / RunEntry 步表，合并发生在读时（零每步写放大前提不变）；客户端 MUST NOT 双命令拼接（useChangeDetail + useChangeFlowRun 双真相源拼缝退场）。`RunUpdate` SHALL 降位为变更通知（无步 / 会话事件载荷或最小载荷，信封形状 design 定稿）：客户端收通知后自行重查统一视图，通知仅失效信号、查询结果为权威；高频通知（每步 / 每会话事件）SHALL 经客户端合并去抖或通知侧 coalesce 抑制重查风暴（策略 design 定稿）。stop / confirm / answer 控制面 SHALL 维持请求-应答形态不变（非推送）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 已建档（db `ChangeRecord` 在案且 `workflow_type=requirement` 相位表在位）、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change)` 复合（修正既有仅按 change 名做键的两 workspace 同名假冲突先例 bug；worktree 隔离解锁同 workspace 多 change 真并行）——无建档的 change（存量 CLI change）SHALL 显式拒绝发起（文档形态 change 不可运行）。

#### Scenario: 发起提前 resolve 与通知流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve（run 起始行已落），后续节点 / 相位状态变化经 Channel 以变更通知流出，客户端收通知重查统一视图；终态以通知收尾、客户端重取定局

#### Scenario: 通知触发重查统一视图

- **WHEN** run 运行期间某步状态变化通知到达
- **THEN** 客户端经单命令统一视图重查（库读史 ∪ 在飞 run 含步表），无第二命令拼接、无前端载荷累积

#### Scenario: 控制面请求-应答不变

- **WHEN** 用户对 waitingAsk 态应答、对 phase 间停等确认、对运行中 run 停止
- **THEN** 三控制命令沿既有请求-应答契约执行（提前 resolve / 幂等 / 回流驱动语义不变），不经通知通道

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change（存量 CLI change）发起 run、或同 workspace 同 change 已有运行中的 run 再次发起、或另一 workspace 存在同名 change 的运行中 run
- **THEN** 前两者发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；第三者不受影响正常发起（复合键下异 workspace 同名不构成冲突）

#### Scenario: auto_next_phase 随发起透传

- **WHEN** 前端以 auto_next_phase=true 发起某 change 的 run，随后另一 change 以 auto_next_phase=false 发起
- **THEN** 两 run 各自按发起参数定格节奏运行：前者全程无 phase 间停等，后者照常停等确认；参数经命令面透传至 `RunRequest.auto_next_phase`

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.27` 升级为 `0.4.28`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.28；`plugins/dev-team` 版本与交付产物零改动

## ADDED Requirements

### Requirement: run 落库写缝与累积器驻服务端

run 落库写缝 SHALL 收在 walker 侧单点：`walk_run` 终态出口（`guard.finish` 处）一次完成终态更新 + 步整包 + active_phase 处置（同事务，desktop-change-state-store「run 运行史落库」「启动标定与 active_phase 悬挂处置」）；`ChangeFlowControl` SHALL 保持进程内零 IO 不变量（不持 store 句柄，store 写经 walker 侧 port / 命令层装配注入）。步累积器 SHALL 移驻服务端：`RunEntry`（注册表条目）自持步列表（emit 序追加、通知触发重读），walker 经 `RunGuard::emit` 间达零变化；重挂快照 SHALL 含 steps（重挂恢复步表不再恒空）。启动装配 SHALL 执行启动标定（残留 running → interrupted，操作面见 desktop-change-state-store）。停止 / 会话失败 / verdict 解析失败等一切 run 死亡路径 SHALL 汇聚同一 finish 落包出口（终态出口唯一既有不变量）。

#### Scenario: 写缝单点与 control 零 IO

- **WHEN** 审查 walker 终态路径与 control 源码
- **THEN** 落包仅出现在 walk_run 终态出口一处；control 无 store 句柄、无 IO 调用（进程内纯状态不变量保持）

#### Scenario: 全死亡路径同一出口

- **WHEN** run 分别以 completed / stopped（用户停止）/ failed（会话失败、verdict 解析失败）收口
- **THEN** 三路径均经同一 finish 落包出口：终态 + reason + 步整包 + active_phase 处置单事务落库，库读史与内存面终态一致

#### Scenario: 重挂恢复步表完整

- **WHEN** run 运行中（已 emit 多步）视图重挂后读取快照
- **THEN** 快照 steps 与已 emit 步逐条一致（emit 序），后续通知继续触发重读；不依赖前端累积

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` | finish 落包缝（终态出口单点） | 终态更新 + 步整包 + active_phase 处置同事务；全死亡路径汇聚；写经 port 缝（core 零 infra 依赖） |
| `crates/core/orchestration/src/control.rs` | RunEntry 步累积器 + 头注释改写 | 注册表自持步列表（emit 序、通知触发重读）；快照含 steps；进程内零 IO 不变量保持；「不建 flow_runs 表」决策原文随翻案改写 |
| `crates/core/orchestration/src/state.rs` | RunUpdate 通知降位 | 信封瘦身（形状 design 定稿）；ChangeRunSnapshot 增 steps；bindings 再生成跟随 |
| `src/commands/changes/mod.rs` | 合并查询装配 | get_change_detail（或扩展）一次出「库读史 ∪ 在飞 run」统一视图；读时合并不入写路径 |
| `src/commands/change_flow/mod.rs` | 通知桥与标定装配 | run 起始行落库装配（落位 design 定稿）；启动标定 sweep 调用；`change_flow_state` 快照命令处置由 design 定稿 |
