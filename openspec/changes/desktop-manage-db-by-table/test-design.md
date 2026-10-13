# 测试设计: desktop-manage-db-by-table

> **日期**: 2026-10-13

---

## 验收范围

<!-- 逐条映射 proposal.md 验收标准；`被测文件或模块` = design「变更清单」修改文件表的源文件
    （无源文件承载的机械步骤记 `—`）。跨模块组合用例挂靠链路入口模块的 per-file
     章节（无独立集成测试层）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 模型驱动自动表格：选中模型后记录区呈 shadcn Table（`ui/table` 组件族）；表头 = `key` 列 + 当前页 `value` 顶层键并集（首见稳定序）、后见键补尾列、缺失键空单元格、`key` 列恒首列；表格生成代码零模型特定分支（新模型出表 git diff 为空） | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-2 | 单元格摘要与退化：单行 JSON 摘要短于 120 原样、恰 120 不截、121 截断加省略号；`value` 顶层非对象退化为单 `value` 摘要列 | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-3 | 分页接口（后端）：`Store::scan` 与 `db_records` 返回 `RecordsPage{ records, total }`（total = 该模型全量计数且与 records 同一读事务）；scope 寻址 / blank root 空结果 / 未知模型 reject / MAX_SCAN_LIMIT 截断语义不变（信封实现半边：`scan_model` 同读事务组装，DD1） | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` |
| AC-3 | （同上——公共签名升级半边：`Store::scan` 返回 `Result<RecordsPage, StoreError>`，薄转调 `envelope::scan`） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` |
| AC-3 | （同上——命令面半边：`db_records` 返回 `Result<RecordsPage, String>`，blank root 空信封 `RecordsPage { records: [], total: 0 }`（DD2），其余 reject 语义不变） | `packages/desktop/src-tauri/src/commands/db/mod.rs` |
| AC-3 | （同上——bindings 生成物半边：`RecordsPage` 类型 + `dbRecords` 返回类型出线，`bindings:check` 干净） | `packages/desktop/src/types/generated/bindings.ts` |
| AC-3 | （同上——`DTO_TYPES` 覆盖清单随动半边：插入 `"RecordsPage"`，DD6） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` |
| AC-3 | （同上——公共导出半边：`lib.rs` re-export `RecordsPage`；见不可测试项 1） | `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` |
| AC-4 | 真实页界（导出半边）：`hasMore` / 页码 / 总页数全部由接口 `total` 导出，废除 `records.length === PAGE_SIZE` 启发式；total 恰为 PAGE_SIZE 整数倍时末页 next 禁用、无幽灵空页 | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` |
| AC-4 | （同上——呈现半边：分页控件呈现「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页） | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-5 | 抽屉单条详情：点表格行 → 行选中态 + `StandardDrawer` 右侧滑出（title = key JSON 文本、content = key + value 信封 pretty JSON、footer 恒「关闭」）；「关闭」或开合闭合后抽屉退场 | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-6 | 列表换血收起与行内退役：换模型 / 翻页 / 切 scope / 刷新时抽屉收起、详情不残留；记录表格行内无 JSON 详情块（详情仅抽屉内） | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-7 | 只读与空错态不回归（视图半边）：无任何写操作入口；空态（计数 0）、inline 持久错误（清单 / 记录 reject、workspace 库打开失败）不白屏、无 toast 顶替 | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-7 | （同上——取数纪律半边：全部取数用户显式动作触发、无轮询、错误 inline 持久） | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` |
| AC-8 | 测试与管线（fixture 形状半边）：`db_records` mock fixture 形状随 DTO 更新为 `{ records, total }`，路由化 db 分支渲染断言不回归 | `packages/desktop/src/app.tsx`（组合入口；路由壳零改动） |
| AC-8 | （同上——查询挂钩纪律半边：查询无样式类名查询（类名仅作被检属性）、不 mock 被测组件与 `ui/**`） | `packages/desktop/src/views/db/db-inspector-view.tsx` |
| AC-8 | （同上——管线半边：`client:check` 与 `test` 全绿、覆盖率达阈值、mutation score 达 80、`openspec/config.json` 无新增 excludes / 豁免；见不可测试项 4） | —（机械步骤） |
| AC-9 | 版本交付（归档时执行）：`packages/desktop/package.json` `version` 0.5.1 → 0.5.2；`tauri.conf.json` 维持 `../package.json` 引用、`src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 零改动（见不可测试项 2） | `packages/desktop/package.json` |

---

## 单元测试

<!-- 测试框架与布局：Rust 侧 `cargo test`（workspace 级注册于 `src-tauri` 根，共置 `*_test.rs`
     模块，`assert_eq!` / `expect_err` 断言）；TS 侧 `vite-plus`（vp test，共置 `*.test.ts(x)`，
     `describe` / `it` + `expect` 断言，`@testing-library/react` 渲染，`vi.mock` 替身进程边界）。
     跨模块组合用例挂靠链路发起方 / 最上层调用方的 per-file 章节（`#### 用例` 表内，describe
     标题写链路方向），不设独立集成测试区。
     映射说明：proposal「测试文件」五项之外，`store_test.rs` 既有 `Store::scan` 消费面
    （`.iter()` / `.len()` / `.is_empty()`）随返回类型升级需类型级最小适配（design「验收标准对齐」
     AC-8「测试文件类型级最小适配」口径），故并列 `store.rs -> store_test.rs` 章节；该章节以
     既有用例适配为主，不另立行为矩阵（行为矩阵落 `envelope_test.rs`，proposal 测试文件清单指定）。
     参数边界按系统化映射推导：str（空串 / 超长 / 特殊字符 / nullish）、int（0 / MAX / 负值不适用）、
     bool、list（空 / 单元素 / 超大 / nullish）、object（空 / 缺键 / 多余键 / nullish）、Enum（各值 +
     非法值）；不适用形态在行内显式标注。 -->

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 挂 AC-3（公共签名升级半边）。既有 Env tempfile 装置沿用；本章节为 `Store::scan`
     公共面章节，行为矩阵（records 区间 + total 同口径）挂 envelope_test.rs 章节。 -->

#### 待测功能

- Store::scan(model: &str, offset: u32, limit: u32) -> Result<RecordsPage, StoreError>: 入参不变；返回值从 `Vec<RecordEnvelope>` 升级为分页信封（records = offset / limit 区间记录、total = 该模型全量计数，同一读事务快照）；未知模型 Err / MAX_SCAN_LIMIT 截断语义不变（AC-3）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `Store::scan` 分页信封返回面 | 正向 | 正常参数（model 已知模型名、offset 10 / limit 10、库存 30 条）→ Ok(`RecordsPage`)：records 恰区间 10 条且信封逐字段与改造前一致，total = 30（全量计数非本页条数）——公共面返回形状锚 | 新增 |
| `Store::scan` 分页信封返回面 | 异常 | model 参数 str 边界：空串 `""` / 超长串（>1000 字符）/ 特殊字符（`\n` `\0` emoji）→ 未知模型 `Err(StoreError)` 不 panic、错误串含模型名；nullish 不适用（Rust `&str` 非 Option，类型面排除） | 适配 |
| `Store::scan` 分页信封返回面 | 边界 | offset / limit 参数 int 边界：offset 0 / 恰等总数 / 超总数 / `u32::MAX`（负值不适用——u32 无符号，以 MAX 代），limit 0 / 大于剩余 / 超 `MAX_SCAN_LIMIT` 截断为 500——records 区间正确、total 恒全量计数 | 适配 |
| `Store::scan` 既有消费面类型级适配 | 正向 | 既有「信封 API 分页正向往返 / `scan分页按offset_limit返回主键自然序翻页拼接不重不漏`」用例消费面改 `.records` 后断言零变化（薄转调不加工） | 适配 |
| `Store::scan` 既有消费面类型级适配 | 异常 | 既有跨维度模型名 Err 用例（workspace 库 `scan("user" 维度模型)` / 全局库 `scan("workspace" 维度模型)`）`expect_err` 面零变化 | 适配 |
| `Store::scan` 既有消费面类型级适配 | 边界 | 既有信封读取类用例（`scan("checklist_item" / "change" / "phase", …).unwrap().len()` 等）消费面适配为 `.records.len()` 后断言零变化（返回面增 total 不影响既有信封面） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile TempDir 真实开库（存储层不 mock 既有口径），Env 装置沿用 | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- 挂 AC-3（信封实现半边，DD1）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`envelope::scan` 为 pub(crate) 模块内分发、
     `scan_model` / `count_model` / per-model 包装为模块私有——设计「公共函数 / API」表显式不列）；
     不虚构条目。被测面 = 修改文件表「`scan_model` 同读事务内取 total（`r.len().primary::<T>()`）
     与区间记录组装 `RecordsPage`」的行为半边，按 proposal 测试文件清单落本测试文件
    （既有「分页边界矩阵 / 翻页拼接不重不漏 / MAX_SCAN_LIMIT 截断 / 新模型零改动」断言面即布局于此），
     经 `Store::scan` 公共面真实组合驱动。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `scan_model` 同读事务 records 区间 + total 同口径（分页边界矩阵重写） | 正向 | 库存 30 条逐页 `scan(offset, limit)`：records = 区间信封、total 恒 30（全量计数非本页条数）；既有分页边界矩阵行（offset 0 / 恰等总数 / 超总数、limit 0 / 大于剩余 / 超 500 截断）全部重写为 `RecordsPage` 两字段断言 | 重写 |
| `scan_model` 同读事务 records 区间 + total 同口径（分页边界矩阵重写） | 异常 | 未知模型 / 空串模型名 → `Err` 语义不变（既有 `scan_err` 装置适配）；limit 超 `MAX_SCAN_LIMIT` 截断后 total 仍全量计数（截断只作用 records 区间，total 不随截断缩水） | 适配 |
| `scan_model` 同读事务 records 区间 + total 同口径（分页边界矩阵重写） | 边界 | 库存形态边界（list 参数映射）：空库 → `{ records: [], total: 0 }`；单条库 → `{ records: [1 条], total: 1 }`；offset 超界 → records 空而 total 恒全量（快照级自洽口径，R2） | 新增 |
| `scan` 翻页拼接与幽灵空页边界（total 口径） | 正向 | 逐页拼接覆盖全量记录不重不漏（既有两新模型翻页用例消费面改 `.records`，并补各页 total 一致断言） | 适配 |
| `scan` 翻页拼接与幽灵空页边界（total 口径） | 异常 | total 恰为 limit 整数倍（100 条 / limit 50）：末页 records 50 条、`offset + records.len() == total`，下一页 records 空而 total 不变——「幽灵空页」在数据接口侧的边界锚（前端由此导出 next 禁用） | 新增 |
| `scan` 翻页拼接与幽灵空页边界（total 口径） | 边界 | limit = 0 连续取页不推进不炸（records 恒空、total 恒全量）；offset `u32::MAX` 空页；单条 / 空库最小库存翻页 | 适配 |
| `RecordsPage` DTO 与新模型零改动覆盖 | 正向 | `RecordsPage` serde camelCase 信封序列化形态恰 `{ records, total }`（与 `ModelInfo` / `RecordEnvelope` 同口径），`Debug` / `Clone` / `PartialEq` derive 面；`ScanFn` 返回类型随动后 11 个 per-model 包装零新分支（`count_model` 保留供 `list_models` 计数口径零改动） | 新增 |
| `RecordsPage` DTO 与新模型零改动覆盖 | 异常 | total 为 `u64` 大值（超 u32 范围）序列化为合法 JSON number 不溢出不乱码（信封可读口径延续） | 新增 |
| `RecordsPage` DTO 与新模型零改动覆盖 | 边界 | 新模型零改动覆盖：`agent_instance` / `agent_provider` 信封 key / value 口径与 total 计数断言照常（「登记一行接入」不因分页信封漂移）；`MODEL_ENTRIES` 注册表零改动面持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile 真开库经 `Store` 公共读写面种子数据（存储层不 mock 既有口径） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/db/mod.rs -> packages/desktop/src-tauri/src/commands/db/mod_test.rs

<!-- 挂 AC-3（命令面半边，DD2）。跨模块组合用例挂靠本章节（链路发起方 = `db_records`，
     最上层调用方）：`db_records` → `Store::scan` → `envelope::scan` / `scan_model` 全链
     经 tauri `State` 真实组合 store crate，describe 标题写链路方向，不设独立组合测试区。 -->

#### 待测功能

- db_records(stores: State<'_, WorkspaceStores>, scope: DbDimension, root: String, model: String, offset: u32, limit: u32) -> Result<RecordsPage, String>: 入参不变；返回 `Result<RecordsPage, String>` 薄透传 `Store::scan`；blank root → `RecordsPage { records: Vec::new(), total: 0 }` 空信封（DD2）；scope 寻址两库 / 未知模型 reject / workspace 库打开失败 reject 语义不变（AC-3）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `db_records` 分页信封薄透传（链路：命令面 → `Store::scan` → `envelope::scan`） | 正向 | 组合链：workspace 库种 30 条 → `db_records(scope, root, model, offset, limit)` 返回 `{ records: 区间, total: 30 }`，与直连 `store.scan` 结果 serde 一致（薄透传不加工；records 区间 + total 全量同快照经命令面贯通——AC-3 全链锚） | 新增 |
| `db_records` 分页信封薄透传（链路：命令面 → `Store::scan` → `envelope::scan`） | 异常 | model 参数 str 边界（空串 / 超长 / 特殊字符含 `\n` `\0` emoji）→ `Err(String)` reject 不静默吞错、错误串与 store 层一致；workspace 库打开失败（坏文件）同样 reject（既有语义零改动） | 适配 |
| `db_records` 分页信封薄透传（链路：命令面 → `Store::scan` → `envelope::scan`） | 边界 | offset / limit int 边界过命令面不加工：offset 0 / 恰等总数 / 超总数 / `u32::MAX`（负值不适用——u32），limit 0 / 超 `MAX_SCAN_LIMIT` 截断透传；records 空页时 total 恒全量 | 适配 |
| `db_records` scope 寻址与 blank root 空信封（DD2） | 正向 | DbDimension 枚举各值：`Workspace` 按 root 寻址 workspace 库、`User` 走全局库忽略 root，两库记录互不混列（scope 寻址语义零改动） | 适配 |
| `db_records` scope 寻址与 blank root 空信封（DD2） | 异常 | 非法枚举值（IPC 序列化面未知变体）→ 反序列化 reject 不 panic（命令层无自定义兜底）；未知模型 / 跨维度模型名经命令面 `Err(String)`（既有 reject 面适配为 `RecordsPage` 返回类型后零语义变化） | 适配 |
| `db_records` scope 寻址与 blank root 空信封（DD2） | 边界 | root 参数 str 边界：空串 `""` 与纯空白（`is_blank_root` 口径）→ 空信封 `{ records: [], total: 0 }` 且不触发库解析（DD2 主锚）；超长 root / 含 `\n` `\0` emoji 的非法路径 root → 按不存在库处理不 panic（不误判 blank） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage 真实 `WorkspaceStores`（tempdir 真开全局库）后经 `app.state` 取 State；沿既有 `mod_test.rs` 装置模式 | 本节全部用例 |
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile 真实全局库 / workspace 库读写；坏库文件以真实非法字节写盘 | blank root、坏文件 reject 用例 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 挂 AC-3（生成物半边）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（bindings.ts 为 specta 再生成物、非手写，禁手写旁路）；
     不虚构条目。被测面 = 修改文件表「`RecordsPage` 类型 + `dbRecords` 返回类型出线」与
     DD6 的 `DTO_TYPES` 清单随动，经 cfg(test) 产物文本断言承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 产物 `RecordsPage` 出线与 `DTO_TYPES` 清单随动（DD6） | 正向 | `bindings:export` 再生后产物含 `export type RecordsPage = { records: RecordEnvelope[], total: number }`、`dbRecords` 返回 `__TAURI_INVOKE<RecordsPage>`；`DTO_TYPES` 插入 `"RecordsPage"`（`"RecordEnvelope"` 之后，字母序）后清单与产物 Types 段全集一致；`dto.ts` 的 `export type *` re-export 自动跟随 | 新增 |
| bindings 产物 `RecordsPage` 出线与 `DTO_TYPES` 清单随动（DD6） | 异常 | 清单与产物强耦合门禁回归：「产物缺出线类型」panic 断言在 `"RecordsPage"` 在场后不触发，清单无幽灵 / 遗漏条目（出线类型增删不随动必挂的反向证明恢复绿） | 新增 |
| bindings 产物 `RecordsPage` 出线与 `DTO_TYPES` 清单随动（DD6） | 边界 | 重导幂等既有守线用例族全绿：连续两次导出逐字节一致、篡改产物重导恢复、产物无 `Result` 包装（`dbRecords` 返回类型变更不破生成面其余契约）；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件 | 进程边界（文件系统，真实组合不 mock）：`export_bindings()` 真实重导 + 生成物文本真实读取（既有装置沿用） | 本节全部用例 |

### packages/desktop/src/views/db/hooks/use-db-inspector.ts -> packages/desktop/src/views/db/hooks/use-db-inspector.test.ts

<!-- 挂 AC-4（页界导出半边，DD5）、AC-7（取数纪律半边）。 -->

#### 待测功能

- useDbInspector(root: string): DbInspectorState: 入参不变；返回面增 `total: number`（接口信封 total 快照）/ `page: number`（`offset / PAGE_SIZE + 1`，1 起）/ `totalPages: number`（`max(1, ceil(total / PAGE_SIZE))`），`hasMore` 改 `offset + records.length < total`（废除 `records.length === PAGE_SIZE` 启发式——AC-4 / DD5）；`PAGE_SIZE` 保持模块私有；scope / root 复位、显式动作触发、无轮询、查询轨 inline 持久错误语义零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `useDbInspector` 分页态 total 导出（AC-4 / DD5） | 正向 | `dbRecords` 收 `{ records, total }` 分页信封（60 条库存首页 50 条 → total 60）：`total` = 信封 total（全量计数非本页条数）、`page` = `offset / 50 + 1` = 1、`totalPages` = `max(1, ceil(60 / 50))` = 2、`hasMore` = `offset + records.length < total` = true（废除「本页记录数 === 50」启发式） | 新增 |
| `useDbInspector` 分页态 total 导出（AC-4 / DD5） | 异常 | `db_records` reject → `recordsError` 置位 inline 持久，`records` / 页界快照不被半残数据顶替（不呈伪 total）、`hasMore` 不误开；错误串 `String(err)` 落态（查询轨语义零改动） | 适配 |
| `useDbInspector` 分页态 total 导出（AC-4 / DD5） | 边界 | total 整数倍边界（bool / int 边界锚）：total = 100（恰 2 整页）首页 hasMore=true、末页（offset 50）`50 + 50 == 100` → hasMore=false、nextPage 不再推进出幽灵空页（AC-4 主锚）；total = 0 → `page` 1 / `totalPages` 1（`max(1, …)` 不除零不呈 0 页）/ hasMore=false；total = 1 单页；total 为超大值（`Number.MAX_SAFE_INTEGER` 量级）导出不溢出不 NaN | 重写 |
| `useDbInspector` 取数纪律与复位（不回归面） | 正向 | 显式动作触发：挂载恰一次 `db_models`；`selectModel` / `nextPage` / `prevPage` / `refresh` 各恰一次 `db_records`，入参 `{ scope, root, model, offset, limit: 50 }` 透传不变（int 参数步进 50） | 适配 |
| `useDbInspector` 取数纪律与复位（不回归面） | 异常 | `db_models` reject → `error` 置位、`models` 空数组；`db_records` reject → `recordsError` 置位、`selected` 与清单不丢；翻页竞态旧响应迟到不顶替新页、旧页迟到失败不置 `recordsError`（取消守卫既有用例全绿） | 适配 |
| `useDbInspector` 取数纪律与复位（不回归面） | 边界 | 静置调用数不增长（无轮询、无恢复循环——AC-7）；offset 0 时 `prevPage` 不动不发负 offset；root 参数 str 边界（空串 blank / 超长 >1000 字符 / 特殊字符 `\n` `\0` emoji / 正常值；nullish 不适用——TS `root: string` 类型面排除）变更即复位选中与分页并以新 root 重取；scope 枚举（`DbDimension` 各值）切换同口径复位 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（IPC 进程边界） | `vi.mock` invoke 替身按命令名分发固定 fixture（含 `{ records, total }` 信封形状）并记录调用序列；沿既有 `use-db-inspector.test.ts` 替身模式 | 本节全部用例 |
| （无） | `useDbInspector` 真实 hook 组合（`renderHook`）：内部 `useDbModels` / `useDbRecords` 不 mock；sonner 不 mock（hook 不引 sonner，错误 inline 由视图承载） | 本节全部用例 |

### packages/desktop/src/views/db/db-inspector-view.tsx -> packages/desktop/src/views/db/db-inspector-view.test.tsx

<!-- 挂 AC-1 / AC-2 / AC-4（呈现半边）/ AC-5 / AC-6 / AC-7（视图半边）/ AC-8（查询挂钩纪律半边）。
     跨模块组合用例挂靠本章节（链路发起方 = `DbInspectorView`，最上层调用方）：
     `DbInspectorView` → `useDbInspector` → invoke 取数 + `StandardDrawer` 开合全链真实组合。 -->

#### 待测功能

- DbInspectorView({ root }: DbInspectorViewProps): React.JSX.Element: props 签名不变；记录区改 shadcn 自动表格（数据驱动列推导：`key` 恒首列 + 当前页 `value` 顶层键并集首见稳定序、缺失键空单元格、非对象行 `value` 尾列退化）+ 单元格 120 字摘要 + 真实页界分页控件 + 单实例受控 `StandardDrawer` 单条详情（AC-1 / AC-2 / AC-4 / AC-5 / AC-6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `DbInspectorView` 模型驱动自动表格（AC-1） | 正向 | 选中模型后记录区呈 shadcn Table（`ui/table` 组件族真实渲染：table / thead / tbody / tr / th / td 结构在场，`db-records-table` 容器）；表头 = `key` 列 + 当前页 `value` 顶层键并集（首见稳定序：后见键补尾列），`key` 列恒首列；各行单元格与键位对齐、缺失键空单元格；零模型特定分支（新模型 fixture 出表无需改组件） | 新增 |
| `DbInspectorView` 模型驱动自动表格（AC-1） | 异常 | `value` 顶层非对象行（null / 数组 / 标量）→ 退化 `value` 尾列承载摘要；全非对象页恰 `key` + `value` 两列（单 `value` 摘要列退化）；混合页并集列 + `value` 尾列并存不串位 | 新增 |
| `DbInspectorView` 模型驱动自动表格（AC-1） | 边界 | 列并集 object / list 边界（系统化映射）：`value` 为 `{}` 空对象（不贡献列）、单记录单键、多记录键序首见稳定（BTreeMap 字节序 + 逐行逐键追加口径）、对象键恰为 `"value"` 与退化列同名合并（DD3 留痕）；records 空 / 单元素 / 超大页（PAGE_SIZE 50 行）渲染不炸 | 新增 |
| `DbInspectorView` 单元格摘要与退化（AC-2） | 正向 | 单元格为单行 JSON 摘要：短于 120 原样（含特殊字符 `\n` `\0` emoji 经 JSON 转义呈单行）；`key` 列单元格同口径 120 截断 | 新增 |
| `DbInspectorView` 单元格摘要与退化（AC-2） | 异常 | `value` / `key` 为 null / undefined 等 JSON 不可序列化形态 → 摘要退化为空串不抛错（既有 `valueSummary` 口径零改动）；嵌套对象 value 摘要为压缩 JSON 单行 | 新增 |
| `DbInspectorView` 单元格摘要与退化（AC-2） | 边界 | 120 字符边界（str 边界锚）：恰 120 不截、121 截断加 `…` 省略号、超长 value（>1000 字符）截断稳定；空对象摘要 `{}`；既有 120 边界用例重写保留断言口径 | 重写 |
| `DbInspectorView` 抽屉单条详情与换血收起（AC-5 / AC-6） | 正向 | 组合链（视图 → hook → IPC + 抽屉）：点表格行（`db-record-row`）→ 行选中态（`data-state="selected"`）+ `StandardDrawer` 右侧滑出：title = key JSON 文本、content = `db-record-json` pretty JSON（key + value 信封）、footer 恒「关闭」；同页点他行即换详情（单实例受控） | 新增 |
| `DbInspectorView` 抽屉单条详情与换血收起（AC-5 / AC-6） | 异常 | 点「关闭」或开合闭合 → 抽屉退场、行选中清除；刷新失败（`recordsError`）不换血 → 抽屉保留现状、详情不越出当前列表（DD 留痕口径） | 新增 |
| `DbInspectorView` 抽屉单条详情与换血收起（AC-5 / AC-6） | 边界 | 列表换血四动作（换模型 / 翻页 / 切 scope / 刷新成功）→ 抽屉收起详情不残留（DD4 effect deps 语义）；行内无 JSON 详情块（`db-record-detail` / 行内 pre 退役，详情仅抽屉内）；records 空列表无行可选、抽屉不可达 | 新增 |
| `DbInspectorView` 抽屉单条详情与换血收起（AC-5 / AC-6） | 废弃 | 既有「点开单条记录行内展开 `db-record-item` / `db-record-detail` 内嵌 JSON 块」与「记录行（`RecordRow` 按钮行）className 选中态」用例随 `RecordRow` / `RecordDetail` 退役废弃，选中态断言重写为表格行 + 抽屉形态 | 废弃 |
| `DbInspectorView` 真实页界控件、空错态与只读边界（AC-4 / AC-7） | 正向 | 分页控件呈现「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页按真实页界启用禁用：total 100 / PAGE_SIZE 50 时末页 next 禁用、无幽灵空页（AC-4 呈现锚）；`db-page-prev` / `db-page-next` 真实翻页 | 新增 |
| `DbInspectorView` 真实页界控件、空错态与只读边界（AC-4 / AC-7） | 异常 | 空态（计数 0 → `db-records-empty`）；inline 持久错误（清单 reject / 记录 reject / workspace 库打开失败 → `db-inspector-error` 等 inline 区）不白屏、无 toast 顶替（sonner spy 断言零调用）；`recordsLoading` 时翻页控件禁用 | 适配 |
| `DbInspectorView` 真实页界控件、空错态与只读边界（AC-4 / AC-7） | 边界 | 只读边界：渲染树可交互控件仅选择 / 翻页 / 刷新类，无任何写操作入口；既有「整页 hasMore 驱动翻页控件」断言重写为 total 导出页界（R5 旧启发式断言重写不失真——整数倍边界显式锚）；查询挂钩纪律：查询全经 role / 可及名 / 文本 / data-*（类名仅作被检属性、不 mock 被测组件与 `ui/**`） | 重写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（IPC 进程边界） | `vi.mock` invoke 替身按命令名分发（`db_models` / `db_records`，fixture 含 `{ records, total }` 信封）并记录调用序列；`useDbInspector` 真实组合不 mock（组件真实组装 hook） | 本节全部用例 |
| sonner（toast 边界） | spy mock 注入：断言「无任何 toast 调用」（查询轨错误 inline 持久、不走 toast 的双轨语义） | 错误态用例 |
| `ui/table` / `ui/button` / `standard/drawer`（`StandardDrawer`） | 不 mock——shadcn vendored 底座与标准组件经真实渲染（desktop-app-shell 纪律，MUST NOT mock）；抽屉以受控 `open` 状态与 title / content / footer 在场断言 | 本节全部用例 |

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

<!-- 挂 AC-8（fixture 形状半边）。app.tsx 不在本变更修改文件清单（`routes.tsx` 与壳层零改动）；
     design.md 公共函数 / API 表对该文件零声明。本章节承接 proposal 测试文件清单 `app.test.tsx`
     的 `db_records` mock fixture 形状随 DTO 更新与路由化 db 分支渲染不回归组合用例
    （跨模块组合挂靠链路入口 = App 壳层：路由 → 壳 → `DbInspectorView`）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（App 壳层 / 路由装配零改动）；不虚构条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App 壳层 db 路由分支渲染 + fixture 形状随 DTO（组合） | 正向 | 路由化 db 分支渲染断言不回归（组合链：路由 → 壳 → `DbInspectorView`）：进入 db 路由后模型清单与记录表格经 fixture `{ records, total }` 信封渲染，壳层框架断言族全绿 | 适配 |
| App 壳层 db 路由分支渲染 + fixture 形状随 DTO（组合） | 异常 | fixture 拒绝（`db_records` / `db_models` reject）→ db 分支 inline 错误态呈现不白屏（路由壳不吞错、不顶替 toast） | 适配 |
| App 壳层 db 路由分支渲染 + fixture 形状随 DTO（组合） | 边界 | `db_records` mock fixture 形状更新为 `RecordsPage { records, total }`（含空信封 `{ records: [], total: 0 }` 与整数倍 total 形态；list / object 边界同系统化映射）后既有断言族全绿——fixture 面与生成物同形、无手写类型旁路（AC-8「含 fixture 形状」） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api`（invoke / app）、`plugin-dialog`、`plugin-updater` | 进程边界 mock（沿既有 `app.test.tsx` mock 方案），`db_records` fixture 形状随 `RecordsPage` DTO 更新 | 本节全部用例 |
| sonner | 不 mock（沿既有 `app.test.tsx` 口径） | 本节全部用例 |

---

## 不可测试项

1. `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（公共导出面） — **原因**: 纯 `pub use` re-export（`RecordsPage` 随 `ModelInfo` / `RecordEnvelope` 出线），零运行时行为、无独立测试路径可解析（不建 `lib_test.rs` 防空套件）；导出正确性由消费方编译期承载（`envelope_test.rs` / `mod_test.rs` / 前端 bindings 消费面即编译锚）。
2. `packages/desktop/package.json`（version 交付） — **原因**: version 0.5.1 → 0.5.2 归档轨道（AC-9 / DD7），无运行时行为可断言；`tauri.conf.json` 引用维持、`src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 保持 2.10.44 零改动由守线静态面核对。
3. `packages/desktop/src-tauri/src/bindings/mod_test.rs`（测试文件自身） — **原因**: 本身即 cfg(test) 产物断言清单（测试文件），无独立被测映射；其 `DTO_TYPES` 随动断言面挂「bindings.ts -> bindings/mod_test.rs」章节承载（DD6 随实现阶段处理，非新用例）。
4. AC-8 管线半边（`pnpm -C packages/desktop run client:check` / `run test` 全绿、覆盖率 lines 90 / branches 85 / functions 90、mutation score 80、`openspec/config.json` 无新增 excludes / 豁免、`bindings:check` 干净） — **原因**: 静态脚本与测试执行类机械步骤由桌面端执行，agent 内不重复执行（归守线 / test-execution 轨道）。
5. `StandardDrawer` 右侧滑出动画、抽屉宽度与像素级视觉 — **原因**: jsdom 无布局引擎，滑出动效与位置像素不可断言；结构断言代理为受控 `open` 状态 + title / content / footer（恒「关闭」）在场，`useOpen` 归一与滑出契约由 desktop-standard-components 既有测试承载（引用沿用零改动）。
6. `packages/desktop/src/components/ui/**`（shadcn vendored 底座，含 `table.tsx` / `drawer.tsx`）内部实现 — **原因**: vendored 禁改面且 `openspec/config.json` excludes 现状已排除；测试经真实组件渲染覆盖行为面（不 mock），不为其建独立套件。
