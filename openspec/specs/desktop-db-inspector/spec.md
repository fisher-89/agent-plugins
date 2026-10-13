# desktop-db-inspector Specification

## Purpose

定义桌面端「DB 查看」系统工具页的契约：全局库与当前 workspace 库（scope 切换）的只读查看面（模型清单与计数、按模型分页扫描、单条 JSON 查看），经记录信封 API 取数、零模型特定代码，仅壳态可达。

## Requirements

### Requirement: 系统工具组入口与可达性

前端 SHALL 在侧栏新增「数据库」入口，位于「系统工具」组（与平移至该组的 [Agent 调试] 并列）；「页面」组保留 [变更]。`TopPage` SHALL 增 `db` 变体，视图切换维持本地 state，MUST NOT 引入路由。数据库页 SHALL 仅壳态可达：欢迎态（root 为 null）MUST NOT 挂载壳与系统工具组，无 DB 查看入口可达。切换至 DB 查看页 MUST NOT 触发 change 取数。

#### Scenario: 壳态入口与切换

- **WHEN** 壳态下用户点击「系统工具」组的「DB 查看」
- **THEN** 主内容区切至 DbInspectorView（本地 state，无路由），workspace 清单组仍在，change 取数未触发

#### Scenario: 欢迎态不可达

- **WHEN** `root` 为 null（欢迎态）
- **THEN** 页面无系统工具组 DOM，DB 查看页不可达

### Requirement: 只读查看面

DB 查看页 SHALL 提供只读查看面，包含四件套：

- **模型清单**：经 `db_models` 按 **scope**（全局库 / 当前 workspace 库）列出对应库全部已注册模型及其记录计数——全局库呈现 user 维度模型，workspace 库呈现 workspace 维度模型，两库清单互不混列
- **分页扫描（自动表格）**：选中模型后经 `db_records` 以 scope + offset / limit 分页扫描对应库的记录并自动生成表格呈现（表格生成细则见「模型驱动自动表格」）；接口 SHALL 随页返回分页元数据（细则见「分页接口与真实页界」）
- **单条查看（抽屉）**：点开单条记录经抽屉以 JSON 呈现完整内容（key 与 value 信封），记录行内 MUST NOT 内嵌详情块（抽屉细则见「抽屉单条详情」）
- **空态与错误态**：模型计数为 0 呈现空态；命令 reject 呈现 inline 持久错误（沿用「错误呈现双轨」查询轨语义）；scope 指向的 workspace 库打开失败同样以 inline 持久错误呈现，不白屏

scope 切换 SHALL 由用户显式动作触发；workspace 库 scope 的寻址基准为当前选定 workspace root（欢迎态无 root 时不可达，见「系统工具组入口与可达性」既有约束）。查看面 MUST NOT 提供任何写操作入口（新增 / 修改 / 删除记录），写操作待真实 debug 需求出现后由后续变更裁定。取数 SHALL 由用户显式动作触发（切换 scope、选中模型、翻页、刷新），MUST NOT 引入轮询或事件订阅。

#### Scenario: 双库 scope 清单与计数

- **WHEN** 在 DB 查看页分别切换全局库与 workspace 库 scope
- **THEN** 全局库 scope 仅呈现 `WorkspaceRecord` 等 user 维度模型，workspace 库 scope 仅呈现 run / 事件 / explore 等 workspace 维度模型，各自计数准确、互不混列

#### Scenario: 分页扫描与单条查看

- **WHEN** 选中某模型并翻页，随后点开一条记录
- **THEN** 当前 scope 对应库的分页区间记录以自动生成的表格呈现，翻页拼接不重不漏；单条记录经抽屉 JSON 完整可读

#### Scenario: scope 切换不串库

- **WHEN** workspace A 处于选定态，切换 scope 至 workspace 库后再切回全局库
- **THEN** 两个 scope 各自呈现所属库内容，切换过程无跨库数据串显

#### Scenario: 只读边界

- **WHEN** 审查 DbInspectorView 与 db 轨道命令
- **THEN** 无任何写操作入口，全部取数经用户显式动作触发、无轮询

#### Scenario: 查询失败 inline 持久

- **WHEN** `db_models` / `db_records` reject（含 workspace 库打开失败）
- **THEN** 查看页 inline 持久呈现错误（无 toast 顶替），不白屏

### Requirement: 模型自动注册红利与类型壁垒

查看器数据面 SHALL 仅经信封 API（`db_models` / `db_records` → store `list_models` / `scan`）；查看器代码 MUST NOT 含任何模型特定分支（per-model 硬编码清单、字段解析、类型映射），新注册模型 SHALL 无需改动查看器即出现在清单并可扫描。native_db 类型 MUST NOT 穿透信封抵达前端（信封值为 JSON Value）。

#### Scenario: 新模型零改动可浏览

- **WHEN** store 新注册一个模型后打开 DB 查看页
- **THEN** 新模型出现在清单（含计数）且可分页扫描、单条查看，查看器代码 git diff 为空

#### Scenario: 类型不越信封

- **WHEN** 审查前端 `types/dto` 与 `db_records` 返回
- **THEN** 前端仅见 JSON Value 信封 DTO，无 native_db / store 模型类型泄漏

### Requirement: 人可读呈现

记录 value 的呈现 SHALL 人可读（JSON 文本）。编码后端选型（native_model 按模型配 serde_json 后端 vs 信封 API 内部解码 bincode）由 design 定夺并以「人可读」加权；无论何种选型，MUST NOT 向用户呈现二进制内容。

#### Scenario: 记录 JSON 可读

- **WHEN** 点开任一记录的单条查看
- **THEN** 呈现结构化 JSON 文本（字段名可读），无二进制乱码

### Requirement: 模型驱动自动表格

选中模型后记录区 SHALL 自动生成表格（shadcn vendored `ui/table` 组件族），MUST NOT 为任何模型手写表格组件或列定义。表格列 SHALL 数据驱动自动推导：`key` 列恒为首列（呈现记录主键 JSON 文本），其余列取当前页记录 `value` 顶层对象键的并集并按首见稳定序排列；某行缺失的键呈现空单元格。单元格内容 SHALL 为对应字段值的单行 JSON 摘要（沿用 120 字符截断口径，超出截断加省略号）；`value` 顶层非对象时 SHALL 退化为单 `value` 摘要列。表格生成 MUST NOT 含模型特定分支（per-model 列定义 / 字段解析 / 类型映射），新注册模型 SHALL 零改动自动出表。

#### Scenario: 表头随数据自动推导

- **WHEN** 同一页内记录的顶层键不一致（如某记录多一个可选字段）
- **THEN** 表头 = `key` 列 + 各记录顶层键并集（首见稳定序），后出现的键补为尾列，缺失键的单元格留空

#### Scenario: 新模型零改动自动出表

- **WHEN** store 新注册一个模型后打开 DB 查看页并选中该模型
- **THEN** 自动生成该模型记录表格（表头随数据推导），查看器与表格生成代码 git diff 为空

#### Scenario: 单元格摘要与非对象退化

- **WHEN** 单元格值短于 / 恰等于 / 超过 120 字符，或某记录 `value` 顶层非对象
- **THEN** 短值原样、恰 120 不截、121 截断加省略号；非对象 `value` 以单 `value` 摘要列呈现

### Requirement: 分页接口与真实页界

记录数据接口 SHALL 带分页元数据：`db_records` SHALL 随页返回分页信封 `RecordsPage { records, total }`（`records` 为 offset / limit 区间记录信封，`total` 为该模型全量计数，二者同一读事务快照，由 store 记录信封 API `scan` 直接产出、命令层薄透传）。前端页界 SHALL 由 `total` 导出（`hasMore = offset + records.length < total`，页码 / 总页数由 offset 与 `ceil(total / PAGE_SIZE)` 导出），MUST NOT 以「本页记录数 === PAGE_SIZE」页长启发式推断页界。分页控件 SHALL 呈现真实页界（当前页 / 总页数或总条数）与上一页 / 下一页，末页边界由 `total` 锁定；分页语义（offset / limit 主键自然序 skip/take、单页上限 MAX_SCAN_LIMIT、翻页由用户显式动作触发）不变。

#### Scenario: 总数整数倍无幽灵空页

- **WHEN** 模型 `total` 恰为 PAGE_SIZE 整数倍（如 100 条、PAGE_SIZE 50）且已翻至最后一页
- **THEN** 下一页按钮禁用（`hasMore = false`），MUST NOT 出现多翻一页零记录的空页

#### Scenario: 页界以接口 total 为准

- **WHEN** 审查 `useDbInspector` 分页态与分页控件
- **THEN** 页界（hasMore / 页码 / 总页数）全部由接口返回的 `total` 导出，源码无 `records.length === PAGE_SIZE` 启发式；分页控件呈现真实页界

#### Scenario: 分页信封同事务快照

- **WHEN** `db_records` / `scan` 返回任一页
- **THEN** `total` 为该模型全量计数（非本页条数）且与 `records` 同一读事务产出；翻页拼接覆盖全量且不重不漏（既有口径不变）

### Requirement: 抽屉单条详情

单条记录详情 SHALL 以抽屉呈现：记录区 SHALL 挂载单实例受控 `StandardDrawer`（无 trigger、右侧滑出、footer 恒「关闭」——沿 desktop-standard-components 既有契约，开合经 `useOpen` 归一）。点击表格行 SHALL 选中该行并打开抽屉，抽屉 content SHALL 为该记录 key 与 value 信封的 pretty JSON（人可读、无二进制），title SHALL 呈现记录标识（key JSON 文本）。换模型 / 换页 / 切 scope / 刷新 SHALL 收起抽屉（记录列表换血、详情不残留）；关窗经「关闭」或开合回报闭合。记录表格行内 MUST NOT 内嵌详情块。

#### Scenario: 点行开抽屉看详情

- **WHEN** 点击表格任一行
- **THEN** 该行呈选中态且抽屉右侧滑出，呈现该记录 key + value 信封 pretty JSON（字段名可读、无二进制）；点击「关闭」或闭合开合后抽屉退场、行选中态解除

#### Scenario: 列表换血详情不残留

- **WHEN** 抽屉打开时切换模型 / 翻页 / 切 scope / 刷新
- **THEN** 抽屉收起，不残留上一列表的记录详情

#### Scenario: 行内无内嵌详情

- **WHEN** 审查记录区渲染树
- **THEN** 表格行内无 JSON 详情块（详情仅在抽屉内呈现），表格与抽屉层级分离

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::db` | db 查看命令轨道（只读） | `db_models(scope)` / `db_records(scope, model, offset, limit)`：scope 寻址全局库或当前 workspace 库，经 `WorkspaceStores` 解析对应实例调用信封 API；`Result<T, String>` 错误模板，reject 显式呈现 |
| `DbInspectorView`（前端） | DB 查看页 | scope 切换（全局库 / 当前 workspace 库）+ 模型清单 + 分页扫描 + 单条 JSON 查看；空态 / inline 持久错误态；零模型特定代码、零写入口、零轮询 |
| 记录信封 API（store） | 模型层通用读面 | `list_models()` / `scan(model, offset, limit)` 按实例生效；JSON Value 信封；native_db 类型不越信封（约束不变） |
