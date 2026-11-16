# 任务: agent-db-query-tools

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 14 项（3 新增文件 + 11 修改面，含 1 个版本行）+ 设计新增的模块落点（`describe.rs` / `query.rs`，见 design DD1）+ 机械改动（`LoopTurn` / `PumpPayload` / `EngineFacade` 字面构造面补位，见 design「编译闭合批」）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：proposal「测试文件」表逐行的新增 / 改造用例归 test-design / test-gen / test-execution 相位；`cargo test` / `pnpm -C packages/desktop test` 等回归由桌面端 test-execution 相位承接。本相位对测试文件的唯一动作是**让工作区随字段面 / 签名变化编译通过**（`LoopTurn { …, db }` / `PumpPayload { …, db_global, db_workspace }` / `EngineFacade { …, db_global, db_workspace }`），MUST NOT 新增用例、MUST NOT 改断言口径。
>
> **依赖序（五阶段）**：
>
> - **阶段一**（store 文档面地基：`Cargo.toml` + `model.rs` 的 12 条 doc 与 3 个 `#[specta(type = String)]`）**零 API 面变化**，是 `describe` 能派生的前提（`Format.map_types` 的 4 个 `serde(with)` 坑在此关上 3 个）。
> - **阶段二**（store 读面：注册表补齐 14 行 + `describe.rs` + `query.rs` + `Store` 薄包装 + `WorkspaceStores.global: Arc` + 出口）依赖阶段一（线名 / doc / 类型图可派生）。
> - **阶段三**（agent 执行面：`db_tool.rs` + `sdk/mod.rs` + 挂载两项 + `ToolExec::Db` + tier + 沙箱豁免 + `LoopTurn.db` 与分派臂）依赖阶段二（`QuerySpec` / `QueryPage` / `ModelDoc` / `dimension_of` / `QueryError` 在场）；本阶段末 `ToolExec::Db` 已闭环（`Option<DbContext>` 恒 `None` 亦可编译）。
> - **阶段四**（装配面：`SdkRunner` 双库 builder + `PumpPayload` + `mint_db_context` + `EngineFacade::with_db_stores` + `compose.rs` 的接线与两臂名单）依赖阶段三（`DbContext::new` 与 `mint_db_context` 的判据在场）。
> - **阶段五**（版本 + 边界自查 + 交接备忘）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **原语单源**：过滤 / 排序 / 投影 / 分页语义**只在 `crates/infra/store/src/query.rs`**；`db_tool.rs` MUST NOT 再实现第二份（它只做 scope 解析、形状解析、敏感字段守卫、预算闸、打码、文案装配）。op 词表同理单源（`store::QUERY_OPS`，挂载 description 与 `Op::parse` 共用）。
> ① **逐字不动面**（`git diff` 出现即越界）：`crates/core/**`（`SessionInjections` 四字段形态 / `AgentEvent` 词表 / `EngineConfig` 字段面 / `AgentStartError` 变体集 / 16 份相位提示词 md）；`crates/infra/agent/src/sdk/{policy,sandbox,protect/**,bash,explore_note,archi_tool,context,compact,resume,log,normalize,web_search}.rs`；`crates/infra/store/src/{change_port,canonical}.rs`；`MAX_SCAN_LIMIT` 与其钳位点（`envelope::scan`）；`Store::list_models` / `Store::scan` / `scan_model` / 既有 11 行注册表行的字段值；`src-tauri/src/**`（`bindings/**` / `main.rs` / `commands/**` 的生产命令面）；`packages/desktop/src/**`。**两处登记的期望随动不在此列**（非零 diff 且非越界）：`crates/infra/agent/src/sdk/policy_test.rs`（三档等价表 11 → 13 名）与 `src-tauri/src/commands/db/mod_test.rs`（workspace 清单 8 → 11 行）——见 design「编译闭合批」末两行。
> ② **双库句柄零二次 open**：`DbContext` 的两把手只能来自 `WorkspaceStores::global_shared()` 与命令层预解析的 `store: Arc<Store>`（= 会话 workspace root 的库实例）；MUST NOT 在 agent 侧按路径 `Store::open*`，MUST NOT 由 cwd / worktree 派生库路径。
> ③ **新读面不出线**：`ModelDoc` / `FieldDoc` / `TableDoc` / `QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage` / `QueryError` **MUST NOT** derive `specta::Type`（也不 derive `Serialize`/`Deserialize`，除 `QuerySpec` / `QueryCond` / `QueryOrder` 需 `Deserialize` 解析工具入参）；`packages/desktop/src/types/generated/**` 与 `src-tauri/src/bindings/**` MUST 零 diff。
> ④ **说明零手写**：表介绍与字段说明 MUST 全部来自 `///` doc + specta 类型图（唯一例外 = `SessionEventRecord` 的 3 字段，spec 明文的「单独供给」，见 design DD3）；MUST NOT 出现第二份字段表 / per-table 手写清单。
> ⑤ **预算闸在 L1 之前**：`db_query` 结果体 SHALL 恒低于 `tools::MAX_RESULT_BYTES`（50 KB），靠 40 KB 预算 + 整体序列化 + 末尾防御性裁剪三件套；MUST NOT 让 `tools::cap_result` 实际截断该工具输出（AC-6）。
> ⑥ **门禁分工**：`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。自查项只收源码级静态判据（`cargo clippy --workspace --all-targets`、`grep` 零命中、读码复核、`git diff` 边界比对）；测试回归与整链门一律写成「交接 …」条目。
> ⑦ **测试声明与 sidecar 同批**：本相位**不**在 `sdk/mod.rs` 加 `#[cfg(test)] mod db_tool_test;`、不在 `store/src/lib.rs` 加 `mod describe_test;` / `mod query_test;`（先加声明会让 `cargo check --all-targets` 因缺文件失败）；声明由 test-gen 创建 sidecar 时同批落地（字母序位置见各阶段条目）。

## 阶段一：store 文档面地基（依赖面 + 12 条 doc + 3 个 specta 覆盖属性）

> 本阶段**零 API 变化**：只加注释与属性，既有测试（`model_test` 的逐字节对拍 / `envelope_test` 的分页面）应保持绿。它是 `Format.map_types` 能通过的前置（4 个 `serde(with)` 坑关掉 3 个，第 4 个 `session_event.event_key` 不进类型图，见 design 校准 1）。

- [x] `crates/infra/store/Cargo.toml`：`[dependencies]` 按字母序在 `serde_json` 与 `sha2` 之间加 `specta-serde = { workspace = true }`（注释一句「`Format::map_types` 应用 serde 重命名：单 crate 构建下也须在案，MUST NOT 指望壳层 feature 统一」）；`[dev-dependencies]` 零改动
- [x] `crates/infra/store/src/model.rs` 补 **12 条**字段 `///`（`git diff` 应只有 `+` 注释行；模型版本号 / `native_model` 头 / 字段面零改动）：
  - `PhaseRecord`（7 条）：`phase`（相位名，取自相位表 key，如 `dev-design`）、`attempt`（该相位累计尝试序号，feedback 重跑递增）、`skipped`（编排跳过标记，跳过行无 verdict）、`stale`（回跳后失效标记，失效行不计入判定）、`backtrack_reason`（回跳记因，非回跳行恒 None）、`evaluator_session_id`（评审侧会话 id；未开评恒 None）、`decision_session_id`（决策侧会话 id；未开决策恒 None）
  - `ChecklistItemRecord`（3 条）：`item`（检查项名，evaluator 自述文本）、`pass`（该检查项是否通过）、`evidence`（判定依据，有界证据文本）
  - `RunNodeRecord`（2 条）：`phase`（所属相位名，同 run 内步序按相位切段）、`attempt`（所属相位的尝试序号）
  - 文案口径：每条首句即结论（`truncate_doc` 按首个 `。` / `；` / `\n` 截断，首句承载全部可见信息），补充说明写在其后一句
- [x] `crates/infra/store/src/model.rs` 三个 `#[serde(with = "event_key_serde")]` 字段加 `#[specta(type = String)]`（置于 `#[primary_key]` 之后、`pub` 字段之前）：`ChecklistItemRecord.item_key` / `RunNodeRecord.node_key` / `RunEdgeRecord.edge_key`。**MUST NOT** 加在 `SessionEventRecord.event_key`（该结构不 derive `Type`，写该属性即未知属性编译错误；其线格式由阶段二的 `schema_session_event` 以 `Primitive(str)` 表达）
- [x] 自查 阶段一（静态判据）：
  - `git diff -- crates/infra/store/src/model.rs` 只含注释行与 3 行 `#[specta(type = String)]`（读 diff 复核：零字段改名、零 `#[serde]` 变化、零版本头变化）
  - `grep -c "specta(type = String)" crates/infra/store/src/model.rs` 为 **3**；`grep -n "SessionEventRecord" -A 12 crates/infra/store/src/model.rs` 的字段块不含该属性
  - `cargo clippy -p store --all-targets` 通过；`cargo tree -p store | grep specta` 出现 `specta-serde`
  - `git diff --stat -- packages/desktop/src/types/generated` 为空（本阶段不生任何生成物）
- [x] 交接 阶段一（测试回归，本相位不执行）：`model_test` 的「补 doc / 加 `specta(type)` 后 decode/encode 逐字节不变」与「`event_key_serde` 字段的十六进制串键集一致」由桌面端 test-design / test-gen / test-execution 相位承接。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段二：store 读面（注册表补齐 + describe + query + 薄包装 + 双库共享 + 出口）

> 依赖阶段一。本阶段把「表清单 / 字段说明 / 条件取数」做成 store 的公共只读面；**一切表↔类型映射只经 `MODEL_ENTRIES`**（design DD2）。`envelope.rs` 的 `list_models` / `scan` / `scan_model` / `MAX_SCAN_LIMIT` 与既有 11 行逐字不动。

- [x] `crates/infra/store/src/envelope.rs` 注册表补齐与扩面：
  - `use crate::model::{… RunEdgeRecord, RunNodeRecord, RunRecord};`（字母序）
  - `struct ModelEntry` → `pub(crate) struct ModelEntry`，字段全 `pub(crate)`；增 `pub(crate) schema: SchemaFn,` / `pub(crate) walk: WalkFn,` 两个 `pub(crate)` 字段（doc：类型图与流式遍历同样由注册表派生——表↔类型映射 MUST NOT 有第二份）
  - 新增两个类型别名（置于 `ScanFn` / `KeyOfFn` 旁）：
    ```rust
    /// 类型图注册 fn-pointer：把本模型（及其依赖的命名类型）灌进 `Types`，
    /// 返回其根 `DataType`（`describe` 据此取线名 / 类型 / doc）。
    pub(crate) type SchemaFn = fn(&mut specta::Types) -> specta::DataType;
    /// 记录流式遍历 fn-pointer：单读事务内逐条解码 → JSON → 交访问器
    /// （访问器返回 `false` 即提前终止）；返回遍历到的总行数。
    pub(crate) type WalkFn = fn(
        &Database<'static>,
        KeyOfFn,
        &mut dyn FnMut(&Value) -> Result<bool, crate::query::QueryError>,
    ) -> Result<u64, crate::query::QueryError>;
    ```
  - `MODEL_ENTRIES` 11 → **14** 行：`workspace` / `agent_provider` / `agent_instance` / `agent_run` / `session` / `session_event` / `explore` / `change` / `phase` / `checklist_item` / `step` 逐行补 `schema:` / `walk:`；尾部追加 `run`（`RunRecord`）/ `run_node`（`RunNodeRecord`）/ `run_edge`（`RunEdgeRecord`）三行（`dimension: DbDimension::Workspace`）。`store_meta` 仍不入表（既有注释口径逐字保留）
  - 新增通用 `walk_model::<T: ToInput + Serialize>`（`db.r_transaction()` **一次**；`r.scan().primary::<T>()?.all()?` 逐条：`total += 1`、`serde_json::to_value` → `key_of(&value)` 由访问器按需取、访问器返回 `false` 即 `break`；返回 `total`）+ **14 个**三行 `walk_*` 包装（照既有 `scan_*` 包装先例，紧邻其 `scan_*` 对应项）
  - 新增 accessor：`pub(crate) fn entries() -> &'static [ModelEntry]`、`pub(crate) fn entry(table: &str) -> Option<&'static ModelEntry>`（跨维度按名直查）、`pub fn dimension_of(table: &str) -> Option<DbDimension>`
  - 新增三个 `key_of`（置于既有 `step_key` 之后）：`run_key`（`value["runId"]`）、`run_node_key`（`{runId, nodeKey}`，与 `checklist_item_key` 同形的打包键可读投影）、`run_edge_key`（`{runId, edgeKey}`）
- [x] 新建 `crates/infra/store/src/describe.rs`（DD1 / DD3 / DD13 / DD14）：
  - 三个公共记录：`ModelDoc { table, dimension, count, summary }`、`TableDoc { table, dimension, summary, fields }`、`FieldDoc { name, ty, doc }`（`Debug + Clone + PartialEq`，**不** derive `Type` / `Serialize`）
  - `pub(crate) fn schema_of<T: specta::Type>(types: &mut Types) -> DataType`（`T::definition(types)`）与 `pub(crate) fn schema_session_event(types: &mut Types) -> DataType`（手写 3 字段：`eventKey` = `Primitive(str)`、`sessionId` = `Primitive(str)`、`event` = `Reference::opaque(())`；`NamedDataType::new("SessionEventRecord", types, …)` + `Struct::named()` 构造；doc 写明「该表不 derive `Type`（`event` 是 `Box<RawValue>`），3 条说明在此单独供给，线名/键集由 ratchet 机械对齐」）
  - 共用派生单点：`fn formatted_types() -> Result<Types, QueryError>`（逐条 `(entry.schema)(&mut types)` → `specta_serde::Format.map_types(&types)`，`Err` 折 `QueryError::Invalid("schema 派生失败: …")`）；`fn named_of<'a>(types: &'a Types, root: &DataType) -> Result<&'a NamedDataType, QueryError>`（`Reference::Named` → `types.get`；非引用即 `Invalid`）
  - `pub(crate) fn tables(db, dimension) -> Result<Vec<ModelDoc>, QueryError>`（按维度过滤注册表 → `(entry.count)(db)` + `truncate_doc(named.docs)`；**不展开字段**）
  - `pub(crate) fn table(db, dimension, name) -> Result<TableDoc, QueryError>`（未知表 → `Invalid` + **本维度候选表名**）
  - `pub(crate) fn field_index(table: &str) -> Result<FieldIndex, QueryError>`，`pub(crate) struct FieldIndex { fields: Vec<(String, FieldKind, String)> }` + `names() -> Vec<&str>` / `kind_of(path: &str) -> FieldKind`（顶层可查；含点号即 `Unknown`）/ `render_of(path: &str) -> Option<&str>`
  - `pub(crate) enum FieldKind { Number, Text, Bool, Object, Unknown }` 与 `fn render_type(ty: &DataType, types: &Types) -> String` / `fn field_kind(ty: &DataType, types: &Types) -> FieldKind`（同一棵 match —— DD14：`Nullable` 取 inner、`Reference(Named)` 取 `types.get(r).name` 并按其 `ty` 再判、`Primitive` 按位宽收敛、`List`/`Map`/`Struct`/`Enum`/opaque 各自的文本；`NamedReference` 无 `Display`，MUST 经 `types.get` 取名）
  - `fn truncate_doc(doc: &str) -> String`（DD13：首个 `。` / `；` / `\n` 含该字符为止；无句读或超 120 字 → 截到 120 字 `char` 边界并补 `…`；尾部空白剥离）
- [x] 新建 `crates/infra/store/src/query.rs`（DD4 / DD5 / DD12）：
  - `pub const QUERY_OPS: [&str; 10]`（`=` `!=` `>` `>=` `<` `<=` `LIKE` `IN` `IS NULL` `IS NOT NULL`）+ `pub const DEFAULT_QUERY_LIMIT: u32 = 20;` / `pub const MAX_QUERY_LIMIT: u32 = 1000;`
  - 四个出线形态类型：`QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage`（derive 面见 design「类型定义」；`QuerySpec.conds` 用 `#[serde(rename = "where")]` 并 `#[serde(default)]`，`offset` 用 `#[serde(default)]`、`limit` 用 `#[serde(default = "default_limit")]`，两层都 `deny_unknown_fields` 以让拼错的键即 `Err`）
  - `pub enum QueryError { Invalid(String), Db(String) }` + `Display`（`invalid: …` / `db: …`）+ `impl std::error::Error`
  - 私有 `enum Op` + `fn parse(op: &str) -> Result<Op, QueryError>`（未知 → 列 `QUERY_OPS` 全词表）
  - 校验链（顺序即错误优先级）：`limit` 越界（`1..=1000`，越界 `Err` 报上限且 MUST NOT 钳位）→ 表（未知 → 候选表名）→ `where` 各条 `field` 顶层线名校验（未知 → 候选线名）与 `op` 解析 → 值形状（`IN` 非数组 / `IS NULL` 携非 null 值 / 值类型与 `FieldKind` 不符 → `Err` 携字段渲染类型）→ `order.field` 同款校验 → `select` 各路径同款校验
  - `resolve_path(value: &Value, path: &str) -> Result<Option<&Value>, QueryError>`（点号下钻；任一段为纯数字 → `Err`（不支持数组下标，携「改看整条记录或用点号取对象字段」指引）；路径缺失 → `None`（等价 null））
  - `fn like_match(text: &str, pattern: &str) -> bool`（`%` 任意长（含空）/ `\\%` `\\_` 字面 / 无 `%` 退化等值 / 区分大小写 / `_` 与 `ESCAPE` 与正则 MUST NOT 支持）
  - `fn compare_values(a: &Value, b: &Value) -> Ordering`（DD12：`null` 恒末位；`number < string < bool < object < array < null` 的类型序 + 同类内规则）
  - `fn project(value: &Value, select: &[String]) -> Value`（按路径重建嵌套对象；缺失路径跳过）
  - `pub(crate) fn run(db, dimension, spec) -> Result<QueryPage, QueryError>`：DD4 双路径——`where` 空且无 `order` → `(entry.scan)(db, offset, limit, entry.key_of)`（`total = matched = page.total`）；否则 `walk` 全扫（`total` / `matched` 精确计数 + 有 `order` 时以「有序窗口 ≤ offset+limit 行」控内存；无 `order` 时按自然序窗口收集）
- [x] `crates/infra/store/src/store.rs`：
  - `impl Store` 增三个薄包装（置于 `list_models` / `scan` 之后）：`describe()` / `describe_table(table)` / `query(&spec)`，各自一行委托；doc 写明「`query` 独立读路径，MUST NOT 复用 `MAX_SCAN_LIMIT`（查看页单页上限管 IPC 载重，查询面上限管 agent 表达力）」与「只读：零写事务」
  - `WorkspaceStores.global: Store` → `global: Arc<Store>`（doc 补「`Arc` 使全局库可交会话共享：redb 单进程单句柄，MUST NOT 二次 open」）；`open()` 改 `Arc::new(Store::open_global(...)?)`；`global()` 改 `self.global.as_ref()`（返回类型仍是 `&Store`）；新增 `pub fn global_shared(&self) -> Arc<Store>`
- [x] `crates/infra/store/src/lib.rs`：`mod describe;` / `mod query;`（`canonical` 之后、`envelope` 之前，字母序）；出口增 `pub use describe::{FieldDoc, ModelDoc, TableDoc};`、`pub use envelope::{dimension_of, ModelInfo, RecordEnvelope, RecordsPage};`、`pub use model::{… RunEdgeRecord, RunNodeRecord, RunRecord, …};`、`pub use query::{QueryCond, QueryError, QueryOrder, QueryPage, QuerySpec, MAX_QUERY_LIMIT, QUERY_OPS};`（**不加** `#[cfg(test)] mod describe_test;` / `query_test;`——约束 ⑦）
- [x] 自查 阶段二（静态判据）：
  - `grep -c "^    ModelEntry {" crates/infra/store/src/envelope.rs` 为 **14**（行首缩进四格的注册表行；不带 `^    ` 时 +1 = 结构体定义行 `pub(crate) struct ModelEntry {`，共 15 命中）；`grep -n "store_meta\|StoreMetaRecord" crates/infra/store/src/envelope.rs` 只命中 1 处 doc 注释（`change_key` 的说明行），注册表内零行
  - `git diff -- crates/infra/store/src/envelope.rs` 的 `MODEL_ENTRIES` 前 11 行块只多 `schema:` / `walk:` 两行，既有 `name` / `dimension` / `count` / `scan` / `key_of` 值零变化；`MAX_SCAN_LIMIT` 与其 `.min(MAX_SCAN_LIMIT)` 钳位点零 hunk
  - `grep -c "^fn walk_" crates/infra/store/src/envelope.rs` 为 **15** = 14 个 `walk_*` 三行包装 + 1 个 `walk_model<T>`（`grep -c "^fn walk_model"` 为 1）；`grep -c "r_transaction()" crates/infra/store/src/envelope.rs` 在 `walk_model` 内恰 1 处（单事务）
  - `grep -c "specta::Type" crates/infra/store/src/{describe,query}.rs` 为 **0**（新读面不出线，约束 ③）；`grep -rn "derive(.*Type.*)" crates/infra/store/src/describe.rs crates/infra/store/src/query.rs` 零命中
  - `grep -c "QUERY_OPS" crates/infra/store/src/query.rs` ≥ 2（定义 + `Op::parse` 引用）；`grep -n "IS NOT NULL" crates/infra/store/src/query.rs` 命中 **4** 处且无第五处独立实现（词表常量行 / `Op` 变体 doc / `Op::parse` 臂 / 判空用法的错误文案——比较语义只在 `matches` 的 `IsNull` / `IsNotNull` 两臂各占一行）
  - `grep -c "MAX_SCAN_LIMIT" crates/infra/store/src/query.rs` 为 **0**；`grep -n "MAX_QUERY_LIMIT" crates/infra/store/src/query.rs` 覆盖校验与常量两处
  - `grep -n "fn order_cmp" -A 14 crates/infra/store/src/query.rs` 读码复核（DD12 归位）：null 分组先于方向反转（`(true, false) → Greater` 不随 `desc` 翻转，降序时 null 仍末位）；`insert_row` 的谓词为 `order_cmp(&other.order, &row.order, desc) != Ordering::Greater`（统一口径，秩不参与方向判定）
  - `grep -n "fn global\b" -A 3 crates/infra/store/src/store.rs` 返回类型仍为 `&Store`；`git diff -- crates/infra/store/src/change_port.rs` 为空（`StoreError` 四变体零改动，DD5）
  - `cargo clippy -p store --all-targets` 通过（`walk` / `schema` 指针与本阶段消费者同批落地，不留 dead_code）
- [x] 交接 阶段二（测试回归，本相位不执行）：`describe_test.rs`（新，14 行清单 / `store_meta` 不出现 / `table` 下钻线名与类型文本 / doc 截断 / ratchet 双向断言：每字段 doc 非空 + 线名集 ↔ 真实 serde JSON 键集 + 内嵌类型不入清单 / `session_event` 三字段）、`query_test.rs`（新，op 全集逐格 / AND / 下钻 / `LIKE` 四形与转义 / `IN` / `IS NULL` 两态 / 类型不符 `Err` / 排序升降与 `null` 末位 / 投影 `key` 恒在 / 分页与上限 / 三元口径 / 未知表字段 op 的候选文案 / 只读）、`envelope_test.rs`（注册表 14 行与三新表 `key_of` 面）、`model_test.rs`（阶段一的逐字节不变）**由桌面端 test-design / test-gen / test-execution 相位承接**；`describe_test.rs` / `query_test.rs` 的 `#[cfg(test)] mod` 声明由 test-gen 与文件同批添加（约束 ⑦）。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段三：agent 执行面（执行体 + 挂载两项 + tier + 沙箱豁免 + 分派臂）

> 依赖阶段二。本阶段末 `ToolExec::Db` 已闭环：分派臂、`LoopTurn.db` 字段、执行体三者齐备；`db` 恒 `None` 也不影响编译与既有行为（铸造在阶段四接上）。

- [x] 新建 `crates/infra/agent/src/sdk/db_tool.rs`（design DD6 / DD7 / DD8 / DD9 / DD10）：
  - 模块头 doc：三句话——①两只只读工具的执行体（scope 解析 / 形状解析 / 敏感字段守卫 / 文字预算闸 / 文案装配），**过滤排序语义单源在 store**（MUST NOT 在此实现第二份）；②打码与预算属 agent 政策（store 是纯数据层，查看页同用）；③错误文案即自纠通道（候选表 / 候选线名 / op 词表 / 字段类型 / 下标指引）
  - 常量：`pub(crate) const RESULT_BUDGET_BYTES: usize = 40 * 1024;`、`pub(crate) const ENVELOPE_RESERVE_BYTES: usize = 1024;`、`pub(crate) const MASKED_VALUE: &str = "***";`、`const SENSITIVE_FIELD: &str = "apiKey";`
  - `pub(crate) struct DbContext { global: Arc<Store>, workspace: Arc<Store> }`（`#[derive(Clone)]`）+ `pub(crate) fn new(...)`
  - `pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String>`：`match name { mount::DB_TABLES_TOOL => self.tables(input), mount::DB_QUERY_TOOL => self.query(input), other => Err(format!("未知 DB 工具: {other}")) }`，结果过 `tools::cap_result`
  - `fn tables(&self, input) -> Result<String, String>`：`scope_of` → 有 `table` 走下钻（`dimension_of` 派库 + `store_for(scope)` 的 `describe_table`），无 `table` 走清单（`Both` = 两库拼接；`User` / `Workspace` 单库）；输出形状 DD8；未知表 `Err` + 候选表名（跨库合并、去重）
  - `fn query(&self, input) -> Result<String, String>`：`let mut payload = input.clone(); let scope = payload.as_object_mut().ok_or_else(|| "入参形状不符：期望 JSON 对象".to_owned())?.remove("scope");` → `serde_json::from_value::<QuerySpec>(payload)`（形状不符 → `Err` 附期望形状：`{table, where:[{field,op,value}], order:{field,desc}, select:[…], offset, limit}`）→ `guard_sensitive(&spec)` → `store_for(scope)` 的 `Store::query` → 记录逐条 `mask_api_key` → 预算闸（DD7：逐条计长整条停止 → 整体序列化 → 末尾防御性裁剪）→ 结果体 `{table,total,matched,returned,truncated,truncatedBy,records}` + 需要时 `hint`
  - `fn scope_of(input: &Value) -> Result<Scope, String>`（三态：`Some("user")` / `Some("workspace")` / 缺省；其他值 → `Err` 列两值）
  - `fn store_for(&self, scope: Scope) -> &Arc<Store>`（`User` → global、`Workspace` / `Both` → workspace；`Both` 只对 `db_tables` 清单形态有意义）
  - `fn guard_sensitive(spec: &QuerySpec) -> Result<(), String>`（`where[].field` / `order.field` 的路径任一段 == `apiKey` → `Err`「该字段不可用于过滤 / 排序」）
  - `fn mask_api_key(value: &mut Value)`（递归：对象键 == `apiKey` 且值为字符串 → 置 `MASKED_VALUE`；数组逐元素递归）
  - `fn truncation_hint(truncated_by: &str, returned: usize, matched: u64) -> String`（「用 `select` 收窄字段 / 加 `where` 条件 / 翻 `offset`（深翻在大表上较慢）」+ 如实三元回报）
  - `fn nested_path_hint(spec: &QuerySpec) -> Option<String>`（`where` 含点号路径且 `matched == 0` 时的 DD11 提示）
- [x] `crates/infra/agent/src/sdk/mod.rs`：`pub(crate) mod db_tool;`（`context` 之后、`explore_note` 之前，字母序）；**不加** `#[cfg(test)] mod db_tool_test;`（约束 ⑦）
- [x] `crates/infra/agent/src/sdk/mount.rs`：
  - 两个常量（置于 `EXPLORE_NOTE_TOOL` 之前，按「通用 → 专用」既有排布）：`pub(crate) const DB_TABLES_TOOL: &str = "db_tables";` / `pub(crate) const DB_QUERY_TOOL: &str = "db_query";`，doc 写明「只读发现 / 取数两件套，执行体在 `db_tool`，可见性单源在 `compose`」
  - `enum ToolExec` 增 `Db,`（doc：「两 DB 只读工具：`LoopTurn.db` 上下文」；与 `Archi` 同形无载荷）
  - `build_registry`：`Vec::with_capacity(11)` → `13`；尾部（archi 三项之后）追加两项 `ToolMount { definition: db_tables_definition(), tier: ToolTier::Read, exec: ToolExec::Db }` 与 `db_query_definition()` 同款；`registry()` / `build_registry()` 的「十一项 / 十一个字面构造项」doc 改「十三项 / 十三个字面构造项」，并补一句「前七项仍是七内建」
  - `fn db_tables_definition() -> ToolDefinition`：description 覆盖——无参给 14 行表清单（`dimension` / `table` / `count` / `summary`）、带 `table` 给该表字段（`name` 是**落库线名**，可直接喂 `db_query` 的 `field`；`type` 是渲染文本；`doc` 是说明）；`scope` 两值 + 缺省两库全列；先 `db_tables` 后 `db_query`。parameters：`{ scope?: "user"|"workspace", table?: string }`
  - `fn db_query_definition() -> ToolDefinition`：description 覆盖——`where` 为 AND 数组、`op` 词表由 `vocabulary(store::QUERY_OPS)` 拼接、字段名是**线名**（camelCase，写错了会报错并列候选）、点号下钻支持而数组下标不支持、`LIKE` 只用 `%`、`order` 缺省主键自然序、`select` 投影（`key` 恒在）、`limit` 缺省 20 / 上限 1000、**「行数是名义上限、40 KB 字节预算是实际生效值：一次可能只回 20 余行，请看 `returned` / `matched` / `truncatedBy` 而不是猜库里有多少」**、以及自读警示「`session` / `session_event` 是本会话自身转录，查询会把当前对话再次喂回上下文，慎查」。parameters：`{ table (required), scope?, where?: [{field, op, value}], order?: {field, desc}, select?: [string], offset?, limit? }`
  - `use` 面补 `store` 的 `QUERY_OPS`（`vocabulary()` 是既有的 `/` 拼接单点，MUST NOT 另写一份词表）
- [x] `crates/infra/agent/src/sdk/tools.rs`：`pub(crate) use crate::sdk::mount::{…}` 名单补 `DB_QUERY_TOOL, DB_TABLES_TOOL`；`input_path` 的显式 `None` 臂由三名扩到五名（doc 补「两 DB 工具无路径参数」）
- [x] `crates/infra/agent/src/sdk/loop.rs`：
  - `LoopTurn` 在 `pub ask: Option<AskContext>,` 之后加 `pub db: Option<DbContext>,`（doc：「两 DB 只读工具执行上下文（per-turn 铸造随轮携带；`None` = 未装配——非 explore / change 会话恒 None，工具调用以 is_error 回灌）」）；`use crate::sdk::db_tool::DbContext;`
  - 执行臂 `match mount.exec` 在 `ToolExec::Archi` 之后加：
    ```rust
    ToolExec::Db => match &turn.db {
        Some(ctx) => match ctx.execute(&name, &input).await {
            Ok(text) => (text, false),
            Err(error) => (error, true),
        },
        None => ("专用工具执行上下文未装配".to_owned(), true),
    },
    ```
  - 守卫链四段（成员校验 / `policy::allows` / `sandbox_rewrite` / `protect.evaluate`）零改动
- [x] 自查 阶段三（静态判据）：
  - `grep -c "pub(crate) mod db_tool" crates/infra/agent/src/sdk/mod.rs` 为 1；`grep -c "db_tool_test" crates/infra/agent/src/sdk/mod.rs` 本相位落盘时为 **0**（约束 ⑦：声明随 sidecar 同批）——现盘为 1，系 test-gen 建 `db_tool_test.rs` 时同批添加的声明（`store/src/lib.rs` 的 `describe_test` / `query_test` 同形），不属本相位越界
  - `grep -c "ToolExec::Db" crates/infra/agent/src/sdk/{mount,loop}.rs` = mount 2（两项 registry 行）+ loop 1（分派臂）；`grep -c "db_tables\|db_query" crates/infra/agent/src/sdk/mount.rs` ≥ 4（两常量 + 两定义）
  - `grep -n "ToolTier::Read" -A 2 crates/infra/agent/src/sdk/mount.rs` 两 DB 项均为 `Read`（读码复核：MUST NOT 出现 `Write` / `Execute`）
  - `grep -c "QUERY_OPS" crates/infra/agent/src/sdk/mount.rs` 为 2（description 词表拼接 + `parameters` 的 `op` enum——同一常量两次引用，词表本体仍单源零复制）
  - `grep -c "RESULT_BUDGET_BYTES\|ENVELOPE_RESERVE_BYTES" crates/infra/agent/src/sdk/db_tool.rs` 为 3 行（两定义 + 一处 `saturating_sub` 联用，按行计数口径）；`grep -n "to_string(&records)\|records 序列化" crates/infra/agent/src/sdk/db_tool.rs` 读码复核「**整体**序列化一次」（无逐条拼串）
  - `grep -n "apiKey" crates/infra/agent/src/sdk/db_tool.rs` 出现在 `guard_sensitive` 与 `mask_api_key` 两处（store 产品源零命中：`grep -rn "apiKey" crates/infra/store/src --include=*.rs | grep -v "_test.rs"` 为 0 —— DD6；`store` 的 `*_test.rs` 里出现属测试相位的键集对拍）
  - `git diff -- crates/infra/agent/src/sdk/{policy,sandbox,bash,explore_note,archi_tool,context,compact,resume,log,normalize,web_search}.rs crates/infra/agent/src/sdk/protect` 为空（约束 ①）
  - `cargo clippy --workspace --all-targets` 通过（含 `LoopTurn` 字面构造面的机械补位；夹具处补 `db: None`）
- [x] 交接 阶段三（测试回归，本相位不执行）：`sdk/db_tool_test.rs`（新：预算闸到界即停 / `truncatedBy` 两因 / 无半截 JSON / `apiKey` 打码（含 `select` 投影）/ `where`·`order` 引用 `apiKey` 即 `Err` / 未知字段携候选线名 / `scope` 三态 / `db_tables` 两形态）、`sdk/mount_test.rs`（两项在清单内 / tier = `Read` / 三档放行 / 定义 schema 形状）、`sdk/tools_test.rs`（`input_path` 对两工具名返回 `None`）、`sdk/loop_test.rs`（`ToolExec::Db` 上下文缺席 `is_error`、在场走执行体）**由桌面端 test-design / test-gen / test-execution 相位承接**；`db_tool_test.rs` 的模块声明由 test-gen 同批添加。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段四：装配面（双库句柄铸造 + 门面 builder + 两臂名字清单）

> 依赖阶段三。本阶段把 `DbContext` 从组合根送到泵、再送到 `LoopTurn`；并在 explore / change 两臂把两名工具下发下去（否则执行体是死代码）。

- [x] `crates/infra/agent/src/sdk/runner.rs`：
  - `SdkRunner` 增两私有字段 `db_global: Option<Arc<Store>>` / `db_workspace: Option<Arc<Store>>`（`new` 内均 `None`）；新增 builder：
    ```rust
    /// 双库句柄旁路注入（builder 缝，与 `with_workspace_root` 同型）：全局库经
    /// `WorkspaceStores::global_shared()`、workspace 库取会话 workspace root 的
    /// 实例——MUST NOT 二次 open（redb 单进程单句柄）。
    pub fn with_db_stores(self, global: Arc<Store>, workspace: Arc<Store>) -> Self {
        Self { db_global: Some(global), db_workspace: Some(workspace), ..self }
    }
    ```
  - `PumpPayload` 增 `db_global: Option<Arc<Store>>` / `db_workspace: Option<Arc<Store>>`（doc：per-turn 铸造 `DbContext` 的原料）；`open_session` 的 `PumpPayload { … }` 补两位（`self.db_global.clone()` / `self.db_workspace.clone()`）
  - 新增铸造判据单点（置于 `mint_archi_context` 之后）：
    ```rust
    /// 两 DB 只读工具执行上下文 per-turn 铸造（单点；`session_pump` 唯一调用处）：
    /// 本轮名字清单含任一 DB 工具名且两库句柄皆在场才铸，否则 `None`。判据与
    /// `mint_archi_context` 同形（名字清单包含），MUST NOT 用枚举比较或自由字符串嗅探。
    pub(crate) fn mint_db_context(
        injections: &agent::SessionInjections,
        global: Option<Arc<Store>>,
        workspace: Option<Arc<Store>>,
    ) -> Option<DbContext> {
        let present = contains_tool(injections, DB_TABLES_TOOL) || contains_tool(injections, DB_QUERY_TOOL);
        if !present {
            return None;
        }
        global
            .zip(workspace)
            .map(|(global, workspace)| DbContext::new(global, workspace))
    }
    ```
  - `session_pump` 起播（`let archi = mint_archi_context(…)` 之后）加 `let db = mint_db_context(&open.injections, payload.db_global.clone(), payload.db_workspace.clone());`，`LoopTurn` 铸造点补 `db,`
  - `use` 面补 `crate::sdk::db_tool::DbContext`、`crate::sdk::mount::{DB_QUERY_TOOL, DB_TABLES_TOOL}`
- [x] `crates/infra/agent/src/lib.rs`（`EngineFacade`）：
  - `EngineFacade` 增两私有字段 `db_global: Option<Arc<store::Store>>` / `db_workspace: Option<Arc<store::Store>>`（doc 同 `explore_note_store` 口径：组合根注入缝，`None` = 未注入）
  - `new` 与六个逐字段 builder（`with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store` / `with_workspace_root`）各补两行字段（design 校准 6 的机械闭合）
  - 新增 builder（`..self` 形态）：
    ```rust
    /// 携双库句柄的门面（builder 缝，与 `with_explore_note_store` 同型）：per-turn
    /// 铸造 `DbContext` 的两把原料（全局库 + 会话 workspace 库），`runner_for` 透传。
    pub fn with_db_stores(self, global: Arc<store::Store>, workspace: Arc<store::Store>) -> Self
    ```
  - `runner_for` 的 `SdkRunner` 链上按在场性追加 `.with_db_stores(g, w)`（两位同时在场才追加；签名零改动）
- [x] `crates/infra/agent/src/compose.rs`：
  - `compose_turn` 内 `let explore_note_store = Arc::clone(&store);` 之后加 `let db_workspace = Arc::clone(&store);`（**必须在 `StoreSink::new(store)` move 之前**）与 `let db_global = stores.global_shared();`；门面链 `.with_explore_note_store(explore_note_store)` 之后加 `.with_db_stores(db_global, db_workspace)`
  - explore 臂：`const EXPLORE_TOOLS: [&str; 7]` → `[&str; 9]`（尾部追加 `mount::DB_TABLES_TOOL, mount::DB_QUERY_TOOL`）；`assemble_injections` 的臂注释补「+ 两只读 DB 工具（单源见 desktop-agent-db-tools）」
  - change 臂：矩阵并入之后（`if let Some(selected) = archi_matrix(...)` 块之后、`finish` 位装配之前）加
    ```rust
    // DB 工具两名恒在：不分 phase / 不分矩阵命中，与 ask 位恒 Flow 同构
    names.push(mount::DB_TABLES_TOOL);
    names.push(mount::DB_QUERY_TOOL);
    ```
  - `archi_matrix`、archi 臂、其余来源臂、`injected()` 的成员校验语义零改动
- [x] 自查 阶段四（静态判据）：
  - `grep -c "DB_TABLES_TOOL\|DB_QUERY_TOOL" crates/infra/agent/src/compose.rs` 为 **4**（explore 数组两项 + change 恒 push 两项；按读码复核 MUST 只出现在这两臂，archi 臂与 debug 臂零处）
  - `grep -n "with_db_stores" crates/infra/agent/src/{lib,compose}.rs crates/infra/agent/src/sdk/runner.rs` 三文件各命中（定义 + 调用）；`grep -n "global_shared" crates/infra/agent/src/compose.rs` 为 1
  - `grep -c "db_global\|db_workspace" crates/infra/agent/src/lib.rs` ≥ 9（两字段 + `new` + 六 builder + `with_db_stores` + `runner_for`）
  - `grep -n "mint_db_context" crates/infra/agent/src/sdk/runner.rs` 为 **2**（定义 + 泵内唯一调用）
  - `grep -n "Store::open\|open_workspace\|for_root" crates/infra/agent/src/sdk/{db_tool,runner}.rs crates/infra/agent/src/compose.rs` 中 `db_tool.rs` 为 **0**（约束 ②：句柄只来自注入）
  - `git diff -- crates/infra/agent/src/compose.rs` 的 `archi_matrix` / archi 臂 / `injected()` 块零 hunk
  - `git diff --stat -- packages/desktop/src packages/desktop/src-tauri/src/bindings` 为空（AC-12 早期守卫）；`packages/desktop/src-tauri/src/commands/**` 只允许 `db/mod_test.rs` 一处的注册表行数期望随动（workspace 维度 `db_models` 8 → 11 行），生产命令面（`db/mod.rs` 等）零 diff
  - `cargo clippy --workspace --all-targets` 通过
- [x] 交接 阶段四（测试回归，本相位不执行）：`compose_test.rs`（explore 9 名含两 DB 工具 / change 臂含矩阵未命中行与畸形 `source_ref` 时仍含两名 / archi 与 debug 臂不含 / 成员校验拒绝语义不变）、`sdk/runner_test.rs`（`mint_db_context` 判据：名单含任一 → `Some`、不含 → `None`、句柄缺席 → `None`）、`compose_test` / `worker_test` 的 `EngineFacade` 夹具随动 **由桌面端 test-design / test-gen / test-execution 相位承接**。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段五：版本、边界自查与交接备忘

- [x] `packages/desktop/package.json`：`version` `0.6.43` → `0.6.44`（AC-13，归档时执行；`dependencies` / `devDependencies` 零变化）
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改，跟随即生效）；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` **未被改动**
- [x] 自查 边界零 diff（逐项 `git diff --stat`）：`crates/core/**`（含 `workflow/src/prompts/**` 16 份 md、`agent/**` 的 `SessionInjections` / `AgentEvent` / `EngineConfig`）、`crates/infra/agent/src/sdk/{policy,sandbox,protect/**,bash,explore_note,archi_tool,context,compact,resume,log,normalize,web_search,model_spec,preamble}.rs`、`crates/infra/store/src/{change_port,canonical}.rs`、`src-tauri/src/**`（`bindings/**` / `main.rs` / `commands/**` 生产命令面）、`packages/desktop/src/**`、`.dev-team/architecture/{models,api}/**`、全部 `Cargo.toml`（`crates/infra/store/Cargo.toml` 的 `specta-serde` 一行除外）、`Cargo.lock`（`specta-serde 0.0.13` 已在锁内 → 零版本变化；实际 diff = `store` 包依赖清单多一行 `"specta-serde"`，见收敛项 7）。**两处登记的期望随动除外**（非零 diff，且非越界）：`crates/infra/agent/src/sdk/policy_test.rs`（逐工具三档等价表 11 → 13 名）与 `src-tauri/src/commands/db/mod_test.rs`（workspace 维度清单 8 → 11 行）——见 design「编译闭合批」末两行
- [x] 自查 关键判据集中复核（AC 逐条，均为 `grep` / 读码 / `git diff` 面，不含任何用例运行）：
  - **AC-1 / AC-2**：`ModelEntry` 14 行；`describe.rs` 无手写字段表（`grep -c "truncate_doc\|render_type\|field_kind"` 覆盖三函数且被 `tables` / `table` / `field_index` 复用）；`model.rs` 12 条 doc 齐；`session_event` 三字段在 `schema_session_event`
  - **AC-3**：`query` 顶层字段校验命中 `field_index`，`Err` 文案含候选线名（读码复核）；`Format` 唯一应用点在 `describe::formatted_types`
  - **AC-4 / AC-5**：`Op` 十词与 `QUERY_OPS` 一一对应；`like_match` 覆盖四形 + 转义；`resolve_path` 数字段即 `Err`；`order_cmp` 先按 `is_null` 分组（null 恒末位、与 `desc` 无关，`compare_values` 只比非 null 段）；`MAX_QUERY_LIMIT` 越界 `Err`（无 `.min(` 钳位）
  - **AC-6**：预算闸在序列化之前（读码：累计 → 整体序列化 → 防御裁剪），`truncatedBy` 两因分支互斥；`grep -n "MAX_RESULT_BYTES" crates/infra/agent/src/sdk/{db_tool,tools}.rs` 只在 `tools.rs` 定义、`db_tool.rs` 的裁剪处引用
  - **AC-7**：打码与守卫两处皆在 `db_tool.rs`；产品源零命中 = `grep -rn "apiKey" crates/infra/store/src --include=*.rs | grep -v "_test.rs"` 为 **0**（`store` 的测试 sidecar 里出现属测试相位对拍用，非产品面）
  - **AC-8**：`ToolTier::Read`；`input_path` 五名 `None` 臂；`policy.rs` / `protect/**` / `sandbox.rs` 零 diff
  - **AC-9**：explore 九名 / change 恒两名 / archi 与 debug 零名（读码 + `grep`）；`RunStarted.tools` 由 `definitions` 单源产出（零改动）
  - **AC-10**：`global_shared()` 唯一外部 `Arc` 出口；`compose_turn` 的两把手来源正确（`stores.global_shared()` + 既有 `store`）；`db_tool.rs` 零 `Store::open*`
  - **AC-11**：`ToolExec::Db` 的 `None` 臂文案与 `Archi` / `ExploreNote` 同形（`is_error` 回灌）；成员校验零改动
  - **AC-12**：`AgentEvent` / `bindings` / 前端零 diff（阶段四 + 本阶段各一次）；`src-tauri/src/commands/**` 的唯一 diff = `db/mod_test.rs` 的注册表行数期望随动（已登记，见 design「编译闭合批」）
  - **AC-13**：`git diff HEAD -- packages/desktop/package.json` 的 `version` 行为 `0.6.43` → `0.6.44`
- [x] 登记计划外收敛（若有）：实现中若出现「任务未列、实现必需」的最小面，如实留痕于本节（说明为何必需、为何不改变 design 任何裁决的语义面）

  **登记的收敛项（7 条，均为最小面，不改变 design 任何裁决的语义）**：
  1. `envelope::WalkFn` 的访问器入参为 `(key, value)` 两元（任务草案为单参 `&Value`）：草案里 `key_of` 会变成无出口的死参数，而信封 = `{key, value}` 两半；由 `walk_model` 统一构造后交给访问器，`query` 侧零二次调用。
  2. `describe::table` 去掉 `db` 形参（任务草案签名含 `&Database`）：`TableDoc` 无记录计数，`db` 在函数体内全无用途（留之即 `unused_variables`）；`Store::describe_table(&self, table)` 公共签名按任务保留。
  3. `describe::truncate_doc` 增括号平衡收口 `balance`：句读（`。` / `；` / 换行）截断点落在未闭合 `（` 内时回退到该开括号之前——长 doc 的括号普遍跨行，否则「不得截出半句」不成立（如 `agent_run` 的表介绍会停在「（workspace 维度，落所属…」）。
  4. `tools::MAX_RESULT_BYTES` 由私有改 `pub(crate)`：`db_tool` 的末尾防御性裁剪必须引用同一常量（「第二份上限实现」禁令）；`cap_result` 语义零改动。
  5. `mount.rs` 的 `store::QUERY_OPS` 引用两处（description 词表拼接 + `parameters` 里 `op` 的 `enum`）：同一常量两次引用，词表本体仍单源；`op` 的 enum 让工具入参 schema 也封闭。
  6. `db_tool::Scope::Both` 承载「`scope` 缺省」第三态：`db_query` 侧把 `Both` 映射为 workspace（D5 缺省口径），`db_tables` 清单形态映射为两库全列——形态差异只在执行体一处。
  7. `packages/desktop/src-tauri/Cargo.lock` 出现一行 `"specta-serde"`（`store` 包的依赖清单条目）：`specta-serde 0.0.13` 早已在锁内，**零版本变化、零新外部依赖**；该行是 cargo 记录包依赖的机械产物。

  **回溯重做的回归修正（3 条，均在本阶段面内，不改变 design 任何裁决）**：
  1. `describe::render_type` / `field_kind` 增 `inlined` 消解 specta 的「容器内联引用」：`String` / `Vec<T>` 这类容器型的具名侧 `ty = None`，具体形状只挂在引用处（`NamedReferenceType::Inline { dt }`）。修前 `String` 字段渲染成 `String`、比较类落 `Unknown`（`query` 的字符串值类型校验随之整体失效）；现取内联 `dt` 递归，命名枚举 / 命名结构仍渲染类型名（`Verdict` / `内嵌结构`）。
  2. `query::scan` 的自然序保留窗改为「首 `offset + limit` 行」（原为「区间 `[offset, offset+limit)`」），切页的 `skip(offset).take(limit)` 只在 `run` 单处做一次——修前扫掠已跳 offset、`run` 再跳一次，`returned` 恒 0（有 `where` 无 `order` 的三元口径全错）。
  3. `db_query_definition` 描述补 `scope` 两值语义（`"user"` = 全局库 / `"workspace"` = 当前工作区库且**缺省**，维度不符时按报错提示补 scope）——原描述只在参数说明里提 `scope`，主描述缺口径，模型无从得知缺省是 workspace。

  **回溯重做的登记修正（C9：口径收窄 + 两处期望随动登记；代码零回退，不改变 design 任何裁决）**：
  根因是「`src/commands/**` 整目录声明零 diff」与同文风险表「注册表补齐改变查看页可见面」及 AC-1（`MODEL_ENTRIES` 11 → 14）自相矛盾——`src/commands/db/mod_test.rs` 的 8 → 11 行期望是必然随动，非越界改动。收口为纯登记面收敛，产品源零改动：
  1. `proposal.md`「不要修改」首条按文件收窄：`src/commands/**` 只声明**生产命令面**零 diff，并列出两处登记的期望随动（`src/commands/db/mod_test.rs` 注册表行数 8 → 11、`crates/infra/agent/src/sdk/policy_test.rs` 三档等价表 11 → 13），与「测试文件」表末两行、AC-1／AC-12 同口径。
  2. `design.md`「编译闭合批」末两行（挂载清单行数期望 → `policy_test.rs`；注册表行数期望 → `src/commands/db/mod_test.rs`）与时点不变量段（MUST NOT 靠把 `src/commands/**` 整目录或 `sdk/{policy,…}.rs` 通配声明零 diff 来收窄自查口径）；AC-12 落点行同步注明唯一例外。
  3. 本文件自查口径三处同批登记：阶段四自查（`:216` 只允许 `db/mod_test.rs` 一处随动、生产命令面零 diff）、阶段五边界自查（`:224` 两处期望随动除外）、交接备忘的期望值随动条（`:264` 三档口径 3 新建 + 9 既有 sidecar 追加 + 2 既有期望随动 = 14）；`test-design.md` 的三档清单与 AC-8／AC-12「不可测试项」表同批对齐。
  4. 落盘核对（读码，不含用例运行）：`sdk/policy_test.rs` 的对拍用例名为「十三项挂载逐工具三档等价于原三张按名常量表」且 `db_tables` / `db_query` 与原 `READONLY_TOOLS` 六名 + `archi_api` 同列三档全放；`src/commands/db/mod_test.rs` 的 workspace 清单用例为「…返回十一行且与直连 serde 一致」并含 `run_node` / `run_edge` 名集——两处期望随动已真实收敛，登记的收窄口径与盘上事实一致。

  **回溯重做的实现修正（4 条：前 3 条按已通过的 test-design 回正口径，全部落在本阶段面内，不改变 design 任何裁决）**：
  上游指出的 5 处不符中 3 处是实现偏离已通过的 test-design（测试无从按设计落笔）；本相位按设计回正产品源，测试侧断言归 test-gen 重跑随动，test-design 不回退。
  1. `query::check_limit` 去掉自加的下界：spec 只钉上限 1000，`limit = 0` 是合法空页请求（`returned == 0`，`total` / `matched` 仍精确）。双路径均能承载——注册表 `scan` 侧 `scan_model` 的 `take(0)` 留空、`walk` 侧保留窗容量取 `offset + 0` 后由 `run` 单处的 `skip(offset).take(0)` 切空（`run` 内并无第二处钳位）。修前 `limit: 0` 直接 `Invalid`，与 test-design「参数类型 → 边界覆盖口径 · `u32`」与「`query::run` 分页边界」两行相悖；本文件阶段二校验链行内的「`1..=1000`」措辞以 spec 的上限口径为准（下界无出处）。
  2. `db_tool::tables` 的入参形状改为显式 `Err`（新增 `table_of` 单点，与 `scope_of` 同址）：非对象入参 / `table` 非字符串 / `table` 纯空白三形态不再静默退化为 14 行清单，`Err` 文案携期望形状（`{table, scope}`）与「省略该键 = 表清单、填表名 = 字段下钻」的改正指引。修前「参数写错」被伪装成「查询成功」，与 spec「形状错误 → `Err` + 期望形状」及 test-design `db_tables` 错误面一行相悖。
  3. `QuerySpec.conds` 的 `where` 显式 `null` ≡ 缺省（`#[serde(rename = "where", default, deserialize_with = "conds_or_null")]`）：`order` / `select` 走 `Option` 天然把 `null` 落 `None`，`where` 用裸 `Vec` 时同一个「没给」会分叉成「全量」与「形状错误」两种结果。修前 `where: null` 是 serde `Err`，与 test-design `db_query` 入参透传一行（三类 `null` 等价缺省）相悖。`where` 非数组仍 `Err`（`Vec` 的形状错误口径不变）。
  4. `describe::inlined` 去掉可省略的生命周期参数（clippy `needless_lifetimes` 告警清零，签名语义逐字等价）——该函数是上一轮回溯新增的回正实现，告警随之带入。
  既有 sidecar 中编码旧口径的三处断言（`sdk/db_tool_test.rs` 的「视同省略 → 退化为清单形态」与「`where` 显式 `null` → 形状错误」、`store/query_test.rs` 的「`limit = 0` 走显式 `Err`」）性质属测试相位口径，本相位不触碰，由 test-gen 重跑按 test-design 回正。
- [x] 交接交付门（本相位不执行，只登记期望并向桌面端移交）：`pnpm -C packages/desktop run check` 整链（期望：`bindings:export` 零 diff、`api:check` 的 `apiFindings` 若列出 store / agent-runtime 新增公共面属预期且不影响 `passed`、`client:check` 零 diff）归 **static-check**；`cargo test` 全 workspace 与 `pnpm -C packages/desktop test` 归 **test-execution**。两者均不写入任何阶段自查项，也不作为本相位的完成判据。**交付物 = 期望登记与移交**
- [x] 交接备忘（发给 test-design / test-gen / test-execution，本相位不写用例；**交付物 = 备忘登记与移交**）：
  - store：`describe_test.rs`（新，含 ratchet 双向断言：每字段 doc 非空 + 线名集 ↔ 真实 `serde_json` 键集；14 行清单与 `store_meta` 缺席；内嵌类型不入清单；`session_event` 三字段；doc 句读截断）、`query_test.rs`（新，op 全集 / AND / 下钻 / `LIKE` 四形与 `\\%` `\\_` / `IN` / `IS NULL` 两态 / 类型不符 `Err` 携类型 / 排序升降与 null 末位 / 投影 `key` 恒在 / 分页与上限越界 / `total`·`matched`·`returned` 三元 / 未知表字段 op 候选文案 / 只读零写面）、`envelope_test.rs`（注册表 14 行 + 三新表 `key_of`）、`model_test.rs`（补 doc 后逐字节不变 + `specta(type = String)` 字段线格式）、`store_test.rs`（`global_shared()` 不二次 open：同一 `Arc` 指针 / 单开纪律）
  - agent：`sdk/db_tool_test.rs`（新，预算闸 / 两因截断 / 无半截 JSON / 打码含投影 / `apiKey` 比较面 `Err` / 错误文案 / `scope` 三态 / 两形态）、`sdk/mount_test.rs`（两项在清单 / tier / 三档放行 / schema 形状）、`sdk/tools_test.rs`（`input_path` 两名 `None`）、`sdk/loop_test.rs`（`ToolExec::Db` 两态）、`sdk/runner_test.rs`（`mint_db_context` 三态）、`compose_test.rs`（explore 9 / change 恒 2 / archi 与 debug 零）
  - 落点差异提醒：proposal 把 describe / query 用例写在 `envelope_test.rs`，实现按 design DD1 落到 `describe.rs` / `query.rs`，故宜建 `describe_test.rs` / `query_test.rs` 并在 `store/src/lib.rs` 的 `#[cfg(test)]` 区同批加声明（字母序），`envelope_test.rs` 只保留注册表面 [design 待决问题 1]
  - 期望值随动（**非新增用例、非口径改写**，与上列新建 / 追加**不同批性质**，须在清单里单列）：`crates/infra/agent/src/sdk/policy_test.rs`（逐工具三档等价表随挂载清单 11 → 13 名补齐，两 DB 工具归 `Read` 三档全放）、`src-tauri/src/commands/db/mod_test.rs`（workspace 维度 `db_models` 清单 8 → 11 行、计数向量与用例名同步）。故清单口径 = **3 新建 + 9 既有 sidecar 追加 + 2 既有期望随动 = 触及 14 个测试文件**（与 proposal「测试文件」表 14 行一一对应）；test-design 的「3 新建 + 9 既有 sidecar」表述需按此同批登记，否则那两行会被误判为越界改动
