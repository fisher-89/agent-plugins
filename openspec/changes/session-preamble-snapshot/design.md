# 设计: session-preamble-snapshot

> **变更**: session-preamble-snapshot（会话系统提示词快照 · 首轮拍存 + 续轮读库 + 前端只读可见）
> **状态**: 设计定稿（基于已定稿 `proposal.md` / `explore.md` 与五份 delta spec：`desktop-agent-execution` / `desktop-workspace-store` / `desktop-agent-chat-infra` / `desktop-agent-db-tools` / `desktop-change-chat`）
> **基于**: `proposal.md`（三项需求 + 四项拍板 + AC-1…AC-13 + 风险表）、`explore.md`（§1 现状 / §3 缓存敌手清单 / §24–§27 生效方式 / §30 最终方案速览）
> **性质**: **一次语义反转 + 一个新持久模型 + 一处出线扩面 + 一处前端只读面**——preamble 从「每轮现算」改为「首轮快照 + 续轮读库」，落点全在 infra（`sdk/preamble.rs` 合成、`compose.rs` 拍存与读回、`store` 新表）；`crates/core/agent` 仅扩一个出线 DTO 字段（+1 具名 struct），`sdk/loop.rs` 由二元合成收敛为一元消费，`src-tauri/src/commands/**` 与 `all_commands!` **零 diff**（无新命令、无新写面）。
>
> **本篇边界**: `crates/core/agent`（DTO 与注入位 doc）、`crates/infra/store`（模型 / 注册 / 两方法 / 单查装配）、`crates/infra/agent/src/{compose.rs,sdk/preamble.rs,sdk/loop.rs}`、前端三文件 + 生成物、一份契约资产 prose、`package.json` 版本行。测试用例与断言随动归测试相位（见末节「交接清单」），本篇不写测试策略、不执行静态检查。

---

## 提案与规格同步状态

proposal 的四项拍板（快照边界 / 写点单点 / 存储位 / 出口形状）与五份 delta spec 已把语义钉到字段名 / 函数名 / 判据 / 计数。本设计不重复论证，只做四件事：

1. **把已钉语义落到具体签名与代码形态**（DD1–DD10），特别是 spec 未定死的四处：`freeze` 的空段语义与指引常量文案、`begin` 三分支的判据取定时机、写一次的实现形态、抽屉的组件落点；
2. **对 spec 措辞做工作树校准**（§事实校准 4 条）——其中 1 条会改动公共面（`preamble::load` 的 `async` 修饰符）；
3. **逐条 AC 给落点**（「验收标准落点」表）；
4. **给下游相位可执行的有序任务**（`tasks.md` 十阶段）。

---

## 起点核实（工作树实测；函数名为准，行号仅作辅助、随改动漂移）

- **preamble 的生产 / 消费链**：`compose.rs:60` `assemble_injections(&provenance)` 产出 `SessionInjections`（`preamble` 位 = `archi_knowledge::knowledge()` / `config_knowledge::knowledge()`，其余来源恒 `None`）→ `compose.rs:461` 送入 `kernel.begin_turn(TurnRequest{ injections, … })`；`sdk/loop.rs:324` 每轮 `system_prompt(turn.injections.preamble, preamble::load(&turn.cwd).await)` 合成单条 System。全仓 `preamble::load` 的**唯一生产调用点**就是这一处（`grep -rn "preamble::load" crates/ src/` 排除 `_test.rs` 后恰两行：`loop.rs:326` 与 `mod.rs:21` 的模块声明）。
- **读取根同源**：`sdk/runner.rs:628` `cwd: open.ctx.workspace_root.clone()` —— `LoopTurn.cwd` 就是 `SessionCtx.workspace_root`。把读取点上移到 `compose.begin` 后读的是**同一个根**（worktree 场景即分支版），AC-8 的「读取根不变」因此是结构事实而非承诺。
- **读取函数形态**：`sdk/preamble.rs:9` `pub(crate) async fn load(root: &Path) -> Option<String>`（`tokio::fs::read_to_string` + `truncate` + `<agent_md path="…">` 包装）；`truncate`（`:21`）是 32KB / char 边界截断的**唯一**实现；`MAX_PREAMBLE_BYTES = 32 * 1024`。
- **组合根是同步函数**：`compose.rs:402` `pub fn begin(&self, session, question, ctx, provenance) -> Result<RunningTurn, String>`；两个生产调用点 `src/commands/exec/mod.rs:176` 与 `crates/infra/agent/src/worker.rs:56`（皆在 `async fn` 内），测试调用点 50 处（`compose_test.rs`）。这是「`load` 是否保持 `async`」的约束来源（事实校准 1）。
- **首轮判据已在场**：`store.rs:1205` `is_unstarted_session`（无句柄 + 无轮行 + 无转录）；`compose.rs:434-444` 已按它做「首轮定型」（`restamp_session_config`）。`kernel.begin_turn`（`core/agent/src/kernel.rs:162`）在返回 `RunningTurn` 之前已落会话行（`New` 臂）+ 轮行，`RunningTurn.session_id` 是 `pub` 字段——即「快照落库时机」所需的 id 在 `begin_turn` 返回后才可得。
- **store 写面先例**：`store.rs:1226` `restamp_session_config`（单写事务内读改写）、`store.rs:570-573` 的「early return 不 commit」（未提交 rw 事务随 drop 回滚）、`model.rs:1727` `now_millis`（落库侧时钟）、`store.rs:614` `find_session_summary`（单查装配）、`store.rs:174` `assemble_summary`（清单 / 单查共用装配，现三参）、`store.rs:759` `query_sessions`（清单全表读）。
- **信封与说明自动派生**：`envelope.rs:98` `MODEL_ENTRIES`（`count` / `scan` / `key_of` / `schema` / `walk` 五指针）；`describe.rs:126` `schema_of::<T>` 与 `:177` `graph()` 逐模型灌 `specta::Type` 图 ⇒ **新模型只需 `///` doc + 一行注册**，零模型特定代码（AC-6）。
- **已用 `native_model` id**：1 / 2 / 4 / 5 / 6 / 7 / 8 / 9 / 10 / 11 / 12 / 13 / 15 / 16 / 17 / 18（空缺 3 / 14；`NotificationRecord` = 18，其注释明写「现役最大值 + 1，不补空洞」）。本变更取 **19**。
- **`db_tables` 零硬编码**：`sdk/db_tool.rs` 的表清单与下钻全由 `envelope::list_models` / `describe` 派生（`Store::scan` / `Store::query` 按注册表分发），代码侧零计数常量（计数只活在测试断言与 spec 文案里）。
- **前端 `SessionSummary` 字面构造点**：生产代码恰一处（`hooks/use-agent-session.ts:151` `sessionFromTurn`，`row.configSnapshot: null` 同型先例）；`applyReplay`（`:346`）整份 `latest`（= `session_detail` 返回值）落镜像 ⇒ 新增字段**零改动即贯通**重放路径。`agent-session-view.tsx` 现无任何 preamble / 系统提示词位（`grep -rn preamble src/` 零命中）。
- **抽屉与按钮先例**：`components/standard/drawer.tsx` 支持 `open` / `onOpenChange` 受控且可无 `trigger`（`views/agent/components/agent-run-history.tsx:130` 同型）；`components/ui/button.tsx` 有 `link` / `ghost` 与 `xs` 档；时间呈现惯例 `new Date(ms).toLocaleString()`（`agent-run-history.tsx:102` 等 3 处）。
- **架构模型**：`Backend.Infra.*` / `Backend.Core.*` 的 `path` 皆为**目录级**（`03-backend-infra.toml:36` 的 `./packages/desktop/src-tauri/crates/infra/store/`）⇒ 本变更后端零元素 / 零边 / 零 path 改动；前端 `Desktop.Components.Agent.AgentSessionView`（`06-frontend.toml:123`）的文件级 `path` 已含目标文件 ⇒ 只改契约资产 prose。`archi_drift` 实读 **55 元素 / 129 关系 / `status = clean` / `apiFindings` 0** ——本变更落地后该读数不变。
- **版本**：`packages/desktop/package.json` `0.7.7`；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随（其 `version` 行即该引用）；`src-tauri/Cargo.toml` 的 crate 版本行不随动。

### 事实校准（proposal / spec 措辞 vs 工作树；不改需求意图，只定实现形态）

1. **`preamble::load` 的 `async` 与 `ComposedTurn::begin` 的「签名零变更」不可同时成立**。spec 的 Function 表把 `load` 钉成 `async fn load(root: &Path) -> Option<String>`，而 compose 的 Module Contract 同时要求 `begin` **签名零变更**（参数表 `session` / `question` / `ctx` / `provenance` 逐字不变）。`begin` 是**同步** `fn`：在同步函数里等一个 `tokio::fs` future 只有 `block_in_place + Handle::block_on` 一条路，而它在 `#[tokio::test]` 的 current-thread runtime 上直接 panic（`compose_test` 的大半用例）；把 `begin` 改成 `async fn` 则本次改动要牵动 50 处测试调用点与两个生产调用点，且 Rust 类型面上**就是**签名变更。定稿：**保 `begin` 同步、`load` 去 `async`（`std::fs::read_to_string`）**——读口径（读取根 / 严格单文件 / 32KB char 边界截断 / `<agent_md>` 包装 / 失败即 `None`）逐条不动，变的只是「读发生在哪个执行体」。归档合并时 delta spec 该行的 `async` 修饰符随之校正（同 `backend-notification-hub` 的事实校准机制：本轮不改 spec 正文）。
2. **`assemble_injections` 仍是知识段的唯一来源，快照不另立第二套来源判定**。spec 要求 `freeze` 的入参是「知识段 + `AGENT.md` 段」，而知识段由 `assemble_injections` 按 provenance 决定（archi 臂 / config 臂 / 其余 `None`）。定稿：`begin` **先** `assemble_injections`，把它的 `injections.preamble` 当知识段喂 `freeze`，随后用冻结结果覆盖同一位——零新增 provenance 分支、零复制知识选择逻辑（否则两条判定必然漂移）。
3. **陈旧计数文案在本变更一并纠正**。`store.rs:345` 的 `workspace_models` doc 写「十二模型」（= 现役 12 个，含 `StoreMetaRecord`）、`store.rs:417` 的 `Store` doc 写「run 运行史十模型」、`store.rs:320` 的 `DbDimension::Workspace` doc 同写「十模型」——后两处按现役 11 个（不含 `store_meta`）本就少了一个数。定稿：三处随本变更落到**准确值**（13 个注册模型 / 12 张表），与 `db_tables` delta spec 的「workspace 12 / 合计 16」同源（`desktop-agent-db-tools` 已登记「基线 15 是 `notification` 登记后未同步的陈旧计数」，本变更顺手补齐代码侧同源文案）。
4. **存量补拍用的是「补拍当轮的知识」而非历史知识**。spec 的两条并置（「存量补拍按首轮口径现算」+「应用升级后旧会话沿用拍存时刻的知识」）在时间上不冲突，但措辞易被误读成「补拍应还原建会话时的知识」——那不可能（没有任何历史见证）。定稿：补拍 = **首次 `Continue` 时刻**的现算（含届时版本的 `archi` / `config` 知识）；此后才冻结。这条写进 `begin` 的 doc，避免实现期把它当缺陷。

---

## 关键设计决策（DD1–DD10）

> proposal 的四项拍板与 explore §30 的最终方案不重复。下表是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | 快照合成单点落 `sdk/preamble.rs`（不拆新文件） | 该文件承载四件：`load`（读取 + 包装，**去 async**）、`truncate`（32KB 单点，零改动）、`freeze(knowledge, agent_md) -> String`（合成单点）、`GUIDANCE`（常量）。模块可见性保持 `pub(crate)`，零新增依赖 | 读取与合成本是一个关注点（「系统提示词从哪来、长什么样」），拆文件会让 `truncate` 的「唯一消费者」关系跨模块；spec 的 Module Contract 也已把四件钉在同一文件。`freeze` MUST 复用 `truncate` 的产物（`load` 已截断），MUST NOT 自己再截一次——两处截断必然在 32KB 边界上漂移 |
| DD2 | `freeze` 的空段语义与 `GUIDANCE` 文案 | 段序固定：知识 → `AGENT.md` 段 → `GUIDANCE`，**段间恰一个空行**（`"\n\n"` 连接）；空串视同缺席（不 trim——`AGENT.md` 段逐字语义不许改动空白）；**知识与 `AGENT.md` 皆空 → 返回空串**（`GUIDANCE` 不单独成文）。`GUIDANCE` 定稿为单句中文常量（107 char / 273 B）：`（本会话的系统提示词已在会话开始时冻结，此后每轮逐字复用，不因文件变更而改写。项目规则的真实出处是 AGENT.md；若用户提示规则已更新，请用 read 工具重新读取 AGENT.md，并按其中的最新规则执行。）` | 三条边界都有硬判据：① 让 `GUIDANCE` 单独成文会凭空给「无知识 + 无 `AGENT.md`」的会话塞一条 System 消息，破 AC-7 的「零 System」；② 对段做 `trim` 会让 `AGENT.md` 的首尾空行消失（「逐字」判据 → 测试会红）；③ 文案须零路径字面量（worktree 场景路径逐会话不同，写死即错）、零 `<>` 占位符（装配层常量，无注入位）、同时表达「冻结 / 真身 `AGENT.md` / `read` 重读」三点 |
| DD3 | `begin` 的判定序与「一次取定」 | 顺序固定：① `assemble_injections`（得知识段）→ ② `Continue` 臂**先** `find_session(id)`（`ok_or_else("会话不存在: {id}")` 的文案是既有契约，MUST NOT 让 `is_unstarted_session` 的内部 `Err` 顶替它）→ ③ **取定首轮布尔量** `first_turn = matches!(session, New) ∨ (Continue{id} 且 is_unstarted_session(id))` → ④ 分支得 `(prior_handle, stamp: Option<String>)` → ⑤ `begin_turn` → ⑥ 按 `stamp` 落库。**`is_unstarted_session` 只查一次**（`begin_turn` 会写轮行，同一判据在之后必然转假）；补拍分支只在「已开轮 ∧ `find_session_preamble` 返回 `None`」时进入 | 「另立第二套首轮判据」会让两份判据在边界上分叉（例如「有转录无轮行」的畸形库）；`begin_turn` 之后再查一次 `is_unstarted_session` 恒假（轮行已落），是必错写法，故判据取定必须在 `begin_turn` 之前并复用同一布尔量 |
| DD4 | 落库时机、接缝与失败处置 | `stamp` 发生在 `begin_turn` **返回之后**、`Ok(running)` 之前，用 `running.session_id`；经 `self.query.store().stamp_session_preamble(&id, &body)` 直写（**不扩 `SessionSink`**）；失败 `crate::sdk::log::append_engine_log("会话系统提示词快照落库失败 session=… cause=…")` 一行，MUST NOT 中断本轮、MUST NOT 上抛 | `New` 路径在 `begin_turn` 之前**没有**会话 id（id 由 `kernel` 铸造），故落库只能在其后；spec 已明写「MUST NOT 早于会话建立」与「本轮 MUST NOT 读回自己刚写的快照」（本轮用内存体，注入面早已随 `TurnRequest` 交给引擎）。直写 `Arc<Store>` 沿 `restamp_session_config` 同处同型先例；扩 `SessionSink` 要动 core 端口契约（proposal Q4 已否）。失败降级沿 `kernel.rs:289` 的 `let _ = sink.bind_remote_session(…)` 尽力语义，但**要留痕**（`append_engine_log` 是 infra 既有日志单点） |
| DD5 | 写一次的实现形态 | 单写事务内先 `rw.get().primary::<SessionPreambleRecord>(session_id)`；命中 → `return Ok(false)`（**不 commit**，未提交 rw 事务随 drop 回滚，沿 `remove_workspace` 先例）；未命中 → `rw.insert(record{ saved_at: now_millis(), … })` + `commit()` + `Ok(true)` | 「先 delete 再 insert」或「upsert 覆盖」都会让快照可变（写一次语义破）；`Ok(false)` 而不是 `Err` 是**语义正确**的返回值（重复拍存不是错误，是幂等命中），服务层据此可直接忽略 |
| DD6 | 出线形状与装配路径 | `SessionPreambleInfo { saved_at: i64, content: String }` 具名 struct（`specta::Type`）+ `SessionSummary.preamble: Option<SessionPreambleInfo>`（**不 skip** 序列化 ⇒ 线面恒有键，`null` 值）；store 侧 `assemble_summary` **增第四参** `preamble: Option<SessionPreambleInfo>`：`find_session_summary` 传 `find_session_preamble` 的映射结果，`query_sessions` 传 `None` | 「由 `find_session_summary` 事后覆写字段」会让清单 / 单查共用同一构造函数却有两种后置改写，读面纪律从结构退化为顺序纪律；参数化后「清单路径不读快照表」是**调用点上的空值**（可 grep、可断言），AC-9 因此结构性成立。`skip_serializing_if` 禁用是仓库既有纪律（`crates/core/agent/src/event.rs` 三处 doc 明写它会令 bindings 裂出相位伴生类型），前端 `null` 形态也更省分支 |
| DD7 | `loop.rs` 一元化的落地形态 | 删 `system_prompt(knowledge, agent_md)` 函数与 `preamble` 的 `use`；`continuation` 循环内改为 `if let Some(preamble) = turn.injections.preamble.as_deref() { chat_history.push(Message::system(preamble)); }`（`&str` 直接进 `impl Into<String>`，零 clone）；`turn.cwd` 在 `sandbox` / `protect` / `tools` 三处照旧使用，**不删字段** | 留一个单参 `system_prompt(Option<&str>)` 只是给 `if let` 套壳（零信息量）；留 `turn.cwd` 是因为它仍是沙箱 / 写保护 / 工具执行的根——「不读文件」不等于「不需要 cwd」 |
| DD8 | 前端可见面的组件落点与数据流 | `SessionMeta` 内新增入口按钮（`Button variant="link" size="xs"`，`data-testid="session-meta-preamble"`，可访问名「系统提示词」）；开合态由新私有组件 `PreambleDrawer`（**不导出**）自持 `useState`，受控传给 `StandardDrawer`（`open` / `onOpenChange`，无 `trigger`）；抽屉内容三段：`拍存于 {new Date(savedAt).toLocaleString()}` + `{bytes} 字节`、静态指引文案、`<pre>` 全文（`whitespace-pre-wrap` + 等宽 + 可滚动 + 可选中）。体量就地算：`new TextEncoder().encode(content).length`。`preamble === null` 时**不渲染入口也不占位** | 拆私有组件的直接原因是 `.tsx` 的 `max-lines-per-function: 80`（`SessionMeta` 已约 40 行）；`StandardDrawer` 受控无 trigger 的用法有 `agent-run-history.tsx:130` 先例；体量不落库（proposal Q11 已否 sha / bytes 落库），故必须在前端算——`TextEncoder` 是浏览器标准 API，零新依赖，且与「UTF-8 字节数」判据逐字对位（`content.length` 是 UTF-16 码元数，中文会算错） |
| DD9 | 文档计数与契约资产 prose 的落笔 | `store.rs` 三处 doc 计数（`workspace_models` 十三模型 / `Store` 与 `DbDimension::Workspace` 十二模型并补「会话系统提示词快照」一项）；`Desktop.Components.Agent.AgentSessionView.md` 在「职能」段末补一句快照只读展示位（公共面表、不变量、留白段、`json api-types` 围栏**零改动**） | 计数文案三处同值（事实校准 3），漏一处即自相矛盾；契约资产只加散文——`api-types` 围栏与 `$defs` 是机械闭环的比对面，加内容会触发 `api:check` 非零并需要重新生成 `INDEX.toml`（本变更**无**公共 API 面变化，AC-12 要求 `api:check` 零 diff） |
| DD10 | 生成物与版本纪律 | `src/types/generated/{bindings.ts,bindings.zod.ts}` 只经 `bindings:export` 再生（`SessionPreambleInfo` 类型 + `SessionSummary.preamble` + `SessionPreambleInfoSchema`）；`packages/desktop/package.json` `0.7.7` → `0.7.8`；`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` crate 版本行**不动** | 生成物是派生面，手改即「零 diff 门」失败；版本补丁号是 desktop 源改动的硬纪律（AGENT.md「Version discipline」），AC-13 亦明列 |

---

## 架构组件

### 1. 快照合成（`crates/infra/agent/src/sdk/preamble.rs`）

- **职责**：把「会话 preamble 知识」与「`AGENT.md` 段」合成为**一份逐字不变的快照正文**，并把「冻结语义」写进正文末段（模型可发现）。
- **文件位置**：同文件（不新增文件）。四件：`load` / `truncate` / `freeze` / `GUIDANCE`。
- **依赖**：仅 `std::path::Path` 与 `std::fs`（去 `tokio::fs` 后本文件零 async、零 runtime 依赖）；`truncate` 复用既有实现。
- **技术**：纯函数 + 一次同步小文件读取（本仓 `AGENT.md` ≈ 6.5 KB）；无状态、无缓存、无 panic 路径。

### 2. 快照拍存 / 读回 / 补拍（`crates/infra/agent/src/compose.rs`）

- **职责**：会话首轮把快照拍进库并让**本轮**用内存体；续轮从库读回覆盖注入位；存量会话首次续轮补拍一次。
- **文件位置**：`ComposedTurn::begin`（内部三分支 + 两个私有小助手）+ `assemble_injections` 的 doc 一行。
- **依赖**：`crate::sdk::preamble::{freeze, load}`、`crate::sdk::log::append_engine_log`、`self.query.store()`（`Store::is_unstarted_session` / `find_session` / `restamp_session_config` / `find_session_preamble` / `stamp_session_preamble`）。
- **技术**：同步分支 + 单次内存体克隆（`Option<String>` → `Some(body)` 与 `stamp` 各持一份，体量 ≤ 37 KB、每会话至多一次）。

### 3. 系统消息一元消费（`crates/infra/agent/src/sdk/loop.rs`）

- **职责**：只把注入位给的字符串搬成**恰一条** `Message::System`；对「系统提示词从哪来」零知识。
- **文件位置**：`continuation` 循环内的系统消息构造点；删 `system_prompt`。
- **依赖**：零新增（`Message` 既有）。
- **技术**：同步 `if let`，零 IO、零 async 新增。

### 4. 快照持久载体（`crates/infra/store`）

- **职责**：workspace 维度的 `SessionPreambleRecord` 模型、写一次落库 / 主键直查、信封登记（`db_tables` / `scan` / `describe` / `db_query` 自动覆盖）、单查装配时填 DTO。
- **文件位置**：`src/model.rs`（模型）+ `src/store.rs`（`workspace_models` 注册 + 两方法 + `find_session_summary` / `assemble_summary`）+ `src/envelope.rs`（注册表一行 + 三包装函数）+ `src/lib.rs`（导出）。
- **依赖**：既有 `native_db` / `native_model` / `serde` / `specta` / `agent`（DTO）。
- **技术**：单写事务内「读主键 → 命中即返；未命中即插」；`saved_at` 取 `now_millis()`（落库侧时钟单点）。

### 5. 出线 DTO 与注入位语义（`crates/core/agent`）

- **职责**：把快照面作为 `SessionSummary` 的**只读出线字段**承载；`SessionInjections.preamble` 的语义 doc 反转为「快照正文」。
- **文件位置**：`src/session.rs`（+ `SessionPreambleInfo`，`SessionSummary` + 一字段）、`src/runner.rs`（仅 doc）、`src/lib.rs`（根导出 +1 名）。
- **依赖**：`serde` / `specta`（既有）。
- **技术**：纯数据；`SessionRow` / `SessionStats` / `TurnSummary` 零改动（避免二次 zod 夹具连锁）。

### 6. 前端只读可见面（`packages/desktop/src`）

- **职责**：把快照从「不可见」变成「元信息行一处入口 + 抽屉全文可查」，且零取数、零写面、不进对话流。
- **文件位置**：`components/agent/agent-session-view.tsx`（入口 + 私有 `PreambleDrawer`）、`hooks/use-agent-session.ts`（本地镜像补 `preamble: null`）。
- **依赖**：`StandardDrawer`（既有）、`Button` / `Badge` / `TextTooltip`（既有）、`SessionPreambleInfo`（生成的绑定类型）。
- **技术**：React 19 `useState` 自持开合态；数据全取自 `SessionSummary.preamble`（零 IPC、零订阅、零 hook 扩面）。

---

## 变更清单

> **覆盖口径**：下列子表覆盖 proposal「变更范围 - 实现文件」全部 12 条目，并补设计层新增 3 处——`crates/infra/store/src/lib.rs`（`SessionPreambleRecord` 必须出 crate 根，compose 要读它的 `content` / `saved_at`）、`crates/core/agent/src/lib.rs`（`pub use session::{…}` 是显式名单，新 DTO 不登记即不可达）、`packages/desktop/package.json` 版本行（AC-13）。成因见事实校准 1 / DD10。测试文件的期望随动**不属本相位改动**，见末节「交接清单」。

### 新增文件

<!-- 本变更零新增源文件：合成单点扩 `sdk/preamble.rs`（DD1）、持久载体追加进既有 `model.rs`（「新模型 = 定义 struct + 登记一行」机制）、前端可见面落既有 `agent-session-view.tsx`（DD8）。故本子表整段省略。 -->

### 修改文件

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | ① 新增 `SessionPreambleInfo { saved_at: i64, content: String }`（`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`；两字段各带 `///` doc；结构体 doc 写明「会话系统提示词快照的**只读**出线面，仅 `session_detail` 携带」）；② `SessionSummary` 追加 `pub preamble: Option<SessionPreambleInfo>`（带 `///` doc：清单路径恒 `None`、单查路径按库内行填） | DTO 扩面单点（DD6）；`SessionRow` / `SessionStats` / `TurnSummary` 零改动 |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | `pub use session::{…}` 名单内按字母序插入 `SessionPreambleInfo`（`SessionPage` 与 `SessionProvenance` 之间） | 显式 re-export 名单，漏登记则 store / 壳 / specta 都取不到该类型 |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `SessionInjections.preamble` 的 doc 由「会话级系统前导（None 即不注入；维持不消费）」改为「会话系统提示词快照正文（None = 本会话不注入 System 消息）——合成与拍存归 infra 装配层，core 只不透明承载」 | **零结构改动**：六位字段面、`serde` 形态、`PartialEq/Eq` 派生链逐字不动（spec 明条） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 追加 `SessionPreambleRecord`（插在 `NotificationRecord` 定义块之后、`pack_key` 段之前，与既有模型定义区集中）：`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]` + `#[native_model(id = 19, version = 1)]` + `#[native_db]`；三字段（`#[primary_key] session_id: String` / `content: String` / `saved_at: i64`）各带 `///` doc；结构体 doc 首句为可截断的表介绍句，并写明「id 取现役最大值 + 1（= 19），不补历史空洞 3 / 14」「无版本链」「空串正文 = 快照成立但无正文」 | 表 / 字段说明的唯一派生源（`describe` + ratchet 测试自动纳入）；见「数据模型」节 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | ① `workspace_models()` 追加 `models.define::<SessionPreambleRecord>().expect("定义 SessionPreambleRecord 失败")`（排在 `StoreMetaRecord` 之前）；② 其 doc 计数「十二模型」→「十三模型」并把新模型并入既有模型枚举；③ `Store` 结构性 doc 与 `DbDimension::Workspace` 的 doc 计数与枚举同步（事实校准 3）；④ 新增 `stamp_session_preamble` / `find_session_preamble` 两方法（DD5）；⑤ `assemble_summary` 增第四参 `preamble: Option<SessionPreambleInfo>` 并填入 `SessionSummary.preamble`；⑥ `find_session_summary` 在装配前读快照行并映射 DTO；`query_sessions` 的调用点传 `None`；⑦ 顶部 `use agent::{…, SessionPreambleInfo}` 与 `use crate::model::{…, SessionPreambleRecord}` | 单查独携 / 清单恒 `None` 且零读（DD6）；方法语义见「公共函数」表 |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | ① `MODEL_ENTRIES` 在 `run_edge` 行之后追加 `ModelEntry { name: "session_preamble", dimension: DbDimension::Workspace, count: count_model::<SessionPreambleRecord>, scan: scan_session_preambles, key_of: session_preamble_key, schema: describe::schema_of::<SessionPreambleRecord>, walk: walk_session_preambles }`；② 三个函数（与同族相邻）：`scan_session_preambles` / `walk_session_preambles`（各一行委托）+ `session_preamble_key`（取 `value["sessionId"]`）；③ 顶部 `use crate::model::{…, SessionPreambleRecord}` | 注册表是「新模型 = 定义 struct + 登记一行」的最小机制；缺登记则 `db_tables` / `scan` / `describe` 全看不到该表（AC-6） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `pub use model::{…}` 名单按字母序插入 `SessionPreambleRecord`（`SessionEventRecord` 与 `SessionRecord` 之间） | 出 crate 根供 compose 读 `content` / `saved_at` |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs` | ① `load` 去 `async`（`std::fs::read_to_string`），**函数体逐字保留**（路径包装 / `truncate` / `None` 语义）；doc 补「调用方为组合根首轮 / 补拍」；② 新增 `freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String`（DD2 的段序与空段语义）；③ 新增 `GUIDANCE: &str` 常量（DD2 的定稿文案）；④ `truncate` **零改动**（32KB 单点，`freeze` 不重复截断） | 合成单点（DD1）；`MAX_PREAMBLE_BYTES` 与 `truncate` 逐字不动（AC-8） |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | ① `begin` 按 DD3 的判定序重排：先 `assemble_injections`（`let mut injections = …`），再取定首轮布尔量，再分支产出 `(prior_handle, stamp)`，再 `begin_turn`，最后按 `stamp` 落库（DD4）；② 两个私有助手：`freeze_preamble(&SessionInjections, &SessionCtx) -> String`（`load(&ctx.workspace_root)` + `freeze`）与 `injectable(String) -> Option<String>`（空串 → `None`）；③ 续轮读回分支：`Some(record)` → `injections.preamble = injectable(record.content)`（**不读文件、不算知识**），`None` → 补拍（同首轮口径）；④ `begin` 的 doc 增「三分支 + 存量补拍取补拍当轮知识（事实校准 4）+ 落库失败不中断」；⑤ `assemble_injections` doc 补一句「`preamble` 位产知识段，最终系统提示词由 `begin` 冻结覆盖」 | `ctx.workspace_root` 的读取 MUST 早于 `ctx` 移入 `TurnRequest`；落库 MUST 用 `running.session_id`（DD4） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | ① 删 `system_prompt` 函数与 `use crate::sdk::{…, preamble, …}` 中的 `preamble`；② `continuation` 循环内的系统消息构造改为只读 `turn.injections.preamble`（DD7），注释改为「系统提示词 = 会话快照正文，逐字进恰一条 System；本模块零文件读取」 | `AGENT.md` / `preamble::load` 字样零残留（AC-2 机械可查）；`turn.cwd` 字段与三处使用零改动 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `pnpm -C packages/desktop run bindings:export` 再生成：新增 `SessionPreambleInfo` 类型、`SessionSummary.preamble`（可空为 `SessionPreambleInfo` 或 `null`）与 `SessionPreambleInfoSchema` | **MUST NOT 手改**；diff 只应来自生成 |
| `packages/desktop/src/hooks/use-agent-session.ts` | `sessionFromTurn` 的 `SessionSummary` 字面补 `preamble: null`（`stats` 行之后）；其余零改动（`applyReplay` 原样落 `session_detail` 的 `preamble`） | 本地轮行镜像没有快照面；重放收口后入口自然出现（DD6 / spec 的 chat-infra 约定） |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | ① `SessionMeta` 内按 `summary.preamble !== null` 渲染入口按钮（`data-testid="session-meta-preamble"`）；② 新增私有 `PreambleDrawer({ preamble })`：自持 `useState` 开合态 + `StandardDrawer`（`title="系统提示词快照"`，`open` / `onOpenChange`）+ 三段内容（拍存时刻 / 体量 / 静态指引 / 正文）；③ 静态指引文案为**组件内常量**（不复用快照里的 `GUIDANCE`，也不借 `SessionPanelState.note`） | 零取数 / 零订阅 / 零写面 / 不进消息流（DD8）；导出面（`AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps`）零变化 |
| `.dev-team/architecture/models/Desktop/Components/Agent/Desktop.Components.Agent.AgentSessionView.md` | 「职能」段末补一句：元信息行在有快照时提供「系统提示词」只读入口（抽屉展示拍存时刻 / 体量 / 静态指引 / 全文，数据取自 `SessionSummary.preamble`） | 公共面表 / 不变量 / 留白段 / `json api-types` 围栏零改动（DD9；`api:check` 保持零 diff） |
| `packages/desktop/package.json` | `version` `0.7.7` → `0.7.8` | AC-13；`tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` crate 版本行**不动** |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `Store::stamp_session_preamble` | `crates/infra/store/src/store.rs` | 新增 | `pub fn stamp_session_preamble(&self, session_id: &str, content: &str) -> Result<bool, StoreError>` | 单写事务**写一次**：行已存在 → `Ok(false)`（不改写、不 commit）；否则插入 `SessionPreambleRecord { session_id, content, saved_at: now_millis() }` + `commit()` + `Ok(true)`。无删除 / 更新 / 重置语义 |
| `Store::find_session_preamble` | 同上 | 新增 | `pub fn find_session_preamble(&self, session_id: &str) -> Result<Option<SessionPreambleRecord>, StoreError>` | 主键直查（miss → `Ok(None)`）；读事务单点 |
| `Store::find_session_summary` | 同上 | 修改（签名零变更） | `pub fn find_session_summary(&self, session_id: &str) -> Result<Option<SessionSummary>, String>` | 实现增一步：装配前 `find_session_preamble` 并映射 `SessionPreambleInfo`（`saved_at` / `content`）；查无会话仍 `Ok(None)` |
| `preamble::load` | `crates/infra/agent/src/sdk/preamble.rs` | 修改 | `pub(crate) fn load(root: &Path) -> Option<String>` | `async` 退役为同步（事实校准 1）；读取根 / 严格单文件 / 32KB char 边界截断 / `<agent_md path="…">` 包装 / 失败即 `None` 逐条不变；调用方由 `loop` 改为 `compose` |
| `preamble::freeze` | 同上 | 新增 | `pub(crate) fn freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String` | 快照正文合成单点：非空段按「知识 → `AGENT.md` 段 → `GUIDANCE`」以 `"\n\n"` 连接；知识与 `AGENT.md` 皆空 → 空串；`GUIDANCE` MUST NOT 单独成文 |
| `preamble::GUIDANCE` | 同上 | 新增 | `pub(crate) const GUIDANCE: &str` | 冻结指引常量（107 char / 273 B ≤ 512 B；零路径字面量、零 `<>` 占位符；文案见 DD2） |
| `ComposedTurn::begin` | `crates/infra/agent/src/compose.rs` | 修改（签名零变更） | `pub fn begin(&self, session: SessionRef, question: String, ctx: SessionCtx, provenance: SessionProvenance) -> Result<RunningTurn, String>` | 内部三分支（首轮 / 续轮读回 / 存量补拍，DD3）+ 落库（DD4）；`ctx.workspace_root` 在 `ctx` 移入 `TurnRequest` 前读取 |

> 私有件不入本表：`preamble::truncate`、`compose::freeze_preamble`、`compose::injectable`、`Store::assemble_summary`（增第四参）、`Store::query_sessions`（签名零变更，仅调用点补 `None`）——形态与口径见「修改文件」表对应行，行为判据见 AC 落点表。
> 前端零新增导出：`PreambleDrawer` 为组件内部私有件（`AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` 导出面逐字不变，`index.tsx` 桶导出零改动）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `SessionPreambleInfo` | `crates/core/agent/src/session.rs` | 新增 | `pub struct SessionPreambleInfo { pub saved_at: i64, pub content: String }`；`Debug` / `Clone` / `PartialEq` / `Eq` / `Serialize` / `Deserialize` / `Type` + `#[serde(rename_all = "camelCase")]`（线名 `savedAt` / `content`）；出线只读 DTO |
| `SessionSummary` | 同上 | 修改 | 追加 `pub preamble: Option<SessionPreambleInfo>`（不 skip 序列化：线面恒出键，单查为对象、清单为 `null`）；其余三字段零改动 |
| `SessionInjections` | `crates/core/agent/src/runner.rs` | 修改（仅 doc） | `preamble: Option<String>` 的语义 doc 反转为「会话系统提示词快照正文」；字段面 / serde / 派生链零改动 |
| `SessionPreambleRecord` | `crates/infra/store/src/model.rs` | 新增 | native_model 模型（`id = 19, version = 1`）：`#[primary_key] session_id: String` / `content: String` / `saved_at: i64`；`Debug` / `Clone` / `PartialEq` / `Eq` / `Serialize` / `Deserialize` / `Type`；无版本链、无二级索引 |
| `PreambleDrawer` | `packages/desktop/src/components/agent/agent-session-view.tsx` | 新增（**私有**） | `function PreambleDrawer({ preamble }: { preamble: SessionPreambleInfo }): React.JSX.Element`；仅供 `SessionMeta` 内部使用，不导出 |

### 配置

<!-- 本变更零配置键改动：`KNOWN_KEYS`、`config.schema.json`、`TestFramework` 枚举、写保护配置与 watch 面逐字不动（proposal「不要修改」明列；「`AGENT.md` 是否纳入写保护」已裁决为否——Q8）。故本子表整段省略。 -->

---

## 数据模型

### `SessionPreambleRecord`（workspace 维度，workspace 库新增一模型）

| 字段 | 类型 | 语义 | 线名 |
|---|---|---|---|
| `session_id` | `String`（主键） | core 铸会话 id（与 `SessionRecord.id` 一对一的**逻辑**关联；**非外键**——不引申存在性检查或级联删除） | `sessionId` |
| `content` | `String` | 快照正文（知识 + `AGENT.md` 段 + 指引常量的合成体）；**空串 = 快照成立但无正文** | `content` |
| `saved_at` | `i64` | 拍存时刻（UTC unix 毫秒，`now_millis()` 取值） | `savedAt` |

- **落盘归属**：workspace 库（`workspace_models()` 第 13 个注册模型，其中 `StoreMetaRecord` 为库级格式标记、不出 `db_tables` 清单 ⇒ 清单计 12 张）。workspace 隔离是结构性的：`Store` 实例在打开点锁定维度，全局库实例上调用两方法由 native_db 拒绝。
- **`native_model` id = 19**：现役已用 1/2/4/5/6/7/8/9/10/11/12/13/15/16/17/18；取「最大值 + 1」而非补空洞 3 / 14——空洞无法证明从未被历史模型占用，上界之外必不冲突（沿 `NotificationRecord` 注释约定）。
- **无版本链**：新增模型而非既有模型字段演进 ⇒ `SessionRecord`（现 `version = 2`）与全部既有行零改动、零 `from` 链、零迁移代码路径；「未快照」由**行缺席**表达（MUST NOT 用 nullable 列或哨兵值）。
- **索引面**：零 `#[secondary_key]`。访问模式只有「按主键直查」与「按主键写一次」，主键即身份键。
- **生命周期**：与会话同落一个 workspace 库文件，随库整体作废口径（`desktop-workspace-store` 的格式版本与旧库重建条款）；**零级联删除代码路径**（会话本身也无删除面）。
- **体量**：单行 ≤ 约 37 KB（32 KB `AGENT.md` 段 + 知识 + 常量）。因独立成表，`query_sessions` 的 `SessionRecord` 全表扫描不被该载荷膨胀（proposal Q3 的直接收益）。
- **填充路径**（写一处的三个触发面，语义同源）：`New` 首轮 / `Continue` + `is_unstarted_session` 首轮 / `Continue` + 已开轮 + 行缺席（存量补拍）。三处都调 `Store::stamp_session_preamble`，写一次语义保证至多一次生效。
- **读面**：`Store::find_session_preamble`（主键直查）只被 `compose::begin` 与 `Store::find_session_summary` 消费；`query_sessions` 系**不读**。

### 出线形态（`SessionSummary`）

```
SessionSummary {
  row: SessionRow,            // 零改动
  stats: SessionStats,        // 零改动
  turns: TurnSummary[],       // 零改动
  preamble: SessionPreambleInfo | null,   // 新增：单查填、清单恒 null
}
SessionPreambleInfo { savedAt: number, content: string }
```

---

## 路由 / API 设计

零新增路由、零新增命令、零新增事件通道（`all_commands!` 与 `commands/**` 零 diff）。唯一对外面变化是既有 `session_detail` 的**载荷扩字段**：

| 通道 | 名称 | 说明 | 输入 | 输出 | auth |
|---|---|---|---|---|---|
| Tauri IPC | `session_detail` | 单查会话（**载荷新增 `preamble`**） | `root: string`、`sessionId: string` | `SessionSummary \| null`（`preamble: { savedAt, content } \| null`） | 本地 IPC，无鉴权 |
| Tauri IPC | `agent_sessions` / `list_change_chat_sessions` / `list_archi_chat_sessions` / `list_config_chat_sessions` | 会话清单四命令（**载荷恒 `preamble: null` 且不读快照表**） | 各自既有入参（不变） | `SessionPage`（每项 `preamble === null`） | 本地 IPC，无鉴权 |
| Tauri IPC | `agent_session_transcript` | 转录重放（零改动；快照不进转录） | 不变 | `AgentEvent[]` | 本地 IPC，无鉴权 |

- **错误面零变化**：`session_detail` 查无此 id 仍 `Err(String)`、blank root 仍空结果（`desktop-agent-execution`「session_detail 单查面」既有语义）。
- **写面零新增**：无 `session_preamble_reload` / `session_preamble_status` / `session_preamble` 命令（proposal「明确不做」+ AC-5）；快照不可变由「无写命令」结构性保证。

---

## 依赖

| 类别 | 依赖 | 用途 |
|---|---|---|
| 运行时 · 内部（后端） | 无新增 crate 边 | `store → agent`（DTO）与 `agent_runtime → store`（两方法）两条既有边原样复用；`compose → sdk::preamble` 是同 crate 内调用 |
| 运行时 · 外部（后端） | 无新增 | `sdk/preamble.rs` 的 `tokio::fs` **退役**为 `std::fs`（`tokio` 仍是本 crate 他处的依赖，Cargo.toml 零改动）；`native_db` / `native_model` / `serde` / `specta` 全既有 |
| 运行时 · 外部（前端） | 无新增 | `StandardDrawer` + `@/components/ui/button`（既有）、`TextEncoder`（Web 标准）、`new Date(ms).toLocaleString()`（既有惯例） |
| 构建 / 测试 | 无新增 | 沿用 `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `cargo test --workspace` 既有链 |

---

## 待决问题

1. **`GUIDANCE` 的语言与最终措辞**：本设计定稿中文单句（DD2，107 char / 273 B）。若评审要求英文或更长说明，只需替换常量字面——「≤ 512 B、零路径字面量、零占位符、三点语义」的口径不变。
2. **存量补拍取「补拍当轮知识」的语义确认**（事实校准 4）：这是唯一可实现的口径（无历史见证），但若评审期望「补拍沿用会话建档时知识」，则需求本身不可满足，须回到 proposal 层裁决。
3. **`SessionQuery` 是否增快照读面**：本设计**不增**（`compose` 经 `StoreQuery::store()` 直读 store，core 端口零改动）。若未来 SDK 侧需要读快照（例如在 loop 内做长度校验），再议端口扩面——本期无消费点，属投机件。
4. **抽屉的排版细节**：等宽 / 滚动 / 段间距按 `StandardDrawer` 内既有 `ScrollArea` 形态实现期微调；`data-testid` 与三段内容（时刻与体量 / 指引 / 全文）为硬判据，样式不是。
5. **快照 37 KB 无清理策略**：proposal 风险表已列（数百会话 < 15 MB），清理属「会话保留策略」独立变更，本期 MUST NOT 顺手加。

---

## 验收标准落点

| AC | 落点 |
|---|---|
| AC-1 首轮拍存（正文 = 知识 + 空行 + `AGENT.md` 段 + 空行 + 常量；空段跳过） | DD1 / DD2（`freeze` 段序与空段语义）+ DD3 / DD4（`New` 与 `is_unstarted` 分支、`begin_turn` 后落库）+ 「修改文件」表 `compose.rs` / `preamble.rs` 两行 |
| AC-2 续轮读库（不读文件、不算知识；`loop.rs` 字样零残留；两轮 System 逐字相等） | DD3（读回分支）+ DD7（`loop` 一元化）+ 「公共函数」表 `preamble::load` 行（调用方迁移）+ 组件 3 |
| AC-3 中途改文件不改前缀 | DD3（续轮分支 MUST NOT `load`）+ 数据模型「读面」段；结构上由「读点在已开轮分支永不触达」保证 |
| AC-4 存量补拍一次性 | DD3（补拍入口唯一）+ DD5（写一次）+ 事实校准 4 + 「修改文件」表 `store.rs` ⑤行 |
| AC-5 写一次冻结、零写面 | DD5（单写事务先查主键，命中 `Ok(false)`）+ 「路由 / API 设计」的「写面零新增」段 |
| AC-6 存储面自动覆盖（注册 / `scan` / `describe` / `describe_table`） | 「起点核实」的信封与说明自动派生条（`schema_of` + `graph()`）+ 「修改文件」表 `model.rs` / `store.rs` ① / `envelope.rs` 三行 + DD5（零版本链） |
| AC-7 空快照与缺席 | DD2（`GUIDANCE` 不单独成文；全空 → 空串）+ DD3（空正文覆盖为 `None`）+ 「修改文件」表 `compose.rs` 行的私有助手 `injectable` |
| AC-8 读取口径不变 | 起点核实第 2 条（读取根同源）+ 事实校准 1（`load` 函数体逐字保留）+ DD1（`truncate` 零改动、`freeze` 不二次截断） |
| AC-9 出口与载荷纪律 | DD6（`SessionPreambleInfo` + `assemble_summary` 增参：清单传 `None`）+ 「路由 / API 设计」表 + DD10（bindings 只经再生成） |
| AC-10 前端只读可见面 | DD8（入口判据 / 抽屉三段 / 零取数 / 零写面 / 不进消息流）+ 「修改文件」表前端两行 + 「类型定义」表 `PreambleDrawer` 行 |
| AC-11 语料与文档一致 | 事实校准 3（`store.rs` 三处计数）+ DD9（契约资产 prose）+ 五份 delta spec 已含 `db_tables` 计数、`desktop-change-chat` 措辞、`desktop-agent-execution` 的 `sdk/preamble.rs` 行替换 |
| AC-12 全管线回归 | DD10（生成物只经 `bindings:export`）+ DD9（`api-types` 围栏零改动 ⇒ `api:check` 零 diff）+ 起点核实「架构模型」条（55 / 129 不变，零模型改动） |
| AC-13 版本交付 | DD10 + 「修改文件」表 `package.json` 行（0.7.7 → 0.7.8；`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 不动） |

---

## 不要修改（实现期红线）

| 对象 | 理由 |
|---|---|
| `crates/core/agent/src/{port.rs,event.rs,kernel.rs}` | 端口契约、事件词汇、内核治理零结构改动：`SessionQuery` 方法面不变（`find_session_detail` 已返回 `SessionSummary`），快照读走 `StoreQuery` 具体类型（DD3 / 待决问题 3） |
| `src-tauri/src/commands/**`（含 `all_commands!` 与 `commands/exec/mod.rs`） | 出口只扩 DTO，不新增命令；`agent_start` / `session_detail` 签名与注册序零变更（AC-5 / AC-9） |
| `crates/infra/agent/src/sdk/{context.rs,compact.rs}` | L2 剪裁 / L3 compaction 零改动——剪裁时刻的前缀重建是已承认代价（proposal 风险表），不在本变更治理 |
| `crates/infra/agent/src/sdk/runner.rs` | `with_automatic_caching` 与 TTL 零改动（proposal Q10 已否 1h TTL） |
| `crates/infra/agent/src/sdk/{tools.rs,policy.rs,sandbox.rs,protect/**}` | 工具面 / 权限 / 沙箱 / 写保护判定零改动；`AGENT.md` 不纳入内置写保护（proposal Q8 已否） |
| `crates/infra/agent/src/{archi_knowledge.rs,config_knowledge.rs}` 与两份骨架 md | 知识生产者零改动——本变更只改「何时读回」，不改「生产什么」 |
| `crates/infra/store/src/{model.rs 既有模型, query.rs, describe.rs, notification.rs}` | 既有模型行零改动（无 `SessionRecord` v3、无版本链）；`describe` 机械面零改动（新模型靠 `///` doc 自动派生）；`query.rs` 的 `db_query` 过滤 / 投影语义零改动 |
| `packages/desktop/src/components/agent/session-panel.tsx` 与 `SessionPanelState` | 面板骨架与 `note` 槽位优先级零改动（入口落 `AgentSessionView` 元信息行） |
| `packages/desktop/src/hooks/use-file-watch-signal.ts`、watch 命令面 | 「`AGENT.md` 已变更」提示（watch chip / `session_preamble_status`）显式不在本期（proposal「明确不做」） |
| `packages/desktop/src/{lib/ipc.ts,lib/ipc-drift.ts}` | Proxy 形态自动覆盖 DTO 字段变化 |
| `openspec/specs/**` | 除本 change 目录下 5 份 delta 外零 diff（归档合并时按事实校准 1 校正 `sdk/preamble.rs` 行的 `async` 修饰符） |
| `.dev-team/architecture/graph/**` | 零元素 / 零边 / 零 path 改动（后端 `path` 是目录级、前端文件已在 `AgentSessionView` 的 `path` 内）⇒ `INDEX.toml` 零 diff、`archi_drift` 读数保持 55 / 129 |
| `AGENT.md`（仓库根） | 文档面随常规提交演进，本变更不为其新增读取源 / 兜底规则（`AGENTS.md` 复数形态不支持，proposal 已钉） |

---

## 交接给下游相位的随动清单（非本相位改动，只登记不写策略）

| 文件 | 随动原因 |
|---|---|
| `crates/infra/store/src/{model,store,envelope}_test.rs` | 新模型注册 / 字段面 / 无版本链；写一次与主键直查；`find_session_summary` 携快照面、清单路径零读；workspace 维度 11 → 12 行与名字集 15 → 16 |
| `crates/core/agent/src/session_test.rs` | `SessionSummary` 新字段的派生与序列化（`preamble: null` 不省略） |
| `crates/infra/agent/src/sdk/preamble_test.rs` | `load` 去 `async` 的调用面；`freeze` 段序 / 空段跳过 / 全空空串 / 常量体量与零占位符 |
| `crates/infra/agent/src/compose_test.rs` | 首轮落库、续轮读库覆盖、补拍一次性、落库失败不中断、空快照零 System |
| `crates/infra/agent/src/sdk/loop_test.rs` | 删 `system_prompt` 后的合成点重锚（两轮 System 逐字相等、`preamble = None` 零 System） |
| `src-tauri/src/bindings/mod_test.rs` | 生成类型清单与 `session_detail` 返回形状（含 `SessionPreambleInfo`） |
| `packages/desktop/src/**` 八个既有夹具 + 会话视图测试 | `SessionSummary` 为 zod `strictObject`，夹具补 `preamble: null`；新增入口 / 抽屉的组件用例（清单见 proposal「测试文件」段） |
| `openspec/changes/session-preamble-snapshot/test-design.md` | 由测试相位新建；口径 = proposal「验收标准」表 + 本设计的「验收标准落点」 |
