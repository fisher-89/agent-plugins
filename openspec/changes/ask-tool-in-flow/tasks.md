# 任务: ask-tool-in-flow

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 34 项（含 2 新增文件 + 32 修改面）+ 机械改动（`SessionInjections` 32 处 / `SessionOpen` 19 处 / `SessionKernel::new` 47 处 / `LoopTurn` 约 40 处 / `compose_turn` 46 处 / `ChangeFlowControl::new` 254 处 / `RunUpdate::Ask` 21 处 / `AskPayload` 2 处，见 design「编译闭合批」与「事实校准 1–8」）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：新增 / 改造用例（proposal「测试文件」表逐行）归 test-design / test-gen / test-execution 相位，`cargo test` / `pnpm -C packages/desktop test` 等回归命令同样**由桌面端 test-execution 相位承接**（约束 ⑤）。本相位对测试文件的唯一动作是**让工作区随字段面 / 签名变化编译通过**（`SessionInjections { …, ask }` / `SessionOpen { …, session_id }` / `SessionKernel::new(…, ask_registry)` / `compose_turn(…, ask_registry, …, ask_port)` / `ChangeFlowControl::new(ask_registry)` / `RunUpdate::Ask { ask }`），MUST NOT 新增用例、MUST NOT 改断言口径、MUST NOT 把用例绿灯写成完成条件。
>
> **依赖序（七阶段）**：
>
> - **阶段一**（core/agent 会话地基：`ask.rs` + `AskMode` / `ask` 位 / `SessionOpen.session_id` + 内核托管）**破坏性**，编译期全暴露；它是全部后续阶段的底座（注册表类型、注入位词汇、core session id 下传三者同源）。
> - **阶段二**（orchestration 工具协议单点：`ask.rs` + `AskPort`）依赖零（只吃既有 `serde_json` / `state::AskPayload`），可与阶段一**并行**；但必须先于阶段四（挂载项要引它的定义与校验）。
> - **阶段三**（orchestration 载荷与应答通道 + 决策协议收窄）依赖阶段一（`AskRegistry` 类型）与阶段二（`AskPort` 类型）。
> - **阶段四**（infra 挂载与执行体 + 装配）依赖阶段一、二、三。
> - **阶段五**（壳层托管 + 新命令 + `AskPort` 实现）依赖阶段一、三、四。
> - **阶段六**（前端：生成物 + hook + 组件 + 六处接线）零 Rust 依赖（生成物来自阶段五的命令登记），可先做组件与接线、生成物再生成；但须在阶段七之前（版本行覆盖）。
> - **阶段七**（版本 + 边界自查 + 交接备忘）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **单点清理纪律**（design DD5 / spec「清理单点在执行体」）：停等态清理只在 `AskPort::ask_resolved` → `ChangeFlowControl::resolve_ask` 一处；两条应答命令体内 **零状态清理、零通知广播**。`Flow` 档 `select!` 的**应答臂与停止臂返回后都必须**调 `ask_resolved` 恰一次——外层泵的 `select!` 只 drop future，不执行清理代码，故显式停止臂是唯一落点。
> ① **`accept_pending` / `register_pending` / `confirm()` / `wait_confirm()` / `cancelled_wait` MUST 零 diff**（D6 的机械判据）：`git diff` 逐块比对，出现 hunk 即实现越界。变更面只有 `answer()`、`answer_tx` 槽、`wait_answer()` 三处。
> ② **解耦判据**（AC-4）：`crates/core/orchestration/src/ask.rs` 全文零 `finish` 字样；`finish.rs` 的 `ask` 只出现在 description 文案字符串；`mount.rs` 的 ask / finish 两条追加分支**不得合取**。
> ③ **两个 session id 不得复用**（design DD6）：`AskContext.core_session_id` 从 `SessionOpen.session_id` 取；`LoopTurn.session_id` 是引擎侧 `sdk-…` id，MUST NOT 作注册表键。
> ④ **装配不得用 `default()` 兜底**（R8）：`assemble_injections` 的「其余来源」臂改为显式构造 `ask = Some(Generic)`；`SessionInjections::default().ask` 恒 `None` 是 fail-closed 形态，出现即静默禁用。
> ⑤ **生成物 / 版本纪律**：`src/types/generated/**` 不手改（由 `bindings:export` 再生成，提交含生成物）；`package.json` 版本按「落地时 `HEAD` +1 patch」相对递增，MUST NOT 写死字面号；`src-tauri/Cargo.toml` crate 版本行不跟。
> ⑥ **门禁分工**：静态脚本链（`pnpm run check` = `server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**（`cargo test`、`pnpm -C packages/desktop test`）由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。据此本文件的自查项**只收源码级静态判据**——`cargo clippy`、`grep` 零命中、读码结构复核、`git diff` 边界比对、前端 `vp check` / `knip`；凡**测试回归**与**整链门运行**一律改为「交接 …（由桌面端 <相位> 承接）」的任务条目，**MUST NOT** 写成任何阶段自查项里的「全绿 / 通过」判据。

## 阶段一：core/agent 会话地基（`AskRegistry` + 注入位词汇 + 内核托管 + core session id 下传）

> 本阶段**破坏性**：`SessionInjections` 加第四字段、`SessionOpen` 加字段、`SessionKernel::new` 加参，三处一起改才可编译（拆开即工作区不可编译）。编译闭合批随本阶段同批落地。

- [x] 新建 `crates/core/agent/src/ask.rs`：`pub struct AskRegistry { slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`（`#[derive(Default)]`；`use std::collections::HashMap; use std::sync::Mutex; use tokio::sync::oneshot;`），doc 写明「键 = core 铸会话 id；与 `StopRegistry` 同型；零编排域类型（无 change id、无 workspace root）」
- [x] 同文件 `pub fn new() -> Self`（薄包装 `Self::default()`，与 `StopRegistry::new` 同形）
- [x] 同文件 `pub fn register(&self, session_id: &str, tx: oneshot::Sender<String>) -> bool`：`slots.lock().expect("ask 注册表锁不可中毒").insert(...)` 的返回值取反（`insert` 返 `None` ⇒ 该键原本为空 ⇒ `true`；返 `Some(_)` ⇒ 顶替了上一问的等待方 ⇒ `false`，旧 `Sender` 随覆盖 drop）；doc 按 design DD1 写明「恒写入；返回值是诊断值，同键覆盖 = 上一问已被放弃，**不是**登记失败面」
- [x] 同文件 `pub fn answer(&self, session_id: &str, text: String) -> Result<(), String>`：锁内 `slots.remove(session_id)`，`None` → `Err("当前无等待中的 ask".to_owned())`，`Some(tx)` → 锁外 `tx.send(text)`（`SendError` 只意味等待方已 drop，按受理返回 `Ok(())`——槽已被 take，幂等语义成立）
- [x] 同文件 `pub fn remove(&self, session_id: &str)`：`slots.lock().…remove(session_id);`（miss 幂等，零返回值）
- [x] `crates/core/agent/src/lib.rs`：`mod ask;`（置于 `mod event;` 之前按字母序）+ `#[cfg(test)] mod ask_test;`；出口加 `pub use ask::AskRegistry;`
- [x] `crates/core/agent/src/runner.rs`：新增 `AskMode`（design DD 表 / spec Module Contract 逐字）——`#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` + `#[serde(rename_all = "camelCase")]` + `pub enum AskMode { Generic, Flow }`；doc 写明「ask 装配档位：`Generic` = 只阻塞等应答 / `Flow` = 额外经 `AskPort` 广播编排状态；**不派生 `specta::Type`**（引擎侧会话词汇不出线 IPC）」；**不加 `Default`**（避免 ask 位被误缺省成「挂载」）；位置紧接 `FinishSpec` 之后
- [x] 同文件 `SessionInjections` 加第四字段：`/// ask 装配档位（None = 显式禁用：该会话拿不到 ask 工具；Some = 按档挂载）` + `#[serde(default)] pub ask: Option<AskMode>`；结构体 doc 由「三位」改「四位」，并写明「`ask = None` 是**显式禁用**，非『未实现』——装配层 MUST 逐臂显式置位」；`#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]` **逐字保留**（`Eq` 链完好是 AC-9 判据）
- [x] 同文件 `SessionOpen` 加字段：`/// core 铸的会话 id（ask 注册表登记键；引擎侧另持自己的 `sdk-…` 标识，两者不同义，MUST NOT 互换）` + `pub session_id: String`（位置在 `session` 之后、`prior_handle` 之前；`TurnQuestion` **逐字不动**）
- [x] `crates/core/agent/src/kernel.rs`：`SessionKernel` 增 `ask_registry: Arc<AskRegistry>` 字段；`new(sink, registry, streams, ask_registry)` 四参（doc 补一句「注册表由组合根注入，MUST NOT 内部自铸——否则与引擎侧登记的表不是同一张」）
- [x] 同文件 `RunningTurn` 增 `ask_registry: Arc<AskRegistry>` 私有字段；`begin_turn` 的 `Ok(RunningTurn { … })` 补 `ask_registry: Arc::clone(&self.ask_registry)`
- [x] 同文件 `begin_turn` 内 `SessionOpen { … }` 补 `session_id: session_id.clone()`（`let session_id` 已先于 `open_session` 算出，零重排）
- [x] 同文件 `RunningTurn::drive` 的解构补 `ask_registry`，并把三处 `Self::converge(...)` 调用与 `converge` 签名各加一参 `ask_registry: &AskRegistry`
- [x] 同文件 `converge` 内紧随 `registry.remove(session_id);` 增 `ask_registry.remove(session_id);`（doc 补一句「ask 槽随轮收敛兜底除名；take 后的空槽在 miss 语义下等价」）
- [x] 编译闭合批 —— `SessionKernel::new` **47 处**补第 4 参 `Arc::new(AskRegistry::default())`：`kernel_test.rs`(42) / `lib_test.rs`(1) / `compose.rs`(2，生产) / `compose_test.rs`(2)。**每用例一条显式 `Arc::new(AskRegistry::default())`，MUST NOT 引入全局静态**（避免测试间串表，R3b）
- [x] 编译闭合批 —— `SessionInjections` 字面构造 **32 处**补 `ask: None`：`core/runner_test`(9) / `sdk/loop_test`(8) / `sdk/tools_test`(6) / `sdk/runner_test`(4) / `sdk/mount_test`(2) / `kernel_test`(1) / `sdk/mount.rs`(1) / `compose.rs`(1，随阶段四四臂重写照应改，本阶段先补 `ask: None` 保编译)
- [x] 编译闭合批 —— `SessionOpen` 字面构造 **19 处**补 `session_id`：生产 `kernel.rs`(1，上条已落) / `core/runner_test`(2) / `compose_test`(3) / `lib_test`(6) / `sdk/config_test`(1) / `sdk/runner_test`(6)（测试侧给各不相同的固定串，便于断言「填的是 core 铸 id 而非引擎侧 id」）
- [x] 自查 阶段一（只列静态判据）：`grep -c "fn register\|fn answer\|fn remove" crates/core/agent/src/ask.rs` 为 3；`grep -rn "specta::Type" crates/core/agent/src/ask.rs` 零命中且 `runner.rs` 的 `AskMode` 定义块内零 `specta` 字样；`grep -rn "session_id" crates/core/agent/src/runner.rs` 只出现在 `SessionOpen`（`TurnQuestion` 块零命中）；`cargo clippy --workspace --all-targets` 通过；读码复核 `converge` 内 `registry.remove` 与 `ask_registry.remove` 相邻同位
- [x] 交接 阶段一（测试回归，本相位不执行）：`ask_test`（新）/ `runner_test`（四字段扩位 + serde 线值）/ `kernel_test`（四参夹具 + converge 除名断言 + `SessionOpen.session_id` 断言）**由桌面端 test-design / test-gen / test-execution 相位承接**；本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段二：orchestration ask 工具协议单点（`ask.rs` + `AskPort`，与阶段一可并行）

- [x] 新建 `crates/core/orchestration/src/ask.rs`：`pub const TOOL_NAME: &str = "AskUserQuestion";`（**全仓唯一定义处**，infra 引它与 `finish::TOOL_NAME` 并列）
- [x] 同文件 `pub struct AskQuestion { pub question: String, pub options: Vec<String> }`（`Debug + Clone + PartialEq + Eq`）
- [x] 同文件 `pub fn description() -> &'static str`：一段静态文案（<300 字：什么情况下该问用户 / 一次一个问题多个问题请多次调用 / `options` 必须非空且尽力给可选项 / 用户应答可为任一选项文本或自由文本 / 调用后本 turn 会阻塞直到用户应答）
- [x] 同文件 `pub fn parameters() -> serde_json::Value`：`{ "type": "object", "properties": { "question": { "type": "string", "description": "要问用户的问题（单问；多个问题请多次调用）" }, "options": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "候选选项（至少一个；用户也可自由作答）" } }, "required": ["question", "options"] }`——**静态 schema，无 prompt 依赖故零参**
- [x] 同文件 `pub fn validate(args: &serde_json::Value) -> Result<AskQuestion, String>`：① `args` 非对象 → `Err("参数形状非法：期望对象 { question, options }，当前非对象")`；② `question` 缺失 / 非字符串 / trim 后空 → `Err` 文案含字段名 + 「期望非空字符串」+ 当前值；③ `options` 缺失 / 非数组 → `Err` 含字段名 + 「期望非空字符串数组」+ 当前值；④ 元素非字符串 → `Err("字段 options 含非字符串元素：期望字符串数组")`；⑤ `options` 为空数组 → `Err("字段 options 为空数组：期望至少一个选项")`；⑥ 合法 → `Ok(AskQuestion { … })`
- [x] 同文件顶部 doc 写明「ask 工具定义与校验的**单点**：工具名、description、parameters、validate 四者只此一份；与收口工具的定义面并行独立（两模块互不引用，机械判据见 spec AC-4）」——**doc 措辞 MUST NOT 出现收口工具那个词**（含注释与字符串在内全文零命中是该条 AC 的 `grep` 判据；用「收口工具 / 与本模块并行的另一工具定义」指代它）
- [x] `crates/core/orchestration/src/lib.rs`：`pub mod ask;`（置于 `pub mod control;` 之前按字母序）+ `pub use ask::{description, parameters, validate, AskQuestion, TOOL_NAME};` + `pub use port::AskPort;`
- [x] `crates/core/orchestration/src/port.rs`：新增 `/// ask 编排状态面（实现驻壳层 sink）：`ask_started` 广播「本条 ask 已开始」（携完整载荷），`ask_resolved` 表达「本条 ask 结束」（停等态清理单点触发）。` + `pub trait AskPort: Send + Sync { fn ask_started(&self, ask: AskPayload); fn ask_resolved(&self); }`（`use crate::state::{ArchivePreflight, AskPayload, RunUpdate};` 的 use 面已含 `RunUpdate`，只需追加 `AskPayload`）
- [x] 自查 阶段二（静态判据）：`grep -ci "finish" crates/core/orchestration/src/ask.rs` 为 **0**（含注释、doc 与字符串——若命中，先改本文件措辞，MUST NOT 放宽判据）；`grep -c '"enum"' crates/core/orchestration/src/ask.rs` 为 0（options 是数组不走 enum）；`grep -n "minItems" crates/core/orchestration/src/ask.rs` 恰 1 处；`cargo clippy --workspace --all-targets` 通过
- [x] 交接 阶段二（测试回归，本相位不执行）：`ask_test`（新，六态校验 + 解耦守卫源码自证）**由桌面端 test-design / test-gen / test-execution 相位承接**；本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段三：orchestration 载荷扩展 + 应答通道转投 + 决策协议收窄

> 依赖阶段一（`AskRegistry`）与阶段二（`AskPort`）。三段同批落地：`state` 的变体收敛会触发 `control` / `walker` 的构造点，`control` 的 `answer` 转投又会牵 `walker` 的停等分支删除，拆开即不可编译或留下双通道。

- [x] `crates/core/orchestration/src/state.rs`：`AskPayload` 增两 additive 字段——`#[serde(default)] pub tool_call_id: Option<String>`（doc：行内卡对账：本次 ask 对应哪个 `tool_use`）/ `#[serde(default)] pub session_id: Option<String>`（doc：core 铸会话 id，转录面板定位用）；`Debug + Clone + PartialEq + Eq + Serialize + Deserialize + Type` 派生面零变
- [x] 同文件 `RunUpdate::Ask { question: String, options: Vec<String> }` 收敛为 `Ask { ask: AskPayload }`（doc 同步为「ask 中断（阻塞工具 `AskUserQuestion` 正在等待应答）」）；`kind()` 的 `RunUpdate::Ask { .. } => "ask"` 与 `From<&RunUpdate> for RunNotice` 的 `RunUpdate::Ask { .. } => RunNotice::Ask` **逐字不动**
- [x] `crates/core/orchestration/src/control.rs`：删 `#[derive(Default)]`（`ChangeFlowControl`），改 `pub fn new(ask_registry: Arc<agent::AskRegistry>) -> Self` —— 结构体增 `ask_registry: Arc<agent::AskRegistry>` 字段，`new` 内 `Self { runs: Mutex::new(HashMap::new()), ask_registry }`（doc 补一句「注册表由壳层托管、组合根注入：MUST NOT 内部自铸，否则与引擎侧登记的表不是同一张」）
- [x] 同文件删 `RunEntry.answer_tx` 字段与 `begin_run` 内的 `answer_tx: Mutex::new(None),`
- [x] 同文件 `publish` 的 Ask 臂改为 `RunUpdate::Ask { ask } => { entry.status = ChangeRunStatus::WaitingAsk; entry.ask = Some(ask.clone()); }`（其余四臂逐字不动）
- [x] 同文件 `answer()` 重写（design DD4 三段判定）：① `runs` 锁内 `get` 缺席 → `Err(format!("change \"{change_id}\" 无运行中的 run"))`；② 会话槽 `None` → `Err("当前无等待中的 ask".to_owned())`；③ `Some(session_id)` 克隆后**释放 `runs` 锁**，再 `self.ask_registry.answer(&session_id, text)`；doc 改为「应答通道唯一：命令只做投递，停等态清理由 ask 执行体返回后的 `AskPort::ask_resolved` 单点执行」
- [x] 同文件删 `RunGuard::wait_answer()`（整方法；`wait_confirm` / `cancelled` / `cancel_flag` / `emitter` / `finish` / `cancel_rx` / `register_pending` 逐字不动）
- [x] 同文件新增 `pub fn resolve_ask(&self, root: &str, change_id: &str)`（design DD5）：`runs` 锁内取 entry，`entry.ask = None` **无条件**；`if entry.status == ChangeRunStatus::WaitingAsk { entry.status = ChangeRunStatus::Running }`；克隆 `entry.updates` 后**锁外** `let _ = updates.send(RunNotice::Step);`；无 entry 时静默返回；doc 写明「`ask` 槽无条件清（行内卡判活靠 `tool_call_id`），`status` 条件回 `Running`（`Finished` 已先到时终态不被倒灌）；失效信号复用 `RunNotice::Step`——`Ask` / `ConfirmWait` 在通知面被读作『新停等已开始』」
- [x] **逐字不动复核点**：本阶段对 `accept_pending` / `register_pending` / `confirm()` / `wait_confirm()` / `cancelled_wait` 零改动（`git diff -- crates/core/orchestration/src/control.rs` 不应出现这五个符号的 hunk）
- [x] `crates/core/orchestration/src/finish.rs`：decision 形态 `parameters` 的 `action.enum` 收为 `["backtrack", "stop"]`，删 `question` / `options` 两个 property（`to` / `reason` 两 property 与 `required: ["action"]` 保留）；`description(FinishSpec::Decision)` 改写为「提交本步的路由决策（两动作封闭集之一）：backtrack（to 必须取 prompt 白名单内相位 + reason ≤500 字符）/ stop（reason ≤500 字符）。无法裁决时调用 `AskUserQuestion` 工具（问题 + 非空选项）。越权目标与超长 reason 会被拒绝」（**工具名只出现在文案字符串，非代码引用**）；evaluator 形态、`FinishPayload`、`validate` 逐字不变
- [x] `crates/core/orchestration/src/decision.rs`：`DecisionAction::Ask` 变体保留，doc 补「生产不可达：`finish_step` 的 decision 形态 `enum` 已不含 `ask`（提问迁出为独立工具 `AskUserQuestion`）；保留以支持历史 `SystemNotice{finish_step}` 载荷反序列化，walker 防御臂据此判漂移」；`validate_decision` 的 `"ask"` 臂与 `other` 臂文案**逐字保留**；`required_str` / `ensure_reason_len` / `ensure_backtrack_allowed` / `MAX_REASON_CHARS` 零变
- [x] `crates/core/orchestration/src/prompt.rs`：`decision_prompt` 的 `## 决策协议` 段改为两动作 + 一句指路——`- backtrack（to 取白名单内相位 id + reason ≤500 字符）：…` / `- stop（reason ≤500 字符）：终止本次运行。` / `无法裁决时调用 `AskUserQuestion` 工具提问（问题 + 非空选项，形状由工具 schema 给出）；应答后你可在同一轮内继续决策。`（**形状不复述**）；`backtrack` / `stop` 两条与红线句保留，删 `ask` 那条
- [x] 同文件删 `pub fn decision_answer_prompt`（整函数含 doc，`:131-139`）；`evaluator_protocol` / `backtrack_option_*` 三助手 / `spec_sync_prompt` / `merge_conflict_prompt` 逐字不动
- [x] `crates/core/orchestration/src/walker.rs`：`decision_session` 去循环化（design DD10）——删 `let mut prompt` / `let mut continue_session` / `loop {` 与其闭合，改为单轮直落：`run_worker(...)` → `decision-log` 挂账 → `let action = match outcome.finalized { … }`：`Some(FinishPayload::Decision(DecisionAction::Ask { .. }))` → `return Err(Terminal::failed("决策漂移：ask 应经 AskUserQuestion 工具提交".to_owned()))`（**不 emit、不停等、不开新轮**）；`Some(FinishPayload::Decision(action))` → `action`；`Some(FinishPayload::Evaluator(_))` → 现有文案保留；`None` → 现有截断 / 未收口两文案保留；随后 `WhitelistGate` 节点 emit 与 `ensure_backtrack_allowed` 判定**逐字不动**
- [x] 同文件 `resolve_deadlock` 的 `DecisionAction::Ask { .. }` 臂：结构逐字保留（仍 `Deadlock::Terminal(Terminal::failed(…))`），文案与注释改为「决策漂移：ask 应经 `AskUserQuestion` 工具提交」（原注释含 `wait_answer` 字样，必须改写——AC-8 的 `grep wait_answer` 零命中判据）
- [x] 同文件顶部 `use` 面：`prompt::{decision_answer_prompt, …}` 去 `decision_answer_prompt`（其余 import 逐字不动）
- [x] 编译闭合批 —— `RunUpdate::Ask` 字面构造 **21 处**收敛为 `RunUpdate::Ask { ask: AskPayload { question, options, tool_call_id: …, session_id: … } }`：`control.rs`(1，实现面已改写) / `control_test.rs`(7) / `state.rs`(2：`kind()` 与 `From` 的 match 臂) / `state_test.rs`(3) / `walker.rs`(2，其中决策会话那处随本条删除、`resolve_deadlock` 那处为 match 臂) / `walker_test.rs`(1) / `changes/mod_test.rs`(3) / `change_flow/mod_test.rs`(2)
- [x] 编译闭合批 —— `AskPayload` 字面构造 **2 处**补两字段：`control.rs`(1，随实现改写) / `state_test.rs:555`(补 `tool_call_id: None, session_id: None`)
- [x] 编译闭合批 —— `ChangeFlowControl::new` **254 处**加 `Arc<AskRegistry>` 实参：`control_test.rs`(63) / `walker_test.rs`(188) / `changes/mod_test.rs`(1) / `change_flow/mod_test.rs`(1) / `main.rs`(1，随阶段五)。两测试文件按 design R3b 用**文件内共享夹具**压平（`fn control() -> Arc<ChangeFlowControl>`），但每条用例的注册表实例独立（MUST NOT 全局静态）
- [x] 编译闭合批 —— `ChangeFlowControl::default` **9 处**（均在 `control_test.rs`）：`Default` 派生随 DD3 删除，9 处改为 `ChangeFlowControl::new(Arc::new(agent::AskRegistry::default()))`
- [x] 自查 阶段三（静态判据）：`grep -rn "wait_answer\|answer_tx" --include=*.rs crates src | grep -v _test` **零命中**（生产面——测试面改造归测试相位，见下条交接）；`grep -rn "decision_answer_prompt" --include=*.rs crates src | grep -v _test` 零命中；`grep -c "enum.*\[\"backtrack\", \"stop\", \"ask\"\]" crates/core/orchestration/src/finish.rs` 为 0 且 `["backtrack", "stop"]` 命中 1 处；`grep -c "question\|options" crates/core/orchestration/src/finish.rs` 只剩 description 文案内的 `问题` / 非空选项（**无 `"question"` / `"options"` 的 JSON 键字面量**）；`grep -c "已收口\|finish" crates/core/orchestration/src/ask.rs` 为 0（跨文件解耦）；`cargo clippy --workspace --all-targets` 通过；读码复核 `resolve_ask` 的锁序（`runs` 锁外 `send`）与 `answer` 的锁序（`runs` 锁释放后才调注册表）
- [x] 交接 阶段三（测试回归，本相位不执行）：`control_test` 的四条 `answer` 用例重写（转投 / 无 run / 无等待方 / 迟滞应答）、`state_test` 的两字段 serde 与变体投影、`walker_test` 的三条 ask 往返用例改防御臂、`finish_test` / `prompt_test` 的两动作断言、`change_flow/mod_test` 的 `wait_answer` 驱动用例改写，**均由桌面端 test-design / test-gen / test-execution 相位承接**；`confirm` 侧四条既有用例**零改动**（D6 的机械判据，由测试相位核验）。本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段四：infra/agent 挂载项 + 两档执行体 + 装配与注入缝

> 依赖阶段一（`AskRegistry` / `AskMode`）、阶段二（`orchestration::ask` 定义与校验）、阶段三（`AskPort` / `AskPayload` 两字段）。

- [x] `crates/infra/agent/src/sdk/mount.rs`：`ToolExec` 增第五臂 `/// ask 工具：一个变体带档位（schema 单源，仅执行体行为不同）` + `Ask(agent::AskMode)`（`Copy + Eq` 保持；位置在 `FinishStep(FinishSpec)` 之后）
- [x] 同文件 `definitions()`：在 `finish` 臂**之前**加独立 `if let Some(mode) = injections.ask { mounts.push(ToolMount { definition: ToolDefinition { name: orchestration::ask::TOOL_NAME.to_owned(), description: orchestration::ask::description().to_owned(), parameters: orchestration::ask::parameters() }, tier: ToolTier::Protocol, exec: ToolExec::Ask(mode) }); }`——两条 `if` **互不引用**（无 `ask.is_some() && finish.is_some()` 一类合取，design DD14 序：ask 先、finish 后）
- [x] 同文件顶部 doc 补一条 provider 侧不变量：「原生 ask 工具（provider 侧的 `AskUserQuestion`）一律不挂——本清单的 ask 项即全量覆盖，MUST NOT 同时下挂 provider 原生同名工具」
- [x] 同文件测试夹具 `injected_finish` 的 `agent::SessionInjections { … }` 补 `ask: None` 字段（阶段一的闭合批已补，此处确认零漏；`injected` 经 `injected_finish` 派生故只此一处）
- [x] `crates/infra/agent/src/sdk/loop.rs`：新增 `pub(crate) struct AskContext { pub(crate) registry: Arc<agent::AskRegistry>, /// 注册表键：core 铸会话 id（**不是** `LoopTurn.session_id`——后者是引擎侧 `sdk-…` 标识） pub(crate) core_session_id: String, /// 编排侧状态面（仅 `Flow` 档消费；`Generic` 档恒 `None`） pub(crate) port: Option<Arc<dyn orchestration::port::AskPort>> }`（design DD6）
- [x] 同文件 `LoopTurn` 增 `pub ask: Option<AskContext>`（doc：per-turn 铸造，按 `injections.ask` 位；`None` = 档位未装配，工具调用以 is_error 回灌）
- [x] 同文件新增私有 `async fn execute_ask(ctx: Option<&AskContext>, mode: agent::AskMode, args: &serde_json::Value, tool_call_id: &str, handle: &RunHandle) -> (String, bool)`：
  - `validate(args)` `Err(msg)` → `(msg, true)`；
  - `ctx` 为 `None` → `("ask 执行上下文未装配".to_owned(), true)`；
  - `mode == Flow` 且 `ctx.port` 为 `None` → `("ask 编排状态面未装配".to_owned(), true)`（design DD2：装配漂移防御臂，生产不可达——DD15 保证两条生产轨道恒传 `Some(port)`）；
  - `Flow` 档先 `port.ask_started(AskPayload { question, options, tool_call_id: Some(tool_call_id.to_owned()), session_id: Some(core_session_id.clone()) })`；
  - `let (tx, rx) = oneshot::channel::<String>(); ctx.registry.register(&core_session_id, tx);`（返回值按 DD1 只写引擎日志，不作失败面）；
  - `let outcome = tokio::select! { answer = rx => match answer { Ok(text) => (text, false), Err(_) => ("ask 等待通道已关闭（会话已终止）".to_owned(), true) }, _ = handle.wait_requested() => ("用户未应答（会话被终止）".to_owned(), true) };`
  - `if mode == Flow { if let Some(port) = &ctx.port { port.ask_resolved(); } }`（**两臂返回后都执行，恰一次**——design ⓪）；
  - `outcome`
- [x] 同文件 `match mount.exec` 增臂 `ToolExec::Ask(mode) => execute_ask(turn.ask.as_ref(), mode, &input, &id, &handle).await,`（其余四臂逐字不动；注意 `input` 是 `sandbox_rewrite` 后的值——ask 无 `path` 字段，`input_path` 走 `path` 取值 → 缺失 → `None` → 沙箱原样返回，零特判）
- [x] 同文件 `use` 面补三行：`use tokio::sync::{mpsc, oneshot};`（原为 `use tokio::sync::mpsc;`）、`use orchestration::ask as ask_tool;`（避免与 `orchestration::finish` 之外再造第三处同名 `ask` 歧义；或直接 `use orchestration::{ask as ask_tool, finish};` 并调 `ask_tool::TOOL_NAME` / `ask_tool::validate`）、`use orchestration::state::AskPayload;`；`AskContext` 的 `Arc<agent::AskRegistry>` 经 `agent::` 全路径引用（不新增 `use`）
- [x] 同文件循环头注释补一句「ask 是唯一可阻塞数十秒的执行臂（阻塞期间不收敛、不发新请求；停止经外层泵 select 或本臂停止臂截停）」；`handle` 在该作用域可用性随实现确认（`run(... handle: RunHandle)` 已按值持有，按需 `&handle`）
- [x] `crates/infra/agent/src/sdk/runner.rs`：`SdkRunner` 增 `ask_registry: Arc<agent::AskRegistry>` 与 `ask_port: Option<Arc<dyn orchestration::port::AskPort>>` 两字段；`new(config, resume, context_window, explore_store)` 签名**不动**，两字段初值 `Arc::new(agent::AskRegistry::default())`（未注入即空表 + 阻塞恒 miss——fail-closed）与 `None`
- [x] 同文件增两条 builder 缝（与 `with_explore_note_store` 同型）：`pub fn with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self` / `pub fn with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self`
- [x] 同文件 `PumpPayload` 增同两字段；`open_session` 的 `PumpPayload { … }` 补 `ask_registry: Arc::clone(&self.ask_registry), ask_port: self.ask_port.clone(),`
- [x] 同文件 `session_pump` 在 `LoopTurn` 铸造前加 per-turn 铸造：`let ask = open.injections.ask.map(|mode| AskContext { registry: Arc::clone(&payload.ask_registry), core_session_id: open.session_id.clone(), port: if mode == agent::AskMode::Flow { payload.ask_port.clone() } else { None } });` 并在 `LoopTurn { … }` 补 `ask,`（doc 补一句「判据 = 位存在（与 explore / archi 的『名字包含』判据并列的两类铸造判据）；`core_session_id` 取 `SessionOpen.session_id`（core 铸 id），MUST NOT 取 `turn.session_id`」）
- [x] `crates/infra/agent/src/lib.rs`：`EngineFacade` 增 `ask_registry: Arc<agent::AskRegistry>`（`new` / `with_resume_transcript` 内 `Arc::new(agent::AskRegistry::default())`）与 `ask_port: Option<Arc<dyn orchestration::port::AskPort>>`（初值 `None`）两字段——**注意**：`new` 与 `with_resume_transcript` 两构造点的字段清单都要补（`EngineFacade::new` 与 `with_resume_transcript` 都是全字段字面构造，漏补即编译失败）
- [x] 同文件增 `pub fn with_ask_registry(self, registry: Arc<agent::AskRegistry>) -> Self` 与 `pub fn with_ask_port(self, port: Option<Arc<dyn orchestration::port::AskPort>>) -> Self`（与既有五条 builder 同型：`Self { resume: self.resume, context_window: …, …, ask_registry: registry, ask_port: self.ask_port }` 全字段列举）
- [x] 同文件 `runner_for` 的 `EngineKind::Sdk` 臂：`sdk::runner::SdkRunner::new(…).with_max_tokens(…).with_web_search(…).with_ask_registry(Arc::clone(&self.ask_registry)).with_ask_port(self.ask_port.clone())`（其后 `with_workspace_root` / `with_explore_note_store` 的既有链式顺序保持）
- [x] `crates/infra/agent/src/compose.rs`：`compose_turn` 签名加两参——`ask_registry: Arc<agent::AskRegistry>`（置于 `streams` 之后）与 `ask_port: Option<Arc<dyn orchestration::port::AskPort>>`（置于 `workspace_root` 之后）；doc 补「注册表透传内核 + 门面（两侧同源单表）；`ask_port` 仅门面（编排侧状态面的实现方是壳层 sink，内核侧不需要它）」
- [x] 同文件 `SessionKernel::new(Arc::new(StoreSink::new(store)), registry, streams, Arc::clone(&ask_registry))`；`EngineFacade` 链加 `.with_ask_registry(ask_registry)` / `.with_ask_port(ask_port)`（置于 `.with_explore_note_store(...)` 之前）
- [x] 同文件 `assemble_injections` 四臂置 `ask` 位：`injected` helper 增第 4 形参 `ask: Option<agent::AskMode>` 并写入结构体；explore 臂 `injected(&EXPLORE_TOOLS, None, None, Some(agent::AskMode::Generic), provenance)`；archi 臂 `injected(&names, knowledge(), None, Some(agent::AskMode::Generic), provenance)`；change 臂 `injected(&names, None, finish, Some(agent::AskMode::Flow), provenance)`（**不分 role / 不分矩阵命中**）；**其余来源臂**改为显式结构字面量 `Ok(agent::SessionInjections { preamble: None, tools: None, finish: None, ask: Some(agent::AskMode::Generic) })`——spec 钉死该臂 `tools = None`（内建七件，不经名字清单、无需成员校验），**MUST NOT** 保留 `return Ok(agent::SessionInjections::default())`（其 `ask = None` 即静默禁用，design ④ / R8）
- [x] 同文件 `composed_with_runner` 测试夹具的 `SessionKernel::new(…)` 补 `Arc::new(agent::AskRegistry::default())`
- [x] 编译闭合批 —— `compose_turn` **46 处**加两实参：`compose_test.rs`(36) / `worker_test.rs`(8)（测试侧传 `Arc::new(agent::AskRegistry::default())` 与 `None`）/ `change_flow/mod.rs`(1) / `exec/mod.rs`(1)（生产侧随阶段五填真值）
- [x] 编译闭合批 —— `LoopTurn` 字面构造（约 40 处，`sdk/loop_test.rs` 约 39 经 `..loop_turn(...)` 兜底）：在 helper `loop_turn` / `loop_turn_with_fast_io` / `turbo_loop_turn` 等的构造体内补 `ask: None`（一处即可覆盖多数用例）；直构且不展开的用例按需补 `ask: None`；生产 `sdk/runner.rs` 已在上条落
- [x] 自查 阶段四（静态判据）：`grep -c "ToolExec::Ask" crates/infra/agent/src/sdk/loop.rs` ≥ 1；`grep -c "ask_resolved()" crates/infra/agent/src/sdk/loop.rs` 为 **1**（单出口形态：`select!` 结果先落 `outcome`，随后 `Flow` 档调一次——两臂都经该出口，故恰一次）；`grep -rn "finish.is_some() && \|ask.is_some() && " crates/infra/agent/src/sdk/mount.rs` 零命中（无合取）；`grep -n "SessionInjections::default()" crates/infra/agent/src/compose.rs` 零命中（④ 的机械判据）；`grep -c "with_ask_registry\|with_ask_port" crates/infra/agent/src/lib.rs` 为 2 定义 + 2 链式调用点；`cargo clippy --workspace --all-targets` 通过；读码复核 `AskContext.core_session_id` 取自 `open.session_id` 而非 `turn.session_id`
- [x] 交接 阶段四（测试回归，本相位不执行）：`mount_test`（四组合 + tier）、`loop_test`（两档执行体 / 停止臂 / 上下文缺席）、`sdk/runner_test`（per-turn 铸造）、`compose_test`（四臂 ask 位）、`worker_test`（`AskPort` 透传）**由桌面端 test-design / test-gen / test-execution 相位承接**；本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段五：壳层托管 + `agent_answer` 命令 + `AskPort` 实现

- [x] `src/main.rs`：`use agent::{AskRegistry, SessionStreamRegistry, StopRegistry};`；`setup_hook` 内 `SessionStreamRegistry` 之后加 `// agent ask 应答注册表（会话级，键 = core session id，与 StopRegistry 同型托管）：ask 执行体 register / 应答命令 answer / 内核 converge 除名，此处只挂空表` + `app.manage(Arc::new(AskRegistry::default()));`
- [x] 同文件 `ChangeFlowControl::new()` → `ChangeFlowControl::new(Arc::clone(app.state::<Arc<AskRegistry>>().inner()))`（注释补「ask 注册表由壳层托管后注入——MUST NOT 内部自铸」）
- [x] `src/commands/exec/mod.rs`：`use ::agent::AskRegistry`（并入既有 `use ::agent::{…}` 清单）；`agent_start_with` 内加 `let ask_registry = app.state::<Arc<AskRegistry>>();`（与 `registry` / `streams` 同段），`compose_turn(...)` 调用补 `Arc::clone(ask_registry.inner())` 与 `Some(Arc::new(ChatAskPort))`；注释补「ask 注册表 `Arc` 先取出再进组合根（不跨 await 持 `State` 句柄）」
- [x] 同文件新增 chat 侧空 `AskPort`（design DD15）：
  ```rust
  /// change-chat 会话的 ask 编排状态面（无 run 形态）：change 来源的 ask 位恒
  /// `Some(Flow)`，而 chat 会话没有 ChangeFlowControl 条目——空实现把「本会话无
  /// 编排停等面」这一事实类型化，使 `Flow` 档的两条广播成为显式无操作（而非
  /// `port` 缺席触发的 is_error 防御臂）。
  struct ChatAskPort;
  impl AskPort for ChatAskPort {
      fn ask_started(&self, _ask: AskPayload) {}
      fn ask_resolved(&self) {}
  }
  ```
  `use orchestration::port::AskPort;` + `use orchestration::state::AskPayload;`
- [x] 同文件新增命令（薄包装三段，沿 `agent_stop` 形态）：

  ```rust
  /// ask 应答（薄包装三段：参数校验 → 注册表转投 → 错误映射）：blank 参数 →
  /// 显式 Err（携字段名）；未登记 / 已应答 → Err「当前无等待中的 ask」；命中 →
  /// Ok。零状态机操作、零通知广播、零流程停等态清理（清理单点在 ask 执行体
  /// 返回后）。miss 显式 Err 而非幂等 Ok——重复 / 迟到应答都是用户可感知的
  /// 「卡已不在」事实，与 agent_stop 的 miss 幂等是不同语义。
  #[tauri::command]
  #[specta::specta]
  pub fn agent_answer(
      registry: State<'_, Arc<AskRegistry>>,
      root: String,
      session_id: String,
      text: String,
  ) -> Result<(), String> {
      if is_blank_root(&root) { return Err("非法 root: 不得为空白".to_owned()); }
      if session_id.trim().is_empty() { return Err("非法 session_id: 不得为空白".to_owned()); }
      registry.answer(&session_id, text)
  }
  ```
- [x] `src/commands/mod.rs`：`all_commands!` 内 `$crate::commands::exec::agent_stop,` 之后加 `$crate::commands::exec::agent_answer,`
- [x] `src/commands/change_flow/mod.rs`：`impl AskPort for ChangeFlowSink { fn ask_started(&self, ask: AskPayload) { self.control.publish(&self.root, &self.change_id, RunUpdate::Ask { ask }); } fn ask_resolved(&self) { self.control.resolve_ask(&self.root, &self.change_id); } }`（`use orchestration::port::{AskPort, …}` + `use orchestration::state::AskPayload`）
- [x] 同文件 `change_flow_start_with` **换序**（design 事实校准 9）：把 `let sink: Arc<dyn RunEventSink> = Arc::new(ChangeFlowSink { … });` 连同其 `Arc` 提前到 `compose_turn` **之前**，改为 `let sink_impl = Arc::new(ChangeFlowSink { root: root.clone(), change_id: id.clone(), control: Arc::clone(&control) }); let sink: Arc<dyn RunEventSink> = Arc::clone(&sink_impl) as Arc<dyn RunEventSink>; let ask_port: Arc<dyn AskPort> = sink_impl;`，`compose_turn(...)` 补 `Arc::clone(app.state::<Arc<AskRegistry>>().inner())` 与 `Some(Arc::clone(&ask_port))`；`KernelWorkerPort::new(composed, sink)` 不变（注释补「sink 同一实例两 trait 视图：`RunEventSink` 给 worker、`AskPort` 给引擎门面——ask 的状态广播与 run 的状态广播同源」）
- [x] 同文件 `change_flow_answer_with` 保持调 `control.answer`（doc 补「应答只投 `AskRegistry`（经 `current_session` 翻译两段寻址）；停等态清理由执行体返回后单点执行」）
- [x] 自查 阶段五（静态判据）：`grep -rn "AskRegistry" src/main.rs src/commands/exec/mod.rs src/commands/change_flow/mod.rs` 各有命中；`grep -c "agent_answer" src/commands/mod.rs` 为 1；`grep -c "impl AskPort for ChangeFlowSink" src/commands/change_flow/mod.rs` 为 1；`grep -n "compose_turn" -A12 src/commands/change_flow/mod.rs` 复核 sink 构造**先于** `compose_turn`；`grep -c "ChatAskPort" src/commands/exec/mod.rs` ≥ 2（定义 + 使用点）；`cargo clippy --workspace --all-targets` 通过
- [x] 交接 阶段五（测试回归 + 生成物，本相位不执行）：`exec/mod_test`（`agent_answer` 四态）与 `change_flow/mod_test`（两入口各一条）**由桌面端 test-design / test-gen / test-execution 相位承接**；`bindings:export` 再生成与 `src/types/generated/**` 的 diff 核验由桌面端 **static-check** 承接（期望：新增 `agentAnswer` 命令 + `AskPayload` 两字段，其余零 diff）。本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段六：前端（生成物 + hook 动作 + 双态卡 + 七处接线）

> 生成物（`bindings.ts` / `bindings.zod.ts`）由阶段五的命令登记驱动，先跑一次 `pnpm -C packages/desktop run bindings:export` 取回再继续（本相位只**触发再生成并提交产物**，MUST NOT 手改生成物）。

- [x] `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts` 与 `bindings.zod.ts`（期望新增 `agentAnswer` 命令与 `AskPayload` 的 `toolCallId` / `sessionId` 两字段；其余零 diff）——产物随变更提交
- [x] `packages/desktop/src/hooks/use-agent-session.ts`：`UseAgentSessionState` 增 `/** ask 应答（通用侧）：`agent_answer` 薄包装；`root` / `sessionId` 为 null 时不 invoke、`Err` 进既有 error 面 */ answer: (text: string) => void;`
- [x] 同文件 `useSessionActions`：增 `const [actionError, setActionError] = useState<string | null>(null);` 与 `const answer = useCallback((text: string) => { const target = mirrors.currentSessionIdRef.current ?? sessionId; if (root === null || target === null) return; setActionError(null); void ipc.agentAnswer(root, target, text).catch((cause: unknown) => setActionError(String(cause))); }, [mirrors.currentSessionIdRef, sessionId, root]);`（`sessionId` / `root` 为函数内既有解构；`String(cause)` 沿用本文件既有 catch 归一惯例，`:383` 同款）
- [x] 同文件 `useSessionActions` 返回类型声明补 `answer: (text: string) => void; actionError: string | null;`，返回面增 `answer` 与 `actionError`
- [x] 同文件 `useAgentSession` 返回面：`error: replay.error ?? (error !== undefined ? error.message : null) ?? actions.actionError` → 改为保序三源合并 `error: replay.error ?? (error !== undefined ? error.message : null) ?? actions.actionError`（design DD11：重放 > 流 > 动作），并把 `...actions` 展开保持（`answer` 随展开透出）
- [x] `packages/desktop/src/components/agent/tool-cards.tsx`：`AgentToolCardProps` 增 `/** ask 应答动作面（在场且本卡为活卡时呈可交互形态；缺席即静态只读卡） */ askAnswer?: { onAnswer: (text: string) => void };`
- [x] 同文件 `parseAsk` 双形态归一（design DD9）：`function parseAsk(input: unknown): AskQuestion[] | null`——① 单问形态 `{ question: string, options: unknown[] }`（`question` 为字符串即命中；`options` 元素为字符串 → `{ label: str, description: null }`，元素为对象 → 取 `label` / `description`）；② 历史形态 `{ questions: [{ question, options }] }`（现 `parseAskInput` 逻辑保留）；③ 皆不匹配 → `null`。`AskOption` / `AskQuestion` 两接口保留（`label: string; description: string | null` / `question: string; options: AskOption[]`）
- [x] 同文件 `AskUserQuestionCard` 双态：签名改 `({ input, askAnswer }: AgentToolCardProps)`；`const [submitted, setSubmitted] = useState(false);` 与 `const [text, setText] = useState('');`；`const live = askAnswer !== undefined && !submitted;`；`const submit = (value: string) => { const trimmed = value.trim(); if (trimmed === '' || askAnswer === undefined || submitted) return; setSubmitted(true); setText(''); askAnswer.onAnswer(trimmed); };`
  - 卡头文案改为中性（**删除「请在下方输入框作答」**）：静态态「向用户提问（AskUserQuestion）」、活卡态「向用户提问（AskUserQuestion）· 选择一个选项或自行作答」
  - `questions === null` → 保留 `<pre>{JSON.stringify(input, null, 2)}</pre>` 回退（`data-testid="ask-question-card"` 不变）
  - 静态态与现形态逐字一致（`ask-question` / `ask-option` 钩子与类名不变）
  - 活卡态：每个 option 渲成 `<Button data-testid="ask-answer-option" onClick={() => submit(option.label)} disabled={submitted}>`（在 `AskQuestionView` 内按 `live` / `onPick` 分支，静态态仍渲 `<li data-testid="ask-option">`）；下方 `<input data-testid="ask-answer-input" value={text} onChange={…} onKeyDown={(e) => e.key === 'Enter' && submit(text)} />` + `<Button data-testid="ask-answer-submit" onClick={() => submit(text)} disabled={submitted || text.trim() === ''}>应答</Button>`
  - 卡根 `cardAttrs(toolCallId)`（`data-testid="block-tool-use"` + `data-tool-id`）逐字保留
- [x] `packages/desktop/src/components/agent/agent-messages.tsx`：导出 `export interface AskAnswerActions { onAnswer: (text: string) => void; /** 明确「当前正在等哪一条」（null = 当前无等待；undefined = 未提供，按未配对判活） */ pendingToolCallId?: string | null; }`；`AgentMessagesProps` 增 `askAnswer?: AskAnswerActions;`
- [x] 同文件 `ToolPartView` 增第二参（design DD8 三态矩阵）：`function ToolPartView({ part, askAnswer }: { part: AgentToolPart; askAnswer?: AskAnswerActions })`——当 `toolName === 'AskUserQuestion'` 时按下列判活决定是否透传 `askAnswer`：`pendingToolCallId === null` → 不透传（全静态）；`typeof pendingToolCallId === 'string'` → `part.toolCallId === pendingToolCallId ? askAnswer : undefined`；`pendingToolCallId === undefined` → `part.state === 'input-available' ? askAnswer : undefined`；其余工具名**恒不透传**
- [x] 同文件 `PartView` / `ChatBubble` 增 `askAnswer` 透传（`ChatBubbleBase` 的第二参 `{ message, askAnswer }`，`ChatBubble` 的 `memo` 比较面因此纳入 `askAnswer`——引用稳定由上层 `useMemo` 保证，design DD12）
- [x] 同文件 `MessageContainer` 增 `askAnswer` 入参并经 `renderItem` / `renderSticky` 传给 `ChatBubble`（依赖数组纳入 `askAnswer`）；`AgentMessages` 内 `const stableAskAnswer = useMemo(() => askAnswer, [askAnswer?.onAnswer, askAnswer?.pendingToolCallId ?? null]);` 用于下行（design DD12：避免每渲染新对象击穿 memo 与虚拟列表 `useCallback`）
- [x] `packages/desktop/src/components/agent/agent-session-view.tsx`：`AgentSessionViewProps` 增 `/** ask 应答动作面（可选注入位；与 composer 插槽互不前置——只读消费点可只接它） */ askAnswer?: AskAnswerActions;`；`<AgentMessages … askAnswer={askAnswer} />`（**自持形态不自动接线**，design DD7——只做原样下行）
- [x] `packages/desktop/src/components/agent/session-panel.tsx`：`ExpandedBody` 内 `<AgentSessionView session={state.session} composer={composer} askAnswer={askAnswerFrom(state)} … />`，`const askAnswerFrom = (state: SessionPanelState): AskAnswerActions => ({ onAnswer: state.session.answer });`（同文件顶部小 helper 或内联 `{{ onAnswer: state.session.answer }}`；`SessionPanelState` 形状零改动）
- [x] `packages/desktop/src/components/agent/index.tsx`：增 `export type { AskAnswerActions } from './agent-messages';`（proposal 表未列、实现必需的最小面：流程侧三文件经 barrel 引入类型，避免深路径第二种导入形态）
- [x] `packages/desktop/src/views/explores/explore-detail-pane.tsx`：`ChatPane` 的 `<AgentSessionView … askAnswer={{ onAnswer: session.session.answer }} />`
- [x] `packages/desktop/src/views/agent/agent-debug-view.tsx`：`<AgentSessionView session={session} askAnswer={{ onAnswer: session.answer }} data-testid="agent-timeline" />`
- [x] `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`：`SessionTranscriptPanelProps` 增 `/** ask 应答动作面（唯一例外：本面板其余动作面仍缺席——无发送 / 停止入口） */ askAnswer?: AskAnswerActions;`；`viewProps` 在场时并入 `askAnswer`（`...(askAnswer ? { askAnswer } : {})` 或直接把 `askAnswer` 传入——缺席即 `undefined` 与不传等价）
- [x] `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：`DetailDrawerProps` 与 `DrawerContent` 各增 `askAnswer?: AskAnswerActions`，逐层原样透传至 `SessionTranscriptPanel`（未传即零行为变化）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`DetailLoadedProps` 增 `askAnswer: AskAnswerActions`，`DetailLoaded` 内 `const askAnswer = useMemo<AskAnswerActions>(() => ({ onAnswer: (text: string) => { void run.answer(text); }, pendingToolCallId: activeRun?.ask?.toolCallId ?? null }), [run.answer, activeRun?.ask?.toolCallId]);`（design DD13：走 `run.answer` 使 `Err` 落 `run.error` 既有错误行）→ `DetailTabs` → `DetailContent` → `DetailDrawer` 逐层传（`DetailTabs` / `DetailContent` 各增同名 prop）
- [x] 自查 阶段六（静态判据）：`grep -rn "请在下方输入框作答" packages/desktop/src` **零命中**；`grep -c "pendingToolCallId" packages/desktop/src/components/agent/agent-messages.tsx` ≥ 3（三态分支各一处）；`grep -rn "ipc\.\(changeFlowAnswer\|agentAnswer\)" packages/desktop/src/components packages/desktop/src/hooks` 只允许出现在 `hooks/use-agent-session.ts`（组件族零 IPC——「组件族不认识业务」纪律的机械判据）；`git diff --stat -- packages/desktop/src` 的文件面 = design「修改文件」表所列 11 项（含 `components/agent/index.tsx` 的一行类型再导出）+ `types/generated/**` 两文件（`agent-blocks.tsx` / `run-control-panel.tsx` / `lib/agent-adapter.ts` / `lib/agent-transport.ts` **零 diff**）；`pnpm -C packages/desktop exec vp check` 通过、`knip` 无新增未用导出（`AskAnswerActions` 被三处消费即为已用）
- [x] 交接 阶段六（测试回归，本相位不执行）：`tool-cards.test.tsx`（双态 + 双形态解析）/ `agent-messages.test.tsx`（三态判活）/ `agent-session-view.test.tsx`（透传）/ `use-agent-session.test.ts`（`answer` 参数面与 null 守卫）/ `session-panel.test.tsx`（骨架合成）/ `session-transcript-panel.test.tsx` 与 `change-detail-view.test.tsx`（流程侧接线）**由桌面端 test-design / test-gen / test-execution 相位承接**；既有 `run-control-panel.test.tsx` 的 `AskCard` 断言**零改动**即应通过（additive 字段不参与其渲染）。本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**

## 阶段七：版本、边界自查与交接备忘

- [x] 修改 `packages/desktop/package.json` 的 `version`：按**落地时 `HEAD` 现值 +1 patch** 相对递增（当前盘面 `0.6.35` → `0.6.36`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号）；`dependencies` / `devDependencies` **零变化**
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改，跟随即生效）；`src-tauri/Cargo.toml` 的 crate 版本行**未被改动**
- [x] 自查 边界零 diff（逐项 `git diff --stat`）：`crates/infra/store/**`、`crates/core/workflow/**`（相位表 / 14 份 prompt md / `include_str!`）、`crates/infra/agent/src/sdk/protect/**`、`crates/infra/agent/src/sdk/{sandbox,context,compact,resume,normalize,web_search,bash,model_spec,tools}.rs`、`crates/core/orchestration/src/{verdict,transcript}.rs`、`packages/desktop/src/lib/**`、`packages/desktop/src/components/agent/agent-blocks.tsx`、`packages/desktop/src/views/changes/flow/run-control-panel.tsx`、`.dev-team/architecture/models/**`、全部 `Cargo.toml` —— 以上**整文件零 diff**
- [x] 自查 关键判据集中复核（AC 逐条，均为 `grep` / 读码 / `git diff` 面，不含任何用例运行）：
  - AC-1 / AC-3：`assemble_injections` 四臂的 `ask` 位逐臂复核（explore / archi / 其余 → `Generic`，change → `Flow`）；`SessionInjections::default()` 在 `compose.rs` 零命中
  - AC-2：`AskPort for ChangeFlowSink` 两方法体各一行（`publish(RunUpdate::Ask { ask })` / `resolve_ask`）；`AskPayload` 两字段的赋值点在 `execute_ask` 一处
  - AC-4：`grep -c "finish" crates/core/orchestration/src/ask.rs` 为 0；`finish.rs` 的 `"question"` / `"options"` JSON 键字面量零命中；`mount.rs` 两 `if` 无合取
  - AC-5：`execute_ask` 的 `select!` 两臂返回后都走 `ask_resolved`（读码）；`converge` 内 `ask_registry.remove` 在场
  - AC-13（流程侧例外面）：`grep -c "impl AskPort for ChatAskPort" src/commands/exec/mod.rs` 为 **1**，且 `compose_turn` 调用处传 `Some(Arc::new(ChatAskPort))`（DD15 的机械判据：change-chat 的 `Flow` 档仍有 port）
  - AC-6：`orchestration::ask::parameters` 的 `minItems: 1`；`validate` 六条 `Err` 文案
  - AC-7：`AskRegistry::answer` 的 `remove` + `Err("当前无等待中的 ask")`；两命令体零二次投递
  - AC-8：`grep -rn "wait_answer\|answer_tx" --include=*.rs crates src | grep -v _test` 零命中；`git diff -- crates/core/orchestration/src/control.rs` 不含 `accept_pending` / `confirm` / `wait_confirm` 的 hunk
  - AC-9：`bindings/mod_test.rs` 反向锚定清单**零改动**（`AskMode` 挂在 `SessionInjections` 上、不派生 `specta::Type`）；`RunNotice` 六变体与 `kind()` 零 diff
  - AC-10：`agent_answer` 在 `all_commands!` 内；命令体 `root` / `session_id` 两参 blank 校验 + 单行转投（`text` 不校验）
  - AC-11 / AC-13：阶段六的自查项复跑一次（旧文案零命中、组件族零 IPC、文件面清单）
  - AC-12：`finish_step` decision 形态的 `enum` 两值 / 两 property 删除 / description 指路；`decision_session` 无 `loop`；`resolve_deadlock` 的 `Ask` 臂在场；`DecisionAction::Ask` 与 `validate_decision` 的 `"ask"` 臂在场
- [x] 登记计划外收敛（若有）：实现中若出现「任务未列、实现必需」的最小面，如实留痕于本节（说明为何必需、为何不改变 design 任何裁决的语义面），以免下轮重演
  **计划外收敛（实现必需，均不改变 design 任何裁决语义）**：

  1. `crates/core/orchestration/src/lib.rs` 的根再导出**只加了 `AskQuestion` / `TOOL_NAME`**：`description` / `parameters` / `validate` 三名与本文件既有 `pub use finish::{description, parameters, validate, …}` 在同值命名空间冲突（`E0252`），Rust 不允许两模块同名项并存于同一根。三函数经 `ask::` 模块路径消费（infra 即 `orchestration::ask::description()` / `::parameters()` / `::validate()`），design「修改文件」表该行的出口面按此读作「六名在 crate 内可到达」；改名（如 `ask_description`）会造出第二套调名，与「工具名 / description / parameters / validate 只此一份」的单点纪律相悖。
  2. `packages/desktop/src/hooks/use-agent-session.ts` 把应答动作抽成独立 `useAskAnswer(mirrors, sessionId, root)`（返回 `{ answer, actionError }`）：`useSessionActions` 加 `answer` 后超 `vp check` 的 per-function 行数上限。语义与 design DD11 逐条一致（`root` / `sessionId` 为 null 零 invoke、`Err` 进合并错误面、`??` 保序三源），hook 返回面 `...actions` + `answer: ask.answer` 与 spec 的 `UseAgentSessionState.answer` 等价。
  3. `packages/desktop/src/components/agent/agent-messages.tsx` 的 `stableAskAnswer` **保留三态**（`undefined` 不归一成 `null`）：DD8 的第三态（未提供 → 按未配对判活）与第二态（`null` → 全静态）语义不同，`?? null` 归一会让「未提供」退化为「全静态」，使通用会话面的活卡永不成立。
  4. `packages/desktop/src/components/agent/tool-cards.tsx` 的 `parseOptions` 增**裸字符串元素**分支：新工具 schema 是 `options: string[]`（DD9 形态①），既有实现只认 `{ label, description }` 对象；不补则该形态每条选项渲染成空串。DD9 已明文覆盖该形态，此处只是落点补齐。
  5. `crates/infra/agent/src/compose.rs` 顶部 doc 的「不得用默认构造」表述**避开 `SessionInjections::default()` 字面量**：阶段四自查判据要求该串在本文件零命中，而 design 原 doc 引用了它；改述后代码面与 doc 面同时满足判据，语义不变。
  6. `#[cfg(test)] mod ask_test;` 两处声明**未随本相位落地**：本仓惯例是「声明与 sidecar 文件同批」（既有 12 + 10 条声明全都有对应文件），先加声明会让 `cargo check --all-targets` 因缺文件失败，与本相位的「可编译」底线冲突。**移交 test-gen**：创建 `crates/core/agent/src/ask_test.rs` 与 `crates/core/orchestration/src/ask_test.rs` 时同批加两行声明（分别置于 `mod event_test;` 之前与 `mod control_test;` 之前，按字母序）。
  7. **交付面行尾归位（回退窗口修复，零语义变化）**：写盘时有文件被整篇换了行尾风格，使 `git diff` 面被 EOL 差异放大。已按字节归位到各自 `HEAD` 的既有风格（未改一个字符语义，逐文件复核内容等价）：① `src/commands/mod.rs` 与 `crates/infra/agent/src/compose_test.rs` 原为纯 LF 却被整篇转成 CRLF，已归一回纯 LF（CRLF 数 91 到 0 / 4349 到 0，无孤立 CR），`git diff` 面由 8072 + 176 行收回到`compose_test.rs` 674 行变动、`mod.rs` 6 行插入；② `crates/infra/agent/src/lib.rs` 的 `HEAD` 为纯 CRLF，新增行里 4 行落成 LF，已按 `HEAD` 风格回 CRLF；③ `src/commands/exec/mod_test.rs` 的 `HEAD` 为 CRLF 主导（2918/2920），新增行整篇落成 LF，已逐行对齐回 `HEAD` 风格（既有 2 行 LF 原样保留），`git diff` 面由 3391 + 2918 行收回到 475 + 2 行。全变更面现 `git diff --shortstat` 与 `--ignore-cr-at-eol` 口径**逐字一致**（`src-tauri` 9839 + 999 行、`src` 3319 + 54 行），即零 EOL 噪声。

- [x] 交接交付门（本相位不执行，只登记期望并向桌面端移交）：`bindings:export` / `api:check` / `client:check` 三段链（`pnpm -C packages/desktop run check`）与 `packages/desktop/src/types/generated/**` 的再生成核实归桌面端 **static-check**（期望：`agentAnswer` 命令与 `AskPayload` 两字段出线，`AskMode` / `SessionInjections` / `AskContext` 不出线）；**测试回归**（`cargo test` 全 workspace、`pnpm -C packages/desktop test`）归桌面端 **test-execution**。两者均不写入任何阶段自查项，也不作为本相位的完成判据（约束 ⑥）。**交付物 = 期望登记与移交**
- [x] 交接备忘（发给 test-design / test-gen / test-execution，本相位不写用例；**交付物 = 备忘登记与移交**）：
  - `crates/core/agent/src/ask_test.rs`（新）：`register` 成功 / 同键覆盖（旧 sender 收 `RecvError`）/ `answer` 投递原文 / take 语义（二次 `answer` `Err`）/ `remove` 后 `answer` `Err` / miss 幂等
  - `runner_test.rs`：`AskMode` serde 线值（`generic` / `flow`）、`ask` 缺省读 `None`、四字段 round-trip 与 `Eq` 链、`SessionOpen.session_id` 构造面
  - `kernel_test.rs`：`new` 四参夹具；`converge` 后 `AskRegistry` 该键除名（`answer` → `Err`）；`begin_turn` 把 core session id 填进 `SessionOpen`
  - `crates/core/orchestration/src/ask_test.rs`（新）：六态校验 + 解耦守卫（`ask.rs` 全文零 `finish` 字样；`finish.rs` 的 `ask` 仅文案字符串）
  - `finish_test.rs` / `decision_test.rs` / `prompt_test.rs`：decision 形态两值 + 无两 property + description 指路；`validate_decision` 的 `"ask"` 臂仍可通过；`decision_prompt` 两动作 + 指路；`decision_answer_prompt` 全仓零命中
  - `control_test.rs`：四条 `answer` 用例重写（转投 / 无 run 携 change id / 无等待方 / 迟滞应答）；`confirm` 侧四条零改动；`resolve_ask` 三态（清槽 + 条件回 `Running` + 广播 `Step`）
  - `state_test.rs`：`AskPayload` 两字段 serde（缺省 `None`）；`RunUpdate::Ask { ask }` 投影与 `kind()` / `RunNotice` 不变
  - `walker_test.rs`：决策会话三条 ask 往返用例改「防御臂」与「ask 工具往返不在此层」；单轮收口零 `Continue`
  - `sdk/mount_test.rs`：`ask = None` 不追加 / `Some(Generic)` / `Some(Flow)` 追加且 `tier == Protocol` / 与 `finish` 位四组合独立
  - `sdk/loop_test.rs`：`Generic` 阻塞 → 应答回灌 → 同一 turn 续跑；`Flow` 先 `ask_started`（`tool_call_id` 相等）→ 应答 → `ask_resolved` 恰一次；停止臂 → `Err` 且 `ask_resolved` 仍执行；上下文缺席 → `is_error`；`Flow` 且 port 缺席 → `is_error`（防御臂）
  - `sdk/runner_test.rs`：`ask` 位 `Some` 时铸 `AskContext`（`None` 不铸）；`Flow` 无 `AskPort` 时降级 `is_error`
  - `compose_test.rs`：四臂 `ask` 位逐条断言（含 debug 臂 `Generic`）
  - `worker_test.rs`：`AskPort` 透传（change 来源拿到句柄、非 change 为 `None`）
  - `exec/mod_test.rs`：`agent_answer` 四态（blank root / blank session_id / 未登记 / 重复应答）
  - `change_flow/mod_test.rs`：两入口各一条（`change_flow_answer` 与 `agent_answer` 投同一图）；`ask` 槽由 `ask_resolved` 清理（命令内零清理）
  - `src/bindings/mod_test.rs`：反向锚定清单零改动即覆盖（`SessionInjections` 在场）；`AskPayload` 两字段出线
  - 前端：`tool-cards.test.tsx`（双态 / 双形态解析 / 空串不提交 / 提交后禁用 / 旧文案零命中）、`agent-messages.test.tsx`（三态判活 / 动作面只达 ask 卡）、`agent-session-view.test.tsx`（两形态同透传）、`use-agent-session.test.ts`（`answer` 三参逐字 / null 守卫 / `Err` 进 error 面）、`session-panel.test.tsx`（骨架合成）、`session-transcript-panel.test.tsx` + `change-detail-view.test.tsx`（流程侧 `onAnswer` → `changeFlowAnswer`、`pendingToolCallId` 取 `activeRun.ask.toolCallId`）
