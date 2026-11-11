# 设计: agent-finalize-tool-for-flow-steps

> **变更**: agent-finalize-tool-for-flow-steps
> **状态**: 设计定稿（基于已定稿 proposal + 6 份 delta spec）
> **基于**: `proposal.md`（问题 / 提案 / D1–D11 / 变更范围 / AC-1…AC-13 / R1–R10）、`explore.md`（只读结论稿）与 `specs/{desktop-agent-execution,desktop-archi-agent,desktop-change-orchestration,desktop-phase-prompts,desktop-agent-chat-infra,desktop-workflow-write-face}/spec.md`
> **性质**: 协议形态变更（文本协议 → 工具协议）+ 工具面单源化（四源并行 → 一张挂载清单）；落库面 / IPC 出线面 / store schema 零变更
>
> **本篇边界**: 三层 crate（`core/agent` 词汇面、`core/workflow` 域类型、`core/orchestration` 协议实现、`infra/agent` 引擎消费面）+ 8 份 evaluator prompt md + 前端一张工具卡 + 版本一行。**零新增 crate、零新增依赖边、零架构模型改动**（`infra/agent` 已有 `orchestration` / `agent` / `workflow`(间接) 依赖）；**零 store 改动**（`ChecklistItemRecord` / `PhaseRecord` 形状不变）；**零 IPC 出线改动**（`SessionInjections` / `FinishSpec` 不出线）。

---

## 提案与规格同步状态

proposal 的三步交付（① 统一挂载 / ② `finish_step` / ③ checklist 逐项）与 proposal 的 D1–D11 十一条裁决、6 份 delta spec 均已定稿。本设计不重复其论证，只做五件事：

1. **把 spec 已钉语义、未钉落点的三处收敛到具体形态**：`registry()` 的静态持有方式（D2，`OnceLock` 惰性单点——`ToolDefinition` 含 `String` / `serde_json::Value`，**不是 const 可构造**）、`definitions()` 的返回形态（D3，取 spec Module Contract 的 `Result`）、compose 的 `finish` 位与矩阵未命中行的**解耦**（D5）。
2. **把 proposal「变更范围」的计数口径按工作树实测纠正**并留痕（见「事实校准」）：`definitions()` 调用点实为 **16**（proposal / explore 记 17–18）、`SessionInjections` 字面构造实为 **27**（proposal 记 34）、`SessionToolset` 全仓 **73** 处 / 12 文件。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（测试由独立相位承接）。
4. **把三处「非显然正确」与两处「知情边界」写成可实现形态并留痕**：loop 内收口三纪律（D7）、ask 停等外移后的决策拓扑（D8）、`TOOL_NAMES` 与三个专用工具名常量的**名字作者单点**（D4）、`protect` / `input_path` 两处按名 match 的显式保留（D9）。
5. **补两条本设计新增的裁决**（D12 / D13）：`FinishPayload` / `FinishSpec` 的线形态与派生面；Step 0 的「纯新增」如何与「定义字面量单份」并存（迁出 + 委派 shim，语义切换落在阶段三）。

### 三步交付 ↔ 本设计阶段 ↔ tasks 阶段的对应

| proposal 步 | 内容 | 本设计落点 | tasks 阶段 |
|---|---|---|---|
| ① Step 0 | `mount.rs` + checklist 解析器，纯新增、行为零变 | D1 / D2 / D4 | 阶段一 |
| ① 接线 | `toolset` 退役 / tier 准入 / `match mount.exec` / compose 名字清单 | D3 / D5 | 阶段二 |
| ② | `finish_step` + 校验器 + loop 收口 + `finalized` 出口 + 三处重试环退役 | D6 / D7 / D8 / D10 | 阶段四 / 五 / 六 |
| ③ | checklist 逐项（ID 解析器 + ID 守卫 + 落账映射） | D11 | 解析器在阶段一，**ID 校验与映射随 ② 同批**（阶段四） |

> **③ 的产品面为何与 ② 同批**：③ 的可分离前提是 ② 的 evaluator 参数先落 `items[].item`（自由文本）再改 `items[].id`——即 `parameters()` 与 `validate_checklist()` 各改两遍。解析器已在阶段一落地（proposal §"①内部再切" 把 `checklist_parser` 划入 Step 0），`items[].id` 的 enum 从第一天就可派生，故「先文本后 ID」只有 churn 收益。③ 中真正可独立成立的只剩「8 份 md 静态清单表守卫」，该守卫属测试相位（`phase_table_test.rs`）。

---

## 起点核实（工作树实测；路径前缀 `crates/`，锚点为改动前行号）

- **core/agent 词汇面**：`runner.rs:45-58` `SessionToolset { Explore, Archi { module, search } }`（`Copy + Eq`、camelCase 线值）；`runner.rs:65-75` `SessionInjections { preamble: Option<String>, tools: Option<Vec<String>>, toolset: Option<SessionToolset>（#[serde(default)]） }`（`Default + PartialEq + Eq`，**无 `specta::Type`**——出线面零参与）；`lib.rs:17-20` 出口含 `SessionToolset`。
- **infra 工具面四源**：`sdk/tools.rs:37` `TOOL_NAMES: [&str; 7]`、`:42/:47/:51` 三个专用工具名常量、`:67` `definitions(&SessionInjections) -> Vec<ToolDefinition>`（三层 `match &injections.toolset`）、`:112` `vocabulary`、`:117/:141` 两个 archi 定义、`:162` `generic_definition`（按名 7 臂 match）；`sdk/policy.rs:4/:8/:11` 三张常量表 + `:20` `allows(mode, tool: &str) -> bool`；`sdk/loop.rs:163` `tools::definitions(&turn.injections)`（**唯一生产调用点**）、`:513` 成员校验（`dispatched.contains(&name)`）、`:519` `policy::allows(mode, &name)`、`:545-566` 执行分派三臂 `if name == EXPLORE_NOTE_TOOL … else if ARCHI_MODULE_TOOL || ARCHI_SEARCH_TOOL … else tools::execute`；`compose.rs:36` `assemble_injections`（三臂）、`:102` `archi_matrix -> Option<SessionToolset>`；`sdk/runner.rs:357` `mint_archi_context(Option<&SessionToolset>, &Path)`、`:392` explore 铸造判据 `toolset == Some(Explore)`、`:402` archi 铸造判据。
- **infra 工具执行体（零改动面）**：`tools.rs:270` `input_path`（`glob` 取 `pattern`、两 archi 工具显式 `None`）、`:281` `execute(root, name, input)`、`:301` `cap_result`；`sdk/protect/mod.rs:69` `evaluate(tool, input, cwd)`（`"write"|"edit"` 按路径、`"bash"` 按命令文本、`_ => None`）。
- **orchestration 文本协议面**：`verdict.rs:11` `EvaluatorChecklist { verdict, report, checklist }`（**仅 `Deserialize`**）、`:20/:41/:51/:58` `extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`、`:6` `MAX_REPORT_CHARS = 2000`（`infra/archi/src/runner.rs:19` 是第二个消费者 ⇒ **非退役面**）；`decision.rs:9` `DecisionAction`（`#[serde(tag = "action")]`、**仅 `Deserialize`**）、`:6` `MAX_REASON_CHARS = 500`、`:55` `parse_decision`、`:67` `ensure_backtrack_allowed`（四参，零改动）；`prompt.rs:46` `evaluator_protocol`、`:64` `verdict_fix_prompt`、`:75` `decision_prompt`（`## 决策协议` JSON 段在 `:129-133`）、`:141` `decision_fix_prompt`；`port.rs:76-82` `WorkerTurnOutcome { session_id, status, final_message, transcript }`（`Debug + Clone`）；`transcript.rs:4` `final_assistant_text`、`:32` `failure_reason`（`SystemNotice{api_error}` 取值的既有先例）。
- **walker 三套重试环**：`:1563` `step_verdict_gate`（反馈环 ≤5）、`:1818` `step_verdict_gate_terminal`（`on_fail=Terminal` 单次）、`:3416` `decision_session`（`drift ≤ DECISION_DRIFT_LIMIT`，常量 `:132`）；退役面 `:2218` `step_verdict_gate_upgraded_phase_log`（唯一调用点 `:1645`）；`feedback_limit` 三调用点 `:844`(static-check) / `:1418`(archi-check) / `:1574`(verdict-gate)；机械 `ChecklistItem` 构造 7 处 / 条目 8 个（`item` 文案行 `:928/:1494/:1999/:2005/:2013/:2113/:2187/:2253`）。
- **checklist 语料**：`core/workflow/src/prompts/*-evaluator.md` 8 份，`## 静态清单` 段表首列 ID 逐份实测 `A1..A5`/`CA1..CA8`/`C1..C7`/`D1..D10`/`I1..I7`/`R1..R11`/`T1..T9`/`G1..G8`（合计 65），且逐份「段落内 ID 集合 == 全文边界 ID 集合」（已复核，8/8 相等）；`test-design-evaluator.md` 含 `### T8 格式检查详细项` 子表（`T8-U1` / `T8-G1` 等 8 个连字符子项）——段落边界规则的反例样本在场。8 份含同一句输出面原文（`- 只产出评估结论：最终消息输出一个 checklist JSON（…）`）。
- **落账面**：`core/workflow/src/write/phase_log.rs:31` 私有 `derive_verdict`（全 pass / 空表 → Pass）；`MAX_REPORT_CHARS` 同 crate 另有 `phase_table.rs:41` 的 `pub(crate)` 同名常量（写面自持，**不受本变更影响**）。
- **前端**：`components/agent/tool-cards.tsx:279` `BUILD_IN_TOOL_CARDS`（6 键：`AskUserQuestion` / `web_search` / `bash` / `read` / `write` / `edit`）；`components/agent/agent-blocks.tsx` 仅导出 `TextBlock` / `ThinkingBlock` / `ToolPairCard` / `WebSearchCard` 四组件（**特化卡本体多在 `tool-cards.tsx` 内定义**，跨文件只回引 `WebSearchCard`）；宽容解析原语 `lib/parse.ts` 的 `asRecord` / `textOf`。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.33"`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟。

### 事实校准（proposal / explore 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **`registry()` 不能是 `const` 静态数组**：`ToolDefinition { name: String, description: String, parameters: serde_json::Value }` 三层都非 `const` 可构造 ⇒ 唯一可行形态是 `static REGISTRY: OnceLock<Vec<ToolMount>>` + `get_or_init`，签名仍是 spec 要求的 `&'static [ToolMount]`。先例：`sdk/log.rs:9` 与 `sdk/protect/shell.rs:325` 的 `OnceLock` 预编译表。
2. **`definitions()` 返回 `Result`**：proposal「变更范围」表写 `definitions(injections, prompt) -> Vec<ToolMount>`，而 desktop-agent-execution spec Module Contract 写 `-> Result<Vec<ToolMount>, String>`。取 spec（`Err` 是「非法名」这条判据的唯一载体）。
3. **`definitions()` 现行对非法名是「静默滤过」而非 `Err`**（`sdk/tools.rs:68-75` 只按 `TOOL_NAMES` 过滤；`tools_test.rs:1128` 的 `definitions非法名与重复名经封闭清单保序滤除` 正是该行为的断言）。spec「名称非法即装配错误」为准 ⇒ ① 在这**一个点**上有显式行为变更（静默滤过 → `Err`）。这不是「行为零变」的破例，而是 AC-1 的验收面本身；proposal 的「① 行为零变」读作「合法注入面下的下发面逐名不变」。
4. **`definitions()` 调用点实为 16 处**（1 生产 `sdk/loop.rs:163` + 15 测试：`tools_test` 13 / `loop_test` 1 / `runner_test` 1）；proposal 记「生产 1 + 测试 17」、explore 记 18。另 2 处是注释提及（`runner_test.rs:2293`、`tools_test.rs:1030`），不计。
5. **`SessionInjections` 字面构造实为 27 处**（`kernel_test` 1 / `core/runner_test` 5 / `compose` 3 / `compose_test` 1 / `loop_test` 6 / `sdk/runner_test` 3 / `tools_test` 9）；另有 7 处 `-> SessionInjections` 返回型声明。proposal / explore 记 34 处。
6. **读侧断言面**：`SessionInjections` 相关 `.tools` 读 42 处、`.toolset` 读 27 处（共 69；全仓 `.tools` 命中 73 处，差额是 `walker.rs` 的 `ctx.tools`（`Arc<dyn ToolStepPort>`）等同名字段，**不属本变更射程**）。`SessionToolset` 全仓 73 处 / 12 文件（`core/runner_test` 27、`compose_test` 8、`core/lib_test` 7、`tools_test` 6、`sdk/runner_test` 6、`sdk/runner` 4、`loop_test` 4、`compose` 4、`tools` 2、`phase_table_test` 2（仅注释）、`core/runner.rs` 2、`lib.rs` 1）。
7. **compose 的 `finish` 位与矩阵未命中行必须解耦**：决策会话的 `source_ref` 尾段是 `<fail_phase>/decision/<attempt>`，`(phase, "decision")` **恒不命中** 7 行矩阵；若沿用现行「矩阵 `None` ⇒ `SessionInjections::default()`」结构，`finish` 位会被一并丢掉，决策会话将拿不到 `finish_step`。装配结构改为「先算 archi 名集合（可空），再按 role 独立置 `finish` 位」。
8. **`bindings/mod_test.rs:429` 反向锚定表是字符串字面量清单**：删 `SessionToolset` 不改**不会**编译失败（proposal 记「否则编译期即红」不确），但会留下一条 `SessionToolset` 全仓零词后恒真的空判据。仍必须改为 `FinishSpec`——它是「引擎侧词汇不出线」这条判据的唯一有效锚点。
9. **`WorkerTurnOutcome.finalized` 的测试装置路径**：`walker_test.rs` 的 `FakeWorker`（`:498-556`）直接构造 `WorkerTurnOutcome`，不经 `infra/agent/src/worker.rs`。故夹具必须与 `worker.rs` 调**同一个** `orchestration::transcript::finish_payload`（从注入转录取值），否则「提取实现单点」在测试面出现第二份。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | `ToolMount` 的字段面与 `ToolExec` 臂集 | `ToolMount { definition: ToolDefinition, tier: ToolTier, exec: ToolExec }`；`ToolTier { Read, Write, Execute, Protocol }`（`Copy + Eq`）；`ToolExec { Pure, ExploreNote, Archi, FinishStep(FinishSpec) }`（`Copy + Eq`） | 沿 spec「类型面」逐字。`ToolExec` 加 `Copy + Eq` 只是**内部**不变量（`FinishSpec` 已是 `Copy + Eq`），使 `matches!(mount.exec, ToolExec::FinishStep(_))` 与测试对拍零成本；`ToolMount` 本身**不派生** `PartialEq`（内含 `serde_json::Value`，无 `Eq`）——它与 `SessionInjections` 不同 crate 面，不承担 `Eq` 链 |
| D2 | `registry()` 的静态持有方式 | `static REGISTRY: OnceLock<Vec<ToolMount>> = OnceLock::new();` + `registry() -> &'static [ToolMount]` 走 `get_or_init(build_registry)` | `ToolDefinition` 三字段全非 const 可构造（见事实校准 1）；`OnceLock` 是本仓预编译静态表的既有形态（`sdk/log.rs` / `protect/shell.rs`）。「十项静态挂载」的语义由 `build_registry()` 的**十个字面构造项**承载（不是运行期拼装） |
| D3 | `definitions()` 的签名与 `Err` 语义 | `definitions(injections: &SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>`；`tools: None` → registry 前七项（内建）；`Some(names)` → **按 registry 序**过滤（等价于原 `TOOL_NAMES` 序，故合法面下发逐名不变）；名字不在 registry → `Err`（文案携非法名与来源）；`finish: Some(spec)` → 末尾铸造并追加一项 `finish_step` | 取 spec Module Contract（事实校准 2）。`Err` 在生产不可达（compose 已前置校验），是**防御面**：`loop` 对 `Err` 走 `finish_error(subtype = "tool_mount_error")` 早退并落引擎日志——不静默降级为空工具面（那会让模型「什么都不能用」而非报错）。`tools: None` 只取前七项的关键实现：`registry()` 的前七项**必须**恰是七内建（建造顺序即契约，`mount` 单测钉住） |
| D4 | 工具名的「作者单点」 | `TOOL_NAMES`、`EXPLORE_NOTE_TOOL`、`ARCHI_MODULE_TOOL`、`ARCHI_SEARCH_TOOL` **全部迁入 `mount.rs`**（与十个挂载项的建造代码同文件）；`tools.rs` / `sdk/runner.rs` / `loop.rs` 经 `crate::sdk::mount::…` 引用。`finish_step` 的工具名常量落 `orchestration::finish::TOOL_NAME`（`infra/agent` 引它），notice subtype 复用同一常量 | AC-1「`TOOL_NAMES` 不参与任何判定」要求它只剩「定义作者处」一个身份——与建造代码同处才名副其实。`finish_step` 的名字必须在 **orchestration**（`parameters` / `description` / 提取侧都认它），而 infra 依赖 orchestration ⇒ 单一字面量放在 orchestration、infra 单向引用，避免 `"finish_step"` 在三处各写一遍 |
| D5 | compose 三臂与 `finish` 位 | `assemble_injections` 结构改为：① 判 source 臂产「archi/专用工具名集合」（explore → 六名 + `create_explore_note`；archi → 七名 + 两名；change → 七名 + 矩阵勾选名；其余 → `None` 即内建七件）；② **独立**按 `role` 置 `finish` 位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`）；③ 每个名字对 `mount::lookup` 校验（`None` 即非法名），非法名 `Err`（携名与来源）。`archi_matrix(phase, role) -> Option<Vec<&'static str>>` | 事实校准 7：决策会话恒不命中矩阵，`finish` 位若挂在「矩阵命中」分支上就会丢。把「archi 工具面」与「收口档位」拆成两个独立输入面，是 spec「`finish` 位按 `role` 置位」的唯一自洽实现。矩阵值改名字清单后 `SessionToolset` 在此文件彻底退场 |
| D6 | 校验器签名与 Err 文案口径 | `validate(spec: FinishSpec, args: &Value, prompt: &str) -> Result<FinishPayload, String>`；内部按 spec 分派到 `validate_checklist(args, prompt)` / `validate_decision(args)`。文案模板统一为「**字段名** + 期望 + 当前值 + 超出量」，例：`items[].id 含清单外 ID ["C9"]：本 prompt 静态清单 ID 集为 ["C1"…"C7"]` / `report 超长：上限 2000 字符，当前 2500 字符，超出 500` | 两个 `*_fix_prompt` 退役后工具 Err 是**唯一反馈面**（D7 的代价），文案自足性即验收面（AC-9）。手写校验（不用 `serde_json::from_value`）的理由：serde 的 `missing field` 诊断不含期望集合与当前值，达不到「自足」 |
| D7 | loop 内收口的三纪律与顺序 | 单点顺序固定为：① 同批后续调用 → 假结果 `is_error: false` + 文案「本步已 finish_step 收口，该调用未执行」，**经 `sender` 发 `Message{ToolResult}`** 并 `results.push`；② `finish_step` 校验 `Ok` → 记 `finalized`、结果文案「已收口」；③ `for` 结束 → `history.push(Message::tool_results(results))`（配对闭合**先于**收口）；④ `finalized` 在场 → 发 `SystemNotice{subtype: "finish_step"}` → `finish_success` → `return history` | 三点各自都曾是推翻过的方案：假结果若 `is_error: true`，`resume::append_tool_results` 会拼「工具执行失败: …」；`results` 若晚于 `return`，同批 `tool_use` 的结果不进恰一条 user 消息 ⇒ provider 400；假结果若只进 `results` 不进 `sender`，`resume::rebuild` 会补 `INTERRUPTED_RESULT`。MUST NOT 用 `RunHandle::request_stop()`（语义链是 `Stopped → Terminal::stopped → UI「已停止」`） |
| D8 | ask 停等外移后的决策拓扑 | `decision_session` 内部 `loop`：每轮跑一次 `run_worker` → `decision-log` 幂等挂账 → 读 `finalized`；`Ask` → `guard.emit(RunUpdate::Ask)` → `wait_answer`（`None` → `Terminal::stopped("ask 停等被终止")`）→ 以 `SessionRef::Continue` 开下一轮（`prompt = decision_answer_prompt(&answer)`）→ 继续本轮循环；非 `Ask` → 发一个 `WhitelistGate` 节点后返回。**ask 轮不发 `WhitelistGate`**（未产生动作裁决），**不计任何失败 / 漂移** | spec ADDED「决策会话经 finish_step 收口与 ask 停等外移」逐条落地。`decision-log` 置于动作裁决**之前**（stop / 驳回路径同样留痕）；「一轮」= 一次 `run_worker`，故会话数 1 / 轮数 2 的判据自然成立 |
| D9 | `protect` / `input_path` 的按名 match 保留 | `tools::input_path` 与 `protect::evaluate` **逐字不动**；`finish_step` 在两处走既有默认臂（`input_path` 取 `path` 字段 → 缺失 → `None` → 沙箱原样返回；`protect` 走 `_ => None`）——**不为 `finish_step` 加特判** | D6 裁决（控制变更范围）。零新增分支的可执行判据：`sdk/protect/**` 与 `tools::input_path` 出现 diff 即说明实现偏离 |
| D10 | `finalized` 的出口链与截断成因 | loop 发 `SystemNotice{subtype: "finish_step", payload: <FinishPayload JSON>}` → `orchestration::transcript::finish_payload(&transcript)` 反序列化（末条命中、缺失 / 损坏 → `None`）→ `WorkerTurnOutcome.finalized` → walker 直接读。截断成因由 walker 私有函数扫转录 `SystemNotice{subtype:"output_truncated"}` 区分 | spec「载荷出口」：MUST NOT 扩 `TurnOutcome`（store 轮行与 IPC 终态共用形状）、MUST NOT 扩 `TurnDone`（infra 认识协议语义即撞分层三角）。`output_truncated` 的扫描放 walker（`orchestration` 内、不新增 transcript 公共面）——它只影响文案，不影响判据 |
| D11 | ID 契约的**三分类 Err** 与退化面 | `validate_checklist` 分列三类：**缺项**（期望集 − 收到集）/ **清单外 ID**（收到集 − 期望集，即 AC-9 的「多余」）/ **未知 ID**（元素形态层：`id` 缺失、非字符串、空串）。`id → item` 映射取 `checklist_entries(prompt)` 的第二列正文。若 prompt 未解析出任何 ID（退化面）→ `Err("本 prompt 无静态清单 ID（期望非空）…")`，`parameters` 的 `enum` 字段**省略**（空数组是非法 JSON Schema 形态） | AC-9 要求三类文案「分列且自足」，但「多余」与「未知 ID」在字面上同指集合外元素；取「集合层面多出」+「元素形态非法」两分列，三类各有专属文案且互斥。退化面生产不可达（8 份 evaluator md 逐份有 ID，守卫在测试相位钉住），显式 `Err` 优于静默放过 |
| D12 | `FinishPayload` 的线形态与派生 | `enum FinishPayload { Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`，`#[serde(rename_all = "camelCase")]`（外部 tag ⇒ `{"evaluator":{…}}` / `{"decision":{…}}`），派生 `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`；`FinishSpec` 派生 `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`，**不派生 `specta::Type`** | `WorkerTurnOutcome` 是 `Debug + Clone`，载荷必须同宽；notice 的 `payload` 是 `serde_json::Value`，故 `FinishPayload` 必须双向 serde。`specta::Type` 不派生的理由与 `SessionInjections` 同款（引擎侧会话词汇零出线，`bindings/mod_test.rs` 反向锚定守着）；`FinishSpec` 的线值 `"evaluatorChecklist"` / `"decision"` 由 camelCase 自动给出（AC-3 的判据面） |
| D13 | Step 0 的「纯新增」如何与「字面量单份」并存 | 阶段一即把定义函数与工具名常量**迁入** `mount.rs`（`tools.rs` 不留副本），`tools.rs` 临时保留**一行委派 shim**（旧签名 `definitions(&SessionInjections) -> Vec<ToolDefinition>`，体内**保持旧的「未知名静默滤过」语义**：`mount::registry()` 前七项按名字清单过滤、`Some(names)` 时为 `names.iter().filter_map(按名查 registry)`）。shim 在阶段三删除，「未知名 → `Err`」的语义切换**与 shim 删除同一提交**。`ToolExec` 在阶段一只落三臂（`Pure` / `ExploreNote` / `Archi`），`FinishStep(FinishSpec)` 随阶段二补臂 | proposal 的「Step 0 纯新增、行为零变」与本设计的「定义字面量单份」在字面上冲突：若阶段一在原处保留完整副本，则同一批 `ToolDefinition` 字面量会存在两份直到阶段三，「守卫面就位」证明的是副本而非真源。取「迁出 + shim」：shim 只做名字→mount 的**转发**（不承载任何定义数据），且**逐字保留旧过滤语义**——故阶段一与既有语义行为等价（含 `tools_test` 的「非法名与重复名经封闭清单保序滤除」；该断言的回归核对由桌面端 test-execution 相位承接，不进本相位自查项）。语义切换（静默滤过 → `Err`，见事实校准 3）落在阶段三，与该断言的测试相位改造同期 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 收口档位词汇（**修改**） | `FinishSpec { EvaluatorChecklist, Decision }` 新增、`SessionToolset` 与 `toolset` 位删除、`finish: Option<FinishSpec>` 位（`#[serde(default)]`） | `packages/desktop/src-tauri/crates/core/agent/src/{runner.rs,lib.rs}` | `serde`（既有；零新依赖） | 闭合枚举 + `#[serde(default)]` additive 演进，`PartialEq + Eq` 链完好 |
| checklist ID 契约解析器（**新增**） | `ChecklistEntry { id, item }`；`checklist_entries(prompt) -> Vec<ChecklistEntry>`（段落边界 + 表首列 ID 形态 + 出现序去重，同时取第二列权威文案） | `crates/core/workflow/src/model/checklist.rs`（新） | 零依赖（纯 `&str` 扫描；不引入 `regex`——行首前缀 + 手写边界判定足够，避免给 `workflow` 引新依赖） | 纯函数、零 IO |
| verdict 推导单点（**迁位**） | `model::derive_verdict(&[ChecklistItem]) -> Verdict`；`phase_log` 改调 | `crates/core/workflow/src/{model/domain.rs,model/mod.rs,write/phase_log.rs}` | 零新增 | 布尔归约（全 pass / 空表 → Pass） |
| 工具挂载单源（**新增**） | `ToolMount` / `ToolTier` / `ToolExec` / `registry()`（十项）/ `definitions(injections, prompt)` / `lookup(name)` / 测试夹具 `injected(names)`；七内建与三专用工具的定义函数**原样搬入** | `crates/infra/agent/src/sdk/mount.rs`（新） | `agent`（`FinishSpec` / `SessionInjections`）、`orchestration`（`finish::TOOL_NAME` / `description` / `parameters`）、`rig::completion::ToolDefinition`、`serde_json`（全部既有） | `OnceLock` 惰性静态表（D2）+ 名字查表 |
| tier 准入（**修改**） | 三张常量表删除；`allows(mode, ToolTier) -> bool` 四行等价映射 | `crates/infra/agent/src/sdk/policy.rs` | `agent::AgentPermissionMode`（既有） | 纯函数 |
| 引擎执行分派与收口（**修改**） | `definitions(&turn.injections, &turn.question)`；成员校验按 mount 名单；`policy::allows(mode, mount.tier)`；`match mount.exec` 四臂；`finish_step` 收口（D7 四步序） | `crates/infra/agent/src/sdk/loop.rs` | 既有（新增 `orchestration::finish` 使用，无新依赖边） | 既有 async 泵循环（零新任务 / 零新通道） |
| per-turn 上下文铸造（**修改**） | 判据由枚举比较改名字包含：`injections.tools` 含 `EXPLORE_NOTE_TOOL` / 含任一 archi 名 | `crates/infra/agent/src/sdk/runner.rs` | 既有 | 纯判据替换 |
| 注入装配（**修改**） | 三臂产名字清单 + 独立 `finish` 位 + `mount::lookup` 成员校验（`None` 即非法名）；`archi_matrix` 返回 `Vec<&'static str>` | `crates/infra/agent/src/compose.rs` | `mount`（同 crate） | 既有纯函数 |
| 轮产出提取（**修改**） | `WorkerTurnOutcome.finalized = transcript::finish_payload(&transcript)` | `crates/infra/agent/src/worker.rs` | `orchestration::transcript`（既有依赖） | 既有提取器先例（`final_assistant_text` 同形） |
| 工具协议单点（**新增**） | `FinishPayload`；`TOOL_NAME`；`description(spec)`；`parameters(spec, prompt)`；`validate(spec, args, prompt)` | `crates/core/orchestration/src/finish.rs`（新） | `agent::FinishSpec`、`workflow::model::{checklist_entries, derive_verdict, ChecklistItem}`、`serde_json`（全部既有） | 纯函数（零 IO、零运行时状态） |
| 校验器（**修改**） | `verdict.rs::validate_checklist`（+`Serialize` for `EvaluatorChecklist`，删文本提取面）；`decision.rs::validate_decision`（+`Serialize` for `DecisionAction`） | `crates/core/orchestration/src/{verdict.rs,decision.rs}` | `workflow::model`（既有） | 手写校验（D6 的文案自足要求） |
| 载荷提取（**修改**） | `transcript::finish_payload(&[AgentEvent]) -> Option<FinishPayload>` | `crates/core/orchestration/src/transcript.rs` | 既有 | 沿 `failure_reason` 的 notice 取值先例 |
| 门与决策（**修改**） | `step_verdict_gate` / `step_verdict_gate_terminal` 合并为单一裁决（读 `finalized`）；反馈环 / `on_fail` 分派 / 升级落账面退役；`decision_session` 单轮收口 + ask 外移 + drift 环退役 | `crates/core/orchestration/src/walker.rs` | 既有 | 既有步执行体骨架 |
| prompt 组装（**修改**） | `evaluator_protocol` 改工具协议；`decision_prompt` 协议段改工具协议；两个 `*_fix_prompt` 退役；新增 `decision_answer_prompt` | `crates/core/orchestration/src/prompt.rs` | 既有 | 纯字符串组装 |
| 角色知识语料（**修改**） | 8 份 `*-evaluator.md` 同一句输出面改写 | `crates/core/workflow/src/prompts/*-evaluator.md` | 编译期 `include_str!`（装配零改动） | 静态 markdown |
| 前端收口卡（**新增组件**） | `FinishStepCard`（evaluator 形态 / decision 形态双分支 + JSON 回退） | `packages/desktop/src/components/agent/agent-blocks.tsx` | 既有 `Trace*` / `Markdown` / `lib/parse` | React + memo（沿 `ToolPairCard` / `WebSearchCard` 形态） |
| 卡注册表（**修改**） | `BUILD_IN_TOOL_CARDS['finish_step'] = FinishStepCard` | `packages/desktop/src/components/agent/tool-cards.tsx` | 同上 | 既有注册表 |

<!-- 无新增 crate、无新增模块目录以外的新依赖、无新增架构模型元素：`App.Backend.Infra.AgentRuntime` / `App.Backend.Core.Orchestration` / `App.Backend.Core.Workflow` / `App.Backend.Core.Agent` 四个既有元素的文件面内部改动（`.dev-team/architecture/models/**` 零改动）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀；前端侧写全 `packages/desktop/…`。测试文件与用例归测试相位，本清单只列**实现文件**，另设「编译闭合批」段登记必须随签名收敛的夹具点位。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `crates/core/workflow/src/model/checklist.rs` | `ChecklistEntry { id: String, item: String }` + `checklist_entries(prompt: &str) -> Vec<ChecklistEntry>`：只扫 `## 静态清单` 标题起、下一个 `## ` 标题止的段落；行首匹配 `^| <ID> |`（`[A-Z]+[0-9]+`、两侧空格约束）才收；按出现序去重；同时取表第二列为权威「检查项」文案。零 `regex` 依赖（行首前缀 + 字符类手写判定） |
| `crates/core/orchestration/src/finish.rs` | `FinishPayload { Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`（D12）；`TOOL_NAME: &str = "finish_step"`；`description(spec) -> &'static str`；`parameters(spec, prompt) -> Value`（evaluator 臂 `items.items.properties.id.enum` 由 `checklist_entries(prompt)` 派生，空集时省略 `enum`）；`validate(spec, args, prompt) -> Result<FinishPayload, String>`（纯函数，Err 文案自足，D6 / D11） |
| `crates/infra/agent/src/sdk/mount.rs` | `ToolMount` / `ToolTier` / `ToolExec` / `TOOL_NAMES` / 三个专用工具名常量 / `registry()`（十项，D2）/ `definitions(injections, prompt)`（D3）/ `lookup(name)`；七内建定义函数（`generic_definition` + `vocabulary`）与两个 archi 定义**自 `tools.rs` 原样搬入**（字面量零改） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/runner.rs` | ① 删 `SessionToolset` 枚举（含 doc）；② 新增 `FinishSpec { EvaluatorChecklist, Decision }`（`Copy + Eq`、`#[serde(rename_all = "camelCase")]`、**不派生 `specta::Type`**）；③ `SessionInjections` 删 `toolset` 位、加 `#[serde(default)] pub finish: Option<FinishSpec>`，结构体 doc 改为「名字清单 + 收口档位」两位语义（`preamble` 消费面表述逐字保留） | AC-3；`AgentPermissionMode` / `AgentRunStatus` / `ModelLevel` / `SessionCtx` / `SessionOpen` / `TurnQuestion` / `RunHandle` / `AgentRunner` **逐字不动** |
| `crates/core/agent/src/lib.rs` | 出口名单去 `SessionToolset`、加 `FinishSpec` | AC-3 的出口面判据 |
| `crates/core/workflow/src/model/mod.rs` | 挂 `pub mod checklist;`；`pub use checklist::{checklist_entries, ChecklistEntry};`；`pub use domain::{derive_verdict, ChecklistItem, Verdict};` | 再导出面单点 |
| `crates/core/workflow/src/model/domain.rs` | 新增 `pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict`（自 `phase_log` 下沉的实现体，语义零变：全 pass / 空表 → `Pass`） | 单点（desktop-workflow-write-face ADDED 的判据：`grep` 无第二份实现） |
| `crates/core/workflow/src/write/phase_log.rs` | 删私有 `derive_verdict`，改调 `crate::model::derive_verdict`；`use` 面随之收敛 | 行为零变（`skipped` 约束、report 上限、单事务落库逐字不动） |
| `crates/core/orchestration/src/verdict.rs` | 删 `extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`；`EvaluatorChecklist` 加 `Serialize`（字段与 serde 属性零变）；新增 `validate_checklist(args: &Value, prompt: &str) -> Result<EvaluatorChecklist, String>`（形状 → ID 三分类 → `id → item` 映射 → report 上限 → `verdict` 由 `derive_verdict` 推导）；`MAX_REPORT_CHARS = 2000` **保留**（`infra/archi` 消费者在场） | AC-9；模型传入的 `verdict` 字段被忽略 |
| `crates/core/orchestration/src/decision.rs` | 删 `parse_decision`；`DecisionAction` 加 `Serialize`（tag / camelCase 零变）；新增 `validate_decision(args: &Value) -> Result<DecisionAction, String>`（`action` tag 封闭集 → 三动作必填字段 → `reason ≤ MAX_REASON_CHARS` → `options` 非空）；`ensure_backtrack_allowed` **逐字不动** | AC-8；`MAX_REASON_CHARS` 沿用 |
| `crates/core/orchestration/src/prompt.rs` | `evaluator_protocol` 改「调用 `finish_step` 收口」（保留禁调 MCP 写通道红线 + `report ≤ 2000` + 逐项 pass / evidence；删 JSON 形状与 `verdict / phase / attempt / skipped` 措辞）；`decision_prompt` 的 `## 决策协议` 段改工具协议（删 JSON 块）；删 `verdict_fix_prompt` / `decision_fix_prompt`；新增 `decision_answer_prompt(answer: &str) -> String`（ask 应答唯一再提示面） | AC-12；`backtrack_option_*` 三个私有格式化助手与 `spec_sync_prompt` / `merge_conflict_prompt` 逐字不动 |
| `crates/core/orchestration/src/port.rs` | `WorkerTurnOutcome` 增 `pub finalized: Option<FinishPayload>` | AC-6；其余字段与派生（`Debug + Clone`）零变 |
| `crates/core/orchestration/src/transcript.rs` | 新增 `pub fn finish_payload(transcript: &[agent::AgentEvent]) -> Option<FinishPayload>`（倒序找末条 `SystemNotice{subtype: finish::TOOL_NAME}`，`serde_json::from_value` 失败 → `None`） | AC-6；`final_assistant_text` / `failure_reason` 逐字不动 |
| `crates/core/orchestration/src/walker.rs` | ① `step_verdict_gate` 与 `step_verdict_gate_terminal` **合并为单一裁决**（签名去掉 `step` 参数）：`eval_outcome` 缺席 → 计划图漂移 `Terminal::failed`；`finalized = Some(Evaluator(..))` → passed 节点（detail 含 `verdict=…`）+ 填 `cursor.verdict` + `Proceed`；`finalized = None` → failed 节点 + `Terminal::failed`，文案按成因分列（转录含 `output_truncated` → 「输出被截断」；否则「未调用 `finish_step` 收口」）；② 反馈环 / `OnFail` 分派 / `feedback_limit(step, "verdict-gate")` 调用点删除（`feedback_limit` 本体逐字保留，服务 static-check / archi-check）；③ **`step_verdict_gate_upgraded_phase_log` 整函数删除**（含 `"评估输出格式"` 条目）；④ `decision_session` 改单轮收口 + ask 停等外移 + drift 环与 `DECISION_DRIFT_LIMIT` 删除；⑤ `import` 面收敛（去 `parse_verdict` / `parse_decision` / 两个 `*_fix_prompt` / `final_assistant_text`，加 `finish::FinishPayload` / `prompt::decision_answer_prompt`）；⑥ 其余四处机械 checklist 代写点（`step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`）**逐字不动** | AC-7 / AC-8 / AC-10；`PhaseCursor` 字段面零变（`verdict` 槽继续存 `EvaluatorChecklist`）；`diagnose_brief` / `truncate_report` / `resolve_deadlock` 骨架零变 |
| `crates/core/orchestration/src/lib.rs` | 挂 `pub mod finish;`；`pub use finish::{description, parameters, validate, FinishPayload};` | 出口面（spec Module Contract） |
| `crates/infra/agent/src/sdk/mod.rs` | 挂 `pub(crate) mod mount;` 与 `#[cfg(test)] mod mount_test;` | 模块声明 |
| `crates/infra/agent/src/sdk/tools.rs` | 迁出 `TOOL_NAMES` / 三个专用工具名常量 / `definitions` 的定义承载体 / `generic_definition` / `vocabulary` / 两个 archi 定义 / `create_explore_note` 定义字面量（进 `mount.rs`，字面量零改、**原文删除不留副本**）；保留 `execute` / `input_path` / `cap_result` / 七个执行体 / 五个上限常量（**逐字不动**）；顶部补 `use crate::sdk::mount::{…};`。**阶段一**：`definitions` 短暂保留为委派 shim（旧签名 + **旧「未知名静默滤过」语义**，见 D13），使 16 个调用点在阶段一零改动；**阶段三**删 shim，语义同步切为 `Err` | AC-1；`input_path` 的 `glob` 特判与两 archi 工具显式 `None` 保留（D9） |
| `crates/infra/agent/src/sdk/policy.rs` | 删三张常量表与 `listed`；改 `pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool`（`Protocol \| Read → true`；`Write \| Execute → mode != Default`） | AC-2；与三张表逐格等价 |
| `crates/infra/agent/src/sdk/loop.rs` | ① `tools::definitions(&turn.injections)` → `mount::definitions(&turn.injections, &turn.question)`（`Err` → 引擎日志 + `finish_error("tool_mount_error")` + 早退）；② 请求 `tools` 由 mount 的 `definition.clone()` 派生；`RunStarted.tools` 与成员校验自同一 mount 名单派生；③ `policy::allows(mode, mount.tier)`；④ 执行分派改 `match &mount.exec` 四臂（`Pure` / `ExploreNote` / `Archi` / `FinishStep(spec)`）；⑤ `finish_step` 收口四步序（D7） | AC-5；`sandbox_rewrite` / `deny` / `finish_success` / `finish_error` / `defend` / 截断与异常终局分支**逐字不动** |
| `crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context(names: &[String], workspace_root: &Path) -> Option<ArchiContext>`（入参由 `Option<&SessionToolset>` 改名字切片）；explore 铸造判据改 `injections.tools` 含 `EXPLORE_NOTE_TOOL`；archi 铸造判据改含任一 archi 名；`use` 面去 `SessionToolset` | AC-1 / AC-4 的 per-turn 判据；`import` 常量化引用改指 `mount` |
| `crates/infra/agent/src/compose.rs` | `assemble_injections` 三臂改产名字清单 + 独立 `finish` 位（D5）；`archi_matrix(phase, role) -> Option<Vec<&'static str>>`；成员校验改 `mount::lookup`（`None` 即非法名）；三臂 doc 同步 | AC-1 / AC-4；`change_phase_role` / `compose_turn` / `resolve_agent_engine` / `map_protocol` 逐字不动 |
| `crates/infra/agent/src/worker.rs` | `WorkerTurnOutcome { …, finalized: orchestration::transcript::finish_payload(&transcript) }` | AC-6 |
| `crates/core/workflow/src/prompts/*-evaluator.md`（8 份） | 同一句改写：`- 只产出评估结论：最终消息输出一个 checklist JSON（形状与落账红线由桌面端在会话 prompt 尾部追加的输出协议给出）。` → `- 只产出评估结论：调用 finish_step 工具收口（形状与落账红线由工具参数与桌面端追加的输出协议给出）。` | AC-12；8 份**逐字一致**，其余正文零改动；`## 静态清单` 表结构逐字保持 |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | 新增 `FinishStepCard`（evaluator 形态 = 清单结论 + `items` 逐项 + 可折叠 `report`；decision 形态 = `action` 摘要行；非预期 `input` 回退 JSON 原样）：卡根 `data-testid="block-tool-use"` + `data-tool-id`、摘要区 `data-testid="block-finish-step-summary"`、结果正文 `data-testid="block-tool-result"` | AC-13；`TextBlock` / `ThinkingBlock` / `ToolPairCard` / `WebSearchCard` 逐字不动 |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `BUILD_IN_TOOL_CARDS` 增 `finish_step: FinishStepCard`（从 `./agent-blocks` 引入） | AC-13；其余 6 键与 miss 回退分支零改动 |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.33`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号） | 版本纪律；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 编译闭合批（随字段面收敛的夹具点位；MUST NOT 新增用例 / 改断言口径）

> 这 **27 处**不是「测试编写」，而是「让工作区随 `SessionInjections` 字段面变化编译通过」的机械适配（漏改即 `E0063`）。签名 / 字段一变工作区即不可编译，故与实现文件同批落地。另附 **1 行壳层锚定字面量**——它不被编译器强制，但属 AC-3 的判据面（design 事实校准 8）。

| 面 | 文件（括号内为字面构造实测数） | 改动 |
|----|----|------|
| core/agent | `kernel_test.rs`(1) / `runner_test.rs`(5) | `toolset: …` → `finish: None`（或 `finish: Some(..)` 的正向夹具） |
| infra/agent 生产 | `compose.rs`(3) | 随三臂结构重写（D5），非纯机械 |
| infra/agent 测试 | `compose_test.rs`(1) / `sdk/loop_test.rs`(6) / `sdk/runner_test.rs`(3) / `sdk/tools_test.rs`(9) | 字段换位；新增 `mount::injected(&[&str])` 夹具压平构造面（D3） |
| `WorkerTurnOutcome` 字面构造 | `walker_test.rs`(3) / `port_test.rs`(1) / `worker.rs`(1，生产) | 补 `finalized:` 字段；**测试侧只补字段值**（`FakeWorker` 的取值语义改造——由「文本 JSON」改为「注 `SystemNotice{finish_step}` 转录取值」——归测试相位，见「事实校准 9」） |
| 壳层反向锚定字面量 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`(1 行，`:429`) | `["SessionInjections", "SessionToolset", "TurnQuestion"]` → `["SessionInjections", "FinishSpec", "TurnQuestion"]`（`:412` / `:421` 两处注释同句随动）。**必须在本相位做**：AC-3 的判据是 `SessionToolset` **全仓**零命中，该字面量留着即判据不成立；而它不被编译器强制（fact 校准 8），漏改不会红。**不新增断言、不改断言语义**（「引擎侧词汇不出线」的意图逐字保持，只是锚点换成现役词汇） |
| **合计** | **27 处 `SessionInjections` 字面构造** + **5 处 `WorkerTurnOutcome` 字面构造**（1 生产 `worker.rs` + 4 测试：`walker_test.rs` 3 / `port_test.rs` 1）+ **1 行壳层锚定字面量** + **7 处 `-> SessionInjections` 返回型声明**（`compose_test` 1 / `loop_test` 3 / `sdk/runner_test` 2 / `tools_test` 1，返回型不变） | — |

`definitions()` 的 **16** 个调用点（1 生产 + 15 测试）各补 `prompt` 实参（生产传 `&turn.question`；测试传 `""` 或被测 prompt 字面量）。`SessionToolset` 全仓 **73** 处 / 12 文件随之退场（`core/runner_test` 27 / `compose_test` 8 / `core/lib_test` 7 / `tools_test` 6 / `sdk/runner_test` 6 / `sdk/runner` 4 / `loop_test` 4 / `compose` 4 / `tools` 2 / `phase_table_test` 2（仅注释）/ `core/runner.rs` 2 / `lib.rs` 1）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `registry` | `crates/infra/agent/src/sdk/mount.rs` | 新增 | `pub(crate) fn registry() -> &'static [ToolMount]` | 十项静态挂载清单（唯一工具事实源）；前七项**必须**恰是七内建（`tools: None` 的语义载体，D2 / D3） |
| `definitions` | `crates/infra/agent/src/sdk/mount.rs` | 新增（自 `tools.rs` 迁入并扩参） | `pub(crate) fn definitions(injections: &agent::SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>` | 名字查表 + `finish` 位铸造；非法名 `Err`（D3） |
| `lookup` | `crates/infra/agent/src/sdk/mount.rs` | 新增 | `pub(crate) fn lookup(name: &str) -> Option<rig::completion::ToolDefinition>` | 按名取定义（克隆出口）；compose 装配层成员校验（`None` 即非法名 → `Err`）与 `definitions` 取表的共同单点 |
| `injected` | `crates/infra/agent/src/sdk/mount.rs` | 新增（测试夹具） | `#[cfg(test)] pub(crate) fn injected(names: &[&str]) -> agent::SessionInjections` | 压平 27 处字面构造面（R6 的缓解面）；`finish` 另设 `injected_finish(names, spec)` 同族 |
| `allows` | `crates/infra/agent/src/sdk/policy.rs` | 修改（签名换档位轴） | `pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool` | tier 准入单点（AC-2） |
| `description` | `crates/core/orchestration/src/finish.rs` | 新增 | `pub fn description(spec: FinishSpec) -> &'static str` | 两形态静态描述（<500 字，无 prompt 膨胀） |
| `parameters` | `crates/core/orchestration/src/finish.rs` | 新增 | `pub fn parameters(spec: FinishSpec, prompt: &str) -> serde_json::Value` | 动态 schema；evaluator 臂 `items.items.properties.id.enum` 由 `checklist_entries(prompt)` 派生（AC-4） |
| `validate` | `crates/core/orchestration/src/finish.rs` | 新增 | `pub fn validate(spec: FinishSpec, args: &serde_json::Value, prompt: &str) -> Result<FinishPayload, String>` | 纯函数校验入口（D6） |
| `validate_checklist` | `crates/core/orchestration/src/verdict.rs` | 新增（取代 `parse_verdict`） | `pub fn validate_checklist(args: &serde_json::Value, prompt: &str) -> Result<EvaluatorChecklist, String>` | ID 三分类 + `id → item` 映射 + report 上限 + `derive_verdict` 推导（AC-9 / D11） |
| `validate_decision` | `crates/core/orchestration/src/decision.rs` | 新增（取代 `parse_decision`） | `pub fn validate_decision(args: &serde_json::Value) -> Result<DecisionAction, String>` | tag 封闭集 + 三动作必填字段 + reason 上限 + options 非空（AC-8） |
| `finish_payload` | `crates/core/orchestration/src/transcript.rs` | 新增 | `pub fn finish_payload(transcript: &[agent::AgentEvent]) -> Option<FinishPayload>` | 末条 `SystemNotice{finish_step}` 提取；缺失 / 损坏 → `None`（AC-6 / D10） |
| `decision_answer_prompt` | `crates/core/orchestration/src/prompt.rs` | 新增 | `pub fn decision_answer_prompt(answer: &str) -> String` | ask 应答的唯一再提示面（AC-8） |
| `checklist_entries` | `crates/core/workflow/src/model/checklist.rs` | 新增 | `pub fn checklist_entries(prompt: &str) -> Vec<ChecklistEntry>` | 三规则解析（段落边界 / 表首列 ID 形态 / 出现序去重），同时取第二列权威文案 |
| `derive_verdict` | `crates/core/workflow/src/model/domain.rs` | 新增（自 `phase_log` 下沉） | `pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict` | 全域单点（校验器与落账共用，MUST NOT 两份实现） |
| `FinishStepCard`（React） | `packages/desktop/src/components/agent/agent-blocks.tsx` | 新增 | `({ toolCallId: string, toolName: string, input: unknown, output: AgentToolOutput \| null }) => React.JSX.Element` | 双形态摘要 + JSON 回退（AC-13） |

<!-- 无新增 CLI 子命令 / 无新增 HTTP 端点 / 无新增 IPC 命令：本变更零命令面演进。 -->

**退役的公共面（含理由）**：`SessionToolset`（枚举，工具面单源化后无第三源）／`tools::definitions`（签名与返回型换血为 `mount::definitions`）／`verdict::{extract_json, fenced_block, brace_slice, parse_verdict}`（唯一消费者是退役的文本协议路径）／`decision::parse_decision`（同上）／`prompt::{verdict_fix_prompt, decision_fix_prompt}`（跨轮修复面随反馈环 / drift 环一并退役，`pub(crate)`）／`walker::step_verdict_gate_terminal` 与 `walker::step_verdict_gate_upgraded_phase_log`（合并 / 零调用者死亡面）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `FinishSpec`（enum） | `crates/core/agent/src/runner.rs` | 新增 | `{ EvaluatorChecklist, Decision }`；`#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` + `#[serde(rename_all = "camelCase")]` ⇒ 线值 `"evaluatorChecklist"` / `"decision"`；**不派生 `specta::Type`**（D12） |
| `SessionToolset`（enum） | `crates/core/agent/src/runner.rs` | 删除 | 枚举整体退役（AC-3 的 `grep` 零命中判据） |
| `SessionInjections`（struct） | `crates/core/agent/src/runner.rs` | 修改 | 形态 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>（#[serde(default)]） }`；`Default + PartialEq + Eq` 完好（`FinishSpec` 是 `Copy + Eq` 闭合枚举）；无 `specta::Type` |
| `ToolMount`（struct） | `crates/infra/agent/src/sdk/mount.rs` | 新增 | `{ definition: rig::completion::ToolDefinition, tier: ToolTier, exec: ToolExec }`；`Debug + Clone`（**不含 `Eq`**——内含 `serde_json::Value`） |
| `ToolTier`（enum） | `crates/infra/agent/src/sdk/mount.rs` | 新增 | `{ Read, Write, Execute, Protocol }`（`Protocol` = 任何档位放行）；`Copy + Eq` |
| `ToolExec`（enum） | `crates/infra/agent/src/sdk/mount.rs` | 新增 | `{ Pure, ExploreNote, Archi, FinishStep(FinishSpec) }`；`Copy + Eq`；loop 分派单点 |
| `ChecklistEntry`（struct） | `crates/core/workflow/src/model/checklist.rs` | 新增 | `{ id: String, item: String }`（`id` 为静态清单表首列，`item` 为第二列权威正文） |
| `FinishPayload`（enum） | `crates/core/orchestration/src/finish.rs` | 新增 | `{ Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`；`Debug + Clone + PartialEq + Eq + Serialize + Deserialize`，camelCase 外部 tag（D12） |
| `EvaluatorChecklist`（struct） | `crates/core/orchestration/src/verdict.rs` | 修改 | 加 `Serialize`（`Deserialize` 保留以支撑载荷往返）；字段 `verdict` / `report` / `checklist` 与 serde 属性**零变** |
| `DecisionAction`（enum） | `crates/core/orchestration/src/decision.rs` | 修改 | 加 `Serialize`；`#[serde(tag = "action", rename_all = "camelCase")]` 与三动作字段面**零变** |
| `WorkerTurnOutcome`（struct） | `crates/core/orchestration/src/port.rs` | 修改 | 增 `pub finalized: Option<FinishPayload>`；`Debug + Clone` 保持 |
| `AgentToolCardProps` / `BUILD_IN_TOOL_CARDS`（TS） | `packages/desktop/src/components/agent/tool-cards.tsx` | 修改（注册表键面） | 增 `finish_step` 键；`AgentToolCard` 类型零变 |

<!-- 无新增 interface / type alias：`FinishSpec` 与 `FinishPayload` 为唯一两个新词汇，其余复用既有 `EvaluatorChecklist` / `DecisionAction` / `ChecklistItem`。 -->

### 配置

**无。** 本变更零 `KNOWN_KEYS` 增删、零 `TestFramework` 变体、零 `openspec/config.json` 改动、零 `crates/core/config/config.schema.json` 镜像改动。`packages/desktop/package.json` 的 `version` 是版本纪律项，已登记在「修改文件」表。

---

## 数据模型

### 收口载荷线形态（`SystemNotice{finish_step}.payload`）

evaluator 形态：

```jsonc
{ "evaluator": {
    "verdict": "fail",                    // 恒由 derive_verdict(checklist) 推导，模型不可提交
    "report": "评估报告（≤2000 字符）",
    "checklist": [                         // item 为 md 静态清单「检查项」列正文
      { "item": "变更代码中无安全漏洞", "pass": false, "evidence": "src/x.rs:42 硬编码密钥" }
] } }
```

decision 形态：

```jsonc
{ "decision": { "action": "backtrack", "to": "test-gen", "reason": "测试覆盖不足" } }
// 或 { "decision": { "action": "stop", "reason": "…" } }
// 或 { "decision": { "action": "ask", "question": "…", "options": ["…"] } }
```

- **键面稳定**：`EvaluatorChecklist` / `DecisionAction` 的 serde 属性逐字未改（无 `rename_all`），故既有字段名（`verdict` / `report` / `checklist` / `action` / `to` / `reason` / `question` / `options`）全部保持；本变更只**新增外层 tag**（`evaluator` / `decision`）。
- **入口形状 ≠ 载荷形状**：工具入参是 `{ report, items: [{ id, pass, evidence }] }`，载荷是 `{ verdict, report, checklist: [{ item, pass, evidence }] }`——`id → item` 的映射在**工具边界**（`validate_checklist`）完成，落账面因此零变更。
- **落账形状零变更**：`PhaseLogInput.checklist: Vec<ChecklistItem>` 与 store 的 `ChecklistItemRecord { item, pass, evidence }` 三字段逐字不变；前端 `desktop-change-queries` 的 checklist 线面（`item` / `pass` / `evidence`）零变更。

### 关系与持久化

| 项 | 现状 | 本变更 |
|----|------|--------|
| store 模型 | `PhaseRecord` + `ChecklistItemRecord`（`item` 人读正文） | **零改动**（`crates/infra/store/**` 零 diff） |
| 文案来源 | 模型自拟 `item` | md 静态清单第二列权威正文（`checklist_entries` 返回的 `item`） |
| `verdict` 来源 | 模型自报 + 落账侧重新推导（两处零校验） | `derive_verdict(checklist)` 单点推导（模型不可提交，提交即忽略） |
| 会话槽位 | `executor_session_id` / `evaluator_session_id` / `decision_session_id` | **零改动**（walker 盖戳语义不变） |
| 转录 | 每条密封事件落 `SessionEventRecord`（JSON 原文） | **零改动**；`finish_step` 的 notice 与工具调用/结果按既有管线落库（`SystemNotice` 是既有变体） |

<!-- 无新表、无新列、无外键、无迁移：载荷只经既有 `SystemNotice.payload`（`serde_json::Value`）与既有 `ChecklistItemRecord` 三字段表达。 -->

<!-- 无路由 / API 设计：本变更零 method / path / IPC 命令演进；`SessionInjections` / `FinishSpec` / `FinishPayload` 均不出线 IPC（`bindings:export` 再生成后零 diff）。`finish_step` 在 IPC 面只是一个普通工具调用（`tool-finish_step` 部件），故此处整段省略。 -->

---

## 依赖

- **运行时依赖**：**零新增**。`mount.rs` 只用既有的 `agent` / `orchestration` / `rig` / `serde_json`；`finish.rs` 只用既有的 `agent` / `workflow` / `serde` / `serde_json`；`checklist.rs` 零依赖（**不引 `regex`**——手写行首前缀与字符类判定，避免给 `workflow` 增依赖）。三个 `Cargo.toml`（`crates/core/{agent,workflow,orchestration}` 与 `crates/infra/agent`）**零 diff**。
- **依赖方向零变化**：`infra/agent → orchestration`（既有，`worker.rs` 已引 `orchestration::transcript`）／`infra/agent → agent`（既有）／`orchestration → workflow`（既有，`decision.rs` 已引 `workflow::model`）／`core/agent` 仍零 workspace 内依赖（`FinishSpec` 不引任何外部类型）。
- **构建 / 测试依赖**：**零新增**。测试侧沿用既有 `rig` 的 `test-utils`（`loop_test` 的流缝脚本化）与 `tempfile`；`mount_test` 为纯函数 + 静态清单断言，零进程边界依赖。
- **无 feature 开关变化**：`rig` facade 锁版 `=0.43.0` 与 features 面逐字不动。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 工具面单源 | `mount.rs::registry()`（十项，前七项 = 七内建）+ `definitions()`（名字查表 / `tools: None` = 七件 / 非法名 `Err`）+ `lookup()`；compose 三臂产名字清单（D5）；`TOOL_NAMES` 迁入 `mount.rs` 且只被 `build_registry` 与测试引用（D4，「不参与任何判定」的可机械判据：`policy.rs` / `loop.rs` 零 `TOOL_NAMES` 命中） |
| AC-2 tier 准入等价 | `policy::allows(mode, tier)` 四行：Default 只放 `Read` / `Protocol`；AcceptEdits / BypassPermissions 全放；`create_explore_note` 归 `Write`、两 archi 工具归 `Read`、`bash` 归 `Execute`（`registry()` 的十项 tier 声明） |
| AC-3 core 词汇面形态 | `runner.rs` 的 `SessionToolset` 删除 / `FinishSpec` 新增 / `SessionInjections` 三位形态 + `#[serde(default)]`；`lib.rs` 出口换血（AC 的 `grep` 零命中判据覆盖全仓 73 处） |
| AC-4 挂载与动态 schema | `mount::definitions` 按 `injections.finish` 铸造（仅 evaluator / decision 步）；`finish::parameters(spec, prompt)` 的 `items.items.properties.id.enum` 由 `checklist_entries(&turn.question)` 派生；`tier == Protocol`；`RunStarted.tools` 自同一 mount 名单枚举（`loop.rs` ② ） |
| AC-5 loop 内收口三纪律 | `loop.rs` ⑤ 的四步序（D7）：假结果 `is_error: false` + 真经 `sender` 发出；`results` 收齐先于 `return`；`Ok` → notice + `TurnDone{is_error:false}` + 零额外请求；`Err` → `is_error: true` ToolResult 回灌且继续下一轮 |
| AC-6 `finalized` 出口 | `transcript::finish_payload` + `port.rs` 字段 + `worker.rs` 填充（D10）；walker 零文本解析（`parse_verdict` / `parse_decision` 退役） |
| AC-7 verdict 门 | `walker::step_verdict_gate` 单一裁决：`Some(Evaluator(..))` → `cursor.verdict` + passed 节点（`verdict=…`）+ `Proceed`；`None` → failed 节点 + `Terminal::failed`（截断 / 未收口两文案，D10）；反馈边零产出、`verdict_fix_prompt` 退役 |
| AC-8 决策协议 | `walker::decision_session` 单轮收口 + ask 外移（D8）+ drift 环与 `DECISION_DRIFT_LIMIT` 删除；`validate_decision`（D6）；`decision_answer_prompt`；越权一次即 `Terminal::failed`（`ensure_backtrack_allowed` 逐字未改） |
| AC-9 checklist 逐项校验 | `validate_checklist`（ID 三分类 + `id → item` 映射 + report 上限 + `derive_verdict` 推导，D11）；`workflow::model::derive_verdict` 单点（`phase_log.rs` 零私有副本）；落账 `ChecklistItem` 三字段形状不变 |
| AC-10 机械豁免与死面退役 | `validate` 的唯一调用点是 `loop.rs` 的 `ToolExec::FinishStep` 臂（机械 checklist 的四处构造点零 diff）；`step_verdict_gate_upgraded_phase_log` 与 `"评估输出格式"` 整函数删除（D10 的 `grep` 零命中判据）；`feedback_limit` 三调用点去一留二 |
| AC-11 解析器双层守卫 | `checklist.rs` 三规则（段落边界 / 表首列 ID 形态 / 出现序去重）；`test-design-evaluator.md` 的 `T8-U1` / `T8-G1` 子项零命中；八份项数与集合相等的断言归测试相位（`phase_table_test.rs`，语料经相位表取得，零文件 IO） |
| AC-12 prompt 文本面 | `prompt.rs` 三处改写（`evaluator_protocol` / `decision_prompt` 协议段 / 新增 `decision_answer_prompt`）+ 8 份 md 同一句（逐字一致）；6 份 planner / generator 零 diff |
| AC-13 前端工具卡 | `agent-blocks.tsx::FinishStepCard` 双形态 + JSON 回退 + 三处钩子；`tool-cards.tsx` 注册表增键；`agent-messages.tsx` / `agent-adapter.ts` / `agent-transport.ts` 零 diff（miss 回退 `ToolPairCard` 分支不动） |

### 交付门（非本变更能力判据）

全仓既有门的**登记面**，本设计与 tasks 均不派生新判据。分工按 tasks 约束 ⑤：源码级静态面（`cargo clippy --all-targets`、前端 `vp check` / `knip`、`grep` 零命中、结构复核、`git diff` 边界比对、版本行相对 `HEAD` +1 patch）是**本相位可自判**的判据，可出现在 tasks 的自查项里；`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**（`cargo nextest run --workspace` / `pnpm -C packages/desktop test`）**由桌面端 static-check / test-execution 相位承接**，在 tasks 中只以「交接 …」条目登记，**MUST NOT** 作为任何阶段自查项的完成条件。

---

## 待决问题

1. **`ToolTier::Protocol` 与 `ToolExec` 的组合面是否会被后续工具复用**（本期恒为 `FinishStep` 一项）：`Protocol` 档的存在意义是「任何档位放行」；若后续出现第二类「非读写非执行」的协议工具，`ToolExec` 可能继续扩臂。本期不预设（不引入 `ToolTrait` / 回调式 `exec`），扩臂成本 = 改 `match mount.exec` 一处。
2. **`finish_step` 的 Err 重试是否有隐性轮数上限**（proposal 明确「不做工具层重试计数上限」）：本设计不引入上限，理由是 loop 本就无轮数上限（仅 `pause_turn` 有 5 次续跑预算）——「模型修不对格式」与「模型无限调 `read`」同类，均靠停止信号与活性护栏兜底。若后续实测出现「模型在 `finish_step` 上原地打转」的实例，再加按工具名的独立预算（届时不得复用 `feedback_limit` 的相位级语义）。
3. **`description` 随 step 的差异度**（proposal 未决项 2）：本期 evaluator 形态两句、decision 形态一段，均 <500 字；若后续 `FinishSpec` 增第三形态并需要 step 级差异文案，`description(spec)` 要扩为 `description(spec, phase)`——届时须评估「prompt 侧只剩一句指引」这条纪律是否仍成立。
4. **`output_truncated` subtype 字面量跨 crate 出现两次**（`infra/agent/src/sdk/loop.rs:38` 私有常量与 `walker.rs` 的成因判据）：本设计未把它收成跨 crate 常量，理由是 `SystemNotice.subtype` 是**开放词典**（`api_retry` / `context_pruned` / `pause_turn_*` 均无双写单点先例），强行收口会把开放词汇变成封闭枚举。若后续出现第三个消费者，再考虑在 `orchestration` 侧建开放词典常量表。
