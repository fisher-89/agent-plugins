# agent-turn-eof — desktop-agent-execution 变更集

> 修复 0.3.14 session-kernel 重构引入的引擎轮不收口缺陷：内核 `drive` 以「观测通道关闭（引擎侧单边 EOF）」为唯一收口缝，而真实引擎泵（CLI / SDK `session_pump`）轮终态后回到提问接收挂起、手握 observations 发送端不释放——互持钥匙死锁，所有引擎轮 turnDone 落库后 converge 永不发生（change-flow walker 卡死与 chat 面终端消息缺失的共同根因）。本变更确立**泵一轮一命**（处理恰一个提问后返回 → 发送端释放 → 内核 EOF 收口），配套 **headless 禁 ask**（`--disallowedTools AskUserQuestion`，agent 以问题文本收口、应答经既有 Continue 续轮）。内核 EOF 收口契约零改动；tests/golden 与前端零 diff。

## ADDED Requirements

### Requirement: 引擎泵一轮一命与 headless 无问收口

`agent-runtime` 的 `cli/` 与 `sdk/` 引擎会话泵任务（`session_pump`）SHALL 服务**恰一个提问**：轮终态产出后泵任务 SHALL 返回（观察通道发送端随之释放 → 内核观测通道关闭）。泵 MUST NOT 回到提问接收等待下一轮（多问循环 MUST NOT 残留）；spawn 失败与停止路径的既有单轮终止语义 SHALL 保持不变。会话史 SHALL 不依赖泵内跨轮回灌（SDK 泵的跨轮 `history_slot` 随一轮一命移除；会话史唯一来源仍是 store 全史转录重建）。

`core/agent` 的 `drive` SHALL 保持「观测通道关闭（引擎侧单边 EOF）→ converge」为**唯一收口缝**，MUST NOT 引入状态机 break、stop select 或第二收口路径；内核测试假 runner 的引擎侧单边 EOF（`close_stream`）即该契约的测试面，真实泵 SHALL 履行同一契约。

消费纪律：任何消费方对同一逻辑会话的每一轮 SHALL 经 `compose_turn → begin → open_session` 新开引擎会话（既有消费现实：chat Continue 与 walker 每步均如此）；泵返回后同一引擎会话通道上的提问送达 SHALL 走内核 ask 失败臂（failed 记因「轮提问送达失败（引擎侧已终止）」），MUST NOT 复活常驻泵。

headless 禁 ask：CLI 引擎命令行组装 SHALL 追加 `--disallowedTools AskUserQuestion`，且 MUST 置于参数序列**末尾**（`--resume` 及其值之后——variadic flag 置中间会吞并后续参数）。无头 agent 需用户决策时 SHALL 以问题文本作为本轮最终消息收口（turnDone 正常收敛），MUST NOT 经 AskUserQuestion 工具中途提问；用户应答 SHALL 经既有 Continue 机制（`SessionRef::Continue` → store 取 `engine_session_id` → `--resume`）同会话续轮。SDK 引擎工具面为封闭清单（无 ask 类工具），MUST NOT 组装该 flag。

注释口径：kernel / runner 的「逐轮等待提问 / 回到等待下一轮 / 逐轮驱动（ask 语义）」注释 SHALL 更新为一轮一命口径。

#### Scenario: 泵单轮收口驱动内核 EOF converge

- **WHEN** 引擎泵产出轮终态事件（CLI 进程退出 / SDK loop 收敛）
- **THEN** 泵任务返回且观察通道发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 的观测循环以通道关闭退出并 converge（finish_turn + `TurnFinished` 流出、轮行落库终态、注册表除名）
- **AND** 泵任务不再回到提问接收等待下一轮

#### Scenario: 泵终止后提问按失败收敛

- **WHEN** 同一引擎会话通道在泵返回后再次收到提问送达
- **THEN** 内核 ask 失败臂收敛 failed（记因「轮提问送达失败（引擎侧已终止）」），系统中不存在挂起的常驻泵
- **AND** 消费方每轮经 `compose_turn → begin → open_session` 新开引擎会话（chat Continue 与 walker 每步既有路径零改动）

#### Scenario: disallowedTools flag 末尾组装

- **WHEN** 以任意 permission 档位（含带 `--resume` 的续会话）组装 CLI 命令行
- **THEN** 参数序列末尾两枚为 `--disallowedTools` 与 `AskUserQuestion`，位于 `--resume` 及其值之后，其余 flag（`-p` / stream-json / `--verbose` / permission 档位）不变
- **AND** SDK 引擎的组装面无 disallowedTools 概念（工具面封闭清单本无 ask 工具）

#### Scenario: 无问文本收口与同会话续轮

- **WHEN** 无头 CLI agent 需要用户决策
- **THEN** 该轮不发生 AskUserQuestion 工具调用，agent 以问题文本作为本轮最终消息收口（turnDone 正常 converge，终端消息到达消费方）
- **AND** 用户应答后经 Continue（store 取 `engine_session_id` → `--resume`）同会话续轮，前轮全史在场（walker 决策环与 chat 面既有路径）

#### Scenario: 停止与 spawn 失败语义保持

- **WHEN** 泵轮中途收到停止信号，或 spawn 失败（CLI 缺失）
- **THEN** 既有语义不变：停止路径不合成收敛、击杀缝被调、轮收敛 `stopped`；spawn 失败以合成收敛事件 failed 记因且泵终止
- **AND** 两路径均为单轮终止，与一轮一命语义同形，无回归

## MODIFIED Requirements

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——进程树击杀失败时 claude 子进程可能残留（Windows 尤甚），应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经 resume 落地**：`--resume <session_id>` 随 explore 会话链实现（见「Claude CLI 租户（MVP）」）；`--continue` 隐式续会话 MUST NOT 实现；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制（agent-turn-eof 修订）**：headless CLI 运行 SHALL 显式禁用 AskUserQuestion（`--disallowedTools AskUserQuestion`，见「引擎泵一轮一命与 headless 无问收口」）——agent 需用户决策时以问题文本作为本轮最终消息收口，应答经 Continue（`--resume`）同会话续轮；explore / chat 页对转录中既存 AskUserQuestion ToolUse 的静态可读卡渲染（问题/选项原样呈现）与 composer 文本输入续话呈现不变；questionnaire 组件接线为二期
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树击杀失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` | CLI 引擎泵 | `session_pump` 一轮一命（单问服务后返回 → observations 发送端释放 → 内核 EOF 收口）；停止 / spawn 失败单轮终止语义不动；`--resume` 尾追加规则不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | SDK 引擎泵 | `session_pump` 同构一轮一命；跨轮 `history_slot` 死代码移除（会话史唯一源仍为 store 全史转录重建，`resume.rs` 不动）；biased select 停止臂语义不动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` | CLI 命令行组装 | `--disallowedTools AskUserQuestion` 恒追加于参数序列末尾（`--resume` 之后，variadic 不吞并后续参数）；其余 flag 面零变化 |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 内核收口缝（仅注释口径） | `drive` EOF 收口逻辑零改动（引擎侧单边 EOF 唯一缝，无状态机 break）；「ask 语义」注释改一轮一命口径 |
