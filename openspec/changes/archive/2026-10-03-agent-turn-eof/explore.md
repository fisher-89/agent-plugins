# explore: agent-turn-eof

## 现象

desktop 应用（0.4.x）运行 change-flow：`move-queries-command-to-change` 的 proposal 相位
executor（proposal-planner）会话成功跑完（49 轮、7.6 分钟、turnDone success 落库），
walker 未进入 evaluator，run 永久卡死，停止按钮亦无效，只能杀进程。

## 证据链（2026-10-02 实测排查）

1. `openspec/changes/move-queries-command-to-change/workflow.json`：
   `active_phase` = proposal/attempt 1（00:26:38Z 落账），无 `eval` 字段 → evaluator
   从未运行、phase_log 从未代写。
2. redb（`~/.dev-team/workspaces/wps-claude-plugin-*.redb`）：executor 会话
   `ses-0-1790900799465`（provenance
   `move-queries-command-to-change/proposal/executor/1`）共 13339 条密封事件，
   最后一条 `{"seq":13338,"kind":"turnDone","subtype":"success","isError":false,"numTurns":49,...}`
   （00:34:19.82Z），与库文件 mtime 完全一致——此后零写入。
3. 库内更早会话（含 SDK 引擎 `sdk-0-*`）最后一条事件同样全是 turnDone →
   **0.3.14 session-kernel 重构起所有引擎轮全部如此**，change-flow 只是第一次显形。
4. 无残留 claude CLI 子进程；desktop 应用进程已关（run 任务随进程消亡）。

## 根因：内核收口等 EOF ↔ 泵等 questions 关闭，互持钥匙死锁

- `crates/core/agent/src/kernel.rs:221`：`drive` 的
  `while let Some(kind) = session.observations.recv().await` 只有观察通道**关闭**
  （所有发送端 drop）才退出、才 `converge`（finish_turn + TurnFinished）。
  而提问发送端 `session.questions` 被 `drive` 自己持有到函数结束。
- `crates/infra/agent/src/cli/runner.rs:71-92`：`session_pump` 在 CLI 进程退出、
  turnDone 已发之后回到 `while let Some(question) = questions.recv().await` 挂起，
  **手握 observations 发送端不释放**。
- `crates/infra/agent/src/sdk/runner.rs:150-177`：SDK 泵同构。
- 死锁闭环：drive 等 observations 关闭 ↔ 泵等 questions 关闭。连停止信号都无效：
  `drive` 无 stop select；CLI 泵的 stop select 只在轮内 `pump_lines`（runner.rs:255）。
- 测试为何全绿：内核假 runner 有 `close_stream()`（kernel_test.rs:187-190，注释即
  「消费端收尾时序的引擎侧单边决定」），显式 drop 发送端模拟 EOF；真实泵从未履行该契约。

## 修复决策（已拍板）

**方案 A：泵一轮一命（one turn, one life）**——CLI/SDK `session_pump` 处理完一个提问
后 `return`（drop observations 发送端 → 内核 EOF 收口）。理由：没有任何消费方向同一
会话通道发第二个提问（chat Continue 与 walker 每步都走
`compose_turn → begin → open_session` 新开），多问循环本就是死代码。

**配套：无头禁 ask**——CLI 参数追加 `--disallowedTools AskUserQuestion`。agent 无法决策
时不许用工具中途提问，而是把问题作为本轮最终文本 return（turnDone 正常收敛）；
用户回答后**同会话下一轮**（既有 Continue 机制：`SessionRef::Continue` → store 取
`engine_session_id` → `--resume`；walker 决策环 `decision_session` 的
`continue_session` 与 chat 面 Continue 均已走此路径，无需新机制）。

核实要点：
- CLI `--disallowedTools` 为 variadic `<tools...>`，组装时须置于参数序列**末尾**
  （`--resume` 之后），避免吞掉后续参数。
- SDK 工具面封闭清单（read/grep/glob/ls/write/edit，未知一律拒，policy.rs），
  无 ask 类工具，SDK 侧无需改动。
- 工具名线格式为 camelCase（runStarted tools 清单佐证）：`AskUserQuestion`。

## 涉及面

- `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs`（session_pump 一轮一命）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（session_pump 同构收口）
- `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs`（--disallowedTools）
- 相应测试：cli runner / sdk runner / flags
- 附带注释修正：port.rs `WorkerTurnRequest` 注释已写明「walker 恒 New」；kernel/
  runner 的「逐轮等待提问 / 回到等待下一轮」ask 语义注释需改为一轮一命口径。

## 非目标 / 约束

- 不改内核 `drive` 的 EOF 收口契约（不引入状态机 break——B 方案不做，保持缝两侧
  契约单一：引擎侧单边 EOF）。
- 不动 golden wire contract（tests/golden 冻结）。
- 用户可见缺陷修复 → 归档时 desktop 版本 bump。
- SDK 泵的跨轮 history_slot 随一轮一命失效，属死代码清理，非语义变更。
