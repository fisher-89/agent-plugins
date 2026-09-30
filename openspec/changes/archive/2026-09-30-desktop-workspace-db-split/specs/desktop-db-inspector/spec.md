# desktop-db-inspector 变更（desktop-workspace-db-split）

## MODIFIED Requirements

### Requirement: 只读查看面

DB 查看页 SHALL 提供只读查看面，包含四件套：

- **模型清单**：经 `db_models` 按 **scope**（全局库 / 当前 workspace 库）列出对应库全部已注册模型及其记录计数——全局库呈现 user 维度模型，workspace 库呈现 workspace 维度模型，两库清单互不混列
- **分页扫描**：选中模型后经 `db_records` 以 scope + offset / limit 分页扫描对应库的记录
- **单条查看**：点开单条记录以 JSON 呈现完整内容（key 与 value 信封）
- **空态与错误态**：模型计数为 0 呈现空态；命令 reject 呈现 inline 持久错误（沿用「错误呈现双轨」查询轨语义）；scope 指向的 workspace 库打开失败同样以 inline 持久错误呈现，不白屏

scope 切换 SHALL 由用户显式动作触发；workspace 库 scope 的寻址基准为当前选定 workspace root（欢迎态无 root 时不可达，见「系统工具组入口与可达性」既有约束）。查看面 MUST NOT 提供任何写操作入口（新增 / 修改 / 删除记录），写操作待真实 debug 需求出现后由后续变更裁定。取数 SHALL 由用户显式动作触发（切换 scope、选中模型、翻页、刷新），MUST NOT 引入轮询或事件订阅。

#### Scenario: 双库 scope 清单与计数

- **WHEN** 在 DB 查看页分别切换全局库与 workspace 库 scope
- **THEN** 全局库 scope 仅呈现 `WorkspaceRecord` 等 user 维度模型，workspace 库 scope 仅呈现 run / 事件 / explore 等 workspace 维度模型，各自计数准确、互不混列

#### Scenario: 分页扫描与单条查看

- **WHEN** 选中某模型并翻页，随后点开一条记录
- **THEN** 当前 scope 对应库的分页区间记录以信封（key/value）呈现，翻页拼接不重不漏；单条记录 JSON 完整可读

#### Scenario: scope 切换不串库

- **WHEN** workspace A 处于选定态，切换 scope 至 workspace 库后再切回全局库
- **THEN** 两个 scope 各自呈现所属库内容，切换过程无跨库数据串显

#### Scenario: 只读边界

- **WHEN** 审查 DbInspectorView 与 db 轨道命令
- **THEN** 无任何写操作入口，全部取数经用户显式动作触发、无轮询

#### Scenario: 查询失败 inline 持久

- **WHEN** `db_models` / `db_records` reject（含 workspace 库打开失败）
- **THEN** 查看页 inline 持久呈现错误（无 toast 顶替），不白屏

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::db` | db 查看命令轨道（只读） | `db_models(scope)` / `db_records(scope, model, offset, limit)`：scope 寻址全局库或当前 workspace 库，经 `WorkspaceStores` 解析对应实例调用信封 API；`Result<T, String>` 错误模板，reject 显式呈现 |
| `DbInspectorView`（前端） | DB 查看页 | scope 切换（全局库 / 当前 workspace 库）+ 模型清单 + 分页扫描 + 单条 JSON 查看；空态 / inline 持久错误态；零模型特定代码、零写入口、零轮询 |
| 记录信封 API（store） | 模型层通用读面 | `list_models()` / `scan(model, offset, limit)` 按实例生效；JSON Value 信封；native_db 类型不越信封（约束不变） |
