# 设计: agent-turn-eof

> **变更**: agent-turn-eof
> **日期**: 2026-10-02

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-agent-execution/spec.md`（路径相对域根）已由提案阶段写入并通过评估，属已完成产物，不在本变更清单与任务列表内。本设计基于其定稿文本展开，仅覆盖「变更范围 - 实现文件」五文件（测试文件 `cli/flags_test.rs`、`cli/runner_test.rs`、`sdk/runner_test.rs` 归 test-design / test-gen / test-execution 阶段承接）。

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | CLI 泵一轮一命收口形态 | `session_pump` 去除 `while let Some(question) = questions.recv().await` 多问循环，改单问服务：`recv` 取一问（`None` 即组合根半边先关，直接 return）→ `spawn_turn` 成功则 `process.run(observations.clone()).await` 完成后**无条件 return**（`stopped` 返回值不再消费——停止路径在 `pump_lines` 内已单轮终止）；spawn 失败臂既有合成收敛 + return 原样。`open_session` / `TurnProcess` / `pump_lines` / spawn 失败 subtype 面零改动 | 一轮一命使 `process.run()` 返回即泵生命周期终点；无论 EOF 收敛还是停止路径均已是单轮终止，无条件 return 与两路径既有语义同形。保留 `.clone()` 属最小 diff（EOF 语义不依赖克隆面，见 D6），不做顺手简化 |
| D2 | SDK 泵同构收口 + `history_slot` 移除 | `session_pump` 同构单问服务；`history_slot`（`Option<Vec<Message>>` 的 `take()` / 回灌）整体移除，泵参数 `history: Vec<Message>` 保留并直传单轮 `r#loop::run`；`run` 返回的累积史在一轮一命下无消费方，泵侧丢弃返回值。`sdk/loop.rs` 签名与实现零改动（返回 `Vec<Message>` 保留）；停止臂既有 `return` 与 biased 臂序语义原样 | 一轮一命下跨轮回灌是死代码；会话史唯一来源仍是 store 全史转录重建（`resume.rs` / `resolve_resume` 零改动）。`loop.rs` 返回值语义成为 crate 内自洽产物，留痕见待决问题 |
| D3 | headless 禁 ask 组装位置 | `build_args` 在 `--resume` 段之后追加两枚 arg：`"--disallowedTools"` 与 `"AskUserQuestion"`——恒为参数序列**最末两枚**；三 permission 档位 × New / Continue（`resume_handle` Some/None）全组合位置一致；工具名字面量为 camelCase `AskUserQuestion`；`TurnParams` 形状与其余 flag 面零变化 | variadic `<tools...>` flag 置中间会吞并后续参数（`--resume` 失效则续会话退化为新会话）；置于 `--resume` 之后是唯一安全位 |
| D4 | 无问收口与续轮（零新机制） | 禁 ask 后 agent 以问题文本作为本轮最终消息 → turnDone 正常收敛（真实引擎行为，desktop 侧零代码触点）；用户应答续轮走既有 `SessionRef::Continue` → 组合根 store 取 `engine_session_id` → `prior_handle` → `--resume`。desktop 侧**零新代码**（walker 决策环与 chat 面已在用该路径） | 无头场景本不能中途提问；文本收口即正常 turnDone 收敛；Continue 既有且已被两类消费方验证 |
| D5 | 内核契约零改动（注释为唯一触点） | `kernel.rs` `drive` 观测循环 `while let Some(kind) = session.observations.recv().await` 保留为唯一收口缝；不增状态机 break / stop select / 第二收口路径。注释位点四处：`kernel.rs` `RunningTurn.question` 字段（「ask 语义的驱动载荷」）与 `drive` doc（「泵驱动（ask 语义）」）；`runner.rs` `AgentSession.questions` 字段（「逐轮送达提问」）与 `AgentRunner::open_session` doc（「逐轮驱动（ask 语义）」）。`kernel_test.rs` 假 runner 的 `close_stream()`（引擎侧单边 EOF）不动——真实泵自此履行同一契约 | 缺陷在泵侧未履约而非内核契约错误；B 方案（状态机 break）破坏缝两侧契约单一，已否决。`converge` doc 的「ask 失败」臂措辞保留——该臂在一轮一命下恰是「泵返回后第二问送达」的 canonical 失败路径，语义仍真 |
| D6 | EOF 释放链语义（实现与测试装置的共同机械依据） | 观测通道 `recv()` 返回 `None` 当且仅当**全部发送端** drop。发送端唯一持有链：`open_session` 产出的 `observation_tx`（move 进 `session_pump`）→（D1 保留的克隆）移交单轮处理。泵任务 return ⇒ 全链 drop ⇒ 观测通道关闭 ⇒ 内核 EOF 收口；无显式 close 调用。`TurnProcess::run` 的克隆随 `pump_lines` 返回先释放，泵本体的随后随 return 释放 | tokio mpsc 无「半边关闭」概念；测试装置若残留任一发送端，`recv()` 恒不返回 `None`——该 nuance 交 test-design 阶段落为装置约束，本变更实现面只须保证泵 return |
| D7 | 版本交付口径 | `packages/desktop/package.json` version 0.4.1 → 0.4.2 于**归档时**执行（proposal AC-6 口径）；不入实现变更清单与任务列表（实现清单零 package.json 触点）。`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动（沿 desktop-change-create 先例） | 用户可见缺陷修复（run 卡死 / 会话不收敛）沿「仅用户可见变更才提升」；proposal 将其显式置于归档动作而非实现文件面 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| CLI 引擎泵一轮一命 | `session_pump` 单问服务：recv 一问 → CLI 发现 → spawn → 逐行泵（EOF / 停止）→ 泵返回（发送端释放 → 内核 EOF 收口）；spawn 失败臂合成收敛后终止；多问循环移除；doc 注释一轮一命口径 | `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` | `core/agent` 协议面（`agent` crate）、`tokio`（mpsc / select / process）、crate 内 `discover` / `flags` / `jsonl` | Rust（tokio 任务） |
| CLI 命令行组装 | `build_args` 尾追加 `--disallowedTools AskUserQuestion`（`--resume` 之后恒最末两枚）；其余 flag 面与 `TurnParams` 零变化 | `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` | `agent::AgentPermissionMode`、`serde_json`（档位值派生） | Rust |
| SDK 引擎泵一轮一命 | `session_pump` 同构单问收口；跨轮 `history_slot` 死代码移除（`history` 直传单轮 loop、返回值弃用）；doc 注释同改；`resolve_resume` / biased 停止臂语义不动 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `rig_core`（`CompletionModel` / `Message`）、`core/agent` 协议面、crate 内 `sdk::loop` / `sdk::resume`、`tokio` | Rust（tokio 任务） |
| 内核收口缝（仅注释口径） | `drive` EOF 唯一收口逻辑零改动；`RunningTurn.question` 字段与 `drive` doc 注释改一轮一命口径 | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | — | Rust（仅注释） |
| 协议面注释口径 | `AgentSession.questions` 字段与 `AgentRunner::open_session` doc 改单问驱动（一轮一命）口径；类型 / trait 签名零改动 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | — | Rust（仅注释） |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件（flags_test / cli runner_test / sdk runner_test）归 test-design / test-gen / test-execution 阶段，不在本清单。 -->

<!-- 无新增文件（proposal「删除文件：无」；本轮全部为既有文件修改，故新增/删除两个子节省略）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs` | `session_pump` 去多问 `while let` 循环改单问服务（D1）；`session_pump` doc 注释与 `QUESTION_CHANNEL_CAPACITY` 注释（「逐轮送达」）改一轮一命口径 | 互持钥匙死锁的 CLI 半边解除：泵返回 → observations 发送端释放 → 内核 EOF 收口；`open_session` / `TurnProcess` / `pump_lines` / `spawn_turn` / `kill_process_tree` / `build_command` 逻辑零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `session_pump` 同构单问收口 + `history_slot` 整体移除（D2）；`session_pump` doc 注释、biased select 注释续轮回灌措辞与 `QUESTION_CHANNEL_CAPACITY` 注释同改 | SDK 半边同构解除；`SdkRunner` / `resolve_resume` / `continue_session_id` / `build_model` / `next_session_id` 零改动，`sdk/loop.rs` / `sdk/resume.rs` 零触点 |
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs` | `build_args` 末尾追加 `"--disallowedTools"` + `"AskUserQuestion"` 两枚 arg（D3，`--resume` 段之后恒最末） | headless 禁 ask；variadic flag 不可置中间；`TurnParams` 形状与既有四要素 / permission 档位 / `--resume` flag 面零变化 |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | 纯注释（D5）：`RunningTurn.question` 字段注释与 `drive` doc 注释「ask 语义」改一轮一命口径 | `drive` 观测循环、`converge`、`begin_turn`、`StopRegistry`、状态机消费逻辑零改动（EOF 唯一收口缝保持） |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 纯注释（D5）：`AgentSession.questions` 字段注释与 `AgentRunner::open_session` doc「逐轮」措辞改单问驱动口径 | 类型 / trait / 字段签名零改动；`RunHandle` / `SessionRef` / `SessionOpen` / `TurnQuestion` 形状零变化 |

### 公共函数 / API

Rust 签名为 Rust 语法。`session_pump`（cli/runner.rs、sdk/runner.rs）为模块私有 `async fn`，不在公共 API 面，其行为契约由架构组件与修改文件表承载；公共导出面除下表外零变化。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `build_args` | `crates/infra/agent/src/cli/flags.rs` | 修改 | `pub fn build_args(params: &TurnParams) -> Vec<String>` | 签名不变；产出序列在最末（`--resume <id>` 两枚若在场，之后）恒追加 `"--disallowedTools"`、`"AskUserQuestion"` 两枚；其余段序（`-p` / prompt / `--output-format stream-json` / `--verbose` / permission 档位 / `--resume`）与相对次序零变化 |

<!-- 类型定义：无 —— TurnParams / AgentSession / SessionOpen / SessionRef / TurnQuestion 形状零变更，省略此子节。 -->

<!-- 配置：无 —— 版本提升（0.4.2）按 proposal AC-6 归档时执行（D7），不入实现清单，零配置键触点。 -->

---

## 数据模型

本变更零 schema 变更、零新模型（proposal「不要修改」：store schema 与落库形态不变）；下表仅留痕修复所涉既有持久形态——本修复使「轮行终态」写路径在真实引擎下从永不可达变为每轮可达，落库形态本身不变。

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| 轮行（既有） | `status`（running → completed / failed / stopped 终态）、`finished_at`、`num_turns`、`duration_ms`、`cost_usd`、`usage`、`error` | `begin_turn` 建 running 行 → 泵一轮一命使观测通道关闭 → `drive` EOF 收口 → `converge`（`finish_turn`）写终态 + `StopRegistry` 除名 + `TurnFinished` 流出 | redb（`store_port.rs` 桥；形态零变更） |
| 事件转录（既有单表） | seq 单调密封事件（末位 TurnDone） | `drive` 密封 write-through 半边逐条落库（增量不上库） | redb（形态零变更） |
| 双 id 映射（既有） | core session id ↔ engine session id | `RunStarted` / `TurnDone` 上报位经 `bind_remote_session` 尽力绑定 | redb（形态零变更） |

<!-- 不涉及 HTTP API（IPC 命令面零新增零改动），路由/API 设计节省略。 -->

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | cli/runner.rs `session_pump` 一轮一命改写（D1）+ EOF 释放链（D6）：泵返回 → 发送端全 drop → 观测通道 `recv` 返回 `None` → 内核 EOF 收口（`finish_turn` + `TurnFinished`、轮行终态落库）；「runner_test 新增用例」由 test-design / test-gen 阶段承接，自动化套件的执行验证归 test-execution 阶段 |
| AC-2 | sdk/runner.rs 同构收口（D2）：单轮收敛后泵任务结束、`history_slot` 整体移除（grep 零命中列守线静态自查）；既有「逐轮常驻」断言改单轮口径由 test-design / test-gen 阶段承接，执行验证归 test-execution 阶段 |
| AC-3 | cli/flags.rs `build_args` 尾追加（D3）：`--disallowedTools` + `AskUserQuestion` 恒位于序列最末两枚（`--resume` 及其值之后，三档 × New / Continue 全组合成立）；SDK 组装面零触点（rig 封闭工具清单，无 disallowedTools 概念）；「flags_test 断言」归 test-design / test-gen 阶段 |
| AC-4 | 修复本身即症状消除根因面（泵收口 + headless 禁 ask + Continue 续轮零新机制，D1–D4）；真实引擎冒烟（debug 面 CLI 会话收敛可 Continue、change-flow 单相位 executor 收口后 walker 进 evaluator、停止按钮可用）归 acceptance 阶段人工验证，不属本设计与任务列表的静态守线 |
| AC-5 | D5 内核契约零改动——`kernel.rs` / `runner.rs` 仅注释位点 diff；禁改面（`tests/golden/**`、前端 `src/**`、`core/orchestration/**`、store schema、`sdk/policy.rs` / `sandbox.rs`）零 diff 与「`drive` 无状态机 break / 第二收口臂」列守线静态自查 |
| AC-6 | 归档时 `packages/desktop/package.json` version 0.4.1 → 0.4.2（proposal AC-6 口径，D7）；实现变更清单与任务列表零 package.json 触点 |

---

## 依赖

### 运行时依赖

- 无新增 — `tokio`（mpsc / select / process）、`serde_json`、`rig_core`、`core/agent` 协议面均为既有依赖，本轮零新引入

### 构建/测试依赖

- 无新增 — 沿 cargo workspace（src-tauri 根注册，套件自动覆盖新改动）、`pnpm -C packages/desktop run server:check` / `client:check` 既有工具链

---

## 待决问题

- 无 — proposal 待决问题为空，修复方向已在 explore 阶段拍板并实测验证；design 相位定稿了泵收口形态（D1 / D2）、flag 位置（D3）与注释位点（D5）。
- 留痕（非待决）：`r#loop::run` 返回值在 runner 侧弃用后，`sdk/loop.rs` 签名（返回 `Vec<Message>`）保留不动；若未来变更复活常驻泵或跨轮回灌，须同步复核该返回值语义与 `sdk/resume.rs` 全史重建链的分工。
