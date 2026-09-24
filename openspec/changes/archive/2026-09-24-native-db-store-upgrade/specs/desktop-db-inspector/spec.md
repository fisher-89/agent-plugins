# desktop-db-inspector Specification

## ADDED Requirements

### Requirement: 系统工具组入口与可达性

前端 SHALL 在侧栏新增「DB 查看」入口，位于「系统工具」组（与平移至该组的 [Agent 调试] 并列）；「页面」组保留 [变更]。`TopPage` SHALL 增 `db` 变体，视图切换维持本地 state，MUST NOT 引入路由。DB 查看页 SHALL 仅壳态可达：欢迎态（root 为 null）MUST NOT 挂载壳与系统工具组，无 DB 查看入口可达。切换至 DB 查看页 MUST NOT 触发 change 取数。

#### Scenario: 壳态入口与切换

- **WHEN** 壳态下用户点击「系统工具」组的「DB 查看」
- **THEN** 主内容区切至 DbInspectorView（本地 state，无路由），workspace 清单组仍在，change 取数未触发

#### Scenario: 欢迎态不可达

- **WHEN** `root` 为 null（欢迎态）
- **THEN** 页面无系统工具组 DOM，DB 查看页不可达

### Requirement: 只读查看面

DB 查看页 SHALL 提供只读查看面，包含四件套：

- **模型清单**：经 `db_models` 列出 user db 全部已注册模型及其记录计数
- **分页扫描**：选中模型后经 `db_records` 以 offset / limit 分页扫描记录
- **单条查看**：点开单条记录以 JSON 呈现完整内容（key 与 value 信封）
- **空态与错误态**：模型计数为 0 呈现空态；命令 reject 呈现 inline 持久错误（沿用「错误呈现双轨」查询轨语义）

查看面 MUST NOT 提供任何写操作入口（新增 / 修改 / 删除记录），写操作待真实 debug 需求出现后由后续变更裁定。取数 SHALL 由用户显式动作触发（选中模型、翻页、刷新），MUST NOT 引入轮询或事件订阅。

#### Scenario: 清单与计数

- **WHEN** 打开 DB 查看页
- **THEN** 呈现全部模型清单（含计数），计数为 0 的模型呈现空态而非错误

#### Scenario: 分页扫描与单条查看

- **WHEN** 选中某模型并翻页，随后点开一条记录
- **THEN** 分页区间内记录以信封（key/value）呈现，翻页拼接不重不漏；单条记录 JSON 完整可读

#### Scenario: 只读边界

- **WHEN** 审查 DbInspectorView 与 db 轨道命令
- **THEN** 无任何写操作入口，全部取数经用户显式动作触发、无轮询

#### Scenario: 查询失败 inline 持久

- **WHEN** `db_models` / `db_records` reject
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
