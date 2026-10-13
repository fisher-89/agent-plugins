# 设计: desktop-manage-db-by-table

> **变更**: desktop-manage-db-by-table
> **日期**: 2026-10-13
> **基于**: proposal.md、specs/desktop-db-inspector/spec.md（delta）、specs/desktop-workspace-store/spec.md（delta）

---

## 提案与规格同步状态

`proposal.md` 与两份能力 delta（`specs/desktop-db-inspector/spec.md` / `specs/desktop-workspace-store/spec.md`）已由提案阶段定稿，本设计基于其文本展开。工作树实况（本变更起点，`packages/desktop` 0.5.1 基线）：

- 记录区为 `RecordRow`（`<button>` 行）平铺 + `RecordDetail` 内嵌 `<pre>` 块（`db-inspector-view.tsx`），无表格结构。
- `use-db-inspector.ts` 以 `hasMore: records.length === PAGE_SIZE` 页长启发式推断页界（:171），分页控件仅呈 `offset` 数字（无页码 / 总条数）。
- `Store::scan`（`store.rs`）/ `envelope::scan`（`envelope.rs`）/ `db_records`（`commands/db/mod.rs`）返回裸 `Vec<RecordEnvelope>` / `RecordEnvelope[]`，无页界元数据；`scan_model` 与 `count_model` 各开各的读事务。
- `components/ui/table.tsx`（shadcn vendored 八件套）与 `components/standard/drawer.tsx`（`StandardDrawer`：trigger 可选、footer 恒「关闭」、`useOpen` 归一）既有可用，proposal 将其列为引用沿用 / 禁改面。
- `bindings/mod_test.rs` 的 `DTO_TYPES` 为「产物 Types 段 `export type` 全集」contains 断言清单，出线类型增删须随动（desktop-drawer-session-column 先例）。

proposal「假设声明」与 D1 已定稿列推导口径 = 数据驱动（当前页键并集），schema 字段清单方案被拒（D1 被拒备选）；本设计按此展开，不复议。

---

## 关键设计决策（proposal 定稿 + 本设计展开）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| DD1 | `total` 同读事务的产出点 | `envelope.rs` 的 `scan_model<T>` 在**同一 `r_transaction`** 内先 `r.len().primary::<T>()` 取 total 再 `skip(offset).take(limit)` 取区间记录，组装 `RecordsPage`；`ScanFn` fn-pointer 返回类型改 `Result<RecordsPage, StoreError>`，11 个 per-model 包装随之返回 `RecordsPage`（签名同步、零新分支）；`CountFn` / `count_model` 保留供 `list_models`（清单计数口径零改动） | proposal D2 的落点选择：records + total 同事务快照必须在扫描发生处组装，命令层 / 前端拼装均跨事务不自洽（D2 被拒备选已论证）；fn-pointer 返回类型随动是类型系统强制的最小触点 |
| DD2 | 命令层 blank root 空结果形态 | `db_records` 的 `Workspace blank root` 分支返回 `RecordsPage { records: Vec::new(), total: 0 }` 空信封（原裸 `Vec::new()`） | 信封是接口一等契约后，空结果也须是合法信封（total 0 自然导出「无下一页」）；保持「blank root 不触发库解析」语义零变化 |
| DD3 | 表格列推导实现点与混合页退化 | 列推导为 `db-inspector-view.tsx` **模块私有纯函数** `deriveColumns(records: RecordEnvelope[]): string[]`（不出模块、不建独立组件文件）：`key` 恒首列；对象 `value` 行逐行逐键扫描、未见键追加尾列（首见稳定序）；`value` 顶层非对象（null / 数组 / 标量）的行经 `value` **尾列**承载摘要——全部行非对象时恰为 `key` + `value` 两列（即 spec「单 `value` 摘要列」退化），混合页并集列 + `value` 尾列并存 | 单处消费（Simplicity first）；「首见稳定序」在 serde_json 默认 `Map`（BTreeMap，键按字节序迭代）+ IPC JSON 保序下是确定性口径，逐行逐键首见追加即满足；混合页把退化列固定为尾列避免列位漂移；列名即键名，对象键恰为 `"value"` 时并入同列（debug 查看器可接受的同名合并，见数据模型留痕） |
| DD4 | 行选中与抽屉开合 = 单状态 | `RecordsSection` 持唯一状态 `selectedIndex: number \| null`（`state.records` 下标）；抽屉 `open ⇔ selectedIndex !== null`，`onOpenChange(false)` 即清选中；点行置 `selectedIndex`（同页内点击他行即换详情）；列表换血收起 = `useEffect(() => setSelectedIndex(null), [state.selected, state.offset, state.records])` | 「行选中态」与「抽屉开」由 spec 绑定（点行选中并开、关闭即解除），单状态结构上杜绝双态漂移；换血信号取 `state.records` 引用变更（每次成功取数新数组）覆盖换模型 / 翻页 / 切 scope / 刷新四动作，迁移既有 `openIndex` 复位语义且不需 hook 暴露新面 |
| DD5 | 页界导出集中在 hook | `useDbInspector` 返回面增 `total: number`（接口信封 `total` 快照）、`page: number`（`offset / PAGE_SIZE + 1`）、`totalPages: number`（`Math.max(1, Math.ceil(total / PAGE_SIZE))`）；`hasMore` 改 `offset + records.length < total`；`PAGE_SIZE` 保持模块私有不出模块；分页控件呈现「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页 | spec「页码 / 总页数由 offset 与 ceil(total / PAGE_SIZE) 导出」的单点落位：派生口径归 hook（PAGE_SIZE 单一持有），view 只呈现；`max(1, …)` 保证空数据页界不除零、不呈 0 页 |
| DD6 | `bindings/mod_test.rs` DTO_TYPES 清单随动 | `DTO_TYPES` 插入 `"RecordsPage"`（`"RecordEnvelope"` 之后，字母序）；属与生成物强耦合的覆盖清单随动（cfg(test) 断言、非新用例），随实现阶段处理 | 先例（desktop-drawer-session-column D8）：出线类型增删不随动清单即破坏「全集」纪律；proposal 未把它列入测试文件面，按实现清单随动 |
| DD7 | 版本 | `packages/desktop/package.json` version 0.5.1 → 0.5.2 **随归档提交执行**（归档轨道，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 引用自动跟随、零改动，`src-tauri/Cargo.toml` 不随动） | proposal AC-9 / 仓库既有 bump 口径（用户可见变更才提升） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 分页信封 DTO | `RecordsPage { records, total }`（与 `ModelInfo` / `RecordEnvelope` 同出口；camelCase、specta Type）（DD1） | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | serde / serde_json / specta | Rust（出线 DTO） |
| 记录信封读面 | `envelope::scan` 返回 `RecordsPage`：`scan_model` 同读事务取 total（`r.len().primary::<T>()`）与区间记录（`skip/take`）；`ScanFn` fn-pointer 返回类型随动；未知模型 reject / MAX_SCAN_LIMIT 截断 / per-model key_of 零改动 | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | native_db、`MODEL_ENTRIES` 注册表 | Rust（通用读面） |
| 通用读面收口 | `Store::scan` 返回类型改 `Result<RecordsPage, StoreError>`（薄转调 `envelope::scan`，语义零改动） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `crate::envelope` | Rust（impl 方法） |
| 公共导出 | `RecordsPage` 随 `ModelInfo` / `RecordEnvelope` re-export（native_db 类型不越公共签名） | `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `envelope` | Rust（mod 出口） |
| db 命令轨道 | `db_records` 返回 `Result<RecordsPage, String>`（薄透传；blank root → 空信封，DD2）；只读轨道纪律零改动 | `packages/desktop/src-tauri/src/commands/db/mod.rs` | tauri `State`、store | Rust（Tauri command） |
| bindings 生成物 | `RecordsPage` 类型 + `dbRecords` 返回类型出线（specta 再生成，非手写） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings crate | TypeScript（生成物） |
| bindings 覆盖清单 | `DTO_TYPES` 插入 `"RecordsPage"`（DD6 随动，cfg(test)） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 生成物文本断言 | Rust（cfg(test) 断言） |
| 分页态 hook | `useDbInspector` 分页面以接口 `total` 为准（`total` / `page` / `totalPages` / `hasMore` 导出，DD5），废除页长启发式；显式动作触发 / 无轮询不变 | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | `bindings`（commands.dbModels / dbRecords） | React hook（TypeScript） |
| DB 查看页 | shadcn 自动表格（`deriveColumns` 数据驱动列推导 + `valueSummary` 单元格摘要）+ 真实页界分页控件 + 单实例受控 `StandardDrawer` 单条详情（DD3 / DD4）；`RecordRow` / `RecordDetail` 退役 | `packages/desktop/src/views/db/db-inspector-view.tsx` | `ui/table`、`ui/button`、`standard/drawer`、`useDbInspector` | React 组件（TSX） |
| shadcn 表格底座（引用沿用，零改动） | `Table` / `TableHeader` / `TableBody` / `TableRow` / `TableHead` / `TableCell` 承载记录表格；测试经真实组件渲染、MUST NOT mock（desktop-app-shell vendored 纪律） | `packages/desktop/src/components/ui/table.tsx` | `cn` | TypeScript、React |
| 标准抽屉（引用沿用，零改动） | `StandardDrawer` 承载单条详情（无 trigger 受控式、右侧滑出、footer 恒「关闭」、`useOpen` 归一） | `packages/desktop/src/components/standard/drawer.tsx` | `ui/drawer`、`ui/button`、`hooks/use-open` | TypeScript、React |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件（proposal「测试文件」五项：db-inspector-view.test.tsx / use-db-inspector.test.ts / app.test.tsx / commands/db/mod_test.rs / envelope_test.rs）归 test-design / test-gen / test-execution 阶段，不在本清单；`bindings/mod_test.rs` 的 DTO_TYPES 清单因与生成物强耦合按实现清单随动（DD6）。 -->

### 新增文件

<!-- 整段省略：本变更零新增文件——RecordsPage 为既有 envelope.rs 内新增 DTO、表格与抽屉为既有 db-inspector-view.tsx 内改造（deriveColumns 等为模块私有函数），不建独立组件 / hook 文件（DD3 / DD4，Simplicity first）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | 新增 `RecordsPage` DTO；`ScanFn` fn-pointer 返回类型改 `Result<RecordsPage, StoreError>`；`scan_model` 同读事务内取 total（`r.len().primary::<T>()`）与区间记录组装 `RecordsPage`；`envelope::scan` 返回 `RecordsPage`（AC-3 / DD1） | `CountFn` / `count_model` 保留供 `list_models`（清单计数口径零改动）；未知模型 reject、`limit.min(MAX_SCAN_LIMIT)` 截断、11 个 per-model 包装（返回类型随动）与 `key_of` 构造零改动；`MODEL_ENTRIES` 注册表零改动；模块 doc「分页信封」措辞随动 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `Store::scan` 返回类型 `Result<Vec<RecordEnvelope>, StoreError>` → `Result<RecordsPage, StoreError>`；import 增 `RecordsPage`（`RecordEnvelope` 若无他用随删）；doc 注释措辞随动（AC-3） | proposal 实现文件清单外的签名随动触点（spec 模块契约点名 `Store::scan -> RecordsPage`，签名不随动不编译）；`list_models` 与其余方法零改动 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `pub use envelope::{ModelInfo, RecordEnvelope, RecordsPage};`（AC-3） | 公共导出单点；native_db / native_model 类型不越公共签名 |
| `packages/desktop/src-tauri/src/commands/db/mod.rs` | `db_records` 返回 `Result<RecordsPage, String>`（薄透传 `Store::scan`）；blank root 分支返回 `RecordsPage { records: Vec::new(), total: 0 }` 空信封（DD2）；import 改 `RecordsPage`；doc 措辞随动（AC-3） | scope 寻址两库 / 未知模型 reject / Global 忽略 root / `Result<T, String>` 错误模板 / 只读纪律零改动 |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生（非手改）：新增 `export type RecordsPage = { records: RecordEnvelope[], total: number }`、`dbRecords` 返回类型改 `RecordsPage`（AC-3） | `dto.ts` 纯 `export type *` re-export 自动跟随、零手改；重导幂等是守线验收面 |
| `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | `useDbRecords` 存接口信封 `total`；`DbInspectorState` 增 `total` / `page` / `totalPages`，`hasMore` 改 `offset + records.length < total`；删除 `records.length === PAGE_SIZE` 启发式；派生口径集中 hook（DD5，AC-4） | `PAGE_SIZE` 保持模块私有；scope / root 复位、显式动作触发、无轮询、查询轨 inline 持久错误语义零改动 |
| `packages/desktop/src/views/db/db-inspector-view.tsx` | 记录区改 shadcn 自动表格 + 真实页界分页控件 + 单实例受控 `StandardDrawer` 详情（AC-1 / AC-2 / AC-4 / AC-5 / AC-6）：模块私有 `deriveColumns`（`key` 恒首列 + 当前页对象键并集首见稳定序、缺失空单元格、非对象行经 `value` 尾列退化，DD3）；单元格 `valueSummary`（120 截断口径零改动）；分页控件文本「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页（DD5）；`RecordRow` / `RecordDetail` 退役（行内 JSON 详情块删除）；`selectedIndex` 单状态 + 抽屉 `open ⇔ selectedIndex !== null`（DD4），title = `JSON.stringify(envelope.key)`、content = `JSON.stringify(envelope, null, 2)` 的 `<pre>`；换血收起 effect deps `[state.selected, state.offset, state.records]` | DOM 观察面：既有 `db-model-item` / `db-model-list` / `db-models-loading` / `db-scope-workspace` / `db-scope-user` / `db-refresh` / `db-page-prev` / `db-page-next` / `db-records-loading` / `db-records-empty` / `db-inspector-error` 锚点零改动；新增 `db-records-table`（表格容器）、`db-record-row`（表格行）；`db-record-json`（抽屉 content `<pre>`）沿用旧名；`db-record-item` / `db-record-detail` 随 `RecordRow` / `RecordDetail` 退役。空态 / inline 持久错误 / 只读 / 无轮询（AC-7）零改动 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | `DTO_TYPES` 断言清单插入 `"RecordsPage"`（`"RecordEnvelope"` 之后，DD6） | 与生成物强耦合的覆盖清单随动，非新用例；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化 |
| `packages/desktop/package.json` | version 0.5.1 → 0.5.2 | **归档轨道**（AC-9 / DD7），随归档提交执行，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 引用零改动，`src-tauri/Cargo.toml` 不随动 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 签名为 TS 语法。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `Store::scan` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 修改 | `pub fn scan(&self, model: &str, offset: u32, limit: u32) -> Result<RecordsPage, StoreError>` | 入参不变；返回值从 `Vec<RecordEnvelope>` 升级为分页信封（records = 区间信封、total = 全量计数，同一读事务快照）；未知模型 Err / MAX_SCAN_LIMIT 截断语义不变（AC-3） |
| `db_records` | `packages/desktop/src-tauri/src/commands/db/mod.rs` | 修改 | `pub fn db_records(stores: State<'_, WorkspaceStores>, scope: DbDimension, root: String, model: String, offset: u32, limit: u32) -> Result<RecordsPage, String>` | Tauri command（IPC）；入参不变，返回 `Result<RecordsPage, String>`（薄透传）；blank root → 空信封（DD2），其余 reject 语义不变（AC-3） |
| `useDbInspector` | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | 修改 | `export function useDbInspector(root: string): DbInspectorState` | 入参不变；返回面增 `total: number` / `page: number` / `totalPages: number`，`hasMore` 改 total 导出（`offset + records.length < total`）（AC-4 / DD5） |
| `DbInspectorView` | `packages/desktop/src/views/db/db-inspector-view.tsx` | 修改 | `export function DbInspectorView({ root }: DbInspectorViewProps): React.JSX.Element` | props 签名不变；记录区改自动表格 + 真实页界 + 抽屉详情（AC-1 / AC-2 / AC-4 / AC-5 / AC-6） |

<!-- `envelope::scan`（pub(crate) 模块内分发）、`scan_model` / `count_model` / per-model 包装（模块私有）、`deriveColumns` / `valueSummary` / `rowKey` / `ModelItem` / `ScopeToggle` / `ModelListSection` / `RecordsHeader` / `RecordsSection`（模块私有，改动在修改文件表与类型定义表说明）不列；`useOpen` / `StandardDrawer` / `ui/table` 组件族为引用沿用零改动不列。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `RecordsPage` | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | 新增 | 出线 DTO：`pub struct RecordsPage { pub records: Vec<RecordEnvelope>, pub total: u64 }`，`#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`（与 `ModelInfo` / `RecordEnvelope` 同口径）（AC-3） |
| `RecordsPage`（TS 生成物镜像） | `packages/desktop/src/types/generated/bindings.ts` | 新增（specta 再生成） | `export type RecordsPage = { records: RecordEnvelope[], total: number }`；`dbRecords` typed 包装返回 `__TAURI_INVOKE<RecordsPage>`（AC-3） |
| `ScanFn` | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | 修改 | 模块私有 fn-pointer 别名：`fn(&Database<'static>, u32, u32, KeyOfFn) -> Result<RecordsPage, StoreError>`（DD1；`CountFn` / `KeyOfFn` 不变） |
| `DbInspectorState` | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | 修改 | interface 增 `total: number`（全量计数快照）、`page: number`（1 起）、`totalPages: number`；`hasMore: boolean` 语义改为 total 导出（doc 随动）；其余成员不变（AC-4 / DD5） |

<!-- `RecordEnvelope` / `ModelInfo` 复用零变化不列；`DbInspectorViewProps` 签名零变化不列；`deriveColumns` 返回 `string[]` 为模块私有纯函数不列（无新类型）。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改（归档时执行） | string | `0.5.2`（现行 `0.5.1`） | 版本交付（AC-9 / DD7）：0.5.1 → 0.5.2；`src-tauri/tauri.conf.json` `"version": "../package.json"` 引用维持、自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动 |
| `tests[]`（desktop 套件） | `openspec/config.json` | 既有（零改动） | object | root `packages/desktop`、framework `vite-plus`、coverage `{lines 90, branches 85, functions 90}`、mutation `{score 80}`、excludes 现状仅 `src/components/ui/**` 与 `src/routes.tsx` | AC-8 达标口径来源；MUST NOT 加 excludes / 豁免 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `RecordsPage` 分页信封（新增） | `records: Vec<RecordEnvelope>`（offset / limit 区间记录信封）+ `total: u64`（该模型全量计数） | 由 `scan_model` 同一读事务组装；`Store::scan` → `db_records` 薄透传 → bindings TS `RecordsPage`；前端 `useDbRecords` 消费 `records` + `total` | 不持久化（IPC 瞬时面）；快照级一致（并发写下页间漂移接受，proposal R2） |
| `RecordEnvelope`（复用，零变更） | `key: Value` + `value: Value`（TS：`unknown`） | 表格行 / 抽屉详情的唯一数据载体；`key_of` 从记录 JSON 构造主键 | 不持久化（IPC 瞬时面）；native_db 类型不越信封 |
| 前端分页态（修改） | `offset`（当前页起始，PAGE_SIZE 50）+ `total` + `page`（`offset / PAGE_SIZE + 1`）+ `totalPages`（`max(1, ceil(total / PAGE_SIZE))`）+ `hasMore`（`offset + records.length < total`） | 全部由接口 `total` 与 `offset` 导出（DD5）；换模型 / 切 scope 复位 offset | 内存态（React state） |
| 行选中 / 抽屉开合（新增视图态） | `selectedIndex: number \| null`（`state.records` 下标） | 抽屉 `open ⇔ selectedIndex !== null`；`onOpenChange(false)` 清选中；换血（`state.selected` / `state.offset` / `state.records` 变更）清选中（DD4） | 内存态（React state） |
| 表格列视图模型（新增派生面） | `columns: string[]`（`key` + 当前页对象键并集 [+ `value` 尾列]） | `deriveColumns(records)` 纯函数按当前页派生；无跨页记忆（列随页数据换血，proposal R1 锁定口径） | 不持久化（运行时派生） |

**留痕（非待决）**：

- 列名即键名——对象 `value` 的顶层键恰为 `"value"` 时与退化列同名并入同列（DD3）；debug 查看器可接受，不引入转义 / 前缀命名。
- serde_json 默认 `Map`（BTreeMap）使单行内键按字节序迭代，「首见稳定序」= 逐行（记录序）逐键（该行键序）首见追加——确定性口径，测试断言以此为准。
- 抽屉 title 为 key 完整 JSON 文本（不截断，主键短小）；表格 key 列单元格与字段单元格一律 `valueSummary` 120 截断（AC-2 口径）。
- 刷新失败（recordsError）不换血：`records` 保持原引用、抽屉保留现状，详情不越出当前列表（「详情不残留」按换血口径）。

---

## 路由 / API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）单命令输出升级，入参与其余命令零变化：

| IPC 命令 | method / path | 说明 | 输入 | 输出 | auth |
|----------|---------------|------|------|------|------|
| `db_records` | invoke `db_records` | 按模型主键自然序分页扫描（分页信封） | `scope: DbDimension, root: string, model: string, offset: u32, limit: u32`（不变） | `RecordsPage { records, total }`（原 `RecordEnvelope[]`；blank root → `{ records: [], total: 0 }`） | 本机 IPC 无鉴权（同既有命令口径） |
| `db_models` | invoke `db_models` | 模型清单与计数 | `scope: DbDimension, root: string`（不变） | `ModelInfo[]`（不变） | 本机 IPC 无鉴权 |

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | DD3 + 架构组件「DB 查看页」：shadcn `ui/table` 组件族 + `deriveColumns`（`key` 恒首列、并集首见稳定序、缺失空单元格）；零模型特定分支由「列推导只吃 `RecordEnvelope.value` 键」结构保证；用例归测试轨道 |
| AC-2 | `valueSummary` 120 口径沿用（`> 120` 截断加省略号，恰 120 不截）；非对象退化 = `value` 尾列（DD3 混合页规则；全非对象页恰两列） |
| AC-3 | DD1（`scan_model` 同读事务 total）+ DD2（blank root 空信封）+ 修改文件表四 Rust/生成物触点；bindings 再生同步、`bindings:check` 干净由守线 + test-execution 验证 |
| AC-4 | DD5（`hasMore = offset + records.length < total`、`page` / `totalPages` 由 total 导出、`PAGE_SIZE` 不出模块、源码无页长启发式）+ 分页控件「第 {page} / {totalPages} 页（共 {total} 条）」；整数倍边界（100 条 / 50 末页 next 禁用）由 `hasMore` 公式直接锁定 |
| AC-5 | DD4：点行 → 行选中态（`data-state="selected"`）+ `StandardDrawer` 右侧滑出（title = key JSON 文本、content = 信封 pretty JSON、footer 恒「关闭」由 standard 契约承载）；关闭清选中 |
| AC-6 | DD4 换血收起 effect（`state.selected` / `state.offset` / `state.records`）；`RecordRow` / `RecordDetail` 退役 = 行内无 JSON 详情块（详情仅抽屉内） |
| AC-7 | 零写入口（只读轨道纪律与表格 / 抽屉均无写操作面）、空态 `db-records-empty`、inline 持久错误（清单 / 记录 reject、库打开失败）、显式动作触发无轮询——全部沿既有结构零改动（修改文件表标注） |
| AC-8 | 测试文件归测试轨道（proposal「测试文件」五项 + fixture 形状随 DTO）；守线阶段静态面 = `server:check` / `client:check` 零新增豁免 + bindings 重导幂等 + 测试文件类型级最小适配；覆盖率 / mutation / 套件全绿归 test-execution |
| AC-9 | DD7 + 配置表：version 0.5.1 → 0.5.2 归档轨道执行 |

---

## 依赖

### 运行时依赖

- 零新增 npm / crate 依赖：表格用 vendored `components/ui/table.tsx`（既有）、抽屉用 `components/standard/drawer.tsx`（既有）；Rust 侧 `serde` / `serde_json` / `specta` / `native_db` 全部既有（`RecordsPage` 与 `RecordEnvelope` 同 derive 面）。

### 构建 / 测试依赖

- 零新增；沿既有工具链：`pnpm -C packages/desktop run bindings:export`（specta 再生成）、`server:check`（cargo fmt + clippy）、`client:check`（vp check + knip）。
- 测试面（test-design / test-gen / test-execution 承接）沿 `vp test` + `@testing-library/react` + coverage / mutation 既有基座，本设计不展开。
- 静态脚本（static-check / test-execution 等机械步骤）由桌面端执行，agent 内不重复执行。

---

## 待决问题

- **R6（proposal 风险表遗留）explore 解读口径确认**：「根据 model 模型自动生成表格」若产品意图是跨页稳定列序的 schema 列（`ModelInfo` 字段元数据驱动），本变更的数据驱动列推导与意图存在偏差面；假设已按 D1 定稿执行（数据驱动列推导），若确认为 schema 列需求，后续变更以 `ModelInfo` 字段元数据 additive 扩展列来源，本变更的表格组件 / 抽屉 / 页界契约（AC-1 ~ AC-7）不受影响。本设计不复议。
- 无其余未决事项：proposal「假设声明」与 D1 ~ D4 已定稿；本设计展开点（DD1 ~ DD7）均已定稿，无 `(待确定)` 项。
