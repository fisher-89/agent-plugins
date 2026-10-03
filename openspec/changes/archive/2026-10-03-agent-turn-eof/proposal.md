# 提案: agent-turn-eof

> **变更**: agent-turn-eof
> **日期**: 2026-10-02
> **状态**: 草案

---

## 问题

自 0.3.14 session-kernel 重构起，**所有引擎轮在终态事件（turnDone）之后永不收口**——轮的终态记录已落库，但内核收敛（finish_turn + `TurnFinished` 流出）永不发生：

1. **change-flow 显形**：walker 的 executor 会话成功跑完后（turnDone success 落库）walker 永不进入 evaluator，run 永久卡死、停止按钮无效，只能杀进程。实测证据（2026-10-02）：`move-queries-command-to-change` proposal 相位 executor 会话 `ses-0-1790900799465` 共 13339 条密封事件，末条为 `turnDone success`（00:34:19.82Z），此后该库零写入；`workflow.json` 无 `eval` 字段——evaluator 从未运行。
2. **debug / chat 面同缺陷**：终端 Record 消息（终态部件）永不发出，前端轮永不收敛，无法继续 Continue。
3. 库内全部历史会话（含 SDK 引擎 `sdk-0-*`）末条事件同样全是 turnDone → **全引擎、全轮如此**，change-flow 只是第一个把它显形出来的消费方。

**根因：互持钥匙死锁**。`core/agent` 的 `drive`（`crates/core/agent/src/kernel.rs:221`）的观察循环 `while let Some(kind) = session.observations.recv().await` 只在观察通道**关闭**（所有发送端 drop）时才退出并收敛，而提问发送端 `session.questions` 被 `drive` 自己持有到函数结束；另一侧，真实引擎泵（`cli/runner.rs:71-92` 与 `sdk/runner.rs:150-177` 的 `session_pump`）在轮终态产出后回到 `questions.recv().await` 挂起等待下一轮，**手握 observations 发送端不释放**。于是 drive 等 observations 关闭 ↔ 泵等 questions 关闭，循环等待。停止信号也不可达：`drive` 无 stop select，CLI 泵的 stop select 只活在轮内 `pump_lines`（轮中途），轮结束后无法触达。

**测试为何全绿**：内核测试假 runner 提供 `close_stream()`（`kernel_test.rs:187-190`，显式 drop 发送端模拟「引擎侧单边 EOF」）；真实泵从未履行该契约——缺陷在泵侧履约缺失，而非内核契约错误。

---

## 提案

修复方向已拍板（explore 阶段实测验证）：**方案 A「一轮一命」+ headless 禁 ask 配套**。

1. **泵一轮一命（one turn, one life）**：CLI / SDK 的 `session_pump` 处理完恰一个提问后 `return`（drop observations 发送端 → 内核观测通道关闭 → drive EOF 收口）。理由：没有任何消费方向同一引擎会话通道发第二个提问——chat Continue 与 walker 每步都走 `compose_turn → begin → open_session` 新开引擎会话，多问循环本就是死代码。**内核 `drive` 的 EOF 收口契约零改动**：引擎侧单边 EOF 仍是唯一收口缝，不引入状态机 break（B 方案不做），缝两侧契约保持单一。
2. **headless 禁 ask**：CLI 命令行组装追加 `--disallowedTools AskUserQuestion`，置于参数序列**末尾**（`--resume` 之后——variadic flag 置中间会吞并后续参数）。无头 agent 需用户决策时 SHALL 以问题文本作为本轮最终消息收口（turnDone 正常收敛）；用户应答后经**既有 Continue 机制**（`SessionRef::Continue` → store 取 `engine_session_id` → `--resume`）同会话续轮。walker 的 `decision_session` 与 chat 面均已走此路径，无新机制。
3. **SDK 引擎无需禁 ask**：工具面是封闭清单（read / grep / glob / ls / write / edit，未知工具名一律拒绝），无 ask 类工具；其泵内跨轮 `history_slot` 随一轮一命失效，作为死代码移除（清理，非语义变更——会话史唯一来源仍是 store 转录重建）。
4. **注释口径修正**：`cli/runner.rs` / `sdk/runner.rs` 的「逐轮等待提问 / 回到等待下一轮」与 `core/agent` 的「逐轮驱动（ask 语义）」注释改为一轮一命口径；`core/orchestration/src/port.rs` 的「walker 恒 New」注释已正确，不动。
5. **版本**：用户可见缺陷修复（run 卡死 / 会话不收敛）→ **归档时** desktop 版本 bump（0.4.1 → 0.4.2）。

---

## 能力

### 新增能力

- 无。

### 修改的能力

- **desktop-agent-execution** — 新增「引擎泵一轮一命与 headless 无问收口」requirement（泵生命周期单问服务、内核 EOF 收口契约保持、headless 禁 ask flag、消费纪律与注释口径）；「MVP 边界与已知限制」requirement 第 4 条（交互工具限制）随改：AskUserQuestion 从「行为面不变（拒绝或受限如实呈现）」改为「headless 显式禁用 + 问题文本收口 + Continue 续轮」。

### 沿用（语义不变，仅被引用）

- **desktop-agent-chat-infra** — chat 面不收敛缺陷由本修复在 agent 层消除，其契约（重放装载 / 停止触达 / 终态同构）零改动，前端零 diff。
- **desktop-change-orchestration** — walker 卡死是症状侧；walker 决策协议的 ask 走封闭集结构化输出（非 ask 工具），不受禁 ask 影响，零改动。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` — `session_pump` 一轮一命：处理完一个提问（轮终态产出 / spawn 失败 / 停止）后返回；doc 注释改一轮一命口径
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` — `session_pump` 同构收口；`history_slot` 跨轮回灌死代码移除；doc 注释同改
- `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` — `build_args` 末尾追加 `--disallowedTools AskUserQuestion`（`--resume` 之后）
- `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`、`kernel.rs` — 纯注释口径修正（「逐轮驱动（ask 语义）」→ 一轮一命），逻辑零改动

### 测试文件

- `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags_test.rs` — disallowedTools 组装断言（末尾位置、与 `--resume` 组合序、各 permission 档位下均在场）
- `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner_test.rs` — 一轮一命语义用例（单问服务后观测通道关闭、内核可 EOF 收口；泵终止后第二提问按失败收敛）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs` — 既有「泵逐轮常驻」断言改一轮一命口径（单轮收敛后泵任务结束）

### 删除文件

- 无（`history_slot` 为函数内局部状态移除，非文件删除）。

### 不要修改

- `crates/core/agent/src/kernel.rs` 的 `drive` EOF 收口**逻辑**（不引入状态机 break / stop select / 第二收口路径——引擎侧单边 EOF 是唯一收口契约；仅注释口径修正）
- `tests/golden/**`（线契约冻结）
- `crates/core/orchestration/**`（walker、port.rs 注释已正确）
- 前端 `src/**`（chat 基建契约零改动；本轮缺陷修复不触及传输 / 适配层）
- store schema 与落库形态（转录单表 / 轮统计行 / write-through 纪律不变）
- SDK 工具面 / 权限 policy / 路径沙箱（`sdk/policy.rs`、`sandbox.rs` 语义不动）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | CLI 泵一轮一命 | `session_pump` 处理完一个提问后任务返回、observations 发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 随之 EOF 收口（finish_turn + `TurnFinished`、轮行落库终态）；runner_test 新增用例证明，`cargo test`（src-tauri workspace 根）全绿 |
| AC-2 | SDK 泵一轮一命 | `session_pump` 同构收口：单轮收敛后泵任务结束；`history_slot` 无残留（grep 零命中）；既有「逐轮常驻」用例改为单轮口径后全绿 |
| AC-3 | headless 禁 ask | flags_test 断言 `--disallowedTools` + `AskUserQuestion` 恒位于参数序列末尾（`--resume` 之后，各 permission 档位与 New / Continue 均成立）；SDK 侧组装无 disallowedTools 概念 |
| AC-4 | 收敛症状消除（真实引擎冒烟） | debug 面以 CLI 引擎发起一轮含工具调用的会话：turnDone 落库后终端 Record 消息到达、前端收敛、可 Continue 续轮；change-flow run 单相位 executor 收口后 walker 进入 evaluator（phase_log 落账）、停止按钮可用 |
| AC-5 | 内核契约与禁改面保持 | `kernel.rs` `drive` 无状态机 break / 第二收口臂（EOF 唯一缝）；`tests/golden` 零改动；前端 `src/**` 零 diff；`core/orchestration` 零 diff |
| AC-6 | 版本 | 归档时 `packages/desktop/package.json` version bump（0.4.1 → 0.4.2） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 存在未识别的消费方向同一引擎会话通道发第二提问 | 该提问按内核 ask 失败臂收敛 failed（「轮提问送达失败（引擎侧已终止）」）——轮显式失败而非卡死 | 低 | 全库检索 `questions.send` 仅内核 `drive` 一处调用；chat Continue 与 walker 每步新开 session 由既有测试锚定；失败形态严格优于现状（永久挂起） |
| variadic `--disallowedTools` 位置错误吞并后续参数 | `--resume` 等后续 flag 失效，续会话退化为新会话 | 低 | flag 恒置于参数序列最末（`--resume` 之后）；flags_test 断言全序 |
| 个别模型被禁 ask 后行为漂移（拒绝态中止轮） | 轮异常收口 | 中 | `--disallowedTools` 语义为工具不可用，模型以文本提问收口即 turnDone 正常收敛；AC-4 真实引擎冒烟验证；即使漂移，收口形态仍是 turnDone 而非挂起 |
| 泵提前 return 误伤停止 / spawn 失败路径 | 停止收敛或失败记因回归 | 低 | 两路径既有 `return` 语义原样保持（它们本就单轮终止）；runner_test 既有停止 / spawn 失败用例全绿守卫 |
| SDK `history_slot` 移除误伤跨轮上下文 | SDK Continue 丢史 | 低 | SDK 会话重建唯一来源是 store 全史转录（`resume.rs`），`history_slot` 仅泵内跨轮回灌、一轮一命下本就只活一轮；resume_test 全绿锚定 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 泵生命周期 | 方案 A 一轮一命：`session_pump` 处理一问即 return | 无消费方复用会话通道发第二问（chat Continue 与 walker 每步 `compose_turn → begin → open_session` 新开）；drop 发送端即内核 EOF 收口，改动面最小且不触碰内核契约 | B 方案（drive 状态机 break 绕过 EOF）——破坏缝两侧契约单一，否决；保留多问循环按需收口（死代码，否决） |
| headless 提问路径 | `--disallowedTools AskUserQuestion` + 问题文本收口 + Continue 续轮 | 无头场景本不能中途提问；文本收口 turnDone 正常收敛；Continue（`--resume`）既有且 walker / chat 已在用，零新机制 | 审批反向通道（SDK 侧已留独立 change，体量不匹配缺陷修复）；`--allowedTools` 白名单改造（面更大，否决） |
| disallowedTools 位置 | 参数序列末尾（`--resume` 之后） | flag 为 variadic `<tools...>`，置于中间会吞并后续参数 | 置于 permission flag 后 `--resume` 前（有吞并 `--resume` 风险，否决） |
| SDK 侧处置 | 不加 disallowedTools；`history_slot` 移除 | 工具面封闭清单无 ask 工具；跨轮回灌在一轮一命下是死代码，会话史唯一源仍是 store 转录重建 | 保留 history_slot（死代码残留，否决） |
| 内核 drive | 契约零改动，仅注释口径修正 | 引擎侧单边 EOF 是唯一收口缝，测试假 runner `close_stream()` 已锚定该契约；缺陷在真实泵未履约 | drive 增 stop select / break（引入第二收口路径，状态机复杂化，否决） |
| 版本 | 归档时 bump 0.4.2 | 用户可见缺陷修复（run 卡死 / 会话不收敛，沿「仅用户可见变更才提升」） | 不 bump（缺陷修复对用户可见，否决） |

### 待决问题

- 无。修复方向已在 explore 阶段拍板并实测验证；design 相位仅需定测试用例细节与注释措辞。
