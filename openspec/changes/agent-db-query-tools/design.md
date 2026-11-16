# 设计: agent-db-query-tools

> **变更**: agent-db-query-tools
> **状态**: 设计定稿（基于已定稿 proposal + 4 份 delta spec）
> **基于**: `proposal.md`（问题四现象 / 提案 / D1–D13 / 变更范围 / AC-1…AC-13 / 风险表）与 `specs/{desktop-agent-db-tools,desktop-agent-execution,desktop-archi-agent,desktop-change-chat}/spec.md` 与 `explore.md`（含真跑实测：5k / 50k 行库 + specta 类型图）
> **性质**: 纯增量**只读数据面**——store 侧新增 `describe` / `query` 两条读原语 + 注册表补齐 3 行；agent 侧新增两只挂载工具（`ToolExec::Db` / `ToolTier::Read`）+ per-turn `DbContext` 铸造 + explore / change 两臂名字清单 +2。事件词汇零新增、IPC 零新增、前端零改动、16 份相位提示词零改动、`crates/core/**` 零改动。
>
> **本篇边界**: `crates/infra/store/**`（schema 文档面 + 两条读原语 + 注册表 + 双库共享）、`crates/infra/agent/**`（执行体 / 挂载 / 分派 / 铸造 / 装配）、`packages/desktop/package.json`（版本一行）。**零新增 crate、零新增外部依赖、零新增依赖边、零 IPC / 生成物 / 前端改动**。

---

## 提案与规格同步状态

proposal 的四个现象、D1–D13 十三条裁决、变更范围与 AC-1…AC-13 均已定稿；4 份 delta spec 把语义钉到了「字段名 + 错误面 + 双档口径」粒度。本设计不重复论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名与模块**（见「关键设计决策」DD1–DD14）：`describe` / `query` 的模块落点、注册表如何同时喂「类型图」与「流式遍历」、`matched` 是否精确、`QueryError` 与 `StoreError` 的分野、`apiKey` 守卫落点、预算闸的原子性口径、`db_tables` 两形态输出形状、`DbContext` 的装配载体形态、嵌套路径校验降级、排序的 `null` / 跨类型口径。
2. **对 proposal 的四处措辞做工作树校准**（见「起点核实 - 事实校准」）：`#[specta(type = String)]` 只能落在 3 个字段而非 4 个、proposal 风险表的「早停」与 AC-5 的精确 `matched` 冲突、`envelope.rs` 的 `MAX_SCAN_LIMIT` 截断点位置、`WorkspaceStores.global` 改 `Arc` 的零调用点波及。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（用例与门禁由独立相位承接）。
4. **把三处「非显然正确」写成可实现形态并留痕**：`specta:type_override` 属性是 `specta_serde::Format` 放行 `#[serde(with)]` 的**唯一**机制（已核 `specta-macros` / `specta-serde` 源码）、`Format::map_types` 后命名引用的 id 存活（`Format::map_types` 是 `types.clone()` + 原地改属性，故引用可解析）、流式遍历必须复用**同一读事务**（否则 `total` / `matched` / `records` 非同快照，违 spec）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **store 读面**：`crates/infra/store/src/envelope.rs:58` `ScanFn`（四参 fn-pointer）/ `:60` `KeyOfFn` / `:64` 私有 `struct ModelEntry`（五字段）/ `:73` `MAX_SCAN_LIMIT = 500` / `:77` `MODEL_ENTRIES`（**11 行**）/ `:159` `list_models` / `:177` `scan`（`limit.min(MAX_SCAN_LIMIT)` 的截断在**这一层**，不在 `scan_model` 内）/ `:194` `scan_model`（**一个读事务**内 `len()` + `skip/take` 惰性扫描）/ `:324` `scan_steps` / `:387` `step_key`（注册表尾部）。
- **store 实例面**：`store.rs:2239` `Store::list_models` / `:2247` `Store::scan` / `:2298` `WorkspaceStores`（`global: Store` **值类型**、`workspaces_dir`、`cache: Mutex<HashMap<String, Arc<Store>>>`）/ `:2322` `global() -> &Store` / `:2331` `for_root() -> Arc<Store>`。**全部外部访问都经 `global()` 访问器**（`grep -rn "\.global\b"` 零直接字段访问），故 `global: Arc<Store>` 的改动只波及 `global()` 实现与 `open()`。
- **store 模型面**：`model.rs:782` `PhaseRecord`（缺 doc：`phase` / `attempt` / `skipped` / `stale` / `backtrack_reason` / `evaluator_session_id` / `decision_session_id`）/ `:824` `ChecklistItemRecord`（缺 `item` / `pass` / `evidence`；`:831` `item_key` 带 `#[serde(with = "event_key_serde")]`）/ `:915` `RunRecord` / `:960` `RunNodeRecord`（缺 `phase` / `attempt`；`:966` `node_key` 带 `serde(with)`）/ `:1063` `RunEdgeRecord`（`:1066` `edge_key` 带 `serde(with)`）/ `:556` `SessionEventRecord`（**唯一不 derive `Type`** 的表；`event_key` 亦带 `serde(with)`；三个字段 doc 已齐）。
- **store 出口面**：`lib.rs:8` `pub use envelope::{ModelInfo, RecordEnvelope, RecordsPage}` / `:9` `pub use model::{…}`（**无** `RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）。
- **agent 执行面**：`sdk/mount.rs:20` `TOOL_NAMES`（七件）/ `:48` `ToolTier` / `:61` `ToolExec`（`Pure` / `ExploreNote` / `Archi` / `FinishStep(S)` / `Ask(M)`）/ `:85` `REGISTRY: OnceLock<Vec<ToolMount>>` / `:95` `build_registry`（**11 项**，`Vec::with_capacity(11)`）/ `:143` `definitions`（`tools = None` 取**前七项**——故前七项必须是内建七件）/ `:8` `sdk/tools.rs` 的 `pub(crate) use crate::sdk::mount::{ARCHI_*}`（执行体取名的既有先例）/ `tools.rs:47` `input_path`（archi 三名显式 `None` 臂）/ `loop.rs:120` `LoopTurn`（`:143` `explore_note` / `:146` `archi` / `:149` `ask` 三上下文列）/ `loop.rs:662` 执行臂 `match mount.exec`（`ToolExec::Archi` 在 `:670`）/ `runner.rs:361` `struct PumpPayload` / `:393` `mint_archi_context` / `:401` `contains_tool` / `:410` `session_pump` / `:481` `LoopTurn` 铸造点（pump 内）/ `sdk/explore_note.rs:57` 与 `sdk/archi_tool.rs:19` 两个执行体上下文先例（`execute(&self, name: &str, input: &Value) -> Result<String, String>`，尾部过 `tools::cap_result`）。
- **agent 装配面**：`lib.rs:42` `struct EngineFacade`（字段面 + `new` / 七个 builder）/ `:149` `with_explore_note_store` / `:183` `with_workspace_root` / `:195` `runner_for`（`SdkRunner::new` 唯一调用点）/ `compose.rs:43` `assemble_injections` / `:45` `EXPLORE_TOOLS: [&str; 7]` / `:145` `archi_matrix` / `:215` `compose_turn`（`let explore_note_store = Arc::clone(&store);` 与 `StoreSink::new(store)` 的 move 次序）。
- **builder 字面构造面**：`EngineFacade` 的 `new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store` / `with_workspace_root` **七处是逐字段字面构造**（`with_ask_registry` / `with_ask_port` 用 `..self`）——新增字段须在这七处各补一行（`SdkRunner` 侧同构，但其 `new` 之外的 builder 全用 `..self`，只需给 `new` 补两行）。
- **版本**：`packages/desktop/package.json:3` `version = "0.6.43"`；`src-tauri/tauri.conf.json:4` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml:7` `version = "0.1.0"`（crate 版本行，MUST NOT 随动）。
- **依赖面**：workspace 根 `Cargo.toml:139/140` 已有 `specta = "=2.0.0-rc.26"`（features 含 `derive` / `serde_json` / `time`）与 `specta-serde = "=0.0.13"`；`crates/infra/store/Cargo.toml` 只引 `specta`，**缺 `specta-serde`**。

### 事实校准（proposal 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **`#[specta(type = String)]` 只能落 3 个字段**（proposal「变更范围」与 spec 均写 4 个）。`#[specta(...)]` 是 `#[derive(Type)]` 的 helper 属性，`SessionEventRecord` 不 derive `Type`（spec 明文「不强制补 derive」），对它的字段写该属性是**未知属性编译错误**。故：`item_key` / `node_key` / `edge_key` 三字段加属性；`event_key` 的线格式由 describe 侧手写 schema 以 `DataType::Primitive(str)` 表达（见 DD3），语义等价、无属性可加。该字段永不进类型图，`Format.map_types` 也不会见到它。
2. **放行机制已核到源码**：`specta-macros-2.0.0-rc.26/src/type/field.rs:69` 在 `#[specta(type = ...)]` 存在时插入运行期属性 `specta:type_override`；`specta-serde-0.0.13/src/validate.rs:737/1053` 对 `has_with` 走 `ensure_codec_override(has_type_override, …)`——即**只有**该属性在场才放行（错误文案自身即建议此解）。
3. **「早停」与 AC-5 的精确 `matched` 不可兼得**：proposal 风险表把「无 `order` + 无 `where` 可早停」当缓解手段，但 AC-5 / spec 要求 `matched` = 过 `where` 后的**行数**且「`returned < matched`」是行数档截断的判据。早停得到的 `matched` 只是下界，会把「还有 700 行」报成「恰好 20 行」——正是本变更要消灭的静默误导。设计改判见 DD4：**精确 `matched`，且无 `where` 无 `order` 时仍可早停**（该分支 `matched == total`，由 `len()` 精确给出并复用既有惰性 `skip/take`）。
4. **`MAX_SCAN_LIMIT` 的截断点在 `envelope::scan`（`:177`）而非 `scan_model`（`:194`）**：`scan_model` 本身对 `limit` 无钳位，故「`query` 不复用 500 上限」**无需改任何既有代码**——`query` 走自己的读路径（`walk` / 直接调 `(entry.scan)(db, offset, limit_for_query, key_of)`）即可，`MAX_SCAN_LIMIT` 与查看页分页口径零 diff。
5. **`WorkspaceStores.global: Store` → `Arc<Store>` 的波及面 = 3 处**：`open()` 构造、`global()` 实现（`self.global.as_ref()`）、新增 `global_shared()`；**零外部调用点改动**（全部经 `global()` 访问器，`compose.rs:370` / 40+ 测试夹具同形）。
6. **`EngineFacade` 七处逐字段字面构造**（见「起点核实」末第二项）是加字段的机械闭合面；`SdkRunner` 走 builder 旁路（`with_db_stores`）可零构造点波及——与 `with_workspace_root` 当初的选择同构。
7. **`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 未从 `store` 根导出**：注册表补行会引它们的 `Type` 定义，故 `lib.rs` 必须同批补 `pub use`（proposal 已列）。

---

## 关键设计决策

> D1–D13 见 proposal「裁决表」（已定稿，本篇不重复）。下表 DD1–DD14 是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | `describe` / `query` 的模块落点（proposal 待决问题） | 新建 `crates/infra/store/src/describe.rs`（类型图派生 + 类型渲染 + doc 截断 + 字段索引）与 `crates/infra/store/src/query.rs`（spec 词表 / 校验 / 过滤 / 排序 / 投影 / 分页）；`envelope.rs` 只保留并扩充**注册表**（补 3 行 + 两个 fn-pointer 字段 + 访问器 + `dimension_of`） | `envelope.rs` 已 389 行且职责是「注册表 + 分页信封」；把 ~120 行 schema 派生与 ~330 行查询执行塞进同一文件会让三类职责（模型登记 / 文档派生 / 查询执行）挤在一个模块，且两处都只被 infra 消费（不出线）。`op` 词表单点也自然落在 `query.rs`（拼 description 时 `store::QUERY_OPS` 直取） |
| DD2 | 注册表如何同时喂「类型图」与「流式遍历」 | `ModelEntry` 增两个 fn-pointer：`schema: SchemaFn`（`fn(&mut Types) -> DataType`，derive 型 = `describe::schema_of::<T>`，`session_event` = 手写）与 `walk: WalkFn`（`fn(&Database<'static>, KeyOfFn, &mut dyn FnMut(&Value) -> Result<bool, QueryError>) -> Result<u64, QueryError>`，逐条 `walk_*` 包装 14 个三行函数）；遍历在**一个读事务**内完成并返回全量计数 | 表↔类型映射**只能有一份**（spec：「MUST NOT 出现 per-table 手写清单或第二份字段表」）。方案对比：①一次性 `scan(limit = u32::MAX)` 物化全表——生产库 1.82 GB / ~70 万行会把 1.5 GB+ 的 JSON 值堆进内存（`scan_model` 先 `collect` 后转 JSON），否决；②分块多次 `scan`——每块一个独立读事务，`total` / `matched` / `records` 不再同快照（违 spec「三者同一读事务快照」），否决；③`walk` 逐条回调（选定）——同一事务、内存上界 = 保留窗口 |
| DD3 | `SessionEventRecord` 的 schema 供给 | `describe.rs::schema_session_event` 用手写 3 字段 schema：`eventKey` = `DataType::Primitive(str)`、`sessionId` = `Primitive(str)`、`event` = `DataType::Reference(Reference::opaque(未展开载荷标记))`；类型渲染把 opaque 渲染为 `object`；doc 三条文案与 `model.rs` 的 `///` 同源（同一段文字，ratchet 只机械校验**线名集 ↔ 真实 serde 键集**与「doc 非空」，不校验文案逐字） | spec 明文「不强制补 `Type` derive」「3 个字段说明单独供给」；`event` 是 `Box<RawValue>`（无 `Type` 实现），opaque 引用是 specta 对「本模型无法表达、由导出器决定」的既有逃逸舱，且 `specta-serde` 的 validate 对 `Reference::Opaque` 一律放行（`validate.rs` 多处 `Reference::Opaque(_) => {}`）。渲染成 `object` 而非 `any`：模型据此知道「是个对象，可点号下钻」 |
| DD4 | `matched` 的精确性（与 proposal 风险表的「早停」对账，见校准 3） | **恒精确**。两条路径：①`where` 空且无 `order` → 直接 `(entry.scan)(db, offset, limit, key_of)`（既有惰性 `skip/take`，`total = matched`，深翻成本 O(offset)）；②其余（有 `where` 或带 `order`）→ 全表 `walk`，`total` / `matched` 边扫边精确计数，带 `order` 时以「有序窗口保留 ≤ offset+limit 行」控内存 | 精确 `matched` 是双档收口「`returned < matched`」判据的唯一锚（AC-6），也是模型判断「要不要翻页」的唯一信号；牺牲它换早停会把「库里有 700 行」谎报成「20 行」。内存用「保留窗 ≤ offset+limit 行」（≈1020 行 × 1.8 KB ≈ 1.8 MB）而非全量物化；耗时是全表 decode 的固有成本（explore 实测 50k 行 93 ms，外推 70 万行 1.3~1.7 s，proposal 风险表已知情接受） |
| DD5 | `QueryError` 与 `StoreError` 的分野 | 新增 `pub enum QueryError { Invalid(String), Db(String) }`（`Display` = `invalid: …` / `db: …`）；`Store::describe` / `Store::describe_table` / `Store::query` 用 `QueryError`；`StoreError` 四变体**逐字不动**（`change_port.rs` 的四臂 exhaustive match 零 diff） | 查询面的失败语义是「模型自纠文本」（未知字段 → 候选线名 / 未知 op → 词表 / 类型不符 → 字段类型），与 change 域的 fault 分类（Conflict / NotFound 映射 `StoreFault`）毫无关系；塞进 `StoreError::Db` 会给模型看到误导性的 `db:` 前缀，新增变体则要改 `change_port` 的映射臂（越界且无收益） |
| DD6 | `apiKey` 双面守卫的落点 | 全住 agent 执行体（`db_tool.rs`）：①`where` / `order` 的路径任一段 == `apiKey` → `Err`（携「不可用于过滤 / 排序」）；②输出前对每条记录值递归打码（键名 `apiKey` 且值为字符串 → `"***"`，含 `select` 显式投影与嵌套出现）；store 侧零感知 | spec：「打码与预算政策 SHALL 住在 agent 执行体，store 读面保持纯数据语义」＋ proposal D13。递归打码是唯一覆盖「投影 / 嵌套 / 整条」三形态的单点；`where` / `order` 守卫必须与打码同址（否则前缀探测 oracle 仍在） |
| DD7 | 预算闸的原子性与计数口径 | 先在 `Vec<Value>` 上逐条累加 `serde_json::to_string(&record)?.len() + 1`，`used + n > RESULT_BUDGET_BYTES（40 KB）` 即停（**整条原子**）；随后整体 `serde_json::to_string(&envelope)` 一次成形（不可能出半截 JSON）；末尾再对成文长度做一次防御性裁剪（`> MAX_RESULT_BYTES(50 KB)` 时自尾部丢记录重序列化），确保 `cap_result` 对 `db_query` 永不实际触发 | 逐条计长后整体序列化：JSON 字符串转义在两种情形下逐字节一致，故预算与实际体积同口径；「先拼字符串再切」必然产生半截 JSON（spec 明令禁止）。防御性裁剪是双保险（envelope 字段 + hint 文案体积不可精确预算） |
| DD8 | `db_tables` 两形态的输出形状 | 无参 → **JSON 数组**（`[{dimension, table, count, summary}]`，`scope` 缺省两库全列、给定时单库）；带 `table` → **对象** `{table, dimension, summary, fields: [{name, type, doc}]}`。两形态字段集互不重叠（spec：「均不吐另一形态的字段」） | 数组/对象已是形态判别信号，无需再塞 `kind` 判别位；也避免「清单里塞空 fields 数组」这种既烧 token 又诱使模型误读的折衷 |
| DD9 | `db_tables` 表名解析与 `db_query` 的 scope 口径 | store 新增 `pub fn dimension_of(table: &str) -> Option<DbDimension>`（注册表直查）。`db_tables` 带 `table` 时按 `dimension_of` 派库（表名跨维度唯一由注册表保证），两库皆 miss → `Err` + **候选表名清单**。`db_query` 的 `scope` 缺省仍为 `workspace`（proposal D5 逐字），但当 `dimension_of(table)` 指向另一维度时 `Err` 携「该表属 user 维度，请加 `scope: \"user\"`」（不自作路由） | 缺省两库全列只对**清单**成立（spec 明文），下钻与取数必须落到单一库；把「表在另一个库」变成显式 `Err` 而非静默空结果，是「错误即自纠通道」的同一条纪律。自动按维度路由看似更方便，但会让 D5 的缺省语义变成「有时生效有时不生效」，模型无法预期 |
| DD10 | `DbContext` 的形态与装配缝（proposal 待决问题） | `pub(crate) struct DbContext { global: Arc<Store>, workspace: Arc<Store> }`（`Clone`）即**装配载体本身**：`EngineFacade` / `SdkRunner` 各存两个 `Option<Arc<Store>>`，`with_db_stores(self, global: Arc<store::Store>, workspace: Arc<store::Store>)` 一次置两位；`PumpPayload` 透传两位；pump 起播调 `mint_db_context(&open.injections, global, workspace) -> Option<DbContext>`（判据 = 名单含任一 DB 工具名，两位皆在场才铸） | builder 收两个具名 `Arc`（读代码即知取哪两库）；内部只搬一个结构体（`Option<DbContext>` 一位，与 `explore_note` / `archi` / `ask` 同列）。`mint_db_context` 独立成 `pub(crate) fn` 与 `mint_archi_context` 同形，判据可被 `runner_test` 直测（不需要真跑泵） |
| DD11 | 嵌套路径的静态校验降级（proposal 待决问题） | 顶层段（`where[].field` / `order.field` / `select[]` 的第一段）**静态校验**：必须命中该表线名集，否则 `Err` + 候选线名。第二段起**不做静态校验**（`event` 为 opaque、嵌入对象名可派生但值形态多样，静态深校验会把「合法但空」误判成错）。运行期解析不到 → 视作 `null`（`IS NULL` 可命中）。**补偿通道**：`where` 含点号路径且 `matched == 0` 时，返回体附 hint「嵌套路径（`x.y`）无法静态校验，确认字段名（本表顶层字段可用 db_tables 下钻）」 | 「静默零命中」是本节存在的理由，而深校验静态可判的只有顶层；把「路径不存在」报成 `Err` 会让过滤掉所有行变成常态失败。hint 是零额外扫描成本的补偿（`matched == 0` 本来就是要被质疑的时刻） |
| DD12 | 排序的比较口径（null / 跨类型 / 缺值） | `null`（含路径缺失）**恒排末位**（升序降序都一样：先比 `is_null`，null 恒 Greater）；同类比较：`number` 按 `as_f64`（NaN 视为相等）、`string` 按 `char` 码点序、`bool` false < true；跨类型按类型序 `number < string < bool < object < array < null`，同类型内按上述规则；`order.field` 路径缺失 = null | 全序、确定性、可测。`null` 恒末位与 spec 一致（「`null` 在排序中恒排末位」），跨类型定序避免「同一字段里混入数字与字符串」时 panic 或随机序 |
| DD13 | 说明文字截断的单点 | `describe.rs::truncate_doc`：取首个 `。` / `；` / `\n`（含该字符）为止；若无句读或该句读位置 > 120 字 → 截到 120 字（`char` 边界）；尾部空白剥离，被截断时补 `…`。**同一函数**用于表介绍（`summary`）与字段 `doc` | proposal D11；单点复用避免「表介绍按句读、字段按首行」两套口径漂移（`PhaseRecord.change_id` 与 `AgentRunRecord` 都是长文，形态不同但截断规则相同） |
| DD14 | 类型渲染与字段类型的**同一分类器** | `describe.rs` 提供 `render_type(&DataType, &Types) -> String`（人读文本）与 `field_kind(&DataType, &Types) -> FieldKind`（`Number` / `Text` / `Bool` / `Object` / `Unknown`，供 `query` 的值类型校验）；两者同处一文件、同走一棵 match。`Primitive` 按位宽收敛（整/浮 → `number`，`str`/`char` → `string`，`bool` → `boolean`）；`Nullable(inner)` → `"<inner> | null"`（kind 取 inner）；`Reference(Named)` → `types.get(r).name`（枚举名如 `ChangeStatus`），其 kind 由该命名的 `ty` 再判（`Enum` → `Text`（线值是字符串）、`Struct` → `Object`）；`List` → `"<T>[]"`（kind `Object`）；`Map` → `"{ [key: <K>]: <V> }"`；`Enum` → `string`；`Struct` / `Tuple` / `Intersection` → `object`；`Reference(Opaque)` / `Generic` → `object` / `Unknown` | 「一处说 string、另一处按 number 校验」是最难查的一类漂移；同一 match 同时产出展示文本与校验类，机械保证二者一致（`Nullable` 的 kind 取 inner 是 `verdict: Verdict` 与 `worktree: string \| null` 两种形态的共同需要） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 记录信封注册表（**扩**） | `MODEL_ENTRIES` 11 → **14** 行（补 `run` / `run_node` / `run_edge`，`store_meta` 仍排除）+ 三个 `key_of`；`ModelEntry` 增 `schema` / `walk` 两个 fn-pointer；`pub(crate)` 化 + `entry(table)` / `entries()` / `dimension_of(table)` 访问器；`walk` 包装 14 个三行函数 | `crates/infra/store/src/envelope.rs` | `native_db`、既有 `model` / `store` | fn-pointer 注册表（既有机制，零新抽象） |
| schema 文档派生（**新增**） | `Types::default()` + 每条 `(entry.schema)` → `specta_serde::Format.map_types` → 按 `NamedDataType` 取表介绍与逐字段（线名 / 渲染类型 / doc）；`truncate_doc`；`render_type` / `field_kind`；`FieldIndex`（供 `query` 校验）；`session_event` 手写 schema | `crates/infra/store/src/describe.rs`（新） | `specta`、`specta-serde`、`crate::envelope`、`crate::store` | 运行时类型图 + 薄 formatter |
| 查询原语（**新增**） | `QUERY_OPS` 词表 + `Op::parse`；`QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage` / `QueryError`；校验（表 / 顶层字段 / op / 值类型 / limit 上限）；过滤（AND / 点号下钻 / `LIKE` / `IN` / `IS NULL`）；排序（DD12）；投影（`key` 恒在）；分页（双路径，DD4） | `crates/infra/store/src/query.rs`（新） | `crate::envelope`、`crate::describe`、`serde_json` | 纯函数 + 单读事务 walk |
| 实例薄包装与双库共享（**扩**） | `Store::describe()` / `Store::describe_table(table)` / `Store::query(&spec)`；`WorkspaceStores.global: Arc<Store>` + `global_shared() -> Arc<Store>`（`global()` 语义不变） | `crates/infra/store/src/store.rs` | 既有 | 单行委托 |
| 两工具执行体（**新增**） | `DbContext { global, workspace }`；`execute(name, input)`：`db_tables` 两形态 / `db_query` 的 scope 解析 → 形状解析 → 敏感字段守卫 → `store::Store::{describe*, query}` → 预算闸 → 打码 → 结果成形；错误文案装配（候选表 / 候选线名 / op 词表 / 类型文本 / 下标指引） | `crates/infra/agent/src/sdk/db_tool.rs`（新）+ `sdk/mod.rs` 一行 `mod` | `store`、`serde_json`、`crate::sdk::{mount, tools}` | 与 `explore_note` / `archi_tool` 同构（context + `async fn execute`，尾部 `cap_result`） |
| 挂载单源（**扩**） | `DB_TABLES_TOOL` / `DB_QUERY_TOOL` 常量 + 两个 `ToolDefinition`（description 覆盖 op 词表 / scope / 预算 / 自读警示）+ `ToolExec::Db` + tier `Read`；清单 11 → **13** 项（`TOOL_NAMES` 七件与前七项次序不动） | `crates/infra/agent/src/sdk/mount.rs` | 既有 + `store::QUERY_OPS` | 静态挂载清单（既有机制） |
| 沙箱改写豁免（**扩**） | `input_path` 增两 DB 工具名显式 `None` 臂（照 archi 三名先例） | `crates/infra/agent/src/sdk/tools.rs` | 既有 | 哑元臂 |
| 执行分派（**扩**） | `LoopTurn.db: Option<DbContext>`；`ToolExec::Db` 臂路由 `DbContext::execute`，上下文缺席以 `is_error` 回灌（生产不可达） | `crates/infra/agent/src/sdk/loop.rs` | 既有守卫链 | 既有守位链第 6 段（成员校验 / tier / 沙箱 / 保护闸全零改动） |
| per-turn 铸造（**扩**） | `PumpPayload` 增两库句柄；`mint_db_context(injections, global, workspace)`（判据 = 名单含任一 DB 工具名且两把手在场）；pump 起播铸造并随 `LoopTurn` 下发 | `crates/infra/agent/src/sdk/runner.rs` | `sdk/db_tool` | 既有铸造位（与 `mint_archi_context` 并列的第二个名字判据） |
| 装配缝（**扩**） | `EngineFacade` 增两 `Option<Arc<Store>>` 字段 + `with_db_stores(...)`；`SdkRunner` 同名 builder；`compose_turn` 从 `stores.global_shared()` 与既有 `store: Arc<Store>` 组装 | `crates/infra/agent/src/{lib,compose}.rs`、`sdk/runner.rs` | `store::WorkspaceStores` | 与 `with_explore_note_store` / `with_workspace_root` 同型旁路缝 |
| 两臂名字清单（**扩**） | explore 臂 `EXPLORE_TOOLS` 7 → **9**；change 臂在矩阵并入之后恒 push 两名（不分 phase / 不分矩阵命中） | `crates/infra/agent/src/compose.rs` | 既有 | 名字清单单源 |

<!-- 无新增 crate、无新增依赖边、零架构模型元素改动（App.Backend.Infra.Store / App.Backend.Infra.AgentRuntime
     两元素均 `api = none`，新增公共面只在 api:check 的 apiFindings 槽位留痕；.dev-team/architecture/models/** 零改动）。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀。测试文件与用例归测试相位，本清单只列**实现文件**，另设「编译闭合批」登记必须随字段面 / 签名收敛的夹具点位，以及必须随注册表 / 挂载清单行数收敛的既有测试期望值点位。

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `crates/infra/store/src/describe.rs` | 表 / 字段说明的机械派生：`ModelDoc` / `FieldDoc` / `TableDoc`、`schema_of::<T>` / `schema_session_event`、`tables()` / `table()` / `field_index()`、`render_type` / `field_kind` / `truncate_doc`、`FieldKind` / `FieldIndex`。doc 写法与 `archi_tool.rs` 同风格（模块头写「为什么说明只能来自 doc 注释 + 为什么必须应用 `Format`」） |
| `crates/infra/store/src/query.rs` | 只读查询原语：`QUERY_OPS`（10 词单点）、`Op` 解析、`QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage`、`QueryError`、校验链、过滤 / 排序 / 投影 / 分页执行（DD4 双路径）、`compare_values` / `resolve_path` / `project` |
| `crates/infra/agent/src/sdk/db_tool.rs` | 两工具执行体：`DbContext`、`Scope`、`RESULT_BUDGET_BYTES` / `ENVELOPE_RESERVE_BYTES`、`execute`、`db_tables` 两形态、`db_query` 全链、`mask_api_key`、错误文案装配（候选表 / 候选线名 / 词表 / 类型 / 下标） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/store/Cargo.toml` | `[dependencies]` 增 `specta-serde = { workspace = true }` | 单 crate 构建下 `serde` 重命名才会被 `Format` 应用（不指望壳层 feature 统一）；workspace 条目已在案，零新外部依赖 |
| `crates/infra/store/src/model.rs` | ①补 12 条字段 `///`（`PhaseRecord`: `phase` / `attempt` / `skipped` / `stale` / `backtrack_reason` / `evaluator_session_id` / `decision_session_id`；`ChecklistItemRecord`: `item` / `pass` / `evidence`；`RunNodeRecord`: `phase` / `attempt`）；②`ChecklistItemRecord.item_key` / `RunNodeRecord.node_key` / `RunEdgeRecord.edge_key` 加 `#[specta(type = String)]`（**不加**在 `SessionEventRecord.event_key`——见校准 1） | 说明来源 = doc（proposal D2）；模型版本号、`native_model` 头、字段面、落库编码**零改动**（`///` 不进线格式） |
| `crates/infra/store/src/envelope.rs` | ①`MODEL_ENTRIES` 11 → 14 行（`run` / `run_node` / `run_edge`，`dimension = Workspace`）；②新增 `run_key`（`value["runId"]`）/ `run_node_key`（`{runId, nodeKey}`）/ `run_edge_key`（`{runId, edgeKey}`）；③`struct ModelEntry` → `pub(crate)` 且字段 `pub(crate)`，增 `schema: SchemaFn` / `walk: WalkFn` 两字段（14 行各补两行）与两个类型别名；④`scan_model` 之后新增 `walk_model::<T>` 通用实现 + 14 个 `walk_*` 三行包装；⑤新增 `pub(crate) fn entries()` / `pub(crate) fn entry(table: &str)` / `pub fn dimension_of(table: &str) -> Option<DbDimension>` | 注册表仍是唯一「表 → { 维度 / 计数 / 分页 / 主键投影 / 类型图 / 遍历 }」映射；`list_models` / `scan` / `MAX_SCAN_LIMIT` 逐字不动（AC-8 / 查看页口径） |
| `crates/infra/store/src/store.rs` | ①`impl Store` 增 `pub fn describe(&self) -> Result<Vec<ModelDoc>, QueryError>` / `pub fn describe_table(&self, table: &str) -> Result<TableDoc, QueryError>` / `pub fn query(&self, spec: &QuerySpec) -> Result<QueryPage, QueryError>`（三行委托）；②`WorkspaceStores.global: Store` → `Arc<Store>`；`open()` 用 `Arc::new(...)`；`global()` 改 `self.global.as_ref()`；新增 `pub fn global_shared(&self) -> Arc<Store>` | `global()` 返回类型不变（`&Store`）→ 既有 40+ 调用点零改动（校准 5）；`for_root` 缓存与单开纪律零改动 |
| `crates/infra/store/src/lib.rs` | `mod describe;` / `mod query;`（字母序置 `canonical` 之后、`envelope` 之前）；`pub use describe::{FieldDoc, ModelDoc, TableDoc};`、`pub use envelope::{dimension_of, …}` 增项、`pub use model::{… RunEdgeRecord, RunNodeRecord, RunRecord};`、`pub use query::{QueryCond, QueryError, QueryOrder, QueryPage, QuerySpec, QUERY_OPS};` | 公共读面词汇（**不 derive `specta::Type`** → 零 IPC / 零 bindings / 零前端改动）；补导出 run 三表（工具覆盖 run 史必须） |
| `crates/infra/agent/src/sdk/mod.rs` | `pub(crate) mod db_tool;`（字母序落在 `context` 与 `explore_note` 之间） | **不加** `#[cfg(test)] mod db_tool_test;`（声明与 sidecar 同批的仓规，由 test-gen 落地） |
| `crates/infra/agent/src/sdk/mount.rs` | ①`pub(crate) const DB_TABLES_TOOL: &str = "db_tables";` / `DB_QUERY_TOOL = "db_query";`；②`ToolExec` 增 `Db` 变体；③`build_registry` 尾部追加两项（`tier: ToolTier::Read`、`exec: ToolExec::Db`），`Vec::with_capacity(11)` → `13`；④`db_tables_definition()` / `db_query_definition()`（description：`scope` 两值 + 缺省口径、`where` / `order` / `select` / `offset` / `limit` 语义、`op` 词表由 `vocabulary(store::QUERY_OPS)` 拼接、`limit` 缺省 20 / 上限 1000 且「预算是实际生效值」、`session` / `session_event` 自读警示、表清单先用 `db_tables`）；⑤`registry()` / `build_registry()` doc 的「十一项」→「十三项」 | 定义与档位唯一真源在挂载清单（core 只持名字）；`TOOL_NAMES` 七件与「前七项」次序不动（`tools = None` 语义） |
| `crates/infra/agent/src/sdk/tools.rs` | `pub(crate) use` 名单补两个 DB 常量（与 archi 三名同址）；`input_path` 的显式 `None` 臂扩到五名 | 路径沙箱面对两工具恒原样返回（无路径参数），未来入参改名也不会被误改写 |
| `crates/infra/agent/src/sdk/loop.rs` | ①`LoopTurn` 增 `pub db: Option<DbContext>`（紧随 `ask` / 与另三上下文同列）；②执行臂 `match mount.exec` 增 `ToolExec::Db => match &turn.db { Some(ctx) => ctx.execute(&name, &input).await 双态折叠, None => ("专用工具执行上下文未装配".to_owned(), true) }` | 守卫链四段（成员校验 / tier / 沙箱 / 保护闸）与 `ToolExec::{Pure, ExploreNote, Archi, FinishStep, Ask}` 分支语义零改动 |
| `crates/infra/agent/src/sdk/runner.rs` | ①`PumpPayload` 增 `db_global: Option<Arc<Store>>` / `db_workspace: Option<Arc<Store>>`；②新增 `pub(crate) fn mint_db_context(injections: &agent::SessionInjections, global: Option<Arc<Store>>, workspace: Option<Arc<Store>>) -> Option<DbContext>`；③`session_pump` 起播铸造 `let db = mint_db_context(&open.injections, payload.db_global.clone(), payload.db_workspace.clone());` 并在 `LoopTurn` 铸造点补 `db,`；④`SdkRunner` 增两 `Option<Arc<Store>>` 字段 + `with_db_stores(self, global: Arc<Store>, workspace: Arc<Store>) -> Self`（builder），`open_session` 填入 `PumpPayload`；⑤`use` 面补 `crate::sdk::db_tool::{DbContext, mint…}` | `SdkRunner::new` 签名**不动**（builder 旁路，与 `with_workspace_root` 同构，零构造点波及）；泵「一轮一命」与 `select!` 两臂零改动 |
| `crates/infra/agent/src/lib.rs` | ①`EngineFacade` 增 `db_global: Option<Arc<store::Store>>` / `db_workspace: Option<Arc<store::Store>>`；②`new` 与六个逐字段 builder 各补一行 `db_global: …` / `db_workspace: …`；③新增 `pub fn with_db_stores(self, global: Arc<store::Store>, workspace: Arc<store::Store>) -> Self`（`..self` 形态）；④`runner_for` 的 `SdkRunner` 链上按在场性 `.with_db_stores(g, w)` | 七处字面构造是字段面的机械闭合（校准 6）；`runner_for` 签名零改动 |
| `crates/infra/agent/src/compose.rs` | ①`compose_turn` 内 `let db_workspace = Arc::clone(&store);`（与既有 `explore_note_store` 相邻、在 `StoreSink::new(store)` move 之前）+ `let db_global = stores.global_shared();`；门面链尾 `.with_db_stores(db_global, db_workspace)`；②`EXPLORE_TOOLS: [&str; 7]` → `[&str; 9]`（+ `mount::DB_TABLES_TOOL` / `mount::DB_QUERY_TOOL`）；③change 臂：`archi_matrix` 并入之后恒 `names.push(mount::DB_TABLES_TOOL); names.push(mount::DB_QUERY_TOOL);` | change 臂两名与 `ask` 位恒 `Flow` 同构（不分 phase / 不分矩阵命中 / 畸形 `source_ref` 同样命中）；`archi_matrix`、archi 臂、debug 臂、成员校验语义零改动 |
| `packages/desktop/package.json` | `version` `0.6.43` → `0.6.44`（AC-13，归档时执行） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行 MUST NOT 随动 |

### 编译闭合批（随字段面 / 签名 / 行数期望收敛的点位；MUST NOT 新增用例 / 改断言口径）

| 面 | 点位 | 改动 |
|----|------|------|
| `LoopTurn` 字面构造 | `sdk/runner.rs`（生产，1 处）+ `sdk/loop_test.rs` / `sdk/runner_test.rs` 等既有夹具（约 40+ 处，编译器可全量列出） | 补 `db: None,` |
| `PumpPayload` 字面构造 | `sdk/runner.rs`（生产，1 处）+ 其测试夹具（若直构） | 补 `db_global: None, db_workspace: None,` |
| `EngineFacade` 字面构造 | `lib.rs` 七处（生产） | 各补两行字段 |
| `WorkspaceStores` 直构 | `store_test.rs` 等（若以字面构造绕开 `open()`） | 不可绕开——`global` 为私有字段，`WorkspaceStores` 只能经 `open()` 构造（实测无字面构造点）；`global()` 语义不变故 `global_shared()` 之外零改动 |
| `&str` 名单常量 | `compose_test.rs` 既有断言（explore 七名 / change 名单） | 断言口径随 spec 更新归测试相位；本清单只登记「编译不受影响」这一事实（数组长度变化在断言侧才暴露） |
| 挂载清单行数期望 | `crates/infra/agent/src/sdk/policy_test.rs`（既有用例「十一项挂载逐工具三档等价于原三张按名常量表」） | 用例名 `十一项` → `十三项`；`Read` 档名单行尾部补 `db_tables` / `db_query`（与原 `READONLY_TOOLS` 六名同列三档全放）；`Write` / `Execute` 档名单与三档 × 四 tier 矩阵**零改动**。这是「挂载清单 11 → 13 项」的必然投影 |
| 注册表行数期望 | `src/commands/db/mod_test.rs`（既有用例「`db_models_scope_workspace` 返回八行且与直连 serde 一致」与「两库清单互不混列」） | workspace 维度 `db_models` 的计数向量与名集尾部补 `run` / `run_node` / `run_edge`（8 → **11** 行）；两处用例名与断言说明文案同步（`八行` → `十一行`）；user 维度三行、跨维度扫描 `Err`、blank root 空结果、坏文件 `Err` 等其余断言**零改动**。这是「`MODEL_ENTRIES` 11 → 14 行」在查看页命令层的必然投影（既有「新模型零改动可浏览」红利） |

> **闭合批的时点不变量**：任一处字段面变化，其**全部**构造点必须在**同一阶段批次**内收敛；任一处行数（`MODEL_ENTRIES` / 挂载清单）变化，其**全部**期望值点位必须在同一批内收敛——阶段末「工作区可编译 + clippy 全绿 + 行数期望零漂移」是阶段划分的不变量，MUST NOT 靠收窄自查口径（如把 `src/commands/**` 整目录或 `sdk/{policy,…}.rs` 通配一并声明「零 diff」）绕过。末两行的共同性质是「既有测试的期望值随动」：不新增用例、不改断言语义、不触生产命令面（`src/commands/db/mod.rs` 的 `db_models` / `db_records` 实现零改动）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `MODEL_ENTRIES`（访问器） | `crates/infra/store/src/envelope.rs` | 新增 | `pub(crate) fn entries() -> &'static [ModelEntry]` | 注册表只读出口（`describe` / `query` 过滤维度用；`ModelEntry` 同时 `pub(crate)` 化） |
| `dimension_of` | 同上 | 新增 | `pub fn dimension_of(table: &str) -> Option<DbDimension>` | 表名 → 维度（跨维度唯一由注册表保证）；`db_tables` 下钻派库与 `db_query` 的维度自纠文案同源 |
| `Store::describe` | `crates/infra/store/src/store.rs` | 新增 | `pub fn describe(&self) -> Result<Vec<ModelDoc>, QueryError>` | 本库维度表清单（`table` / `dimension` / `count` / `summary`），`store_meta` 不在注册表故天然排除 |
| `Store::describe_table` | 同上 | 新增 | `pub fn describe_table(&self, table: &str) -> Result<TableDoc, QueryError>` | 单表全字段（线名 / 渲染类型 / doc）；未知表 `Err` + 候选表名（本库维度） |
| `Store::query` | 同上 | 新增 | `pub fn query(&self, spec: &QuerySpec) -> Result<QueryPage, QueryError>` | 过滤 → 排序 → 投影 → 分页（`limit` 缺省 20 / 上限 1000，**不复用** `MAX_SCAN_LIMIT`）；`total` / `matched` / `records` 同一读事务快照 |
| `WorkspaceStores::global_shared` | 同上 | 新增 | `pub fn global_shared(&self) -> Arc<Store>` | 全局库 `Arc` 出口（redb 单进程单句柄 → MUST NOT 二次 `open`；`global()` 语义不变） |
| `describe::tables` | `crates/infra/store/src/describe.rs` | 新增（`pub(crate)`） | `pub(crate) fn tables(db: &Database<'static>, dimension: DbDimension) -> Result<Vec<ModelDoc>, QueryError>` | 注册表按维度过滤 → 逐条 `count` + `summary`（不展开字段） |
| `describe::table` | 同上 | 新增（`pub(crate)`） | `pub(crate) fn table(db: &Database<'static>, dimension: DbDimension, name: &str) -> Result<TableDoc, QueryError>` | 未知表 `Err` 携本维度候选表名 |
| `describe::field_index` | 同上 | 新增（`pub(crate)`） | `pub(crate) fn field_index(table: &str) -> Result<FieldIndex, QueryError>` | 该表线名 / 渲染类型 / `FieldKind` 三元组集合，`query` 校验用（与 `db_tables` 输出同源） |
| `describe::schema_of` | 同上 | 新增（`pub(crate)`） | `pub(crate) fn schema_of<T: specta::Type>(types: &mut Types) -> DataType` | 注册表 `schema` 字段的 derive 型实现（`T::definition(types)`） |
| `query::run` | `crates/infra/store/src/query.rs` | 新增（`pub(crate)`） | `pub(crate) fn run(db: &Database<'static>, dimension: DbDimension, spec: &QuerySpec) -> Result<QueryPage, QueryError>` | DD4 双路径执行体；`Op::parse` / 校验 / 比较 / 投影全在本模块 |
| `DbContext::new` | `crates/infra/agent/src/sdk/db_tool.rs` | 新增 | `pub(crate) fn new(global: Arc<Store>, workspace: Arc<Store>) -> Self` | 双库句柄（均为 `WorkspaceStores` 交出的既有实例，MUST NOT 二次 open） |
| `DbContext::execute` | 同上 | 新增 | `pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String>` | 与 `ArchiContext::execute` 同形：`db_tables` / `db_query` 两臂，未知名 `Err`；尾部 `tools::cap_result` |
| `mint_db_context` | `crates/infra/agent/src/sdk/runner.rs` | 新增（`pub(crate)`） | `pub(crate) fn mint_db_context(injections: &agent::SessionInjections, global: Option<Arc<Store>>, workspace: Option<Arc<Store>>) -> Option<DbContext>` | per-turn 铸造判据单点（与 `mint_archi_context` 并列的第二个名字判据；两把手任一缺席即 `None`） |
| `EngineFacade::with_db_stores` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_db_stores(self, global: Arc<store::Store>, workspace: Arc<store::Store>) -> Self` | 组合根注入缝（与 `with_explore_note_store` 同型）；`runner_for` 透传 |
| `SdkRunner::with_db_stores` | `crates/infra/agent/src/sdk/runner.rs` | 新增 | `pub fn with_db_stores(self, global: Arc<Store>, workspace: Arc<Store>) -> Self` | builder 旁路（`new` 签名不动，与 `with_workspace_root` 同构） |
| `tools::input_path` | `crates/infra/agent/src/sdk/tools.rs` | 修改 | `pub fn input_path<'a>(name: &str, input: &'a Value) -> Option<&'a str>` | 显式 `None` 臂扩到五名（archi 三名 + 两 DB 工具名） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ModelDoc` | `crates/infra/store/src/describe.rs` | 新增 | `{ pub table: String, pub dimension: DbDimension, pub count: u64, pub summary: String }`；`Debug + Clone + PartialEq`（**不** derive `Type` 与 `Serialize`） |
| `TableDoc` | 同上 | 新增 | `{ pub table: String, pub dimension: DbDimension, pub summary: String, pub fields: Vec<FieldDoc> }` |
| `FieldDoc` | 同上 | 新增 | `{ pub name: String, pub ty: String, pub doc: String }`（`name` = 落库线名） |
| `FieldKind` | 同上 | 新增 | `enum { Number, Text, Bool, Object, Unknown }`（`pub(crate)`；`query` 值类型校验用，与 `render_type` 同源分类器） |
| `FieldIndex` | 同上 | 新增 | `pub(crate) struct { fields: Vec<(String, FieldKind, String)> }` + `names()` / `kind_of(path)` / `render_of(path)`；顶层可查，嵌套返回 `Unknown` |
| `QuerySpec` | `crates/infra/store/src/query.rs` | 新增 | `#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase", deny_unknown_fields)]`；`{ pub table: String, #[serde(rename = "where", default)] pub conds: Vec<QueryCond>, pub order: Option<QueryOrder>, pub select: Option<Vec<String>>, #[serde(default)] pub offset: u32, #[serde(default = "default_limit")] pub limit: u32 }`（`default_limit()` = 20；`Option` 两字段缺省即 `None`，`offset` / `limit` 走 `default`） |
| `QueryCond` | 同上 | 新增 | `#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase", deny_unknown_fields)]`；`{ pub field: String, pub op: String, #[serde(default)] pub value: Option<Value> }`（`op` 保持 `String`：封闭词表由 `Op::parse` 单点判定，`Err` 才能列全词表） |
| `QueryOrder` | 同上 | 新增 | `{ pub field: String, #[serde(default)] pub desc: bool }` |
| `QueryPage` | 同上 | 新增 | `{ pub total: u64, pub matched: u64, pub records: Vec<RecordEnvelope> }`（`RecordEnvelope` 复用既有信封，`key` / `value` 均为 JSON 值） |
| `QueryError` | 同上 | 新增 | `pub enum { Invalid(String), Db(String) }`，`Display` = `invalid: …` / `db: …`（DD5；不进 `StoreError`） |
| `QUERY_OPS` | 同上 | 新增 | `pub const QUERY_OPS: [&str; 10] = ["=", "!=", ">", ">=", "<", "<=", "LIKE", "IN", "IS NULL", "IS NOT NULL"]`（`mount` 拼 description 与 `Op::parse` 的同一词表） |
| `ModelEntry` | `crates/infra/store/src/envelope.rs` | 修改 | 由私有四元组改为 `pub(crate)`：`{ name, dimension, count, scan, key_of, schema: SchemaFn, walk: WalkFn }`；两个别名 `type SchemaFn = fn(&mut Types) -> DataType;` / `type WalkFn = fn(&Database<'static>, KeyOfFn, &mut dyn FnMut(&Value) -> Result<bool, QueryError>) -> Result<u64, QueryError>;` |
| `ToolExec::Db` | `crates/infra/agent/src/sdk/mount.rs` | 新增（变体） | 无载荷变体（两工具共用一个执行体，工具名由 loop 侧传入——与 `Archi` 同形） |
| `LoopTurn.db` | `crates/infra/agent/src/sdk/loop.rs` | 新增（字段） | `pub db: Option<DbContext>`（`None` = 未装配 → `is_error` 回灌） |
| `PumpPayload.db_global` / `db_workspace` | `crates/infra/agent/src/sdk/runner.rs` | 新增（字段） | `Option<Arc<Store>>` 各一（per-turn 铸造原料） |
| `EngineFacade.db_global` / `db_workspace` / `SdkRunner.db_global` / `db_workspace` | `crates/infra/agent/src/lib.rs` / `sdk/runner.rs` | 新增（字段） | 私有 `Option<Arc<store::Store>>` 各一 |
| `DbContext` | `crates/infra/agent/src/sdk/db_tool.rs` | 新增 | `pub(crate) struct { global: Arc<Store>, workspace: Arc<Store> }`（`Clone`；字段私有，取值范围只经两库解析单点） |

<!-- 配置：本变更不新增 / 不修改任何配置键——.dev-team/config.json、config.schema.json、
     provider 记录字段面、引擎 EngineConfig 字段面均零改动（工具面走名字清单通道，无开关）。 -->

---

## 数据模型

### `db_query` 的请求 / 响应（工具面 JSON，非 IPC、不入库）

请求（`QuerySpec` 的线形态；`scope` 由执行体取出后不参与 store 反序列化）：

| 键 | 类型 | 必需 | 缺省 | 语义 |
|----|------|------|------|------|
| `table` | `string` | ✅ | — | 注册表表名（14 张之一） |
| `scope` | `"user"` \| `"workspace"` | ❌ | `workspace` | 取哪一库；与表维度不符时 `Err` 携改正提示（DD9） |
| `where` | `QueryCond[]` | ❌ | `[]`（全量） | 多项 **AND**；字段 = 落库线名，支持点号下钻（无数组下标） |
| `where[].field` | `string` | ✅ | — | 顶层段静态校验（未知 → `Err` + 候选线名） |
| `where[].op` | `string` | ✅ | — | `QUERY_OPS` 封闭词表（未知 → `Err` + 全词表） |
| `where[].value` | 任意 JSON | ❌ | 视同 `null` | `IS NULL` / `IS NOT NULL` 携带非 null 值 → `Err`（显式 `null` 等价省略，JSON 语义下不可区分） |
| `order` | `{ field, desc? }` | ❌ | 无（= 主键自然序） | 给出即全表排序后切页；`null` 恒末位（DD12） |
| `select` | `string[]` | ❌ | 无（= 全字段） | 投影路径集；`key` 恒在、不占名额 |
| `offset` | `number` | ❌ | 0 | 深翻偏移 |
| `limit` | `number` | ❌ | 20 | 上限 1000；越界 `Err`（不钳位） |

响应（执行体成形）：

| 键 | 类型 | 语义 |
|----|------|------|
| `table` | `string` | 回显表名 |
| `total` | `number` | 该表全量计数（同一读事务快照） |
| `matched` | `number` | 过 `where` 后的行数（**精确**，DD4） |
| `returned` | `number` | 本页实际返回行数（`returned ≤ matched ≤ total`） |
| `truncated` | `boolean` | 任一拍收口触发 |
| `truncatedBy` | `"limit"` \| `"bytes"` \| `null` | 触发者（未触发为 `null`） |
| `records` | `[{ key, value }]` | `key` 恒在（主键投影，与 `RecordsPage` 同形）；`value` 为记录 JSON（`select` 时经投影、`apiKey` 已打码） |
| `hint` | `string`（可缺省） | 截断时的收缩指引（`select` / `where` / `offset`）或 `matched == 0` 时的嵌套路径提示（DD11） |

### `db_tables` 的两种响应形态（DD8）

| 形态 | 触发 | 形状 |
|------|------|------|
| 清单 | 无 `table` | `[{ dimension, table, count, summary }]`（`scope` 缺省 = user 3 + workspace 11 = 14 行；`store_meta` 不出现） |
| 下钻 | 有 `table` | `{ table, dimension, summary, fields: [{ name, type, doc }] }`（`name` = 落库线名，`type` = 渲染文本，`doc` = 截断后说明） |

### 说明的来源链（零手写、零漂移）

```
<T as specta::Type>::definition(&mut Types)
  → DataType::Reference(Named)   ── Format::map_types（types.clone() + 原地改属性，引用 id 存活）
  → NamedDataType { docs = struct ///, ty: Struct(NamedFields) }
       → 每字段 { 线名（camelCase，serde rename_all 经 Format 应用）, docs = ///, ty = DataType }
                                                    │
                        render_type / field_kind ────┴──→ db_tables 的 type 文本 / query 的值类型校验
```
`session_event` 一支走 DD3 的手写 schema（3 字段，`event` 为 opaque → 渲染 `object`）。

### 持久化与关系（零改动）

| 项 | 现状 | 本变更 |
|----|------|--------|
| 记录模型 / `native_model` 版本头 / 落库编码 | 14 表既有 | **零改动**（`///` 与 `#[specta(type = …)]` 不进线格式；`model_test` 的逐字节对拍由测试相位复验） |
| 数据库文件 / 维度划分 | `desktop-global.redb`（user 3）+ `workspaces/<identity>.redb`（workspace 11 已注册） | 零改动；注册表补 3 行使 workspace 库的**已存在**三表可浏览 / 可查询（原生 `workspace_models()` 十二模型中的 run 三表） |
| 读写纪律 | redb 单进程单句柄；`Store::open_global` 单点；`for_root` 缓存 | 零改动；`global: Arc<Store>` 只改所有权形态，MUST NOT 二次 `open`（AC-10） |
| 写面 | 既有写 API | **零新增**（`describe` / `query` 全读事务；AC-8） |
| 事件 / 转录 / 编排状态 | `AgentEvent` 六变体、`session_event` 落库、change 状态机 | 零改动（工具调用与结果走既有密封事件通道） |

---

<!-- 路由 / API 设计：本变更零 IPC 命令、零 HTTP 端点（工具面不是 IPC——它经既有 agent
     工具调用通道，工具定义在 infra 挂载清单，不经 specta 出线）。故本段省略。 -->

---

## 依赖

- **运行时依赖**：`crates/infra/store/Cargo.toml` 增 `specta-serde = { workspace = true }`（workspace 根 `Cargo.toml:140` 已有 `specta-serde = "=0.0.13"`，**零新外部依赖、零版本上升**）。用途：应用 serde 重命名（`camelCase` 线名）并展平类型图，是「说明 / 类型 / 线名」机械派生的唯一入口。`store` 已有的 `specta`（features `derive` / `serde_json` / `time` 由 workspace 条目给定）提供 `Types` / `DataType` / derive。
- **依赖方向**：`agent-runtime → store`（既有）＋`store → {agent, workflow, foundation}`（既有），**零新依赖边**（架构模型 `dependsOn` 零变化）。`specta-serde` 是 `store` 的新叶依赖，与 `specta` 同族。
- **构建 / 测试依赖**：**零新增**。`describe` / `query` 的自测装置只用既有 `tempfile` / `native_db`（`store` 的 dev-dependencies 已在案）；agent 侧 `db_tool` 自测用既有 `tempfile` 打开的库实例。**无 feature 开关变化**。
- **前端依赖**：零变化（不 produce 任何生成物）。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 `db_tables` 发现面 | `describe::tables`（注册表按维度过滤，14 行 / `store_meta` 天然排除）+ `describe::table`（`FieldDoc.name` = `Format` 应用后的线名）+ `db_tool::db_tables` 的两形态（DD8）与 `scope_of` 三态（DD9） |
| AC-2 说明来源 + ratchet | `describe.rs` 全量取自 `Types` + `Format::map_types`（零手写字段表）；`model.rs` 补 12 条 doc；`schema_session_event` 的 3 字段（线名与真实 serde 键集一致，由测试相位双向断言）；`truncate_doc` 单点（DD13） |
| AC-3 线名可查 | `describe::field_index`（`Format` 派生的线名集）+ `query` 的顶层字段校验（未知 → `Err` + 候选线名，DD11）；`change_id` 一类蛇形名必然不在集内 |
| AC-4 查询原语语义 | `query.rs::Op::parse`（`QUERY_OPS` 单点）+ 值类型校验（`describe::field_kind`）+ `resolve_path`（点号下钻 / 数组下标 → `Err`）+ `LIKE` 四形与 `\\%` / `\\_` + `IN` + `IS NULL` 两态 + `null` 比较语义 |
| AC-5 排序 / 投影 / 分页 | `compare_values`（DD12 全序）+ `project`（`key` 恒在；未选字段不出现）+ `limit` 上限 1000 / 越界 `Err`（`query::run` 校验）+ 三元口径（`QueryPage` 的 `total` / `matched` / `records.len()`，DD4 保证精确） |
| AC-6 双档收口 | `db_tool` 的 `RESULT_BUDGET_BYTES`（40 KB）逐条累加 + 整体序列化（DD7 原子性）+ `truncatedBy` 两因 + hint 文案 + 末尾防御性裁剪（`< MAX_RESULT_BYTES`，使 `cap_result` 对该工具不可达） |
| AC-7 敏感面 | `db_tool::guard_sensitive`（`where` / `order` 引用 `apiKey` → `Err`）+ `mask_api_key`（递归打码，含 `select` 投影与嵌套）+ store 侧零感知（DD6） |
| AC-8 只读与零介入 | 挂载两项 `tier: ToolTier::Read`（`build_registry`）+ `policy::allows` 三档天然放行 + `tools::input_path` 显式 `None` 臂 + `protect` 表 `_ => None` 零介入 + store 侧零写入口（`crates/core/**` / `policy.rs` / `protect/**` 零 diff） |
| AC-9 会话可见性两臂 | `compose::assemble_injections`：explore 9 名（`:45` 数组长度 7 → 9）、change 臂矩阵之后恒 push 两名；archi 臂 / debug 臂零改动；`RunStarted.tools` 由 `definitions` 自动一致；spec 语料单源（本变更已改三份 delta 的表述） |
| AC-10 双库 scope 与身份锚 | `WorkspaceStores.global: Arc<Store>` + `global_shared()`（单开纪律不破）+ `compose_turn` 用 `stores.global_shared()` 与既有 `store` 组装（DB 句柄不经 cwd 派生 → change 会话恒主仓 root） |
| AC-11 成员校验与幻觉调用 | `definitions` 按名单过滤（既有机制，零改动）→ 未下发会话的调用被拒；`ToolExec::Db` 臂的 `None` → `is_error` 回灌（生产不可达） |
| AC-12 事件 / IPC / 前端零改动 | 无新 `AgentEvent` 变体（工具结果走既有 `Message{role: Tool}` 密封路径）；`src-tauri/src/bindings/**`、`src-tauri/src/commands/**` 的**生产命令面**与 `packages/desktop/src/**` 零 diff（新类型不 derive `Type`、无新命令）；唯一例外 = `src-tauri/src/commands/db/mod_test.rs` 的注册表行数期望随动（既有测试期望值随 `MODEL_ENTRIES` 11 → 14 行收敛，见「编译闭合批」末行，不属生产命令面） |
| AC-13 版本交付 | `packages/desktop/package.json` `0.6.43` → `0.6.44`（归档时执行；`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` crate 行不跟随） |

### 交付门（非本变更能力判据）

全仓既有门的**登记面**，本设计与 tasks 均不派生新判据。源码级静态面（`cargo clippy --workspace --all-targets`、`grep` 零命中、读码结构复核、`git diff` 边界比对、版本行）可在 tasks 自查项中自判；`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位承接，在 tasks 中只以「交接 …」条目登记。

---

## 待决问题

1. **测试文件的模块落点**：proposal「测试文件」表把 `describe` / `query` 的用例都写在 `envelope_test.rs`，但实现按 DD1 落到 `describe.rs` / `query.rs`。测试相位宜按 sidecar 同批惯例建 `describe_test.rs` / `query_test.rs`（并在 `lib.rs` 的 `#[cfg(test)]` 区同批加声明），`envelope_test.rs` 只保留「注册表 14 行 / `dimension_of` / 三新表 `key_of`」面。本设计不写用例，只登记该落点差异以免重复。
   **同批登记的文件计数口径**：proposal「测试文件」表共 14 行 = **3 新建**（`describe_test.rs` / `query_test.rs` / `sdk/db_tool_test.rs`）+ **9 既有 sidecar 追加**（`envelope_test` / `model_test` / `store_test` / `mount_test` / `compose_test` / `runner_test` / `tools_test` / `loop_test` / `lib_test`）+ **2 既有期望随动**（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`，登记在本篇「编译闭合批」末两行）。后两者的性质是「行数期望随注册表 / 挂载清单补齐而随动」，MUST NOT 被表述成「追加用例」，也 MUST NOT 因此把 `src/commands/**` 或 `sdk/*_test.rs` 整目录声明为「零 diff」——下游测试相位与 static-check 的清单表述须按这三档口径对齐。
2. **`FieldDoc` 是否补主键 / 索引标注**（explore 草稿曾有 `key: "primary"`）：本期**不做**——`native_db` 的键信息在 specta `Field.attributes` 里以 `native_db:primary_key` 一类字符串存在，取它要额外解析属性键（属第二份元数据读取面）。若后续模型频繁问「哪个字段能精确取」，再从属性派生（加一个 `key?: "primary" | "secondary"` 位即可，输出形态向后兼容）。
3. **`Types` 是否缓存**：本期**每次调用重建**（14 个 `definition` + 一次 `Format.map_types`，亚毫秒级，实测探针量级）。缓存需要 `'static` 单例并验证 `Types: Send + Sync`（未验证），而工具调用频率远低于每次重建的成本量级——若后续出现「大表深翻 + 多表 describe 同轮」的性能实例再议。
4. **`db_query` 在超大表上的单次耗时**：DD4 选「精确 `matched`」意味着带 `where` / `order` 时必全表 decode（70 万行外推 1.3~1.7 s）。proposal 风险表已知情接受；工具描述里以「大表用 `where` / `select` 收窄」引导。若实测不可接受，二期按「`where` 等值命中二级索引 → `scan().secondary().range()` 下推」优化（不改本设计任何对外语义）。
5. **`db_tables` 的 `summary` 长度分布**：`AgentRunRecord` 一类整篇 doc 经 `truncate_doc` 后约 120 字，14 张表合计约 1.4 KB（explore 实测口径）。若某些表的首句本身信息量过低（截断后模型仍需下钻），属文案问题而非机制问题——补 `///` 首句即可，不改截断规则。
