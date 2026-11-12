# 设计: ask-tool-in-flow

> **变更**: ask-tool-in-flow
> **状态**: 设计定稿（基于已定稿 proposal + 4 份 delta spec）
> **基于**: `proposal.md`（问题 / 提案 / D1–D14 / 三层改动 / 变更范围 / AC-1…AC-13 / R1–R10）、`explore.md`（只读结论稿）与 `specs/{desktop-agent-execution,desktop-change-orchestration,desktop-change-flow-view,desktop-agent-chat-infra}/spec.md`
> **性质**: ask 由「`finish_step` 的一个枚举值」升为**挂载清单里的独立阻塞工具**；应答通道由 `ChangeFlowControl.answer_tx` 下沉为**会话级注册表** `AskRegistry`。调用面加一条 IPC 命令（`agent_answer`），`AskPayload` additive 扩两字段，其余落库面 / store schema / 工具执行体零变更。
>
> **本篇边界**: `core/agent`（新注册表 + 两个词汇位）、`core/orchestration`（ask 工具协议 + 应答通道转投 + 决策协议收窄）、`infra/agent`（挂载项 + 两档执行体 + 装配）、Tauri 壳（注册表托管 + 一条新命令 + `AskPort` 实现）、前端（一条 hook 动作 + 双态卡 + 七处接线）+ 版本一行。**零新增 crate、零新增依赖边、零架构模型改动**（`infra/agent` 已依赖 `core/agent` 与 `core/orchestration`）；**零 store 改动**（停等态仍不落库）；**零新出线类型**（`AskMode` / `SessionInjections` 不出线，反向锚定守）；`AskPayload` 两字段随统一视图自动透出。

---

## 提案与规格同步状态

proposal 的三层改动（① 会话地基 / ② 流程覆盖 / ③ 禁用位只建不接线）、D1–D14 十四条裁决与 4 份 delta spec 均已定稿。本设计不重复其论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名**（见「关键设计决策」DD1–DD15）：`AskRegistry::register` 返回值的语义、`Flow` 档在 `ask_port` 缺席时的行为与 change-chat 的 port 注入、`ChangeFlowControl` 的单构造面 / `answer` 三段判定与锁序 / `resolve_ask` 清理条件、`AskContext` 与两个 session id 的隔离、决策会话去循环化的两处文案、`pendingToolCallId` 三态判活矩阵、ask 卡双形态解析、前端 `answer` 的错误面合并与引用稳定性。
2. **把 proposal「变更范围」的机械计数按工作树实测校准**并留痕（见「起点核实」）：`SessionKernel::new` 47 处、`SessionInjections` 字面构造 32 处、`SessionOpen` 字面构造 19 处、`LoopTurn` 字面构造约 40 处、`compose_turn` 调用 46 处、`ChangeFlowControl::new` 254 处、`RunUpdate::Ask` 字面构造 21 处。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（测试由独立相位承接）。
4. **把三处「非显然正确」写成可实现形态并留痕**：`accept_pending` / `confirm` 侧的逐字不动边界（D6）、`orchestration/ask.rs` 与 `finish.rs` 的机械解耦判据、`Flow` 档停止臂是清理唯一落点的原因（外层 drop 不执行清理代码）。

---

## 起点核实（工作树实测；路径口径见下表，锚点为改动前行号）

- **core/agent**：`runner.rs:41-53` `FinishSpec { EvaluatorChecklist, Decision }`（`Copy + Eq`、camelCase、不派生 `specta::Type`）；`runner.rs:55-69` `SessionInjections { preamble, tools, finish }`（`Default + PartialEq + Eq`，无 `specta::Type`）；`runner.rs:106-118` `SessionOpen { injections, ctx, session, prior_handle }`（**无 session id**——本变更新增 `session_id` 的唯一缝）；`kernel.rs:17-55` `StopRegistry`（`Mutex<HashMap<String, RunHandle>>`，`register` / `request_stop` / `remove`——`AskRegistry` 的同位形态先例）；`kernel.rs:122-203` `SessionKernel::new(sink, registry, streams)` + `begin_turn` 内 `let session_id = …` **先于** `open_session`；`kernel.rs:338-374` `converge`（`registry.remove` 在收口单点）；`lib.rs:14-28` 出口面。
- **core/orchestration**：`control.rs:47` `answer_tx: Mutex<Option<oneshot::Sender<String>>>`；`control.rs:227-238` `answer()` → `accept_pending`；`control.rs:288-311` `accept_pending`（**泛型模板**，`confirm` 共用）；`control.rs:378-400` `wait_confirm` / `wait_answer`；`control.rs:179-185` `publish` 的 `RunUpdate::Ask` 臂；`state.rs:203-209` `AskPayload { question, options }`；`state.rs:275-300` `RunUpdate::Ask { question, options }`；`port.rs:635-637` `RunEventSink`；`finish.rs:96-123` decision 形态 `parameters`（`enum: ["backtrack","stop","ask"]` + `question` / `options` 两 property）；`finish.rs:41-45` decision 形态 `description`（三动作）；`decision.rs:12-22` `DecisionAction` 三变体；`decision.rs:61-100` `validate_decision`（`"ask"` 臂在 `:81-95`）；`prompt.rs:121-125` `decision_prompt` 三选一段；`prompt.rs:133-139` `decision_answer_prompt`；`walker.rs:3246-3345` `decision_session`（`loop` + `Ask` 分支 `:3289-3304` + `wait_answer`）；`walker.rs:3222-3228` `resolve_deadlock` 的 `DecisionAction::Ask` 防御臂。
- **infra/agent**：`sdk/mount.rs:53-62` `ToolExec` 四臂；`sdk/mount.rs:79-114` `registry()` 十项（`OnceLock`）；`sdk/mount.rs:128-159` `definitions()`（`finish` 臂在 `:147-157`）；`sdk/mount.rs:337-354` 测试夹具 `injected` / `injected_finish`；`sdk/loop.rs:103-137` `LoopTurn`（`session_id` 是引擎侧 `sdk-…` id）；`sdk/loop.rs:600-629` `match mount.exec` 四臂；`sdk/loop.rs:537-560` 收口假结果；`sdk/runner.rs:372-464` `session_pump`（`LoopTurn` 铸造单点 `:429-445`）；`lib.rs:42-183` `EngineFacade`（六条 builder 缝，`with_explore_note_store` 为同型先例）；`compose.rs:39-103` `assemble_injections`（explore / archi / change / 其余四臂 + `injected` 成员校验）；`compose.rs:188-241` `compose_turn`。
- **壳层**：`src/main.rs:101-127` `setup_hook`（`StopRegistry` / `SessionStreamRegistry` / `ChangeFlowControl` 三层同址托管）；`src/commands/exec/mod.rs:92-167` `agent_start_with`（注册表 `Arc` 取出点）；`src/commands/change_flow/mod.rs:52-66` `ChangeFlowSink` + `RunEventSink` impl；`:144-158` `compose_turn` 与 sink 的装配序（**sink 在 compose 之后创建**——本变更需换序）；`:256-270` `change_flow_answer_with` → `control.answer`。
- **前端**：`components/agent/tool-cards.tsx:26-32` `AgentToolCardProps`（五字段，无动作面）；`:58-115` `parseAskInput` / `AskUserQuestionCard`（**只认 SDK 原生 `{questions:[…]}` 形态**，文案含「请在下方输入框作答」）；`components/agent/agent-messages.tsx:30-37` `AgentMessagesProps`；`:51-76` `ToolPartView`；`:182-218` `ChatBubble`（`memo`，入参仅 `message`）；`components/agent/agent-session-view.tsx:22-34` `AgentSessionViewProps`；`components/agent/session-panel.tsx:307-311` `AgentSessionView` 挂载点；`hooks/use-agent-session.ts:42-56` `UseAgentSessionState`；`:536-597` `useSessionActions`（`sendMessage` / `stop` / `reset`）；`:649-658` hook 返回面（`error: replay.error ?? chatError`）；`views/changes/change-detail-view.tsx:341-395` `DetailLoaded` → `DetailTabs` → `DetailContent` → `DetailDrawer`；`views/changes/flow/detail-drawer.tsx:86-124` `DrawerContent`；`views/changes/flow/session-transcript-panel.tsx:27-52`（**自持形态**、零动作面）；`views/explores/explore-detail-pane.tsx:298-314` `ChatPane`；`views/agent/agent-debug-view.tsx:110`。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.35"`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟。

### 事实校准（proposal 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **`SessionKernel::new` 的调用点实为 47 处**（`kernel_test` 42 / `lib_test` 1 / `compose` 2 / `compose_test` 2）。proposal 记「42 处夹具」只计测试面，生产两处（`compose.rs:203`、`:397` 的 `composed_with_runner`）同样随动。
2. **`SessionOpen` 字面构造实为 19 处**（`kernel.rs` 1 生产 / 18 测试：`core/runner_test` 2 / `compose_test` 3 / `lib_test` 6 / `sdk/config_test` 1 / `sdk/runner_test` 6）。proposal 记 28 处，差额是 `SessionOpen {` 之外的同名 grep 命中（`open_session(SessionOpen {` 的行前文）。
3. **`SessionInjections` 字面构造实为 32 处**（`core/runner_test` 9 / `sdk/loop_test` 8 / `sdk/tools_test` 6 / `sdk/runner_test` 4 / `sdk/mount_test` 2 / `kernel_test` 1 / `compose` 1 / `mount` 1）。proposal 记「11 处 loop_test」含两处 `-> SessionInjections` 返回型声明。
4. **`LoopTurn` 字面构造约 40 处**（`sdk/loop_test` 约 39 / 生产 `sdk/runner.rs` 1）。该计数只用来说明改动量：`ask` 字段带 `..loop_turn(...)` 兜底形态的构造点可零改动（`LoopTurn` 不派生 `Default`，故仍需在 helper 内补一处）。
5. **`compose_turn` 调用实为 46 处**（`compose_test` 36 / `worker_test` 8 / `change_flow` 1 / `exec` 1）。
6. **`ChangeFlowControl::new` 调用 254 处**（`control_test` 63 / `walker_test` 188 / `changes/mod_test` 1 / `change_flow/mod_test` 1 / `main.rs` 1）——与 proposal R3b 数字一致。
7. **`RunUpdate::Ask` 字面构造 21 处**（`control` 1 / `control_test` 7 / `state` 2 / `state_test` 3 / `walker` 2 / `walker_test` 1 / `changes/mod_test` 3 / `change_flow/mod_test` 2）。注意 proposal 记「两命令测试 5」，实测 3 + 2 = 5，一致。
8. **`AskPayload` 字面构造只有 2 处**（`control.rs:181`、`state_test.rs:555`）——两新字段加进去后编译器兜住全部点，无运行期静默。
9. **`ChangeFlowSink` 当前在 `compose_turn` 之后创建**（`change_flow/mod.rs:144-158`）：本变更要把 `Arc<dyn AskPort>` 传进 `compose_turn`，故 sink 必须先于组合根构造（同一 `Arc<ChangeFlowSink>` 两次 trait 强转）。这是**装配序调整**，不是新机制。
10. **`AskUserQuestion` 静态卡当前只认 SDK 原生形态 `{questions:[{question,options:[{label,description}]}]}`**：本变更新工具的 schema 是 `{question, options: string[]}`（单问）。若卡只支持新形态，历史转录中的 ask 部件会从「可读卡」退化为 JSON 原样——与 spec「静态态与变更前逐字相同的可读卡形态」冲突，故卡必须**双形态解析**（DD9）。
11. **`bindings/mod_test.rs:428` 的反向锚定清单当前是 `["SessionInjections", "FinishSpec", "TurnQuestion"]`**：`AskMode` 挂在 `SessionInjections` 的字段面上，`SessionInjections` 已在该清单内，故反向锚定**零改动**即覆盖新词汇（前提是 `AskMode` 不派生 `specta::Type`——它不作为命令参数 / 返回出现，派生与否都不出线，但不派生是 fail-closed 的自证）。
12. **`agent_start_with` 已经把 `StopRegistry` / `SessionStreamRegistry` 以 `Arc::clone(state.inner())` 取出**（`exec/mod.rs:132-136`）：`AskRegistry` 沿同一形态加一行，无新装配模式。
13. **change-chat 会话是 `source = "change"` 的「无 run」形态**（`views/changes/agent/hooks/use-change-chat.ts:239-240` 传 `source: 'change'` + `sourceRef: changeId`）：按 spec 的 change 臂置位规则它的 `ask` 位恒 `Some(AskMode::Flow)`（spec 明文「不分 role / 不分矩阵命中」，故不能按段数改成 Generic），但该会话**没有 `ChangeFlowControl` 条目**，`agent_start` 当前也不传 `ask_port`。于是 spec 的「`Flow` 档 `port` 缺席 → `is_error` 回灌」与 AC-13（change-chat 必须可 ask 可答）在**同一份 spec 内互相矛盾**——见 DD15 的解法（不改这两条 spec 条款，改装配面）。

---

## 关键设计决策

> D1–D14 见 proposal「裁决表」（已定稿，本篇不重复）。下表 DD1–DD15 是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | `AskRegistry::register` 的返回值语义 | `register(&self, session_id: &str, tx: oneshot::Sender<String>) -> bool` **恒写入**（同键覆盖），返回值 = 「登记时该键已空」（`true` 无人被顶替 / `false` 顶替了上一问的等待方，旧 `oneshot::Sender` 随覆盖 drop，旧等待方收 `RecvError`）。它是**诊断值，不是失败面**——spec 的「返回是否登记成功」按此读作「是否无上一问被顶替」（登记本身恒成功） | spec 要求 `-> bool` 但未定义语义。若把 `false` 当「登记失败」→ 执行体要 `is_error` 回灌，而「上一问已被放弃」恰恰是 ask 重新提问的正常路径（模型两次调用之间用户没答），回灌会把正常路径写成错误。执行体侧的 `is_error` 面因此只来自**上下文 / `AskPort` 缺席**（DD2） |
| DD2 | `Flow` 档在 `ask_port` 缺席时的行为 | 按 spec 逐字：**不注册、不阻塞，直接 `is_error` 回灌**（文案「ask 编排状态面未装配」）——它是**装配漂移的防御臂**，生产不可达（DD15 保证两条生产轨道恒传 `Some(port)`） | 流程侧「应答从哪来」的 UI 入口由 `AskPort::ask_started` 广播决定（详情页 `AskCard` 读 `activeRun.ask`）。若 port 缺席仍照常注册并阻塞，流程会话在装配漂移时无人能应答 → 会话永久悬挂；`is_error` 至少把「装配坏了」变成模型可见的一行文案。Generic 档 `port` 恒 `None` 是正常形态（通用侧靠聊天窗按「未配对」判活），故该判据只在 `Flow` 档生效 |
| DD3 | `ChangeFlowControl` 的构造面 | 删除 `#[derive(Default)]`，`new(ask_registry: Arc<AskRegistry>) -> Self` 为**唯一构造面** | proposal 明写「`Arc` 共享，MUST NOT 内部自铸——否则与引擎侧登记的表不是同一张」。留着 `Default` 就留着一条「静默铸第二张表」的路径（`Arc<AskRegistry>` 满足 `Default`，`ChangeFlowControl::default()` 会编译通过且行为静默错误）。删 `Default` 即把该错误变成编译错误 |
| DD4 | `answer()` 的三段判定与锁序 | ① `runs.get(key)` 缺席 → `Err("change \"{id}\" 无运行中的 run")`；② 会话槽 `None` → `Err("当前无等待中的 ask")`；③ `Some(session_id)` → **释放 `runs` 锁后**调 `ask_registry.answer(session_id, text)` | ①② 是两种可分辨的事实（无 run vs 有 run 无等待方），spec 要求前者「文案携 change id」、后者含「无等待中的 ask」。锁序取「先读后投」：`runs` 锁与 `AskRegistry` 锁绝不嵌套——引擎侧执行体持 `AskRegistry` 锁（`register`）时可能同步调 `AskPort`（DD5 的 `ask_started` → `control.publish` → 取 `runs` 锁），反向嵌套即死锁 |
| DD5 | `resolve_ask` 的清理条件与广播时点 | 同一把 `runs` 锁内：`ask` 槽**无条件清空**；`status` **仅当恰为 `WaitingAsk` 时**回 `Running`；克隆 `updates` 后**锁外**广播 `RunNotice::Step` | spec「终态不被倒灌」+「`ask` 载荷槽仍被清空」两条同时成立只能这样写：终态已先到时 `status` 不能被改回 `Running`，但行内卡的判活靠 `ask.tool_call_id`，槽不清就永远判「正在等」。锁外广播沿用 `accept_pending` 既有顺序纪律（广播会让订阅方重查统一视图，持锁广播把它们拉进锁竞争） |
| DD6 | 两个 session id 的隔离 | `AskContext { registry: Arc<AskRegistry>, core_session_id: String, port: Option<Arc<dyn AskPort>> }`；`LoopTurn.session_id`（引擎侧 `sdk-…`）**不进注册表**，只用于日志与 `RunStarted` 上报 | 两 id 同名不同义（proposal 已点明）。用**独立字段**而不是复用 `LoopTurn.session_id`：复用会让「注册表键」与「引擎日志标识」在类型上不可分辨，误传后表现为「应答永远 miss」——排查成本极高且无编译期保护 |
| DD7 | 自持形态是否自动接线 `askAnswer` | **不自动接线**：`AgentSessionView` 的 `askAnswer` 恒由来源侧注入；自持形态（`root` + `sessionId`）缺席 `askAnswer` 即全静态 | spec 写「自持形态 **MAY** 由组件以 `session.answer` 自组装」，取「不」。理由：change 流程抽屉与历史重放都是自持形态（`SessionTranscriptPanel`），自动接线会让**历史转录中的未配对 `tool_use` 全部变成可点活卡并发 `agent_answer`**——直接违反 AC-13「只读历史重放零应答入口」与 desktop-change-flow-view「`activeRun.ask` 为 null 时全为静态」 |
| DD8 | `pendingToolCallId` 的三态判活矩阵 | `undefined`（未给出）→ 判活 = `state === 'input-available'`；`null`（明确无等待）→ **全静态**；`string` → 仅 `toolCallId` 相等者活 | 两份 spec 的表面冲突（agent-chat-infra「未给出时按无结果态判活」vs change-flow-view「为 null 时全为静态」）由**三态**消解：`undefined` 与 `null` 语义不同，TS 可选 + 可空正好承载。类型必须写成 `pendingToolCallId?: string \| null`（不可只写 `?: string`），否则流程侧「当前无等待」无法表达 |
| DD9 | ask 卡的输入形态 | `parseAsk(input)` 归一为 `{ question: string; options: AskOption[] }[]`：① 新工具形态 `{ question: string, options: (string \| {label,description})[] }`（`options` 元素为字符串时 `description = null`）；② 历史 SDK 形态 `{ questions: [{…}] }`；③ 两者皆不匹配 → `null`（JSON 原样回退） | 事实校准 10：只认新形态会让历史转录的可读卡退化。双形态归一后渲染体单点（同一 `AskQuestionView`），活卡/静态两态共用同一解析 |
| DD10 | 决策会话去循环化与两处防御文案 | `decision_session` 的 `loop` 删除（单轮直落）：`Ask` 载荷 → `Terminal::failed`；`resolve_deadlock` 的 `DecisionAction::Ask` 臂**结构逐字保留**（仍 `Deadlock::Terminal(Terminal::failed(…))`），文案与注释统一改为「决策漂移：ask 应经 `AskUserQuestion` 工具提交」 | ASK 停等搬进执行体后 `decision_session` 再无 `continue`（无第二轮），保留 `loop` 是僵尸结构。两处文案改写是被 AC-8「`wait_answer` grep 零命中」**强制**的：原注释与文案里都含 `wait_answer` 字样，留着即判据不成立；改动只涉措辞，节点 / 分支形态零变（R5 的「防御臂保留」判据看的是分支在场而非措辞） |
| DD11 | 前端 `answer` 的错误面 | `useSessionActions` 增本地 `actionError` 态；hook 返回面合并为 `error: replay.error ?? chatError ?? actionError`；`answer` 内 `root` / `sessionId` 任一为 `null` → 零 invoke 零副作用 | spec 要求「`Err` 进既有 `error` 面呈现」。既有 `error` 是两个来源的 `??` 链，应答错误是第三源；`??` 链保序（重放 > 流 > 动作）以免一次应答失败遮住更权威的重放错误 |
| DD12 | `askAnswer` 的引用稳定性 | `AgentMessages` 内以 `useMemo` 合成「透传动作面」（依赖 `onAnswer` 与 `pendingToolCallId`）；`MessageContainer` 的 `renderItem` / `renderSticky` 依赖数组纳入该对象。`onAnswer` 侧由来源 `useCallback` 稳定（`session.answer` 本身是 `useCallback`） | `ChatBubble` 是 `memo` 且虚拟列表 `renderItem` 是 `useCallback`：每渲染新造一个对象字面量会同时击穿两者——流式期间每个 delta 都让整棵树重渲染（用户可感知卡顿） |
| DD13 | 流程侧 `onAnswer` 的实现选择 | `onAnswer: (text) => { void run.answer(text); }`（`useChangeFlowRun.answer`，内部即 `ipc.changeFlowAnswer(root, id, text)`） | spec 的示例是裸 `ipc.changeFlowAnswer`；取既有动作面的理由：`run.answer` 已把 reject 落 `run.error`（`RunControlPanel` 既有错误行呈现），裸调则 `Err` 无人接（违反「应答被拒可归因」）。调用面与参数逐字等价，测试对 `ipc.changeFlowAnswer` 的断言不受影响 |
| DD15 | change-chat 的 `Flow` 档如何不撞「port 缺席 → `is_error`」 | **在 exec 轨道也传一个 `AskPort`**：`src/commands/exec/mod.rs` 增私有 `struct ChatAskPort;` + `impl AskPort for ChatAskPort { fn ask_started(&self, _ask: AskPayload) {} fn ask_resolved(&self) {} }`，`agent_start` 以 `Some(Arc::new(ChatAskPort))` 传入组合根 | 事实校准 13 的矛盾（spec 一句话钉「`Flow` 恒为 change 臂的位值」，另一句话钉「port 缺席 → `is_error`」，而 change-chat 恰是「change 臂 + 无 run」）只能在**装配面**消解：给 chat 一个语义正确的 port——「本会话无编排停等面」的显式空实现（两条广播无处可播，`ChatAskPort` 正是这个事实的类型化表达）。三条对比：① 改 compose 按段数选档 → 违反 spec「不分 role / 不分矩阵命中」；② 让 chat 拿真实 `ChangeFlowSink` → 它会 `publish` 到一个不存在的 run 条目（静默无操作），等价于空实现但多一层「以为有 run」的误解；③ 保留 `None` → change-chat 静默失去 ask（违反 AC-13）。取 ①' = 空实现，零运行期语义、十行代码、且让 spec 的两条条款都成立 |
| DD14 | 挂载追加序 | `definitions()` 内两条独立 `if`，**`ask` 先、`finish` 后**：`ask.is_some()` → 追加 `AskUserQuestion`；`finish.is_some()` → 追加 `finish_step` | 两条不可合取（AC-4）。`finish` 保持末位是既有观感与既有 `mount_test`（「finish 位追加在末尾」）的零改动前提；ask 插在中间不影响任何既有断言 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 会话级应答注册表（**新增**） | `AskRegistry { slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`：`register` / `answer`（take 语义）/ `remove`（miss 幂等）；键 = core session id | `crates/core/agent/src/ask.rs`（新）+ `lib.rs` 出口 | `std::sync::{Mutex, HashMap}`、`tokio::sync::oneshot`（全部既有；`core/agent` 已依赖 `tokio sync`） | 与 `StopRegistry` 同型的内存表；零编排域类型 |
| 注入位与 ask 档位词汇（**修改**） | `AskMode { Generic, Flow }`（`Copy + Eq`、camelCase、不派生 `specta::Type`）；`SessionInjections` 加 `#[serde(default)] ask: Option<AskMode>`（第四位）；`SessionOpen` 加 `session_id: String` | `crates/core/agent/src/runner.rs` | `serde`（既有） | 闭合枚举 + `#[serde(default)]` additive 演进；`PartialEq + Eq` 链完好 |
| 内核注册表托管与除名兜底（**修改**） | `SessionKernel` 第 4 字段 `ask_registry: Arc<AskRegistry>`；`RunningTurn` 携带并在 `converge` 内 `remove(session_id)`（与 `registry.remove` 同址）；`begin_turn` 把 core 铸 id 填进 `SessionOpen` | `crates/core/agent/src/kernel.rs` | 既有 | 与 `StopRegistry` 同址的单一收口缝 |
| ask 工具协议单点（**新增**） | `TOOL_NAME` / `description()` / `parameters()`（静态，无 prompt 依赖）/ `validate(args) -> Result<AskQuestion, String>`；`AskQuestion { question, options }` | `crates/core/orchestration/src/ask.rs`（新）+ `lib.rs` 出口 | `serde_json`（既有）；**MUST NOT `use crate::finish`** | 纯函数、零 IO |
| 编排侧 ask 状态面（**新增**） | `trait AskPort { fn ask_started(&self, ask: AskPayload); fn ask_resolved(&self); }`（实现驻壳层 `ChangeFlowSink`） | `crates/core/orchestration/src/port.rs` | `crate::state::AskPayload`（既有） | 同步 trait（沿 `RunEventSink` 形态） |
| 应答通道转投与停等清理（**修改**） | `answer()` → `current_session` → `AskRegistry::answer`；新增 `resolve_ask()`；删 `answer_tx` 槽与 `wait_answer()`；`accept_pending` / `confirm` / `wait_confirm` / 锁序纪律**逐字不动** | `crates/core/orchestration/src/control.rs` | `agent::AskRegistry`（既依赖） | 既有注册表形态 |
| 载荷扩展与变体收敛（**修改**） | `AskPayload` 增 `tool_call_id` / `session_id`（additive）；`RunUpdate::Ask { question, options }` → `Ask { ask: AskPayload }` | `crates/core/orchestration/src/state.rs` | `serde` / `specta`（既有） | 载荷单源；`kind()` / `RunNotice` 零变 |
| 决策协议收窄（**修改**） | decision 形态 `enum` 收两值、删两 property、description 改写；`DecisionAction::Ask` 保留 + doc 注明生产不可达；`decision_prompt` 两动作 + ask 指路；删 `decision_answer_prompt`；`decision_session` 去循环化 + 防御臂 | `crates/core/orchestration/src/{finish.rs,decision.rs,prompt.rs,walker.rs}` | 既有 | 纯函数 / 既有步执行体骨架 |
| 挂载项与执行体档位（**修改**） | `ToolExec::Ask(agent::AskMode)` 第五臂；`definitions()` 增 ask 追加 `if`（`tier = Protocol`，定义取自 `orchestration::ask`） | `crates/infra/agent/src/sdk/mount.rs` | `agent` / `orchestration`（既有） | `OnceLock` 静态表 + 按位铸造 |
| 阻塞执行体两档（**修改**） | `AskContext`（registry + core session id + port）；`LoopTurn.ask: Option<AskContext>`；`ToolExec::Ask(mode)` 分派：`Generic` = `register` + `select!`；`Flow` = `ask_started` + `register` + `select!` + 两臂都 `ask_resolved` | `crates/infra/agent/src/sdk/loop.rs` | 既有 | 既有 async 泵循环（零新任务 / 零新通道） |
| per-turn 铸造与注入缝（**修改**） | `PumpPayload` 增 `ask_registry` / `ask_port`；`session_pump` 按 `injections.ask` 位铸 `AskContext`；`EngineFacade::with_ask_registry` + `with_ask_port`（与 `with_explore_note_store` 同型） | `crates/infra/agent/src/sdk/runner.rs` + `lib.rs` | 既有 | builder 缝 |
| 装配与成员校验（**修改**） | `compose_turn` 加两参（注册表透传内核 + 门面；port 仅门面）；`assemble_injections` 四臂显式置 `ask` 位（explore / archi / 其余 → `Generic`，change → `Flow`），**MUST NOT 以 `default()` 兜底** | `crates/infra/agent/src/compose.rs` | 既有 | 纯函数 |
| 壳层托管与新命令（**修改**） | `app.manage(Arc::new(AskRegistry::default()))`；`ChangeFlowControl::new(ask_registry)`；`agent_start` 传注册表；新增 `agent_answer`；`ChangeFlowSink` 实现 `AskPort`；`change_flow_start` 换序（sink 先于组合根） | `src/main.rs` + `src/commands/{exec,change_flow}/mod.rs` + `src/commands/mod.rs` | `agent` / `orchestration`（既有） | Tauri command 薄包装（三件事纪律） |
| 前端应答动作面与双态卡（**修改**） | `useAgentSession.answer`；`AgentToolCardProps` / `AgentMessagesProps` / `AgentSessionViewProps` 的 `askAnswer` 注入位；`AskUserQuestionCard` 双态 + 双形态解析；`SessionPanel` 骨架合成 | `packages/desktop/src/{hooks/use-agent-session.ts,components/agent/*.tsx}` | 既有（生成物驱动 IPC） | React + memo（沿 `composer` 插槽的「可选动作面」先例） |
| 流程侧接线（**修改**） | `change-detail-view` 组装 `askAnswer`（`run.answer` + `activeRun.ask.toolCallId`）→ `DetailDrawer` → `SessionTranscriptPanel` → `AgentSessionView` | `packages/desktop/src/views/changes/{change-detail-view.tsx,flow/detail-drawer.tsx,flow/session-transcript-panel.tsx}` | 同上 | 纯 prop 透传 |

<!-- 无新增 crate、无新增依赖边、无新增架构模型元素：`App.Backend.Core.Agent` / `App.Backend.Core.Orchestration` / `App.Backend.Infra.AgentRuntime` / `App.Backend.Shell` 四个既有元素的文件面内部改动；`ask.rs` 是既有模块内的新文件，不构成新 module 元素（.dev-team/architecture/models/** 零改动）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。测试文件与用例归测试相位，本清单只列**实现文件**，另设「编译闭合批」段登记必须随签名 / 字段面收敛的夹具点位。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `crates/core/agent/src/ask.rs` | `AskRegistry { slots: Mutex<HashMap<String, oneshot::Sender<String>>> }` + `new` / `register`（DD1 语义）/ `answer`（take 语义，miss 或重复 → `Err("当前无等待中的 ask")`）/ `remove`（miss 幂等）；`#[derive(Default)]`（壳层 `manage` 需要）；零编排域类型、零 workspace root / change 知识 |
| `crates/core/orchestration/src/ask.rs` | `pub const TOOL_NAME: &str = "AskUserQuestion"`；`description() -> &'static str`；`parameters() -> serde_json::Value`（`{ question: string（必填）, options: string[]（必填，minItems: 1）}`，无动态段故零参）；`validate(args: &Value) -> Result<AskQuestion, String>`（问题非空 / options 为非空字符串数组，Err 文案自足：字段名 + 期望 + 当前值）；`AskQuestion { question: String, options: Vec<String> }`。**MUST NOT `use crate::finish`**、全文（含 doc / 注释 / 字符串）零 `finish` 字样——故本文档措辞在源文件里须用「收口工具 / 并行模块」指代 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/lib.rs` | 挂 `mod ask;`（含 `#[cfg(test)] mod ask_test;`）+ 出口 `pub use ask::AskRegistry;` | `AskMode` 随 `runner` 出口 |
| `crates/core/agent/src/runner.rs` | ① 新增 `AskMode { Generic, Flow }`（`Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize`、`#[serde(rename_all = "camelCase")]`、**不派生 `specta::Type`**、不加 `Default`）；② `SessionInjections` 加 `#[serde(default)] pub ask: Option<AskMode>`（第四位，doc 明确 `None` = 显式禁用）；③ `SessionOpen` 加 `pub session_id: String`（doc 注明用途 = ask 登记键，`TurnQuestion` 不动） | AC-9；`AgentPermissionMode` / `AgentRunStatus` / `FinishSpec` / `SessionCtx` / `SessionRef` / `TurnQuestion` / `RunHandle` / `AgentRunner` 逐字不动 |
| `crates/core/agent/src/kernel.rs` | ① `SessionKernel` 增 `ask_registry: Arc<AskRegistry>` 字段；`new(sink, registry, streams, ask_registry)` 四参；② `RunningTurn` 增同名 `Arc` 字段；③ `converge(… ask_registry: &AskRegistry …)` 内 `ask_registry.remove(session_id)`（紧随 `registry.remove`）；④ `begin_turn` 把 `session_id.clone()` 填进 `SessionOpen` | AC-5；`StopRegistry` 逐字不动 |
| `crates/core/orchestration/src/lib.rs` | 挂 `pub mod ask;`；`pub use ask::{description, parameters, validate, AskQuestion, TOOL_NAME};`；`pub use port::AskPort;` | 出口面（spec Module Contract） |
| `crates/core/orchestration/src/port.rs` | 新增 `pub trait AskPort: Send + Sync { fn ask_started(&self, ask: AskPayload); fn ask_resolved(&self); }` | AC-2；`RunEventSink` 等既有 trait 逐字不动 |
| `crates/core/orchestration/src/state.rs` | ① `AskPayload` 增 `#[serde(default)] pub tool_call_id: Option<String>` / `#[serde(default)] pub session_id: Option<String>`；② `RunUpdate::Ask { question, options }` → `Ask { ask: AskPayload }`；`kind()` 与 `From<&RunUpdate> for RunNotice` 逐字不动 | AC-9；`RunNotice` 六变体零变 |
| `crates/core/orchestration/src/control.rs` | ① 删 `#[derive(Default)]`，`new(ask_registry: Arc<AskRegistry>)`（DD3）；② 删 `RunEntry.answer_tx` 字段与 `wait_answer()`；③ `answer()` 改三段判定（DD4）；④ `publish` 的 Ask 臂改 `entry.ask = Some(ask)`（`status = WaitingAsk` 保留）；⑤ 新增 `resolve_ask(root, change_id)`（DD5）；**`accept_pending` / `register_pending` / `confirm()` / `wait_confirm()` / `cancelled_wait` 逐字不动** | AC-8；`RunGuard` 其余方法零变 |
| `crates/core/orchestration/src/finish.rs` | decision 形态 `parameters`：`enum` 收为 `["backtrack","stop"]`、删 `question` / `options` 两 property；`description(Decision)` 改两动作 + 「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」（**文案里出现工具名，非代码引用**）；`validate` 仍走 `validate_decision`；evaluator 形态与 `FinishPayload` 逐字不变 | AC-12 / AC-4 |
| `crates/core/orchestration/src/decision.rs` | `DecisionAction::Ask` 保留，doc 补「生产不可达：`finish_step` 的 enum 已不含它，历史 notice 反序列化仍可读，walker 防御臂据此判漂移」；`validate_decision` 的 `"ask"` 臂（`:81-95`）与 `other` 臂文案逐字保留；`ensure_backtrack_allowed` / `MAX_REASON_CHARS` 零变 | AC-12（R5 历史兼容面） |
| `crates/core/orchestration/src/prompt.rs` | `decision_prompt` 的「决策协议」段由三选一改两选一 + 一句「无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）」（**形状不复述**）；**删 `decision_answer_prompt`**（`:131-139`）；`evaluator_protocol` 与 `backtrack_option_*` 三助手 / `spec_sync_prompt` / `merge_conflict_prompt` 逐字不动 | AC-12；`decision_prompt` 其余四段逐字不动 |
| `crates/core/orchestration/src/walker.rs` | ① `decision_session`：删 `loop` 与 `continue_session`（单轮直落）；`Some(FinishPayload::Decision(DecisionAction::Ask { .. }))` → `Terminal::failed("决策漂移：ask 应经 AskUserQuestion 工具提交")`（不 emit、不停等、不开新轮）；② `resolve_deadlock` 的 `DecisionAction::Ask` 臂结构保留、文案与注释改写（DD10）；③ 顶部 `use` 面去 `decision_answer_prompt`；④ 两处 `wait_answer` 相关注释改写（DD10） | AC-12；`step_verdict_gate*` / `feedback_limit` / `ensure_backtrack_allowed` / `WhitelistGate` 节点语义 / 四处机械 checklist 代写点**逐字不动** |
| `crates/infra/agent/src/sdk/mount.rs` | ① `ToolExec` 增 `Ask(agent::AskMode)`（第五臂）；② `definitions()` 在 `injections.ask` 为 `Some(mode)` 时追加 `AskUserQuestion` 挂载项（`description` / `parameters` 取自 `orchestration::ask`，`tier = ToolTier::Protocol`，`exec = Ask(mode)`），与 `finish` 位**两段独立 `if`**（DD14 序：ask 先、finish 后）；③ 测试夹具 `injected` / `injected_finish` 补 `ask: None` 字段（编译闭合）；④ doc 补一条 provider 侧不变量（原生 ask 工具一律不挂 = 同名覆盖） | AC-3 / AC-4 |
| `crates/infra/agent/src/sdk/loop.rs` | ① `LoopTurn` 增 `pub ask: Option<AskContext>`；新 `pub(crate) struct AskContext { registry: Arc<agent::AskRegistry>, core_session_id: String, port: Option<Arc<dyn orchestration::port::AskPort>> }`（DD6）；② `match mount.exec` 增 `ToolExec::Ask(mode)` 臂 → 私有 `async fn execute_ask(ctx: Option<&AskContext>, mode: agent::AskMode, args: &Value, tool_call_id: &str, handle: &RunHandle) -> (String, bool)`；③ 函数体：`validate` Err → `(msg, true)`；上下文缺席 → `(「ask 执行上下文未装配」, true)`；`Flow` 且 `port` 缺席 → `(「ask 编排状态面未装配」, true)`（DD2 防御臂，生产不可达）；`Flow` 先 `ask_started`（`AskPayload { question, options, tool_call_id: Some(id), session_id: Some(core_session_id) }`）再 `register`；两档 `tokio::select! { answer = rx => …, _ = handle.wait_requested() => ("用户未应答（会话被终止）", true) }`；`Flow` 两臂返回后**恰一次** `ask_resolved` | AC-1 / AC-2 / AC-3 / AC-5；`sandbox_rewrite` / `deny` / `protect` 闸 / 收口四步序 / 截断终局**逐字不动** |
| `crates/infra/agent/src/sdk/runner.rs` | ① `SdkRunner` 增 `ask_registry: Arc<agent::AskRegistry>` 与 `ask_port: Option<Arc<dyn AskPort>>` 字段 + `with_ask_registry` / `with_ask_port` 两条 builder 缝（`new` 签名不动）；② `PumpPayload` 增两字段；③ `open_session` 把两个句柄放进 `PumpPayload`；④ `session_pump` 按 `open.injections.ask` 位铸 `AskContext`（`core_session_id = open.session_id.clone()`、`Flow` 档带 `port`、`Generic` 档 `port = None`）并填 `LoopTurn.ask` | AC-1 / AC-3 |
| `crates/infra/agent/src/lib.rs` | `EngineFacade` 增两字段与 `with_ask_registry(Arc<agent::AskRegistry>)` / `with_ask_port(Option<Arc<dyn orchestration::port::AskPort>>)`；`runner_for` 内把两者装进 `SdkRunner` | 两条 builder 缝与 `with_explore_note_store` 同型 |
| `crates/infra/agent/src/compose.rs` | ① `compose_turn(…, ask_registry: Arc<agent::AskRegistry>, …, ask_port: Option<Arc<dyn AskPort>>)`（注册表透传内核 + 门面；port 仅门面）；② `SessionKernel::new(..., Arc::clone(&ask_registry))`；③ `compose_turn` 的 `EngineFacade` 链加 `.with_ask_registry(...)` / `.with_ask_port(...)`；④ `assemble_injections` 四臂显式置 `ask` 位：explore / archi 臂经 `injected` helper（增 `ask: Option<agent::AskMode>` 形参）传 `Some(Generic)`，change 臂传 `Some(Flow)`；**其余来源臂**改为显式结构字面量 `agent::SessionInjections { preamble: None, tools: None, finish: None, ask: Some(agent::AskMode::Generic) }`（spec 钉死该臂 `tools = None`，故不经 `injected` 的 `Some(names)` 形态；内建七件无需成员校验），**MUST NOT 用 `SessionInjections::default()`**（其 `ask = None` 即静默禁用）；⑤ `composed_with_runner` 测试夹具补注册表 | AC-1 / AC-3 / AC-4；`change_phase_role` / `archi_matrix` / `resolve_agent_engine` / `map_protocol` 逐字不动 |
| `src/main.rs` | `app.manage(Arc::new(AskRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` 三层同址，注释同步）；`ChangeFlowControl::new(Arc::clone(app.state::<Arc<AskRegistry>>().inner()))` | AC-2 / AC-8 |
| `src/commands/exec/mod.rs` | ① `agent_start_with` 取 `AskRegistry` 句柄（沿 `registry` / `streams` 既有形态）并传入 `compose_turn`；② 新增私有 `struct ChatAskPort;` + `impl AskPort for ChatAskPort { fn ask_started(&self, _ask: AskPayload) {} fn ask_resolved(&self) {} }`（DD15：chat 会话无编排停等面的类型化空实现，`agent_start` 以 `Some(Arc::new(ChatAskPort))` 传入——使 change-chat 的 `Flow` 档在「无 run」形态下仍可 ask 可答，AC-13）；③ 新增 `agent_answer(registry: State<'_, Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>`（三件事纪律：`root` / `session_id` 两参 blank 校验（`text` 不校验——自由应答取值域不设限）→ `registry.answer(...)` → `Err` 透传；零状态机、零广播、零清理） | AC-10 / AC-13 |
| `src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::exec::agent_answer`（紧随 `agent_stop`，`commands/exec` 轨道） | AC-10 |
| `src/commands/change_flow/mod.rs` | ① `impl AskPort for ChangeFlowSink`（`ask_started` → `control.publish(RunUpdate::Ask { ask })`；`ask_resolved` → `control.resolve_ask`）；② `change_flow_start_with` **换序**：先建 `Arc<ChangeFlowSink>`（`Arc<dyn RunEventSink>` 与 `Arc<dyn AskPort>` 两次强转同一 `Arc`），再 `compose_turn`（传注册表 + `Some(ask_port)`），最后 `KernelWorkerPort::new`；③ `change_flow_answer_with` 保持调 `control.answer`（签名与前端参数面零改动） | AC-2 / AC-8 |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | `agentAnswer: (root: string, sessionId: string, text: string) => Promise<null>` + `AskPayload` 两字段（`toolCallId: string \| null` / `sessionId: string \| null`） | `bindings:export` 生成；`bindings.zod.ts` 同步再生成（禁手改） |
| `packages/desktop/src/hooks/use-agent-session.ts` | `UseAgentSessionState` 增 `answer: (text: string) => void`；`useSessionActions` 增 `actionError` 与 `answer`（`root` / `sessionId` 任一 null 即不发；`ipc.agentAnswer(root, sessionId, text)` 的 `Err` 落 `actionError`）；返回面 `error` 三源合并（DD11） | AC-13 |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `AgentToolCardProps` 增可选 `askAnswer?: { onAnswer: (text: string) => void }`；`AskUserQuestionCard` 双态：静态（可读、零交互、**文案去掉「请在下方输入框作答」**）/ 活卡（选项按钮 + 自由文本 + 提交，提交后本地禁用防连点，空串与纯空白不提交）；`parseAsk` 双形态归一（DD9） | AC-11 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | `AgentMessagesProps` 增 `askAnswer?: AskAnswerActions`（`{ onAnswer: (text: string) => void; pendingToolCallId?: string \| null }`，导出类型 `AskAnswerActions`）；`ToolPartView` 仅对 `AskUserQuestion` 透传并按三态矩阵判活（DD8）；`ChatBubble` / `PartView` / `MessageContainer` 透传；`askAnswer` 经 `useMemo` 稳定化（DD12） | AC-11 |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | `AgentSessionViewProps` 增可选 `askAnswer?: AskAnswerActions`；自持 / 注入两形态同透传 `AgentMessages`（**不自动接线**，DD7） | AC-11 |
| `packages/desktop/src/components/agent/session-panel.tsx` | `ExpandedBody` 从 `state.session.answer` 合成 `askAnswer={{ onAnswer: state.session.answer }}` 下传 `AgentSessionView`（`SessionPanelState` 形状零改动） | AC-13（archi / change-chat 两面板零改动即得活卡） |
| `packages/desktop/src/components/agent/index.tsx` | 再导出 `export type { AskAnswerActions } from './agent-messages';` | **proposal 表未列，实现必需的最小面**：流程侧三文件（`detail-drawer` / `session-transcript-panel` / `change-detail-view`）经 barrel `@/components/agent` 引入组件与类型，`AskAnswerActions` 是新动作面的公共类型，不导出则该三处须绕开既有导入惯例改深路径（第二种导入形态）；本行只是一行类型再导出，零运行期语义 |
| `packages/desktop/src/views/explores/explore-detail-pane.tsx` | `ChatPane` 注入 `askAnswer={{ onAnswer: session.session.answer }}` | AC-13 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 注入 `askAnswer={{ onAnswer: session.answer }}` | AC-13 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 增可选 prop `askAnswer?: AskAnswerActions`；在场时并入 `viewProps` 透传 `AgentSessionView`（其余「不接动作面」纪律不变——它是 ask 应答的单点例外） | AC-13 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 增可选 prop `askAnswer`，原样透传 `DrawerContent` → `SessionTranscriptPanel` | AC-13 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `DetailLoaded` 组装 `askAnswer`（`onAnswer: (text) => { void run.answer(text) }`，`pendingToolCallId: activeRun?.ask?.toolCallId ?? null`，DD13）→ `DetailTabs` → `DetailContent` → `DetailDrawer`（同名文件内 props 链，不新增文件） | AC-2 / AC-13 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（现值 `0.6.35` → `0.6.36`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行）；`dependencies` / `devDependencies` 零变化 | 版本纪律；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行 MUST NOT 随动 |

### 编译闭合批（随字段面 / 签名收敛的夹具点位；MUST NOT 新增用例 / 改断言口径）

> 这些不是「测试编写」，而是「让工作区随字段面 / 签名变化编译通过」的机械适配（漏改即 `E0063` / `E0061`）。签名一变工作区即不可编译，故与实现文件同批落地。

| 面 | 点位（实测计数） | 改动 |
|----|----|------|
| `SessionInjections` 字面构造 | **32 处**：`core/runner_test`(9) / `sdk/loop_test`(8) / `sdk/tools_test`(6) / `sdk/runner_test`(4) / `sdk/mount_test`(2) / `kernel_test`(1) / `compose`(1) / `mount`(1) | 补 `ask: None` 字段（`compose` 的 1 处随四臂重写为显式置位，非纯机械） |
| `SessionOpen` 字面构造 | **19 处**：`kernel.rs`(1，生产) / `core/runner_test`(2) / `compose_test`(3) / `lib_test`(6) / `sdk/config_test`(1) / `sdk/runner_test`(6) | 补 `session_id: <被测语义 id>`（测试侧给固定串如 `"core-session-1"`，生产由 `begin_turn` 填真值） |
| `SessionKernel::new` 调用 | **47 处**：`kernel_test`(42) / `lib_test`(1) / `compose`(2，生产) / `compose_test`(2) | 补第 4 参 `Arc<AskRegistry>`（测试侧每用例一条显式 `Arc::new(AskRegistry::default())`，**不引入全局静态**——避免测试间串表，沿 R3b 纪律） |
| `LoopTurn` 字面构造 | 约 **40 处**：`sdk/loop_test` 约 39（多数经 `..loop_turn(...)` 兜底，只需在 helper `loop_turn` 内补一处 `ask: None`）/ `sdk/runner.rs`(1，生产) | helper 内补 `ask: None`；直构用例按需补字段 |
| `compose_turn` 调用 | **46 处**：`compose_test`(36) / `worker_test`(8) / `change_flow`(1) / `exec`(1) | 加两实参（`ask_registry` 与 `ask_port`）；测试侧可传 `Arc::new(AskRegistry::default())` + `None` |
| `ChangeFlowControl::new` 调用 | **254 处**：`control_test`(63) / `walker_test`(188) / `changes/mod_test`(1) / `change_flow/mod_test`(1) / `main.rs`(1) | 加 `Arc<AskRegistry>` 实参；两测试文件用**文件内共享夹具**压平（`fn control() -> Arc<ChangeFlowControl>`），但每条用例的注册表实例独立（R3b） |
| `ChangeFlowControl::default` 调用 | **9 处**（`control_test.rs`） | `Default` 派生随 DD3 删除（唯一构造面 = `new(ask_registry)`），9 处改为 `ChangeFlowControl::new(Arc::new(AskRegistry::default()))` |
| `RunUpdate::Ask` 字面构造 | **21 处**：`control`(1) / `control_test`(7) / `state`(2) / `state_test`(3) / `walker`(2) / `walker_test`(1) / `changes/mod_test`(3) / `change_flow/mod_test`(2) | 变体收敛为 `Ask { ask: AskPayload { … } }`（构造处补两新字段 `None`） |
| `AskPayload` 字面构造 | **2 处**：`control.rs:181`（随实现改写）/ `state_test.rs:555`（补两字段） | 补 `tool_call_id` / `session_id` |
| `AgentToolCardProps` 使用面 | `agent-messages.tsx` 的 `ToolPartView`（生产）；`tool-cards.test.tsx` 等测试面 | 新增可选 prop **不破坏**既有构造（无需机械补位）；测试面改造归测试相位 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `AskRegistry::new` | `crates/core/agent/src/ask.rs` | 新增 | `pub fn new() -> Self` | 空注册表（`Default` 派生 + `new` 薄包装，与 `StopRegistry` 同形） |
| `AskRegistry::register` | 同上 | 新增 | `pub fn register(&self, session_id: &str, tx: tokio::sync::oneshot::Sender<String>) -> bool` | 登记等待方；**恒写入**（同键覆盖旧等待方）；返回 `true` = 登记时该键已空（DD1，诊断值非失败面） |
| `AskRegistry::answer` | 同上 | 新增 | `pub fn answer(&self, session_id: &str, text: String) -> Result<(), String>` | take 语义：移除槽并 `send`；miss / 重复 → `Err("当前无等待中的 ask")` |
| `AskRegistry::remove` | 同上 | 新增 | `pub fn remove(&self, session_id: &str)` | 除名（轮收敛兜底）；miss 幂等 |
| `SessionKernel::new` | `crates/core/agent/src/kernel.rs` | 修改（加第 4 参） | `pub fn new(sink: Arc<dyn SessionSink>, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<AskRegistry>) -> Self` | 注册表由组合根注入，MUST NOT 内部自铸 |
| `description` | `crates/core/orchestration/src/ask.rs` | 新增 | `pub fn description() -> &'static str` | ask 工具静态描述（<300 字：何时问、单问一次、非空选项、自由文本应答如何回灌） |
| `parameters` | 同上 | 新增 | `pub fn parameters() -> serde_json::Value` | 静态 schema（无 prompt 依赖，故零参）；`options` `minItems: 1` |
| `validate` | 同上 | 新增 | `pub fn validate(args: &serde_json::Value) -> Result<AskQuestion, String>` | 纯函数校验；Err 文案自足（字段名 + 期望 + 当前值），六态覆盖 question 缺失/空白、options 缺失/非数组/元素非字符串/空数组 |
| `AskPort::ask_started` | `crates/core/orchestration/src/port.rs` | 新增（trait 方法） | `fn ask_started(&self, ask: AskPayload)` | 「ask 已开始」广播（携 `tool_call_id` 与 `session_id`） |
| `AskPort::ask_resolved` | 同上 | 新增（trait 方法） | `fn ask_resolved(&self)` | 「本条 ask 结束」信号（停等态清理单点触发） |
| `ChangeFlowControl::new` | `crates/core/orchestration/src/control.rs` | 修改（加参 + 删 `Default`） | `pub fn new(ask_registry: Arc<agent::AskRegistry>) -> Self` | 唯一构造面（DD3） |
| `ChangeFlowControl::answer` | 同上 | 修改（语义换，签名不变） | `pub fn answer(&self, root: &str, change_id: &str, text: String) -> Result<(), String>` | 三段判定 + 转投 `AskRegistry`（DD4）；零状态清理、零广播 |
| `ChangeFlowControl::resolve_ask` | 同上 | 新增 | `pub fn resolve_ask(&self, root: &str, change_id: &str)` | 停等态清理单点（`ask` 槽无条件清、`status` 条件回 `Running`、锁外广播 `RunNotice::Step`，DD5） |
| `RunGuard::wait_answer` | 同上 | **删除** | — | ask 停等搬进执行体；`grep wait_answer` 零命中为 AC-8 判据 |
| `description` / `parameters` / `validate`（`finish`） | `crates/core/orchestration/src/finish.rs` | 修改（decision 形态收窄，签名不变） | `pub fn description(spec: FinishSpec) -> &'static str`；`pub fn parameters(spec: FinishSpec, prompt: &str) -> Value`；`pub fn validate(spec: FinishSpec, args: &Value, prompt: &str) -> Result<FinishPayload, String>` | `Action` 枚举两值、两 property 删除、description 指路 ask 工具 |
| `decision_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改（段文本） | `pub fn decision_prompt(input: &DecisionInput) -> String` | 协议段两动作 + ask 工具指路 |
| `decision_answer_prompt` | 同上 | **删除** | — | 应答不再开新一轮；`grep` 零命中 |
| `definitions`（infra 挂载单源） | `crates/infra/agent/src/sdk/mount.rs` | 修改（加 ask 臂，签名不变） | `pub(crate) fn definitions(injections: &agent::SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>` | `ask` 位 `Some(mode)` → 追加 `AskUserQuestion`（`Protocol` / `Ask(mode)`），与 `finish` 两条独立 `if`（DD14） |
| `SdkRunner::with_ask_registry` | `crates/infra/agent/src/sdk/runner.rs` | 新增（builder 缝） | `pub fn with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self` | 与 `with_explore_note_store` 同型 |
| `SdkRunner::with_ask_port` | 同上 | 新增（builder 缝） | `pub fn with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self` | 仅 `Flow` 档消费 |
| `EngineFacade::with_ask_registry` | `crates/infra/agent/src/lib.rs` | 新增（builder 缝） | `pub fn with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self` | 门面 → runner 单一通道 |
| `EngineFacade::with_ask_port` | 同上 | 新增（builder 缝） | `pub fn with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self` | 同上 |
| `compose_turn` | `crates/infra/agent/src/compose.rs` | 修改（加两参） | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<agent::AskRegistry>, store: Arc<Store>, agent: Option<i64>, workspace_root: &Path, ask_port: Option<Arc<dyn orchestration::port::AskPort>>) -> Result<ComposedTurn, String>` | 注册表透传内核 + 门面；`ask_port` 仅门面（编排侧状态面实现方是壳层 sink） |
| `agent_answer`（IPC 命令） | `src/commands/exec/mod.rs` | 新增 | `pub fn agent_answer(registry: State<'_, Arc<agent::AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>` | 薄包装三段；blank `root` / `session_id` → `Err`（携字段名）；miss → `Err("当前无等待中的 ask")`；命中 → `Ok`；`text` 不做空白校验（自由应答） |
| `AskPort for ChangeFlowSink` | `src/commands/change_flow/mod.rs` | 新增（trait impl） | `fn ask_started(&self, ask: AskPayload)`；`fn ask_resolved(&self)` | `publish(RunUpdate::Ask { ask })` / `resolve_ask` |
| `answer`（前端 hook 动作） | `packages/desktop/src/hooks/use-agent-session.ts` | 新增 | `answer: (text: string) => void` | `ipc.agentAnswer(root, sessionId, text)`；`root` / `sessionId` 为 null 零 invoke；`Err` 落 `error` 面（DD11） |

<!-- 无新增 CLI 子命令、无新增 HTTP 端点。IPC 面只增一条命令（下表）。 -->

**退役的公共面（含理由）**：`ChangeFlowControl::answer_tx`（槽字段，单通道化后无第二真相）／`RunGuard::wait_answer()`（ask 停等搬进执行体，walker 零调用者）／`prompt::decision_answer_prompt`（应答不再开新一轮，`grep` 零命中）／`finish_step` decision 形态的 `question` / `options` 两 property 与 `ask` 枚举值（迁出为独立工具）。

**「不要修改」的越界面（出现 diff 即说明实现越界）**：

- `crates/core/orchestration/src/control.rs::accept_pending` / `register_pending` / `confirm()` / `wait_confirm()` / `cancelled_wait`（D6 的机械判据——泛型模板与 confirm 路径零改动）。
- `crates/core/orchestration/src/verdict.rs` / `transcript.rs` / `finish.rs::FinishPayload` / `walker.rs` 的 `step_verdict_gate*` 与四处机械 checklist 代写点。
- `crates/infra/agent/src/sdk/protect/**`、`sdk/{sandbox,context,compact,resume,normalize,web_search,bash,model_spec}.rs`、`sdk/tools.rs`（七通用工具执行体与 `TOOL_NAMES`）。
- `crates/core/workflow/**`（相位表 / 14 份 prompt md / `include_str!` 装配零改动）、`crates/infra/store/**`（停等态仍不落库）。
- `packages/desktop/src/lib/{agent-adapter,agent-transport}.ts`、`components/agent/agent-blocks.tsx`、`views/changes/flow/run-control-panel.tsx`（`AskCard` 零改动）。
- `.dev-team/architecture/models/**`（零新元素、零新依赖边）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AskRegistry`（struct） | `crates/core/agent/src/ask.rs` | 新增 | `{ slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`；`#[derive(Default)]`；键 = core 铸 session id；零编排域类型（无 change id、无 `RunEventSink`） |
| `AskMode`（enum） | `crates/core/agent/src/runner.rs` | 新增 | `{ Generic, Flow }`；`Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize`、`#[serde(rename_all = "camelCase")]` ⇒ 线值 `"generic"` / `"flow"`；**不派生 `specta::Type`**、不加 `Default` |
| `SessionInjections`（struct） | 同上 | 修改 | 四字段 `{ preamble, tools, finish, ask: Option<AskMode>（#[serde(default)]） }`；`Default + PartialEq + Eq` 完好；`ask = None` = 显式禁用（fail-closed） |
| `SessionOpen`（struct） | 同上 | 修改 | 增 `pub session_id: String`（core 铸 id 下传，用途 = ask 登记键与引擎侧归属）；`TurnQuestion` 零改动 |
| `AskQuestion`（struct） | `crates/core/orchestration/src/ask.rs` | 新增 | `{ pub question: String, pub options: Vec<String> }`（`Debug + Clone + PartialEq + Eq`）；`validate` 的产物 |
| `AskPort`（trait） | `crates/core/orchestration/src/port.rs` | 新增 | `Send + Sync`；两方法（`ask_started` / `ask_resolved`）；实现驻壳层 |
| `AskPayload`（struct） | `crates/core/orchestration/src/state.rs` | 修改 | 增 `#[serde(default)] tool_call_id: Option<String>` / `session_id: Option<String>`（additive，旧线缺字段读 `None`） |
| `RunUpdate`（enum） | 同上 | 修改 | `Ask { ask: AskPayload }`（载荷单源；`kind()` == `"ask"`、`RunNotice` 投影零变） |
| `AskContext`（struct） | `crates/infra/agent/src/sdk/loop.rs` | 新增 | `pub(crate) { registry: Arc<agent::AskRegistry>, core_session_id: String, port: Option<Arc<dyn AskPort>> }`（DD6：`core_session_id` 与 `LoopTurn.session_id` 不同义） |
| `ToolExec`（enum） | `crates/infra/agent/src/sdk/mount.rs` | 修改 | 增 `Ask(agent::AskMode)`（`Copy + Eq` 保持） |
| `AskAnswerActions`（TS type） | `packages/desktop/src/components/agent/agent-messages.tsx` | 新增 | `{ onAnswer: (text: string) => void; pendingToolCallId?: string \| null }`（三态判活矩阵的承载类型，DD8） |
| `AgentToolCardProps`（TS interface） | `packages/desktop/src/components/agent/tool-cards.tsx` | 修改 | 增可选 `askAnswer?: { onAnswer: (text: string) => void }`（卡内不再需要 `pendingToolCallId`——判活在上层完成） |
| `AgentMessagesProps` / `AgentSessionViewProps`（TS interface） | `agent-messages.tsx` / `agent-session-view.tsx` | 修改 | 各增可选 `askAnswer?: AskAnswerActions` |
| `UseAgentSessionState`（TS interface） | `packages/desktop/src/hooks/use-agent-session.ts` | 修改 | 增 `answer: (text: string) => void` |

<!-- 无新增 interface / type alias 之外的词汇：`AskMode` / `AskQuestion` / `AskContext` / `AskAnswerActions` 为唯一四新类型，其余复用既有 `AskPayload` / `SessionInjections` / `AgentToolCardProps`。 -->

### 配置

**无。** 本变更零 `KNOWN_KEYS` 增删、零 `TestFramework` 变体、零 `openspec/config.json` 改动、零 `crates/core/config/config.schema.json` 镜像改动（`SessionInjections` / `AskMode` 是进程内注入词汇，不读工作区配置）。`packages/desktop/package.json` 的 `version` 是版本纪律项，已登记在「修改文件」表。

---

## 数据模型

### ask 载荷线形态（`RunUpdate::Ask { ask }` / `ChangeRunSnapshot.ask` / `ActiveRunView.ask`）

```jsonc
{
  "question": "选择重试策略？",
  "options": ["指数退避", "固定间隔"],   // 非空（工具校验 minItems: 1）
  "toolCallId": "call_abc123",          // additive：行内卡对账（null = 旧线载荷）
  "sessionId": "0192f0…"                // additive：core 铸会话 id（null = 旧线载荷）
}
```

- **两字段 additive**：`#[serde(default)]`，旧线读 `None` → 前端 `null`；`AskCard`（详情页）不消费两者（零改动即可工作），聊天活卡消费 `toolCallId`。
- **`tool_call_id` 是双端同步的关键**：它与工具部件的 `toolCallId` 相等才判活（DD8 三态矩阵），使「正在等」与「上次崩了没回灌」可分辨（后者 `toolCallId` 不在 `activeRun.ask` 上）。
- **`session_id` 与 `toolCallId` 同源**：都来自执行体上下文（`tool_call_id` = 本次 `tool_use` 的 id；`session_id` = `AskContext.core_session_id`）。

### 注册表内存态（`AskRegistry`）

| 项 | 形态 |
|----|------|
| 键 | core 铸会话 id（`new_session_id()` 产出；进程内唯一） |
| 值 | `oneshot::Sender<String>`（单次投递通道；被 take 或覆盖即失效） |
| 登记 | `ToolExec::Ask(*)` 执行体在**进入 `select!` 之前**登记 |
| 消费 | `answer()` take（先到者胜）；第二次 `Err` |
| 除名 | ① 轮收敛 `converge` 兜底；② take 后槽即空（等价除名）；③ 同键覆盖（新问顶替旧问） |
| 生命周期 | 进程内，随应用重启清空（停等态不落库——与 `StopRegistry` / `SessionStreamRegistry` 同族） |

### 关系与持久化

| 项 | 现状 | 本变更 |
|----|------|--------|
| store 模型 | 会话 / 轮 / 转录表；`ChangeRunSnapshot` 不落库 | **零改动**（`crates/infra/store/**` 零 diff；停等态仍活在进程内） |
| 停等态载体 | `RunEntry.ask`（进程内 `ChangeFlowControl`） | 仍 `RunEntry.ask`——但载荷由 `AskPort::ask_started` 写入、由 `resolve_ask` 清除（写入方与清除方互换，槽位类型零变） |
| 应答通道 | `RunEntry.answer_tx`（`(root, change_id)` 寻址） | `AskRegistry`（core session id 寻址）；两段寻址由壳层翻译 |
| 转录 | `tool_use` / `tool_result` 一对，按既有管线落库 | **零改动**；ask 的阻塞期转录只增 `tool_use`（无结果即仍在等），停止时由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐（既有机制） |
| IPC 出线 | `AskPayload` 出线（`ActiveRunView.ask`） | 两字段自动透出；`AskMode` / `SessionInjections` / `AskContext` 不出线（反向锚定守） |

<!-- 无新表、无新列、无外键、无迁移：所有新增状态都是进程内内存态。 -->

---

## 路由 / API 设计

**IPC 命令面**（Tauri `invoke`；`Result<T, String>` 错误模板沿用既有约定）：

| method | path（命令） | 说明 | 输入 | 输出 | auth |
|--------|--------------|------|------|------|------|
| IPC | `agent_answer` | ask 应答（**新增**）：薄包装三段，零状态机 / 零广播 / 零清理 | `root: string` / `sessionId: string` / `text: string` | `null`（`Ok`）或 `Err(String)` | 无（本机桌面应用；root 即寻址） |
| IPC | `change_flow_answer` | 流程侧 ask 应答（**语义改**：内部由 `accept_pending` 转投 `AskRegistry`；签名与前端参数面零改动） | `root: string` / `id: string` / `answer: string` | `null` / `Err(String)` | 无 |

**错误语义（两命令同口径）**：

```
blank root            → Err("非法 root: 不得为空白")
blank session_id/id   → Err("非法 session_id: 不得为空白") / Err("非法 id: 不得为空白")
change_flow_answer: 无运行 run        → Err("change \"<id>\" 无运行中的 run")
有 run 但无会话 / 无等待方            → Err("当前无等待中的 ask")
agent_answer: 未登记 / 已应答         → Err("当前无等待中的 ask")
命中                                  → Ok(())
```

> **miss 显式 `Err` 而非幂等 `Ok`**：与 `agent_stop` 的 miss 幂等是不同语义——重复 / 迟到应答都是用户可感知的「卡已不在」事实，静默成功会掩盖双端不同步（spec 明文）。
>
> **`text` 不做空白校验（AC-10「blank 参数 `Err`」的口径收敛）**：proposal「测试文件」表把命令面判据写作「blank root / blank session_id → `Err`」，未列 `text`；且前端活卡与 `AskCard` 均已在提交侧拦空串与纯空白。故校验面只覆盖两个**寻址**参数（它们的空白会让注册表查不到任何条目，是纯误用），应答正文原样透传（后端不替用户裁量内容——若反向白名单化，未来「以空格表达跳过」一类应答会被静默改写）。

**内部调用链（非出线面，但决定幂等性）**：

```
聊天活卡 / SessionPanel / explore / debug
  └─ useAgentSession.answer(text)  → invoke agent_answer      ─┐
流程详情页 AskCard                                             ├→ AskRegistry.answer(session_id, text)
  └─ run.answer(text)            → invoke change_flow_answer ─┘     （先到者胜，后者 Err）
      └─ control.current_session(root, id) → session_id

ask 执行体（ToolExec::Ask(Flow)）返回
  └─ AskPort::ask_resolved → control.resolve_ask（清 ask 槽 + status 回 running + 广播 Step）
```

<!-- 无 HTTP 路由：本应用无 HTTP 服务面。 -->

---

## 依赖

- **运行时依赖**：**零新增**。`core/agent` 的 `ask.rs` 只用 `std::sync` + 既有 `tokio::sync::oneshot`（`crates/core/agent/Cargo.toml` 已有 `tokio = { workspace = true }`，workspace 层含 `sync` feature）；`orchestration/ask.rs` 只用 `serde_json`；`infra/agent` 只用既有 `agent` / `orchestration` / `tokio` / `serde_json`。五处 `Cargo.toml`（`core/agent`、`core/orchestration`、`infra/agent`、`src-tauri`、workspace 根）**零 diff**。
- **依赖方向零变化**：`infra/agent → agent`（既有）／`infra/agent → orchestration`（既有，`worker.rs` 已引 `orchestration::transcript`）／`orchestration → agent`（既有，`finish.rs` 已引 `agent::FinishSpec`，本变更加引 `agent::AskRegistry`）／`core/agent` 仍零 workspace 内依赖（`AskRegistry` 不引任何外部类型）。**零新依赖边**。
- **构建 / 测试依赖**：**零新增**。测试侧沿用既有 `rig` 的 `test-utils`（`loop_test` 的流缝脚本化）与 `tokio` 的 `test-util`；`ask_test` 为纯函数 + `oneshot` 对拍，零进程边界依赖。
- **无 feature 开关变化**：`rig` facade 锁版与 features 面逐字不动。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 通用会话同 turn 阻塞应答 | `mount::definitions` 的 ask 臂（`Protocol` / `Ask(mode)`）+ `compose::assemble_injections` 四臂（explore / archi / 其余 → `Generic`）+ `LoopTurn.ask` 铸造（`core_session_id = open.session_id`）+ `execute_ask` 的 `Generic` 臂（`register` → `select!` → 返回文本）+ `AskRegistry.answer`（DD1 take 语义）；**零 `SessionRef::Continue`**：阻塞在 `loop::run` 的同一 turn 内，`session_pump` 不重开 |
| AC-2 流程覆盖与双端同步 | `ToolExec::Ask(Flow)` 臂（`ask_started` → `register` → `select!` → `ask_resolved` 恰一次）+ `AskPayload` 两字段（`tool_call_id` = 该 `tool_use` id、`session_id` = core id）+ `AskPort for ChangeFlowSink` + `control.answer` 转投 + `control.resolve_ask`（两卡同步收起的单点）+ `ChangeFlowControl::publish` 的 `Ask { ask }` 臂（`status = WaitingAsk`） |
| AC-3 显式禁用面 | `definitions()` 的两条独立 `if`（`ask = None` 零追加）+ `loop.rs` 的成员校验（`dispatched` 不含该名即 `tool_unavailable`，既有机制零改动）+ `assemble_injections` 显式置位（`MUST NOT default()`——`default().ask == None` 即静默禁用，R8） |
| AC-4 解耦 | `orchestration/ask.rs` 全文零 `finish` 字样（源码自证）；`finish.rs` 的 `ask` 只出现在 description 文案字符串；`mount.rs` 两段独立 `if`（DD14）；`ask = None` 的会话仍有 `finish_step`、`finish = None` 的会话仍有 ask（`(ask, finish)` 四组合） |
| AC-5 停止语义 | `execute_ask` 两档均带 `handle.wait_requested()` 停止臂（`Flow` 臂是清理唯一落点——外层泵 `select!` 只 drop future，不执行清理代码）+ `converge` 内 `AskRegistry::remove` 兜底 + `resume::rebuild` 的 `INTERRUPTED_RESULT`（既有机制，只消费不改写） |
| AC-6 参数校验 | `orchestration::ask::parameters`（`options` `minItems: 1`）+ `validate` 六态 Err 文案（字段名 + 期望 + 当前值）+ 前端活卡选项与自由文本两路并存（`AskUserQuestionCard` 活卡态） |
| AC-7 重复应答幂等 | `AskRegistry::answer` 的 take 语义（槽被移除即 miss → `Err`）+ `agent_answer` / `change_flow_answer` 两命令均薄透传（零二次投递）+ `resolve_ask` 不清出「第二个等待方」 |
| AC-8 单通道 | `control.rs` 删 `answer_tx` 与 `wait_answer`（`grep` 零命中）+ `answer()` 改转投 + `accept_pending` / `confirm` / `wait_confirm` 零 diff（`git diff` 逐块比对，D6） |
| AC-9 载荷 additive 与出线 | `AskPayload` 两字段（`#[serde(default)]`）+ `RunUpdate::Ask { ask }` 单源 + `RunNotice` 六变体与 `kind()` 零变 + `bindings:export` 再生成（`AskMode` / `SessionInjections` 不出线：`bindings/mod_test.rs` 反向锚定清单零改动即覆盖，事实校准 11） |
| AC-10 命令面 | `commands/exec/mod.rs::agent_answer`（三段薄包装 + `root` / `session_id` 两参 blank `Err` + miss `Err`）+ `commands/mod.rs` 的 `all_commands!` 登记 |
| AC-11 前端双态卡 | `AskUserQuestionCard` 双态（静态零交互 / 活卡选项 + 文本 + 提交后禁用）+ `parseAsk` 双形态（DD9）+ `AgentMessages` 三态判活（DD8）+ `AgentSessionView` 只透传不自动接线（DD7）+ `Card` 根钩子与 `ToolPairCard` 回退零变 |
| AC-12 决策协议收窄 | `finish.rs` decision 形态（`enum` 两值、两 property 删除、description 指路）+ `walker::decision_session` 去循环化与 `Ask` → `Terminal::failed`（DD10）+ `resolve_deadlock` 防御臂保留 + `prompt.rs` 两动作与 `decision_answer_prompt` 删除 + `DecisionAction::Ask` / `validate_decision` 的 `"ask"` 臂保留 |
| AC-13 消费点接线 | `session-panel.tsx`（archi / change-chat 零改动即得）+ `explore-detail-pane` / `agent-debug-view`（通用侧 `agent_answer`）+ `change-detail-view` → `detail-drawer` → `session-transcript-panel`（流程侧 `change_flow_answer` + `pendingToolCallId`）+ `SessionTranscriptPanel` 未注入即全静态（历史重放零入口） |

### 交付门（非本变更能力判据）

全仓既有门的**登记面**，本设计与 tasks 均不派生新判据。分工按 tasks 约束 ⑤：源码级静态面（`cargo clippy --all-targets`、前端 `vp check` / `knip`、`grep` 零命中、结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）是**本相位可自判**的判据，可出现在 tasks 的自查项里；`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位承接，在 tasks 中只以「交接 …」条目登记，**MUST NOT** 作为任何阶段自查项的完成条件。

---

## 待决问题

1. **`AskRegistry::answer` 的 Err 文案是否要与 `ChangeFlowControl` 共用常量**：本期两处各持字面量（core 不认识编排词汇，反之 `orchestration` 引 core 常量则方向虽合法但把「应答 miss」变成跨 crate 词汇）。若后续出现第三个应答入口，再评估把文案收成 `core::ask::NO_WAITING_ASK` 一条 `pub const`（届时只是把字面量搬家，行为零变）。
2. **ask 阻塞是否需要引擎日志留痕**（`append_engine_log` 记「ask 开始 / 结束 / 停止臂」）：本期**不加**——`Flow` 档已有 `RunNotice` 与统一视图可观测，`Generic` 档的等待可由「转录有 `tool_use` 无 `tool_result`」判别；加日志反而在长阻塞期产生噪声。若后续出现「ask 悬挂排查困难」的实例，再按 `subtype` 记两条（届时须评估日志体量与 `sdk/log.rs` 的既有格式）。
3. **通用侧是否需要 `pendingToolCallId` 对账**：本期通用侧（explore / archi / debug / change-chat）按「未配对即活」判活（`pendingToolCallId` 缺省 `undefined`）。若后续出现「通用会话历史里的崩溃残留 `tool_use` 被误呈活卡」的真实报告，可由 `useAgentSession` 侧提供「当前在等」的 `tool_callId`（执行体已在 `AskPayload` 里给出，代价是会话侧多一条状态面），本设计不改其形状。
4. **spec 内两条款的措辞是否需要在归档时澄清**（事实校准 13 / DD15）：本期不动 spec 正文，靠装配面（exec 轨道传 `ChatAskPort`）让两条条款同时成立。归档合并时可考虑在 desktop-change-orchestration「流程内 ask 停等与双入口应答」的 `Flow` 档描述后补一句「无编排停等面的会话（change chat）恒由来源侧注入空实现 port，故 `port` 缺席只表示装配漂移」——这是文档澄清而非行为变更，进不进本期由合并相位决定。
5. **`AskPort` 是否要合并进 `RunEventSink`**：本期分立——`RunEventSink::emit` 是 `RunUpdate` 的通用出口（六变体），`AskPort` 是 ask 语义的收窄面（两方法）；合并会让「任何 sink 都必须实现 ask 的两个方法」，非 change 来源的空实现即噪音。若后续 sink 实现数增至 3 个以上，再评估合并或提供默认方法。
