# 测试设计: agent-finalize-tool-for-flow-steps

> **变更**: agent-finalize-tool-for-flow-steps
> **依据**: `proposal.md`（AC-1…AC-13；「变更范围」实现文件 26 项 = 3 新增 + 23 修改面；「测试文件」表 24 行）、`design.md`（D1–D13；「变更清单」新增 3 / 修改 23；「公共函数 / API」15 行；「类型定义」10 行；「事实校准」9 条）。
> **范围**: design「变更清单」的**新增文件 3** 与**修改文件 23**（删除文件无独立条目——被删的 `SessionToolset` / `parse_verdict` / `parse_decision` / 两个 `*_fix_prompt` / `step_verdict_gate_terminal` / `step_verdict_gate_upgraded_phase_log` 均挂在所属修改文件的被测面内，以「零命中」与「用例退役」两类判据表达）。跨模块组合用例按链路发起方挂靠，不新建集成章节。
> **路径口径**: 全部路径为**项目根相对**。design / proposal 的 Rust 简写 `crates/**` 一律展开为 `packages/desktop/src-tauri/crates/**`；前端写全 `packages/desktop/**`。

## 测试框架与既有布局

- **Rust**：`cargo nextest run --workspace`（`packages/desktop/package.json` 的 `server:test`）逐 crate 亦可 `cargo test -p <crate>`；断言库只有 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；异步用例 `#[tokio::test]`。单测布局为 **sidecar 同目录同前缀** `*_test.rs`，由父模块以 `#[cfg(test)] mod <name>_test;` 挂载（`crates/core/orchestration/src/lib.rs` 的十一条 `#[cfg(test)] mod …_test;`、`crates/core/workflow/src/write/mod.rs` 的九条、`crates/infra/agent/src/sdk/mod.rs` 的十七条为先例）。测试函数名为中文短语，断言消息即判据。
  - **新增挂载点**（编译前置，属测试相位落地）：`sdk/mod.rs` 挂 `#[cfg(test)] mod mount_test;`（design 已登记）；`model/mod.rs` 挂 `#[cfg(test)] mod checklist_test;` 与 `#[cfg(test)] mod domain_test;`（沿 `write/mod.rs` 先例——design「修改文件」表未逐条登记，属挂载声明而非被测面变更）；`orchestration/src/lib.rs` 挂 `#[cfg(test)] mod finish_test;`。
- **前端**：`vp test`（`vite-plus/test` 的 `describe` / `it` / `expect` / `vi`）+ `@testing-library/react` 的 `render` / `screen` / `within` / `fireEvent`；文件头 `// @vitest-environment jsdom`；查询以 `data-testid` 为主，纯展示断言优先 `textContent`。
  - **新增的进程边界替身（仅 `tool-cards.test.tsx`）**：`vi.hoisted` + `vi.mock('@/lib/highlighter')` 注入 `loadHighlighter` 替身——边界＝shiki 语法包的动态 `import` 与重型 grammar 装载（既有 `hooks/use-highlighted-code.test.ts` 同款），使 `useHighlightedCode` 的**语言实参**成为可断言面。`agent-blocks.test.tsx` 除既有 `@tauri-apps/plugin-opener` 替身外零新增 mock（`react-markdown` 亦不 mock：比较器判据取引用列表 DOM，见 `Mock策略`）。
  - **可观测性约束（决定用例判据形态）**：`TraceCollapsibleContent` 在折叠态**不挂载**（`@base-ui` panel 默认 `keepMounted=false`），故折叠区内容必须「先展开再断言」，或依赖 `defaultOpen`（`WriteCard` / `EditCard` 在 `output` 在场时 `defaultOpen` 为真）；`TraceCollapsible` 的 `disabled` 经 `useButton({focusableWhenDisabled: true})` 落成 `aria-disabled="true"` 与点击拦截（**不是**原生 `disabled` 属性），折叠态按钮仍可点但面板不展开；`TextBlock` 的 memo 比较器只决定「是否重渲染」，其 `useMemo` 元素复用使 `react-markdown` 不重解析——故比较器用例的判据一律取**引用列表 DOM 是否随新值更新**，不以解析次数取值。
- **突变面口径**：测试相位对 `agent-blocks.tsx` / `tool-cards.tsx` 的突变度量按**整文件**取——这两文件是本 change 的整文件编辑集，既有四卡与既有块分支与新增 `FinishStepCard` 同在被测面。故本设计除新增面外，把两文件的既有**零覆盖 / 未杀死面**逐分支纳入用例：`BashCard` / `ReadCard` / `WriteCard` / `EditCard`（零渲染）、`rangeSuffix` / `languageOf` / `cardAttrs`、`ThinkingBlock` 的 state 两态、`TextBlock` 的 `memo` 比较器与引用回退链、`WebSearchCard` 的空载荷面与 `dedupeByUrl` 的 url 缺席面、`ExternalLink` 的 scheme 锚定面、`FinishStepCard` 的结构/空位面。**无行为差异的等价突变体不追杀**，逐条登记在「## 不可测试项」。
- **既有流缝装置**：`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs` 的 `MockScript` / `FakeState` / 脚本化 transport（`rig::test_utils`）是 provider 流面的既有替身，`loop_test` 的全部新用例沿用该装置，零新增测试依赖；`walker_test.rs` 的 `FakeWorker`（注 `with_transcripts()`）是转录注入的既有入口。
- **计数纪律**：design 的 27 处 `SessionInjections` 字面构造 / 16 处 `definitions()` 调用点 / 73 处 `SessionToolset` 是**落地时刻实测值**；测试相位新增夹具会改变这些数字，故任何用例 MUST NOT 以绝对构造数作断言，只以「每个构造点都带 `finish` 位」「每个调用点都带 `prompt` 实参」为判据。
- **门禁分工**：`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与测试回归由桌面端 static-check / test-execution 相位执行；本设计只给判据，不复制运行脚本细节。文中「源码自证」类用例指在 `#[test]` 体内以 `include_str!` / `std::fs::read_to_string` 读**产品源文件**并断言结构形态（既有先例：`bash_test` 的可见性面、`bindings/mod_test` 的产物面），不引入新装置。

### 参数类型 → 边界覆盖口径

| 类型 | 覆盖要求 | 本变更加载点（按被测 API 入参 / 字段） |
|---|---|---|
| int / number | `0`、边界值、`MAX`、`None` | `report` / `evidence` / `reason` 的字符数（`0` / 恰上限 / 上限 +1 / 5000）；`MAX_REPORT_CHARS = 2000`、`MAX_REASON_CHARS = 500` 常量断言；`minItems = 1`；`turn_index` 递增（`0 → 1` 的会话继续面） |
| str / string | `""`、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None` | `checklist_entries(prompt)` 的入参（`""` / 无段落 / 100k 字符 / emoji / CRLF）；`definitions(_, prompt)` 的 prompt；`lookup(name)` 的 `""` / 大小写漂移；`report` / `evidence` / `reason` / `question` / `answer` / `to` / `action` / `id`；`SessionInjections.tools` 的元素（`""` / 超长 / 重复） |
| bool | `true` / `false` / `None` | `items[].pass`（真 / 假 / 缺失 / 非布尔 `"true"`）；`allows` 的返回值两值（`ToolTier::Read` 与 `Write` 对 Default 档）；`SessionInjections.finish` 的 `Some` / `None` |
| list / array | `[]`、单元素、超大、`None` | `items`（`[]` / 单元素 / 覆盖全部 ID / 1000 项）；`tools: Some(vec![])` 与 `None`（语义可辨）；`registry()` 的 10 项、`definitions` 的 7 / 9 / 10 项；`options`（`[]` / 单 / 1000）；`transcript`（空 / 1000 条事件）；`prompt` 的 8 份语料、`test-design-evaluator.md` 的 `T8-U1..U5` / `T8-G1` 六项子表负例 |
| dict / object | `{}`、缺失必填字段、多余字段、`None` | `args`（`{}` / 缺 `report` / 缺 `items` / 多 `verdict` 键 / `null`）；`parameters()` 产出的 schema 对象（`properties` / `required` / 省略的 `enum`）；`SessionInjections` 三字段线形（缺 `finish` 键的旧线） |
| Optional[T] | `None` | `finish: Option<FinishSpec>`、`definitions` 的 `tools: Option<Vec<String>>`、`lookup` 的 `Option<ToolDefinition>`、`finish_payload` 的 `Option<FinishPayload>`、`WorkerTurnOutcome.finalized`、`wait_answer()` 的 `None`（ask 停等被终止）、`archi_matrix` 的 `None` |
| Enum | 每值 + 非法值（N+1） | `FinishSpec`（2 + 非法 4：PascalCase / `evaluator` / `""` / `null`）；`ToolTier`（4 + 非法不可表达）；`AgentPermissionMode`（3 × 4 tier = 12 格）；`DecisionAction` 的 tag（3 + `retry` / 大小写漂移 / `unknown` / 缺失）；`FinishPayload` 的外部 tag（2 + 非法）；`SystemNotice` 的 subtype（`finish_step` / `output_truncated` / 其它） |
| float | `0.0` / `-0.0` / `NaN` / `Inf` / `None` | **契约面零 float**：`parameters()` 产出的 JSON 无浮点字段，`validate` 对浮点入参只在错误分支体现（`report: 1.5` 非字符串 → 形状 Err），不设浮点判定面。 |

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **工具面单源**：`mount::registry()` 是唯一工具事实源（十项：`read` / `grep` / `glob` / `ls` / `write` / `edit` / `bash` / `create_explore_note` / `archi_module` / `archi_search`，前七项恰为七内建）；`definitions()` 按名字查表、`tools: None` = 内建七件、专用工具以**名字**进清单；非法名在装配层 `Err`（文案携非法名）；`TOOL_NAMES` 不参与任何判定（仅定义作者处引用） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`（定义面迁出）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`（`definitions` 换参与 `RunStarted.tools`）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`（per-turn 名字判据） |
| AC-2 | **tier 准入等价**：`policy::allows(mode, tier)` 的 4 × 3 矩阵逐格（Default 只放 `Read` / `Protocol`；AcceptEdits / BypassPermissions 全放），与改造前三张常量表判定逐格相同（`create_explore_note` 归 Write、两 archi 工具归 Read、`bash` 归 Execute） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（tier 声明面） |
| AC-3 | **core 词汇面形态**：`SessionToolset` 枚举与 `SessionInjections.toolset` 位删除；`FinishSpec { EvaluatorChecklist, Decision }` 为 `Copy + Eq` 闭合枚举且 `#[serde(rename_all = "camelCase")]`（线值 `"evaluatorChecklist"` / `"decision"`）；`SessionInjections { preamble, tools, finish }` 三字段、`finish` 带 `#[serde(default)]`、`PartialEq + Eq` 齐全；`core/agent/src/lib.rs` 出口去 `SessionToolset` 加 `FinishSpec`；`SessionToolset` 全仓零命中 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs`；`packages/desktop/src-tauri/crates/core/agent/src/lib.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs`；`packages/desktop/src-tauri/src/bindings/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`（反向锚定清单 `SessionToolset` → `FinishSpec`，既有用例） |
| AC-4 | **`finish_step` 挂载与动态 schema**：仅 `finish` 位在位的步挂载（evaluator → 清单形态、decision → 三动作形态），其余步不挂；evaluator 形态 `parameters.properties.items.items.properties.id.enum` **逐值等于**该轮 prompt 的 `## 静态清单` ID 集；`tier == Protocol`；`RunStarted.tools` 含 `finish_step` 时其余名单不变 | `packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs` |
| AC-5 | **loop 内收口三纪律**：① 同批 `finish_step` 之后的调用产出假结果且 `is_error: false`、文案「本步已 finish_step 收口，该调用未执行」、**经 `sender` 发出**（转录可见、无 `INTERRUPTED_RESULT`）；② 同批全部 `tool_use` 的结果进**恰一条** user 消息；③ 校验 `Ok` → `SystemNotice{subtype:"finish_step"}` + `TurnDone{is_error: false}` + 零额外请求；校验 `Err` → `is_error: true` 回灌且会话继续 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs` |
| AC-6 | **`finalized` 出口**：`transcript::finish_payload` 从转录末条 `SystemNotice{finish_step}` 提取 `FinishPayload`（缺失 / 损坏 → `None`）；`WorkerTurnOutcome.finalized` 由 `worker.rs` 填充；walker 全链零文本解析（`parse_verdict` / `parse_decision` 全仓零命中） | `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/transcript_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`（零文本解析面） |
| AC-7 | **verdict 门**：`finalized = Some(Evaluator(..))` → `cursor.verdict` 落位、节点 detail 含 `verdict=…`、交 PhaseLog 落账；`finalized` 缺席（Completed 却未调 `finish_step`，或截断终局）→ `VerdictGate` failed 节点 + `Terminal::failed`，两成因文案可区分（截断文案取 `output_truncated` notice）；反馈边节点零产出、`verdict_fix_prompt` 退役 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs`（退役断言） |
| AC-8 | **决策协议**：决策会话单轮收口（`decision-log` 挂账一次、会话数 1）；`ask` → `RunUpdate::Ask` → `wait_answer` → 新一轮 `Continue`（会话数 1、轮数 2、第二轮 prompt 为 `decision_answer_prompt`），ask 不计失败 / 不计漂移；越权 `backtrack` → `WhitelistGate` failed + `Terminal::failed`（一次即停，文案携 `allowed`）；`DECISION_DRIFT_LIMIT` / `decision_fix_prompt` 全仓零命中；`ensure_backtrack_allowed` 逐字未改 | `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-9 | **checklist 逐项校验**：`validate_checklist` 对「缺项 / 清单外 ID / 未知 ID」三类 Err 文案分列且自足（字段名、期望 ID 集、实际收到 ID）；`item` 由 ID 映射为 md「检查项」列**逐字正文**；`report` 超 2000 报当前字符数；模型传入的 `verdict` 被忽略、`verdict` 恒由 `derive_verdict(checklist)` 推导；落账 `ChecklistItem` 三字段形状不变 | `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs`；`packages/desktop/src-tauri/crates/core/workflow/src/model/domain.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/model/domain_test.rs`；`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs` |
| AC-10 | **机械 checklist 豁免与死面退役**：① 「ID 集合相等」只作用于 `ToolExec::FinishStep(EvaluatorChecklist)`，walker 的 **7 个机械代写条目**零改动、不经 `finish_step`、不需 ID；② `step_verdict_gate_upgraded_phase_log` 与 `"评估输出格式"` 条目全仓零存在；`feedback_limit` 仍存但只服务 static-check / archi-check（`feedback_limit(step, "verdict-gate")` 零命中） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`（`validate` 唯一调用点）；`packages/desktop/src-tauri/crates/core/workflow/src/model/checklist.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/model/checklist_test.rs`（`checklist_entries` 消费面零外溢） |
| AC-11 | **解析器双层守卫**：8 份 evaluator prompt 逐文件项数 = 5 / 8 / 7 / 10 / 7 / 11 / 9 / 8（合计 65）；每份「全文边界 ID 集合 == 段落内 ID 集合」（`T8-U1` / `T8-G1` 等子项零命中） | `packages/desktop/src-tauri/crates/core/workflow/src/prompts/*-evaluator.md`（8 份）-> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`；`packages/desktop/src-tauri/crates/core/workflow/src/model/checklist.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/model/checklist_test.rs` |
| AC-12 | **prompt 文本面**：8 份 `*-evaluator.md` 的输出面同句改写为「调用 `finish_step` 收口」（8 份逐字一致、旧串零命中）；6 份 planner / generator 零改动且 `finish_step` 零字样；`evaluator_protocol` 不再要求最终消息输出 JSON 形状、保留禁调写通道红线与 `report` 上限；`decision_prompt` 协议段改工具协议 | `packages/desktop/src-tauri/crates/core/workflow/src/prompts/*-evaluator.md`（8 份）-> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` |
| AC-13 | **前端工具卡**：`BUILD_IN_TOOL_CARDS['finish_step']` 在场；evaluator 形态卡呈派生 verdict 摘要 + `items` 逐项（ID / pass 标记 / evidence）+ `report`；decision 形态卡呈 `action` 摘要（backtrack 目标 / stop / ask 问题与选项）；`input` 非预期形态回退 JSON 原样且不抛错；未注册工具的 `ToolPairCard` 回退行为零变；**整文件突变面达 80**（两文件均为本 change 的整文件编辑集：既有 `Bash` / `Read` / `Write` / `Edit` 四卡、`rangeSuffix` / `languageOf` / `cardAttrs`、`ThinkingBlock` state 两态、`TextBlock` memo 比较器与引用回退链、`WebSearchCard` 空载荷面与 `ExternalLink` scheme 面、`FinishStepCard` 结构与空位面逐分支有判据） | `packages/desktop/src/components/agent/agent-blocks.tsx` -> `packages/desktop/src/components/agent/agent-blocks.test.tsx`；`packages/desktop/src/components/agent/tool-cards.tsx` -> `packages/desktop/src/components/agent/tool-cards.test.tsx` |

> **不可测试面**（配置面 / 纯声明与再导出 / 无注入面的防御分支）逐条登记在文末「## 不可测试项」，不计入上表。

## 单元测试

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- `FinishSpec.serde()`: 收口档位闭合枚举 `{ EvaluatorChecklist, Decision }`，`Copy + Eq`，`#[serde(rename_all = "camelCase")]` ⇒ 线值 `"evaluatorChecklist"` / `"decision"`，**不派生 `specta::Type`**、不加 `Default`（类型定义面；design「类型定义」表）
- `SessionInjections.round_trip()`: 三位形态 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>（#[serde(default)]） }`，`Debug + Clone + Default + PartialEq + Eq` 链完好；`toolset` 位零残留（类型定义面；design「类型定义」表）

<!-- design.md 未声明该文件的公共函数 / API 变更：`FinishSpec` / `SessionInjections` 取自 design「类型定义」表的两行（`FinishSpec` 新增、`SessionInjections` 修改）与「修改文件」表的 `core/agent/src/runner.rs` 行，非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-3（`SessionToolset` 枚举与 `toolset` 位删除、`FinishSpec` 线值与派生面、`SessionInjections` 三字段 + `#[serde(default)]` + `Eq` 链）与 design D12（`FinishSpec` 派生清单与「不派生 `specta::Type`」）组织。旧 `SessionToolset` 的 serde / round-trip / 非法线值用例（现盘面 27 处引用）**整体退役**——被测类型已不存在，无替代面；其「线值非法即 `Err`」判据由 `FinishSpec` 的四类非法线值用例承接。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `FinishSpec` 两线值（AC-3） | 正向 | `serde_json::to_value(FinishSpec::EvaluatorChecklist)` == `"evaluatorChecklist"`、`Decision` == `"decision"`；两值 `from_value` 回转逐值相等（round-trip 等值） | 表驱动（2 值 × 双向） |
| `FinishSpec` 非法线值（AC-3 / Enum 边界 N+1） | 异常 | 四类非法输入 `"EvaluatorChecklist"`（PascalCase）/ `"evaluator"`（短形）/ `""`（空串）/ `null`（None）各 `from_value::<FinishSpec>` 返回 `Err`（大小写敏感、无别名容忍） | 表驱动（4 形态） |
| `FinishSpec` 派生面（D12） | 边界 | 编译期锚定 `Copy`：`let a = FinishSpec::Decision; let b = a;` 后 `a == b` 仍可用（`Copy` 未被移走）；`assert_eq!` 逐值相等（`PartialEq + Eq`）；`format!("{:?}", …)` 非空（`Debug`）；源码自证 `runner.rs` 内 `specta::Type` 零命中、`impl Default for FinishSpec` 零命中（不派生出线类型、不设缺省档） | 单例（3 断言 + 源码自证 2 断言） |
| `SessionInjections` 三字段 round-trip（AC-3） | 正向 | `{ preamble: Some("前导"), tools: Some(vec!["read","archi_module"]), finish: Some(Decision) }` → `to_value` 后 `from_value` 逐字段相等；键面恰 `preamble` / `tools` / `finish` 三键（`toolset` 零命中） | 单例 |
| `SessionInjections` 旧线兼容（AC-3 / `#[serde(default)]`） | 边界 | 旧线 JSON 缺 `finish` 键 → `finish == None`（`#[serde(default)]` 语义，非「必填缺失」`Err`）；`"finish": null` → 同样 `None`；缺 `tools` 键 → `None`；三键全缺 → `SessionInjections::default()` | 表驱动（4 形态） |
| `SessionInjections` 缺省与等值（AC-3） | 边界 | `SessionInjections::default()` 三字段全 `None`；`default()` 与「三字段显式 `None`」构造 `==`（`PartialEq`）；`finish: Some(EvaluatorChecklist)` 与 `Some(Decision)` 构造**不相等**（`Eq` 链可辨档位） | 表驱动（3 断言） |
| `SessionInjections` 值域边界（dict 边界） | 边界 | `tools: Some(vec![])`（空清单）与 `None`（内建七件）**不相等**、round-trip 后仍可辨；`tools: Some(vec!["".to_owned()])`（空串元素）round-trip 保真（形态校验归 `mount::definitions`）；`preamble` 超长（5000 字符）/ 含 `\n` / emoji 时 round-trip 逐字保真 | 表驱动（4 形态） |
| `SessionToolset` 退役（AC-3 零命中判据） | 异常 | 源码自证：`runner.rs` / `lib.rs` 内 `SessionToolset` 与 `toolset` 零命中；`runner_test.rs` 内零 `SessionToolset::` 引用（27 处旧断言整体删除后不留残句） | 单例（源码自证 3 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：纯 serde / 派生面断言，全部输入为内存 `serde_json::json!` 字面量与内存构造，零进程边界依赖（零文件、零网络、零 DB、零子进程） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

#### 待测功能

- `FinishSpec.reexport()`: 经 crate 根再导出，可达且可构造，两构造臂与该根再导出路径（导出面 / AC-3）
- `SessionToolset.retired()`: crate 根不可达，`lib.rs` 与 `lib_test.rs` 的导入面零命中（退役面 / AC-3）
- `crate.exports()`: 既有出口清单（`AgentPermissionMode` / `AgentRunStatus` / `ModelLevel` / `SessionInjections` / `SessionOpen` / `TurnQuestion` / `RunHandle` / `AgentRunner` …）逐名仍可导入并构造，本变更只增删一行（回归锚定；用例：出口面其余名零变）

<!-- design.md 未声明该文件的公共 API 变更：`lib.rs` 的改动是「出口名单去 `SessionToolset`、加 `FinishSpec`」，属 design「修改文件」表行（AC-3 的出口面判据），非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-3 的出口面判据组织——`FinishSpec` 经 crate 根可达且可构造，`SessionToolset` 不可达。既有 `crate根导出面锚定_引用与搜索来源类型可达且可构造` 用例的导入面换位（`SessionToolset` → `FinishSpec`），断言口径不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| crate 根 `FinishSpec` 可达（AC-3） | 正向 | `use crate::runner::{…, FinishSpec, …}` 后以 `FinishSpec::EvaluatorChecklist` / `FinishSpec::Decision` 构造并 `assert_eq!`（导入即判据：任一出口缺失即编译失败）；`serde_json::to_value` 线值仍为 `"evaluatorChecklist"` / `"decision"`（经根再导出后语义不漂） | 单例（2 断言） |
| crate 根 `SessionToolset` 不可达（AC-3） | 异常 | 源码自证：`lib.rs` 内 `SessionToolset` 零命中；`lib_test.rs` 的导入面零 `SessionToolset`（旧导入行整体删除，MUST NOT 留下被注释的引用） | 单例（源码自证 2 断言） |
| 出口面其余名零变（回归锚定） | 边界 | 既有导出清单（`AgentPermissionMode` / `AgentRunStatus` / `ModelLevel` / `SessionInjections` / `SessionOpen` / `TurnQuestion` / `RunHandle` / `AgentRunner` …）逐名仍可导入并构造（既有断言逐字保留，本变更只增删一行） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：出口面可达性由编译期与内存构造断言，零进程边界依赖 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/model/checklist.rs -> packages/desktop/src-tauri/crates/core/workflow/src/model/checklist_test.rs

#### 待测功能

- `checklist_entries()`: `checklist_entries(prompt: &str) -> Vec<ChecklistEntry>`——三规则解析（① 只扫 `## 静态清单` 标题起、下一个 `## ` 标题止的段落；② 行首匹配「两侧空格夹 `[A-Z]+[0-9]+` 形态 ID 的表首列」才收；③ 按出现序去重），同时取表第二列为权威「检查项」正文；零 `regex` 依赖（公共函数 / API）

<!-- `ChecklistEntry { id, item }` 取自 design「类型定义」表（新增行），非「公共函数 / API」表行。 -->

> 说明：用例按 AC-11 与 design D11 / proposal「解析器规范」三条规则组织，含三条实测反例（全文扫描吃到 `T8-U1` / `T8-G1` 子表；无边界 `[A-Z]+[0-9]+` 切出假 ID；不去重产生重复项）。`prompt` 一律为内存字符串 fixture（含一条仿 `test-design-evaluator.md` 结构的合成样本），不读盘、零文件 IO；真实 8 份语料的项数与集合相等守卫挂 `write/phase_table_test.rs` 章节（语料自相位表取得），本节不重复。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `checklist_entries` 正向三列解析 | 正向 | 单段表 3 行（`\| A1 \| 检查项甲 \| 判断依据甲 \|` 等）→ `len == 3`，`ids == ["A1","A2","A3"]`（出现序），`entries[0].item == "检查项甲"`（第二列 trim 逐字，不含首尾空格、不含第三列） | 单例 |
| `checklist_entries` 段落边界（规则 ①） | 边界 | 同一 prompt 中 `## 静态清单` 之前另有一段含 `\| Z9 \| … \|` 表 → `"Z9"` 零命中；`## 静态清单` 之后的 `## 附录` 段含 `\| Y1 \| … \|` → `"Y1"` 零命中；紧随静态清单段的**第一个** `## ` 标题处截断（`## 附录` 之前全部收、之后全部不收） | 表驱动（3 形态） |
| `checklist_entries` `###` 子表零命中（规则 ①，反例 1） | 异常 | 仿 `test-design-evaluator.md` 合成样本：主清单 2 项 + 段内 `### T8 格式检查详细项` 子表 3 行（`\| T8-U1 \| … \|` / `\| T8-G1 \| … \|` / `\| T8-U2 \| … \|`）→ 收集结果恰 2 项，`T8-U1` / `T8-G1` / `T8-U2` 零命中（双保险：子表在段内但行形态不匹配首列 ID；若放宽为无边界 `[A-Z]+[0-9]+` 全行扫描，`T8-U1` 会被切成 `T8` / `U1` 两个假 ID、`T8-G1` 切成 `T8` / `G1`——本合成样本的严格计数 2 vs 宽松 6（含假 ID `T8` / `U1` / `G1` / `U2`），与真实语料 `test-design-evaluator.md` 实测的严格 9 / 宽松 15 同型，见 proposal「解析器规范」反例 2） | 单例（2 类假 ID 负例） |
| `checklist_entries` ID 形态边界（规则 ②） | 边界 | 逐形态注入单行表项：`\| T1 \|` 收；`\|t1 \|`（小写）/ `\| T1a \|`（尾随字母）/ `\| 1T \|`（首字符数字）/ `\| T \|`（无数字）/ `\| T1\|`（右无空格）/ `\|\tT1 \t\|`（制表符替代空格）/ `\| T1 \|` 前置非行首字符（`> \| T1 \|`）→ 均不收；`\| AB12 \|`（多字母前缀）收 | 表驱动（7 拒 + 2 收） |
| `checklist_entries` 表头与分隔行不收 | 边界 | 表头 `\| ID \| 检查项 \| 判断依据 \|`、分隔行 `\|---\|---\|---\|`、空行、纯说明段行 → 均不收（表头首列 `ID` 非 `[A-Z]+[0-9]+` 形态） | 表驱动（4 形态） |
| `checklist_entries` 出现序去重（规则 ③，反例 3） | 异常 | 同一 ID 在段内出现两次（正文重复引用）：`\| C1 \| 正文A \| … \|` 后接 `\| C2 \| … \|` 再接 `\| C1 \| 正文A \| … \|` → `len == 2`、`ids == ["C1","C2"]`（首现位置保序）；同 ID 两次但第二列文本不同 → 保留**首现**正文（去重按 ID，不按整行） | 表驱动（2 形态） |
| `checklist_entries` 第二列权威正文 | 正向 | `\| C1 \| 变更代码中无安全漏洞 \| 检查：SQL注入、XSS… \|` → `item == "变更代码中无安全漏洞"`（第三列不参与）；第二列含中文标点 / 全角括号 / 内嵌空格时逐字保真；第二列前后多余空格被 trim | 表驱动（3 形态） |
| `checklist_entries` 空与缺席（str 边界） | 边界 | `prompt == ""` → `Vec::new()`；无 `## 静态清单` 标题的长文本 → 空；标题在场但段内零表行 → 空；`## 静态清单` 为全文最后一段且无后续 `## ` → 正常收（无上界亦不 panic） | 表驱动（4 形态） |
| `checklist_entries` 换行与特殊字符（str 边界） | 边界 | CRLF（`\r\n`）行尾 → 与 LF 形态结果**逐字段相等**（`\r` 不进 `item`）；含 `\0` 字符、emoji（`🚀`）的判断依据列 → 不 panic、`item` 不受污染；超长 prompt（>100k 字符、1000 行表）→ 不 panic、`len == 1000` | 表驱动（4 形态） |
| `checklist_entries` 幂等与确定性 | 边界 | 同一 prompt 连续两次调用逐字段相等（零内部状态、零随机）；8 份语料之外的任意输入下「段落内 ID 集 == 结果 ID 集」（无静默丢项） | 表驱动（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：纯 `&str` 扫描的纯函数，全部输入为测试体内内存字符串字面量；零文件 IO、零网络、零子进程、零环境变量；不引 `regex`（手写行首前缀 + 字符类判定），故也无正则编译期状态可替身 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/model/domain.rs -> packages/desktop/src-tauri/crates/core/workflow/src/model/domain_test.rs

#### 待测功能

- `derive_verdict()`: `derive_verdict(checklist: &[ChecklistItem]) -> Verdict`——自 `phase_log` 私有实现下沉的单点（全 pass → `Pass`、含任一 fail → `Fail`、空表 → `Pass`）；校验器与落账共用，MUST NOT 两份实现（公共函数 / API）

> 说明：用例按 AC-9 与 `desktop-workflow-write-face` ADDED「verdict 推导单点」组织——单点性由「`grep` 恰 1 处 `fn derive_verdict`」+ 行为三态锚定双向表达。`Verdict` / `ChecklistItem` 定义零变更，本节不重复其字段面用例。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `derive_verdict` 三态（AC-9） | 正向 | 全 pass（2 项）→ `Verdict::Pass`；含 1 项 `pass: false` → `Verdict::Fail`；空表 `&[]` → `Verdict::Pass`（空表即通过，与既有插件 `every` 语义一致，语义零变） | 表驱动（3 形态） |
| `derive_verdict` 单元素边界 | 边界 | 单元素 `pass: true` → `Pass`；单元素 `pass: false` → `Fail`（`all` 在单元素上的两臂） | 表驱动（2 形态） |
| `derive_verdict` 末项决定结论（list 边界） | 边界 | 99 项 pass + 末项 fail → `Fail`（不因「多数 pass」而通过）；1000 项全 pass → `Pass`（大列表不 panic、不超时） | 表驱动（2 规模） |
| `derive_verdict` 只依赖 `pass` 位 | 边界 | `item` 为空串 / 含 emoji / 含 `\n` / 超长（5000 字符）、`evidence` 为空串时结论不变（`item` / `evidence` 不参与推导——推导面对文本零依赖） | 表驱动（4 形态） |
| `derive_verdict` 单点性（源码自证） | 异常 | `packages/desktop/src-tauri/crates/core/workflow/src` 全树内 `fn derive_verdict` 恰 **1** 处且位于 `model/domain.rs`；`write/phase_log.rs` 内 `derive_verdict` 只出现在 `use` 行与调用处（零第二份实现体） | 单例（源码自证 2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：布尔归约纯函数，输入为内存 `ChecklistItem` 构造；零进程边界依赖（零文件 / 网络 / DB / 子进程 / 环境变量） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

#### 待测功能

- `phase_log()`: verdict 由 `model::derive_verdict` 推导（空表 → `Pass`、含 fail → `Fail`），落账 `ChecklistItem` 三字段形状零变（正向 / AC-9；用例：空 checklist 落账 → Pass / 全 pass / 含 fail 两态落账）
- `phase_log.guards()`: 落账既有校验与 fault 传播——report 上限 / `skipped` 约束 / 未建档 id / `StoreFault::Conflict` 逐字零变（回归锚定；用例：落账面零行为漂移）
- `derive_verdict.single_point()`: 迁位结构——`phase_log.rs` 内私有实现零命中、`model::derive_verdict` 单点调用（结构自证）

<!-- design.md 未声明该文件的公共 API 变更：`phase_log.rs` 的改动是「删私有 `derive_verdict`、改调 `model::derive_verdict`」，属 design「修改文件」表行（行为零变），非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-9 的「落账 `ChecklistItem` 三字段形状不变」与 proposal「`derive_verdict` 迁位后语义不变」组织。既有 `phase_log_test` 的断言面（`skipped` 约束 / report 上限 / 表位 / `active_phase` 匹配 / `StoreFault::Conflict` 传播 / 三会话槽位透传）**逐字零改**，本节新增的只是「按 `model::derive_verdict` 直接调用」的三态锚定行与迁位结构自证。Mock 策略沿用既有假 store——它是被测函数的显式注入依赖（入参 port），非「进程边界 mock」。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 空 checklist 落账 → Pass（AC-9 / 回归锚定） | 正向 | `PhaseLogInput { checklist: vec![], .. }` → 下发的 `PhaseLogCommand.verdict == Verdict::Pass`；既有 `derive_verdict([]) = Pass` 断言随迁位改指 `model::derive_verdict(&[])` 后仍成立 | 单例 |
| 全 pass / 含 fail 两态落账（AC-9） | 正向 | 2 项全 pass → `verdict == Pass`；含 1 项 fail → `verdict == Fail`；两条命令的 `checklist` 三字段（`item` / `pass` / `evidence`）与入参逐字段相等（形状零变） | 表驱动（2 形态） |
| 落账面零行为漂移（AC-9 / 回归锚定） | 边界 | report 超 `MAX_REPORT_CHARS` → `Err` 文案与阈值不变（零写命令下发）；`skipped` 约束（skip 时 checklist 空）不变；未建档 id → 显式 `Err`；`StoreFault::Conflict`（重复 attempt）传播不变——既有用例逐条保留，本节零改断言 | 表驱动（4 形态） |
| `derive_verdict` 迁位结构（源码自证） | 异常 | `phase_log.rs` 内 `fn derive_verdict` **零命中**（私有实现已删）；`use crate::model::derive_verdict;` 恰 1 行、调用点 `derive_verdict(&input.checklist)` 恰 1 处；`skipped` 约束 / report 上限 / `log_phase` 调用三块无 hunk（结构复核，非运行断言） | 单例（源码自证 2 断言 + 结构复核） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore`（store 落库 port） | 进程内假件实现 trait——捕获 `PhaseLogCommand` 逐字段比对 + 可编程 `StoreFault`。理由：它是 `phase_log` 的**显式注入依赖**（函数入参 port），真件是 redb 落库（进程边界）。既有装置沿用，零新增 | 全部落账用例 |
| `derive_verdict` | **不 mock**：真实调 `workflow::model::derive_verdict`（单点即被测语义，替身会引入第二事实源） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/*-evaluator.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- `phase_table.section_vs_whole()`: 段落集合 == 全文集合——`## 静态清单` 段内 ID 集合与行首 `[A-Z]+[0-9]+` 全文扫描集合相等，`T8-U1` / `T8-G1` 等子项两集合均零命中（语料面 / AC-11 第二层；用例：段落集合 == 全文集合）
- `phase_table.evaluator_corpus()`: 两表去重后恰 8 份不同 prompt，逐份非空（观察面 / AC-11 前置；用例：8 份语料取得面）
- `phase_table.output_line()`: 8 份输出面改写句 `只产出评估结论：调用 finish_step 工具收口` 逐份恰 1 处，且 8 份之间该行逐字一致（语料面 / AC-12；用例：输出面同句改写）
- `phase_table.legacy_line_absent()`: 8 份旧串 `最终消息输出一个 checklist JSON` 逐份 `contains == false`（语料面 / AC-12；用例：旧串零命中）
- `phase_table.item_count()`: 逐文件项数守卫 5 / 8 / 7 / 10 / 7 / 11 / 9 / 8 + 合计 65 逐份断言（语料面 / AC-11 第一层；用例：逐文件项数守卫）
- `phase_table.id_sequence()`: 逐文件 ID 序列 `R1..R11` / `D1..D10` / `T1..T9` / `G1..G8` / `I1..I7` / `C1..C7` / `CA1..CA8` / `A1..A5` 逐值且有序（语料面 / AC-11、D8；用例：逐文件 ID 序列）
- `phase_table.table_structure()`: 静态清单表结构——`## 静态清单` 段在场恰 1 处、三列表头（`ID` / `检查项` / `判断依据`）在场、每项第二列非空（语料面 / AC-11、D8；用例：静态清单表结构）
- `phase_table.executor_finish_step_absent()`: 6 份 `*-planner.md` / `*-generator.md` 内 `finish_step` 与 `## 静态清单` 均零字样（语料面 / AC-12；用例：6 份 executor 零字样）
- `phase_table.table_body()`: `## 静态清单` 表体 8 份逐格与改写前快照逐字相等，改写只落输出面单句（回归锚定；用例：其余正文零改动）

<!-- design.md 未声明该文件的公共 API 变更：8 份 `*-evaluator.md` 的改动是「输出面同一句改写 + `## 静态清单` 表结构保持不变」，属 design「修改文件」表行与 AC-11 / AC-12 的判据面，非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-11（双层守卫：逐文件项数 + 段落/全文 ID 集合相等）与 AC-12（8 份同句改写、6 份零字样）组织。观察面是 `phase_table()` 返回的 `PhaseAgentSpec.prompt`（`include_str!` 编译期内嵌），故**零文件 IO**：语料自 `phase_table("requirement")`（7 份 evaluator）与 `phase_table("test-only")`（含 code-analyze）的 `PhaseAgentSpec.prompt` 取得并按常量去重成 8 份。既有两处 `SessionToolset` 注释文案改写（design 明列），断言本体零改。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 8 份语料取得面（AC-11 前置） | 正向 | `phase_table("requirement")` 中 `evaluator.is_some()` 的相位恰 **7** 份（proposal / dev-design / test-design / implement / test-gen / code-review / acceptance）；`phase_table("test-only")` 的 evaluator 中 **code-analyze** 为 requirement 表缺席的那一份（`test-execution` / `archive` 两相位 `evaluator == None`）；两表去重后的 evaluator prompt **恰 8 份不同文本**（proposal / test-design / test-gen 两表共用同一 `include_str!` 常量，去重后不重复计数），逐份 `prompt` 非空——语料全部经相位表取得，用例内零 `fs::read` 调用 | 单例（4 断言） |
| 逐文件项数守卫（AC-11 第一层） | 正向 | 逐份断言 `checklist_entries(prompt).len()`：`proposal 11` / `dev-design 10` / `test-design 9` / `test-gen 8` / `implementation 7` / `code-review 7` / `code-analyze 8` / `acceptance 5`，合计 **65**（表结构被改坏即刻红） | 表驱动（8 份 + 合计） |
| 逐文件 ID 序列（AC-11 / D8：只断总数挡不住 A 少一项 B 多一项） | 正向 | 逐份断言 ID 序列**逐值且有序**：`R1..R11` / `D1..D10` / `T1..T9` / `G1..G8` / `I1..I7` / `C1..C7` / `CA1..CA8` / `A1..A5` | 表驱动（8 份） |
| 段落集合 == 全文集合（AC-11 第二层） | 边界 | 逐份：以行首 `[A-Z]+[0-9]+` 形态扫全文得到的 ID 集合，与 `## 静态清单` 段落内集合**相等**（无多无少）；`test-design-evaluator.md` 的 `T8-U1` / `T8-G1` 等连字符子项在两集合中均零命中 | 表驱动（8 份 + 子项负例） |
| 静态清单表结构（AC-11 / D8） | 边界 | 逐份：`## 静态清单` 段在场恰 1 处；三列表头 `\| ID \| 检查项 \| 判断依据 \|` 在场；每项第二列非空——表头或列数被改坏即红 | 表驱动（8 份 × 3 断言） |
| 输出面同句改写（AC-12） | 正向 | 8 份逐份：新串 `只产出评估结论：调用 finish_step 工具收口` 恰 **1** 处命中，且 8 份之间该行**逐字相同** | 表驱动（8 份 + 一致性） |
| 旧串零命中（AC-12） | 异常 | 8 份逐份：旧串 `最终消息输出一个 checklist JSON` 零命中（`contains == false`） | 表驱动（8 份） |
| 6 份 executor 零字样（AC-12） | 异常 | `*-planner.md` / `*-generator.md` 6 份语料（经两表 `executor.unwrap().prompt` 取得并去重：proposal / dev-design / test-design / implement / test-gen / code-analyze）内 `finish_step` **零字样**；`## 静态清单` 段在这 6 份内零命中（不设 ID 契约面） | 表驱动（6 份 × 2 断言） |
| 其余正文零改动（回归锚定） | 边界 | 8 份的 `## 静态清单` 表体（ID / 检查项 / 判断依据 三列逐格）与改写前的期望快照逐字相等，改写只落在输出面单句——MUST NOT 顺手改表体（整份内联快照不做，见「不可测试项」） | 表驱动（8 份） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据文件（8 份 md） | **不 mock**：语料经 `phase_table()` 的 `include_str!` 内嵌文本取得（编译期装配，运行期零文件 IO）；MUST NOT 用 `fs::read` 读 `prompts/` 目录（那会引入测试对磁盘布局的依赖，与「语料自相位表取得」的单源口径相反） | 全部用例 |
| 无其它 | **不 mock**：`checklist_entries` 真实调用（解析规则与守卫口径共用同一函数是 AC-11 / R4 的核心判据，替身会掩盖口径漂移） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/finish.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/finish_test.rs

#### 待测功能

- `description()`: `description(spec: FinishSpec) -> &'static str`——两形态静态描述（evaluator = 本步收尾 + 清单逐项 + report 上限提示；decision = 三动作封闭集 + `to` 取白名单 + reason 上限），两形态均 <500 字（公共函数 / API）
- `parameters()`: `parameters(spec: FinishSpec, prompt: &str) -> serde_json::Value`——动态 schema；evaluator 臂的 `properties.items.items.properties.id.enum` 由 `checklist_entries(prompt)` 派生（ID 集为空时省略 `enum` 键，D11 退化面）；decision 臂为三动作形状（公共函数 / API）
- `validate()`: `validate(spec: FinishSpec, args: &serde_json::Value, prompt: &str) -> Result<FinishPayload, String>`——纯函数校验入口，按 spec 分派 `validate_checklist` / `validate_decision` 并包成 `FinishPayload` 对应臂（公共函数 / API）

<!-- `FinishPayload`（`{ Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`，camelCase 外部 tag，`Debug + Clone + PartialEq + Eq + Serialize + Deserialize`）与 `TOOL_NAME = "finish_step"` 取自 design「类型定义」表与 D12 / D4，非「公共函数 / API」表行。 -->

> 说明：用例按 AC-4（动态 enum 逐值等于 prompt 的 `## 静态清单` ID 集）、AC-9（Err 文案自足）、design D6（文案模板「字段名 + 期望 + 当前值 + 超出量」）、D11（三分类 + 退化面）、D12（`FinishPayload` 线形态）与 D4（`TOOL_NAME` 单点）组织。`validate` 的具体规则面（三类 ID Err / report 上限 / 三动作字段）在 `verdict.rs` / `decision.rs` 章节细化，本节只判「分派正确的臂 + 包成正确的 `FinishPayload` + 线形态」。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TOOL_NAME` 单点（D4） | 正向 | `TOOL_NAME == "finish_step"`（`sys` 日志侧与 `SystemNotice.subtype` 同常量，源码自证 `orchestration` 与 `infra/agent` 内 `"finish_step"` 字面量只此一处 + 引用点） | 单例（2 断言） |
| `description` 两形态（AC-4） | 正向 | evaluator 形态非空、含「清单」语义与 `2000` 上限字样；decision 形态非空、含三动作名（`backtrack` / `stop` / `ask`）与「白名单」字样；两形态**不相等**（档位可辨）；两形态均 `chars().count() < 500`（无 prompt 膨胀） | 表驱动（2 形态 × 4 断言） |
| `parameters` evaluator 臂 enum 逐值对拍（AC-4） | 正向 | prompt = `code-review-evaluator.md` 语料 → `parameters(FinishSpec::EvaluatorChecklist, prompt)["properties"]["items"]["items"]["properties"]["id"]["enum"]` 是数组且**逐值有序等于** `checklist_entries(prompt)` 的 ID 序列 `["C1"…"C7"]`；同断言对 8 份语料逐份成立（8 份 ID 集各不相同，判据力来自逐份对拍） | 表驱动（8 份语料） |
| `parameters` evaluator 臂结构（AC-4 / D11） | 边界 | `properties` 含 `report`（`type: "string"`）与 `items`（`type: "array"`、`minItems == 1`）；`items.items.required == ["id","pass","evidence"]`；`pass.type == "boolean"`；`required` 顶层 == `["report","items"]`；`verdict` 键在 `properties` 内**零命中**（模型不可提交 verdict） | 单例（6 断言） |
| `parameters` evaluator 臂退化面（D11） | 异常 | `prompt == ""`（或含 `## 静态清单` 但零表行）→ `id` 的 `enum` 键**缺失**（空数组是非法 JSON Schema 形态），其余结构仍在、不 panic | 表驱动（2 形态） |
| `parameters` decision 臂形状（AC-4） | 正向 | `parameters(FinishSpec::Decision, "")` 的 `action` 取值集含 `backtrack` / `stop` / `ask`；三动作各自必填字段在 schema 内声明（`to` + `reason` / `reason` / `question` + `options`）；`prompt` 入参对 decision 臂零影响（两次不同 prompt 产出逐字段相等——decision 臂不依赖 prompt ID 集） | 表驱动（2 断言 + 对照） |
| `parameters` 幂等与确定性（D12） | 边界 | 同 `(spec, prompt)` 连续两次调用产出 `serde_json::Value` **逐字段相等**（零内部状态）；`serde_json::to_string` 键序稳定（`serde_json::Map` 保序，可逐字 diff 的工具定义） | 单例（2 断言） |
| `validate` 分派 evaluator 臂（AC-4 / AC-9） | 正向 | `validate(FinishSpec::EvaluatorChecklist, 合法 args, prompt)` → `Ok(FinishPayload::Evaluator(checklist))`，`checklist.verdict` 由 `derive_verdict` 推导、`checklist.checklist[i].item` 为 md 正文（分派命中的臂可辨：外层 tag 为 `evaluator`） | 单例 |
| `validate` 分派 decision 臂（AC-8） | 正向 | `validate(FinishSpec::Decision, {"action":"stop","reason":"…"}, "")` → `Ok(FinishPayload::Decision(DecisionAction::Stop { .. }))`（外层 tag 为 `decision`） | 单例 |
| `validate` 分派下的 Err 透传（AC-9） | 异常 | evaluator 臂非法 args（缺 `report`）→ `Err(msg)`，`msg` 由 `validate_checklist` 产出（含字段名 `report`）；decision 臂非法 args（`action: "unknown"`）→ `Err(msg)` 含封闭集三值；两臂 Err 文案互不为前缀（可归因到臂） | 表驱动（2 形态） |
| `FinishPayload` 线形态（D12） | 边界 | `serde_json::to_value(FinishPayload::Evaluator(..))` == `{"evaluator": {...}}`、`Decision` == `{"decision": {...}}`（camelCase 外部 tag）；两向 serde round-trip 逐字段相等；`from_value` 对待测非法 tag（`"finish"` / `{}` / `null`）返回 `Err`；派生面锚定 `Debug` / `Clone` / `PartialEq` / `Eq`（`assert_eq!` + `format!("{:?}")`）；源码自证零 `specta::Type` | 表驱动（2 线值 + 3 非法） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：`checklist_entries` / `validate_checklist` / `validate_decision` / `derive_verdict` 全部真实组合（分派语义即被测对象，替身会掩盖臂错配）；prompt 语料取自 `workflow::write::phase_table` 的 `include_str!` 内嵌文本（编译期，零文件 IO） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs

#### 待测功能

- `validate_checklist()`: `validate_checklist(args: &serde_json::Value, prompt: &str) -> Result<EvaluatorChecklist, String>`——形状校验（`report` 为串、`items` 为非空数组、逐项 `pass` 布尔 / `evidence` 串）→ ID 三分类（缺项 / 清单外 ID / 未知 ID）→ `id → item` 映射（md 第二列正文）→ `report` 上限 → `verdict` 由 `workflow::model::derive_verdict` 推导（模型传入的 `verdict` 完全忽略）（公共函数 / API）

<!-- `EvaluatorChecklist`（加 `Serialize`、字段与 serde 属性零变）与 `MAX_REPORT_CHARS = 2000`（保留，`infra/archi` 有第二消费者）取自 design「类型定义」表与「修改文件」表，非「公共函数 / API」表行。 -->

> 说明：用例按 AC-9（三类 Err 分列且自足、`item` 映射逐字、report 超限报当前值、`verdict` 恒由推导）与 design D11 / D6 组织。全部旧文本解析用例（`parse_verdict` / `extract_json` / `fenced_block` / `brace_slice` 的裸 JSON / 围栏 / 混杂叙述 / 结构漂移族，现盘面整节）**整体退役**——被测函数已删除，复用其代码即编译期红；其「非法输入即 `Err`」判据由本节形状类用例承接。`prompt` fixture 用 2–3 行合成静态清单（不取 8 份真实语料——真实语料的项数与集合相等守卫挂 `phase_table_test`，本节只判校验规则）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `validate_checklist` 全链路正向（AC-9） | 正向 | prompt 的静态清单 2 项（`C1` / `C2`）、args = `{ report: "…", items: [{id:"C1",pass:true,evidence:"甲"}, {id:"C2",pass:false,evidence:"乙"}] }` → `Ok`：`checklist.len() == 2`、`item` 逐项为 md 第二列正文（非 ID）、`verdict == Verdict::Fail`（含 fail 项推导）、`report` 原文保真 | 单例（4 断言） |
| 全 pass → Pass（AC-9） | 正向 | 两项全 `pass: true` → `verdict == Verdict::Pass`（推导两态可辨） | 单例 |
| 缺项 Err（D11 第一类） | 异常 | prompt 集合 `{C1,C2}`、args 只给 `C1` → `Err(msg)`：`msg` 含字段名 `items`、含缺失 ID `C2`、含期望集合与实收集合（三项自足性断言）；MUST NOT 返回部分 checklist | 单例（3 断言） |
| 清单外 ID Err（D11 第二类） | 异常 | args 给 `{C1, C2, C9}` → `Err(msg)`：`msg` 含 `C9`、含期望 ID 集、且**不含**「缺项」文案（与第一类分列、互斥） | 单例（3 断言） |
| 未知 ID 形态 Err（D11 第三类） | 异常 | 逐形态注入 `id`：缺失 / `123`（非字符串）/ `""`（空串）/ `null` → 各 `Err(msg)`，文案含「字段 `items[].id`」与期望形态（字符串、非空）；与前两类文案互不为前缀（三分类互斥，D11 判据） | 表驱动（4 形态） |
| `item` 映射为 md 正文（AC-9） | 正向 | prompt 的 `C1` 第二列为「变更代码中无安全漏洞」→ 产出的 `checklist[0].item` 逐字等于该正文（含全角括号 / 中文标点），**不等于** `"C1"`；两项映射互不串位（按 ID 对位，不按顺序猜） | 表驱动（2 断言） |
| `verdict` 恒由推导（AC-9） | 边界 | args 显式带 `"verdict": "pass"` 但含 `pass: false` 项 → 产出 `verdict == Fail`（模型字段被忽略）；args 带 `"verdict": "fail"` 但全 pass → `Pass`；args 带 `"verdict": "bogus"`（非法值）→ 仍 `Ok` 且由推导决定（模型 verdict 既不读也不校验） | 表驱动（3 形态） |
| `report` 上限（AC-9） | 异常 | `report` 长度恰 `MAX_REPORT_CHARS`（2000）→ `Ok`；2001 → `Err(msg)` 含上限数 `2000`、当前值 `2001`、超出量 `1`（三项自足性断言）；超长 5000 → 文案报 `5000` 与 `3000`；`report` 为空串 → `Ok`（无下限约束，与落账侧 `MAX_REPORT_CHARS` 校验同口径） | 表驱动（4 形态） |
| 形状异常（dict 边界） | 异常 | args 为 `{}` / 缺 `report` / 缺 `items` / `items: []`（空数组）/ `items: null` / `items` 为对象而非数组 / `report: 1.5`（非字符串）/ `report: null` / args 为 `null` 或数组 → 各 `Err(msg)` 含具体字段名与期望形态，零 panic | 表驱动（9 形态） |
| 元素级形状异常 | 异常 | `items` 单元素缺 `pass` / `pass: "true"`（字符串）/ `pass: null` / 缺 `evidence` / `evidence: 42` → 各 `Err(msg)` 定位到元素索引（`items[0].pass` 形态），不 panic | 表驱动（5 形态） |
| 多余字段与重复 ID（dict / list 边界） | 边界 | args 顶层多余键（`phase` / `attempt` / `skipped` / `verdict`）→ 静默忽略、`Ok`（沿既有「无 `deny_unknown_fields`」口径）；`items` 内同 ID 出现两次（`C1` ×2）→ 集合相等则 `Ok`，产出按 args 序保留两项但 `verdict` 按全部项推导（重复项不静默去重、不报错——同 ID 重复是模型层噪声，集合层面无缺无多） | 表驱动（2 形态） |
| 规模边界（list 边界） | 边界 | 100 项清单 + 100 项 args（全覆盖）→ `Ok`，`checklist.len() == 100`；1000 项 → 不 panic、不超时 | 表驱动（2 规模） |
| 文本特殊字符（str 边界） | 边界 | `report` / `evidence` 含 `\n` / `\0` / emoji / 超长（5000 字符的 `evidence`）→ `Ok` 且内容逐字保真（`evidence` 无长度上限）；prompt 含 CRLF / emoji 时 ID 解析与映射不受污染 | 表驱动（4 形态） |
| 退役面零残留（AC-6 / 源码自证） | 异常 | `verdict.rs` 内 `parse_verdict` / `extract_json` / `fenced_block` / `brace_slice` **零命中**；`MAX_REPORT_CHARS` 仍在场且 == `2000`（`infra/archi` 消费者在场，非退役面） | 单例（源码自证 2 断言 + 常量断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：`args` 为内存 `serde_json::json!` 字面量，`prompt` 为内存合成静态清单字符串；`checklist_entries` / `derive_verdict` 真实调用（单点语义即被测面）；零文件 / 网络 / DB / 子进程 / 环境变量 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs

#### 待测功能

- `validate_decision()`: `validate_decision(args: &serde_json::Value) -> Result<DecisionAction, String>`——`action` tag ∈ 封闭集（`backtrack` / `stop` / `ask`）→ 各动作必填字段（`backtrack`: `to` + `reason`；`stop`: `reason`；`ask`: `question` + 非空 `options`）→ `reason ≤ MAX_REASON_CHARS`（公共函数 / API）
- `ensure_backtrack_allowed()`: 白名单门（四参）——非 `backtrack` 恒 `Ok`；目标不在白名单时分「自环」与「通用越权」两文案（公共函数 / API，**逐字未改**，用例回归锚定）

<!-- `DecisionAction`（加 `Serialize`，`tag = "action"` / camelCase / 三动作字段零变）与 `MAX_REASON_CHARS = 500` 取自 design「类型定义」表与「修改文件」表，非「公共函数 / API」表行。 -->

> 说明：用例按 AC-8（`action` 封闭集 / 三动作必填字段 / `reason` 上限 / `options` 非空）、design D6（Err 文案自足）组织。旧 `parse_decision`（文本 JSON 提取）用例**整体退役**，其「非法输入即 `Err`」判据由本节 tag 与字段类用例承接。`ensure_backtrack_allowed` 的既有用例**逐字保留**（design 明列「逐字不动」），本节只登记其为回归锚定行。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `validate_decision` 三动作合法（AC-8） | 正向 | `{"action":"backtrack","to":"dev-design","reason":"…"}` → `Backtrack { to, reason }` 逐字段相等；`{"action":"stop","reason":"…"}` → `Stop`；`{"action":"ask","question":"…","options":["甲","乙"]}` → `Ask { question, options }`（三变体表驱动） | 表驱动（3 变体） |
| tag 封闭集（AC-8 / Enum 边界 N+1） | 异常 | 非法 tag：`"retry"`（已退役动作）/ `"Backtrack"`（PascalCase）/ `"unknown"` / `""` / tag 缺失 → 各 `Err(msg)`，文案含封闭集三值与实际收到的 tag（自足） | 表驱动（5 形态） |
| 必填字段缺失（dict 边界） | 异常 | `backtrack` 缺 `to` / 缺 `reason` / `stop` 缺 `reason` / `ask` 缺 `question` / 缺 `options` → 各 `Err(msg)` 含字段名与动作名；`to` 非字符串 / `reason` 非字符串 / `options` 非数组 → 同族 `Err` | 表驱动（8 形态） |
| `options` 非空（AC-8） | 边界 | `options: []`（空数组）→ `Err(msg)` 含「非空」语义与当前长度 `0`；单元素 → `Ok`；`options` 含空串元素 → `Ok`（元素内容不校验，长度下限只判数组非空）；1000 元素 → `Ok` 不 panic | 表驱动（4 形态） |
| `reason` 上限（AC-8） | 异常 | `reason` 长度恰 `MAX_REASON_CHARS`（500）→ `Ok`；501 → `Err(msg)` 含上限 `500`、当前 `501`、超出 `1`；5000 → 文案报 `5000` / 超出 `4500`；`reason == ""`（空串）→ `Ok`（无下限） | 表驱动（4 形态） |
| 文本特殊字符（str 边界） | 边界 | `question` / `reason` / `to` / `options` 元素含 `\n` / `\0` / emoji → `Ok` 且逐字保真；`question` 为空串 → `Ok`（形状层不设下限）；超长 `question`（5000 字符）→ `Ok`（无上限） | 表驱动（4 形态） |
| `DecisionAction` 线形态（AC-8） | 边界 | `to_value(Backtrack { .. })` == `{"action":"backtrack","to":…,"reason":…}`（`tag` + camelCase 零变）；三变体双向 round-trip 逐字段相等；`from_value` 对其他 tag 返回 `Err`；派生面 `Debug` / `Clone` / `PartialEq` / `Eq` 锚定 | 表驱动（3 变体 + 1 非法） |
| `ensure_backtrack_allowed` 回归锚定（AC-8） | 正向 | 非 `backtrack`（`stop` / `ask`）→ `Ok`；目标在白名单 → `Ok`；目标不在白名单 → `Err(msg)` 含 `allowed` 各相位 id；自环（`to == failed_phase` 且不在白名单）→ `Err` 文案为「零 agent 步，不支持重做」形态且与通用越权文案**不相等**——既有六条用例逐字保留，本节只作锚定登记 | 表驱动（5 形态） |
| 退役面零残留（AC-6 / 源码自证） | 异常 | `decision.rs` 内 `parse_decision` 零命中；`MAX_REASON_CHARS == 500` 仍在场；`ensure_backtrack_allowed` 的函数体无 hunk（`git diff` 结构复核，逐字不动） | 单例（源码自证 2 断言 + 结构复核） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `DecisionInput` / `BacktrackOption` 构造 fixture | **不 mock**：白名单项以内存 `BacktrackOption` 字面量构造（`phase` 为唯一判据位，`description` / `artifacts` 取空）——沿既有 `allowed_phase` 夹具先例 | `ensure_backtrack_allowed` 用例 |
| 无其它 | **不 mock**：`args` 为内存 `serde_json::json!` 字面量，纯函数零进程边界依赖 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

#### 待测功能

- `decision_answer_prompt()`: `decision_answer_prompt(answer: &str) -> String`——ask 应答的唯一再提示面：应答原文逐字在场 + 一句「请基于应答调用 `finish_step` 提交决策」（公共函数 / API）

<!-- design.md 未声明其余函数的公共 API 变更：`evaluator_protocol` / `decision_prompt` 的改动是「协议段改工具协议」（`pub` 面签名零变，属 design「修改文件」表行），两个 `*_fix_prompt` 是**退役**面（design「退役的公共面」段）。 -->

> 说明：用例按 AC-12（`evaluator_protocol` 不再要求最终消息输出 JSON 形状、保留禁调写通道红线与 `report` 上限；`decision_prompt` 协议段改工具协议）、AC-7（`verdict_fix_prompt` 退役）/ AC-8（`decision_fix_prompt` 退役）与 design D4（`finish_step` 名字单点，prompt 侧只作引用不作形状定义）组织。既有 `prompt_test` 中针对两个 `*_fix_prompt` 的段落断言**整体退役**；`backtrack_option_*` 三私有助手的既有用例经 `decision_prompt` 间接观测，逐字零改。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `evaluator_protocol` 工具协议改写（AC-12） | 正向 | 经 `evaluator_prompt(phase_prompt)` 产出的文本含 `finish_step`；含禁调写通道红线（`phase_log` / `phase_next` / `phase_start` / `backtrack` 四名逐字在场 + 「禁止调用」语义）；含 `report` 上限数 `2000`；含「逐项」与 `evidence` 语义 | 单例（4 断言） |
| `evaluator_protocol` 删 JSON 形状（AC-12） | 异常 | 产出文本内**零** JSON 形状块要素：`"verdict"` / `"checklist"` / ` ```json ` 围栏 / `"report":` 键面逐项零命中；`phase / attempt / skipped` 三词零命中（回声禁令随形状块一并删除） | 单例（5 否定断言） |
| `evaluator_prompt` 拼接契约（回归锚定） | 边界 | `evaluator_prompt("主体")` 以 `"主体"` 开头、协议段以 `\n\n---\n\n` 分隔（唯一动态 append 面）；`phase_prompt == ""` → 产出非空且不 panic；静态主体部分逐字不被协议段改写污染 | 表驱动（3 断言） |
| `decision_prompt` 协议段改工具协议（AC-12） | 正向 | `decision_prompt(input)` 的协议段含 `finish_step`、含三动作名（`backtrack` / `stop` / `ask`）、含「白名单」与 `reason ≤ 500` 语义；失败相位报告 / 失败清单 / 白名单 / 候选报告四段与 `backtrack_option_*` 渲染逐字不变（既有四段断言零改） | 单例（3 新增断言 + 既有保留） |
| `decision_prompt` 删 JSON 代码块（AC-12） | 异常 | 产出文本内 ` ```json ` 围栏零命中、`"action":` 键面零命中；`<MAX_REASON_CHARS>` 数值仍以 `500` 形态提示（上限不退化为字面占位） | 单例（3 否定断言） |
| `decision_answer_prompt` 单点（AC-8） | 正向 | `decision_answer_prompt("选指数退避")` 含应答原文 `选指数退避` 逐字、含 `finish_step`、含「决策」语义；`answer == ""` → 产出非空不 panic；两不同应答产出**不相等**（原文进提示，非固定模板） | 表驱动（3 形态） |
| `decision_answer_prompt` 边界值（str 边界） | 边界 | 应答含 `\n` / `\0` / emoji / 超长（5000 字符）→ 逐字保真、不 panic、不截断（应答原文即唯一信息面）；应答含 `{}` 与 `"action"` 字样 → 原样透传（不转义、不解析——它不是协议载体） | 表驱动（5 形态） |
| 两个 `*_fix_prompt` 退役（AC-7 / AC-8） | 异常 | 源码自证：`prompt.rs` 内 `verdict_fix_prompt` / `decision_fix_prompt` 零命中；`prompt_test.rs` 内两词的段落断言用例已整体删除（零残留注释掉的用例体） | 单例（源码自证 2 断言） |
| 其余 prompt 面零改动（回归锚定） | 边界 | `spec_sync_prompt` / `merge_conflict_prompt` / 归档 prompt 单点的既有断言逐字保留（`git diff` 面：该三块无 hunk） | 单例（结构复核） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `DecisionInput` / `BacktrackOption` fixture | **不 mock**：内存字面量构造（失败相位 / attempt / fail checklist / 白名单 / 候选报告逐字段直填），沿既有 `prompt_test` 装置 | `decision_prompt` 用例 |
| 无其它 | **不 mock**：纯字符串组装函数，零 IO / 零网络 / 零子进程 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- `WorkerTurnOutcome.finalized()`: `Option<FinishPayload>` 在场、可选（`None` 合法），`Some` 载荷往返保真（字段面 / AC-6）
- `WorkerTurnOutcome.derives()`: `Debug` / `Clone` 保持，`finalized` 随 `clone()` 等值（回归锚定；用例：`finalized` 默认缺席）
- `WorkerTurnOutcome.field_independence()`: `final_message` 与 `finalized` 互不推导，两字段独立（两组合均合法）（边界；用例：`final_message` 语义未变）
- `WorkerTurnOutcome.payload_tolerance()`: 载荷损坏不外溢——`port` 层零 `unwrap()` / `expect(`，损坏形态不升级为 `Err` / panic（异常面 / AC-6；用例：`finalized` 载荷损坏不外溢）

<!-- design.md 未声明该文件的公共 API 变更：改动是 `WorkerTurnOutcome` 增 `pub finalized: Option<FinishPayload>`（类型定义面，design「类型定义」表行），非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-6（`WorkerTurnOutcome.finalized` 字段在场与默认 `None`）组织。既有 `port_test` 的五假 port 装置与写面原生载荷断言零改，本节只补字段面与派生面锚定。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `finalized` 字段在场（AC-6） | 正向 | 构造 `WorkerTurnOutcome { …, finalized: Some(FinishPayload::Evaluator(..)) }` 并读回逐字段相等（`finish_payload` 往返保真）；`finalized: None` 亦为合法构造（字段是 `Option`） | 表驱动（2 形态） |
| `finalized` 默认缺席（AC-6 / Optional 边界） | 边界 | 既有 3 处字面构造补 `finalized: None` 后**其余字段断言零改**（`session_id` / `status` / `final_message` / `transcript` 逐字段值不变）；既有 `Debug` / `Clone` 抛锚点仍成立（`format!("{:?}")` 非空、`clone()` 后 `finalized` 等值） | 单例（3 断言） |
| `final_message` 语义未变（回归锚定） | 边界 | `final_message` 仍承载「末条 assistant 文本」，与 `finalized` **互不推导**（两者独立字段：`final_message` 非空 + `finalized: None` 组合合法、`final_message` 空 + `finalized: Some` 组合亦合法） | 表驱动（2 组合） |
| `finalized` 载荷损坏不外溢（AC-6 / 异常面） | 异常 | 转录内 `finish_step` notice 的 `payload` 为损坏形态（`null` / `{}`）时 worker 侧 `finalized == None` 且 `WorkerTurnOutcome` 仍返回 `Ok`（提取的宽容性由 `transcript::finish_payload` 保证，`port` 层不引入新的 `Err` 面）；`finalized: Some(..)` 的载荷字段若被下游 `match` 穷尽（`Evaluator` / `Decision` 两臂），编译期即保证穷尽——源码自证 `port.rs` 内零 `unwrap()` / 零 `expect(`（不把载荷提取失败升级为 panic） | 单例（2 断言 + 源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：结构体字面构造与派生面断言；既有五假 port 装置沿用（它们是被测 API 的显式注入依赖，非新增替身） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/transcript_test.rs

#### 待测功能

- `finish_payload()`: `finish_payload(transcript: &[agent::AgentEvent]) -> Option<FinishPayload>`——倒序找末条 `SystemNotice { subtype: "finish_step" }` 并 `serde_json::from_value` 其 `payload`；缺失 / 反序列化失败 → `None`（公共函数 / API；用例：末条 notice 命中 / decision 形态命中 / 缺 notice → `None` / payload 损坏 → `None` / 多条取末条 / 规模与位置边界 / 单点性）

<!-- `final_assistant_text` / `failure_reason` 逐字不动，非本变更加载点。 -->

> 说明：用例按 AC-6 与 design D10（末条命中 / 缺失 → `None` / 损坏 → `None`）组织。既有 `transcript_test` 的 `final_assistant_text` 用例（多文本块拼接 / 工具块不参与 / 空转录缺席）**逐字保留**（该函数零改动），本节只新增 `finish_payload` 族。事件 fixture 用既有 `message` / `notice` 装置（内存 `AgentEvent::stamp` 构造，零 IO）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 末条 notice 命中（AC-6） | 正向 | 转录含一条 `SystemNotice { subtype: "finish_step", payload: {"evaluator": {verdict, report, checklist}} }` → `Some(FinishPayload::Evaluator(..))`，逐字段相等（`verdict` / `report` / `checklist[0].item`） | 单例（3 断言） |
| decision 形态命中（AC-6） | 正向 | `payload: {"decision": {"action":"ask","question":"…","options":["甲"]}}` → `Some(FinishPayload::Decision(Ask { .. }))` 逐字段相等（两臂皆可辨） | 单例 |
| 缺 notice → `None`（AC-6 / Optional 边界） | 边界 | 空转录 `&[]` → `None`；只有 `Message` / `ToolUse` / 其他 subtype（`api_retry` / `context_pruned` / `output_truncated`）的转录 → `None`（`output_truncated` 存在但无 `finish_step` 时仍 `None`——截断的判据在 walker，不在本函数） | 表驱动（3 形态） |
| payload 损坏 → `None`（AC-6） | 异常 | `subtype == "finish_step"` 但 `payload` 为 `null` / `{}` / `{"unknown": 1}` / `{"evaluator": "str"}`（内层形状错）→ 各 `None`，不 panic、不 `unwrap` 抛错 | 表驱动（4 形态） |
| 多条取末条（AC-6 / list 边界） | 边界 | 两条 `finish_step` notice（先 `Decision::Stop`、后 `Evaluator`）→ 取**末条**（`Some(Evaluator)`）；末条损坏、前条完好 → `None`（MUST NOT 回退到前条——「末条」是语义而非「最后一条可解析的」） | 表驱动（2 形态） |
| 规模与位置边界（list 边界） | 边界 | 1000 条事件、`finish_step` 在第 1 条（最前）→ 仍 `Some`（倒序扫描覆盖全长）；1000 条事件中零 notice → `None` 且不超时；`subtype` 大小写漂移（`Finish_Step`）→ `None`（字面量精确匹配） | 表驱动（3 形态） |
| 单点性（源码自证，AC-6） | 异常 | `transcript.rs` 内 `fn finish_payload` 恰 1 处；`infra/agent/src/worker.rs` 内调 `orchestration::transcript::finish_payload` 恰 1 处且零第二份 `from_value::<FinishPayload>` 提取实现 | 单例（源码自证 2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：转录为内存 `Vec<AgentEvent>`（既有 `AgentEvent::stamp` 装置构造）；`serde_json` 反序列化即被测面（替身会掩盖「损坏 payload → `None`」判据）；零文件 / 网络 / DB / 子进程 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- `step_verdict_gate()`: 单一裁决（原 `step_verdict_gate` 与 `step_verdict_gate_terminal` 合并）——`finalized` 在位 → `cursor.verdict` 落位 + 节点 passed；缺席（未调 `finish_step` / `output_truncated` / `eval_outcome` 缺失）→ `VerdictGate` failed + `Terminal::failed`，成因文案可区分；两 `on_fail` 声明（`FeedbackLoop{limit}` / `Terminal`）走同一裁决函数（行为面 / AC-7；用例：verdict 门 `finalized` 在位 / verdict 门缺席：未调 `finish_step` / verdict 门缺席：输出被截断 / verdict 门：`eval_outcome` 缺席 / verdict 门合并形态）
- `decision_session()`: 单轮收口（`decision-log` 恰一次）→ 三动作分派（`stop` / `backtrack` 白名单门 / `ask` 停等、第二轮 `decision_answer_prompt`、`wait_answer` 返 `None` 即 `Terminal::stopped`），越权一次即停；`finalized` 缺席与相位表角色漂移两臂均 `Terminal::failed`（行为面 / AC-8；用例：决策单轮收口 / 决策 backtrack 合法 / 决策 ask 停等外移 / 决策 ask 停等被终止 / 决策越权一次即停 / 决策 `finalized` 缺席与角色漂移）
- `mechanical_checklist_entries()`: 机械代写条目七处（`step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`）文案逐字在场、不含 ID、零 `finish_step` 调用，且 `git diff` 四块无 hunk（豁免面 / AC-10 ①；用例：机械 checklist 七条目豁免 / 机械代写点零改动）
- `retired_surfaces()`: 死面退役——`step_verdict_gate_upgraded_phase_log` / `"评估输出格式"` / `step_verdict_gate_terminal` / `DECISION_DRIFT_LIMIT` / `parse_verdict` / `parse_decision` / 两个 `*_fix_prompt` 零命中（退役面 / AC-10 ②、AC-6、AC-7、AC-8；用例：死面退役）
- `feedback_limit()`: 本体五值边界与三臂 `Err` 文案逐字未改，调用点去一留二（回归锚定 / AC-10 ②）
- `FakeWorker.finalized_extraction()`: 夹具提取单点——经 `orchestration::transcript::finish_payload` 取值，`walker_test.rs` 内零第二份 `from_value::<FinishPayload>`（结构自证 / AC-6；用例：夹具提取单点）

<!-- design.md 未声明该文件的公共 API 变更：改动面是 `step_verdict_gate` 与 `step_verdict_gate_terminal` 合并为单一裁决、`step_verdict_gate_upgraded_phase_log` 整函数删除、`decision_session` 改写（均为行为面，属 design「修改文件」表行；`/公共函数 / API` 表零行）。`feedback_limit` 本体逐字不动、只删一处调用点。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-7（verdict 门单一裁决 + 两成因文案）、AC-8（决策三形态 + drift 环退役 + 越权一次即停）、AC-10（机械 checklist 豁免 + 死面退役）与 design D7 / D8 / D10 组织。
>
> **夹具语义改造（R1 的核心风险面）**：`FakeWorker` 的产出夹具由「最终消息文本 JSON」改为「注 `SystemNotice{finish_step}`」——沿既有 `with_transcripts()` 注入口，且**必须调 `orchestration::transcript::finish_payload` 从注入转录取值**（design 事实校准 9：MUST NOT 在测试面另写提取实现，否则「提取实现单点」出现第二份）。改造面按 proposal 实测：`with_evaluator_reports` 18 处 / `with_decision_reports` 35 处，另 3 处 `WorkerTurnOutcome` 字面构造补 `finalized` 位。
>
> **退役用例（整体删除，零替代面）**：`verdict解析失败反馈边修复_续注同evaluator会话重解析`（`:3734`）与 `真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop`（`:3844`）——被测路径（反馈环续注与超限升格双槽位落账）随反馈环与 `step_verdict_gate_upgraded_phase_log` 一同消失；后者的 `item == "评估输出格式"` 断言与 30 轮 evaluator 会话计数断言无替代面，「`finalized` 缺席即 `Terminal::failed`」由下表新用例承接。**`feedback_limit_escalate误声明显式漂移err`（`:11094`）与 `feedback_limit` 五值边界用例保留零改**（本变更加载点只有调用点删除，函数本体逐字不动；既有用例本就以 static-check 为驱动面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| verdict 门 `finalized` 在位（AC-7） | 正向 | 注 evaluator 收口 notice 的转录 → `cursor.verdict == Some(EvaluatorChecklist { verdict: Pass/Fail, .. })`（逐字段相等）；`VerdictGate` 节点为 passed 且 detail 含 `verdict=pass` / `verdict=fail`（两值各一例）；后续 PhaseLog 步落账的 `PhaseLogInput.checklist` 与载荷 checklist 逐字段相等 | 表驱动（2 值 × 3 断言） |
| verdict 门缺席：未调 `finish_step`（AC-7） | 异常 | 会话 Completed、转录零 `finish_step` notice（有正常 assistant 文本）→ `VerdictGate` 节点 failed、`Terminal::failed`，文案含「未调用 finish_step 收口」语义；run 状态收口为 failed；**反馈边节点零产出**（`EdgeKind::Feedback` 边计数 `0`）、evaluator 会话计数 `1`（零续注） | 单例（5 断言） |
| verdict 门缺席：输出被截断（AC-7 / 两文案可区分） | 异常 | 转录含 `SystemNotice{subtype:"output_truncated"}` 且无 `finish_step` notice → 同样 `VerdictGate` failed + `Terminal::failed`，文案含「输出被截断」语义且与上一条文案**不相等**（成因可区分，用户可调 `max_tokens` 归因） | 单例（2 断言 + 对照） |
| verdict 门：`eval_outcome` 缺席（AC-7） | 异常 | worker 返回 `Err`（无 outcome）→ 计划图漂移类 `Terminal::failed`，文案与既有 `:1610` 同款（逐字不变）；不 panic | 单例 |
| verdict 门合并形态（AC-7 / D8 合并判据） | 边界 | `on_fail=FeedbackLoop{limit}`（原 `step_verdict_gate`）与 `on_fail=Terminal`（原 `step_verdict_gate_terminal`，code-analyze 相位）两声明下走**同一裁决函数**：两者在 `finalized` 在位时均 passed、缺席时均 `Terminal::failed`；两声明下的 evaluator 会话计数均 `1`（`on_fail` 分派已消失） | 表驱动（2 声明 × 2 结局） |
| 决策单轮收口（AC-8） | 正向 | decision 会话注 `{"decision":{"action":"stop","reason":"…"}}` → 会话数 `1`、轮数 `1`；`decision-log` 挂账恰一次（幂等覆写，命令逐字段相等）；`WhitelistGate` 节点在场；run 以 `Terminal` 收口且 `action == Stop` | 单例（4 断言） |
| 决策 backtrack 合法（AC-8） | 正向 | `{"decision":{"action":"backtrack","to":"<白名单内相位>","reason":"…"}}` → 挂账一次 + `WhitelistGate` passed 节点 + 返回 `Backtrack`（`to` / `reason` 逐字段相等）；交写面 `backtrack` 的单事务语义零改 | 单例 |
| 决策 ask 停等外移（AC-8） | 正向 | 首轮注 `ask` notice → `guard.emit(RunUpdate::Ask)` 候选中出现 Ask 载荷（`question` / `options` 逐字段）+ `wait_answer` 被调用一次；应答就绪后**新一轮** `run_worker` 发起：会话数 `1`（`SessionRef::Continue` 复用同会话）、轮数 `2`、第二轮 prompt 为 `decision_answer_prompt(应答)`（逐字对拍）；第二轮注 `stop` → 收口为 `Stop`；ask 轮**不发** `WhitelistGate` 节点、不计失败、不计漂移（drift 常量已删） | 单例（6 断言） |
| 决策 ask 停等被终止（AC-8 / Optional 边界） | 异常 | `wait_answer()` 返回 `None` → `Terminal::stopped("ask 停等被终止")` 形态收口；不再发起第二轮会话（轮数恒 `1`）；`decision-log` 仍恰一次挂账（挂账置于动作裁决之前的既有顺序） | 单例（3 断言） |
| 决策越权一次即停（AC-8） | 异常 | `backtrack.to` 不在白名单 → `WhitelistGate` failed 节点 + `Terminal::failed`，文案携 `allowed` 各相位 id；决策会话数恒 `1`（零软拒续轮、零 drift 计数）；`ensure_backtrack_allowed` 逐字未改（既有自环 / 越权两文案断言保留）；对照白名单内目标 → passed（判据力自证） | 表驱动（2 形态） |
| 决策 `finalized` 缺席与角色漂移（AC-8） | 异常 | 转录零 `finish_step` notice → `Terminal::failed`（成因两文案同 verdict 门口径：截断 / 未收口）；注 `{"evaluator": {…}}`（角色漂移，理论不可达）→ `Terminal::failed` 文案含「相位表角色漂移」语义；两分支均不 panic | 表驱动（3 形态） |
| 机械 checklist 七条目豁免（AC-10 ①） | 边界 | 静态检查失败 / 架构检查失败（`step_gate_fail_phase_log` 两调用点）/ 测试执行三处（suite 结论一致 / 聚合计数一致 / mutation null 自动通过）/ 测试执行升格 / 突变测试升格——七条目文案各逐字在场，落账 `ChecklistItem.item` 为硬编码中文短语（**不含 ID**），且全程零 `finish_step` 调用、零 ID 集合校验 | 表驱动（7 条目） |
| 机械代写点零改动（AC-10 ① / 结构复核） | 边界 | `git diff -- crates/core/orchestration/src/walker.rs` 中 `step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 四块**无 hunk**；`ChecklistItem {` 构造点由 7 降为 **6**（差额即退役的 `"评估输出格式"`） | 单例（结构复核 2 断言） |
| 死面退役（AC-10 ②） | 异常 | 源码自证：`step_verdict_gate_upgraded_phase_log` / `"评估输出格式"` / `step_verdict_gate_terminal` / `DECISION_DRIFT_LIMIT` / `parse_verdict` / `parse_decision` / `verdict_fix_prompt` / `decision_fix_prompt` 八词在 `walker.rs` 零命中（`_test.rs` 之外零命中亦断言）；`feedback_limit(` 调用点去一留二（`"static-check"` / `"archi-check"` 各 1、`"verdict-gate"` 0） | 单例（源码自证 3 断言） |
| `feedback_limit` 服务面收窄（AC-10 ②） | 边界 | 既有五值边界用例（`FeedbackLoop{0}` / `{1}` / `{u32::MAX}` / `Terminal` / `None`）**逐条保留零改**，仍经 static-check 步驱动；`feedback_limit_escalate误声明显式漂移err` 保留（函数本体的三臂 `Err` 文案逐字不变） | 表驱动（5 值） |
| 夹具提取单点（AC-6 / D10 结构自证） | 边界 | `walker_test.rs` 内 `FakeWorker` 的 `finalized` 取值调 `orchestration::transcript::finish_payload`（导入即判据）；`from_value::<FinishPayload>` 在 `walker_test.rs` 内零命中（MUST NOT 另写提取）；`with_transcripts()` 注入口被两族报告夹具复用 | 单例（源码自证 2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkerAgentPort`（会话执行 port） | 进程内假件 `FakeWorker` 实现 trait——按脚本返回 `WorkerTurnOutcome`（含注 notice 的转录）。理由：它是 walker 的**显式注入依赖**（`RunRequest` 的 port 入参），真件会真起 provider 会话（进程边界 + 网络）。既有装置沿用，只改产出夹具的**语义**（文本 JSON → 注 notice），不新增替身 | verdict 门 / 决策两族全部用例 |
| `RunEventSink`（运行事件出口） | 进程内假 sink 捕获全部 `RunUpdate`（`Ask` / 节点 detail 断言面）。理由：同上——port 注入依赖，且真件落 DB | 全部用例 |
| `RunHistoryPort` / `WorkflowSnapshotPort` / 静检 / 架构检查 / 差异 port | 假件沿用既有装置（捕获命令逐字段比对 + 可编程 fault）。理由：均为 walker 显式注入的 port，真件跨进程（cargo / archi CLI / redb） | 相位全链用例、机械 checklist 用例 |
| `orchestration::transcript::finish_payload` | **不 mock**：`FakeWorker` 真实调它（单点即被测语义，另写提取会让「提取实现单点」在测试面出现第二份——design 事实校准 9 明禁） | 全部注 notice 的用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

#### 待测功能

- `registry()`: `registry() -> &'static [ToolMount]`——十项静态挂载清单（唯一工具事实源）；前七项**必须**恰是七内建（`tools: None` 的语义载体）（公共函数 / API）
- `definitions()`: `definitions(injections: &agent::SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>`——名字查表（`tools: None` → 前七项；`Some(names)` → 按 registry 序过滤）+ `finish` 位铸造 `finish_step`；非法名 `Err`（公共函数 / API）
- `lookup()`: `lookup(name: &str) -> Option<rig::completion::ToolDefinition>`——按名取定义（克隆出口），compose 装配层成员校验与 `definitions` 取表的共同单点（公共函数 / API）
- `injected()`: `#[cfg(test)] injected(names: &[&str]) -> agent::SessionInjections`——压平构造面的测试夹具（同族 `injected_finish(names, spec)`）（公共函数 / API）

<!-- `ToolMount` / `ToolTier` / `ToolExec` 与 `TOOL_NAMES` / 三个专用工具名常量取自 design「类型定义」表与 D1 / D2 / D4，非「公共函数 / API」表的四行之外。 -->

> 说明：用例按 AC-1（十项单源 / 名字查表 / 非法名 `Err`）、AC-2 的 tier 声明面（`create_explore_note` 归 Write、两 archi 工具归 Read、`bash` 归 Execute）、AC-4（`finish` 位铸造与 `enum` 对拍）与 design D2（`OnceLock` 惰性静态表）/ D3（`Err` 是防御面）/ D4（工具名作者单点）组织。`definitions` 对非法名的语义由「静默滤过」**显式改为 `Err`**（design 事实校准 3）——既有 `definitions非法名与重复名经封闭清单保序滤除` 用例须**改造**为 `Err` 断言（本变更的验收面本身，非回归漂移）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `registry` 十项名字唯一且有序（AC-1） | 正向 | 名字序列逐值等于 `["read","grep","glob","ls","write","edit","bash","create_explore_note","archi_module","archi_search"]`；`len() == 10`；无重复名（`HashSet` 去重后仍 10）；`registry()[..7].map(name) == TOOL_NAMES`（前七项恰七内建） | 单例（4 断言） |
| `registry` tier 归属（AC-2 声明面） | 正向 | 逐项断言：Read 六项（`read` / `grep` / `glob` / `ls` / `archi_module` / `archi_search`）、Write 三项（`write` / `edit` / `create_explore_note`）、Execute 一项（`bash`）——与改造前三张常量表逐项对应 | 表驱动（10 项） |
| `registry` exec 归属（AC-1 / D1） | 正向 | 七内建 → `ToolExec::Pure`；`create_explore_note` → `ExploreNote`；两 archi 工具 → `Archi`；`matches!(mount.exec, ToolExec::FinishStep(_))` 在静态表内**零命中**（`finish_step` 不入静态 registry） | 表驱动（9 + 1 否定断言） |
| `registry` 定义面完整（AC-1） | 边界 | 十项的 `definition.name == registry 名`、`description` 非空、`parameters` 为对象且含 `"type": "object"`；七内建的定义与改造前 `tools.rs` 逐字段相等（字面量零改——以既定期望快照对拍，非「只判非空」） | 表驱动（10 项 × 3 断言） |
| `registry` 惰性单点与幂等（D2） | 边界 | 两次 `registry()` 返回**同一指针**（`std::ptr::eq`，`OnceLock` 单点语义）；十项内容逐字段相等（同一次 `get_or_init` 不重建） | 单例（2 断言） |
| `definitions` 内建七件（AC-1） | 正向 | `definitions(&SessionInjections::default(), "")`（`tools == None`）→ `Ok` 且 `len() == 7`，名字序列 == `TOOL_NAMES`（`tools: None` 的语义载体）；`finish_step` 零命中 | 单例（2 断言） |
| `definitions` 名字清单收窄（AC-1） | 正向 | `tools: Some(["read","archi_module"])` → 下发两项且按 **registry 序**（`read` 先于 `archi_module`）；`tools: Some(["bash","read"])` → 顺序为 `["read","bash"]`（不按入参序）；专有工具以**名字**进清单后定义由 registry 提供（`create_explore_note` / `archi_module` / `archi_search` 各一例） | 表驱动（3 形态） |
| `definitions` 重复名与空清单（list 边界） | 边界 | `tools: Some(["read","read"])` → 下发**一份** `read`（不重复下发）；`tools: Some(vec![])` → `Ok` 且 `len() == 0`（合法空面，与 `None` = 七件的语义可辨）；`tools: Some(["read",""])`（空串元素）→ `Err`（空串是非法名） | 表驱动（3 形态） |
| `definitions` 非法名 `Err`（AC-1 / 语义切换点） | 异常 | `tools: Some(["read","no_such_tool"])` → `Err(msg)`，`msg` 含非法名 `no_such_tool`（可归因）；`tools: Some(["README"])`（大小写漂移）→ `Err`；`Err` 面 MUST NOT 返回部分清单（`Err` 不是「滤过后的子集」——与改造前静默滤过显式不同） | 表驱动（3 形态） |
| `definitions` `finish` 位不置时零追加（AC-4） | 边界 | `finish: None` + 任意 `tools` → 结果名单与「同 `tools` 且本变更前」逐名相等（`finish_step` 零命中）；explore / archi 两臂的注入面（`finish: None`）同样零追加 | 表驱动（3 形态） |
| `definitions` `finish` 置位追加（AC-4） | 正向 | `finish: Some(EvaluatorChecklist)` + `tools: None` → `len() == 8`，末项 `name == "finish_step"`、`tier == ToolTier::Protocol`、`exec` 匹配 `ToolExec::FinishStep(FinishSpec::EvaluatorChecklist)`、`description == orchestration::finish::description(spec)`；**追加在末尾**（前七项逐名不变，`RunStarted.tools` 其余名单不变） | 单例（6 断言） |
| `definitions` 动态 enum 对拍（AC-4） | 正向 | `definitions(&injected_finish(&[], EvaluatorChecklist), prompt)` 的 `parameters.properties.items.items.properties.id.enum` **逐值有序等于** `checklist_entries(prompt)` 的 ID 序列（3 份不同 prompt 各一例）；`prompt == ""` → `enum` 键缺失且其余结构在（退化面不 panic） | 表驱动（3 prompt + 1 退化） |
| `definitions` decision 形态（AC-4） | 正向 | `finish: Some(Decision)` → 末项 `finish_step`，`description == description(Decision)`（与 evaluator 形态**不相等**），`parameters` 为三动作形状（含 `backtrack` / `stop` / `ask`）；`prompt` 内容对 decision 形态零影响（两次不同 prompt 产出逐字段相等） | 单例（3 断言 + 对照） |
| `definitions` 非法名与 `finish` 并存（异常优先序） | 异常 | `tools: Some(["no_such_tool"])` + `finish: Some(Decision)` → `Err`（不静默降级为「只下发 `finish_step`」——`Err` 先于铸造，defend 判据） | 单例 |
| `lookup` 命中与未命中（AC-1） | 边界 | 十项名字逐个 `lookup` → `Some(def)` 且 `def.name` 与入参相等（克隆出口不改名）；`""` / `"Read"`（大小写漂移）/ `"read "`（尾空格）/ `"no_such_tool"` → 全 `None`（精确匹配、零归一化） | 表驱动（10 命中 + 4 未命中） |
| `lookup` 与 `registry` 同源（AC-1 单点判据） | 正向 | 对每项：`lookup(name)` 的返回值与 `registry()` 中该项的 `definition` **逐字段相等**（`lookup` 不产第二份定义）；源码自证 `mount.rs` 内十项字面构造恰在 `build_registry()` 一处 | 单例（2 断言） |
| `injected` / `injected_finish` 夹具形态 | 边界 | `injected(&["read","archi_module"])` → `tools == Some([...])`、`preamble == None`、`finish == None`；`injected(&[])` → `tools == Some(vec![])`（非 `None`——夹具表达「显式空清单」）；`injected_finish(&["read"], Decision)` → `finish == Some(Decision)`；两夹具产出可 `assert_eq!`（`Eq` 链） | 表驱动（4 形态） |
| 类型派生面（D1） | 边界 | `ToolTier` / `ToolExec` 编译期锚定 `Copy + Eq`（赋值后原值仍可用 + `assert_eq!`）；`ToolMount` 锚定 `Debug + Clone`（`format!("{:?}")` 非空、`clone()` 后 `definition.name` 相等）；源码自证 `pub struct ToolMount` 的 `derive` 行**不含** `PartialEq` / `Eq`（内含 `serde_json::Value`，D1 显式留白） | 单例（3 断言 + 源码自证） |
| 工具名作者单点（D4 / AC-1） | 异常 | 源码自证：`TOOL_NAMES` / `EXPLORE_NOTE_TOOL` / `ARCHI_MODULE_TOOL` / `ARCHI_SEARCH_TOOL` 四常量定义只在 `mount.rs` 一处；`policy.rs` / `loop.rs` 内 `TOOL_NAMES` 零命中（「不参与任何判定」的可机械判据）；`"finish_step"` 字面量在 `mount.rs` 零命中（经 `orchestration::finish::TOOL_NAME` 引用） | 单例（源码自证 3 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：`registry()` / `definitions` / `lookup` 全为进程内纯函数（`OnceLock` 一次性静态表，无 IO / 无 spawn / 无网络）；`prompt` 用内存合成静态清单字符串；`orchestration::finish::{description, parameters}` 真实调用（铸造内容即被测面，替身会掩盖 enum 对拍） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

#### 待测功能

- `allows()`: `allows(mode: AgentPermissionMode, tier: ToolTier) -> bool`——tier 准入单点（`ToolTier::Protocol` 与 `ToolTier::Read` 恒 `true`；`ToolTier::Write` 与 `ToolTier::Execute` 需 `mode != AgentPermissionMode::Default`）（公共函数 / API；用例：12 格矩阵 / 与旧三张常量表逐格等价 / 未知工具名面 / 常量表退役 / 纯函数与幂等）

> 说明：用例按 AC-2（4 × 3 矩阵逐格 + 与改造前三张常量表**逐格等价**）组织。既有 `policy_test` 的三档决策表用例**重写**为 tier 轴 12 格（工具名轴已退役），并保留「拒绝流出形态」用例（`SystemNotice{permission_denied}` + `is_error` ToolResult）——后者被测面在 loop，本节只作等价性锚定。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 12 格矩阵（AC-2） | 正向 | 逐格断言：`Default` × {`Protocol`: true, `Read`: true, `Write`: false, `Execute`: false}；`AcceptEdits` × 四 tier 全 true；`BypassPermissions` × 四 tier 全 true | 表驱动（12 格） |
| 与旧三张常量表逐格等价（AC-2 对拍） | 正向 | 测试体内以**旧表快照**（`READONLY_TOOLS` 6 名 / `WRITE_TOOLS` 3 名 / `EXECUTE_TOOLS` 1 名 + 旧 `allows(mode, tool_name)` 判定逻辑）重建期望函数，对 3 档 × 10 工具名的 30 格逐格断言 `allows(mode, registry_tier_of(name)) == 旧判定(mode, name)`（改造前后逐格相同，含 `create_explore_note` 归 Write、两 archi 归 Read、`bash` 归 Execute） | 表驱动（30 格对拍） |
| 未知工具名面（异常） | 异常 | 旧表面对未知名一律 `false`；tier 面在类型层不可表达「未知 tier」（`ToolTier` 四值闭合），故断言改造后**不存在**「清单外工具名」判定面：`registry()` 的十项 tier 全部可判，第十一项名字 `lookup` 返回 `None` 因而不进入 `allows` 调用（loop 成员校验先拒） | 单例（2 断言） |
| 常量表退役（AC-2 / 源码自证） | 异常 | 源码自证：`policy.rs` 内 `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` / `listed` 四名零命中；`fn allows` 恰 1 处且首参类型为 `AgentPermissionMode`、次参为 `ToolTier`（工具名轴零残留） | 单例（源码自证 2 断言） |
| 纯函数与幂等（边界） | 边界 | 同一 `(mode, tier)` 连续两次调用结果相同（零内部状态、零 IO）；四 tier × 三档共 12 次调用无 panic（覆盖 `ToolTier` 全值域） | 表驱动（12 次调用） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：纯决策表函数，入参为 `Copy` 枚举；旧表快照以测试体内常量 + 内联判定重建（不是替身模块，而是对拍基准）；零进程边界依赖 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

- `definitions.moved_out()`: 定义面迁出——`definitions` / `generic_definition` / `vocabulary` / 两 archi 定义 / `TOOL_NAMES` / 三专用常量的定义式在 `tools.rs` 零命中，改自 `mount` 引用（退役面 / AC-1；用例：定义面迁出）
- `execute.seven_tools()`: 七工具执行体（`read` / `grep` / `glob` / `ls` / `write` / `edit` / `bash`）与 `cap_result` 断言逐字零改（回归锚定 / D9；用例：执行体面零改动）
- `input_path()`: `glob` 取 `pattern`、两 archi 显式 `None`、`finish_step` 走既有默认臂、函数体无 hunk（边界 / D9）
- `TOOL_NAMES.constant_refs()`: 常量引用面——迁出后 `bash` 与两 archi 分支的工具名比较仍命中（结构锚定 / AC-1；用例：常量迁出后的引用面）

<!-- design.md 未声明该文件的公共 API 变更：`tools.rs` 的改动是「`definitions` / `generic_definition` / 两 archi 定义 / `TOOL_NAMES` / 三专用常量**迁出**（原文删除不留副本）」，执行体面（`execute` / `input_path` / `cap_result` / 七执行体）**逐字不动**——属 design「修改文件」表行，非「公共函数 / API」表行（该表零行）。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-1（定义面迁出、`TOOL_NAMES` 降级为定义作者处）与 D9（`input_path` 与 `protect` 零改动）组织。既有 `tools_test` 的 `definitions` 族用例（13 处调用点）**迁移至 `mount_test`** 或改造为 `mount::definitions(...)` 调用；执行体族（`execute` 七工具 / `input_path` / `cap_result` / 沙箱）断言**逐字零改**——它们是本变更的显式「不要修改」面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 定义面迁出（AC-1 / 源码自证） | 异常 | 源码自证：`tools.rs` 内 `fn definitions` / `fn generic_definition` / `fn vocabulary` / `fn archi_module_definition` / `fn archi_search_definition` / `const TOOL_NAMES` / 三专用常量的定义式**零命中**（全部迁入 `mount.rs`，原文删除不留副本）；`use crate::sdk::mount::{…}` 行在场 | 单例（源码自证 2 断言） |
| 执行体面零改动（D9 / 回归锚定） | 正向 | 既有 `execute` 七工具用例（`read` 的 offset/limit / 大文件预览、`grep` 的递归与上限、`glob` 上限、`ls`、`write`、`edit`、`bash`）**逐条保留零改**；`cap_result` 的 L1 字节上限用例零改 | 表驱动（既有全集） |
| `input_path` 两特判保留（D9 机械判据） | 边界 | `glob` 取 `pattern` 字段（非 `path`）；两 archi 工具（`archi_module` / `archi_search`）显式 `None`；`finish_step` 走既有默认臂（取 `path` 字段 → 缺失 → `None` → 沙箱原样返回，**零新增特判**）；`git diff` 面：`input_path` 函数体无 hunk | 表驱动（4 形态 + 结构复核） |
| 常量迁出后的引用面（AC-1） | 边界 | `tools.rs` 内经 `use` 引用的四名（`TOOL_NAMES` / 三专用名）在 `input_path` / 执行体处语义不变：`bash` 分支与两 archi 分支的工具名比较仍命中（既有执行体用例已覆盖，本节只作结构锚定） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：既有 `tempdir` 真件装置（`tempfile` 已在 `[dev-dependencies]`），沙箱与执行体用例全部落真实临时目录 | 执行体族用例 |
| 子进程（`bash` 执行体） | **不 mock**：真实 `bash` 子进程（既有装置，属被测语义本身——`bash` 执行体的超时 / 截断 / stderr 合并即被测面） | `bash` 族用例 |
| 无其它 | **不 mock**：纯函数（`input_path` / `cap_result`）以内存 `serde_json::json!` 入参 | 路径与截断用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

- `run()`: `pub(crate)` 泵函数的 `finish_step` 收口四步序——校验 `Ok` → `SystemNotice{finish_step}` + `TurnDone{is_error:false}` + 零额外请求；`Err` → `is_error:true` 回灌（文案即 `validate` 原文、三要素自足）且会话继续（行为面 / AC-5 ③、AC-9；用例：收口成功零额外请求 / 校验 `Err` 当场回灌且会话继续）
- `run.batch_fake_results()`: 同批假结果——`finish_step` 之后的调用产出 `is_error:false` 假结果、文案「本步已 finish_step 收口，该调用未执行」、真经 `sender` 发出（转录可见、零 `INTERRUPTED_RESULT`）（行为面 / AC-5 ①；用例：同批假结果 `is_error: false` 且真经 `sender`）
- `run.tool_result_pairing()`: 同批结果收齐——全部 `tool_use` 的结果进恰一条 user 消息，零悬空 `tool_use`（行为面 / AC-5 ②；用例：同批结果进恰一条 user 消息）
- `definitions.membership()`: `definitions` 换参与成员校验——名单自 `mount::definitions` 派生、`finish` 位未置时 `finish_step` 被 `tool_unavailable` 拒且不进校验器、非法名 `finish_error("tool_mount_error")` 早退（异常面 / AC-1、AC-4）
- `allows.tier_gate()`: tier 门与两闸——`ToolTier::Protocol` 在 `Default` 档放行、同批 `write` 仍被拒；沙箱与 `protect` 对 `finish_step` 零介入（边界 / AC-2、D9；用例：tier 门放行）
- `run.truncation()`: 截断终局——`output_truncated` notice 在场而 `finish_step` notice 零命中（异常面 / AC-7 成因；用例：截断终局不发 notice）

<!-- design.md 未声明该文件的公共 API 变更：`loop.rs` 的改动是「`definitions` 调用换参 + 成员校验按 mount 名单 + `policy::allows(mode, mount.tier)` + `match mount.exec` 四臂 + `finish_step` 收口四步序」——`r#loop::run` 是 `pub(crate)` 的内部泵函数，属 design「修改文件」表行，非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-5（收口三纪律逐条）、AC-4（`RunStarted.tools` 自同一 mount 名单派生）与 D7（四步序）组织，全部走既有 `MockScript` 流缝脚本化装置（`rig::test_utils`，零新增依赖）。既有 `loop_test` 的全部用例（流 / 重试 / 悬挂 / 暂停续跑 / 配对闭合 / 截断）**零改**，本节只新增收口族用例与 6 处 `SessionInjections` 构造换位。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 收口成功零额外请求（AC-5 ③） | 正向 | 单轮脚本：assistant 发一个 `finish_step` 调用（合法 args）→ 事件流含 `SystemNotice{subtype:"finish_step"}` **恰一条**且其 `payload` 可 `from_value::<FinishPayload>` 成功；`TurnDone{is_error: false}`；**请求数恒 1**（零额外请求，`turn_index` 未递增）；`RunStarted.tools` 含 `finish_step` 且其余名单逐名与 `mount::definitions` 一致 | 单例（5 断言） |
| 同批假结果 `is_error: false` 且真经 `sender`（AC-5 ①） | 异常 | 同一 assistant 消息发两个调用（先 `finish_step` 合法、后 `read`）→ `read` 的 `ToolResult` 在**密封事件流**内可见（真经 `sender` 发出）、`is_error == false`、`content == "本步已 finish_step 收口，该调用未执行"`；转录内 `INTERRUPTED_RESULT` **零命中**（假结果已入库，回放不补齐） | 单例（4 断言） |
| 同批结果进恰一条 user 消息（AC-5 ②） | 边界 | 同批三调用（`finish_step` + `read` + `bash`）→ `history` 末条 user 消息含**恰三条** `ToolResult` 块且与三个 `tool_use` id 一一对应（`results` 收齐先于 `return`，零悬空 tool_use）；顺序为调用序 | 表驱动（2 批形态：2 调用 / 3 调用） |
| 校验 `Err` 当场回灌且会话继续（AC-5 ③） | 异常 | 首轮 `finish_step` args 缺 `report` → `ToolResult{is_error: true, content == validate 的 Err 文案}`；无 `SystemNotice{finish_step}`、无 `TurnDone` 收口；**第二轮请求发起**（请求数 2、`turn_index` 递增）；第二轮合法 `finish_step` → 收口成功（同 turn 内可重试，无轮数上限） | 单例（4 断言） |
| `Err` 文案可见性（AC-9 / D6） | 异常 | 校验 `Err` 的 ToolResult 内容即 `orchestration::finish::validate` 的原文（含字段名 / 期望 / 当前值三要素），测试断言三段子串同时在位（模型可见的反馈面自足） | 表驱动（3 类 Err：缺项 / report 超长 / unknown action） |
| 成员校验先于收口（AC-1 / AC-4） | 异常 | `finish: None` 的注入面下调用 `finish_step` → `tool_unavailable` deny（`SystemNotice{permission_denied}` + `is_error: true`），**不**进入校验器、不发 `finish_step` notice；对照 `finish` 置位 → 正常收口（判据力自证） | 表驱动（2 形态） |
| tier 门放行（AC-2 / AC-4） | 边界 | `Default` 档下 `finish_step`（`ToolTier::Protocol`）**放行**（不被 `permission_denied` 拒）；`Default` 档下同批 `write` → `permission_denied`（tier 门仍按 Mount tier 生效，`finish_step` 未把门放宽） | 单例（2 断言） |
| 截断终局不发 notice（AC-7 成因 / 接受面） | 异常 | 脚本以 `max_tokens` 收尾（`finish_success` 路径）→ `TurnDone` 在、`SystemNotice{finish_step}` **零命中**（`finalized` 恒 `None`，D9：截断回退可接受）；`output_truncated` notice 在场（walker 侧成因判据的输入） | 单例（3 断言） |
| `definitions` 换参与 `Err` 早退（AC-1 / D3） | 异常 | inject 一个非法工具名（`tools: Some(["no_such_tool"])`）→ loop 以 `finish_error("tool_mount_error")` 早退：`TurnDone{is_error: true}`、`RunStarted` 后无请求发出（不静默降级为空工具面）、引擎日志含非法名 | 单例（3 断言） |
| 沙箱与保护闸零放宽（D9 / 回归锚定） | 边界 | 既有沙箱（路径越界）与写入保护（受保护文件）用例**逐条保留零改**；`finish_step` 的 args 不含路径字段 → `input_path` 返 `None` → 沙箱原样返回、`protect::evaluate` 走 `_ => None`（两闸对 `finish_step` 零介入） | 表驱动（既有全集 + 1 新形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| provider 模型流（rig Wire / Transport） | 既有 `MockScript` + 脚本化 `Transport`（`rig::test_utils`）——进程边界（网络 / provider 端点）的显式替代；逐轮预录流事件脚本。沿用既有装置，零新增 | 全部用例 |
| 事件通道（`mpsc::Sender<AgentEventKind>`） | **不 mock**：真实 `tokio::sync::mpsc` 通道 + 测试侧接收端收集（「假结果是否真经 `sender` 发出」的判据面本身就是真实通道） | 全部用例 |
| 专用工具执行上下文（`explore_note` / `archi`） | 既有假 `ExploreNoteContext` / `ArchiContext`（显式注入依赖），或用真实上下文 + `tempdir`（既有两族用例各自沿用） | 同批多调用用例 |
| 文件系统 | 既有 `tempdir` 真件（`tempfile`），工具执行体落真实临时目录 | `read` / `bash` 同批用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- `mint_archi_context()`: `pub(crate)` 内部函数，入参由 `Option<&SessionToolset>` 改名字切片——含 `archi_module` / `archi_search` 任一即铸造、空切片 / 近名 / 大小写漂移 → `None`（行为面 / AC-1）
- `contains_tool()`: 两铸造位（`explore_note` / `archi`）由枚举比较改名字包含，两臂互不污染（行为面 / AC-1、AC-4；用例：explore 铸造判据 / archi 铸造判据 / 两臂互不污染）
- `toolset_enum_absent()`: 枚举面零残留——`sdk/runner.rs` 内 `SessionToolset` / `toolset` 零命中、改引 `crate::sdk::mount::{…}` 常量（退役面 / AC-3；用例：枚举面零残留）

<!-- design.md 未声明该文件的公共 API 变更：`mint_archi_context` 是 `pub(crate)` 内部函数，改动为「入参由 `Option<&SessionToolset>` 改名字切片」，per-turn 铸造判据由枚举比较改名字包含——属 design「修改文件」表行，非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-1（专用工具以名字进清单、per-turn 铸造按名字包含）与 AC-4（per-turn 判据）组织。既有 `sdk/runner_test` 的 6 处 `SessionToolset` 构造换位为名字清单，另 3 处 `SessionInjections` 字面构造补 `finish` 位。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| explore 铸造判据（AC-1） | 正向 | `tools: Some([... "create_explore_note" ...])` → `explore_note` 上下文**铸造在场**（`Some`）；`tools: Some(["read","grep"])`（零该名）→ `None`；`tools: None`（内建七件，不含专用名）→ `None`；名字大小写漂移（`Create_Explore_Note`）→ `None` | 表驱动（4 形态） |
| archi 铸造判据（AC-1） | 正向 | 含 `archi_module` → `Some`；含 `archi_search` → `Some`；两者同在 → `Some`；两者皆无（`tools: None` / 空清单）→ `None`；近名（`archi` / `archi_module_x`）→ `None`（精确名比较） | 表驱动（5 形态） |
| `mint_archi_context` 入参形态（AC-1） | 边界 | 名字切片为空 `&[]` → `None`；workspace_root 不存在 → `None` 不 panic（既有判据保留）；切片含重复名 → 仍 `Some` 一份（不因重复二次铸造） | 表驱动（3 形态） |
| 两臂互不污染（AC-1） | 边界 | 只含 archi 名 → `explore_note == None` 且 `archi == Some`；只含 `create_explore_note` → `archi == None` 且 `explore_note == Some`（两判据独立，不互相触发） | 表驱动（2 形态） |
| 枚举面零残留（AC-3 / 源码自证） | 异常 | `sdk/runner.rs` 内 `SessionToolset` / `toolset` 零命中；`use` 面改引 `crate::sdk::mount::{…}` 常量（源码自证 2 断言） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 provider / 会话内核 | 既有假 session / kernel 装置沿用（被测函数是组装期纯判据，`SessionOpen` 以内存字面量构造，不真起会话） | 全部用例 |
| 文件系统 | 既有 `tempdir` 真件（`workspace_root` 需真实目录以走既有判据） | `mint_archi_context` 用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- `assemble_injections()`: 三臂（explore / archi / change）改产名字清单 + 独立 `finish` 位（crate 私有）——`role == "evaluator"` → `EvaluatorChecklist`、`"decision"` → `Decision`、`"executor"` → `None`；非法名返回 `Err(msg)` 携名与来源（行为面 / AC-1、AC-4；用例：explore 臂名字清单 / archi 臂名字清单 / change 臂矩阵命中 / `finish` 位按 role 置位 / 非法名 `Err`）
- `archi_matrix()`: 返回值由枚举改 `Option<Vec<&'static str>>`（crate 私有），7 行矩阵逐格与改造前子集对应；未命中行 → `None`（行为面 / AC-1；用例：change 臂矩阵命中 / 矩阵未命中行零 archi 名）
- `injected()`: 成员校验单点（`mount::lookup` 返回 `None` 即 `Err`，crate 私有），不静默滤过（防御面 / AC-1；用例见本节「非法名 `Err`」）
- `assemble_injections.finish_slot_decoupled()`: `finish` 位与矩阵解耦——决策会话恒不命中矩阵但 `finish` 位仍在位（边界 / AC-4、D5；用例：`finish` 位与矩阵解耦）
- `change_phase_role()`: 段数 / 末段 `u32` / 畸形拒逐条零改（回归锚定）

<!-- design.md 未声明该文件的公共函数 / API 变更（该表零行）：改动面是 `assemble_injections` 三臂改产名字清单 + 独立 `finish` 位、`archi_matrix` 返回值由枚举改名字切片——两者均为 crate 私有函数，属 design「修改文件」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-1（三臂产名字清单、非法名在装配层 `Err`）与 AC-4（`finish` 位按 role 置位）、design D5（`finish` 位与矩阵未命中行**解耦**）组织。既有 `compose_test` 的 8 处 `SessionToolset` 读侧断言与 21 处读侧断言换位为名字清单；解耦判据是本变更新增的关键用例（决策会话恒不命中矩阵）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| explore 臂名字清单（AC-1） | 正向 | `source == "explore"` → `tools == Some(["read","grep","glob","ls","edit","bash","create_explore_note"])`（六名 + 专用名，序逐值）；`finish == None`；`preamble == None`；零 `source_ref` 读取（畸形 `source_ref` 下断言不变） | 单例（4 断言） |
| archi 臂名字清单（AC-1） | 正向 | `source == "archi"` → `tools == Some([七内建 + archi_module + archi_search])`（九名）；`finish == None`；`preamble` 为知识文本（拼装失败时 `None` 但工具面不变——既有断言保留） | 单例（3 断言） |
| change 臂矩阵命中（AC-1） | 正向 | 7 行矩阵逐行断言：`(proposal,executor)` → 七名 + `archi_search`；`(dev-design,executor)` / `(dev-design,evaluator)` → 七名 + 两 archi；`(test-design,executor)` → `archi_search`；`(implement,executor)` → `archi_module`；`(code-review,evaluator)` / `(acceptance,evaluator)` → `archi_module`（矩阵值改名字清单后逐格与改造前子集对应） | 表驱动（7 行） |
| `finish` 位按 role 置位（AC-4 / D5） | 正向 | `role == "evaluator"` → `finish == Some(EvaluatorChecklist)`；`role == "decision"` → `Some(Decision)`；`role == "executor"` → `None`（6 份 executor md 无 `finish_step` 字样的一致性面） | 表驱动（3 形态） |
| `finish` 位与矩阵解耦（AC-4 / D5 关键判据） | 边界 | `source_ref = "<id>/<fail_phase>/decision/<attempt>"`（`(phase, "decision")` **恒不命中** 7 行矩阵）→ `finish == Some(Decision)` **且在位**、`tools == None`（七内建）——`finish` 位 MUST NOT 随矩阵 `None` 一并丢失（否则决策会话拿不到 `finish_step`） | 单例（2 断言 + 对照） |
| 矩阵未命中行零 archi 名（AC-1） | 边界 | `(phase, role)` 未命中（如 `(archive, evaluator)` / `(proposal, evaluator)` / 畸形 `source_ref`）→ `tools == None` 且零 archi 名；`source` 非三者（`None` / 未知串）→ `SessionInjections::default()`（三字段全 `None`）；`source_ref` 短段 / 末段非数字 → 同上 | 表驱动（4 形态） |
| 非法名 `Err`（AC-1） | 异常 | 以可控注入面构造非法名（如矩阵行值被人为替换为清单外名，或 explore 六名清单含非法名）→ `assemble_injections` 返回 `Err(msg)`，`msg` 含非法名与来源（`mount::lookup` 返回 `None` 即判）；`Err` 面 MUST NOT 返回部分注入面 | 表驱动（2 形态） |
| `change_phase_role` 解析不变（回归锚定） | 边界 | 既有用例（段数 ≥ 4 / 末段可解析 `u32` / 空段拒 / 畸形拒）逐条保留零改；`archi_matrix` 返回值形态换为 `Option<Vec<&'static str>>` 后，源侧断言的「子集内容」换位表达、判据本身不变 | 表驱动（既有全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | **不 mock**：`assemble_injections` 为纯函数（入参 `SessionProvenance` 内存构造 + 返回值断言）；`archi_knowledge::knowledge()` 真实调用（知识文本非被测重点，既有断言只判在场/缺席）；`mount::lookup` 真实调用（成员校验单点即被测面）；零 IO / 网络 / DB / 子进程 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/worker.rs -> packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs

#### 待测功能

- `execute()`: `WorkerTurnOutcome { …, finalized: orchestration::transcript::finish_payload(&transcript) }` 单点填充（crate 私有）——收口转录 → `Some` 逐字段相等，无收口 / 异常收敛 / 中止 → `None`（行为面 / AC-6；用例：`finalized` 提取贯通 / 无收口 → `None` / 损坏载荷 → `None` / 提取实现单点）
- `execute.payload_tolerance()`: `finish_step` notice 的 payload 为 `null` / `{}` → `finalized == None` 且 `Err` 面不外溢（异常面 / AC-6；用例：损坏载荷 → `None`）
- `execute.extraction_single_point()`: 提取实现单点——`finish_payload` 调用恰 1 次、`from_value::<FinishPayload>` 零命中（结构自证 / AC-6；用例：提取实现单点）

<!-- design.md 未声明该文件的公共 API 变更：改动是 `WorkerTurnOutcome { …, finalized: orchestration::transcript::finish_payload(&transcript) }`（一处字段填充），属 design「修改文件」表行，非「公共函数 / API」表行。下列条目按该文件的**类型定义面 / 行为面 / 结构自证 / 回归锚定面**逐条登记——条目源为同节 `#### 用例` 表的测试对象（不新增用例、不改断言与 AC 映射），故与该表未登记本文件并不矛盾 -->

> 说明：用例按 AC-6（`finalized` 由 `worker.rs` 填充，且提取实现单点在 `transcript`）组织。既有 `worker_test` 的 `DualEnv` 真库全链用例（假 sink + `KernelWorkerPort`）**零改**，本节只新增 `finalized` 贯通断言。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `finalized` 提取贯通（AC-6） | 正向 | 会话脚本收口于 `finish_step`（notice 进转录）→ `WorkerTurnOutcome.finalized == Some(FinishPayload::Evaluator(..))`，逐字段与载荷相等（`verdict` / `report` / `checklist`）；同轮 `final_message` 仍为末条 assistant 文本（两字段互不推导） | 单例（3 断言） |
| 无收口 → `None`（AC-6 / Optional 边界） | 边界 | 会话正常收敛但零 `finish_step` notice → `finalized == None`；会话异常收敛（`api_error` 路径）→ `finalized == None`；中止（`Stopped`）→ `None` | 表驱动（3 形态） |
| 损坏载荷 → `None`（AC-6） | 异常 | 转录内 `finish_step` notice 的 payload 被注为损坏形态（`null` / `{}`）→ `finalized == None` 且 worker 不报错（`Err` 面不外溢——`transcript::finish_payload` 的宽容已保证） | 表驱动（2 形态） |
| 提取实现单点（AC-6 / 结构自证） | 异常 | 源码自证：`worker.rs` 内 `finish_payload` 调用恰 1 次、`from_value::<FinishPayload>` 零命中（MUST NOT 另写提取）；`transcript` 与 `finalized` 的借用顺序正确（`finalized` 先算、`transcript` 后移——编译期由 `.clone()` / 顺序表达，读码复核） | 单例（源码自证 2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 provider | 既有假 provider 记录（`DualEnv` 真库 + 假 `SessionStreamRegistry`）沿用——真件会起真实 provider 会话（网络 / 进程边界） | 全部用例 |
| 数据根 / workspace 根 | 既有 `tempfile::TempDir` 双临时目录（真件，`WorkspaceStores` 以 `OnceLock` 缓存——redb 单进程单句柄） | 全部用例 |
| `RunEventSink` | 既有假 sink（双面捕获）；它是 `KernelWorkerPort::new` 的显式注入依赖 | 全部用例 |

---

### packages/desktop/src-tauri/src/bindings/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

- `export_bindings()`: 产物面本变更**零改动**（`FinishSpec` / `FinishPayload` / `SessionInjections` 全不出线），产物确定性可逐字 diff（正向 / AC-3；用例：`引擎侧词汇不出线` / `产物面稳定`）
- `export_bindings.reverse_anchor()`: 反向锚定清单——sidecar 的引擎侧词汇清单由 `["SessionInjections","SessionToolset","TurnQuestion"]` 换为 `["SessionInjections","FinishSpec","TurnQuestion"]`，三词在产物内逐个零命中，对照臂 `export type ArchiModuleDetail =` 在场（判据面 / AC-3；用例：`引擎侧词汇不出线` / `锚定换位自证`）
- `export_bindings.anchor_self_check()`: 锚定换位自证——`mod_test.rs` 内 `SessionToolset` 零命中、`FinishSpec` 在锚定数组内恰 1 次、注释段同句已随动（结构自证 / AC-3；用例：锚定换位自证）
- `export_bindings.leak_detection()`: 误出线即红——三词任一被误接进出线面即断言失败，对照臂保证「零命中」非空产物误判（判据力 / AC-3；用例见本节「误出线即红（AC-3 / 判据力）」）

<!-- design.md 未声明该源文件的公共 API 变更：`src/bindings/mod.rs`（生成物导出）**零改动**；本变更加载点是其 sidecar 的反向锚定**字面量清单**换位（`SessionToolset` → `FinishSpec`），属 design「编译闭合批」表的壳层锚定行与 AC-3 的判据面。 -->

> 说明：用例按 AC-3 的「引擎侧词汇不出线」判据组织。既有 `引擎侧会话注入词汇不出线ipc产物` 用例的锚定清单由 `["SessionInjections","SessionToolset","TurnQuestion"]` 换为 `["SessionInjections","FinishSpec","TurnQuestion"]`——**不新增断言、不改断言语义**（design 事实校准 8：该字面量不被编译器强制，漏改不会红，故必须在实现相位同批改）。旁注：本仓 `src/` 下该 sidecar 与设计文档中 `crates/core/orchestration/src/port.rs` 的 sidecar **同名不同物**（`port.rs -> port_test.rs` 与 `src/bindings/mod.rs -> mod_test.rs` 是两处独立文件），本设计分别设节。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 引擎侧词汇不出线（AC-3） | 正向 | 产物快照文本内三词（`SessionInjections` / `FinishSpec` / `TurnQuestion`）逐个零命中；对照臂：`export type ArchiModuleDetail =` 在场（非空产物误判为「零命中」的对照） | 单例（4 断言） |
| 锚定换位自证（AC-3 / 判据力） | 边界 | 源码自证：`mod_test.rs` 内 `SessionToolset` 零命中（旧词面残留即判据失效——全仓零命中判据的一部分）；`FinishSpec` 在锚定数组内恰 1 次；注释段 `:412` / `:421` 同句已随动（零 `SessionToolset`） | 单例（源码自证 3 断言） |
| 产物面稳定（AC-3） | 边界 | 同一世代连续两次 `authoritative_snapshot()` 字节逐字相等（产物是确定性的纯投影，可逐字 diff——`api:check`/`bindings:export` 零 diff 门的前提）；本变更对该产物**零改动**（`FinishSpec` / `FinishPayload` / `SessionInjections` 全不出线） | 单例（2 断言） |
| 误出线即红（AC-3 / 判据力） | 异常 | 若引擎侧词汇被误接进出线面，产物文本将出现该类型名 → 「零命中」断言即刻失败；判据以既有同款形态表达（三词逐个 `!content.contains`，任一命中即红，非新增机制）；对照臂（`export type ArchiModuleDetail =` 在场）保证「零命中」不是空产物误判 | 单例（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件 | **不 mock**：经既有 `authoritative_snapshot()` 装置读在册产物（进程边界=数据文件；装置本身内调既有 `export_bindings()` 取权威字节，属既有形态、零新增替身）。**不在本节运行 `pnpm run check` / `bindings:export` 命令链**——那是 static-check 相位的交付门，本节只给产物内容判据 | 全部用例 |
| 互斥锁 | 既有 `lock()`（串行化快照读，沿既有装置） | 全部用例 |

---

### packages/desktop/src/components/agent/agent-blocks.tsx -> packages/desktop/src/components/agent/agent-blocks.test.tsx

#### 待测功能

- `FinishStepCard()`: `FinishStepCard({ toolCallId, toolName, input, output }): React.JSX.Element`——收口工具专化卡：evaluator 形态（派生 verdict 摘要 + `items` 逐项 ID / pass 标记 / evidence + 可折叠 `report`）、decision 形态（`action` 摘要：backtrack 目标 / stop / ask 问题与选项）、非预期 `input` 回退 JSON 原样（公共函数 / API）
- `TextBlock()`: memo 比较器面（私有 `sameCitations()`）——`citations` 按 `url` / `title` / `citedText` 三字段逐项判等、长度先判、同引用短路；判不等时引用列表随新值更新（本 change 整文件突变面的既有分支）
- `ThinkingBlock()`: `state` 两态——`'streaming'` → 标题「思考中…」+ `TraceLine` 的 `data-status="pending"`；`'done'` / 缺席 → 标题「思考」+ 无 `data-status`（同上）
- `WebSearchCard()`: 查询词与来源面（私有 `searchQuery()` / `dedupeByUrl()`）——查询词空值即省略、正文空值不落假串、`search.error` 空串即无错误态、`results` 缺席 / 空数组零来源容器、无 `url` 条目不去重（同上）
- `CitationList()`: 四级回退链 `title ?? url ?? citedText ?? ''`（同上）
- `ExternalLink()`: 仅**行首** `http:` / `https:`（大小写不敏感）渲 `<a>` 并经壳侧 opener 打开；其余 scheme、前导串、相对路径与非字符串一律降级纯文本（同上）
- `isToolPart()`: `tool-*` 前缀收窄命中，`dynamic-tool` / `text` / `data-*` / `step-start` 不命中（同上）

<!-- design.md 未声明本文件其余条目为公共 API 变更：「公共函数 / API」表只登记 `FinishStepCard` 一行（新增）。上面六条既有面是本 change 的**整文件突变面**补充——`agent-blocks.tsx` 与 `tool-cards.tsx` 同属本 change 的整文件编辑集，既有分支与新增组件同在被测面，故按既有导出与私有件逐分支补足；不为测试新增任何导出（私有件经公开组件间接驱动）。 -->

> 说明：用例按 AC-13（`FinishStepCard` 两形态 + JSON 回退）与 design「修改文件」表行（卡根 `data-testid="block-tool-use"` + `data-tool-id`、摘要区 `data-testid="block-finish-step-summary"`、结果正文 `data-testid="block-tool-result"`）组织；既有面用例按整文件突变面的未覆盖 / 未杀死分支补齐（`ThinkingBlock` state 两态、`TextBlock` 比较器、`WebSearchCard` 空载荷、`ExternalLink` scheme、`CitationList` 回退链）。宽容解析原语 `lib/parse` 的 `asRecord` / `textOf` 真实调用（沿 `AskUserQuestionCard` 先例）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `FinishStepCard` evaluator 形态渲染（AC-13） | 正向 | `input = { report: "评估报告正文", items: [{id:"C1",pass:true,evidence:"甲"}, {id:"C2",pass:false,evidence:"乙"}] }` → 卡根 `block-tool-use` 在场且 `data-tool-id == "tu_finish"`；摘要区 `block-finish-step-summary` 文本含「含未通过项」（派生面：任一 `pass === false`）；两项 ID 与 evidence 各自在场；`report` 文本在折叠区内可见（点击 `TraceCollapsibleTrigger` 后放大验证） | 单例（6 断言） |
| `FinishStepCard` evaluator 全通过摘要（AC-13） | 正向 | 两项全 `pass: true` → 摘要含「清单全通过」语义；零 `pass === false` 项时摘要不出现「含未通过项」（两摘要文案互斥） | 表驱动（2 形态） |
| `FinishStepCard` decision 三动作渲染（AC-13） | 正向 | `backtrack` → 摘要**逐字**为 `backtrack → test-gen：覆盖不足`；`stop` → 逐字 `stop：受控终止`；`ask`（`question: "选哪个？"`、`options: ["甲方案","乙方案"]`）→ 逐字 `ask：选哪个？（甲方案 / 乙方案）`（分隔符 ` / ` 与括号段逐字判据）；未知 `action`（`"retry"`）→ 摘要逐字为 `retry` 且不抛错 | 表驱动（4 形态） |
| `FinishStepCard` `input` 非预期形态回退（AC-13） | 异常 | `input` 为 `null` / `"裸字符串"` / `[]`（数组）/ `{}` / `{ items: "str" }`（items 非数组）/ `{ action: 42 }`（action 非字符串）→ 摘要区文本**逐字为 `finish_step`**、展开后折叠区内 `<pre>` 文本为 `JSON.stringify(input, null, 2)` 原样、`render` **不抛错**（沿 `AskUserQuestionCard` 宽容先例）；对照组：`{ action: "stop", reason: "受控终止" }` → 摘要为 `stop：受控终止`、折叠区内**无** `<pre>`（decision 分支不落回退面板） | 表驱动（6 形态 + 1 对照） |
| `FinishStepCard` 逐项行结构与空位（dict / str / bool 边界） | 边界 | 逐项行 = 「ID span + pass 标记 span」+ **可选** evidence span（`querySelectorAll('span')` 计数）：① 项缺 `id` → ID span 文本为空串（不含任何假串，计数 3）；② 项缺 `evidence` / `evidence: null` → **不渲** evidence span（计数 2）；③ `evidence: ""`（空串 ≠ null）→ span 在场而文本为空（计数 3，判据是 `!== null` 而非真值）；④ `pass` 为 `"true"`（字符串）/ `1` / 缺失 → 均呈「未通过」（`record.pass === true` 严格判据，计数 2） | 表驱动（5 形态） |
| `FinishStepCard` pass 标记着色与 report 在场判据（AC-13） | 边界 | `pass: true` 行的标记 span 命中 `.text-pass`、`pass: false` 行命中 `.text-fail`（两行同屏各一，文本分别为「通过」/「未通过」——着色钩子是 pass/fail 唯一的 DOM 可观测面，故按类名断言；两类同时也唯一锚定 `text-pass` / `text-fail` 字面）；`report` 在场 → 折叠区出现该文本；`report: ""` → report 容器**在场**而文本为空（判据是 `!== null` 而非真值：`.whitespace-pre-wrap` 计数 1、文本空）；`report` 缺失 / 非字符串（`42`）→ 该容器**缺席**（`.whitespace-pre-wrap` 计数 0） | 表驱动（4 形态） |
| `FinishStepCard` decision 摘要缺字段（dict 边界） | 边界 | `{ action: "backtrack" }`（缺 `to` / `reason`）→ 摘要**逐字** `backtrack → ：`（两空位均不落假串）；`{ action: "stop" }` → 逐字 `stop：`；`{ action: "ask", question: 42, options: ["甲"] }` → 逐字 `ask：（甲）`（题干段空、括号段仍在）；`{ action: "ask", question: "选哪个？", options: "非数组" }` 与 `{ action: "ask", question: "选哪个？", options: [] }` → 均无括号段（逐字 `ask：选哪个？`，非数组与空数组同义）；`{ action: "ask", question: "q", options: ["甲", 42, "乙"] }` → 括号段逐字 `甲 /  / 乙`（非字符串元素落空串、`join(' / ')` 保序） | 表驱动（6 形态） |
| `FinishStepCard` `items` 边界（list 边界） | 边界 | `items: []` → 摘要**逐字**「清单全通过」（`every` 于空数组为真）、零逐项行、不抛错；单元素 → 一行；100 项 → 逐项行数 == 100（不截断）；`evidence` 超长（5000 字符）→ 不抛错且该行 evidence span 文本长度 5000；`items: null` / `items: "str"`（非数组）→ 落 JSON 回退（`items === null` 分支） | 表驱动（5 形态） |
| `FinishStepCard` 文本特殊字符（str 边界） | 边界 | `evidence` / `report` / `question` 含 `\n` / `\0` / emoji → 渲染不抛错且文本可见（React 转义，非 `dangerouslySetInnerHTML`：`querySelector('script')` 为 null）；`id` 含 emoji 时逐项行仍配对 | 表驱动（3 形态） |
| `FinishStepCard` 结果与错误形态（AC-13） | 边界 | `output = { content: "已收口", isError: false }` → 展开后 `block-tool-result` 文本为「已收口」、`[data-status]` 缺席；`output = { content: "report 超长…", isError: true }` → `[data-status == "error"]` 在场且结果区文本仍为该 content（错误不替换正文）；`output = null` → 展开后 `block-tool-result` **零命中**、卡不抛错 | 表驱动（3 形态） |
| `TextBlock` 比较器：三字段任一变化即放过（既有分支） | 正向 | `text` 不变、`citations` 换新数组且**仅** `url` 变（`title` / `citedText` 逐字相同）→ 引用列表文本更新为新 `url`；同形用例只改 `title` → 列表文本更新为新 title；只改 `citedText`（另两字段同）→ 列表文本更新为该 citedText（三字段各自单点判据；漏比其中任一字段则列表停在旧值） | 表驱动（3 形态） |
| `TextBlock` 比较器：部分相等（既有分支 / 逐项全等的必需判据） | 正向 | 两条引用，**首条逐字相同、次条仅 `title` 变**（`url` / `citedText` 同）→ 列表次条文本更新为新 title（`every` 语义下首条相等不足以判等；若比较退化为「存在相等项即判等」，列表会停在旧 title） | 单例（2 断言） |
| `TextBlock` 比较器：长度差与同引用（既有分支） | 边界 | 1 项 → 2 项（长度变化、首项逐字相同）→ 列表渲染 2 条（长度先判分支：漏判即按首项相等误认为未变、停在 1 条）；`citations` 传入**同一数组引用**且 `text` 不变 → 渲染不抛错、列表条数与文本保持原样（同引用短路分支）；`citations` 换新数组但三项逐字全同 → 列表与文案不变（逐项全等分支，与「有变化」对偶） | 表驱动（3 形态） |
| `TextBlock` `citations` 缺席与空数组互换（Optional 边界） | 边界 | `citations` 由 `undefined` → `[]`（`text` 不变）→ 不抛错、无 `block-citations` 容器；反向 `[]` → `undefined` 同断言；两形态之间再夹「有引用」对照各渲染一次（正反向不把非空清单吞掉）。**口径更正**：本行只覆盖「`?? []` 不被求值」的两向（`undefined` 与 `[]` 互替），**不足以判死 `?? []` 两枚突变体**——`[]` 做右臂时长度同为 0，DOM 无差异。判死面在下一行（三字段全 undefined 引用 ↔ `undefined`）| 表驱动（3 形态） |
| `TextBlock` `undefined` ↔ 三字段全 undefined 引用互换（Optional 边界 / 判死 `?? []` 两枚） | 边界 | `citations` 由 `undefined` → `[{ url: undefined, title: undefined, citedText: undefined, raw: null }]`（类型合法；四个可选字段全缺，等同于线上 `{}` 引用）→ `block-citations` 容器出现恰 1 个、条目文本为空串；反向同值 → 容器消失。判据取「容器与条目数随值变化」：`left ?? []` 被替换成哨兵单元素数组时长度先判错位（首向转红），`right ?? []` 同（次向转红）——两枚 `ArrayDeclaration` 突变体各由一向判死 | 单例（3 断言，两向） |
| `CitationList` 四级回退链（既有分支） | 边界 | `{ title: null, url: null, citedText: null }` → 条目文本为**空串**（不落假串）；`title` 缺席有 `url` → 回退 `url`；`url` 与 `title` 皆缺席 → 回退 `citedText`；`title: ""`（空串 ≠ null）→ 呈空串**不**继续回退到 `url`（`??` 判据，非真值判据） | 表驱动（4 形态） |
| `dedupeByUrl` 无 url 条目不去重（既有分支） | 边界 | 两条 `url: null` 且文本相同的引用 → 列表 **2** 条（无 url 条目逐条保留）；一条有 `url` + 一条 `url: null` → 2 条；两条同 `url` → 1 条（去重生效对照）；`search.results` 同 url 的两条来源 → 1 条 | 表驱动（3 形态 + 1 对照） |
| `ThinkingBlock` state 两态（既有分支） | 正向 | `state="streaming"` → 标题文本「思考中…」且 `[data-slot="trace-line"]` 的 `data-status == "pending"`；`state="done"` → 标题「思考」且 `data-status` 属性缺席；`state` 缺席（`undefined`）→ 与 `"done"` 逐字一致（两态文案互斥、无第三态） | 表驱动（3 形态） |
| `ThinkingBlock` 两态旋载图标（判死图标选择分支 4 枚） | 正向 | `state="streaming"` → `[data-slot="trace-icon"]` 内 `[data-slot="spinner"]` **在场**；`state="done"` / `undefined` → spinner **缺席**且图标位仍有一枚 `svg`（落 `DotIcon`）。判据取 `Spinner` 自身产品锚点（`data-slot`），MUST NOT 断言 lucide 内部 class——`L163` 的条件 → true / false、算子 → `!==`、字面量 → `""` 四枚各由一向转红（实测：2 / 1 / 3 / 1 条断言失败） | 表驱动（3 形态） |
| `ToolPairCard` 两态旋载图标（判死图标选择分支 4 枚） | 正向 | `state="output-available"` → spinner **缺席**（落 `ToolboxIcon`）；`state="input-available"` → spinner **在场**；两态图标位均有一枚 `svg`。`output` 恒 `null` 使 `isError` 面不介入；`L192` 四枚同形态各由一向转红（实测：1 / 1 / 2 / 1 条断言失败） | 表驱动（2 形态） |
| `WebSearchCard` 查询词空值面（既有分支） | 边界 | `input.query` 为空串 `""` → 无 `block-web-search-query` 节点且标题栏 `TraceTitle` 文本为**空串**（不落假串）；`query` 为空白串 `"   "` → **照常**渲查询词（仅 `""` / `null` 省略，空白不归一）；`query` 为 `42` / `null`（非字符串）→ 省略 | 表驱动（4 形态） |
| `WebSearchCard` 正文与错误面（既有分支） | 异常 | `output = { content: "来源正文", isError: false }` → 展开后卡文本含该正文（`content` 的 `?? ''` 兜底被判据经过）；`output.content = ""` → 正文为空且不落假串；`search.error = ""`（空串）→ 无 `block-web-search-error` 节点且 `[data-status]` 属性缺席（`error` 为空串即视作无错误）；`search.error = "max_uses_exceeded"` → 错误区文本逐字出现 + `[data-status == "error"]` 在场（空串与非空串两向可辨） | 表驱动（4 形态） |
| `WebSearchCard` 正文位在 `output` 缺席时不落假串（判死 `output?.content ?? ''`） | 边界 | `output = null` + `input = null` → 展开后 `[data-slot=trace-collapsible-content-inner]` 的 `textContent` 为**空串**。`{ content }` 位在折叠区内**恒挂载**（与来源列表 / 错误区的条件挂载不同），故 `?? ''` 的兜底值直接进 DOM——上行的 `content: ""` 分支走的是载荷自带空串，杀不掉该突变体 | 单例（1 断言） |
| `WebSearchCard` 来源区缺席三形态（既有分支） | 边界 | `output` 非 null 但 `search` 缺席 / `search: null` / `search.results: []` → 均无 `block-web-search-sources` 容器、不抛错（三形态一致） | 表驱动（3 形态） |
| `ExternalLink` scheme 锚定（既有分支） | 异常 | `https://a/x` / `HTTPS://A/X`（大小写）→ 渲 `<a href>` 且点击以**原样** url 恰调用一次 `openUrl`；`javascript:alert(1)` / `file:///etc` / `mailto:a@b` / `/relative/path` / `prefixhttps://evil.com`（非行首 scheme）→ **无** `<a>`、点击零 `openUrl` 调用（行首锚定 `^` 判据；`prefixhttps` 是 `^` 唯一的判别输入） | 表驱动（8 形态） |
| `ExternalLink` 非字符串与非 http 边界（str 边界） | 边界 | `url` 为 `null` / `undefined` / `42` / 对象（经 `as unknown` 注入）→ 均降级纯文本、不抛错、零 `openUrl`；`openUrl` resolve 与 reject 两向均无未捕获拒绝（`.catch` 吞掉） | 表驱动（4 形态 + 2 向） |
| `ExternalLink` 非字符串但 String 后命中 scheme（判死 `typeof url === 'string'`） | 边界 | `url` 为数组 `['https://array/1']` → 条目**无** `<a>`、文本回退标题（`/^https?:\/\//i.test(['https://array/1'])` 为真——正则会把非字符串 String 化后匹配，故 `typeof url === 'string'` 是该判据的唯一防线）；同值再作 `it.each` 第 5 形态并入上表 | 单例（2 断言）+ 上表扩位 |
| `isToolPart` 收窄（既有分支） | 边界 | `tool-read` / `tool-finish_step` → `true`；`dynamic-tool` / `text` / `data-raw` / `step-start` / `''`（空 type）→ `false`（既有断言逐字保留；第二合取 `!== 'dynamic-tool'` 为不可达面，见「不可测试项」） | 表驱动（7 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/plugin-opener`（外壳 IPC） | 既有 `vi.mock` 替身 + `vi.hoisted` 提升（沿既有装置，`openUrlMock` 可按用例切 resolve / reject）——进程边界（Tauri IPC） | `ExternalLink` 与 `WebSearchCard` / `CitationList` 的链接族用例 |
| `react-markdown` | **不 mock**：比较器用例的判据取引用列表 DOM（`block-citations` 的 `li` 文本 / 条数），而非解析次数——`TextBlockBase` 的 `useMemo` 元素复用已使 markdown 不随 `citations` 变化重解析，解析次数与比较器判定不相关，为它加 spy 只会引入无用装置 | — |
| 无其它 | **不 mock**：`agent-blocks.tsx` 内部协作件（私有 `CitationList` / `ExternalLink` / `dedupeByUrl` / `sameCitations` / `searchQuery` / `parseFinishItems` / `finishSummary` / `decisionSummary` / `Trace*` / `lib/parse` / `react-markdown`）全部真实组合——不为测试新增导出（沿既有纪律）；`FinishStepCard` 直接以 `render(<FinishStepCard …/>)` 驱动 | 全部新用例 |

---

### packages/desktop/src/components/agent/tool-cards.tsx -> packages/desktop/src/components/agent/tool-cards.test.tsx

#### 待测功能

- `BUILD_IN_TOOL_CARDS.keys()`: 注册表 `Record<string, AgentToolCard>` 增 `finish_step: FinishStepCard` 键（七键，按源码声明序；未注册名 `undefined`，由消费方落 `ToolPairCard`）（类型定义面 / 注册表键面；用例：注册表含 `finish_step` / 未注册回退零变）
- `AskUserQuestionCard()`: 静态问答卡——`questions` 非数组回退 JSON；题干 / 选项 label / description 的宽容解析与省略判据（既有面）
- `BashCard()`: 命令进 CodeBlock 头（标题恒为 `Bash`）、输出折叠——`output` 三态（缺席 / 正常 / `isError`）的 `aria-disabled`、`data-status`、正文文本（既有面）
- `ReadCard()`: 路径 + 行区间进标题（`Read <path><rangeSuffix>`）与 CodeBlock 头——`rangeSuffix` 三形态、`output` 三态、`isError` 时错误提示与正文两处并存（既有面）
- `WriteCard()`: 按路径扩展名着色——`languageOf` 有 / 无扩展名两向（高亮行 vs 纯文本切行，以**行数**区分）、`aria-disabled` / `defaultOpen` 两向、`isError` 分支（既有面）
- `EditCard()`: 旧 / 新两区着色——`removed` / `added` 两 variant、两向 `languageOf`、`aria-disabled` / `defaultOpen`、`isError` 分支、`trace-line` 恒无 `data-status`（既有面）
- `rangeSuffix()`: 私有原语（两参 `offset` / `limit`）——`offset` 非 number → 空串；`limit` 为 number → `:起-止`；`limit` 缺席 → `:起-最后`（既有面）
- `languageOf()`: 取路径扩展名——`null` → `null`；无点路径 → `null`；取**末段**扩展名（多于两段时判据）；尾点路径 → 空串扩展名（既有面）
- `cardAttrs()`: 四张折叠卡共用的卡根钩子 `data-testid='block-tool-use'` + `data-tool-id`（`AskUserQuestion` 为容器形态，不参与）
- `parseAskInput()`: 私有解析族（`parseOptions()` / `AskQuestionView()`）——选项 label / description 的缺席判据与描述段省略（既有面，私有件经公开组件间接驱动）——用例: `AskUserQuestionCard` 宽容解析存活面

<!-- design.md 未声明本文件的公共函数 / API 变更（该表零行）：改动是 `BUILD_IN_TOOL_CARDS` 注册表增一键（类型定义面 / 注册表键面），属 design「类型定义」表行与「修改文件」表行。上面其余条目是本 change **整文件突变面**的既有分支补充——`tool-cards.tsx` 四张内建卡（`Bash` / `Read` / `Write` / `Edit`）在改动前零渲染用例（branch 覆盖 26.66%），与新增键同在整文件被测面，故逐分支补足；不为测试新增任何导出（私有件经注册表取卡渲染间接驱动）。 -->

> 说明：用例按 AC-13（`BUILD_IN_TOOL_CARDS['finish_step']` 在场、未注册回退行为零变）组织，并按整文件突变面补齐四张既有内建卡与三个私有原语的分支判据。注册表键面断言（`Object.keys` 的**顺序数组**断言）与既有 `web_search` / `AskUserQuestion` 族用例随动扩位，其余断言零改。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 注册表含 `finish_step`（AC-13） | 正向 | `BUILD_IN_TOOL_CARDS['finish_step']` 为 `FinishStepCard` 本体（同引用 `toBe`）；键面数组断言随动为七键（新增键位按源码声明序） | 单例（2 断言） |
| 经注册表命中渲染（AC-13） | 正向 | 取卡以 `toolName="finish_step"` + evaluator 形态 `input` 渲染 → `block-finish-step-summary` 在场；decision 形态 `input` 渲染 → `action` 摘要在场（注册表取卡到渲染的链完整，非只判键存在） | 表驱动（2 形态） |
| 未注册回退零变（AC-13 反向锚定） | 边界 | `BUILD_IN_TOOL_CARDS['NoSuchTool']` 仍 `undefined`（消费方落 `ToolPairCard` 的回退分支零改）；键名精确匹配（`'Finish_Step'` / `'finish-step'` / `'FinishStep'` / `' finish_step'` / `''` 全 `undefined`——键面不做归一化，沿既有 `web_search` 断言口径） | 表驱动（5 miss） |
| 卡内渲染异常不外溢（AC-13 / 异常面） | 异常 | 以 `finish_step` 卡渲染非预期 `input`（`null` / `"裸字符串"` / `[]`）→ 经注册表取卡渲染**不抛错**（`render` 不 throw，回退 JSON 原样）；对照：注册表取到 `undefined` 时必须由消费方落 `ToolPairCard`（本文件只锚定 `undefined` 这一契约，不复制消费方逻辑） | 表驱动（3 形态 + 1 对照） |
| 既有六键零回归（AC-13） | 边界 | 既有六键（`AskUserQuestion` / `web_search` / `bash` / `read` / `write` / `edit`）的值仍为原组件本体（同引用）；**六卡逐张经注册表取卡渲染**（`Bash` / `Read` / `Write` / `Edit` 为本次补齐的首个渲染用例）→ 卡根 `block-tool-use` 在场且 `data-tool-id` 逐卡等于入参（注册表 → 卡的装配链逐键打通，非只判引用相等） | 表驱动（6 键 + 6 渲染） |
| `BashCard` 三态（既有分支） | 正向 | `output = null` → 卡根在场、触发按钮 `aria-disabled == "true"`、`[data-slot="trace-line"]` 的 `data-status == "running"`（标题行常驻，**无需**展开）、点按钮后仍无 `block-tool-result`（面板不挂载）；`output = { content: "目录内容", isError: false }` → 按钮 `aria-disabled == "false"`（base-ui 显式落串，非属性缺席）、`data-status` 缺席、展开后 `block-tool-result` 文本为「目录内容」；`output = { content: "命令失败", isError: true }` → 按钮 `aria-disabled == "false"`、`data-status == "error"`、展开后结果区文本仍为该 content（错误结果在正文内呈现，非替换） | 表驱动（3 形态） |
| `BashCard` 两态旋载图标（判死图标选择分支 3 枚） | 正向 | `output = null` → `[data-slot="trace-icon"]` 内 `[data-slot="spinner"]` **在场**；`output = { content: "ok", isError: false }` → spinner **缺席**（落 `TerminalIcon`）且图标位仍有一枚 `svg`。判据取 `Spinner` 自身产品锚点 `data-slot`，MUST NOT 断言 lucide 内部 class——`L132` 的条件 → true / false、算子 → `!==` 三枚各由一向转红（实测：1 / 1 / 2 条断言失败） | 表驱动（2 形态） |
| `BashCard` 命令栏与异常载荷（既有分支） | 边界 | `input = { command: "ls -la" }` + `output` 在场 → **展开后** `code-block-header` 文本为「ls -la」；`input = {}` / `null` / `"裸字符串"`（`asRecord` 收敛为空对象）+ `output` 在场 → 头文本为空串且不抛错；`output = null` → 展开被拦、`code-block-header` 与 `block-tool-result` 均零命中（可选链的 `null` 分支不可达，见「不可测试项」） | 表驱动（4 形态） |
| `ReadCard` 标题与行区间三形态（既有分支 / `rangeSuffix`） | 正向 | `input = { path: "src/a.ts", offset: 10, limit: 5 }` → 标题为 `Read src/a.ts:10-15`；`{ path: "src/a.ts", offset: 10 }`（无 `limit`）→ `Read src/a.ts:10-最后`；`{ path: "src/a.ts" }`（无 `offset`）→ `Read src/a.ts`（无后缀，点号后无区间段） | 表驱动（3 形态） |
| `ReadCard` `rangeSuffix` 非法入参（既有分支 / 边界） | 边界 | `offset: "10"`（字符串）/ `offset: null` / `offset: 0` + `limit: 0` → 前三者：前两形态无后缀；`0 + 0` → `:0-0`（`0` 是合法 number，`typeof` 判据而非真值判据） | 表驱动（3 形态） |
| `ReadCard` 三态与路径缺席（既有分支） | 边界 | `output = null` → 按钮 `aria-disabled == "true"`、`data-status == "running"`、`code-block` 零命中（面板不挂载）；正常（`output` 在场）→ 按钮 `aria-disabled == "false"`、`data-status` 缺席、展开后 `block-tool-result` 文本为 output content 且 `.text-destructive` 计数为 **0**；`isError` → `data-status == "error"`、展开后 `.text-destructive` 容器**恰 1 个**且文本为 content、`block-tool-result` 同样在场（两处并存：错误提示 + 正文）；`input = {}` → 标题文本逐字为 `Read null`（`textOf` 回 `null` 后模板串落 `null` 字面，非空串） | 表驱动（4 形态） |
| `WriteCard` 有扩展名 → 高亮行（既有分支 / `languageOf`） | 正向 | `input = { path: "src/a.ts", content: "x\n" }` + `output = { content: "ok", isError: false }` → `loadHighlighter` 被调用且语言实参为 `'ts'`（私有 `languageOf` 的扩展名实参面）；行文本来自高亮替身（标记行可见）而非原文切行——**尾换行判据**：`"x\n"` 在高亮路径下 **1** 行（hook 削尾换行）、纯文本 `split` 路径下 **2** 行（保留空尾行），行数即可区分两臂；`defaultOpen` 为真 → **未点击**即可见行（展开判据） | 单例（4 断言） |
| `WriteCard` 无扩展名 → 纯文本切行（既有分支 / `languageOf`） | 边界 | `path = "README"`（无点）+ `content = "x\n"` + `output` 在场 → `loadHighlighter` 语言实参为 `''`（`languageOf` 回 `null` → `?? ''`）、行数 **2**（原文 `split` 面，含空尾行）；`path = "a.b.c"` → 语言实参为 `'c'`（**末段**判据，非首段 / 次段）；`path = "a."`（尾点）→ 实参为 `''`（`suffix === path` 兜底）；`path` 缺失（`null`）→ 实参为 `''` 且不抛错 | 表驱动（4 形态） |
| `EditCard` 旧 / 新两区 variant（既有分支） | 正向 | `input = { path: "src/a.ts", old_string: "旧行", new_string: "新行" }` + `output` 在场 → 两枚 `code-block-line`：旧区 `data-variant == "removed"`、新区 `data-variant == "added"`，文本分别为「旧行」/「新行」（variant 是 removed/added 唯一可观测面）；两区各在两枚独立 `code-block-content` 内（旧区先、新区后） | 单例（4 断言） |
| `EditCard` 两向 `languageOf`（既有分支） | 边界 | 有扩展名（`path: "src/a.ts"` + 两个字符串字段）→ `loadHighlighter` **两次**调用、语言实参均 `'ts'`、行文本来自高亮替身；无扩展名（`path: "README"`）→ 实参均 `''` 且行文本为原文切行；`old_string` / `new_string` 双双缺失 → 实参均为 `'ts'`（按 `path` 判）且两区各 1 空行（`data-variant` 分别为 `removed` / `added`） | 表驱动（3 形态） |
| `EditCard` 三态与错误分支（既有分支） | 边界 | `output = null` → 按钮 `aria-disabled == "true"`、零 `code-block-line`；正常 → 按钮 `aria-disabled == "false"`、两区行**未点击即可见**；`isError` → 展开后错误 div 含 content 且零 `code-block-line`（两区均不渲）；**对照**：`trace-line` 在 `EditCard` 中**恒无** `data-status`（源码未传 `status`，`isError` 与正常两态一致——判据是「无 status 属性」而非「有 error」） | 表驱动（3 形态 + 1 对照） |
| `languageOf` 分支表（既有分支 / 私有原语经 Write/Edit 驱动） | 边界 | 语言实参逐值断言（经 `loadHighlighter` 替身的调用实参，四卡中 `write` 一调 / `edit` 两调）：`path` 缺失 → `''`；`"README"` → `''`；`"a.ts"` → `'ts'`；`"x/a.b.c"` → `'c'`；`"a."` → `''`；`".ts"` → `'ts'`；`""` → `''`（七形态覆盖 `path === null` / 无点 / 末段 / 尾点兜底 / 隐藏文件五条分支） | 表驱动（7 形态） |
| `cardAttrs` 卡根同形（既有分支） | 正向 | 四内建卡（`bash` / `read` / `write` / `edit`）分别取卡渲染 → 各卡根 `data-testid == 'block-tool-use'`、`data-tool-id` 等于传入 id、且 `block-tool-use` 恰 **1** 个；`AskUserQuestion` 为容器形态（卡根 `ask-question-card`），**不**带该钩子——作为对照锚定「该钩子只服务四张折叠卡」 | 表驱动（4 卡 + 1 对照） |
| `AskUserQuestionCard` 宽容解析存活面（既有分支） | 边界 | `options: [{ label: 42, description: 42 }]` → 选项文本为**空串**且无描述 span（两字段非字符串各落 `''` / `null`）；`questions: [{ question: 42 }]` → 题干文本为空串；`questions: [{ question: "问", options: "非数组" }]` → 该问题**零** `ask-option` 行（非数组落 `[]`，不落假串）；`options: [{ label: "甲" }]`（description 缺席）→ 选项文本恰为「甲」 | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@/lib/highlighter`（`loadHighlighter`） | `vi.hoisted` + `vi.mock('@/lib/highlighter', …)` 注入替身：返回 `{ highlight(code, state) }` 产出可断言行，并记录**语言实参**；可按用例切 `mockResolvedValue(null)`（未登记语言 → 纯文本回退）——进程边界＝shiki 语法包的动态 `import` 与重型 grammar 装载（既有 `hooks/use-highlighted-code.test.ts` 同款） | `WriteCard` / `EditCard` 的「语言实参」与「高亮行 vs 纯文本切行」判据；`languageOf` 分支表（经两卡驱动）；`Bash` / `Read` / `AskUserQuestion` / `finish_step` 卡不调该 hook，替身对其无影响 |
| `@tauri-apps/plugin-opener` | **不 mock**：既有文件零 `vi.mock`，新增用例亦不引入；本文件所有用例均**不点击**外链（`bash` / `read` / `write` / `edit` 四卡与 `finish_step` 卡都不渲 `<a>`），故真模块只被加载不被调用 | — |
| 无其它 | **不 mock**：注册表、四张既有卡、`agent-blocks` 的 `FinishStepCard` / `WebSearchCard`、`lib/parse` 真实组合；`Trace*` / `CodeBlock` / `Collapsible` 真实渲染（jsdom + `src/test-setup.ts` 的 `BASE_UI_ANIMATIONS_DISABLED` 同步收尾） | 全部用例 |

---

## 不可测试项

| 项 | 位置 | 原因 |
|---|---|---|
| `SessionInjections` / `WorkerTurnOutcome` 的**机械字段补位**（27 + 5 处字面构造） | `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`(1)、`packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`(1)、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`(6)、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`(3)、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`(9)、`packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`(3)、`packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs`(1) 等（另 7 处 `-> SessionInjections` 返回型声明随字段面不变） | **纯编译闭合**（漏补即 `E0063`），无新增行为面——断言口径 MUST NOT 变。设计上不设独立用例：其正确性由各所属源文件章节的行为用例间接证明（构造若不合法，那些用例无法编译或无法通过）。design「编译闭合批」明列其性质为「机械适配」，非测试编写 |
| `definitions()` 的 16 个调用点补 `prompt` 实参 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/{tools_test,loop_test,runner_test}.rs`（15 测试点）+ `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（1 生产点） | 同上：签名收敛的编译前置，属「实参补齐（元数）」形态。语义面（`prompt` 如何影响 `enum`）由 `sdk/mount.rs` / `finish.rs` 章节承载 |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs` | 再导出面 | 纯声明（`pub mod checklist;` + 三条 `pub use`）——零行为。可达性由 `checklist_test` / `domain_test` / `orchestration::finish` 的导入面编译期证明；无独立可写用例（若再导出缺失，那些 sidecar 直接编译失败） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs` | 模块声明与再导出面 | 同上（`pub mod finish;` + `pub use finish::{…}`）。`FinishPayload` / `description` / `parameters` / `validate` 的行为面已分别落在 `finish.rs` / `walker.rs` / `sdk/loop.rs` 章节 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 模块声明 | 同上（`pub(crate) mod mount;` + `#[cfg(test)] mod mount_test;`）。漏声明即编译失败，零独立行为 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` 的**注释文案**改写（2 处 `SessionToolset` → 「infra 挂载清单」） | 同上文件 | 注释文案零行为面（proposal 明列「断言本体零改」）。改写正确性无法用断言表达；其是否遗漏由 AC-3 的「`SessionToolset` 全仓零命中」`grep` 判据覆盖（由 static-check 相位执行） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs::feedback_limit` 的**调用点删除**（`feedback_limit(step, "verdict-gate")`） | 同上文件 | 删除本身零行为面；判据已表达为「源码自证 3 断言」中的调用点计数（`"static-check"` / `"archi-check"` 各 1、`"verdict-gate"` 0），落在 `walker.rs` 章节的「死面退役」用例 |
| `packages/desktop/package.json` 的 `version` 递增 | 同文件 | **配置面**（版本纪律）。判据是相对 `HEAD` 的 +1 patch（`git diff HEAD -- packages/desktop/package.json` 的版本行），属交付门由 static-check 相位核对；写死字面号会与「落地时 HEAD 现值」冲突，故不出现在任何测试设计用例里 |
| `packages/desktop/src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` | 同文件 | 零改动面（前者经 `"version": "../package.json"` 跟随、后者 crate 版本行 MUST NOT 跟）。无待测面 |
| `packages/desktop/src/types/generated/**` | 生成物 | 不手改面；期望零 diff。若 `bindings:export` 再生成产生 diff 即说明 `FinishSpec` / `FinishPayload` / `SessionInjections` 误出线——判据已落在 `src/bindings/mod_test.rs` 章节的反向锚定用例（产物快照逐字相等），不另立用例 |
| `packages/desktop/src/lib/{agent-adapter.ts,agent-transport.ts}` / `src/hooks/use-agent-session.ts` / `src/components/agent/agent-messages.tsx` | 同目录 | **零改动面**（design「不要修改」表）。`finish_step` 走既有工具部件管线；无被测变更即无用例。若出现 diff 即实现越界，属 static-check 的 `git diff` 边界比对，非测试用例面 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 与 `tools.rs::input_path` | 同目录 / 同文件 | **零改动面**（D9 的机械判据：出现 diff 即实现偏离）。其既有用例（保护闸 + 沙箱）在 `tools_test` 章节以「逐条保留零改」锚定；「零 diff」本身不是断言而是 `git diff` 复核项 |
| 8 份 `*-evaluator.md` 的**逐字文本快照**（表体三列整份内联） | `packages/desktop/src-tauri/crates/core/workflow/src/prompts/**` | 已在 `phase_table_test` 章节以「项数 + ID 序列 + 集合相等 + 表结构 + 输出面单句」五层判据覆盖；**整份 65 行表体的逐字内联**不做（会与 `include_str!` 语料形成第二份真源，表体小改即双处改动，违反单源纪律）。表体零改动的判据表达为「`git diff` 面对 8 份 md 只余输出面单句一行」的结构复核项 |
| `FinishStepCard` 的 CSS / 视觉层表现（间距、字号、折叠动画） | `packages/desktop/src/components/agent/agent-blocks.tsx` | 无断言面（样式类名断言脆弱且无判据力）。行为面（可见性、折叠展开后文本在场、`data-testid` 钩子）已由该章节用例覆盖。**例外**：`text-pass` / `text-fail` 两个字面串**不在**此列——它们是 pass/fail 在 DOM 上的唯一可观测址，故按类名断言，两枚 `StringLiteral` 突变体由「pass 标记着色」用例判死 |
| `agent-blocks.tsx` 的**等价突变体**（11 枚 = 10 Survived + 1 NoCoverage；`id` 为本次报告实测值——`id` 随文件内容重排漂移，**位置是稳定锚**） | `packages/desktop/src/components/agent/agent-blocks.tsx` | 口径按 `reports/test/packages_desktop_vite-plus/mutation/mutation-seg-0.json` **逐条实测**复核（该文件 241 枚、2 枚 Ignored；存活 10 + 零覆盖 1）。**等价＝在任何输入下产出同一 DOM，无可观测差异**，且已用「按报告位置注入 → 跑 `agent-blocks.test.tsx` 全量」逐枚验过 survived：① `isToolPart` 的 `part.type !== 'dynamic-tool'` 整串 `→ true`（#7，`L27:43`）与 `'dynamic-tool' → ""`（#9，`L27:57`）——`isToolPart` 先判 `startsWith('tool-')`，`'dynamic-tool'` 与 `''` 均不以 `'tool-'` 开头，第二合取恒短路、两臂结果同值；② `sameCitations` 的 `left === right → false`（#42，`L109:7-21`）与 `return true → false`（#44，`L109:30`）——异引用逐项相等时仍走到 `every` 得真值，同引用时返 `true`/`false` 只影响是否重渲染、渲染结果同值；③ 同函数体 `{}`（#40，`L108:82`）、`a.every(…) → {}`（#54，`L113:35`）、逐字段整串 `→ false`（#56，`L116:7`）——返回 `undefined`/`false` 一律落「判不等」分支＝恒重渲染，DOM 同值；④ `a.length !== b.length → true`（#49，`L112:7`）——恒判不等同上；⑤ `TextBlock` 的 memo 比较器整体 `→ false`（#81，`L148:19`）——同上（用例实测 9 条断言仍绿）；⑥ `searchQuery` 的 `query === null → false`（#117，`L218:10`）——`textOf` 返回的 `null` 在两臂都原样返回，`\|\|` 的右侧 `query === ''` 兜住，输出同值；⑦ `decisionSummary` 的 `textOf(record.action) ?? ''` 右臂 `→ "Stryker was here!"`（#175，`L319:43`，报告里该枚为 `NoCoverage`）——`??` 右臂仅在 `record.action` 非字符串时求值，而 `decisionSummary` 的唯一调用点被外层 `action !== null`（`textOf` 结果为 `null` 即分流到 JSON 回退分支）守死，故右臂在本产品面**不可达**；`decisionSummary` 未导出、无第二调用点。**判死登记（本轮新增 15 条，均经「逐对注入突变 → 用例转红」实测）**：`#14` 由「数组 url 仍降级纯文本」；`#46` / `#48` 由「`undefined` ↔ 三字段全 undefined 引用互换」；`#117`（旧 id）由「`output` 为 null 展开后正文位零文本」；**图标选择分支 11 枚**——`L163` 四枚（`state === 'streaming' ? <Spinner/> : <DotIcon/>` 的条件 → true / false、算子 → `!==`、字面量 → `""`）、`L192` 四枚（`state === 'output-available' ? <ToolboxIcon/> : <Spinner/>` 同四形态）由「ThinkingBlock / ToolPairCard 两态旋载图标在场性」判死，`tool-cards.tsx` `L132` 三枚（`output === null ? <Spinner/> : <TerminalIcon/>` 的条件 → true / false、算子取反）由「BashCard 两态旋载图标在场性」判死（判据取 `Spinner` 自身产品锚点 `data-slot="spinner"`，不依赖 lucide 内部 class）。 |
| `tool-cards.tsx` 的**等价突变体**（3 枚，全部实测 survived 且 DOM 同值；`id` 为本次报告实测值） | `packages/desktop/src/components/agent/tool-cards.tsx` | 该文件 133 枚、1 枚 Ignored，未杀死集合恰 3 枚：① `languageOf` 的 `suffix === undefined → false`（#325，`L186:10`）——`String.prototype.split` 恒返至少 1 项，`at(-1)` 永不为 `undefined`，该合取恒假（不可达面，同 `isToolPart` 第二合取的形态）；② / ③ `EditCard` 两区 `key` 模板串 `→ \`\`` （#370 `removed-${index}` `L261:37`、#372 `added-${index}` `L268:37`）——`key` 不进 DOM，替换后 React 每拍重建该区子节点，**输出同值**（实测：多轮长度增减 + 同位置内容替换 + 等长全替换四个回合的行数与行文本逐向相等，仅有 `Encountered two children with the same key` 控制台警告）。 |
| `ToolTier::Protocol` 在 debug / explore 会话下是否触发（proposal 未决项 1） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` / `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | 本期恒为恒定放行（`Protocol` / `Read` 两档一律 `true`），**不存在可观测的差异面**：debug / explore 不挂 `finish_step`，该档位在生产面不可达。12 格矩阵已含 `Protocol` 行（对齐判据），无额外用例可写 |
