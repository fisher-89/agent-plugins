# 任务: session-preamble-snapshot

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 12 条目 + 设计层新增 3 处（`crates/infra/store/src/lib.rs` 的 `SessionPreambleRecord` 导出、`crates/core/agent/src/lib.rs` 的 `SessionPreambleInfo` 根导出、`packages/desktop/package.json` 版本行）——逐条登记在 design「变更清单」，成因见 design 事实校准 1 / DD10。
>
> **本相位只写实现**：不改测试用例与断言、不新建 `test-design.md`、不执行任何静态检查 / 测试回归。测试面随动归 test-design / test-gen / test-execution 相位——清单见 design 末节「交接清单」。
>
> **依赖序（十阶段）**：
>
> - **阶段一**（core 出线 DTO + 注入位 doc）是一切的前置：store 的单查装配与前端生成物都要 `SessionPreambleInfo`。
> - **阶段二**（store 持久载体与操作面）依赖阶段一（`SessionSummary` 的填充需要 DTO）；它同时解锁阶段四（`find_session_preamble` / `stamp_session_preamble`）与阶段六（生成物里出现新类型）。
> - **阶段三**（快照合成单点）独立于后端其余部分，只动 `sdk/preamble.rs`；阶段四依赖它（`freeze` / `load`）。
> - **阶段四**（组合根三分支）依赖阶段二 + 阶段三。
> - **阶段五**（引擎系统消息一元化）依赖阶段四——**两者必须同批落地**：`loop` 一旦停止读文件，`compose` 尚未冻结就会让首轮丢失 `AGENT.md` 段（中间态是行为回退，不是可发布的稳定态）。
> - **阶段六**（IPC 类型再生成）依赖阶段一（DTO 面已定）；阶段七 / 八依赖阶段六（生成了 `preamble` 字段才能过类型检查）。
> - **阶段九**（契约资产 prose）独立于代码，落笔后 `api:check` 口径不变。
> - **阶段十**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **core 零结构改动**：`crates/core/agent` 只增一个具名 DTO 与 `SessionSummary` 的一个字段；`SessionInjections` 六位字段面、`SessionQuery` / `SessionSink` 方法面、事件词汇、`SessionRow` / `SessionStats` / `TurnSummary` 逐字不动。
> ② **命令面零 diff**：`src-tauri/src/commands/**`（含 `all_commands!`）与 `src-tauri/src/main.rs` 零改动；零新增写命令、零新增事件通道。
> ③ **引擎零文件读取**：`sdk/loop.rs` 的 `AGENT.md` 与 `preamble::load` 字样必须归零；`turn.cwd` 只服务沙箱 / 写保护 / 工具执行。
> ④ **截断单点**：32KB / char 边界只由 `preamble::truncate` 实现，`freeze` 不复制、不二次截断；`load` 的读取根与包装形态逐字不变。
> ⑤ **落库失败不中断**：任何 `stamp_session_preamble` 失败只 `append_engine_log` 一行，MUST NOT 让 `begin` 返回 `Err`、MUST NOT 影响本轮驱动。
> ⑥ **生成物不手改**：`packages/desktop/src/types/generated/**` 只经 `bindings:export`；`.dev-team/architecture/graph/INDEX.toml` 只经 `api:export`（本变更应零 diff）。
> ⑦ **门禁分工**：`pnpm -C packages/desktop run check` 四段、`vp test`、`cargo test --workspace` 由桌面端 static-check / test-execution 相位执行；本相位只做源码级自查（`grep` 零命中、读码复核、`git status --porcelain` 边界比对）。

## 阶段一：出线 DTO 与注入位语义（`crates/core/agent`）

- [x] `packages/desktop/src-tauri/crates/core/agent/src/session.rs`：在 `SessionStats` 之后、`SessionSummary` 之前追加 `SessionPreambleInfo`：
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`（与 `SessionStats` 同型）。
  - 结构体 `///` doc：会话系统提示词快照的**只读**出线面；仅 `session_detail` 单查路径携带，清单路径恒 `None`；快照不可变、无写命令。
  - 字段与 doc：`pub saved_at: i64`（拍存时刻，UTC unix 毫秒）、`pub content: String`（快照正文全文，可能为空串）。
- [x] 同文件 `SessionSummary`：追加第四个字段 `pub preamble: Option<SessionPreambleInfo>`（排在 `turns` 之后）；字段 `///` doc 写明「清单路径恒 `None` 且实现不读快照表；单查路径按库内 `session_preamble` 行填充」。**MUST NOT** 加 `#[serde(skip_serializing_if)]`（线面恒有键，值为 `null`）。
- [x] `packages/desktop/src-tauri/crates/core/agent/src/lib.rs`：`pub use session::{…}` 名单内按字母序插入 `SessionPreambleInfo`（`SessionPage` 与 `SessionProvenance` 之间）。
- [x] `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`：`SessionInjections.preamble` 的 doc 由「会话级系统前导（None 即不注入；维持不消费）」改为「会话系统提示词快照正文（None = 本会话不注入 System 消息）；合成、拍存与读回归 infra 装配层，core 只不透明承载」。**字段面 / serde 形态 / `PartialEq`、`Eq` 派生链零改动**。

## 阶段二：store 持久载体与操作面（`crates/infra/store`）

- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：追加 `SessionPreambleRecord`（位置：`NotificationRecord` 定义块之后、`pack_key` 段之前，与既有模型定义区集中）：
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]` + `#[native_model(id = 19, version = 1)]` + `#[native_db]`。
  - 三字段与各字段 `///` doc（表 / 字段说明的唯一派生源）：`#[primary_key] pub session_id: String` / `pub content: String` / `pub saved_at: i64`。
  - 结构体 `///` doc：首句为可截断的表介绍（≤ 120 字、以 `。` 收句），随后写明——workspace 维度落 workspace 库；无行 = 未快照、空串正文 = 快照成立但无正文（两状态 MUST NOT 混用）；`native_model` id 取现役最大值 + 1（= 19）而非补历史空洞 3 / 14；无版本链、零 `from` 链、无迁移；`session_id` 与 `SessionRecord` 一对一的**逻辑**关联而非外键；快照不可变（写一次）。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：
  1. `workspace_models()` 追加 `models.define::<SessionPreambleRecord>().expect("定义 SessionPreambleRecord 失败");`（排在 `StoreMetaRecord` 之前）；其 doc 的「十二模型」改「十三模型」并把「会话系统提示词快照」并入既有模型枚举（「… / explore / 会话系统提示词快照 五既有模型，change 流程状态四模型，run 执行图三模型…」）。
  2. 同文件 `Store` 结构性 doc 的「run 运行史十模型」→「… / run 运行史 / 会话系统提示词快照十二模型」；`DbDimension::Workspace` 的 doc 同步（design 事实校准 3）。
  3. 新增 `pub fn stamp_session_preamble(&self, session_id: &str, content: &str) -> Result<bool, StoreError>`：单写事务 → `rw.get().primary::<SessionPreambleRecord>(session_id)`；命中即 `return Ok(false)`（**不 commit**，沿 `remove_workspace` 的 early-return 先例）；未命中 → `rw.insert(SessionPreambleRecord { session_id: session_id.to_owned(), content: content.to_owned(), saved_at: now_millis() })` + `commit()` + `Ok(true)`。doc 写明「写一次、快照不可变、无更新 / 重置语义、无删除面」。
  4. 新增 `pub fn find_session_preamble(&self, session_id: &str) -> Result<Option<SessionPreambleRecord>, StoreError>`：读事务主键直查，miss → `Ok(None)`。
  5. 私有 `fn assemble_summary(session: &SessionRecord, turns: Vec<TurnSummary>, events: Vec<AgentEvent>, preamble: Option<SessionPreambleInfo>) -> Result<SessionSummary, String>`：签名增第四参并填入 `SessionSummary { row, stats, turns, preamble }`；doc 补一句「快照面由调用方决定：单查填、清单恒 `None`」。
  6. `find_session_summary`：在 `assemble_summary` 之前 `let preamble = self.find_session_preamble(session_id).map_err(|e| e.to_string())?.map(|record| SessionPreambleInfo { saved_at: record.saved_at, content: record.content });` 并传入。
  7. `query_sessions` 的 `assemble_summary(...)` 调用点传 `None`（**清单路径 MUST NOT 读快照表**——不是「读了不给」，是不调用）。
  8. 顶部 `use agent::{…, SessionPreambleInfo}`（按字母序插入）与 `use crate::model::{…, SessionPreambleRecord}`。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`：① `MODEL_ENTRIES` 在 `run_edge` 行之后追加 `ModelEntry { name: "session_preamble", dimension: DbDimension::Workspace, count: count_model::<SessionPreambleRecord>, scan: scan_session_preambles, key_of: session_preamble_key, schema: describe::schema_of::<SessionPreambleRecord>, walk: walk_session_preambles }`；② 按同族相邻位置追加三函数——`scan_session_preambles` / `walk_session_preambles`（各一行委托 `scan_model::<…>` / `walk_model::<…>`）+ `session_preamble_key(value: &Value) -> Value`（`value["sessionId"]`，`session_key` 同形）；③ 顶部 `use crate::model::{…, SessionPreambleRecord}`。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：`pub use model::{…}` 名单按字母序插入 `SessionPreambleRecord`（`SessionEventRecord` 与 `SessionRecord` 之间）。

## 阶段三：快照合成单点（`crates/infra/agent/src/sdk/preamble.rs`）

- [x] `load` 去 `async`：签名改为 `pub(crate) fn load(root: &Path) -> Option<String>`，函数体改为 `std::fs::read_to_string(&agent_file_path_buf).ok()?`（其余逐字保留：`root.join("AGENT.md")`、`to_str().unwrap_or("AGENT.md")`、`truncate`、`<agent_md path="…">\n…\n</agent_md>` 包装、失败即 `None`）。doc 补「调用方 = 组合根的会话首轮 / 存量补拍（一次读取）」并保留「读取侧永不报错」一句。
- [x] 新增 `pub(crate) const GUIDANCE: &str`：常量字面定稿为
  `（本会话的系统提示词已在会话开始时冻结，此后每轮逐字复用，不因文件变更而改写。项目规则的真实出处是 AGENT.md；若用户提示规则已更新，请用 read 工具重新读取 AGENT.md，并按其中的最新规则执行。）`
  ；doc 写明三条硬约束——≤ 512 B（char 计，实际 107 char / 273 B）、零路径字面量（只用文件名 `AGENT.md`）、零 `<>` 占位符；并写明语义三点（冻结 / 真身 `AGENT.md` / `read` 重读）。
- [x] 新增 `pub(crate) fn freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String`：按「知识 → `AGENT.md` 段 → `GUIDANCE`」收集**非空**段（`Option::filter(|s| !s.is_empty())`，**不 trim**——`AGENT.md` 段逐字语义），以 `"\n\n"` 连接；知识与 `AGENT.md` 皆空 → 返回 `String::new()`（`GUIDANCE` MUST NOT 单独成文）。doc 写明「合成单点：段序与空段语义的唯一真相；32KB 截断已在 `load` 内完成，本函数 MUST NOT 再截」。
- [x] `truncate` 与 `MAX_PREAMBLE_BYTES` **零改动**（自查：`grep -n "MAX_PREAMBLE_BYTES" crates/infra/agent/src/sdk/preamble.rs` 仍恰一处定义 + 一处消费）。

## 阶段四：组合根三分支（`crates/infra/agent/src/compose.rs`）

- [x] `ComposedTurn::begin` 按固定序重排（design DD3 / DD4）：
  1. 既有 `snapshot`（`SessionConfigSnapshot`）组装逐字保留。
  2. **先** `let mut injections = assemble_injections(&provenance)?;`（知识段落在 `injections.preamble`）。
  3. 取定首轮布尔量：`SessionRef::New` → 首轮；`SessionRef::Continue{id}` → **先**保留既有 `find_session(id)`（`ok_or_else("会话不存在: {id}")` 文案是既有契约，MUST NOT 被 `is_unstarted_session` 的内部 `Err` 顶替），**再**取 `is_unstarted_session(id)`（**只查一次**，与既有 `prior_handle` 分支共用同一布尔量，MUST NOT 二次查询）。
  4. 分支产出 `(prior_handle, stamp: Option<String>)`：
     - **首轮**（`New` 或 `is_unstarted`）：`is_unstarted` 臂保留既有 `restamp_session_config(id, &snapshot)` 与 `prior_handle = None`；`let body = freeze_preamble(&injections, &ctx); injections.preamble = injectable(body.clone()); stamp = Some(body);`。
     - **续轮已开轮**：保留既有跨引擎守卫（`record.config_snapshot.engine != self.store_kind` 即 `Err`）与 `prior_handle = record.engine_session_id.clone()`；随后 `match self.query.store().find_session_preamble(id).map_err(|e| e.to_string())?`：`Some(record)` → `injections.preamble = injectable(record.content)`（**不读文件、不算知识**，空正文 → `None`）；`None` → **存量补拍**：同首轮口径（`freeze_preamble` + 写入注入位 + `stamp = Some(body)`），`prior_handle` 不变。
  5. `config_snapshot` 序列化逐字保留；`kernel.begin_turn(... TurnRequest { session, question, injections, ctx, provenance, config_snapshot, prior_handle })` 逐字保留（`ctx` 在 `freeze_preamble` 之后才被移入）。
  6. `begin_turn` 返回后、`Ok(running)` 之前：`if let Some(body) = stamp { if let Err(error) = self.query.store().stamp_session_preamble(&running.session_id, &body) { crate::sdk::log::append_engine_log(&format!("会话系统提示词快照落库失败 session={} cause={error}", running.session_id)); } }`（**MUST NOT** 上抛、MUST NOT 影响 `running` 的返回）。
- [x] 新增两个私有助手（与 `begin` 相邻，不入导出面）：
  - `fn freeze_preamble(injections: &agent::SessionInjections, ctx: &agent::SessionCtx) -> String`：`let agent_md = crate::sdk::preamble::load(&ctx.workspace_root); crate::sdk::preamble::freeze(injections.preamble.as_deref(), agent_md.as_deref())`。doc：首轮 / 补拍共用；读取根 = 会话执行根（worktree 即分支版）；补拍取**补拍当轮**的知识（design 事实校准 4）。
  - `fn injectable(body: String) -> Option<String>`：`(!body.is_empty()).then_some(body)`。doc：空快照 ⇒ 零 System 消息。
- [x] `ComposedTurn::begin` 的 doc 增「三分支（首轮 / 续轮读库 / 存量补拍）+ 落库在 `begin_turn` 之后（`New` 路径只有此刻才知会话 id）+ 本轮不读回自己刚写的快照 + 落库失败仅日志」。
- [x] `assemble_injections` 的 doc 在 `preamble` 相关处补一句：「`preamble` 位在此按 provenance 产**知识段**；最终系统提示词由 [`ComposedTurn::begin`] 冻结合成后覆盖同一位（快照语义见 `desktop-agent-execution`“会话系统提示词快照的拍存与续轮复用”）」。

## 阶段五：引擎系统消息一元化（`crates/infra/agent/src/sdk/loop.rs`）

- [x] 删 `pub(crate) fn system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String>` 整个函数与其 doc 段（其中含 `AGENT.md` / `preamble::load` 字样）。
- [x] `use crate::sdk::{compact, context, normalize, policy, preamble, sandbox, tools, web_search};` 删去 `preamble`。
- [x] `continuation` 循环内的系统消息构造点改为（注释同步改写）：
  ```rust
  // 系统提示词 = 会话快照正文，逐字进恰一条 System；本模块零文件读取
  let mut chat_history = Vec::with_capacity(history.len() + 1);
  if let Some(preamble) = turn.injections.preamble.as_deref() {
      chat_history.push(Message::system(preamble));
  }
  chat_history.extend(history.iter().cloned());
  ```
- [x] 自查：`grep -n "AGENT.md\|preamble::load\|system_prompt" crates/infra/agent/src/sdk/loop.rs` ⇒ **零命中**；`turn.cwd` 的三处既有使用（沙箱 / 写保护 / `tools::execute`）逐字不动。

## 阶段六：IPC 类型再生成（`packages/desktop`）

- [x] `pnpm -C packages/desktop run bindings:export` 再生成 `src/types/generated/bindings.ts` 与 `bindings.zod.ts`。
- [x] 核对 diff **只含**：`SessionPreambleInfo` 类型块、`SessionSummary.preamble: SessionPreambleInfo | null`、`SessionPreambleInfoSchema`、`SessionSummarySchema` 的 `get preamble() { return SessionPreambleInfoSchema.nullable(); }`；其余零 diff（尤其 `SessionRow` / `SessionStats` / `TurnSummary` 不变）。**MUST NOT** 手改两份生成物。

## 阶段七：前端状态面字段（`packages/desktop/src/hooks/use-agent-session.ts`）

- [x] `sessionFromTurn` 返回字面补 `preamble: null`（放在 `stats` 之后、`turns` 之前或之后均可，按 `SessionSummary` 字段序对齐），并保持既有注释风格；零其他改动（`applyReplay` / `useSessionReplay` / 订阅面逐字不动）。

## 阶段八：前端只读可见面（`packages/desktop/src/components/agent/agent-session-view.tsx`）

- [x] 顶部 import 补 `useState`（`react`）与 `StandardDrawer`（`@/components/standard/drawer`）；类型 import 补 `SessionPreambleInfo`（`@/types/generated/bindings`，与既有 `SessionSummary` 同句）。
- [x] 新增组件内常量 `PREAMBLE_GUIDE`（一行静态指引文案，语义三点：本会话系统提示词冻结于会话开始、规则真身为 `AGENT.md`、需要更新时直接要求 agent 重新读取 `AGENT.md`）。**MUST NOT** 复用快照正文里的常量、MUST NOT 借 `SessionPanelState.note`。
- [x] 新增**私有**组件 `PreambleDrawer({ preamble }: { preamble: SessionPreambleInfo })`：
  - 自持 `const [open, setOpen] = useState(false)`；入口为 `Button variant="link" size="xs"`（`data-testid="session-meta-preamble"`，文案「系统提示词」）；`StandardDrawer` 受控（`title="系统提示词快照"`、`open`、`onOpenChange={setOpen}`、无 `trigger`）。
  - `content` 三段：① 元信息行 `拍存于 {new Date(preamble.savedAt).toLocaleString()}` + `{new TextEncoder().encode(preamble.content).length} 字节`；② `PREAMBLE_GUIDE` 静态指引；③ `<pre>` 全文（等宽 + `whitespace-pre-wrap` + `break-words`，交由抽屉既有 `ScrollArea` 滚动，可选中复制）。
  - 只读：零输入框、零提交 / 重载按钮、零 IPC、零订阅。
- [x] `SessionMeta`：在 tokens 位之后追加 `{summary.preamble !== null && <PreambleDrawer preamble={summary.preamble} />}`（`null` 时不渲染入口、不占位）。
- [x] 自查导出面：`export function AgentSessionView` / `export interface AgentSessionActions` / `export interface AgentSessionViewProps` 逐字不动，`PreambleDrawer` 与 `PREAMBLE_GUIDE` 均**不导出**（`knip` 面零增长）。
- [x] 自查函数规模：`SessionMeta` / `PreambleDrawer` 各自 ≤ 80 行（`max-lines-per-function` 对 `.tsx` 的阈值，skip blank / comments）。

## 阶段九：契约资产 prose（`.dev-team/architecture/models`）

- [x] `.dev-team/architecture/models/Desktop/Components/Agent/Desktop.Components.Agent.AgentSessionView.md`：「职能」段末补一句——元信息行在有快照时提供「系统提示词」只读入口（`data-testid="session-meta-preamble"`），抽屉展示拍存时刻 / 体量（按正文字节数就地算）/ 静态指引 / 全文，数据全取自 `SessionSummary.preamble`（零取数、零写面、不进消息流）。
- [x] 自查：`公共面` 表、「不变量」段、`## 契约面留白` 段与 `json api-types` 围栏**零改动**（`{}` 仍是 `{}`）；`.dev-team/architecture/graph/model/**` 与 `graph/INDEX.toml` **零 diff**（后端 `path` 是目录级、前端文件已在 `AgentSessionView` 的 `path` 内）。

## 阶段十：版本、边界自查与交接

- [x] `packages/desktop/package.json`：`version` `0.7.7` → `0.7.8`（`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 自动跟随，**MUST NOT** 手改；`src-tauri/Cargo.toml` 的 crate 版本行**不动**）。
- [x] 边界自查（只读命令，不跑门禁）：
  - `grep -rn "preamble::load\|AGENT.md" packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` ⇒ 零命中。
  - `grep -rn "async fn load" packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs` ⇒ 零命中。
  - `grep -rn "stamp_session_preamble\|find_session_preamble" packages/desktop/src-tauri/crates packages/desktop/src-tauri/src` ⇒ 生产命中恰在 `store.rs`（两方法定义）/ `compose.rs`（读 + 写）/ `store.rs`（`find_session_summary` 读）；`src/commands/**` 零命中。
  - `grep -rn "preamble" packages/desktop/src-tauri/crates/infra/store/src/store.rs` ⇒ `query_sessions` 路径零 `find_session_preamble` 调用（清单不读快照表的机械判据）。
  - `git status --porcelain` 的改动集 ⊆ design「变更清单」；`packages/desktop/src-tauri/src/commands/**`、`src-tauri/src/main.rs`、`crates/core/agent/src/{port,event,kernel}.rs`、`openspec/specs/**`、`.dev-team/architecture/graph/**` 零 diff。
- [x] 交接登记（不执行）：把 design 末节「交接清单」原样交给 test-design 相位；本相位不新建 `test-design.md`、不改任何 `*_test.rs` / `*.test.ts(x)`。
