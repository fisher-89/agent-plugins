# 测试设计: ask-tool-in-flow

> **变更**: ask-tool-in-flow
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（问题四现象 / D1–D14 裁决 / 三层改动 / 变更范围实现文件 **34 行** + 测试文件 **18 行** / AC-1..AC-13 / R1–R10）、`design.md`（DD1–DD15 / 架构组件 15 行 / 新增文件 **2 行** + 修改文件 **33 行** / 编译闭合批 10 行 / 公共函数·API **21 行** + 退役面 4 项 + 越界面 6 组 / 类型定义 14 行 / 路由与错误语义 / AC 落点表）、四份 delta spec（`desktop-agent-execution` 4 MODIFIED + 1 ADDED / `desktop-change-orchestration` 3 MODIFIED + 1 ADDED / `desktop-change-flow-view` 1 MODIFIED / `desktop-agent-chat-infra` 2 MODIFIED + 1 ADDED）
> **被测范围**: design「新增文件」2 项（`core/agent/src/ask.rs`、`core/orchestration/src/ask.rs`）+ design「修改文件」全 33 项（其中 `packages/desktop/package.json` 为版本行）。**删除文件零项、删除公共面 4 项（`answer_tx` / `wait_answer` / `decision_answer_prompt` / `finish_step` 的 ask 枚举值与两 property）不纳入被测范围，其零残留判据另行登记。**
> **本变更性质**: ask 由「`finish_step` 的一个枚举值」升为挂载清单里的独立阻塞工具，应答通道由 `ChangeFlowControl.answer_tx` 下沉为会话级 `AskRegistry`。行为面集中在四处：① 纯表（注册表三方法 take 语义 / ask 工具协议校验）；② 会话基建（`ask` 位与 `SessionOpen.session_id` 的 serde 与下传、轮收敛除名）；③ 执行体两档（`Generic` 只阻塞 / `Flow` 额外广播 + 两臂清理单点）；④ 双入口投递与停等清理分离（命令零清理、`resolve_ask` 单点条件清理）。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**` 与 `src/**`） | `cargo nextest run --workspace`（`server:test`；本变更为 `-p agent` / `-p orchestration` / `-p agent-runtime` / `-p dev-team` 四包定向） | **sidecar 同目录同前缀** `*_test.rs`，源文件尾 / `mod.rs` / `lib.rs` 以 `#[cfg(test)] mod <name>_test;` 挂载（`core/agent/src/lib.rs:14-28` 九条、`core/orchestration/src/lib.rs:34-52` 十二条、`sdk/mod.rs:24-44` 先例）；**本变更新建 2 个测试文件**（`core/agent/src/ask_test.rs`、`core/orchestration/src/ask_test.rs`）+ **19 个既有 sidecar 改造/追加**（本表下方「测试文件面清单」） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**；装置为手写端口假件（`FakeModel` / `FakeWorker` / `FakeTools` / `StubSnapshot` / `DualEnv` / `Env` + `tauri::test::mock_app`）与 `include_str!` 源码自证（`finish_test.rs:72` / `verdict_test.rs:44` / `mount_test.rs:47` 先例） |
| 前端（`packages/desktop/src/**`） | `vp test`（vite-plus 承载 vitest；`client:test`） | **同目录同前缀** `*.test.tsx` / `*.test.ts`（`components/agent/*.test.tsx`、`hooks/use-agent-session.test.ts`、`views/**/*.test.tsx` 先例）；**本变更零新建测试文件，11 个既有测试文件追加/改造** | `vite-plus/test` 的 `describe` / `it` / `expect` / `vi`；`@testing-library/react`（`render` / `screen` / `fireEvent` / `waitFor` / `within`）；`// @vitest-environment jsdom` 首行；进程边界一律 `vi.mock('@tauri-apps/api/core', ...)` + `vi.hoisted` 的 `ChannelMock` / `invokeMock` |
| 生成物 / 配置 | — | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json` **零直接测试路径**（生成物由 `bindings:export` + `src/bindings/mod_test.rs` 反向锚定守住；版本行走 diff 审查） | 登记于「不可测试项」 |

**测试文件面清单（本变更触及的 21 个 Rust 测试文件 + 11 个前端测试文件）**

| 面 | 新建 | 改造 / 追加 |
|---|---|---|
| Rust | `core/agent/src/ask_test.rs`、`core/orchestration/src/ask_test.rs` | `core/agent/src/{runner,kernel,lib}_test.rs`；`core/orchestration/src/{port,state,control,finish,decision,prompt,walker}_test.rs`；`infra/agent/src/sdk/{mount,loop,runner}_test.rs`；`infra/agent/src/{lib,compose,worker}_test.rs`；`src/commands/{exec,change_flow}/mod_test.rs`；`src/bindings/mod_test.rs` |
| 前端 | —（零新建） | `hooks/use-agent-session.test.ts`；`components/agent/{tool-cards,agent-messages,agent-session-view,session-panel,index}.test.tsx`；`views/explores/explore-detail-pane.test.tsx`；`views/agent/agent-debug-view.test.tsx`；`views/changes/flow/{session-transcript-panel,detail-drawer}.test.tsx`；`views/changes/change-detail-view.test.tsx` |
| 复用（零改动，回归判据） | — | `infra/agent/src/sdk/{resume,tools,protect/*}_test.rs`；`views/changes/flow/run-control-panel.test.tsx`；`components/agent/agent-blocks.test.tsx`（见「不可测试项」） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、防御式分支、并发与竞态窗口）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该断言由既有用例锁定，本设计**只做回归登记**（不新增断言、不改断言口径）；未带前缀者为新增或按新形状改写。
- **「↔ 编译闭合」**：该行只是让工作区随字段面 / 签名收敛编译通过的机械点位，**MUST NOT 新增用例或改断言值**（行内只登记点位与计数）。
- **「下游 test-gen 补，G3 骨架」**：该行是回退原因点名的三处缺失骨架（`finish_step` 的 backtrack 往返形态 / `agent-messages` 的 `output-available` 静态态 / `session-transcript-panel` 的双 ask 对账）在本设计里的补位声明——本节只钉用例坐标与断言口径，用例代码由 test-gen 落地。
- **「引用相等」判据的观测装置 = 探针装置**：凡用例条件含「收到同一对象」「同一引用」「同引用」「不击穿 memo」一类 **props 引用相等**断言的，都**不可**由 DOM 结构或行为断言反推（DOM 只呈现值、不呈现引用；行为断言只能证明「调用发生」）。该判据只能经**探针装置**观测：以 `vi.mock(<被测下游模块路径>, async (importOriginal) => { const actual = await importOriginal(); return { ...actual, <被观测组件>: <探针组件> }; })` 包住**被测下游组件**——`importOriginal` 保留全部真实实现，探针只做一件事：把收到的 props 逐次 push 进 `vi.hoisted` 的 `propsLog` 后原样透传（`<Real {...props} />`）。断言读 `propsLog[0].askAnswer === propsLog[1].askAnswer`（引用比较）与 `propsLog[n].askAnswer === <注入对象>`（同源比较）。因此这些章节的 Mock策略表里，**探针装置条目与被观测组件条目并存**：探针**不替换实现**（`importOriginal` 保真、下游行为照旧），只是**在边界上加一层 props 记录**；「不 mock」口径相应写明「不 mock 实现、仅记录 props」。**探针挂点只能用既有出口面**：模块级导出组件（`AgentMessages` / `AgentSessionView` / `SessionTranscriptPanel` / `DetailDrawer`）或既有注册表键（`BUILD_IN_TOOL_CARDS['AskUserQuestion']`）；**MUST NOT 为探针新增产品导出**（test-only export 会被死代码扫描判为未用，污染报告）。例外：hook 自身（`use-agent-session`）的引用稳定性不需要探针——`renderHook` 的返回值直接持有引用。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `String`（`session_id` / `question` / 应答 `text`） | `""`（空串）、纯空白 `"   "`（寻址参数在命令面被拒、注册表键不 trim）、`"\n`、`\0`、emoji（`"a\0b🚀"`）、超长（`"字".repeat(1200)`，>1000 字符）、恰好阈值（`MAX_REASON_CHARS` 类常量所在层不在本变更面）；`None` 由 `Option<String>` 承担 |
| `Vec<String>`（`options`） | `[]`（空数组 → `validate` `Err`）、单元素（最小合法）、超大列表（1000 元素）、非字符串元素（`Err`）、`None` / 非数组（`Err`） |
| `Option<String>`（`tool_call_id` / `session_id` / `prior_handle`） | `None`（旧线载荷）、`Some("")`（空串可构造，结构层不校验）、`Some(<超长>)` |
| `Option<AskMode>`（`ask` 位） | `None`（显式禁用 / 旧线缺字段）、`Some(Generic)`、`Some(Flow)`（N+1 = 3）；非法线值 `"unknown"` 反序列化 `Err` |
| `bool`（`AskRegistry::register` 返回值） | `true`（登记时该键已空）/ `false`（顶替上一问，旧 `Sender` 随覆盖 drop）；无 `None`（非 `Option`） |
| `oneshot::Sender<String>` 槽 | 登记未消费 / take 后 miss / 同键覆盖后旧接收方 `RecvError` / 接收方先行 drop 后 send 落空 四形态 |
| `Mutex<HashMap<String, _>>`（表状态） | 单键空表、多键隔离、同键并发登记与并发 `answer`（恰一成功）、`remove` 后迟滞投递、miss 幂等 |
| TS `pendingToolCallId?: string \| null`（三态判活） | `undefined`（未给出 → 按 `input-available` 判活）、`null`（明确无等待 → 全静态）、`""`（空串 → 不匹配任何卡）、不匹配串（该卡静态）、匹配串（该卡活） |
| TS `askAnswer` 动作面 | 未注入（全静态）/ 注入（活卡）；`onAnswer` 引用稳定性（`useCallback` / `useMemo` 同一引用）——**引用相等只能经探针装置观测**（`importOriginal` 保留真实实现 + 记录 props，见「用例表列语义」末条） |
| `float` | 零 float 面：无 `f64` 入参或字段，本变更全部判据为字符串 / 枚举 / 计数 |

**跨模块组合用例挂靠（按链路发起方 / 最上层调用方）**

- **装配链**（`assemble_injections` → `definitions` → `RunStarted.tools`）发起方 = `compose` ⇒ 组合用例写 `crates/infra/agent/src/compose.rs -> compose_test.rs` 章节。
- **通用侧应答链**（`agent_answer` 命令 → `AskRegistry` → `LoopTurn.ask` 铸造 → `ToolExec::Ask(Generic)` 执行体回灌 → 同 turn 续跑）最上层调用方 = 命令面 `commands/exec` ⇒ 组合用例写 `src-tauri/src/commands/exec/mod.rs -> mod_test.rs` 章节。
- **流程侧双入口链**（`change_flow_answer` / `agent_answer` → `AskRegistry`；执行体返回 → `AskPort::ask_resolved` → `control.resolve_ask`）最上层调用方 = 命令面 `commands/change_flow` ⇒ 组合用例写 `src-tauri/src/commands/change_flow/mod.rs -> mod_test.rs` 章节。
- **双端同步呈现链**（`activeRun.ask` 单源 → 详情页 `AskCard` 与聊天窗活卡；应答 → `RunNotice::Step` → 双处收起）最上层调用方 = 视图 `change-detail-view` ⇒ 组合用例写 `views/changes/change-detail-view.tsx -> change-detail-view.test.tsx` 章节。
- 不新建独立集成测试章节、不新建组合测试区；跨 crate 的「同名覆盖」判据以**定义逐字节相等对拍**（`orchestration::ask::{description, parameters}` 与两档挂载项）承载。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/{core/agent,core/orchestration,infra/agent}/src/**/*_test.rs` 与 `packages/desktop/src-tauri/src/**/mod_test.rs`（sidecar，`cargo nextest`）；前端侧 `packages/desktop/src/**/*.test.ts(x)`（`vp test`）。本变更新建 2 个测试文件（`core/agent/src/ask_test.rs`、`core/orchestration/src/ask_test.rs`）、改造 19 个既有 Rust sidecar 与 11 个既有前端测试文件（32 个测试文件 → **34 个 per-file 章节**：`core/orchestration/src/ask_test.rs` 承接 `ask.rs` 与 `lib.rs` 两个源文件、`src/bindings/mod_test.rs` 承接 `commands/mod.rs` 与生成物 `bindings.ts` 两个源文件）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **通用会话同 turn 阻塞应答**：`source = explore`（或 archi / debug）会话里模型调 `AskUserQuestion` → 该轮不收敛、`tool_use` 保持未配对 → `agent_answer` 投递 → `tool_result` 回灌 → 同一 turn 内继续（零新会话 / 零 `SessionRef::Continue`）；注册表键 = 该会话 core session id | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `compose_test.rs`（三臂 `ask` 位）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（`Generic` 臂阻塞 → 回灌 → 续跑）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `sdk/runner_test.rs`（per-turn 铸造 + core id 透传）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（`ask = Some(_)` 追加）；`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` -> `lib_test.rs`（门面两条 builder 注入缝）；`packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（组合行） |
| AC-2 | **流程覆盖与双端同步**：`source = change` 下发同一 `AskUserQuestion`（名称 / schema 同源），额外把 `activeRun` 切 `WaitingAsk`、`AskPayload.tool_call_id` == 该 `tool_use` id、`session_id` == core session id；两入口任一应答后两卡同步收起、`status` 回 `running`、`ask` 槽清空 | `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` -> `control_test.rs`（`answer` 转投 + `resolve_ask` 条件清理）；`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` -> `port_test.rs`（`AskPort` 两方法）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（`Flow` 臂 `ask_started` → 应答 → `ask_resolved` 恰一次）；`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` -> `mod_test.rs`（`AskPort for ChangeFlowSink` + 双入口组合行）；`packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` -> `worker_test.rs`（`AskPort` 句柄透传组合挂靠）；`packages/desktop/src-tauri/src/main.rs`（壳层托管两行，见「不可测试项」）；`packages/desktop/src/views/changes/change-detail-view.tsx` -> `change-detail-view.test.tsx`（双端同源组合行） |
| AC-3 | **显式禁用面**：`ask = None` 的会话 `RunStarted.tools` 不含 `AskUserQuestion`、`definitions()` 无该挂载项；模型硬调即 `tool_unavailable`（`SystemNotice` + is_error ToolResult，run 不中断） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（四组合）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（幻觉调用被拒，既有机制零改动）；`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `compose_test.rs`（逐臂显式置位，MUST NOT `default()`） |
| AC-4 | **解耦**：`orchestration/ask.rs` 全文零 `finish` 字样；`finish.rs` 的 `ask` 只在文案字符串；`mount.rs` 两段独立 `if`；`finish = None` 的会话仍有 ask、`ask = None` 的会话仍有 `finish_step`；`finish_step(Decision)` 的 `enum == ["backtrack","stop"]` 且无 `question` / `options` | `packages/desktop/src-tauri/crates/core/orchestration/src/ask.rs` -> `ask_test.rs`（源码自证）；`packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs` -> `finish_test.rs`（schema 收窄 + 文案指路 + 源码自证）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（四组合 + 无合取自证） |
| AC-5 | **停止语义**：ask 阻塞期间停止 → 轮收敛 `stopped`；转录留未配对 `tool_use`，`resume::rebuild` 补 `INTERRUPTED_RESULT`；`AskRegistry` 槽由 `converge` 除名（其后 `answer` → `Err`）；`Flow` 臂取消分支仍执行 `ask_resolved` | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `kernel_test.rs`（`converge` 除名）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（停止臂 + `ask_resolved` 恰一次）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs`（既有 `INTERRUPTED_RESULT` 用例零回归，不列待测功能） |
| AC-6 | **参数校验**：`question` 必填非空；`options` 非空字符串数组（`minItems: 1`）；`Err` 文案自足（字段名 + 期望 + 当前值）；自由文本应答两路并存 | `packages/desktop/src-tauri/crates/core/orchestration/src/ask.rs` -> `ask_test.rs`（schema + 六态 `validate`）；`packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs` -> `ask_test.rs`（出口面六名可达）；`packages/desktop/src/components/agent/tool-cards.tsx` -> `tool-cards.test.tsx`（选项按钮与自由文本输入两路） |
| AC-7 | **重复应答幂等**：同一 ask 二次应答 → `Err`（「当前无等待中的 ask」），等待方只收一次原文、不产生第二次回灌；`remove` 后迟滞应答同样 `Err` | `packages/desktop/src-tauri/crates/core/agent/src/ask.rs` -> `ask_test.rs`（take 语义）；`packages/desktop/src-tauri/crates/core/agent/src/lib.rs` -> `lib_test.rs`（出口面锚定）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（单次回灌）；`packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（命令面二次调用） |
| AC-8 | **单通道**：`ChangeFlowControl` 无 `answer_tx` 槽与 `wait_answer()`（`grep` 零命中）；`answer()` 与 `agent_answer` 都只投 `AskRegistry`；`accept_pending` / `confirm()` / `wait_confirm()` 逐字未改（`confirm` 侧既有四条用例零改动通过） | `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` -> `control_test.rs`（`answer` 三段判定 + `resolve_ask` + 退役面自证 + confirm 回归登记）；`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` -> `mod_test.rs` |
| AC-9 | **载荷 additive 与出线**：`AskPayload` 增 `toolCallId` / `sessionId`（缺省 `null`）；`RunUpdate::Ask { ask }` 为载荷单源；`RunNotice` 六变体与 `kind()` 零变；`AskMode` / `SessionInjections` 不出线 IPC | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` -> `state_test.rs`（serde + 变体收敛 + `kind()` / 通知投影）；`packages/desktop/src-tauri/crates/core/agent/src/runner.rs` -> `runner_test.rs`（`AskMode` 线值 + 四字段 serde）；`packages/desktop/src-tauri/src/bindings/mod_test.rs`（反向锚定，登记于 `commands/mod.rs` 章节与「不可测试项」）；`packages/desktop/src/types/generated/bindings.ts`（生成物，读盘快照断言 + diff 审查） |
| AC-10 | **命令面**：`agent_answer` 为薄包装（blank 参数 `Err`、未登记 `Err`、命中 `Ok`）；命令落 `commands/exec` 轨道并登记于 `all_commands!` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（四态）；`packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`（登记面 + 计数 + 生成序） |
| AC-11 | **前端双态卡**：注入动作面 → 活卡（选项按钮 + 自由文本 + 提交后禁用防连点）；未注入 → 静态只读卡（旧文案「请在下方输入框作答」不再出现）；`pendingToolCallId` 给出时仅该 id 为活卡；未注册工具与 `ToolPairCard` 回退零变 | `packages/desktop/src/components/agent/tool-cards.tsx` -> `tool-cards.test.tsx`（双态 + 双形态解析）；`packages/desktop/src/components/agent/agent-messages.tsx` -> `agent-messages.test.tsx`（三态判活矩阵 + 引用稳定）；`packages/desktop/src/components/agent/agent-session-view.tsx` -> `agent-session-view.test.tsx`（两形态同透传、不自动接线）；`packages/desktop/src/components/agent/index.tsx` -> `index.test.tsx`（类型再导出） |
| AC-12 | **决策协议收窄**：决策会话单轮收口（ask 不再开新一轮）；`finish_step(Decision)` schema 不含 `ask`；`finalized = Ask` 载荷出现即 `Terminal::failed`（防御文案指路 ask 工具）；`decision_prompt` 两动作 + ask 指路；`decision_answer_prompt` 全仓零命中；`DecisionAction::Ask` 与 `validate_decision` 的 `"ask"` 臂保留 | `packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs` -> `finish_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` -> `decision_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `prompt_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `walker_test.rs`（去循环化 + 防御臂） |
| AC-13 | **消费点接线**：explore / archi / change-chat / debug 四处通用会话可应答（`agent_answer`）；change 流程抽屉可应答（`change_flow_answer`，`pendingToolCallId` 取 `activeRun.ask.toolCallId`）；只读历史重放零应答入口 | `packages/desktop/src/components/agent/session-panel.tsx` -> `session-panel.test.tsx`（骨架合成）；`packages/desktop/src/views/explores/explore-detail-pane.tsx` -> `explore-detail-pane.test.tsx`；`packages/desktop/src/views/agent/agent-debug-view.tsx` -> `agent-debug-view.test.tsx`；`packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` -> `session-transcript-panel.test.tsx`（未注入即全静态）；`packages/desktop/src/views/changes/flow/detail-drawer.tsx` -> `detail-drawer.test.tsx`；`packages/desktop/src/views/changes/change-detail-view.tsx` -> `change-detail-view.test.tsx`；`packages/desktop/src/hooks/use-agent-session.ts` -> `use-agent-session.test.ts`（`answer` 动作） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/agent/src/ask.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/ask_test.rs`

**新建 sidecar**（沿 `kernel_test.rs` 的 `StopRegistry` 同族先例）；`lib.rs` 加 `#[cfg(test)] mod ask_test;` 一行。装置为零进程边界对拍：用例直接持有 `oneshot::channel` 的接收方，注册表为真实实例。

#### 待测功能

- `AskRegistry::new() -> Self`: 空注册表（`Default` 派生的薄包装，壳层 `manage` 需要 `Default`）。
- `AskRegistry::register(&self, session_id: &str, tx: oneshot::Sender<String>) -> bool`: 登记等待方；**恒写入**（同键覆盖旧等待方，旧 `Sender` 随覆盖 drop）；返回值 = 「登记时该键已空」（DD1 诊断值，非失败面）。
- `AskRegistry::answer(&self, session_id: &str, text: String) -> Result<(), String>`: take 语义——移除槽并 `send`；miss / 重复 → `Err("当前无等待中的 ask")`；零编排域词汇（无 change id、无 `RunEventSink`）。
- `AskRegistry::remove(&self, session_id: &str)`: 除名（轮收敛兜底）；miss 幂等。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskRegistry::new` / `AskRegistry::answer` | 正向 | 空表形态：`new()` 后立即 `answer("s1", "x")` → `Err`，文案含「当前无等待中的 ask」；`AskRegistry::default()` 与 `new()` 行为等价（可构造、可登记、可 answer） | 单次 |
| `AskRegistry::register` / `AskRegistry::answer` | 正向 | 首登往返：`register("s1", tx)` → `true`；`answer("s1", "选项A")` → `Ok`；接收方 `rx.await` 逐字 `== "选项A"`（take 语义的投递半边） | 单次 |
| `AskRegistry::answer` | 正向 | take 语义：同一键 `answer` 两次 → 第一次 `Ok`、第二次 `Err`；接收方**只收到一次**（无第二次回灌） | 单次 |
| `AskRegistry::register` | 正向 | 同键覆盖（DD1）：先 `register("s1", tx1)` 再 `register("s1", tx2)` → 第二次返回 `false`；`tx1` 侧接收方收 `RecvError`（旧等待方被顶替）；`answer("s1", …)` 命中 `tx2` 的接收方 | 单次 |
| `AskRegistry::remove` | 正向 | 除名与幂等：`register` → `remove` → `answer` → `Err`；`remove` 对未登记键连调两次不 panic、不波及其它键 | 参数化迭代（2 形态：已登记键 / miss 键） |
| `AskRegistry::register` / `AskRegistry::answer` | 正向 | 多键隔离：`s1` / `s2` 各自登记 → `answer("s1")` 只唤醒 `s1`，`s2` 接收方仍 pending（`try_recv` 断言未就绪）；随后 `answer("s2")` 成功 | 组合迭代（2 键 × 2 顺序） |
| `AskRegistry::answer` | 异常 | 三形态 miss（AC-7）：① 从未登记 ② `remove` 后迟滞应答 ③ 已被 take 后重复应答 → 三者均 `Err`，文案逐字相同（`"当前无等待中的 ask"`），且不改写表项（其后同键 `register` 仍返回 `true`） | 参数化迭代（3 形态） |
| `AskRegistry::answer` | 异常 | 接收方先行 drop：`let (tx, rx) = channel(); register("s1", tx); drop(rx);` → `answer("s1", …)` 的槽已被 take，**只断言其后同键 `answer` 必 `Err`**（槽已空）、不裁决首次返回值、不 panic | 单次 |
| `AskRegistry::register` / `AskRegistry::answer` | 边界 | 键边界：`""` / `"   "` / `"a\0b🚀"` / `"字".repeat(1200)` 四形态 → 表不 trim 不截断，原串可被同串 `answer` 命中；近邻键（`""` vs `" "`）互不命中 | 参数化迭代（4 形态） |
| `AskRegistry::answer` | 边界 | 正文边界（零加工）：`""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 接收方逐字收到同一串（零 trim、零截断、零空白校验） | 参数化迭代（5 形态） |
| `AskRegistry::answer` | 边界 | 并发取用原子性：同键 `register` 后两线程并发 `answer` → 恰一个 `Ok`、恰一个 `Err`，接收方收一次（`std::thread` + `Arc` 共享注册表，不引异步运行时） | 单次 |
| `AskRegistry::register` / `AskRegistry::remove` / `AskRegistry::answer` | 边界 | 并发读写不 panic：多线程混合 `register` 同键 / `remove` / `answer` → 无 panic、无死锁；结束态可查（末次 `answer` 结果二值之一） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是进程内内存表 + 既有 `tokio::sync::oneshot`；无 IO、无配置、无环境变量、无子进程 | 全章节 |
| 不 mock：`oneshot::channel` | 真实通道对拍——take 语义与「同键覆盖即 drop 旧 Sender」两条判据只能由真实通道的 `RecvError` / `await` 观测 | 全章节 |
| 不 mock：`Mutex` / `HashMap` | 真实容器（并发行需要真实锁语义；替身化会让原子性断言空转） | 并发两行 |

### `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs`

**既有 sidecar 改造**：三字段用例扩为四字段；追加 `AskMode` 线值与 `SessionOpen.session_id` 两条断言组。既有 `FinishSpec` / `SessionCtx` / `ModelLevel` / `AgentRunStatus` serde 用例零改动（回归登记）。

#### 待测功能

- `AskMode`（新增枚举，`{ Generic, Flow }`）: `Copy + Eq + Serialize + Deserialize`、`#[serde(rename_all = "camelCase")]` ⇒ 线值 `"generic"` / `"flow"`；**不派生 `specta::Type`**、不加 `Default`。
- `SessionInjections.ask: Option<AskMode>`（第四位，`#[serde(default)]`）: 四字段延展后 `PartialEq + Eq` / `Default` 链完好；`None` = 显式禁用（fail-closed）。
- `SessionOpen` 的 `session_id`（新增字段，`String`）: core 铸 id 下传（用途 = ask 登记键与引擎侧归属）；`TurnQuestion` 不动。

<!-- design.md「公共函数 / API」表未声明本文件的公共 API 变更（该表无 runner.rs 行）；上述三条取自 design「类型定义」表该文件三行与「修改文件」表该文件行。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskMode { Generic, Flow }` | 正向 | 线值往返：`Generic` ↔ `"generic"`、`Flow` ↔ `"flow"` 双向 round-trip 逐字相等；与 `FinishSpec` 的 `"decision"` / `"evaluatorChecklist"` 词面互不混用 | 参数化迭代（2 变体） |
| `SessionInjections` | 正向 | 四字段往返：`{preamble: Some, tools: Some([...]), finish: Some, ask: Some(Flow)}` → serialize → deserialize 逐字段相等；`PartialEq` / `Eq` 链（同值相等、`ask` 不同即不等、`finish` 不同即不等） | 单次 |
| `SessionOpen` 的 `session_id` | 正向 | 构造面：`SessionOpen { injections, ctx, session, prior_handle, session_id }` 逐字段可读；`SessionRef::New` 与 `Continue { id }` 两形态均可构造（`session_id` 与 `session.id` 互不推断——同值由内核填，本层不约束） | 参数化迭代（2 形态） |
| `SessionInjections.ask` | 异常 | 非法线值拒绝：`{"ask":"genericFlow"}` / `{"ask":1}` / `{"ask":{}}` / `{"ask":["generic"]}` → 反序列化 `Err`（闭合枚举，无宽松承接） | 参数化迭代（4 形态） |
| `AskMode` | 异常 | 派生面自证（AC-9）：`runner.rs` 源码中 `AskMode` 的 `#[derive(...)]` 不含 `specta::Type`；`SessionInjections` 的 derive 面亦不含（引擎侧会话词汇不出线 IPC，反向锚定见 `bindings/mod_test.rs` 章节） | 单次 |
| `SessionInjections.ask` | 边界 | 缺省与显式 null：旧线 JSON 无 `ask` 键 → 读出 `None`；`{"ask":null}` → `None`（`#[serde(default)]` + `Option` 双承接） | 参数化迭代（2 形态） |
| `SessionInjections` | 边界 | `Default::default().ask == None`：fail-closed 语义锚定（R8——装配层 MUST 显式置位，走 `default()` 即静默禁用） | 单次 |
| `SessionInjections` | 边界 | 两位独立可辨（AC-4 的类型侧证）：`(ask, finish)` 的 `Some×Some` / `Some×None` / `None×Some` / `None×None` 四组合各可构造且两两 `!=` | 组合迭代（4 组合） |
| `SessionInjections` | 边界 | `tools` 三形态并存：`None` / `Some(vec![])` / `Some(七件名)` 分别与 `ask = Some(Generic)` 组合 → round-trip 逐项保真（`Some([])` 不被压成 `None`） | 参数化迭代（3 形态） |
| 编译闭合（`SessionOpen` / `SessionInjections` 字面构造） | ↔ 编译闭合 | `SessionOpen` 19 处补 `session_id`（`kernel.rs` 1 生产 / `runner_test` 2 / `compose_test` 3 / `lib_test` 6 / `sdk/config_test` 1 / `sdk/runner_test` 6）；`SessionInjections` 32 处补 `ask: None`（`runner_test` 9 / `loop_test` 8 / `tools_test` 6 / `sdk/runner_test` 4 / `mount_test` 2 / `kernel_test` 1 / `compose` 1 / `mount` 1）——只补字段，零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯 serde 值类型与会话词汇；无 IO、无端口、无时钟 | 全章节 |
| 不 mock：`serde_json` | 真实序列化往返——线值与 additive 缺省行为是被测契约本体 | 全章节 |
| 源码自证装置 | `std::fs::read_to_string` + `env!("CARGO_MANIFEST_DIR")`（沿 `runner_test.rs:723` 既有先例），非 mock | 派生面自证行 |

### `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`

**既有 sidecar 改造**：`SessionKernel::new` 42 处夹具补第 4 参（每用例独立 `Arc::new(AskRegistry::default())`，**不引入全局静态**——避免测试间串表，沿 R3b 纪律）；追加两条断言（`begin_turn` 填 id / `converge` 除名）。既有停止注册表 / 流注册表 / 状态机用例零改动（回归登记）。

#### 待测功能

- `SessionKernel::new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<AskRegistry>) -> Self`（第 4 参新增）: 注册表由组合根注入，MUST NOT 内部自铸（`Arc` 共享判据在 compose 章节的组合行）。

<!-- design.md「公共函数 / API」表对本文件只声明上面一行；`RunningTurn` 携 `Arc` 与 `converge` 内 `remove`、`begin_turn` 填 `SessionOpen.session_id` 三条取自「修改文件」表该文件行（私有面，经 `new` 与 `drive` 公共面覆盖）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionKernel::new` / `begin_turn` | 正向 | 新会话 id 下传（AC-1 的一半）：`SessionRef::New` 发起 → 假 runner 捕获的 `SessionOpen.session_id` 等于 `RunningTurn.session_id`（core 铸 id，非 `sdk-` 引擎侧形态） | 单次 |
| `SessionKernel::new` / `begin_turn` | 正向 | Continue 形态：`SessionRef::Continue { id }` → `SessionOpen.session_id == id`（沿用既有行，同会话同 id） | 单次 |
| `converge`（经 `drive`） | 正向 | 除名兜底（AC-5）：轮中预置 `ask_registry.register(session_id, tx)`（模拟引擎侧执行体登记）→ 驱动至收敛 → 其后 `answer(session_id, …)` → `Err`；同时断言 `StopRegistry` 该键亦已除名（两表同一出口，R7） | 组合迭代（2 收敛形态：Completed / Stopped） |
| `SessionKernel::new` | 异常 | 四启动失败臂不污染注册表：`open_session` Err / `create_session` Err / `next_seq` Err / `begin_turn` Err → 返回 `AgentStartError`，`answer(<任何 id>)` 仍 `Err`、无 panic | 参数化迭代（4 形态） |
| `converge` | 异常 | 观察流异常终止（无 `TurnDone`）兜底 `Failed` 路径同样除名：`close_stream` 后 `answer` → `Err` | 单次 |
| `SessionKernel::new` / `converge` | 异常 | 送达失败臂（引擎侧已终止）：提问 `send` 失败 → `Failed` 收敛且除名（既有错误文案零改动，新字段不参与） | 单次 |
| `converge` | 边界 | 只除本会话键：两会话并行登记 → 会话 A 收敛后 `answer(A)` → `Err`、`answer(B)` → `Ok`（按 core session id 隔离） | 组合迭代（2 会话 × 2 收敛序） |
| 编译闭合（`SessionKernel::new` 调用点） | ↔ 编译闭合 | 47 处补第 4 参（`kernel_test` 42 / `lib_test` 1 / `compose` 2 生产 / `compose_test` 2）——只加实参，零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `SessionSink` / `AgentRunner`（既有手写假件） | 假 sink 捕获落库调用、假 runner 捕获 `SessionOpen` 并按脚本报事件——二者是**持久面边界 + 引擎边界**，且是 `SessionKernel` 的显式注入依赖 | 全章节 |
| 不 mock：`AskRegistry` / `StopRegistry` / `SessionStreamRegistry` | 真实实例（除名与隔离是判据本体；替身化会让「两表同址兜底」自证） | 全章节 |
| 不 mock：`RunStateMachine` / `AgentEvent::stamp` | 真实实现（既有收敛语义面） | 全章节 |
| 不 mock：时钟 | 既有 `now_millis()` 只进 `started_at`，断言不依赖其值 | 全章节 |

### `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs`

**既有 sidecar 追加**出口面锚定（沿 `crate根导出面锚定_*` 既有四条的同族形态）；既有导出面用例零改动。

#### 待测功能

- 出口面（`mod ask;` + `#[cfg(test)] mod ask_test;` + `pub use ask::AskRegistry;`；`AskMode` 随 `runner` 出口）: crate 根路径 `agent::AskRegistry` / `agent::AskMode` 可达且可构造。

<!-- design.md「公共函数 / API」表未声明本文件的公共 API 变更（该表无 core/agent/lib.rs 行）；本条目取自 design「修改文件」表该文件行（「挂 mod ask + 出口 pub use ask::AskRegistry」）与 spec Module Contract「crates/core/agent/src/lib.rs（扩）| 出口」。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 出口面 | 正向 | `agent::AskRegistry` 可达：`AskRegistry::new()` / `AskRegistry::default()` 可构造；三方法签名以函数指针类型注解锚定（`register` → `bool`、`answer` → `Result<(), String>`、`remove` → `()`） | 单次 |
| 出口面 | 正向 | `agent::AskMode` 随 runner 出口可达：`Generic` / `Flow` 两变体可构造并赋给 `SessionInjections.ask`（经 crate 根导入路径完成四字段构造） | 单次 |
| 出口面 | 异常 | `mod ask;` 与 `#[cfg(test)] mod ask_test;` 双挂载在位：本文件与 `ask_test.rs` 的 `use crate::AskRegistry` 能编译即证（漏挂 `mod ask` → 模块不可见、编译失败；漏挂 `mod ask_test` → 整个新建 sidecar 不参与编译，`ask_test` 永不运行） | 单次 |
| 出口面 | 边界 | 既有九条 `mod *_test` 挂载与既有四条出口面锚定用例零改动全绿（新增一行 `mod` 不改既有出口集合） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 出口面锚定是编译期事实，无运行期依赖 | 全章节 |
| 不 mock：`AskRegistry` | 真实类型构造（签名锚定即被测面） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/ask.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/ask_test.rs`

**新建 sidecar**（`lib.rs` 加 `pub mod ask;` + `#[cfg(test)] mod ask_test;`）。两类用例：协议面（schema + 六态校验）与解耦守卫（源码自证，沿 `finish_test.rs:72` 的 `source_of` 装置先例）。

#### 待测功能

- `TOOL_NAME: &str`（常量，`"AskUserQuestion"`）: 名字单点，线值逐字；字面量在代码行单处。
- `description() -> &'static str`: ask 工具静态描述（何时问 / 单问一次 / 选项非空 / 自由文本应答如何回灌），< 300 字。
- `parameters() -> serde_json::Value`: 静态 schema（零参、无 prompt 依赖）；`question` 必填、`options` `minItems: 1`。
- `validate(args: &serde_json::Value) -> Result<AskQuestion, String>`: 纯函数校验；Err 文案自足（字段名 + 期望 + 当前值），六态覆盖。
- `AskQuestion { question: String, options: Vec<String> }`（新增类型）: 校验产物，`Debug + Clone + PartialEq + Eq`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TOOL_NAME` | 正向 | 名字单点：`assert_eq!(TOOL_NAME, "AskUserQuestion")`；源码自证——该字面量在 `ask.rs` 代码行（注释行剔除）恰一处、作者处为 `pub const TOOL_NAME` | 单次 |
| `parameters` | 正向 | schema 形状（AC-6）：`type == "object"`；`properties` 键集恰 `{question, options}`；`required == ["question","options"]`（逐字序）；`options.type == "array"` + `options.minItems == 1` + `options.items.type == "string"`；`question.type == "string"`；连调两次 `assert_eq!`（零动态段 / 纯静态） | 单次 |
| `description` | 正向 | 语义覆盖与体量：含单问语义、选项非空语义、自由文本应答语义；字符数 < 300（design 显式约束；与收口工具描述的 <500 口径不同源，各自锚定互不串用） | 单次 |
| `validate` / `AskQuestion` | 正向 | 合法入参：`{"question":"选哪个重试策略？","options":["指数退避","固定间隔"]}` → `Ok(AskQuestion { question, options })` 逐字段相等（题干与选项逐字保真、顺序不重排） | 单次 |
| `validate` | 异常 | 六态 Err（AC-6）：① `question` 缺失 ② `question` 空白（`""` / `"   "`）③ `options` 缺失 ④ `options` 非数组（`"x"` / `1` / `{}`）⑤ `options` 元素非字符串（`[1]` / `["a", null]`）⑥ `options` 空数组 `[]` → 逐一 `Err`；每条文案同时含**字段名** + **期望**（非空 / 数组 / 字符串数组）+ **当前值或形态** 三要素（「文案自足」判据） | 参数化迭代（6 形态） |
| `validate` | 异常 | 入参非对象：`json!([])` / `json!("x")` / `json!(null)` / `json!(42)` → `Err`（文案含「期望对象」类语义），零 panic（`Value` 索引不越界） | 参数化迭代（4 形态） |
| `ask.rs`（源码自证） | 异常 | 解耦守卫（AC-4）：`ask.rs` **全文**（含 doc / 注释 / 字符串——与 `finish_test` 的「只判代码行」相反，本处全文判）零 `finish` / `Finish` 字样——`use crate::finish` 与共享类型因此不可能存在 | 单次 |
| `validate` / `parameters` | 边界 | `question` 边界：`""`（Err）/ `"   "`（Err）/ `"a\nb"`（Ok，保留换行）/ `"a\0b🚀"`（Ok）/ `"字".repeat(1001)`（Ok——本层零长度门、零截断） | 参数化迭代（5 形态） |
| `validate` | 边界 | `options` 边界：单元素（最小合法 → Ok）/ `[]`（下界 → Err）/ 1000 元素（超大 → Ok 且逐项保真不截断）/ 单元素为 `""`（空串选项，本层不裁量内容 → Ok） | 参数化迭代（4 形态） |
| `validate` | 边界 | 多余字段宽容：`{"question":…, "options":[…], "extra":1}` → `Ok`；`options` 元素顺序与入参一致（不排序、不去重） | 单次 |
| `AskQuestion` | 边界 | 构造面与相等性：`AskQuestion { question: ""， options: vec![] }` 可直构（校验产物之外的结构不设约束）；`PartialEq` / `Eq` 两向可辨（题干或任一选项不同即 `!=`）；`Clone` 后逐字段相等 | 单次 |
| `parameters` | 边界 | 词汇隔离：schema 序列化后的全文零 `action` / `backtrack` / `stop` 命中——本模块是 ask 单源，MUST NOT 复用收口工具的动作枚举 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是纯函数 + `serde_json` 字面量；无 IO、无时钟、无注入依赖 | 全章节 |
| 不 mock：`serde_json` | 真实构造与读取（schema 形状是契约本体；替身化会让 `minItems` / `required` 断言自证） | 全章节 |
| 源码自证装置 | `std::fs::read_to_string` + `env!("CARGO_MANIFEST_DIR")` 读 `src/ask.rs` 全文（沿 `finish_test.rs:72` / `prompt_test.rs:1268` 先例），非 mock | 名字单点行、解耦守卫行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/ask_test.rs`

**出口面归属说明**：本 crate **无 `lib_test.rs` 惯例**（既有十二条 `mod *_test` 挂载里没有出口面 sidecar），故 design 声明的出口面（`pub mod ask;` + `pub use ask::{description, parameters, validate, AskQuestion, TOOL_NAME}` + `pub use port::AskPort`）的锚定用例落在**同 crate 的 `ask_test.rs`**（它是本变更新建的唯一 crate 根导入口使用方）。这使「出口一行不写」变成 `ask_test.rs` 的编译失败，而不是需要额外 sidecar。

#### 待测功能

- crate 根出口（`lib.rs` 两行变更）: 模块声明 `ask`（`pub mod`）+ 两条 `pub use`（`ask::{description, parameters, validate, AskQuestion, TOOL_NAME}` 与 `port::AskPort`）——`crate::{description, parameters, validate, AskQuestion, TOOL_NAME, AskPort}` 六名可达。

<!-- design.md「公共函数 / API」表未声明本文件的行（该表逐条列 ask.rs / port.rs 的函数，出口行只出现在「修改文件」表 lib.rs 行）；本条目取自「修改文件」表该文件行与 spec Module Contract「crates/core/orchestration/src/lib.rs」（出口面）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| crate 根出口（`lib.rs`） | 正向 | 六名可达：`ask_test.rs` 以 `use crate::{description, parameters, validate, AskQuestion, TOOL_NAME};` 与 `use crate::AskPort;` 导入后可直接调用（漏 `pub use` 即编译失败）；`AskQuestion` 可构造且字段可读 | 单次 |
| crate 根出口（`lib.rs`） | 正向 | 与 `ask::` 路径同源：`crate::TOOL_NAME == crate::ask::TOOL_NAME`、`crate::parameters() == crate::ask::parameters()`（再导出不是第二份实现——同一函数的两种路径） | 单次 |
| crate 根出口（`lib.rs`） | 异常 | `AskPort` 与既有 port 出口并存：`crate::AskPort` 与 `crate::RunEventSink` / `crate::WorkerAgentPort` 等八条既有再导出同时可达（新增一行 `pub use` 不遮蔽既有名）；`AskQuestion` 与 `AskPayload` 两名字不冲突（`ask` 模块的类型 vs `state` 的类型） | 单次 |
| crate 根出口（`lib.rs`） | 边界 | 既有十二个 `pub use` 面零改动：既有 `ChangeFlowControl` / `DecisionAction` / `DecisionInput` / `FinishPayload` / `StoreSnapshot` / `RunNotice` / `RunUpdate` / `AskPayload` 等名全部仍可达且指向原定义（再导出面扩位不改既有映射） | 参数化迭代（既有名全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 出口面是编译期事实，无运行期依赖 | 全章节 |
| 不 mock：`ask::*` / `port::AskPort` | 真实实现（「再导出不是第二份实现」需要真实函数调用对拍） | 同源行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs`

**既有 sidecar 追加** `AskPort` 实现面用例组；既有五假 port / `ToolCommand` 描述用例零改动（回归登记）。

#### 待测功能

- `AskPort::ask_started(&self, ask: AskPayload)`: 「ask 已开始」广播（携 `question` / `options` / `tool_call_id` / `session_id` 四字段，载荷完整不丢）。
- `AskPort::ask_resolved(&self)`: 「本条 ask 结束」信号（无载荷——解除信号只表达「结束」，不携带第二次真相）。

<!-- design.md「公共函数 / API」表对本文件声明上述两行（新增 trait 方法）；`Send + Sync` 超 trait 与实现驻壳层的约束见「类型定义」表 AskPort 行。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskPort::ask_started` / `ask_resolved` | 正向 | 替身实现可注入：手写假 port（`Mutex<Vec<…>>` 记录）实现两方法并在 `Arc<dyn AskPort>` 位注入 → 调用后记录逐条可读（trait 对象可用、两方法签名可直接调用） | 单次 |
| `AskPort::ask_started` | 正向 | 载荷完整性（AC-2）：传入 `AskPayload { question, options, tool_call_id: Some("call_1"), session_id: Some("core-1") }` → 替身记录的四字段逐字段相等（零裁剪、零默认填充） | 单次 |
| `AskPort::ask_started` / `ask_resolved` | 异常 | 调用序可观测：`ask_started` → `ask_resolved` 的调用序由替身记录为 `["started","resolved"]`；`ask_resolved` 无载荷（连续两次调用产生两条 `"resolved"` 记录——trait 不幂等，幂等由实现方负责） | 单次 |
| `AskPort`（超 trait） | 边界 | `Send + Sync` 可得：`fn assert_send_sync<T: Send + Sync + ?Sized>() {}` + `assert_send_sync::<dyn AskPort>()`（`Arc<dyn AskPort>` 跨任务传递的类型前提） | 单次 |
| `AskPort::ask_started` | 边界 | 载荷边界直构：`question` / `options` 取 `""` / `[]` / 1000 元素；`tool_call_id` / `session_id` 取 `None` / `Some("")` / `Some(<超长>)` → 替身记录逐字保真、零 panic（trait 层纯数据载体，不校验） | 组合迭代（4 × 2 形态） |
| 既有 port 面 | 异常（零改动回归） | AC-8 侧证：`RunEventSink` / `WorkerAgentPort` / `ToolStepPort` 等八条既有 trait 的实现面与 `ToolCommand` 描述用例零断言改动全绿（新增 trait 不扰动既有词汇） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（替身即被测面） | `AskPort` 是 trait，用例自带手写实现——这是「trait 形状 + 对象安全性」的唯一测试途径，不是对产品实现的替身 | 全章节 |
| 不 mock：`AskPayload` | 真实结构体直构（字段面由编译器强制：缺 `tool_call_id` / `session_id` 即 `E0063`） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs`

**既有 sidecar 新增 + 字面构造随动**（3 处 `RunUpdate::Ask` 随变体收敛改写、1 处 `AskPayload` 补两字段）。既有 `ChangeStepState` / `node_id` / 快照用例零改动。


<!-- 本文件的公共面变更未进 design「公共函数 / API」表（该表无 state.rs 行）；上述两条取自「类型定义」表的两行（`AskPayload` / `RunUpdate`）与「修改文件」表该文件行（「`AskPayload` 增两 additive 字段；`RunUpdate::Ask` 收敛为单载荷」）。 -->
#### 待测功能

- `AskPayload` 的两新字段 `tool_call_id` / `session_id`（皆 `Option<String>`，`#[serde(default)]`）: 缺省读 `None`、出线为 `null`；旧线载荷可读。
- `RunUpdate::Ask { ask: AskPayload }`（变体收敛）: 载荷单源；`kind() == "ask"`；`From<&RunUpdate> for RunNotice` 投影仍为 `RunNotice::Ask`（六变体与 `kind()` 逐字不变）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskPayload` 的 `tool_call_id` / `session_id` | 正向 | 四字段往返（AC-9）：`{question, options, tool_call_id: Some("call_1"), session_id: Some("core-1")}` → serialize → deserialize 逐字段相等；camelCase 线键恰为 `question` / `options` / `toolCallId` / `sessionId` | 单次 |
| `RunUpdate::Ask` | 正向 | 单源形态（AC-9）：`RunUpdate::Ask { ask }` 可构造，`kind() == "ask"`；`RunNotice::from(&update) == RunNotice::Ask`（投影逐字不变）；同时断「变体字段里无第二份 `question` / `options`」——由构造面签名强制（变体只有 `ask` 一个字段，写第二份即 `E0560`） | 单次 |
| `RunUpdate::Ask` / `AskPayload` | 正向 | 六变体与通知投影零变（回归）：`RunUpdate` 六变体的 `kind()` 逐字 `["step","edge","sessionEvent","ask","confirmWait","finished"]`；`RunNotice` 六变体一一对应且 `kind()` 与前者同序同值；`step` / `edge` / `sessionEvent` / `confirmWait` / `finished` 五臂的 `publish` 语义断言零改动全绿 | 参数化迭代（6 变体） |
| `AskPayload` 的 `tool_call_id` / `session_id` | 异常 | 非法线值拒绝：`{"question":1,…}` / `{"toolCallId":1}` / `{"sessionId":["x"]}` → 反序列化 `Err`（`Option<String>` 不接受非字符串） | 参数化迭代（3 形态） |
| `AskPayload` | 异常 | 必填缺失：`options` 键完全缺席 → `Err`（`options` 无 `#[serde(default)]`，缺即失败）；`question` 缺席 → 同上（additive 只覆盖两新字段，绝不覆盖既有必填） | 参数化迭代（2 形态） |
| `AskPayload` 的 `tool_call_id` / `session_id` | 边界 | 旧线 additive 承接（AC-9）：`{"question":"q","options":["a"]}`（无两新键）→ 读出 `tool_call_id == None`、`session_id == None`，不报错；`{"…","toolCallId":null,"sessionId":null}` → 同 `None` | 参数化迭代（2 形态） |
| `AskPayload` 的 `tool_call_id` / `session_id` | 边界 | 字段值边界：`question` 取 `""` / `"a\n\0🚀"` / `"字".repeat(1200)`；`options` 取 `[]` / 单元素 / 1000 元素；`tool_call_id` / `session_id` 取 `Some("")` / `Some(<超长>)` → 逐字保真、零裁剪（本层纯数据载体，`minItems: 1` 的裁决在工具校验） | 组合迭代（3 × 2 形态） |
| 编译闭合（`RunUpdate::Ask` / `AskPayload` 字面构造） | ↔ 编译闭合 | `RunUpdate::Ask` 21 处改 `Ask { ask: AskPayload { … } }` 且构造处补两新字段 `None`（`control` 1 / `control_test` 7 / `state` 2 / `state_test` 3 / `walker` 2 / `walker_test` 1 / `changes/mod_test` 3 / `change_flow/mod_test` 2）；`AskPayload` 2 处（`control.rs:181` 随实现改写 / `state_test.rs:555` 补字段）——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯数据 + serde；无 IO、无时钟、无端口 | 全章节 |
| 不 mock：`serde_json` / `specta` | 真实派生与往返（出线形状是契约本体） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`

**既有 sidecar 改造**：63 处 `ChangeFlowControl::new` 加 `Arc<AskRegistry>` 实参（文件内共享夹具 `fn control() -> Arc<ChangeFlowControl>` 压平，但**每用例注册表实例独立**，R3b）；9 处 `::default()` 改为 `new(Arc::new(AskRegistry::default()))`（`Default` 派生随 DD3 删除）；4 条 `answer*` 用例按新形状重写；追加 `resolve_ask` 用例组与退役面自证。**`confirm` 侧四条用例零改动**（D6 的机械判据）。

#### 待测功能

- `ChangeFlowControl::new(ask_registry: Arc<agent::AskRegistry>) -> Self`: 唯一构造面（`Default` 派生删除）；`Arc` 共享，MUST NOT 内部自铸。
- `ChangeFlowControl::answer(&self, root: &str, change_id: &str, text: String) -> Result<(), String>`: 三段判定（无 run → `Err` 携 change id；会话槽 `None` → `Err("当前无等待中的 ask")`；`Some(id)` → **释放 `runs` 锁后**转投 `ask_registry.answer`）；零状态清理、零广播。
- `ChangeFlowControl::resolve_ask(&self, root: &str, change_id: &str)`: 停等态清理单点——`ask` 槽**无条件清空**、`status` **仅当恰为 `WaitingAsk` 时**回 `Running`、克隆 `updates` 后**锁外**广播 `RunNotice::Step`。
- `ChangeFlowControl::publish(&self, …)`（Ask 臂改写）: `RunUpdate::Ask { ask }` → `entry.ask = Some(ask)`（`status = WaitingAsk` 保留）；其余五臂（`Step` / `Edge` / `SessionEvent` / `ConfirmWait` / `Finished`）逐字不动。

<!-- design.md「公共函数 / API」表对本文件声明 `new` / `answer` / `resolve_ask` 三行 + `wait_answer` 退役行；`publish` 的 Ask 臂改写取自「修改文件」表该文件行（既有公共函数的体内变化）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeFlowControl::answer` | 正向 | 转投原文回流（AC-2）：`begin_run` + `set_session(root, id, Some("core-1"))` + `ask_registry.register("core-1", tx)` → `answer(root, id, "选项A")` → `Ok`；接收方逐字收 `"选项A"`；**命令内零副作用**：`status` / `ask` 槽 / 广播（`updates` 订阅端零新帧）三者均未变（清理搬到执行体，Y-1） | 单次 |
| `ChangeFlowControl::answer` | 正向 | 三段判定第一段（AC-8）：未 `begin_run` 的 `(root, change)` → `Err`，文案逐字含 `change "<id>" 无运行中的 run`（携 change id 可归因） | 单次 |
| `ChangeFlowControl::answer` | 正向 | 三段判定第二段：有 run 但会话槽为 `None` → `Err`，文案含「当前无等待中的 ask」；会话槽有值但注册表无登记 → 同样 `Err`（两种「有 run 无等待方」形态同文案） | 参数化迭代（2 形态） |
| `ChangeFlowControl::answer` | 正向 | 锁序（DD4）：`answer` 持 `runs` 锁期间注册表 `answer` 的实现方（同线程替身）回调读取 `runs` 不阻塞——以「转投前已释放 `runs` 锁」的可观测形式断言：`answer` 期间另一线程的 `publish` 不被阻塞（超时护栏内的并发探针） | 单次 |
| `ChangeFlowControl::resolve_ask` | 正向 | 条件清理（AC-2 / DD5）：`publish(RunUpdate::Ask { ask })` → `status == WaitingAsk` + `ask == Some(…)`；`resolve_ask(root, id)` → `status == Running`、`ask == None`、`updates` 订阅端恰收一条 `RunNotice::Step` | 单次 |
| `ChangeFlowControl::resolve_ask` | 正向 | `ask` 槽无条件清空 + 终态保护（DD5 / spec「终态不被倒灌」）：构造「`status` 已是 `Completed`、而 `ask` 槽仍被置位」的竞态窗口形态 → `resolve_ask` 后 `status` 仍 `Completed`（不被改回 `Running`）但 `ask == None`、广播仍发一条 `Step`（槽清空与状态条件两半各自独立成立） | 单次 |
| `ChangeFlowControl::resolve_ask` | 正向 | 锁外广播：`resolve_ask` 的广播不被 `runs` 锁持有阻塞（订阅端在回调里读快照不阻塞，超时护栏内完成） | 单次 |
| `ChangeFlowControl::publish`（Ask 臂） | 正向 | 载荷单源（AC-2）：`publish(RunUpdate::Ask { ask: AskPayload { question, options, tool_call_id: Some("call_1"), session_id: Some("core-1") } })` → 快照 `ChangeRunSnapshot.ask` 四字段逐字段相等、`status == WaitingAsk`；`kind()` 投影的广播仍为 `RunNotice::Ask`（不是 `Step`） | 单次 |
| `ChangeFlowControl::publish`（其余五臂） | 正向（零改动回归） | AC-8 / D6 侧证：`Step` / `Edge` / `SessionEvent` / `ConfirmWait` / `Finished` 五臂（`RunUpdate` 六变体除 `Ask` 外的全部）的快照突变与广播断言零改动全绿（Ask 臂改写不扰动其余） | 参数化迭代（5 臂） |
| `ChangeFlowControl::answer` | 异常 | 迟到 / 重复应答（AC-7）：同键连续两次 `answer` → 第一次 `Ok`、第二次 `Err`（文案含「当前无等待中的 ask」），第二次不产生第二条投递（接收方仅一次 `await` 成功） | 单次 |
| `ChangeFlowControl::answer` / `ChangeFlowControl::resolve_ask` | 异常 | 除名后路径：`end_run`（或 run 收口）后 `answer` → `Err` 携 change id；`resolve_ask` 对已除名 run → 静默无操作、不 panic（miss 幂等，与 `nodes()` / `cancelled()` 的除名纪律同型） | 参数化迭代（2 形态） |
| `control.rs`（源码自证） | 异常 | 退役面零残留（AC-8）：`control.rs` 全文（注释行剔除）零 `answer_tx`、零 `wait_answer` 字样；反向在场断言：`accept_pending` / `register_pending` / `confirm` / `wait_confirm` / `cancelled_wait` 五个标识符仍在位（逐字未改的机械判据另一半） | 单次 |
| `ChangeFlowControl::confirm` / `wait_confirm`（既有四条） | 异常（零改动回归） | D6 机械判据：`confirm后停等态解除且归档槽清空` / `confirm 无运行 run → Err` / `confirm 通道空置 → Err` / `wait_confirm` 取消置位收敛 四条断言零改动全绿——`accept_pending` 泛型模板与 locks 顺序纪律逐字未改的运行期证据 | 参数化迭代（4 情形） |
| `ChangeFlowControl::new` | 边界 | 唯一构造面（DD3）：`ChangeFlowControl::new(Arc::clone(&registry))` 后 `registry.answer` 与 `control.answer` 作用于**同一张表**（`new` 前登记的槽可被 `control.answer` 命中）——`Arc` 共享、未自铸第二张表的机械判据 | 单次 |
| `ChangeFlowControl::answer` / `ChangeFlowControl::resolve_ask` | 边界 | 键边界：`root` / `change_id` 取 `""` / `"   "` / `"a\0b🚀"` → 复合键原样使用，未 `begin_run` 时 `Err`、零 panic（命令面 blank 校验在壳层，本层不二次校验） | 参数化迭代（3 形态） |
| `ChangeFlowControl::answer` | 边界 | 正文边界（零加工）：`text` 取 `""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 接收方逐字收同一串（`answer` 不裁量应答内容） | 参数化迭代（5 形态） |
| 编译闭合（构造点 / 字面构造） | ↔ 编译闭合 | `ChangeFlowControl::new` 254 处加实参（`control_test` 63 / `walker_test` 188 / `changes/mod_test` 1 / `change_flow/mod_test` 1 / `main.rs` 1）；`::default()` 9 处改 `new(...)`；`RunUpdate::Ask` 本文件 7 处随变体收敛——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `AskRegistry` | 真实实例（每用例独立 `Arc`；`answer` 的转投语义与 take 语义是被测契约本体，替身化会让「转投」断言空转） | 全章节 |
| 不 mock：`ChangeFlowControl` / `RunGuard` | 真实实例（快照 + 广播 + 锁序是被测面） | 全章节 |
| 不 mock：`broadcast` 订阅端 | 真实 `updates.subscribe()`（`RunNotice::Step` 的条数断言需要真实广播语义） | `resolve_ask` / `publish` 行 |
| 源码自证装置 | `std::fs::read_to_string` + `env!("CARGO_MANIFEST_DIR")` 读 `src/control.rs`（沿 `finish_test.rs:72` 先例） | 退役面零残留行 |
| 不 mock：`ChangeRunSnapshot` | 真实快照读取（`snapshot_of` 形态的既有装置） | 快照断言行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs`

**既有 sidecar 改造**：decision 形态的三值断言 → 两值；追加「无 `question` / `options` property」与「description 指路 ask 工具」两组断言；追加解耦守卫的**反向半边**（`finish.rs` 的 `ask` 只出现在文案字符串）。evaluator 形态用例零改动（回归登记）。

#### 待测功能

- `parameters(spec: FinishSpec, prompt: &str) -> serde_json::Value`（decision 形态体内收窄）: `action.enum == ["backtrack","stop"]`、删除 `question` / `options` 两 property；evaluator 形态零改动。
- `description(spec: FinishSpec) -> &'static str`（decision 形态文案改写）: 两动作 + 「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」（**文案里出现工具名，非代码引用**）。
- `validate(spec, args, prompt) -> Result<FinishPayload, String>`（decision 形态体内不变）: 仍走 `validate_decision`；`FinishPayload` 与 evaluator 分支逐字不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `parameters` | 正向 | 动作封闭集两值（AC-4 / AC-12）：`parameters(Decision, …)["properties"]["action"]["enum"]` 逐字 `== json!(["backtrack","stop"])`；元素个数恰 2；集合内不含 `"ask"` | 单次 |
| `parameters` | 正向 | 保留字段面：`to` / `reason` 两 property 仍在；`required == ["action"]`（未加 `to` / `reason` 的必填）；`action.type == "string"` | 单次 |
| `description` | 正向 | 指路文案（AC-12）：decision 形态描述含 `AskUserQuestion` 字面（工具名）+ 「无法裁决」类语义；且含两动作词 `backtrack` / `stop`、**不含** `ask` 动作词形态（如 `ask（` / `/ ask` 一类枚举式表述）——「两动作 + 一句指路」的双向判据 | 单次 |
| `validate` | 正向 | 两动作仍可通过：`{"action":"backtrack","to":"proposal","reason":"r"}` → `Ok(Decision(Backtrack{..}))`；`{"action":"stop","reason":"r"}` → `Ok(Decision(Stop{..}))`（`validate_decision` 逐字未改的路径证据） | 参数化迭代（2 形态） |
| `parameters` / `description` | 异常 | 删面零残留（AC-4）：decision 形态的 `properties` 键集**恰** `{action, to, reason}`（`question` / `options` 零命中）；description 全文不含 `question + 非空 options` 一类 ask 形状复述（形状归 ask 工具 schema，本处只指路） | 单次 |
| `parameters` / `validate` | 异常 | `ask` 不再产出（AC-12）：`{"action":"ask","question":"q","options":["a"]}` → `validate` 仍可由 `validate_decision` 的 `"ask"` 臂通过（历史 notice 兼容）；但 `parameters` 的 enum 不含 `"ask"` ⇒ 模型侧不可见（schema 拒绝是契约面、`validate` 的兼容是历史面，两者并存且不矛盾） | 单次 |
| `finish.rs`（源码自证） | 异常 | 解耦反向半边（AC-4）：`finish.rs` 全文内 `ask` 字样**只出现在字符串字面量与注释**中（剔除字符串与注释后代码行零 `ask`）；且全文零 `use crate::ask` / `crate::ask::` 路径（两模块互不引用） | 单次 |
| evaluator 形态 | 异常（零改动回归） | `parameters(EvaluatorChecklist, …)` 的 `report` / `items` 形状与 `id` enum 派生断言零改动全绿；`description(EvaluatorChecklist)` 逐字不变；`FinishPayload` 两变体逐字不变 | 参数化迭代（既有用例全集） |
| `parameters` | 边界 | 两形态互不渗透：decision 形态的序列化全文零 `report` / `items` / `evidence` 命中；evaluator 形态的序列化全文零 `action` / `backtrack` / `stop` / `to` 命中 | 参数化迭代（2 形态） |
| `description` | 边界 | 体量与稳定性：decision 与 evaluator 两形态描述字符数各 < 500（既有约束）；`description(Decision)` 连调两次逐字相等（`&'static str`，零动态段） | 单次 |
| `finish_step` 的 backtrack 形态（下游 test-gen 补） | 正向 | **G3 骨架补位**：`{"action":"backtrack","to":"proposal","reason":"r"}` 走完整往返——`validate` 产出 `FinishPayload::Decision(Backtrack{ to: Proposal, reason })` 后**在 flow step 内以正常 ToolResult 收口**（`is_error == false`）；白名单内的 `to` 值集逐项可通过（`proposal` / `design` / `tasks` / 实施类 step 名），逐项断言 `Ok` 且 `to` 字段逐字回读——本行只补形态骨架，白名单越权面仍由 `ensure_backtrack_allowed` 既有用例锁定 | 参数化迭代（`allowed` 列表逐项） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯函数 + `serde_json`；无 IO、无时钟、无注入依赖 | 全章节 |
| 不 mock：`validate_decision` | 真实调用（`finish::validate` 的 decision 分支是被测面，替身化会让「仍走 validate_decision」自证） | `validate` 行 |
| 源码自证装置 | `std::fs::read_to_string` 读 `src/finish.rs`（既有 `source_of` + `code_lines` 两装置直接复用） | 解耦反向半边行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`

**既有 sidecar 保留（零删除）**：`"ask"` 臂用例逐字保留；追加 `DecisionAction::Ask` 的 doc / 生产不可达注释面自证与 `validate_decision` 的 `"ask"` 臂回归锚定。

#### 待测功能

- `DecisionAction::Ask`（保留变体，载荷 `{ question, options }`）: 生产不可达（`finish_step` 的 enum 已不含它），历史 notice 反序列化仍可读；walker 防御臂据此判漂移。
- `validate_decision(args: &serde_json::Value) -> Result<DecisionAction, String>`（`"ask"` 臂逐字保留）: 历史兼容面零改动。

<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 decision.rs 行）；上述两条取自「修改文件」表该文件行与「类型定义」表（`DecisionAction` 保留 + doc 补注）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DecisionAction::Ask` | 正向 | 保留变体可读（R5）：`{"action":"ask","question":"q","options":["a","b"]}` → `validate_decision` `Ok(Ask{question, options})` 逐字段相等（既有用例零改动） | 单次 |
| `DecisionAction::Ask` | 正向 | 反序列化兼容（历史 notice）：`serde_json::from_value::<DecisionAction>(json!({"action":"ask","question":"q","options":["a"]}))` → `Ok`（历史 `FinishPayload::Decision(Ask{..})` 仍可读） | 单次 |
| `validate_decision` | 异常 | `"ask"` 臂三个 Err 面逐字保留：`question` 缺失 / `options` 非数组 / `options` 空数组 → 三文案与改动前逐字相同（历史兼容面 MUST NOT 随本变更改文案） | 参数化迭代（3 形态） |
| `validate_decision` | 异常 | 非法动作文案未扩容：`{"action":"other"}` → `Err` 文案仍列 `["backtrack", "stop", "ask"]` 三值（**MUST NOT** 随 `finish_step` schema 收窄而删 `"ask"`——本层是历史兼容面，schema 才是模型可见面） | 单次 |
| `DecisionAction::Ask` | 异常 | 生产不可达的文档面自证：`decision.rs` 源码含「生产不可达」类注释且在 `Ask` 变体上方（doc 注释落位），同时 `finish.rs` 的 decision enum 不含 `"ask"`（跨文件一致性由两文件各自的用例承担） | 单次 |
| `ensure_backtrack_allowed` / `MAX_REASON_CHARS` | 异常（零改动回归） | 白名单校验与长度上限断言零改动全绿（AC-12 的「逐字不变」半面） | 参数化迭代（既有用例全集） |
| `DecisionAction` | 边界 | 三变体可辨：`Backtrack` / `Stop` / `Ask` 两两 `!=`；`serde` tag 逐字 `backtrack` / `stop` / `ask`（线值与 `finish_step` schema 的收窄无关——`DecisionAction` 是载荷类型，不是 schema） | 参数化迭代（3 变体） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯函数 + serde 值类型 | 全章节 |
| 不 mock：`validate_decision` | 真实调用（历史兼容面是被测本体） | 全章节 |
| 源码自证装置 | `std::fs::read_to_string` 读 `src/decision.rs`（既有装置） | 文档面自证行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs`

**既有 sidecar 改造**：「决策协议」段的三动作断言删除、两动作 + ask 指路断言追加；`decision_answer_prompt` 退役自证（既有 `两个修正prompt退役` 用例扩一项）。`evaluator_protocol` / `backtrack_option_*` / `spec_sync_prompt` / `merge_conflict_prompt` 用例零改动。

#### 待测功能

- `decision_prompt(input: &DecisionInput) -> String`（段文本改写）: 「决策协议」段两选一 + 一句「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」；其余四段（失败相位报告 / 失败清单 / 可回溯白名单 / 候选报告 / 报告文件）逐字不动。
- `decision_answer_prompt`（**删除**）: 全仓零命中（应答不再开新一轮）；源码自证。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `decision_prompt` | 正向 | 两动作在场（AC-12）：协议段含 `backtrack` 与 `stop` 两条目；条目数与「两选一」一致（不再有三动作表述）；白名单 / 候选 / 失败清单 / 报告文件四段在场且逐字与既有断言相等 | 单次 |
| `decision_prompt` | 正向 | ask 工具指路（AC-12）：全文含 `AskUserQuestion` 字面 + 「无法裁决」类语义；指路句位于协议段内（`find("决策协议") < find("AskUserQuestion")`） | 单次 |
| `decision_prompt` | 正向 | 形状不复述：指路句不复制 schema 形状（全文零 `"minItems"` / `"properties"` / `"required"` 一类 JSON Schema 词；零 `question + 非空 options` 的字段级复述）——形状归工具 schema | 单次 |
| `decision_prompt` | 正向 | 其余段零扰动（回归）：`## 失败相位报告` / `## 失败清单` / `## 可回溯白名单…` / `## 候选相位最近一次评估报告` / `## 报告文件` 五段在场且段序与既有断言逐条相等；`backtrack_option_*` 三助手产出的白名单逐行渲染（含「= 重做本相位」自注记两形态）零改动 | 参数化迭代（5 段 + 2 注记） |
| `decision_prompt` | 异常 | 三动作残留零命中（AC-12）：全文零 `ask（` / `/ ask` / `ask`（`question` + 非空 `options`）` 一类**决策动作枚举式**表述；零「三选一」/「三动作」字样 | 单次 |
| `decision_answer_prompt`（源码自证，`prompt.rs` 内） | 异常 | 退役面零残留（AC-12）：`prompt.rs` 全文零 `fn decision_answer_prompt`；反向在场：`fn decision_prompt` / `fn evaluator_protocol` 仍在位（既有用例已断两 `*_fix_prompt` 退役，本行扩一项） | 单次 |
| `evaluator_protocol` / `evaluator_prompt` | 异常（零改动回归） | 评估侧协议段与追加面逐字不变（本变更只动决策侧） | 参数化迭代（既有用例全集） |
| `decision_prompt` | 边界 | 信封插值边界：`attempt` 取 `0` / `u32::MAX`、`phase` 取 `""` / 超长串、`fail_report` 取 `""` / 超长 → 不 panic、协议段与指路句仍逐字在场 | 组合迭代（4 × 2 形态） |
| `decision_prompt` | 边界 | 白名单空集：`allowed == []` → 白名单段仍渲染（空清单占位），协议段与指路句不受影响；`backtrack` 条目仍在（白名单由词表派生，不由条目数派生） | 单次 |
| `spec_sync_prompt` / `merge_conflict_prompt` | 边界（零改动回归） | 归档两步产物零 `AskUserQuestion` 字样（archive 面不消费 ask）；文案逐字不变 | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | `decision_prompt` 是纯 `format!` 串；无 IO、无时钟、无注入依赖 | 全章节 |
| 不 mock：`workflow` crate 的 `BacktrackOption` / `ChecklistItem` | 真实类型直构（既有 `base_input()` 装置） | 全章节 |
| 源码自证装置 | `std::fs::read_to_string` 读 `src/prompt.rs`（既有装置直接复用） | 退役面零残留行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`

**既有 sidecar 改造**：188 处 `ChangeFlowControl::new` 加实参（文件内共享夹具压平，每用例注册表独立）；三条 ask 往返用例重写为「防御臂」与「单轮收口」；1 处 `RunUpdate::Ask` 字面构造随变体收敛。`step_verdict_gate*` / `feedback_limit` / `ensure_backtrack_allowed` / `WhitelistGate` 节点语义 / 四处机械 checklist 代写点用例零改动。

#### 待测功能

- `decision_session(...)`（私有，体内去循环化）: 删 `loop` 与 `continue_session`；`Some(FinishPayload::Decision(DecisionAction::Ask { .. }))` → `Terminal::failed("决策漂移：ask 应经 AskUserQuestion 工具提交")`（不 emit、不停等、不开新轮）。
- `resolve_deadlock(...)`（私有，`DecisionAction::Ask` 臂结构保留）: 仍 `Deadlock::Terminal(Terminal::failed(…))`，文案与注释改写为「决策漂移」。

<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 walker.rs 行）；上述两条取自「修改文件」表该文件行与 DD10（私有函数，经 `walk_run` 公共面覆盖）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `decision_session` | 正向 | 单轮收口零 `Continue`（AC-12）：决策步一次 `finish_step{action:"backtrack"}` → `FakeWorker` 收到的 `WorkerTurnRequest` 条数恰 1（`continue_session` 恒 `None`）、`WhitelistGate` 节点恰一个、run 终态按动作裁决（不是 `Failed`） | 单次 |
| `decision_session` | 正向 | 合法两动作各自单轮：`stop` 与 `backtrack`（白名单内）两形态 → 各恰 1 次 worker 调用、零第二轮、终态与既有断言一致 | 参数化迭代（2 形态） |
| `decision_session` | 异常 | 防御臂（AC-12）：`finalized = Some(FinishPayload::Decision(DecisionAction::Ask { question, options }))` → `Terminal::failed`，文案含「决策漂移」+ `AskUserQuestion` 字面；`RunUpdate::Ask` **零 emit**（广播订阅端零 `RunNotice::Ask` 帧）、`wait_answer` 零调用（函数已删，编译期事实）、worker 调用恰 1 次（不开新轮） | 单次 |
| `resolve_deadlock` | 异常 | 防御臂结构保留（DD10）：`DecisionAction::Ask` 输入 → `Deadlock::Terminal(Terminal::failed(…))`；文案含「决策漂移」类语义、零 panic；其余两动作的 `Deadlock` 分支断言零改动 | 参数化迭代（3 动作） |
| `decision_session` | 异常 | 既有失败臂零改动：`finalized == None` 且输出被截断 → `Terminal::failed` 携截断文案；未截断 → 「未调用 finish_step 收口」文案；`Some(Evaluator(..))` → 「相位表角色漂移」文案（三文案逐字不变） | 参数化迭代（3 形态） |
| `walker.rs`（源码自证） | 异常 | 退役面零残留（AC-8 / AC-12）：`walker.rs` 代码行（注释行剔除，沿既有 `walker.rs 源码自证入口` 装置）零 `wait_answer`、零 `decision_answer_prompt`；`decision_session` 函数体内零 `loop {` 与零 `continue;`（去循环化的机械判据） | 单次 |
| `WhitelistGate` / `ensure_backtrack_allowed` | 异常（零改动回归） | 白名单越权一次即 `Terminal::failed`（文案携 `allowed`）、`WhitelistGate` 每轮一节点的图面语义、`step_verdict_gate*` 与四处机械 checklist 代写点断言零改动全绿（AC-12 与 AC-8 的「逐字保留」半面） | 参数化迭代（既有用例全集） |
| `decision_session` | 边界 | 白名单两形态：`allowed == []`（空白名单）与单元素白名单 → `backtrack` 臂分别 `Terminal::failed`（越权）/ 通过；决策 prompt 的白名单段与 ask 指路句两形态均在场 | 参数化迭代（2 形态） |
| `decision_session` | 边界 | 决策输入边界：`attempt` 取 `0` / `u32::MAX`、`phase` 取 `""` → 单轮收口语义不变、零 panic；`fail_checklist` 空集 / 1000 项两形态的 prompt 组装不阻断裁决 | 组合迭代（2 × 2 形态） |
| 编译闭合（构造点） | ↔ 编译闭合 | `ChangeFlowControl::new` 188 处加实参、`RunUpdate::Ask` 1 处随变体收敛——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkerAgentPort` / `ToolStepPort`（既有 `FakeWorker` / `FakeTools`） | 按调用序弹出预录产出并捕获 `WorkerTurnRequest`（调用条数断言面）——二者是**进程边界**（agent 会话 / 工具步）且是 `walk_run` 的显式注入依赖 | 全章节 |
| `WorkflowSnapshotPort`（既有 `StubSnapshot` / 真实 `StoreSnapshot` + `TestDb`） | 快照是 DB 边界；单轮收口用例用 `StubSnapshot`，落账面用例用 `TestDb` | 全章节 |
| `ChangeFlowControl` / `AskRegistry` | 真实实例（广播帧数与快照是被测面；`AskRegistry` 每用例独立） | 全章节 |
| 不 mock：`decision_prompt` / `evaluator_prompt` | 真实组装（prompt 段序与指路句是被测契约的一部分） | 决策链行 |
| 源码自证装置 | `std::fs::read_to_string` + 注释行剔除（既有 `walker.rs 源码自证入口`） | 退役面零残留行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`

**既有 sidecar 改造**：两处夹具（`injected` / `injected_finish`）补 `ask: None`（编译闭合）；追加 ask 追加分支与四组合用例组、无合取自证。既有十项清单 / `lookup` / `finish` 追加序用例零改动（回归登记）。

#### 待测功能

- `ToolExec::Ask(agent::AskMode)`（第五臂）: `Copy + Eq` 保持；挂载项执行体分类。
- `definitions(injections: &agent::SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>`（加 ask 臂，签名不变）: `injections.ask` 为 `Some(mode)` 时追加 `AskUserQuestion`（`description` / `parameters` 取自 `orchestration::ask`，`tier = ToolTier::Protocol`，`exec = Ask(mode)`）；与 `finish` 位**两段独立 `if`**（DD14 序：ask 先、finish 后）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `definitions` | 正向 | `Some(Generic)` 追加（AC-1 / AC-6）：`tools: Some([七件名])` + `ask: Some(Generic)` → 挂载项数 = 七 + 1；末项 `definition.name == "AskUserQuestion"`、`tier == ToolTier::Protocol`、`exec == ToolExec::Ask(Generic)`；`description` / `parameters` 与 `orchestration::ask::{description, parameters}` **逐字节相等**（同名覆盖的定义单源判据） | 单次 |
| `definitions` | 正向 | `Some(Flow)` 档位可辨（AC-2）：同上但 `ask: Some(Flow)` → 名称 / 描述 / schema 三项与 `Generic` 形态**逐字相同**（模型视角零差异），唯一差异是 `exec == ToolExec::Ask(Flow)` | 单次 |
| `definitions` | 正向 | 追加序（DD14）：`ask: Some(Generic)` + `finish: Some(Decision)` → 名单尾两项依次 `AskUserQuestion`、`finish_step`（ask 先、finish 保持末位）；既有「finish 追加在末尾」断言零改动仍成立 | 单次 |
| `definitions` | 正向 | 名字清单不收窄 ask：`tools: Some([六件名])`（explore 面）+ `ask: Some(Generic)` → ask 仍追加（ask 是**按位追加的基线工具**，不占名字清单、不被清单过滤碰到） | 单次 |
| `definitions` | 异常 | 四组合独立（AC-3 / AC-4，核心判据）：`(ask, finish)` 四组合 → `(Some, Some)` 两项都在；`(Some, None)` 只有 `AskUserQuestion`（无 `finish_step`）；`(None, Some)` 只有 `finish_step`（无 `AskUserQuestion`）；`(None, None)` 两项都不在——四组逐一断言名单投影，证两分支无合取判据 | 组合迭代（4 组合） |
| `definitions` | 异常 | `ask: None` 零追加（AC-3）：七件名清单 + `ask: None` → 挂载项数恰 7、名单内零 `AskUserQuestion`（`RunStarted.tools` 随该名单派生，硬禁用面成立） | 单次 |
| `mount.rs`（源码自证） | 异常 | 无合取自证（AC-4）：`mount.rs` 代码行内零 `ask.is_some() &&` / `&& injections.ask` 一类合取形态；且两处 `if` 分别只读 `injections.ask` 与 `injections.finish`（互不引用） | 单次 |
| `mount.rs`（源码自证） | 异常 | 工具名字面量单点（AC-4 延伸）：`mount.rs` 全文零 `"AskUserQuestion"` 字面量（经 `orchestration::ask::TOOL_NAME` 引用）——与既有 `finish_step` 字面量零命中断言同族；`TOOL_NAMES` 常量内亦不含 `AskUserQuestion`（它不是七件之一） | 单次 |
| `definitions` | 边界 | `tools` 三形态 × ask 两位（组合）：`tools: None`（内建七件）/ `Some(vec![])`（空清单）/ `Some(含 archi 名)` 分别配 `ask: None` / `Some(Generic)` → 七件底座形态零改动；ask 追加与 `tools` 形态无关（唯一依赖 = `ask` 位） | 组合迭代（3 × 2 组合） |
| `definitions` | 边界 | `prompt` 无关性：`ask: Some(_)` 时 `parameters()` 不依赖 `prompt` 文本（`prompt: ""` 与 `prompt: <长清单>` 两形态产出的 schema 逐字节相等）；对照臂 = `finish: Some(EvaluatorChecklist)` 的 schema 随 `prompt` 变化（证明本行不是空转） | 参数化迭代（2 形态） |
| `ToolExec` | 边界 | 五臂可辨：`Pure` / `ExploreNote` / `Archi` / `FinishStep(spec)` / `Ask(mode)` 五值两两 `!=`；`Ask(Generic) != Ask(Flow)`（档位参与 `Eq`） | 参数化迭代（5 臂） |
| 编译闭合（夹具） | ↔ 编译闭合 | `injected` / `injected_finish` 补 `ask: None` 字段（`mount.rs` 1 处）+ `mount_test.rs` 2 处字面构造——只补字段，零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | `registry()` / `definitions` / `lookup` 全为进程内纯函数（`OnceLock` 一次性静态表，无 IO / 无 spawn / 无网络） | 全章节 |
| 不 mock：`orchestration::ask::{description, parameters, TOOL_NAME}` | 真实调用（铸造内容即被测面——同名覆盖的「定义逐字节相等」判据只能由真实调用成立） | 定义单源两行 |
| 不 mock：`orchestration::finish::{description, parameters}` | 真实调用（四组合的 `finish` 半边与追加序断言需要真实 schema） | 四组合 / 追加序行 |
| `prompt` 语料 | 内存合成静态清单字符串（既有 `prompt_of` 装置），非 mock | `prompt` 无关性行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`

**既有 sidecar 新增 + 改造**：`loop_turn` 助手补 `ask: None` 一处（其余约 39 处直构点经 `..loop_turn(...)` 兜底零改动）；8 处 `SessionInjections` 字面构造补 `ask` 字段；追加两档执行体用例组。既有防线 / 重试 / 收口 / 截断 / `pause_turn` / `web_search` / 成员校验用例零改动。

#### 待测功能

- `LoopTurn.ask: Option<AskContext>`（新增字段）: `AskContext { registry: Arc<AskRegistry>, core_session_id: String, port: Option<Arc<dyn AskPort>> }`——`core_session_id` 是**注册表键**（与 `LoopTurn.session_id` 的引擎侧 `sdk-…` id 同名不同义，MUST NOT 复用后者）。
- `ToolExec::Ask(Generic)` 执行臂: `validate` → `register` → `select!(oneshot, handle.wait_requested())` → 返回应答文本作为 tool result（无编排广播）。
- `ToolExec::Ask(Flow)` 执行臂: `ask_started`（携 `tool_call_id` / `session_id`）→ `register` → `select!` → **两臂返回后都** `ask_resolved`（恰一次）；`port` 缺席 → `is_error` 回灌（DD2 防御臂）。
- 上下文缺席与登记语义（`AskContext` 缺席 → `is_error`「ask 执行上下文未装配」类文案、零 panic；`AskRegistry::register` 的返回值不构成失败面，DD1）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ToolExec::Ask(Generic)` | 正向 | 阻塞 → 回灌 → 同 turn 续跑（AC-1）：脚本化流缝「tool_call `AskUserQuestion` → 第二请求 → 文本收口」；主线程应答注册表 `answer(core_session_id, "选项A")` → 该 `tool_use` 的 `ToolResult{content:"选项A", is_error:false}` 回灌、同一 `turn_index` 内续跑、`RunStarted.session_id` 恒定（零新会话）；**模型请求计数恰为 2 且第二次请求发生在应答之后**（阻塞期间零新请求——`FakeModel` 的请求序即判据） | 单次 |
| `ToolExec::Ask(Generic)` | 正向 | 同名工具双档 schema 同源（AC-2 的模型视角）：`Generic` 与 `Flow` 两档下发的 `RunStarted.tools` 名单逐名相等、工具定义逐字节相等（仅执行体不同） | 单次 |
| `ToolExec::Ask(Flow)` | 正向 | 序列与次数（AC-2）：假 port 记录调用序 → 恰 `["started","resolved"]`；`started` 载荷的 `tool_call_id == 本次 tool_use 的 id`、`session_id == AskContext.core_session_id`（**不是** `LoopTurn.session_id`）、`question` / `options` 与入参逐字相等 | 单次 |
| `ToolExec::Ask(Flow)` | 正向 | 应答后回灌与续跑：`Flow` 档应答 `"固定间隔"` → tool result 逐字回灌、同 turn 续跑；`ask_resolved` 在回灌**之前**已调用（清理先于协议回灌的序） | 单次 |
| `ToolExec::Ask(Flow)` | 正向 | 停止臂也清理（AC-5 / DD2 的必要性）：`handle.request_stop()` 置位 → 执行体返回 `Err`（「用户未应答（会话被终止）」类文案，`is_error: true`）且 `ask_resolved` **仍执行恰一次** | 单次 |
| `ToolExec::Ask(Flow)` | 异常 | `port` 缺席（DD2 防御臂）：`ask: Some(Flow)` 但 `AskContext.port == None` → 该调用 `is_error` 回灌（文案含「ask 编排状态面未装配」）；假 port 零调用、注册表零登记（其后 `answer` → `Err`）、run 不中断 | 单次 |
| `LoopTurn.ask` / `ToolExec::Ask(*)` | 异常 | 上下文缺席（字段面判据）：`injections.ask = Some(Generic)` 但 `LoopTurn.ask == None`（装配漂移）→ 该 `AskUserQuestion` 调用以 `is_error` 回灌（「ask 执行上下文未装配」类文案）、run 不中断、零 panic；注册表零登记 | 单次 |
| `ToolExec::Ask(*)` | 异常 | `validate` Err 当场回灌：`{"question":"","options":[]}` → `is_error` tool result 携 `ask` 模块的 Err 文案（字段名 + 期望）；注册表零登记（校验先于登记）、run 不中断、同 turn 内可重试 | 单次 |
| `ToolExec::Ask(Generic)` | 异常 | 单次回灌（AC-7）：应答一次后再次 `answer(core_session_id, …)` → `Err`（注册表已被 take）；转录内该 `tool_use` 恰一条 `tool_result`（零第二条） | 单次 |
| `ToolExec::Ask(Flow)` | 异常 | `ask_started` 先于阻塞：应答方在 `ask_started` 回调里就能读到完整载荷并立刻 `answer`（回调内同步投递）→ 执行体不悬挂、序列仍 `["started","resolved"]`（广播先于阻塞的时序判据） | 单次 |
| `ToolExec::Ask(*)` | 边界 | 应答正文边界：`""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 逐字进 tool result（`is_error: false`，零 trim 零截断） | 参数化迭代（5 形态） |
| `ToolExec::Ask(*)` | 边界 | `options` 边界：单元素 / 1000 元素 → 均进 `ask_started` 载荷并逐项保真（`Generic` 档无 port，只断言回灌不受影响） | 参数化迭代（2 形态） |
| `ToolExec::Ask(*)` | 边界 | 同批多工具共存：`read` + `AskUserQuestion` 同批两调用 → 结果收进**恰一条** user 消息（`history` 末条逐条覆盖，既有配对协议零破坏）；`Ask` 臂不打断同批其它执行体 | 单次 |
| `ToolExec::Ask(Generic)` | 边界 | `ask: None` 的硬禁用（AC-3，既有机制零改动回归）：`ask: None` 会话中模型硬调 `AskUserQuestion` → `tool_unavailable`（`SystemNotice` + is_error ToolResult）、注册表零登记、run 不中断 | 单次 |
| 编译闭合（`SessionInjections` / `LoopTurn` 字面构造） | ↔ 编译闭合 | `SessionInjections` 8 处补 `ask` 字段；`LoopTurn` 助手 `loop_turn` 内补 `ask: None` 一处（其余约 39 处经 `..loop_turn(...)` 零改动）——只补字段，零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `rig` 流（既有 `FakeModel` / `MockStreamEvent` 脚本化流缝） | 按请求序脚本化产出 `raw_tool_call` / `raw_text` / `raw_final`——这是 **provider 网络边界**，且是 `loop::run` 的显式入参 | 全章节 |
| `AskPort`（手写假 port） | 记录两方法调用序与载荷——`AskPort` 是**显式注入依赖**（`AskContext.port`），且「`ask_resolved` 恰一次」这条判据只能由调用计数观测 | `Flow` 全行 |
| `AskRegistry` | 真实实例（take 语义与登记键是判据本体；替身化会让「键 = core session id」自证） | 全章节 |
| `protect` / `sandbox` / `policy` | 真实调用（既有守卫链零改动，本变更不触碰；用例只保证 ask 臂不绕过它们） | 全章节 |
| 不 mock：`append_engine_log` | 真实调用（尽力而为的日志面；既有实现落临时目录，断言不依赖其内容） | 全章节 |
| 不 mock：`RunHandle` | 真实实例（停止臂判据需要真实 `wait_requested()` 唤醒） | 停止臂行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`

**既有 sidecar 新增 + 改造**：6 处 `SessionOpen` 字面构造补 `session_id`（编译闭合）；追加 per-turn 铸造用例组与两条 builder 缝用例。

#### 待测功能

- `SdkRunner::with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self`（新增 builder 缝）: 与 `with_explore_note_store` 同型；`new` 签名不动。
- `SdkRunner::with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self`（新增 builder 缝）: 仅 `Flow` 档消费。
- `session_pump` 的 per-turn 铸造（消费 `PumpPayload` 两新字段 `ask_registry` / `ask_port`）: 按 `open.injections.ask` 位铸 `AskContext`（`core_session_id = open.session_id.clone()`；`Flow` 档带 `ask_port`；`Generic` 档 `port = None`），`None` 位不铸。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `session_pump` 的 per-turn 铸造 | 正向 | `Some(Generic)` 铸上下文（AC-1）：`SessionOpen { injections.ask: Some(Generic), session_id: "core-session-1" }` → 泵内 `LoopTurn.ask` 为 `Some`，且 `core_session_id == "core-session-1"`、`port == None`；与 `LoopTurn.session_id`（`sdk-` 形态）**不等**（两 id 隔离的判据） | 单次 |
| `session_pump` 的 per-turn 铸造 | 正向 | `Some(Flow)` 带 port（AC-2）：`ask: Some(Flow)` + `with_ask_port(Some(fake_port))` → `LoopTurn.ask.port` 为 `Some`（同一 `Arc`：指针相等或行为等价——以假 port 收到的调用为证） | 单次 |
| `session_pump` 的 per-turn 铸造 | 异常 | `None` 位不铸（AC-3）：`ask: None` → `LoopTurn.ask == None`；模型即便硬调该名也被成员校验拒绝（既有机制） | 单次 |
| `session_pump` 的 per-turn 铸造 | 异常 | `Flow` 无 port（DD2 降级）：`ask: Some(Flow)` 且未注入 `ask_port` → 上下文在场但 `port == None` → 该工具调用 `is_error` 回灌（与 loop 章节的 DD2 行构成两级判据：本行证铸造形态、loop 行证执行体行为） | 单次 |
| `with_ask_registry` / `with_ask_port` | 正向 | builder 链式构造（`with_context_window` 先例）：`.with_ask_registry(reg).with_ask_port(Some(port))` 后 open 成功，且注册表在会话侧仍可 `answer`（同一 `Arc` 未被克隆出新表——注册后 `answer` 命中即证） | 单次 |
| `with_ask_registry` / `with_ask_port` | 正向 | 缺省形态（空表兜底）：未调两条 builder 的 `SdkRunner` 仍可 open 并铸上下文（按 design，注册表是组合根注入的非 `Option` `Arc`，`new` 内给空表）；该形态下 `answer(<任何 id>, …)` 恒 `Err`、无 panic（未注入 ≠ 装配失败，只是无等待方） | 单次 |
| `SdkRunner` | 边界 | 注册表独立性：两 `SdkRunner` 实例各持不同 `Arc` → A 的表登记不影响 B（`answer` 互不命中）；同一 `Arc` 注入两实例 → 共享（登记后另一实例可见） | 组合迭代（2 形态） |
| 编译闭合（`SessionOpen` 字面构造） | ↔ 编译闭合 | `sdk/runner_test.rs` 6 处 `SessionOpen` 补 `session_id`（测试侧给固定串如 `"core-session-1"`）；`SessionInjections` 4 处补 `ask`——只补字段，零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `EngineConfig` / provider 端点 | 既有 `complete_config` 指向本机 discard 端口（恒拒连，零网络触达）——这是**网络边界** | 全章节 |
| `ResumeTranscript`（既有 `counting_loader` 闭包替身） | 装载缝显式注入依赖；调用计数即断言面 | 全章节 |
| `AskPort`（手写假 port） | 显式注入依赖（`with_ask_port` 入参），调用可记录 | `Flow` 铸造行 |
| 不 mock：`AskRegistry` | 真实实例（`Arc` 共享判据需要真实表） | 全章节 |
| 不 mock：`session_pump` | 真实泵任务（per-turn 铸造点是泵内单点；直测该函数形态即契约） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs`

**既有 sidecar 追加**：两条 `EngineFacade` builder 缝的链式构造与分发行为断言（沿既有 `with_context_window_builder链式构造且分发行为与直构一致` 先例）；既有四条出口面锚定用例零改动。

#### 待测功能

- `EngineFacade::with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self`（新增 builder 缝）: 门面 → runner 单一通道。
- `EngineFacade::with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self`（新增 builder 缝）: 同上。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_ask_registry` | 正向 | 链式构造与分发一致：`EngineFacade::new().with_ask_registry(reg)` 构造后 `runner_for` 产出的 `SdkRunner` 持同一 `Arc`（登记后 `answer` 命中）；与直构 `SdkRunner::new(...).with_ask_registry(reg)` 行为等价（沿既有 `with_context_window` 用例形态） | 单次 |
| `with_ask_port` | 正向 | 可选位两形态：`with_ask_port(Some(port))` 与 `with_ask_port(None)` 均可构造；`Some` 形态在 `Flow` 会话里可被假 port 观测到调用，`None` 形态零调用 | 参数化迭代（2 形态） |
| `with_ask_registry` / `with_ask_port` | 异常 | 缺省形态零漂移（回归）：未调两条 builder 的既有 open / 配置校验 / 续会话用例零改动全绿（`EngineFacade::new()` 的字段面扩张不改变既有行为） | 单次 |
| `with_ask_registry` | 边界 | 与既有六条 builder 并存：`.with_resume_transcript(..).with_context_window(..).with_max_tokens(..).with_web_search(..).with_explore_note_store(..).with_workspace_root(..).with_ask_registry(..).with_ask_port(..)` 全链后各字段互不覆盖（逐字段可辨：以行为观测替代字段读取） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `store::Store` / `WorkspaceStores`（既有 `temp_store` + `tempfile`） | 真实临时库（文件系统边界，由 `tempfile` RAII 隔离） | 全章节 |
| `AskPort`（手写假 port） | 显式注入依赖 | `Some` 形态行 |
| 不 mock：`EngineFacade` / `SdkRunner` | 真实构造（builder 链是被测面） | 全章节 |
| 不 mock：`AskRegistry` | 真实实例 | 全章节 |

### `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`

**既有 sidecar 改造**：36 处 `compose_turn` 调用加两实参（`Arc::new(AskRegistry::default())` + `None`）；1 处 `SessionInjections` 字面构造随四臂重写；2 处 `composed_with_runner` 夹具补注册表；2 处 `SessionKernel::new` 补第 4 参；3 处 `SessionOpen` 补 `session_id`。追加逐臂 `ask` 位断言与组合用例组（**装配链入口**）。

#### 待测功能

- `compose_turn(stores, registry, streams, ask_registry, store, agent, workspace_root, ask_port) -> Result<ComposedTurn, String>`（加两参）: 注册表透传内核 + 门面；`ask_port` 仅门面（编排侧状态面实现方是壳层 sink）。
- `assemble_injections(provenance) -> Result<agent::SessionInjections, String>`（四臂显式置位）: explore / archi / 其余来源 → `Some(Generic)`；change（任意 role）→ `Some(Flow)`；**MUST NOT** `SessionInjections::default()`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `assemble_injections` | 正向 | explore 臂（AC-1）：`source="explore"` → `ask == Some(Generic)`；`tools` 清单逐名不变（六件 + `create_explore_note`）；名字清单内不含 `AskUserQuestion`（ask 不占清单） | 单次 |
| `assemble_injections` | 正向 | archi 臂：`source="archi"` → `ask == Some(Generic)`；`tools` 九名 + `preamble` 在场；`source_ref` 取任意值不改变该臂（含 `Some("<change_id>/dev-design/executor/1")`） | 参数化迭代（2 形态 source_ref） |
| `assemble_injections` | 正向 | change 臂（任意 role）（AC-2）：`source="change"` + `source_ref` 取 executor / evaluator / decision 三种 role 与畸形源（段数不足 / 空串）→ 全部 `ask == Some(Flow)`（**不分 role / 不分矩阵命中**）；`finish` 位按 role 各自置位（`evaluator` → `EvaluatorChecklist`、`decision` → `Decision`、其余 `None`） | 组合迭代（4 源形态 × 3 role） |
| `assemble_injections` | 正向 | 其余来源臂显式 Generic（AC-1 / R8）：`source="debug"` 与「不传来源」（命令层缺省 `debug`）→ `ask == Some(Generic)`、`tools == None`、`preamble == None`、`finish == None`；**不是** `SessionInjections::default()`（`default().ask == None`，两者 `!=`） | 参数化迭代（2 形态） |
| `compose_turn` / 组合（入口 = 本文件） | 组合 | 装配链端到端（AC-1 / AC-3）：`compose_turn(…, Arc::new(AskRegistry::default()), …, None)` 后以 `source="explore"` 起一轮 → `RunStarted.tools` 含 `AskUserQuestion`（经 `definitions` 派生）；以 `source="change"` 起一轮 → 名单同样含该名（同名覆盖的端到端侧证）；`ask=None` 形态（手工构造 `SessionInjections`，不经 `assemble_injections`）→ 名单不含该名 | 组合迭代（3 来源形态） |
| `compose_turn` | 正向 | 注册表单表共享（DD3 的机械判据）：`compose_turn(…, Arc::clone(&reg), …)` 后组合根内的内核与门面指的是**同一张表**——外部 `reg.answer(<core session id>, …)` 能命中引擎侧登记（登记 → 外部应答 → 接收方收到） | 单次 |
| `compose_turn` / `assemble_injections` | 异常 | 非法名 `Err` 语义零改：构造含挂载清单外名字的注入面 → 既有 `Err`（文案携非法名与来源），零 ask 相关新增分支干扰 | 单次 |
| `assemble_injections` | 异常 | 零 `default()` 兜底自证（R8）：`compose.rs` 源码内 `assemble_injections` 函数体零 `SessionInjections::default()` 命中（其余来源臂为显式结构字面量，四字段全给） | 单次 |
| `assemble_injections` | 边界 | 逐臂 `ask` 位穷举：`source` 取 `explore` / `archi` / `change` / `debug` / `""` / 未知串 六形态 → `ask` 位恒 `Some(_)`（前二是 `Generic`、change 是 `Flow`、其余三全 `Generic`）——**任何来源都不落 `None`**（首期无禁用场景，D13） | 参数化迭代（6 形态） |
| `compose_turn` | 边界 | `ask_port` 两形态：`Some(fake_port)` 与 `None` 均可组合成功（`None` 不报错——它是 debug/explore 轨道的正常形态）；`Some` 形态在 `Flow` 会话里可被观测到调用（组合根到执行体的贯通） | 参数化迭代（2 形态） |
| 编译闭合（构造点） | ↔ 编译闭合 | `compose_turn` 36 处加两实参（本文件）+ `worker_test` 8 处（同签名，见 `worker.rs` 章节的编译闭合登记）；`SessionKernel::new` 2 处、`SessionInjections` 1 处（四臂重写非纯机械）、`SessionOpen` 3 处补字段——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `store::Store` / `WorkspaceStores`（既有 tempdir 真库） | 真实临时库（文件系统 + DB 边界，`tempfile` RAII 隔离） | 全章节 |
| `agent` 实例解析（既有 seed 装置） | 真库内 seed 的 provider / instance 记录（DB 边界，非 mock） | 组合行 |
| `AskPort`（手写假 port） | 显式注入依赖（`compose_turn` 实参） | `ask_port` 两形态行 |
| 不 mock：`AskRegistry` | 真实实例（单表共享判据需要真实 `Arc`） | 全章节 |
| 不 mock：`assemble_injections` / `definitions` | 真实函数（三臂置位与名单派生是被测契约本体） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs`

**既有 sidecar 新增**：`AskPort` 透传用例组（`KernelWorkerPort` 与 sink 的装配关系）；8 处 `compose_turn` 调用加两实参属编译闭合（同 `compose_test` 章节口径）。

#### 待测功能

- `KernelWorkerPort` / `AskPort` 透传（`new` 签名不变，装配面扩 ask port）: change 来源的 sink 同时实现 `AskPort` 时，`Flow` 档执行体拿到该句柄；非 change 来源为 `None`。

<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 worker.rs 行），且 `worker.rs` 本身不在 design「修改文件」表内——它的 `KernelWorkerPort::new` 签名零改动，只有 `compose_turn` 的调用点随签名加两实参（design「编译闭合批」明列 `worker_test` 8 处）。本章节因此只登记**组合用例挂靠**：「`AskPort` 句柄是否真的抵达 `Flow` 档执行体」这条链路的最上层可观测点是本文件的 `KernelWorkerPort`（它是 `ComposedTurn` + sink 的唯一装配处），proposal「测试文件」表的 `worker_test` 行亦为此处；不虚构 `worker.rs` 的产品 API 变更。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `KernelWorkerPort` / `AskPort` 透传 | 正向 | change 来源拿到句柄（AC-2）：假 sink 同时实现 `RunEventSink` + `AskPort`（记录调用）；以 `source="change"` + `ask=Some(Flow)` 轮 → 流缝脚本化一次 `AskUserQuestion` → 假 sink 的 `ask_started` / `ask_resolved` 各记录一条、载荷 `tool_call_id` 非空 | 单次 |
| `KernelWorkerPort` / `AskPort` 透传 | 正向 | 非 change 来源为 `None`：同装配但 `ask=Some(Generic)`（explore / debug 形态）→ 假 sink 的 `AskPort` 方法零调用（`Generic` 档不吃 port），但应答仍可回灌续跑 | 单次 |
| `KernelWorkerPort` / `AskPort` 透传 | 异常 | 无 port 的 `Flow` 轮（DD2 防御臂的生产形态）：`ask=Some(Flow)` 且 `compose_turn` 传 `None` → 该调用 `is_error` 回灌、sink 零记录、run 仍正常收口（不悬挂） | 单次 |
| `KernelWorkerPort` / `AskPort` 透传 | 边界 | 同一 `Arc` 两次 trait 强转：`Arc<ChangeFlowSink>` 分别作为 `Arc<dyn RunEventSink>` 与 `Arc<dyn AskPort>` 传入 → 两侧观测到同一实例状态（同一 `Arc` 强转，非两份 sink——以 sink 内部计数两路可见即证） | 单次 |
| 既有全链用例 | 异常（零改动回归） | 既有 sdk 轮全链用例（假 sink + `DualEnv` 真库 + `KernelWorkerPort`）在补两实参后零断言改动全绿（`SessionEvent` 透传 / 收敛 / 转录面零回归） | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `RunEventSink` + `AskPort`（手写假 sink） | 双面捕获的记录式替身——两者都是**显式注入依赖**（`KernelWorkerPort::new` 入参），且是「透传是否存在」的唯一观测面 | 全章节 |
| `rig` 流（既有流缝脚本化） | provider 网络边界 | 全章节 |
| `store` / `WorkspaceStores`（既有 `DualEnv` 真库） | 真实临时库（DB 边界） | 全章节 |
| 不 mock：`compose_turn` / `ComposedTurn` | 真实组合根（装配链是被测面） | 全章节 |
| 不 mock：`AskRegistry` | 真实实例 | 全章节 |

### `packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**既有 sidecar 新增 + 改造**：`app_with_stores` 装置补 `app.manage(Arc::new(AskRegistry::default()))`；追加 `agent_answer` 四态用例组与通用侧应答链组合用例（**最上层调用方 = 本文件**）。既有 `agent_start` / `agent_stop` / 查询命令用例零改动。

#### 待测功能

- `agent_answer(registry: State<'_, Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>`（新增 IPC 命令）: 薄包装三段——blank `root` / blank `session_id` → `Err`（文案携字段名）；miss → `Err("当前无等待中的 ask")`；命中 → `Ok`。**`text` 不校验**（自由应答）；零状态机、零广播、零清理。
- `agent_start_with(…)`（既有命令体加一行）: 取 `AskRegistry` 句柄传入 `compose_turn`；私有 `struct ChatAskPort` + `impl AskPort`（DD15：chat 会话无编排停等面的类型化空实现），`agent_start` 以 `Some(Arc::new(ChatAskPort))` 传入组合根。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agent_answer` | 正向 | 命中即转投（AC-10 / AC-2）：`registry.register("core-1", tx)` → `agent_answer(registry, root, "core-1", "选项A")` → `Ok(())`；接收方逐字收 `"选项A"` | 单次 |
| `agent_answer` | 正向 | 命令体三件事纪律（AC-10）：`root` / `session_id` 非空白时零 `app.state` 变更、零广播（无 `Channel` 参与）——以「注册表外无副作用」形式断言：调用后 `answer` 再次 `Err`（槽已被 take，无第二次回灌） | 单次 |
| `agent_answer` | 异常 | blank 两参（AC-10）：`root = ""` / `"   "` → `Err` 文案携 `root`；`session_id = ""` / `"   "` → `Err` 文案携 `session_id`；两者均零注册表访问（登记中的槽仍在：随后合法调用命中） | 参数化迭代（4 形态） |
| `agent_answer` | 异常 | miss 显式 `Err`（AC-7 / AC-10）：未登记 `session_id` → `Err`，文案含「当前无等待中的 ask」（**不是**幂等 `Ok`——与 `agent_stop` 的 miss 幂等语义不同）；已应答后二次调用 → 同文案 `Err` | 参数化迭代（2 形态） |
| `agent_answer` | 边界 | `text` 零校验：`""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 均 `Ok` 且接收方逐字收到同一串（后端不替用户裁量内容） | 参数化迭代（5 形态） |
| `agent_answer` | 边界 | `session_id` 边界：`"a\0b🚀"` / 超长串 → 与注册表键原样比对（不 trim 不截断）；近邻键（`""` 空格变体）互不命中 | 参数化迭代（2 形态） |
| `agent_answer` | 边界 | 幂等性边界：同键连续两次合法调用 → 第一次 `Ok`、第二次 `Err`；等待方只收一次（AC-7 的命令面证据） | 单次 |
| `agent_answer` / `AskRegistry` / `agent_start` / loop（入口 = 本文件） | 组合 | 通用侧应答链端到端（AC-1 / AC-13）：mock app 托管 `AskRegistry` + 真库 seed → `agent_start_with(source="explore")` 起一轮（流缝脚本化一次 `AskUserQuestion`）→ 从 `app.state::<Arc<AskRegistry>>()` 取句柄 → `agent_answer(root, session_id, "选项A")` → `Ok`；会话转录中出现该 `tool_use` 的配对 `tool_result{content:"选项A", is_error:false}`；会话行状态最终 `completed`（同 turn 续跑、零新会话：`session_id` 与 `agent_start` 返回值一致） | 单次 |
| `agent_start_with`（DD15） | 组合 | change-chat 的 `Flow` 档可 ask（AC-13）：`source="change"` 且 `source_ref = <change id>`（无 run 形态）+ `ask=Some(Flow)` → `ChatAskPort` 空实现使该轮 `AskUserQuestion` **不返回 `is_error`**（若 port 缺席则必 `is_error`「ask 编排状态面未装配」）；应答经 `agent_answer` 回灌成功 | 单次 |
| `agent_start_with` | 异常 | 注册表未托管时的显式失败：mock app 未 `manage(AskRegistry)` → `agent_start` 的 `State` 取出失败（Tauri 运行期报错），不产生半截会话行（装置层判据：会话表零新增） | 单次 |
| 编译闭合（`compose_turn` 调用 / 托管态） | ↔ 编译闭合 | `compose_turn` 1 处加两实参；`app_with_stores` 装置补 `app.manage(Arc::new(AskRegistry::new()))`——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri runtime（既有 `tauri::test::mock_app`） | Mock 运行时注入（`AppHandle<R>` 泛型缝的既有装置）——进程边界（宿主运行时） | 全章节 |
| `workspace` DB（既有 `Env` = tempdir 双根 + 真库） | 真实临时库（文件系统 + DB 边界，RAII 清理） | 全章节 |
| `rig` 流（既有流缝脚本化） | provider 网络边界 | 组合行 |
| 不 mock：`AskRegistry` | 真实实例托管在 mock app 上（单表/转投/take 语义是判据本体） | 全章节 |
| 不 mock：`SessionKernel` / `compose_turn` | 真实组合根（组合行需要真实链路） | 组合行 |

### `packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

**既有 sidecar 改造**：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各补一条（`agent_answer` / `agentAnswer`，紧随 `agent_stop`）；条目数 48 → 49；`DTO_TYPES` 清单零补录（`AskPayload` 是既有出线类型，两 additive 字段不新增类型名）；反向锚定清单零改动即覆盖 `AskMode`。生成序断言（`agent_start` / `agent_stop` / `agent_session_watch` 三包装相对序）扩入新命令。

#### 待测功能

- `all_commands!` 登记面（macros 单点）: `$crate::commands::exec::agent_answer` 紧随 `agent_stop`（`commands/exec` 轨道）；`bindings:export` 产物含 `agentAnswer` 包装。
- `SessionInjections` / `FinishSpec` / `TurnQuestion` 反向锚定清单（零改动即覆盖 `AskMode`）: `AskMode` 挂在 `SessionInjections` 的字段面上且不派生 `specta::Type`，故不出线；`AskPayload` 出线且含 `toolCallId` / `sessionId` 两字段。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` / 产物 | 正向 | 命令补录（AC-10）：`COMMAND_NAMES` 含 `"agent_answer"`、`COMMAND_WRAPPERS` 含 `"agentAnswer"`；两份清单条目数**同时** 48 → 49 且持衡（漏登记或重复登记即红）；首锚仍 `list_changes`、尾锚仍 `read_file` | 单次 |
| 生成序 | 正向 | 序断言：`agentAnswer` 在产物中的偏移紧随 `agentStop`（`all_commands!` 登记序 = 生成序，沿既有三包装相对序先例） | 单次 |
| 产物 `AskPayload` | 正向 | 两字段出线（AC-9）：产物内 `AskPayload` 类型声明含 `toolCallId` 与 `sessionId` 两键（`string` 与 `null` 的联合）；`DTO_TYPES` 清单零补录（不新增类型名——`AskPayload` 是既有出线类型，两字段 additive 不改变清单） | 单次 |
| 反向锚定 | 异常 | `AskMode` 不出线（AC-9）：既有「引擎侧会话注入词汇不出线 ipc 产物」用例对 `SessionInjections` / `FinishSpec` / `TurnQuestion` 三名的零命中断言零改动全绿；追加断言：产物全文零 `"AskMode"`、零 `"generic"` / `"flow"` 作为**类型或命令参数**出现（词面可能出现在其它语境，故只判类型名 `AskMode` 与 `ask:` 字段的类型引用） | 单次 |
| 产物 | 异常 | `agent_answer` 的返回类型：产物内 `agentAnswer: (root: string, sessionId: string, text: string) => Promise<null>`（三参逐字、返回 `null` = `Result<(), String>` 的出线形态） | 单次 |
| 产物 | 边界 | additive 承接：`AskPayload` 既有两键 `question` / `options` 不受影响（逐字仍在）；旧线载荷（无两新键）在 TS 侧为必给可空（`string` 与 `null` 的联合）而非可选（`?:`）——`Option<T>` 出线为必给可空，与既有 `Option` 形态一致 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 产物快照（既有 `authoritative_snapshot` + `export_bindings` 副作用守卫） | 真实 `bindings:export` 产出的读盘快照 + `RestoreOnDrop` 恢复守卫——**文件系统边界**，且是「产物与清单一致」判据的唯一来源 | 全章节 |
| 零 mock：`COMMAND_NAMES` / `DTO_TYPES` / 反向锚定清单 | 真实常量（清单本身是被测面） | 全章节 |

### `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` -> `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`

**既有 sidecar 改造**：1 处 `ChangeFlowControl::new` 加实参（+ 每用例注册表独立）；2 处 `RunUpdate::Ask` 字面构造随变体收敛；既有 `wait_answer` 驱动的应答用例重写为「转投 + `ask_resolved` 清理」两段；追加 `AskPort for ChangeFlowSink` 用例组与双入口组合用例（**最上层调用方 = 本文件**）。

#### 待测功能

- `impl AskPort for ChangeFlowSink`（新增 trait impl）: `ask_started(ask)` → `control.publish(RunUpdate::Ask { ask })`；`ask_resolved()` → `control.resolve_ask(root, change_id)`。
- `change_flow_start_with(…)`（装配序调整）: 先建 `Arc<ChangeFlowSink>`（两次 trait 强转同一 `Arc`），再 `compose_turn`（传注册表 + `Some(ask_port)`），最后 `KernelWorkerPort::new`。
- `change_flow_answer_with(…)`（既有命令，语义改）: 保持调 `control.answer`（签名与前端参数面零改动）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `impl AskPort for ChangeFlowSink` | 正向 | `ask_started` → 快照置位（AC-2）：`sink.ask_started(AskPayload { question, options, tool_call_id: Some("call_1"), session_id: Some("core-1") })` → 该 `(root, change)` 条目的 `status == WaitingAsk`、`ask` 四字段逐字段相等、广播端收一条 `RunNotice::Ask` | 单次 |
| `impl AskPort for ChangeFlowSink` | 正向 | `ask_resolved` → 清理单点（AC-2 / Y-1）：先 `ask_started` 再 `sink.ask_resolved()` → `status == Running`、`ask == None`、广播端收一条 `RunNotice::Step`（**不是** `Ask`——失效信号复用既有通用 kind） | 单次 |
| `impl AskPort for ChangeFlowSink` | 正向 | 两方法都只依赖 `(root, change_id)`：`sink` 的 `root` / `change_id` 字段决定寻址，不读 run_id / 不读注册表（`session_id` 只进载荷） | 单次 |
| `change_flow_answer_with` | 正向 | 转投（AC-8）：`begin_run` + `set_session(Some("core-1"))` + `ask_registry.register("core-1", tx)` → `change_flow_answer_with(app, root, id, "选项A")` → `Ok`；接收方逐字收原文；**命令内零清理**：`status` / `ask` 槽 / 广播均未变（清理等 `ask_resolved`） | 单次 |
| `change_flow_answer_with` | 异常 | 无 run / 无等待方两 `Err`（AC-8）：未 `begin_run` → `Err` 携 change id；有 run 无会话槽 → `Err` 含「当前无等待中的 ask」；blank `root` / `id` → 既有 `Err`（文案逐字不变） | 参数化迭代（4 形态） |
| `change_flow_start_with` | 正向 | 装配序（D 事实校准 9）：起一轮后 `AskPort` 已注入 → 流缝脚本化一次 `AskUserQuestion` → 快照 `activeRun.ask` 四字段在位、`status == WaitingAsk`；`ask_resolved` 后回 `Running`（同一 `Arc<ChangeFlowSink>` 两次强转的连通性证据） | 单次 |
| `change_flow_answer` + `agent_answer`（入口 = 本文件） | 组合 | 双入口幂等（AC-2 / AC-7）：同一 ask 先经 `change_flow_answer_with` 应答 → `Ok`；再经 `agent_answer(registry, root, session_id, …)` → `Err`（「当前无等待中的 ask」）；反向顺序亦成立（`agent_answer` 先 `Ok`、`change_flow_answer` 后 `Err`）；两条路径下等待方**只收一次**原文、快照 `ask` 槽最终清空、`status` 回 `Running` | 组合迭代（2 入口序） |
| `change_flow_answer_with` + `AskPort`（入口 = 本文件） | 组合 | 停止路径不残留（AC-5 / spec「停止期间停等态不残留」）：`ask_started` 后置位 run 取消 → `ask_resolved` 执行 → `status` 随 run 收口为 `stopped`（或回运行态）+ `ask == None`；`AskRegistry` 该键由轮收敛除名（随后 `answer` → `Err`） | 单次 |
| `ChangeFlowSink` | 异常 | 终态不被倒灌（DD5）：`ask_started` 后先 `publish(Finished { Completed })` → `ask_resolved()` → `status` 仍 `Completed`、`ask == None`、广播一条 `Step` | 单次 |
| 既有 `RunEventSink` 面 | 异常（零改动回归） | `SessionEvent` 的 `set_session` 前置同步与 `publish` 透传断言零改动全绿（`AskPort` 新增 trait 不改既有 `emit` 路径） | 单次 |
| `impl AskPort for ChangeFlowSink` | 边界 | 载荷边界：`ask_started` 的 `AskPayload` 取 `question=""` / `options=[]` / 1000 元素 / `tool_call_id=None` / `session_id=None` → 快照逐字保真、`status` 仍切 `WaitingAsk`（`sink` 不裁量内容，`minItems` 的裁决在工具校验） | 组合迭代（4 形态） |
| `change_flow_answer_with` | 边界 | 应答正文边界：`answer` 取 `""` / `"   "` / 含换行的两行串 / 含 NUL 字符与 emoji 的串 / `"字".repeat(1200)` → 逐字投递给等待方（命令层零 trim、零空白校验——拦空在 `AskCard` 提交侧） | 参数化迭代（5 形态） |
| 编译闭合（构造点 / 字面构造） | ↔ 编译闭合 | `ChangeFlowControl::new` 1 处加实参；`RunUpdate::Ask` 2 处随变体收敛——零新增用例、零断言改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri runtime（既有 `tauri::test::mock_app`） | Mock 运行时（宿主边界） | 全章节 |
| `workspace` DB（既有 `Env` = tempdir 双根 + 真库） | 真实临时库（DB 边界） | 全章节 |
| `rig` 流（既有流缝脚本化） | provider 网络边界 | 起轮组合行 |
| 不 mock：`ChangeFlowControl` / `RunGuard` | 真实实例（快照 + 广播是被测面） | 全章节 |
| 不 mock：`AskRegistry` | 真实实例（每用例独立 `Arc`；双入口幂等判据需要真实 take 语义） | 全章节 |
| 不 mock：`ChangeFlowSink` | 真实实例（`AskPort` impl 是被测面） | 全章节 |

### `packages/desktop/src/types/generated/bindings.ts` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

**生成物零直接测试路径**：文件由 `bindings:export` 再生成（禁手改），其判据面在 Rust 侧 sidecar 的读盘快照用例与 `git diff` 审查（见「不可测试项」）。本表只登记「被测文件或模块」映射，用例不重复列（与 `commands/mod.rs` 章节同址同文件）。

<!-- design.md 未声明该文件的公共 API 变更（生成物，不在「公共函数 / API」表）；本条目取自「修改文件」表该文件行与 spec Module Contract「packages/desktop/src/types/generated/bindings.ts（生成物）| 前端调用面」。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更——生成物由 `bindings:export` 产出，其形状断言在 `packages/desktop/src-tauri/src/bindings/mod_test.rs` 章节（AC-9 / AC-10）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 生成物 | 正向 | 生成物在场与形状（AC-9 / AC-10）：用例挂在 `packages/desktop/src-tauri/src/bindings/mod_test.rs` 章节（`agentAnswer` 三参签名、`AskPayload` 两字段、命令清单 48 → 49）——本行只作映射登记，不新增独立断言 | 单次 |
| 生成物 | 异常 | 手改判据（AC-9）：`bindings:export` 后再生成零 diff（`git diff --stat -- src/types/generated` 为空）由桌面端门禁执行，见「不可测试项」 | 单次 |
| 生成物 | 边界 | additive 承接（AC-9）：`AskPayload` 两新字段在 TS 侧为必给可空（`toolCallId: string \| null` / `sessionId: string \| null`），旧线载荷（后端 `None`）落到前端为 `null` 而非 `undefined`——`AskCard` 与活卡的判据因此不需要 `??` 兜底之外的第二分支 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 生成物无运行期行为；TS 侧类型契约由 `tsc`（`client:check`）与 `knip` 覆盖 | 全章节 |

### `packages/desktop/src/hooks/use-agent-session.ts` -> `packages/desktop/src/hooks/use-agent-session.test.ts`

**既有测试文件新增**：`answer` 动作与三源错误面合并用例组（沿既有 `invokeMock` 按命令名可编程 + `driftReports` 装置）。既有重放装载 / 状态驱动挂流 / 单流在挂 / 断流重订 / 发送与停止用例零改动（回归登记）。

#### 待测功能

- `answer(text: string) => void`（`UseAgentSessionState` 新增动作）: `ipc.agentAnswer(root, sessionId, text)` 薄包装；`root` / `sessionId` 任一为 `null` → 零 invoke 零副作用；`Err` 落 `actionError`。
- 返回面 `error`（`string | null`，三源合并）: `replay.error ?? chatError ?? actionError`（保序：重放 > 流 > 动作）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `answer` | 正向 | 参数逐字（AC-13）：`root="D:/ws"` + `sessionId="core-1"` + `answer("选项A")` → `invokeMock` 收到 `("agent_answer", { root: "D:/ws", sessionId: "core-1", text: "选项A" })`（键名与值逐字，零额外字段） | 单次 |
| `answer` | 正向 | 成功路径零错误：`invoke` resolve → `error === null`；`answer` 自身不触发 `send` / `stop` / 重放重取（`invokeMock` 调用次数恰 +1 且命令名为 `agent_answer`） | 单次 |
| `answer` | 异常 | `root` 为 `null`（AC-13）：`answer("x")` → 零 invoke（`invokeMock` 未被以 `agent_answer` 调用）、零副作用、`error` 不变 | 单次 |
| `answer` | 异常 | `sessionId` 为 `null`：同上（会话未建立形态——历史重放/空态）；两 null 并存同断言 | 参数化迭代（2 形态） |
| `answer` | 异常 | `Err` 落错误面（DD11）：`invoke` reject 携 `"当前无等待中的 ask"` → `error` 为该文案（原文呈现、不吞不开成通用串） | 单次 |
| `answer` / `error` | 异常 | 三源合并保序（DD11）：① 仅 `actionError` → 呈现它；② `chatError` 与 `actionError` 并存 → 呈现 `chatError`；③ `replay.error` 与两者并存 → 呈现 `replay.error`（逐级 `??` 序的机械判据） | 组合迭代（3 组合） |
| `answer` | 异常 | 连续两次应答：第二次 `invoke` 仍发出（前端不本地去重——去重是后端 take 语义；命令面第二次收 `Err` 并落 `actionError`） | 单次 |
| `answer` | 边界 | 正文边界：`""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 逐字透传（hook 不做 trim、不做空白拦截——拦空是活卡与 `AskCard` 的提交侧职责） | 参数化迭代（5 形态） |
| `answer` | 边界 | 引用稳定性（DD12）：连续两次渲染后 `state.session.answer` 为**同一引用**（`useCallback` 稳定性；活卡与 `renderItem` 的 memo 前提）。本行**不需要探针装置**——hook 是被测本体，`renderHook` 返回值里直接持有该引用，比较是直接的；探针只用于「引用跨组件边界传递」的下游章节 | 单次 |
| `answer` | 边界 | 与既有动作面共存：`sendMessage` / `stop` / `reset` 三动作签名与行为零改动（既有用例零改动全绿）；`answer` 不进入发送链路（不推本地提问副本、不改 `pendingQuestionIdRef`） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `ChannelMock` + `invokeMock`，`vi.hoisted`） | 按命令名 + 调用序可编程的替身（IPC = 进程边界）；`Channel` 替身捕获 `onmessage` 供直投信封 | 全章节 |
| 不 mock：`agent-transport` / `agent-adapter` / `@ai-sdk/react` `useChat` | 真实组合（hook 即链路入口；替身化会让「应答不进入发送链路」自证） | 全章节 |
| 不 mock：`ipc-drift`（既有 `driftReports` / `resetDrift`） | 真实实现（漂移报告面零改动） | 全章节 |

### `packages/desktop/src/components/agent/tool-cards.tsx` -> `packages/desktop/src/components/agent/tool-cards.test.tsx`

**既有测试文件改造 + 新增**：`AskUserQuestionCard` 静态态断言改造（旧文案「请在下方输入框作答」零命中）；追加活卡态、双形态解析、提交禁用三组用例。既有注册表六键 / `FinishStepCard` / `ToolPairCard` / `WebSearchCard` / `cardAttrs` / `languageOf` 用例零改动（回归登记）。

#### 待测功能

- `AgentToolCardProps.askAnswer`（新增可选动作面，`{ onAnswer: (text: string) => void }`）: 在场即可应答；卡内不需要 `pendingToolCallId`（判活在上层完成）。
- `AskUserQuestionCard({ input, askAnswer }: AgentToolCardProps)`: 双态——静态（可读、零交互、文案去掉「请在下方输入框作答」）/ 活卡（选项按钮 + 自由文本输入 + 提交，提交后本地禁用防连点，空串与纯空白不提交）。
- `parseAsk(input)`（双形态归一，DD9）: ① 新工具形态 `{ question: string, options: (string | {label,description})[] }`（字符串元素 → `description = null`）；② 历史 SDK 形态 `{ questions: [{…}] }`；③ 两者皆不匹配 → `null`（JSON 原样回退）。

<!-- design.md「公共函数 / API」表未声明 `parseAsk` 的行为面（该表只有 `AgentToolCardProps` 与 `AskUserQuestionCard` 相关的「类型定义」行）；本条目取自「修改文件」表该文件行（`parseAsk` 双形态归一，DD9）与「类型定义」表 `AgentToolCardProps` 行。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskUserQuestionCard` 静态态 | 正向 | 未注入动作面（AC-11）：`askAnswer` 省略 + 新工具形态 input → 卡在场（`data-testid="ask-question-card"`）、题干与选项逐字可读；**零按钮**（`role="button"` 数 0）、**零 `<input>` / `<textarea>`**、零 `onAnswer` 调用 | 单次 |
| `AskUserQuestionCard` 静态态 | 异常 | 旧误导文案零残留（AC-11）：静态卡全文零命中「请在下方输入框作答」；同时断卡头仍含「向用户提问（AskUserQuestion）」类标识（文案改写不是删文案） | 单次 |
| `AskUserQuestionCard` 活卡态 | 正向 | 选项即应答（AC-6 / AC-11）：注入 `askAnswer={{ onAnswer }}` → 每个选项渲染为可点按钮；点击第 N 个选项 → `onAnswer` 收到该选项**文本**逐字（一次调用、零多余参数） | 单次 |
| `AskUserQuestionCard` 活卡态 | 正向 | 自由文本两路并存（AC-6）：输入框输入 `"自由作答"` 后提交 → `onAnswer("自由作答")`；与选项按钮同卡并存（选项仍在、输入框仍在） | 单次 |
| `AskUserQuestionCard` 活卡态 | 正向 | 提交后本地禁用防连点（AC-11）：点选项后本地交互置 `disabled`（再次点击不产生第二次 `onAnswer`）；文本提交后同理（提交按钮 / 输入框禁用） | 参数化迭代（2 形态：选项 / 文本） |
| `AskUserQuestionCard` 活卡态 | 异常 | 空串与纯空白不提交（AC-11）：输入框为空 / 全空白时提交按钮禁用、`fireEvent.click` 后 `onAnswer` 零调用（提交侧拦截，不发 IPC） | 参数化迭代（2 形态） |
| `parseAsk`（经卡驱动） | 正向 | 历史 SDK 形态仍可读（DD9 / 事实校准 10）：`{ questions: [{ question: "q", options: [{ label: "L", description: "D" }] }] }` → 渲染题干 `q` + 选项 `L` 与描述 `D`（**MUST NOT** 退化为 JSON 原样） | 单次 |
| `parseAsk`（经卡驱动） | 正向 | 新形态字符串选项（DD9）：`{ question: "q", options: ["A", "B"] }` → 两选项渲染为 `A` / `B`、`description` 为 `null`（无描述节点）；活卡态下两点均可点 | 单次 |
| `parseAsk`（经卡驱动） | 异常 | 两形态皆不匹配（DD9）：`{}` / `{ question: 1 }` / `"str"` / `null` / `[]` → 回退 `<pre>` JSON 原样（既有宽容解析零回归）；零 panic | 参数化迭代（5 形态） |
| `parseAsk`（经卡驱动） | 边界 | 混合选项形态：`options: ["A", { label: "B", description: "d" }]` → 逐项归一到 `AskOption`（前者 `description = null`、后者 `"d"`）——同一 `AskQuestionView` 单点渲染 | 单次 |
| `parseAsk`（经卡驱动） | 边界 | 边界形态：`options: []`（零选项 → 空列表、卡仍渲染题干）/ `options` 1000 元素（不 panic、逐项渲染）/ `question: ""`（空题干仍渲染节点） | 参数化迭代（3 形态） |
| `AgentToolCardProps.askAnswer` | 正向 | 接口面两形态：`AgentToolCard` 类型化调用（`ComponentType<AgentToolCardProps>`）在「省略 `askAnswer`」与「给值」两形态下均可编译；给值时卡内不消费 `pendingToolCallId`（判活在上层完成——卡内只认 `onAnswer`） | 参数化迭代（2 形态） |
| 既有卡面 | 异常（零改动回归） | AC-11 后半：未注册工具与 `ToolPairCard` 回退用例、`BUILD_IN_TOOL_CARDS` 六键 / `finish_step` / `web_search` 命中用例、`cardAttrs` 卡根同形用例零改动全绿（活卡改造不触其它卡） | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@/lib/highlighter`（既有 `loadHighlighterMock`，`vi.hoisted`） | 语法高亮替身（shiki 动态 `import` 与重型 grammar 装载 = 进程边界资源）；替身产出可断言行并记录语言实参 | 全章节（既有装置沿用） |
| 不 mock：`AskUserQuestionCard` / `parseAsk` | 真实实现（双态与双形态解析是被测契约本体） | 全章节 |
| 不 mock：`asRecord` / `textOf`（`@/lib/parse`） | 真实工具函数（宽容解析的一半） | 全章节 |
| 不 mock：`Spinner` / `CodeBlock` / `Trace*` | 真实组件（卡根同形断言面） | 全章节 |

### `packages/desktop/src/components/agent/agent-messages.tsx` -> `packages/desktop/src/components/agent/agent-messages.test.tsx`

**既有测试文件新增**：`askAnswer` 下行与三态判活矩阵用例组（DD8）+ 引用稳定性（DD12）。既有四变体气泡 / system 过滤 / 虚拟窗口 / 吸附 / 轮内状态行 / citations 用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 agent-messages 行）；上述两条取自「类型定义」表的 `AskAnswerActions` 行与「修改文件」表该文件行（`AgentMessagesProps.askAnswer` + 三态判活 + `useMemo` 稳定化）。 -->
#### 待测功能

- `AgentMessagesProps.askAnswer`（新增可选动作面，`AskAnswerActions`）: `{ onAnswer: (text: string) => void; pendingToolCallId?: string | null }`；导出类型 `AskAnswerActions`。
- `ToolPartView`（按卡名与判据透传）: 仅 `AskUserQuestion` 部件拿到 `askAnswer`；三态判活——`undefined`（未给出）→ 判活 = `state === 'input-available'`；`null`（明确无等待）→ 全静态；`string` → 仅 `toolCallId` 相等者活；未传 `askAnswer` → 全卡静态。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentMessagesProps.askAnswer` | 正向 | 接口面与导出类型（AC-11）：`AskAnswerActions` 可 import（两字段：必给 `onAnswer`、可选可空 `pendingToolCallId`）；`AgentMessagesProps` 该字段可选（省略即静态）——以类型化两形态渲染编译通过为判据 | 参数化迭代（2 形态） |
| `ToolPartView` | 正向 | 仅 ask 部件拿到动作面（AC-11）：一条消息含 `tool-AskUserQuestion` + `tool-read` + `tool-bash` 三部件，注入 `askAnswer` → ask 卡为活卡（按钮在场），`read` / `bash` 卡与既有形态逐字相同（零动作面、零按钮差异） | 单次 |
| `ToolPartView` 三态判活 | 正向 | `undefined`（未给出）：`askAnswer = { onAnswer }` + ask 部件 `state == 'input-available'` → 活卡；同一件 `state == 'output-available'` → 静态（未配对即活，配对即静） | 组合迭代（2 state） |
| `ToolPartView` 三态判活 | 正向 | `string` 精确对账（DD8 / AC-11）：`pendingToolCallId = "call_2"`；消息含 `call_1` 与 `call_2` 两个 ask 部件（均 `input-available`）→ 仅 `call_2` 活卡，`call_1` 静态 | 单次 |
| `ToolPartView` 三态判活 | 异常 | `null` 全静态（DD8 / spec「`activeRun.ask` 为 null 时全为静态」）：`pendingToolCallId = null` + 两 ask 部件均 `input-available` → 两张卡全静态（流程侧「当前无等待」的表达） | 单次 |
| `ToolPartView` 三态判活 | 正向 | **已配对即静（下游 test-gen 补，G3 骨架）**：三态取任一值（`undefined` / 匹配串 / `null`）时，`state === 'output-available'` 的 ask 部件**恒为静态卡**（结果已回灌，即使 `pendingToolCallId` 与之相等也不再判活）；同消息内 `input-available` 的 ask 部件仍按三态矩阵判活——「结果在场优先于等待面」的静态态骨架 | 组合迭代（3 三态 × 2 state） |
| `ToolPartView` 三态判活 | 异常 | 未传 `askAnswer`：全卡静态（历史重放形态）——ask 卡零按钮零输入，其它卡零差异 | 单次 |
| `AgentMessages.askAnswer` | 异常 | `onAnswer` 透传唯一性：多个活卡并存（`pendingToolCallId = undefined`）时点任一卡的选项 → `onAnswer` 恰一次调用且参数为**该卡**的选项文本（不串卡） | 单次 |
| `AgentMessages.askAnswer` | 边界 | `pendingToolCallId = ""`（空串）：无 `toolCallId` 为空的部件（生产恒非空）→ 全静态、零 panic（三态矩阵的退化取值） | 单次 |
| `AgentMessages.askAnswer` | 边界 | 引用稳定性（DD12）：父组件重渲染但 `askAnswer` 依赖未变 → 下游 ask 卡收到的 `askAnswer` 与上一渲染**同引用**（判据只能经探针装置读 `propsLog` 做引用比较：`propsLog[0].askAnswer === propsLog[1].askAnswer`；DOM 与调用次数反推不出引用相等）；`askAnswer` 对象在 `AgentMessages` 内 `useMemo` 合成（同一输入两次渲染同引用）；`renderItem` / `renderSticky` 依赖数组不因新对象重建（以探针记录的组件渲染次数不增长为侧证） | 单次 |
| 既有消息面 | 异常（零改动回归） | 四变体气泡 / system 过滤 / 空态 / 虚拟窗口 / 吸附 / 空内容轮可见性 / 一轮一条气泡 / citations 八组用例零改动全绿（新增可选 prop 不扰动既有形状） | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ResizeObserver` / 几何测量（既有装置） | jsdom 缺省能力的替身（运行环境边界，既有测试文件已装） | 虚拟窗口 / 吸附行 |
| 不 mock：`AgentMessages` / `ToolPartView` / `ChatBubble` | 真实实现（透传与判活是被测面）；引用相等断言的观测点不在本层，而在下游 ask 卡的探针（见下条） | 全章节 |
| 不 mock（实现保真）：`BUILD_IN_TOOL_CARDS`（经 `tool-cards`） | 真实注册表与真实卡组件（「仅 ask 部件拿到动作面」需真实卡）；实现**不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`BUILD_IN_TOOL_CARDS['AskUserQuestion']`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/tool-cards', async (importOriginal) => ({ ...(await importOriginal()), BUILD_IN_TOOL_CARDS: { ...actual.BUILD_IN_TOOL_CARDS, AskUserQuestion: Probe } }))`；`Probe` 取 `actual.BUILD_IN_TOOL_CARDS['AskUserQuestion']` 作 `Real`，把 props（含 `askAnswer` / `input`）push 进 `vi.hoisted` 的 `propsLog` 后原样 `<Real {...props} />`——`importOriginal` **保留全部真实实现**（其余卡 / `parseAsk` 照旧），探针零行为、只记录。**探针挂注册表键而非组件名**：`AskUserQuestionCard` 不是模块导出面，为测试加导出会污染死代码扫描（沿仓库「无 test-only export」纪律） | DD12 引用稳定性行（两次渲染的 `propsLog[n].askAnswer` 引用比较）；「仅 ask 部件拿到动作面」行（`propsLog` 只出现 ask 卡一条、`read` / `bash` 零记录）；「`onAnswer` 透传唯一性」行（点击照走真卡回调，探针只留证） |
| 不 mock：`onAnswer` | 真实 `vi.fn()` 记录调用（被测注入依赖，不是模块替身） | 全章节 |

### `packages/desktop/src/components/agent/agent-session-view.tsx` -> `packages/desktop/src/components/agent/agent-session-view.test.tsx`

**既有测试文件新增**：`askAnswer` 两形态同透传、不自动接线（DD7）。既有自持 / 注入 / `composer` 插槽 / 空态 / 状态重评估用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 agent-session-view 行）；上述条目取自「类型定义」表的 `AgentSessionViewProps` 行与「修改文件」表该文件行（两形态同透传、**不自动接线**，DD7）。 -->
#### 待测功能

- `AgentSessionViewProps.askAnswer`（新增可选动作面，`AskAnswerActions`）: 自持（`root` + `sessionId`）与注入（`session`）两形态同透传 `AgentMessages`；**不自动接线**（缺席即全静态——不自行用 `session.answer` 组装，DD7）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentSessionViewProps.askAnswer` | 正向 | 接口面可选（AC-11）：省略与给值两形态下 `AgentSessionView` 均可渲染（组件 props 类型面）；给值时下游收到同一对象（判据经探针读 `propsLog[n].askAnswer === <注入对象>`——引用相等不可由 DOM 或行为反推） | 参数化迭代（2 形态） |
| `AgentSessionView.askAnswer` | 正向 | 注入形态透传（AC-11）：`session` 注入 + `askAnswer` 在场 + ask 部件 `input-available` → `AgentMessages` 收到同一对象（探针 `propsLog` 引用比较 + 活卡在场双判据），`onAnswer` 点击可达 | 单次 |
| `AgentSessionView.askAnswer` | 正向 | 自持形态透传：`root` + `sessionId`（无 `session`）+ `askAnswer` 在场 → 活卡同样生效（两形态同透传，spec「两形态同透传」） | 单次 |
| `AgentSessionView.askAnswer` | 异常 | 不自动接线（DD7 / AC-13）：自持形态**未传** `askAnswer`（即便 `session.answer` 在 hook 层存在）→ ask 卡全静态（零按钮零输入）、`agent_answer` 零 invoke（历史重放零应答入口的核心判据） | 单次 |
| `AgentSessionView.askAnswer` | 异常 | 注入形态未传：同上（静态）；`composer` 插槽仍在位（两个可选动作面互不前置） | 单次 |
| `AgentSessionView.askAnswer` | 边界 | 两个动作面独立：`composer` 只给 + `askAnswer` 不给 → 静态卡 + composer 可用；`askAnswer` 只给 + `composer` 不给 → 活卡 + 无 composer（只读消费点可只接 ask 应答） | 组合迭代（2 组合） |
| `AgentSessionView.askAnswer` | 边界 | `pendingToolCallId` 直通：`askAnswer.pendingToolCallId` 取 `null` / 不匹配串 / 匹配串三形态 → 下游判活结果与 `agent-messages` 章节同（本层零改写，直通判据） | 参数化迭代（3 形态） |
| 既有会话面 | 异常（零改动回归） | 自持 / 注入两形态的取数、订阅、`composer` 插槽、空态与错误面、状态重评估用例零改动全绿 | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 自持形态行 |
| 不 mock（实现保真）：`AgentMessages` | 真实下游（透传是否发生是被测面）；**实现不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`AgentMessages`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/agent-messages', async (importOriginal) => ({ ...(await importOriginal()), AgentMessages: Probe }))`；`Probe` 把 props（含 `askAnswer`）push 进 `vi.hoisted` 的 `propsLog` 后原样渲染真实组件 `<Real {...props} />`——`importOriginal` **保留真实实现**（气泡 / 判活 / 虚拟窗口照旧），探针零行为、只记录 | 「接口面给值时下游收到同一对象」与「注入形态透传」两行（`propsLog[n].askAnswer` 引用比较）；自持 / 注入两形态各一条记录（两形态同透传判据） |
| 不 mock：`useAgentSession` | 注入形态下由用例直接给 `UseAgentSessionState` 形状的字面量；自持形态真实 hook | 两形态行 |

### `packages/desktop/src/components/agent/session-panel.tsx` -> `packages/desktop/src/components/agent/session-panel.test.tsx`

**既有测试文件新增断言**（既有用例零改动）：`askAnswer` 骨架合成。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 session-panel 行）；上述条目取自「修改文件」表该文件行（`ExpandedBody` 从 `state.session.answer` 合成 `askAnswer`，`SessionPanelState` 形状零改动）。 -->
#### 待测功能

- `SessionPanel` 的会话体（`ExpandedBody`）: 从 `state.session.answer` 合成 `askAnswer={{ onAnswer: state.session.answer }}` 下传 `AgentSessionView`（`SessionPanelState` 形状零改动——`session` 字段本就是 `UseAgentSessionState`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionPanel` 骨架合成 | 正向 | 合成下行（AC-13）：`state.session.answer` 为 `vi.fn()` + ask 部件 `input-available` → 活卡在场；点选项 → 该 `answer` 恰一次以选项文本调用（archi / change-chat 两面板零改动即得活卡） | 单次 |
| `SessionPanel` 骨架合成 | 正向 | 引用等价（DD12）：经探针读 `propsLog` —— `askAnswer.onAnswer === state.session.answer` 且两次渲染的 `askAnswer` 同引用（不额外包装、不新造对象击穿 memo）；**本判据只能经「`AgentSessionView` 保留真实实现 + 记录 props」的探针观测**（DOM 与调用次数反推不出引用相等） | 单次 |
| `SessionPanel` 骨架合成 | 异常 | 不注入 `pendingToolCallId`（DD8 三态语义）：合成对象只有 `onAnswer` → 判活按 `undefined` 分支（未配对即活），与 `agent-messages` 章节的 `undefined` 行一致 | 单次 |
| `SessionPanel` 骨架合成 | 边界 | `SessionPanelState` 形状零改动（回归）：既有 testid 前缀 / 头区动作 / 状态行 / 历史清单 / 折叠窄条 / `composer` 插槽 / 空集合七组用例零改动全绿 | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 全章节 |
| 不 mock（实现保真）：`AgentSessionView` / `AgentMessages` | 真实下游（合成对象是否到位需真实链路）；**实现不替换**，只在 `AgentSessionView` 边界包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`AgentSessionView`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/agent-session-view', async (importOriginal) => ({ ...(await importOriginal()), AgentSessionView: Probe }))`；`Probe` 把 props（含 `askAnswer`）push 进 `vi.hoisted` 的 `propsLog` 后原样渲染真实组件 `<Real {...props} />`——`importOriginal` **保留真实实现**，探针零行为、只记录。挂点取 `AgentSessionView`（既有导出面，且它正是合成对象的第一接收方）而非 `AgentMessages` | 「引用等价」行（`propsLog[n].askAnswer.onAnswer` 与 `state.session.answer` 引用比较 + 两次渲染同引用）；「不注入 `pendingToolCallId`」行（`propsLog` 内 `askAnswer` 无该键） |
| 不 mock：`state.session.answer` | 真实 `vi.fn()`（被测注入依赖，不是模块替身；引用比较的右值即它本身） | 全章节 |

### `packages/desktop/src/components/agent/index.tsx` -> `packages/desktop/src/components/agent/index.test.tsx`

**既有测试文件新增断言**：barrel 出口集合扩一项（`AskAnswerActions` 类型再导出）。既有「出口集合与单一引用」/「冒烟渲染」用例改造为含新导出的集合断言（零断言语义漂移）。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无 index.tsx 行）；上述条目取自「修改文件」表该文件行（`export type { AskAnswerActions } from './agent-messages'` 一行类型再导出，proposal 表未列但实现必需）。 -->
#### 待测功能

- barrel 类型再导出（`AskAnswerActions`，一行 `export type`）: 零运行期语义；使流程侧三文件经既有导入惯例取得公共类型，不必改深路径。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskAnswerActions` barrel 出口 | 正向 | 类型出口可达：`import type { AskAnswerActions } from '@/components/agent'` 编译通过（`tsc` 面）；`export * from './agent-messages'` 的既有面不重复导出同名值（类型再导出零冲突） | 单次 |
| `AskAnswerActions` barrel 出口 | 正向 | 出口集合扩位：既有 `index.test.tsx` 的出口集合断言补 `AskAnswerActions` 一项（组件集合不变——只有类型新增） | 单次 |
| `AskAnswerActions` barrel 出口 | 异常 | 单一引用纪律：流程侧三文件（`detail-drawer` / `session-transcript-panel` / `change-detail-view`）经 barrel 引入 `AskAnswerActions`，零深路径 import（源码自证：三文件零 `from '@/components/agent/agent-messages'` 形态的**仅类型**导入；若出现即两套导入形态并存） | 单次 |
| `AskAnswerActions` barrel 出口 | 边界 | 冒烟渲染零漂移：既有「组件族 barrel 冒烟渲染」用例零改动全绿（类型再导出零运行期影响） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | barrel 是纯再导出面；运行期只做组件引用转发 | 全章节 |
| 不 mock：`AgentSessionView` / `SessionPanel` | 真实组件（冒烟渲染行） | 全章节 |

### `packages/desktop/src/views/explores/explore-detail-pane.tsx` -> `packages/desktop/src/views/explores/explore-detail-pane.test.tsx`

**既有测试文件新增断言**：`ChatPane` 的 `askAnswer` 注入（通用侧 `agent_answer`）。既有详情头 / promoted 两态 / title 回填 / autoPrompt / run 终态 / 冒烟等用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无该视图行）；上述条目取自「修改文件」表该文件行（`ChatPane` 注入 `askAnswer={{ onAnswer: session.session.answer }}`）。 -->
#### 待测功能

- `ChatPane`（私有组件，体内变化）: 向 `AgentSessionView` 注入 `askAnswer={{ onAnswer: session.session.answer }}`；`composer` 插槽零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChatPane` | 正向 | 通用侧活卡（AC-13）：explore 会话面 + ask 部件 `input-available` → 活卡在场；点选项 → `invokeMock` 收到 `("agent_answer", { root, sessionId, text })`（走通用侧命令，**不是** `change_flow_answer`） | 单次 |
| `ChatPane` | 正向 | 与 `composer` 插槽并存：`composer`（`ExploreComposer`）在场 + `askAnswer` 在场 → 两者均可用（互不前置） | 单次 |
| `ChatPane` | 异常 | 无会话形态：`session.session` 为 null / 会话未建立 → ask 部件历史形态全静态、零 `agent_answer` invoke（`answer` 内部 null 守卫的另一半） | 单次 |
| `ChatPane` | 边界 | 引用稳定性（DD12）：`session.session.answer` 未变时重渲染 → 下游收到的 `askAnswer` 与上一渲染**同引用**（判据经「`AgentSessionView` 保留真实实现 + 记录 props」的探针读 `propsLog` 做引用比较，DOM 反推不出） | 单次 |
| `ChatPane` 既有面 | 异常（零改动回归） | ChatPane 注入形态 / composer 插槽 / 运行态边界（库态 running 禁发与停止接线）/ 错误面四组既有用例零改动全绿 | 参数化迭代（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 全章节 |
| 不 mock（实现保真）：`useExploreSession` / `AgentSessionView` | 真实 hook 与真实下游（注入是否到位需真实链路）；`AgentSessionView` 实现**不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`AgentSessionView`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/agent-session-view', async (importOriginal) => ({ ...(await importOriginal()), AgentSessionView: Probe }))`；`Probe` 记录 props（含 `askAnswer`）后原样渲染真实组件——`importOriginal` **保留真实实现**（活卡 / 判活照旧），探针零行为、只记录 | 「引用稳定性」行（`propsLog` 两次渲染 `askAnswer` 引用比较）；「通用侧活卡」行（`propsLog[n].askAnswer.onAnswer` 为 `session.session.answer` 同引用，点击后 `invokeMock` 收到 `agent_answer`） |

### `packages/desktop/src/views/agent/agent-debug-view.tsx` -> `packages/desktop/src/views/agent/agent-debug-view.test.tsx`

**既有测试文件新增断言**：调试页通用会话的 `askAnswer` 注入。既有停止入口 / 区域滚动 / agent 选择 / 来源筛选 / 每跑重置 / 流式呈现用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无该视图行）；上述条目取自「修改文件」表该文件行（注入 `askAnswer={{ onAnswer: session.answer }}`）。 -->
#### 待测功能

- 会话面注入（`AgentDebugView` 的 `AgentSessionView` 挂载点，体内变化）: 注入 `askAnswer={{ onAnswer: session.answer }}`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentDebugView` 会话面注入 | 正向 | 调试页可应答（AC-13）：`source` 缺省（debug）会话 + ask 部件 `input-available` → 活卡；点选项 → `("agent_answer", { root, sessionId, text })` 逐字 | 单次 |
| `AgentDebugView` 会话面注入 | 正向 | 与 `showRaw` 切换互不干扰：`showRaw = true`（原始流视图）时无 ask 卡（不经 `AgentSessionView`）；切回 `false` 后活卡在场 | 单次 |
| `AgentDebugView` 会话面注入 | 异常 | 会话未建立（`session.session === null`）→ 全静态、零 invoke | 单次 |
| `AgentDebugView` 会话面注入 | 边界 | 引用稳定性（DD12）：`session.answer` 稳定 → 两次渲染下游收到的 `askAnswer` 同引用（判据经「`AgentSessionView` 保留真实实现 + 记录 props」的探针读 `propsLog` 做引用比较） | 单次 |
| 既有调试页 | 异常（零改动回归） | 停止入口与收敛呈现 / 区域滚动框架 / agent 选择 / 来源筛选 / 每跑重置 / 流式呈现 / configSnapshot fixture 七组用例零改动全绿 | 参数化迭代（7 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 全章节 |
| 不 mock（实现保真）：`useAgentSession` / `AgentSessionView` | 真实 hook 与真实下游；`AgentSessionView` 实现**不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`AgentSessionView`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/agent-session-view', async (importOriginal) => ({ ...(await importOriginal()), AgentSessionView: Probe }))`；`Probe` 记录 props（含 `askAnswer`）后原样渲染真实组件——`importOriginal` **保留真实实现**，探针零行为、只记录 | 「引用稳定性」行（`propsLog` 两次渲染 `askAnswer` 引用比较）；「调试页可应答」行（`propsLog[n].askAnswer.onAnswer` 为 `session.answer` 同引用，点击后 `invokeMock` 收到 `agent_answer`） |

### `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` -> `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx`

**既有测试文件新增 + 改造**：自持形态「零动作面」断言改为「默认零动作面、注入后单点例外」；追加 `askAnswer` 透传用例组。既有面板接线 / 空态与错误 / `refreshKey` 三组用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无该视图行）；上述条目取自「修改文件」表该文件行（增可选 prop `askAnswer`，在场时并入 `viewProps` 透传；其余「不接动作面」纪律不变——ask 应答的单点例外）。 -->
#### 待测功能

- `SessionTranscriptPanelProps.askAnswer`（新增可选 prop，`AskAnswerActions`）: 在场时并入 `viewProps` 透传 `AgentSessionView`（其余「不接动作面」纪律不变——它是 ask 应答的**单点例外**）；缺席即全静态（历史重放零应答入口）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionTranscriptPanelProps.askAnswer` 透传 | 正向 | 注入即透传（AC-13）：`askAnswer = { onAnswer }` + ask 部件 `input-available` → `AgentSessionView` 收到**同一对象**（判据经探针读 `propsLog`：`propsLog[n].askAnswer === <注入对象>`，DOM 与活卡呈现反推不出引用相等）、活卡在场；点选项触发 `onAnswer` | 单次 |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 正向 | `pendingToolCallId` 直通：`askAnswer = { onAnswer, pendingToolCallId: "call_2" }` → 下游按该 id 精确对账（`call_1` 静态、`call_2` 活） | 单次 |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 正向 | **双 ask 对账（下游 test-gen 补，G3 骨架）**：同一会话转录含两个 `input-available` 的 `AskUserQuestion` 部件（`call_1` / `call_2`）+ `askAnswer.pendingToolCallId = "call_2"` → 仅 `call_2` 为活卡（按钮 / 输入框在场）、`call_1` 为静态卡（零按钮零输入）；点 `call_2` 的选项 → `onAnswer` 恰一次且参数为 `call_2` 的选项文本（不串卡）。`pendingToolCallId` 切到 `"call_1"` 后两卡活性互为镜像（第二条断言） | 组合迭代（2 指向 × 2 卡） |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 异常 | 未注入即全静态（AC-13 关键判据）：不传 `askAnswer` → ask 卡零按钮零输入、`change_flow_answer` / `agent_answer` 双命令零 invoke（历史重放零应答入口） | 单次 |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 异常 | `composer` 仍不可注入：本面板 `viewProps` 无 `composer` 键（只读纪律不因新增 ask 例外而松动）——源码/渲染双向证（渲染面无发送输入框） | 单次 |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 边界 | 既有 `viewProps` 三项（`root` / `sessionId` / `refreshKey`）与 `data-testid` 不因新增键而改变：注入与不注入两形态的既有断言逐条相等 | 参数化迭代（2 形态） |
| `SessionTranscriptPanelProps.askAnswer` 透传 | 边界 | `roleRef.sessionId === null`（此步无会话记录）：仍走空态分支（`drawer-session-empty`），`askAnswer` 在场也不渲染卡 | 单次 |
| 既有面板面 | 异常（零改动回归） | 面板接线与元信息 / 空态与错误 / `refreshKey` 状态重评估三组用例零改动全绿 | 参数化迭代（3 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 全章节 |
| 不 mock（实现保真）：`AgentSessionView` / `AgentMessages` | 真实下游（透传与判活需真实链路）；`AgentSessionView` 实现**不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`AgentSessionView`（保留真实实现，仅记录 props） | `vi.mock('@/components/agent/agent-session-view', async (importOriginal) => ({ ...(await importOriginal()), AgentSessionView: Probe }))`；`Probe` 把 props（含 `askAnswer`）push 进 `vi.hoisted` 的 `propsLog` 后原样渲染真实组件——`importOriginal` **保留真实实现**（活卡 / 判活照旧），探针零行为、只记录 | 「注入即透传」行（`propsLog[n].askAnswer` 与注入对象引用比较）；「`pendingToolCallId` 直通」行（`propsLog[n].askAnswer.pendingToolCallId === "call_2"`）；「未注入即全静态」行（`propsLog` 各条 `askAnswer` 恒 `undefined`） |

### `packages/desktop/src/views/changes/flow/detail-drawer.tsx` -> `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx`

**既有测试文件新增断言**：`askAnswer` 纯 prop 透传链（`DrawerContent` → `SessionTranscriptPanel`）。既有 drawer 选中 / 材料 / 关闭用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无该视图行）；上述条目取自「修改文件」表该文件行（增可选 prop `askAnswer` 并原样透传 `SessionTranscriptPanel`）。 -->
#### 待测功能

- `DetailDrawerProps.askAnswer`（新增可选 prop，`AskAnswerActions`）: 原样透传 `DrawerContent` → `SessionTranscriptPanel`（零加工、零包装）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DetailDrawerProps.askAnswer` 透传 | 正向 | 三层原样透传（AC-13）：`DetailDrawer` 传入 `askAnswer` → 会话转录面板活卡在场；点选项 → `onAnswer` 收到选项文本（透传链逐层零改写）；「同一对象」半边经探针读 `propsLog[n].askAnswer === <注入对象>`（引用相等不可由 DOM 反推） | 单次 |
| `DetailDrawerProps.askAnswer` 透传 | 正向 | `pendingToolCallId` 同链透传：以 `"call_2"` 注入 → 渲染为「仅 `call_2` 活」（三态矩阵跨两层保持） | 单次 |
| `DetailDrawerProps.askAnswer` 透传 | 异常 | 未传形态（默认）：历史 drawer 调用形态零改动 → 全静态；既有 drawer 用例零改动即证 | 单次 |
| `DetailDrawerProps.askAnswer` 透传 | 边界 | 非会话选中对象：`selection.scope !== 'node'` 或 `roleRef` 为 null → 不渲染会话面板，`askAnswer` 零消费（无卡可应答） | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界） | 全章节 |
| 不 mock（实现保真）：`SessionTranscriptPanel` | 真实下游（透传是被测面）；实现**不替换**，只在其上包一层 props 探针（见下条） | 全章节 |
| **探针装置**：`SessionTranscriptPanel`（保留真实实现，仅记录 props） | `vi.mock('@/views/changes/flow/session-transcript-panel', async (importOriginal) => ({ ...(await importOriginal()), SessionTranscriptPanel: Probe }))`；`Probe` 记录 props（含 `askAnswer`）后原样渲染真实组件——`importOriginal` **保留真实实现**，探针零行为、只记录 | 「三层原样透传」行（`propsLog[n].askAnswer` 与注入对象引用比较 + `onAnswer` 同引用）；「`pendingToolCallId` 同链透传」行（值直通而非重建对象） |
| 不 mock：`FlowGraph` / `FlowMaterials` 装置 | 真实结构体直构（既有 fixture） | 全章节 |

### `packages/desktop/src/views/changes/change-detail-view.tsx` -> `packages/desktop/src/views/changes/change-detail-view.test.tsx`

**既有测试文件新增 + 组合用例组**：`DetailLoaded` 的 `askAnswer` 组装（`onAnswer` → `run.answer`、`pendingToolCallId` → `activeRun?.ask?.toolCallId ?? null`）与双端同步组合用例（**流程侧链最上层调用方 = 本文件**）。既有 detail 取数 / tabs / drawer / 材料 / 归档 preflight 用例零改动。


<!-- design.md「公共函数 / API」表未声明本文件的公共函数变更（该表无该视图行）；上述条目取自「修改文件」表该文件行与 DD13（`DetailLoaded` 组装 `askAnswer`：`onAnswer` → `run.answer`、`pendingToolCallId` → `activeRun?.ask?.toolCallId ?? null`）。 -->
#### 待测功能

- `DetailLoaded` 的 `askAnswer` 组装（体内变化，props 链同文件扩位）: `onAnswer: (text) => { void run.answer(text); }`（DD13——`run.answer` 内部即 `ipc.changeFlowAnswer`，且已把 reject 落 `run.error`）；`pendingToolCallId: activeRun?.ask?.toolCallId ?? null`；经 `DetailTabs` → `DetailContent` → `DetailDrawer` 下行。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DetailLoaded` 的 `askAnswer` 组装 | 正向 | 流程侧命令与参数（AC-13 / DD13）：`activeRun.ask.toolCallId = "call_2"` + 抽屉选中该步会话 → 活卡在场；点选项 → `invokeMock` 收到 `("change_flow_answer", { root, id, answer })` 逐字（**不是** `agent_answer`） | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 正向 | `pendingToolCallId` 取自统一视图：`activeRun.ask.toolCallId` 变化 → 下游判活目标随之变化（两 ask 部件场景仅当前者活）；`activeRun.ask` 为 null → `pendingToolCallId === null`（全静态） | 组合迭代（3 形态：匹配 / null / 不匹配） |
| `DetailLoaded` 的 `askAnswer` 组装 | 正向 | 错误面归因（DD13）：`change_flow_answer` reject → `run.error` 落值并经 `RunControlPanel` 既有错误行呈现（应答被拒可归因，不是静默） | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 + `AskCard`（入口 = 本文件） | 组合 | 双端同步（AC-2）：通道推入 `RunUpdate::Ask { ask }` → 详情页 `AskCard`（`run-ask-card`）与聊天窗活卡**同时**呈现同一 payload；经 `AskCard` 按钮应答 → `change_flow_answer` → 推入 `RunNotice::Step` + 快照 `ask == null` → **两处同时收起**、`activeRun.status` 回 `running` | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 异常 | 无 activeRun（历史重放 / 已收口）：`activeRun === null` → `pendingToolCallId === null` 且抽屉会话全静态；历史未配对 `tool_use` 不呈活卡（AC-13「只读历史重放零应答入口」） | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 异常 | 抽屉未选中任何节点：`askAnswer` 组装完成但下游零消费（零卡、零 invoke）；选中会话后恢复正常 | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 边界 | props 链不变形：`DetailTabs` / `DetailContent` / `DetailDrawer` 三层同名透传（无改名、无包装对象）；既有三层其余 props（`selection` / `graph` / `materials` / `root` / `changeId` / `name` / `sessionRefreshKey` / `onClose`）逐字不变 | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 边界 | `run.answer` 引用稳定性（DD12）：同一 `run` 下两次渲染 → 下游收到的 `onAnswer` **同引用**（判据经「`DetailDrawer` 保留真实实现 + 记录 props」的探针读 `propsLog` 做引用比较：`propsLog[0].askAnswer.onAnswer === propsLog[1].askAnswer.onAnswer`——DOM 与调用次数反推不出引用相等；`useCallback` 纪律，避免击穿 memo）；`activeRun` 变化时不导致 `onAnswer` 重建（`onAnswer` 只依赖 `run.answer`） | 单次 |
| `DetailLoaded` 的 `askAnswer` 组装 | 边界 | 应答正文边界：空串 / 纯空白在提交侧已拦（活卡与 `AskCard`）；`onAnswer` 收到超长文本（1200 字符）时逐字透传（本层零 trim） | 参数化迭代（2 形态） |
| 既有 detail 面 | 异常（零改动回归） | detail 取数 / tabs 切换 / drawer 选中寻址 / 材料拼装 / 归档 preflight / 通知重查等既有用例零改动全绿（`AskPayload` 两新字段 additive 不影响既有断言） | 参数化迭代（既有用例全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（既有 `invokeMock` + `ChannelMock`） | IPC 替身（进程边界；`Channel` 替身可直投 `RunUpdate` / `RunNotice` 信封） | 全章节 |
| `./flow/attachments`（既有 `vi.mock(..., { spy: true })`） | 磁盘材料拼装是**文件系统边界**，既有装置即 spy 观测面 | 既有材料行 |
| 不 mock（实现保真）：`useChangeFlowRun` / `RunControlPanel` / `DetailDrawer` | 真实实现（`run.answer` 与 `AskCard` 是双端同步的另一端）；`DetailDrawer` 实现**不替换**，只在其上包一层 props 探针（见下条） | 组合行 |
| **探针装置**：`DetailDrawer`（保留真实实现，仅记录 props） | `vi.mock('@/views/changes/flow/detail-drawer', async (importOriginal) => ({ ...(await importOriginal()), DetailDrawer: Probe }))`；`Probe` 把 props（含 `askAnswer`）push 进 `vi.hoisted` 的 `propsLog` 后原样渲染真实组件 `<Real {...props} />`——`importOriginal` **保留真实实现**（抽屉内容 / 会话面板照旧），探针零行为、只记录 | 「`run.answer` 引用稳定性」行（`propsLog` 两次渲染 `askAnswer.onAnswer` 引用比较）；「应答正文边界」行（`propsLog[n].askAnswer.onAnswer === run.answer`）；「`pendingToolCallId` 取自统一视图」行（`propsLog[n].askAnswer.pendingToolCallId` 值直通） |
| 不 mock：`ipc.changeFlowAnswer` 的 id 来源 | 真实 URL id（页内单一身份源） | 命令参数行 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 一行） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（落地时 `HEAD` 现值 +1 patch，MUST NOT 写死字面号——design 提「现值 `0.6.35` → `0.6.36`」但禁实现写死） | 桌面端 diff 审查：`git diff HEAD -- packages/desktop/package.json` 的 `version` 行恰 +1 patch；`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行未动 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（生成物） | 由 `bindings:export` 产出、**禁手改**；写成测试会与生成链重复并随排版漂移 | `pnpm run bindings:export` 后 `git diff --stat -- src/types/generated` 为空（桌面端执行）；形状判据在 `src/bindings/mod_test.rs` 章节（产物快照读盘） |
| `packages/desktop/src-tauri/src/main.rs`（`app.manage(Arc::new(AskRegistry::default()))` 与 `ChangeFlowControl::new(…)` 两行） | `setup_hook` 是**进程启动路径**，`main.rs` 是 bin 目标、不参与 `cfg(test)`（既有仓库无 `main_test` 先例）；托管态与构造面在功能等价层面由各命令测试的 `mock_app` 装置覆盖（同一 `manage` 调用形态） | `cargo clippy --all-targets` 编译闭合（`ChangeFlowControl::new` 换参必编译失败即兜住）+ 各命令测试 `app_with_stores` 装置的同形态 `manage` 调用 + 桌面端 `grep` 复核两行在位 |
| AC-8 的**退役面零残留**（`answer_tx` 槽 / `wait_answer()` 在产品 `.rs` 零命中） | 源码位置事实；运行期只能覆盖被驱动的路径，覆盖不到「某分支不存在」这类**缺席**事实。`control_test.rs` / `walker_test.rs` 的自证用例各只判本文件，跨文件零命中须 `grep` | 桌面端 `grep -rn "answer_tx\|wait_answer" crates/ src/`（仅允许命中 `*_test.rs` 的断言串）+ 运行期侧证 = `control_test` 退役面自证行 + `walker_test` 的 `decision_session` 体内零 `loop` / 零 `continue` 行 |
| AC-12 的 **`decision_answer_prompt` 全仓零命中** | 同上（跨 crate / 跨文件缺席事实） | 桌面端 `grep -rn "decision_answer_prompt" crates/ src/`（仅允许命中 `prompt_test.rs` 的退役断言串）+ `prompt_test` 退役面自证行 + 编译期（`walker.rs` 去掉 `use` 后若仍有调用必 `E0425`） |
| AC-4 的**跨文件一致性半边**（`ask.rs` 零 `finish` / `finish.rs` 的 `ask` 只在文案 / `mount.rs` 两段 `if` 无合取） | 各自文件内的自证用例已覆盖（`ask_test` 全文判 + `finish_test` 剔除字符串后代码行判 + `mount_test` 无合取判）；**跨文件**那一层不可单测——`ask.rs` 不知道 `finish.rs` 存在，反之亦然，一个测试无法同时断两个模块互不认识 | 三处自证用例组合即等价（各自断言 + 双向零引用）+ 编译期：任一交叉引用必引入显式 `use`，`git diff` 可观察 |
| `crates/infra/agent/src/sdk/mount.rs` 的 **provider 侧不变量 doc** 与 `loop.rs` / `runner.rs` 的执行体文档注释（「`core_session_id` 与 `LoopTurn.session_id` 同名不同义」等） | 注释文本，零运行期路径 | 桌面端 diff 审查（doc 行随产品文件变更）；行为判据在 `loop_test` 的 `Flow` 臂行（`session_id` == `AskContext.core_session_id` 且 != `LoopTurn.session_id`） |
| `crates/core/orchestration/src/decision.rs` 的 `DecisionAction::Ask` **doc 注释**（「生产不可达」说明）与 `walker.rs` 两处防御文案措辞 | 注释 / 文案文本；`decision_test` 只能断言关键字**存在**，断不了措辞完整性 | `decision_test` 文档面自证行 + 桌面端 diff 审查 |
| `ChangeFlowControl::accept_pending` / `register_pending` / `confirm()` / `wait_confirm()` / `cancelled_wait` 的**逐字未改**（D6 机械判据） | 「零 diff」是结构事实，运行期无法断言「这段代码没被改」；`confirm` 四条既有用例全绿只能证行为等价，不能证逐字未改 | 桌面端 `git diff` 逐块比对（design 明列该五标识符为「越界面」）+ `control_test` 的 confirm 四条零改动全绿（行为侧证） |
| `.dev-team/architecture/models/**` 与 `openspec/specs/**` 的**零 diff** | 「正确性主张」是这些文件一行不变 | 桌面端 `git diff --stat` 逐项审查；行为侧证 = 零新 crate / 零新依赖边（`Cargo.toml` 五处零 diff） |
| `crates/infra/{store,exec,vcs,watch}/**`、`crates/core/{workflow,checks,config}/**`、`packages/desktop/src/lib/{agent-adapter,agent-transport}.ts`、`src/components/agent/agent-blocks.tsx`、`views/changes/flow/run-control-panel.tsx` 的**零改动**（design「不要修改」列表） | 均为「一行不变」的结构性主张（停等态仍不落库、14 份 prompt md 零改动、`AskCard` 零改动、适配层零改动） | 桌面端 `git diff --stat` 逐文件审查；行为侧证 = 相应既有测试零改动全绿（`resume_test` / `agent-blocks.test.tsx` / `run-control-panel.test.tsx` / `tools_test` / `protect/**` 各自既有用例） |
| AC-13 的 **archi 面板 / change-chat 面板零改动即得活卡** | 两面板源码零 diff；行为由 `session-panel.tsx` 的骨架合成覆盖（同一 `SessionPanel` 组件在两处复用） | `session-panel.test.tsx` 骨架合成三行 + 桌面端 `git diff` 确认 `views/archi/**` 与 `views/changes/agent/**` 零改动 |
| AC-9 的 **`AskMode` 不出线 IPC**（不含 `specta::Type` 派生） | 派生面是源码事实，产物零命中是生成物事实，二者已由源码自证与产物快照用例覆盖；「未来被误接出线」这一**未来态**无法运行期断言 | 两处自证用例组合 + 桌面端 `bindings:export` 后 diff 审查；若 `AskMode` 被误加到命令参数面，`bindings/mod_test` 反向锚定断言即红（遍历产物查类型名） |
| 编译闭合批的**纯机械点位**（`compose_turn` 46 处 / `ChangeFlowControl::new` 254 处 / `SessionKernel::new` 47 处 / `SessionOpen` 19 处 / `SessionInjections` 32 处 / `LoopTurn` 约 40 处 / `RunUpdate::Ask` 21 处 / `AskPayload` 2 处 / `ChangeFlowControl::default` 9 处） | 只补字段 / 只加实参、只求可编译；design 明令**不新增用例、不改断言值、不改用例名**。缺字段即 `E0063` / `E0061`，无运行期中间态 | `cargo check --workspace --all-targets` 通过（`git diff --stat` 对这些文件显示为纯新增行）；各章节的「↔ 编译闭合」行只登记计数与点位 |
| `crates/core/agent/src/ask.rs` 的 `#[derive(Default)]` 与 `new()` 的**等价性来源**（`new` 是 `Self::default()` 薄包装） | 实现细节；`ask_test` 的等价性断言已覆盖行为，但「`new` 体内恰一行 `Self::default()`」是源码事实 | `ask_test` 等价性行 + 桌面端 diff 审查（`new` 体内除 `Self::default()` 外无第二个构造路径） |
| `Flow` 档停止路径的**外层 drop 语义**（外层泵 `select!` drop 整个 loop future、不执行清理代码） | 跨层任务调度语义；`loop_test` 只能观测「执行体内停止臂命中 → `ask_resolved` 恰一次」，观测不到「若没有显式停止臂，外层 drop 会跳过清理」这一**反事实** | `loop_test` 停止臂行（正）+ 桌面端源码审查（`ToolExec::Ask(Flow)` 臂必带 `handle.wait_requested()` 分支）+ proposal D11 / R4 的设计留痕 |
| 真机 E2E（真实 provider 端点下的 ask 往返） | proposal 明写「验收止于单元测试；真机 E2E 不属本工作流自动化验证面」 | `loop_test` 的流缝脚本化四条路径（阻塞 → 应答回灌 → 同 turn 续跑 / 停止截停 / `Flow` 覆盖清态 / 上下文缺席回灌）即本变更的全部行为面 |
| 机械门禁（`cargo clippy --all-targets` / `pnpm run check` 四段链 / `vp test` 全量 / `knip`） | 机械门禁步骤，由桌面端 static-check / test-execution 相位执行；在设计里复述脚本细节等于复制运行脚本 | 门禁执行结果 + 本设计各章节对覆盖率的支撑（断言全部落在真实契约面：take 语义 / 双档执行体 / 三段判定 / 条件清理 / 三态判活，不为覆盖率造句） |
