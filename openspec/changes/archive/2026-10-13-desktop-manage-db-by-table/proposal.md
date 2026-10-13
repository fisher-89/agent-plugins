# 提案: desktop-manage-db-by-table

> **变更**: desktop-manage-db-by-table
> **日期**: 2026-10-13
> **状态**: draft
> **探索**: `openspec/changes/desktop-manage-db-by-table/explore.md`（一句话目标：packages/desktop 查看数据库页面优化——根据 model 模型自动生成表格（ui 使用 shadcn）、数据接口带分页、使用抽屉查看详情）

---

## 问题

DB 查看页（`DbInspectorView`，能力 desktop-db-inspector）的记录区当前是「按钮行 + 截断 JSON 摘要」的平铺清单，explore 三项优化各自对应一个现状缺口：

1. **无表格结构，记录不可对照**：记录区为 `<button>` 行平铺（`RecordRow`），字段不成列、记录间无法横向对照，多字段记录只能读 120 字符截断摘要；不存在「根据模型自动生成的表格」，也没有用 shadcn Table 组件族承载记录列表。
2. **数据接口分页无页界元数据**：`db_records` 返回裸 `Vec<RecordEnvelope>`，前端以「本页记录数 === PAGE_SIZE」启发式推断 `hasMore`（`use-db-inspector.ts`）——模型 total 恰为 PAGE_SIZE 整数倍时「下一页」多翻一页零记录的幽灵空页；页码 / 总条数无从呈现，页界不是数据接口的一等契约。
3. **单条详情内嵌列表**：`RecordDetail` 以内嵌 `<pre>` 块贴在列表行下方，挤占列表布局、长 JSON 撑开页面，且换模型 / 换页需同步收起逻辑（`openIndex` 复位）；无抽屉式详情面。

## 提案

对 DB 查看页做三项优化（记录信封 API 与零模型特定代码、只读、无轮询等既有约束全部保持）：

1. **模型驱动自动表格（shadcn）**：选中模型后记录区自动生成表格，使用 vendored shadcn `ui/table` 组件族（`Table` / `TableHeader` / `TableBody` / `TableRow` / `TableHead` / `TableCell`）。表格列数据驱动自动推导：`key` 列恒首列（主键 JSON 文本），其余列取当前页记录 `value` 顶层对象键的并集（首见稳定序），缺失键空单元格；单元格为单行 JSON 摘要（沿用 120 字符截断口径）；`value` 顶层非对象退化为单 `value` 摘要列。MUST NOT 为任何模型手写表格组件或列定义，新模型零改动自动出表。
2. **数据接口带分页**：记录信封 API `scan` 与命令 `db_records` 升级为返回分页信封 `RecordsPage { records, total }`（`records` = offset / limit 区间记录，`total` = 该模型全量计数，同一读事务快照）。前端页界（`hasMore` / 页码 / 总页数）全部由 `total` 导出，废除页长启发式；分页控件呈现真实页界，末页与幽灵空页边界由 `total` 锁定。
3. **抽屉查看详情**：记录区挂载单实例受控 `StandardDrawer`（无 trigger、右侧滑出、footer 恒「关闭」，沿 desktop-standard-components 既有契约），点击表格行选中该行并开抽屉，呈现 key + value 信封 pretty JSON；行内嵌详情块退役；换模型 / 换页 / 切 scope / 刷新即收起抽屉。

**假设声明**：explore 项 1「根据 model 模型自动生成表格」解读为「选中模型后由该模型记录的字段自动推导表格列（数据驱动列推导）」，而非为每个模型维护 schema 字段清单（字段元数据方案见 D1 被拒备选；若产品意图是跨页稳定列序的 schema 列，由后续变更以 `ModelInfo` 字段元数据 additive 扩展，本变更的表格组件与页界契约不动——见 R6）。

## 关键决策

| ID | 决策 | 理由 | 被拒绝的备选方案 |
|----|------|------|------------------|
| D1 | **表格列数据驱动自动推导**（当前页 `value` 顶层键并集、首见稳定序、`key` 列恒首列） | 信封层只有 JSON Value、无运行时字段反射；列 = 数据实际键，永不与 struct 漂移；「新模型零改动出表」（查看器 git diff 为空）与既有「零模型特定代码」约束天然成立 | **备选：注册表为每模型登记字段清单（schema 元数据）**。被拒绝：每模型维护字段清单，struct 演进双处改、清单漂移即列错，破坏「登记一行接入」最小机制。**备选：以首条记录的键定列**。被拒绝：首条缺字段即整表漏列，稀疏字段漏列面更大 |
| D2 | **分页元数据随接口返回 `RecordsPage { records, total }`**（`scan` 同读事务产出 total，命令层薄透传） | 页 + total 同一读事务快照，并发写下二者自洽；total 是页界唯一事实源（hasMore / 页码 / 总页数皆可导出），「数据接口带分页」落实为接口一等契约 | **备选：前端拿 `ModelInfo.count` 当 total**。被拒绝：count 是模型清单快照、与扫描不同事务，workspace 库被 run / 事件并发写入时与本页记录不自洽。**备选：DTO 直接带 `hasMore` 布尔**。被拒绝：total 已可导出一切页界，布尔只是冗余镜像且丢失总条数呈现面 |
| D3 | **单实例受控 `StandardDrawer`（无 trigger）承载详情** | 沿 desktop-standard-components 既有契约（右侧滑出、footer 恒「关闭」、`useOpen` 归一），单实例避免 N 行 N 弹层；受控 `open` / `onOpenChange` 与「列表换血即收起」迁移自既有 `openIndex` 语义 | **备选：每行一个带 trigger 的 drawer**。被拒绝：N 行 N 个弹层实例，开合态散落。**备选：直接用 `ui/drawer`**。被拒绝：绕过 standard 层契约与 `useOpen` 归一。**备选：保留内嵌详情块**。被拒绝：explore 明确要求抽屉，内嵌块挤占列表布局 |
| D4 | **能力归属：delta 落 `desktop-db-inspector`（主）+ `desktop-workspace-store`（信封 API 签名）** | 三项优化全部作用于同一 DB 查看页能力面；`scan` 返回形状变更属「记录信封 API」requirement 的签名演进，须同账 | **备选：新开能力 `desktop-manage-db-by-table`**。被拒绝：同一查看页能力边界错位、契约分家。**备选：只动命令层保 `scan` 签名**。被拒绝：分页元数据属数据接口职责，命令层拼装 `scan` + `list_models` 双读事务不自洽（同 D2 备选缺陷） |

## 能力

### 新增能力

- 无（三项优化均为既有能力 `desktop-db-inspector` 的关注点演进，以 ADDED requirement 归入）。

### 修改的能力

- **desktop-db-inspector** — MODIFIED「只读查看面」（分页扫描升级为自动表格呈现 + 分页元数据随接口返回；单条查看从内嵌 JSON 块改为抽屉呈现）；ADDED「模型驱动自动表格」（shadcn `ui/table`、数据驱动列推导、零模型特定代码）、「分页接口与真实页界」（`RecordsPage{ records, total }`、页界由 total 导出、废除页长启发式）、「抽屉单条详情」（单实例受控 `StandardDrawer`、行内无内嵌详情）。
- **desktop-workspace-store** — MODIFIED「记录信封 API」：`scan(model, offset, limit)` 返回值从 `Vec<RecordEnvelope>` 升级为 `RecordsPage{ records, total }`（records + total 同一读事务快照）；`list_models`、JSON Value 信封、native_db 类型壁垒、只读边界、新模型零改动覆盖全部不变。

### 引用沿用（零 delta）

- **desktop-standard-components** — `StandardDrawer` 契约照单全收（trigger 可选双态取无 trigger 受控式、footer 恒「关闭」、`useOpen` 归一、右侧滑出）。
- **desktop-app-shell** — 测试挂钩纪律（role / 可及名 / 文本 / data-* 查询，无样式类名查询）与 shadcn vendored 底座纪律沿用（`ui/table` 为 `components/ui/**` vendored 组件，测试经真实组件渲染、MUST NOT mock）。

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` — 新增 `RecordsPage{ records, total }` DTO；`scan` 返回分页信封（`scan_model` 同读事务取 total 与区间记录）
- `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` — re-export `RecordsPage`
- `packages/desktop/src-tauri/src/commands/db/mod.rs` — `db_records` 返回 `Result<RecordsPage, String>`（薄透传，scope 寻址 / blank root / 未知模型 reject 语义不变）
- `packages/desktop/src/types/generated/bindings.ts` — specta 再生成（`pnpm -C packages/desktop run bindings:export`）：`RecordsPage` 类型 + `dbRecords` 返回类型
- `packages/desktop/src/views/db/hooks/use-db-inspector.ts` — 分页态以接口 `total` 为准（`hasMore` / 页码 / 总页数由 total 导出），删除 `records.length === PAGE_SIZE` 启发式
- `packages/desktop/src/views/db/db-inspector-view.tsx` — 记录区改 shadcn 自动表格（数据驱动列推导、`key` 列恒首列、单元格 120 字摘要）+ 真实页界分页控件 + 单实例受控 `StandardDrawer` 单条详情（替换 `RecordRow` / `RecordDetail` 内嵌形态）
- `packages/desktop/package.json` — `version` 0.5.1 → 0.5.2（归档时执行）

### 测试文件

- `packages/desktop/src/views/db/db-inspector-view.test.tsx` — 表头自动推导（并集补列 / 首见稳定序 / 缺失空单元格）、单元格摘要截断与非对象退化、抽屉开合与「关闭」、列表换血收起、行内无内嵌详情、真实页界控件、空态 / inline 持久错误态 / 只读边界 / scope 双 tab 不回归
- `packages/desktop/src/views/db/hooks/use-db-inspector.test.ts` — `dbRecords` 收 `RecordsPage` 分页信封；页界由 `total` 导出（整数倍无幽灵空页、末页 next 禁用）；显式动作触发 / 无轮询不变
- `packages/desktop/src/app.test.tsx` — `db_records` mock fixture 形状随 DTO 更新（返回 `{ records, total }`），路由化 db 分支渲染断言不回归
- `packages/desktop/src-tauri/src/commands/db/mod_test.rs` — `db_records` 返回分页信封（total 全量计数、scope 寻址 / blank root / 未知模型 reject 语义不变）
- `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs` — `scan` 分页信封（records 区间 + total 同口径、翻页拼接不重不漏、MAX_SCAN_LIMIT 截断、新模型零改动覆盖）

### 不要修改

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` 与 `MODEL_ENTRIES` 注册机制（表格列 MUST NOT 引入模型字段元数据；「登记一行接入」不变）
- `packages/desktop/src/components/ui/**`（shadcn vendored 底座，含 `table.tsx` / `drawer.tsx`）与 `packages/desktop/src/components/standard/**`（`StandardDrawer` / `useOpen` 契约零改动）
- `packages/desktop/src/routes.tsx` 与侧栏入口（页面身份、壳态可达性、scope 切换取数边界零变化）
- db 轨道只读边界：MUST NOT 新增任何写命令（写操作仍待真实 debug 需求出现后由后续变更裁定）
- `openspec/specs/` 本体（能力基线只经本 change 的 `specs/` delta 演进，归档时合并）
- `plugins/dev-team` 全部（保持 2.10.44 零改动）

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 模型驱动自动表格 | 选中模型后记录区呈 shadcn Table（`ui/table` 组件族）；表头 = `key` 列 + 当前页 `value` 顶层键并集（首见稳定序），后出现的键补尾列、缺失键空单元格；`key` 列恒首列；查看器 / 表格生成代码零模型特定分支（新模型出表 git diff 为空） |
| AC-2 | 单元格摘要与退化 | 单元格为单行 JSON 摘要：短于 120 原样、恰 120 不截、121 截断加省略号；`value` 顶层非对象退化为单 `value` 摘要列 |
| AC-3 | 分页接口（后端） | `Store::scan` 与 `db_records` 返回 `RecordsPage{ records, total }`：`total` 为该模型全量计数（非本页条数）且与 `records` 同一读事务；scope 寻址 / blank root 空结果 / 未知模型 reject / MAX_SCAN_LIMIT 截断语义不变；bindings 再生成同步（`bindings:check` 干净） |
| AC-4 | 真实页界 | 页界（hasMore / 页码 / 总页数）全部由接口 `total` 导出，`use-db-inspector` 源码无 `records.length === PAGE_SIZE` 启发式；total 恰为 PAGE_SIZE 整数倍时末页下一页禁用、无幽灵空页；分页控件呈现真实页界 |
| AC-5 | 抽屉单条详情 | 点击表格行 → 行选中态 + `StandardDrawer` 右侧滑出（title 为 key JSON 文本、content 为 key + value 信封 pretty JSON、footer 恒「关闭」）；「关闭」或开合闭合后抽屉退场 |
| AC-6 | 列表换血收起与行内退役 | 换模型 / 翻页 / 切 scope / 刷新时抽屉收起、详情不残留；记录表格行内无 JSON 详情块（详情仅抽屉内） |
| AC-7 | 只读与空错态不回归 | 无任何写操作入口；空态（计数 0）、inline 持久错误（清单 / 记录 reject、workspace 库打开失败）不白屏、无 toast 顶替；取数全部用户显式动作触发、无轮询 |
| AC-8 | 测试与管线 | 五个测试文件按「测试文件」清单更新（含 fixture 形状）；查询挂钩无样式类名查询（类名仅作被检属性）、不 mock 被测组件与 `ui/**`；`pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test` 全绿，desktop 套件覆盖率达阈值（lines 90 / branches 85 / functions 90）、mutation score 达 80；`openspec/config.json` 无新增 excludes / 豁免 |
| AC-9 | 版本交付（归档时执行） | `packages/desktop/package.json` `version` 0.5.1 → 0.5.2；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 稀疏字段跨页列漂移 | 各页顶层键并集不同则表头列集合 / 列序随页变化，跨页对照时列位不稳 | 中 | 当前页并集 + 首见稳定序 + 缺失键空单元格 + `key` 列恒首列（AC-1 锁定口径）；若需要跨页稳定列序，后续变更为 `ModelInfo` 增字段元数据（信封 API additive 扩展），本变更的表格组件与页界契约不动 |
| R2 并发写下 total 与翻页漂移 | workspace 库被 run / 会话事件并发写入，翻页期间 total 与行集在页间可能漂移 | 低 | 页 + total 同一读事务快照（单页内自洽）；只读调试面接受快照级一致，「翻页拼接不重不漏」维持既有偏移语义口径，不引入游标分页（无真实需求不做） |
| R3 长 JSON / 大 value 渲染开销 | 单元格或抽屉渲染大 value（如长转录字段）拖慢表格帧率 | 低 | 单元格沿 120 字符摘要截断（仅单行文本）；完整 JSON 只在抽屉内渲染一次（单实例、打开时才渲染） |
| R4 bindings 再生成与 fixture 形状波及 | `dbRecords` 返回类型变更牵动 `app.test.tsx` 等既有 mock fixture，漏改即编译 / 测试红 | 低 | fixture 随 DTO 形状显式更新（AC-3 / AC-8）；`bindings:check` 保证 generated 与源同步，无手写类型旁路 |
| R5 旧启发式断言重写失真 | `use-db-inspector.test.ts` / `db-inspector-view.test.tsx` 现有「整页 hasMore 驱动翻页控件」断言按旧口径重写时可能弱化边界 | 低 | AC-4 显式断言整数倍边界（100 条 / PAGE_SIZE 50 末页 next 禁用、无空页）与「源码无页长启发式」，幽灵空页回归有锚 |
| R6 explore 解读口径分歧 | 「根据 model 模型自动生成表格」若产品意图是 schema 字段清单驱动的稳定列，数据驱动列推导与意图有偏差 | 中 | 假设已在「提案」节显式声明（D1 含被拒备选与理由）；若确认为 schema 列需求，后续变更以 `ModelInfo` 字段元数据 additive 扩展列来源，表格组件 / 抽屉 / 页界契约（AC-1 ~ AC-7）不受影响 |
