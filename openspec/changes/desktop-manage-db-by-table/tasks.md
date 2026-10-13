# 任务: desktop-manage-db-by-table

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件（proposal「测试文件」五项：`db-inspector-view.test.tsx` / `use-db-inspector.test.ts` / `app.test.tsx` / `commands/db/mod_test.rs` / `envelope_test.rs` 归测试轨道）。顺序按依赖排列：store 分页信封 → 命令层与 bindings 再生 → 前端分页态 → 自动表格与抽屉 → 守线。版本 bump 归档轨道（AC-9 / design DD7），随归档提交执行，不在本列表。

## 阶段一：store 分页信封（AC-3）

- [x] 修改 `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`：新增 `RecordsPage` DTO（`#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`，`records: Vec<RecordEnvelope>` + `total: u64`，与 `ModelInfo` / `RecordEnvelope` 同出口口径）；`ScanFn` fn-pointer 返回类型改 `Result<RecordsPage, StoreError>`；`scan_model` 在同一 `r_transaction` 内先 `r.len().primary::<T>()` 取 total 再 `skip(offset).take(limit)` 取区间记录组装 `RecordsPage`（design DD1）；`envelope::scan` 返回 `RecordsPage`（未知模型 reject / `limit.min(MAX_SCAN_LIMIT)` 截断 / 11 个 per-model 包装返回类型随动与 `key_of` 构造零改动）；`CountFn` / `count_model` 保留供 `list_models`（清单计数口径零改动）；模块 doc「分页信封」措辞随动
- [x] 修改 `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`Store::scan` 返回类型改 `Result<RecordsPage, StoreError>`（入参不变、薄转调零逻辑改动），import 增 `RecordsPage`（`RecordEnvelope` 若无他用随删），doc 注释「分页信封（records + total 同一读事务）」措辞随动（design DD1；proposal 实现文件清单外的签名随动触点）
- [x] 修改 `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：`pub use envelope::{ModelInfo, RecordEnvelope, RecordsPage};`（design 修改文件表）

## 阶段二：命令层与 bindings 再生（AC-3）

- [x] 修改 `packages/desktop/src-tauri/src/commands/db/mod.rs`：`db_records` 返回 `Result<RecordsPage, String>`（薄透传 `Store::scan`）；`Workspace blank root` 分支返回 `RecordsPage { records: Vec::new(), total: 0 }` 空信封（design DD2）；import 改 `RecordsPage`；doc 措辞随动——scope 寻址两库 / 未知模型 reject / Global 忽略 root / `Result<T, String>` 错误模板 / 只读纪律零改动
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生 `packages/desktop/src/types/generated/bindings.ts`（非手改）：新增 `RecordsPage` 类型 + `dbRecords` 返回类型改 `RecordsPage`；`dto.ts` 纯 `export type *` re-export 自动跟随、零手改
- [x] 修改 `packages/desktop/src-tauri/src/bindings/mod_test.rs`：`DTO_TYPES` 断言清单插入 `"RecordsPage"`（`"RecordEnvelope"` 之后，字母序；design DD6——与生成物强耦合的覆盖清单随动，非新用例）；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化

## 阶段三：前端分页态 total 导出（AC-4）

- [x] 修改 `packages/desktop/src/views/db/hooks/use-db-inspector.ts`：`useDbRecords` 消费信封并存 `total` 快照（未选中置 0）；`DbInspectorState` 增 `total: number` / `page: number`（`offset / PAGE_SIZE + 1`）/ `totalPages: number`（`Math.max(1, Math.ceil(total / PAGE_SIZE))`）；`hasMore` 改 `offset + records.length < total`，删除 `records.length === PAGE_SIZE` 页长启发式（design DD5）；`PAGE_SIZE` 保持模块私有；`DbInspectorState` doc / 注释措辞随动；scope / root 复位、用户显式动作触发、无轮询、查询轨 inline 持久错误语义零改动

## 阶段四：自动表格 + 抽屉详情（AC-1 / AC-2 / AC-5 / AC-6 / AC-7）

- [x] 修改 `packages/desktop/src/views/db/db-inspector-view.tsx` 记录区为 shadcn 自动表格 + 真实页界 + 抽屉详情（design DD3 / DD4）：
  - 模块私有 `deriveColumns(records: RecordEnvelope[]): string[]`——`key` 恒首列、对象 `value` 行逐行逐键首见追加并集列、缺失键空单元格、非对象 `value` 行经 `value` 尾列承载摘要（全非对象页恰 `key` + `value` 两列）；表格用 `components/ui/table.tsx` 七件套（`Table` / `TableHeader` / `TableBody` / `TableRow` / `TableHead` / `TableCell`），零模型特定分支、不为任何模型手写列定义（AC-1）
  - 单元格一律 `valueSummary`（120 截断口径零改动：短于原样 / 恰 120 不截 / 121 截断加省略号）（AC-2）
  - 分页控件改真实页界：「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页（禁用边界 = `offset === 0` / `!hasMore`，均 total 导出）（AC-4）
  - 单实例受控 `StandardDrawer`（无 trigger、`containerClassName` 右侧滑出宽度、footer 恒「关闭」由 standard 契约承载）：点行置 `selectedIndex` 并开抽屉、行呈选中态（`data-state="selected"`）；title = `JSON.stringify(envelope.key)`、content = `JSON.stringify(envelope, null, 2)` 的 `<pre>`（`data-testid="db-record-json"`）；`open ⇔ selectedIndex !== null`、`onOpenChange(false)` 清选中（AC-5）
  - 列表换血收起：`useEffect(() => setSelectedIndex(null), [state.selected, state.offset, state.records])`（换模型 / 翻页 / 切 scope / 刷新即收起、详情不残留，AC-6）
  - `RecordRow` / `RecordDetail` 退役（行内 JSON 详情块删除，AC-6）；DOM 锚点随动：新增 `db-records-table` / `db-record-row`，退役 `db-record-item` / `db-record-detail`，其余锚点零改动；空态 `db-records-empty` / inline 持久错误 / 只读零写入口 / 显式动作无轮询零改动（AC-7）

## 阶段五：守线与静态自查（静态，不含测试执行）

- [x] 测试文件类型级最小适配（使 `pnpm -C packages/desktop run client:check` 类型面恢复编译绿）：`use-db-inspector.test.ts` / `db-inspector-view.test.tsx` / `app.test.tsx` 的 `dbRecords` mock 与夹具改 `{ records, total }` 信封形状、`DbInspectorState` 夹具补 `total` / `page` / `totalPages` 字段——仅形状级适配，不改写断言语义；断言重写与新用例（表头推导 / 单元格退化 / 抽屉开合 / 换血收起 / 真实页界边界）归 test-design 派生（AC-8）；适配完成后留痕于本任务完成备注
  - 完成备注：三文件仅 mock / 夹具形状级适配（`mockDispatch` / `mockIpc` / app 分发器的 `db_records` 分支改回 `{ records, total }` 信封、在途 resolver 泛型改 `RecordsPage`；`RecordsPage` 类型 import 随动），零断言改写。`DbInspectorState` 夹具补字段一项无实际落点——测试经真实 hook 组合，仓库无 state 对象夹具。测试轨道需注意：旧启发式语义断言（整页倍数第 2 页 `hasMore=true`、「offset N」分页文案、`db-record-item` / `db-record-detail` 锚点、行 className 精确匹配等）与新 DOM / 新页界口径不一致，按任务声明归 test-design 重写。
- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（vp check + knip，零新增豁免条目）；前端与 Rust 自动化套件全绿、覆盖率（lines 90 / branches 85 / functions 90）与 mutation score 80 达标归 test-execution 阶段（AC-8 执行面），不入本任务面
- [x] bindings 重导幂等自查：再跑 `pnpm -C packages/desktop run bindings:export` 零新增 diff，`bindings.ts` 变化仅 `RecordsPage` 类型新增 + `dbRecords` 返回类型升级（design DD1 面）
- [x] 静态自查：禁改面零 diff——`packages/desktop/src-tauri/crates/infra/store/src/model.rs` 与 `MODEL_ENTRIES` 注册机制（无模型字段元数据）、`packages/desktop/src/components/ui/**`（含 `table.tsx` / `drawer.tsx`）与 `packages/desktop/src/components/standard/**`（`StandardDrawer` / `useOpen` 契约零改动）、`packages/desktop/src/routes.tsx` 与侧栏入口、db 轨道零写命令（`commands/db/mod.rs` 只读纪律）、`openspec/specs/` 本体、`plugins/dev-team`（保持 2.10.44）；`openspec/config.json` 无新增 excludes / 豁免
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏，含 `store.rs` / `bindings/mod_test.rs` 两个随动触点）；`packages/desktop/package.json` 实现期零触点（version 0.5.1 → 0.5.2 归档轨道执行，AC-9 / design DD7）
